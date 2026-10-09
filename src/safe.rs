//! The only place commands are sent to the server through.
//! Makes sure the bot never spends mushrooms and adds random pauses between actions.

use std::time::Duration;

use chrono::{DateTime, Local};
use sf_api::{
    command::{Command, FortunePayment, TimeSkip},
    error::SFError,
    gamestate::{
        GameState, ShopPosition,
        character::Mount,
        guild::GuildSkill,
        items::{ItemCommandIdent, PlayerItemPlace, PlayerItemPosition},
    },
};

use crate::session::SimpleSession;

/// Margin after a cooldown ends before we may act (Arena, Dungeons) – server and local clocks can differ.
pub const COOLDOWN_SAFETY_SEC: i64 = 30;

/// Commands the bot may send. Everything else is forbidden (whitelist),
/// so a new command never reaches the server until it is deliberately added here.
/// None of them spends mushrooms (`Fight`/`FightDungeon` only off cooldown, see `cooldown_free`;
/// `SellShop` costs nothing, `Equip` only moves an item from the backpack onto the character,
/// Wheel of Fortune only `FreeTurn` and only when a free spin is available,
/// `BuyShop` only for an item with no mushroom price, see `shop_buy_ok`;
/// `GuildJoinAttack`/`GuildJoinDefense` only sign up for a planned guild battle, free;
/// `UsePotion` only a potion from the backpack, `RemovePotion` only per `potions::removal_ok`;
/// task chests are free; `UpgradeSkill` (attributes) costs only gold; `GuildIncreaseSkill` only when its price has
/// no mushrooms (`guild_upgrade_ok`); `GambleSilver` = shell game for gold, within the game limits (`gamble_ok`);
/// `ExpeditionSkipWait` only with `TimeSkip::Glass` (hourglass, replenishable, never `Mushroom`)).
fn is_allowed(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::Update
            | Command::ExpeditionStart { .. }
            | Command::ExpeditionPickEncounter { .. }
            | Command::ExpeditionContinue
            | Command::ExpeditionPickReward { .. }
            // Hourglass-only expedition skip, to avoid wasting bonus Thirst for Adventure right
            // before the midnight reset (user 2026-10-08, see tavern::should_skip_wait_with_glass).
            // TimeSkip::Mushroom stays forbidden.
            | Command::ExpeditionSkipWait { typ: TimeSkip::Glass }
            | Command::CheckArena
            | Command::ViewPlayer { .. }
            | Command::Fight { use_mushroom: false, .. }
            | Command::UpdateDungeons
            // Reward of a redeemed coupon waiting in the Mail (user 2026-10-09); only a Coupon-type mail, see `claimable_ok`
            | Command::ClaimablePreview { .. }
            | Command::ClaimableClaim { .. }
            // Dungeon unlock (user 2026-10-09: the game client itself sends `UnlockFeature` `30/1` when a character
            // with a pending dungeon unlock opens the Dungeons screen; nothing to pay), see dungeons::unlock_pending.
            // Ident 5 = Scrapbook, 9 = beginners task list (user 2026-10-09, wanted on every character).
            // Other idents stay forbidden until the user says what they are.
            | Command::UnlockFeature { unlockable: sf_api::gamestate::unlockables::Unlockable { main_ident: 30 | 5 | 9, .. } }
            | Command::FightDungeon { use_mushroom: false, .. }
            | Command::SellShop { .. }
            | Command::BuyShop { .. }
            | Command::Equip { .. }
            | Command::CollectDailyQuestReward { .. }
            | Command::CollectEventTaskReward { .. }
            | Command::UpgradeSkill { .. }
            | Command::GuildIncreaseSkill { skill: GuildSkill::Treasure | GuildSkill::Instructor, .. }
            | Command::GambleSilver { .. }
            // Beer only for the last missing Gleeman task to a chest with mushrooms (user 2026-10-07)
            | Command::BuyBeer
            | Command::HallOfFamePage { .. }
            // Taking an item off (equipment → backpack) for "bare hands"/"no chest plate" tasks, see hunt.rs
            | Command::PlayerItemMove {
                from: PlayerItemPosition { place: PlayerItemPlace::Equipment, .. },
                to: PlayerItemPosition { place: PlayerItemPlace::MainInventory | PlayerItemPlace::ExtendedInventory, .. },
                ..
            }
            | Command::UsePotion { .. }
            | Command::RemovePotion { .. }
            | Command::GuildJoinAttack
            | Command::GuildJoinDefense
            | Command::StartWork { .. }
            | Command::FinishWork
            | Command::CollectCalendar
            | Command::SpinWheelOfFortune { payment: FortunePayment::FreeTurn }
            // Lucky coins only when a task chest needs the spins (user 2026-10-07), see `tasks::plan`
            | Command::SpinWheelOfFortune { payment: FortunePayment::LuckyCoins }
            // The only allowed exception to the mushroom rule (user, 2026-10-07), only without a mount
            | Command::BuyMount { mount: Mount::Dragon | Mount::Tiger }
    )
}

/// Kind of action with a cooldown. On cooldown it would cost mushrooms (the server ignores `use_mushroom`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cooldown {
    Arena,
    Dungeon,
    /// Free Wheel of Fortune spin (otherwise it would cost mushrooms/lucky coins)
    Wheel,
}

/// When we last used each cooldown action (during this run of the bot).
static LAST_FIGHT: crate::ctx::PerChar<Vec<(Cooldown, DateTime<Local>)>> = crate::ctx::PerChar::new();

fn last_fight(kind: Cooldown) -> Option<DateTime<Local>> {
    LAST_FIGHT.lock().ok()?.iter().find(|(k, _)| *k == kind).map(|(_, t)| *t)
}

fn remember_fight(kind: Cooldown) {
    if let Ok(mut v) = LAST_FIGHT.lock() {
        v.retain(|(k, _)| *k != kind);
        v.push((kind, Local::now()));
    }
}

/// Is the action free? The cooldown end (+ margin) must be in the past and, if we already acted,
/// the server must have sent a NEW cooldown end since then (later than our action). Otherwise the
/// state could be stale (e.g. the Dungeons timer is only refreshed by UpdateDungeons) and it would cost mushrooms.
pub fn cooldown_free(kind: Cooldown, next_free: Option<DateTime<Local>>) -> bool {
    let last = last_fight(kind);
    match next_free {
        None => last.is_none(),
        Some(t) => Local::now() >= t + chrono::Duration::seconds(COOLDOWN_SAFETY_SEC) && last.is_none_or(|l| t > l),
    }
}

fn fight_kind(cmd: &Command) -> Option<Cooldown> {
    match cmd {
        Command::Fight { .. } => Some(Cooldown::Arena),
        Command::FightDungeon { .. } => Some(Cooldown::Dungeon),
        // Only the free spin has a cooldown; lucky-coin spins are guarded by `tasks::lucky_spin_justified`
        Command::SpinWheelOfFortune { payment: FortunePayment::FreeTurn } => Some(Cooldown::Wheel),
        _ => None,
    }
}

/// Is the Arena free (including the margin)? On cooldown a fight would cost a mushroom
/// (the server ignores `use_mushroom` and always fights).
pub fn arena_is_free(gs: &GameState) -> bool {
    cooldown_free(Cooldown::Arena, gs.arena.next_free_fight)
}

/// Is a free Wheel of Fortune spin available? Without a known time (None) we do not spin.
pub fn wheel_is_free(gs: &GameState) -> bool {
    gs.specials.wheel.next_free_spin.is_some() && cooldown_free(Cooldown::Wheel, gs.specials.wheel.next_free_spin)
}

/// Are the Dungeons free? Only valid for a state fresh after `UpdateDungeons`.
pub fn dungeon_is_free(gs: &GameState) -> bool {
    cooldown_free(Cooldown::Dungeon, gs.dungeons.next_free_fight)
}

/// May this shop item be bought? Only when it costs no mushrooms, is still exactly the item we looked at
/// (`item_ident` contains both prices, the server also checks it) and we have the gold for it.
fn shop_buy_ok(gs: &GameState, shop_pos: ShopPosition, item_ident: ItemCommandIdent) -> bool {
    gs.shops[shop_pos.typ].items.get(shop_pos.pos).is_some_and(|item| {
        item.mushroom_price == 0 && item.command_ident() == item_ident && u64::from(item.price) <= gs.character.silver
    })
}

/// Guild skill upgrade only for gold (from higher levels it also costs mushrooms) and only at the current level.
fn guild_upgrade_ok(gs: &GameState, skill: GuildSkill, current: u16) -> bool {
    gs.guild.as_ref().is_some_and(|g| {
        let own = if skill == GuildSkill::Instructor { g.own_instructor_skill } else { g.own_treasure_skill };
        g.upgrade_price[skill].mushrooms == 0 && g.upgrade_price[skill].silver <= gs.character.silver && own == current
    })
}

/// Shell game: only with at least 5 gold and a bet of at most 1/10 of the gold.
fn gamble_ok(gs: &GameState, amount: u64) -> bool {
    let silver = gs.character.silver;
    amount > 0 && silver >= crate::tasks::GAMBLE_MIN_SILVER && amount <= silver / 10
}

/// Random pause between actions (human-like behaviour).
pub async fn human_pause() {
    tokio::time::sleep(Duration::from_millis(fastrand::u64(2500..7000))).await;
}

/// Sends the command if it is allowed, then waits a moment.
pub async fn send(session: &mut SimpleSession, cmd: Command) -> Result<&mut GameState, SFError> {
    send_raw(session, cmd).await?;
    session.game_state_mut().ok_or(SFError::EmptyResponse)
}

/// Commands sf-api does not know (`Command::Custom`, captured straight from a browser Network tab).
/// Guild ones (docs/guild.md): none of them spend mushrooms; leaving the guild only with our own player id
/// (never kicking anyone else). Shop ones (docs/daily-rewards.md, the free "new customer" pack): `ShopCatalog`
/// is read-only; `ShopCheckout`'s real safety (never checking out anything that costs real money/mushrooms) is
/// enforced by the caller (`daily::claim_welcome_pack`), which only ever passes an identifier whose catalog price
/// it just confirmed was exactly 0 – this whitelist only pins the shop id ("1") and the request shape.
fn custom_allowed(gs: Option<&GameState>, cmd_name: &str, arguments: &[String]) -> bool {
    match (cmd_name, arguments) {
        ("GroupJoinList", [page]) => page.parse::<u32>().is_ok(),
        ("GroupJoin", [_, suffix]) => suffix == "int",
        ("GroupRemoveMember", [id]) => gs.is_some_and(|gs| *id == gs.character.player_id.to_string()),
        ("ShopCatalog", [shop, mid, page]) => shop == "1" && mid.is_empty() && page == "1",
        ("ShopCheckout", [shop, identifier, suffix]) => shop == "1" && suffix.is_empty() && !identifier.is_empty(),
        // Marks/un-marks a fight as saved (shows up in Quarter -> Mail), free UI action, both directions
        // confirmed from real captured responses (docs/arena-highlights.md: "1" = mark, "0" = un-mark). Only
        // ever an id already present in our own combat log (src/arena_highlights.rs), never an arbitrary id.
        ("PlayerCombatLogMark", [id, flag]) => {
            (flag == "1" || flag == "0") && gs.is_some_and(|gs| gs.mail.combat_log.iter().any(|e| e.msg_id.to_string() == *id))
        }
        _ => false,
    }
}

/// Like `send`, but returns the raw server response (for keys sf-api ignores).
pub async fn send_raw(session: &mut SimpleSession, cmd: Command) -> Result<String, SFError> {
    let custom_ok = match &cmd {
        Command::Custom { cmd_name, arguments } => custom_allowed(session.game_state(), cmd_name, arguments),
        _ => false,
    };
    if !is_allowed(&cmd) && !custom_ok {
        return Err(SFError::InvalidRequest("command is not whitelisted (mushroom protection)"));
    }
    let kind = fight_kind(&cmd);
    match kind {
        Some(Cooldown::Arena) if !session.game_state().is_some_and(arena_is_free) => {
            return Err(SFError::InvalidRequest("Arena is on cooldown, a fight would cost a mushroom"));
        }
        Some(Cooldown::Dungeon) if !session.game_state().is_some_and(dungeon_is_free) => {
            return Err(SFError::InvalidRequest("Dungeons are on cooldown, a fight would cost a mushroom"));
        }
        Some(Cooldown::Wheel) if !session.game_state().is_some_and(wheel_is_free) => {
            return Err(SFError::InvalidRequest("no free Wheel of Fortune spin available"));
        }
        _ => {}
    }
    if let Command::BuyShop { shop_pos, item_ident, .. } = &cmd
        && !session.game_state().is_some_and(|gs| shop_buy_ok(gs, *shop_pos, *item_ident))
    {
        return Err(SFError::InvalidRequest("shop item costs mushrooms or changed, not buying"));
    }
    if let Command::ClaimablePreview { msg_id } | Command::ClaimableClaim { msg_id } = &cmd
        && !session.game_state().is_some_and(|gs| {
            gs.mail.claimables.iter().any(|m| {
                m.msg_id == *msg_id && m.typ == sf_api::gamestate::social::ClaimableMailType::Coupon
            })
        })
    {
        return Err(SFError::InvalidRequest("only the reward of a coupon mail may be viewed or claimed"));
    }
    if let Command::GuildIncreaseSkill { skill, current } = &cmd
        && !session.game_state().is_some_and(|gs| guild_upgrade_ok(gs, *skill, *current))
    {
        return Err(SFError::InvalidRequest("guild upgrade costs mushrooms or is not affordable"));
    }
    if matches!(cmd, Command::SpinWheelOfFortune { payment: FortunePayment::LuckyCoins })
        && !session.game_state().is_some_and(crate::tasks::lucky_spin_justified)
    {
        return Err(SFError::InvalidRequest("lucky coins only when a task chest needs the spins"));
    }
    if let Command::UsePotion { item_ident, .. } = &cmd
        && !session.game_state().is_some_and(|gs| {
            gs.character.inventory.backpack.iter().flatten().any(|i| {
                i.command_ident() == *item_ident && matches!(i.typ, sf_api::gamestate::items::ItemType::Potion(_))
            })
        })
    {
        return Err(SFError::InvalidRequest("only a potion from the backpack can be drunk"));
    }
    if let Command::RemovePotion { pos } = &cmd
        && !session.game_state().is_some_and(|gs| crate::potions::removal_ok(gs, *pos))
    {
        return Err(SFError::InvalidRequest("this active potion may not be removed"));
    }
    if matches!(cmd, Command::BuyBeer) && !session.game_state().is_some_and(crate::tasks::beer_justified) {
        return Err(SFError::InvalidRequest("beer costs a mushroom and is not justified by a task chest"));
    }
    if let Command::GambleSilver { amount } = &cmd
        && !session.game_state().is_some_and(|gs| gamble_ok(gs, *amount))
    {
        return Err(SFError::InvalidRequest("shell game bet outside the allowed limits"));
    }
    if matches!(cmd, Command::BuyMount { .. }) && !session.game_state().is_some_and(crate::stable::needs_mount) {
        return Err(SFError::InvalidRequest("character already has a mount, buying would waste mushrooms"));
    }
    // How many mushrooms this command may spend (only renting a mount, exactly its price)
    let allowed_spend = match &cmd {
        Command::BuyMount { mount } => u32::from(mount.cost().mushrooms),
        Command::BuyBeer => crate::tasks::BEER_MUSHROOMS,
        _ => 0,
    };

    let mushrooms_before = session.game_state().map(|gs| gs.character.mushrooms);
    let silver_before = session.game_state().map(|gs| gs.character.silver);
    let xp_before = session.game_state().map(|gs| (gs.character.level, gs.character.experience, gs.character.next_level_xp));
    let res = session.send_raw(cmd).await;
    // Gold and mushrooms gained (for the daily report of the character challenge)
    if let (Some(s0), Some(m0), Some((l0, x0, next0)), Some(gs)) =
        (silver_before, mushrooms_before, xp_before, session.game_state())
    {
        let c = &gs.character;
        // XP: within a level the difference; after a level-up the rest of the old level + the new progress
        let xp = if c.level > l0 { next0.saturating_sub(x0) + c.experience } else { c.experience.saturating_sub(x0) };
        crate::roster::ledger(c.silver.saturating_sub(s0), c.mushrooms.saturating_sub(m0), xp);
    }
    if let Some(kind) = kind {
        // Even on error: the server may have performed the action, next one only after a new server time
        remember_fight(kind);
    }

    // Last line of defence: if mushrooms decreased anyway, the bot stops immediately
    if let (Some(before), Some(after)) = (mushrooms_before, session.game_state().map(|gs| gs.character.mushrooms))
        && after + allowed_spend < before
    {
        report!("!!! MUSHROOMS DECREASED ({before} → {after}, allowed {allowed_spend}). Bot stops immediately, investigate!");
        crate::tray::message_box(&format!("MUSHROOMS DECREASED ({before} → {after}). The bot stopped, details in the log."));
        std::process::exit(2);
    }

    human_pause().await;
    res
}

/// Like `send_raw`, but for the shop's custom commands whose response `GameState::update` cannot be trusted to
/// parse (`ShopCatalog`'s is a bare JSON blob; `ShopCheckout`'s is that same JSON shape followed by `&key:value…`
/// pairs – neither is the one shape sf-api's parser expects) – never touches the game state itself, and skips the
/// mushroom watchdog this file otherwise always runs, since there is nothing fresh to compare against here. The
/// caller (`daily::claim_welcome_pack`) always follows up with a normal `Command::Update` (via `send`), which both
/// refreshes the game state and re-triggers that same watchdog, comparing against the state from before this call.
pub async fn send_raw_only(session: &mut SimpleSession, cmd: Command) -> Result<String, SFError> {
    let custom_ok = match &cmd {
        Command::Custom { cmd_name, arguments } => custom_allowed(session.game_state(), cmd_name, arguments),
        _ => false,
    };
    if !custom_ok {
        return Err(SFError::InvalidRequest("command is not whitelisted (mushroom protection)"));
    }
    let res = session.send_raw_only(cmd).await;
    human_pause().await;
    res
}
