//! The only place commands are sent to the server through.
//! Makes sure the bot never spends mushrooms and adds random pauses between actions.

use std::{sync::Mutex, time::Duration};

use chrono::{DateTime, Local};
use sf_api::{
    command::{Command, FortunePayment},
    error::SFError,
    gamestate::{GameState, ShopPosition, character::Mount, items::ItemCommandIdent},
};

use crate::session::SimpleSession;

/// Margin after a cooldown ends before we may act (Arena, Dungeons) – server and local clocks can differ.
pub const COOLDOWN_SAFETY_SEC: i64 = 30;

/// Commands the bot may send. Everything else is forbidden (whitelist),
/// so a new command never reaches the server until it is deliberately added here.
/// None of them spends mushrooms (`Fight`/`FightDungeon` only off cooldown, see `cooldown_free`;
/// `SellShop` costs nothing, `Equip` only moves an item from the backpack onto the character,
/// Wheel of Fortune only `FreeTurn` and only when a free spin is available,
/// `BuyShop` only for an item with no mushroom price, see `shop_buy_ok`).
fn is_allowed(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::Update
            | Command::ExpeditionStart { .. }
            | Command::ExpeditionPickEncounter { .. }
            | Command::ExpeditionContinue
            | Command::ExpeditionPickReward { .. }
            | Command::CheckArena
            | Command::ViewPlayer { .. }
            | Command::Fight { use_mushroom: false, .. }
            | Command::UpdateDungeons
            | Command::FightDungeon { use_mushroom: false, .. }
            | Command::SellShop { .. }
            | Command::BuyShop { .. }
            | Command::Equip { .. }
            | Command::StartWork { .. }
            | Command::FinishWork
            | Command::CollectCalendar
            | Command::SpinWheelOfFortune { payment: FortunePayment::FreeTurn }
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
static LAST_FIGHT: Mutex<Vec<(Cooldown, DateTime<Local>)>> = Mutex::new(Vec::new());

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
        Command::SpinWheelOfFortune { .. } => Some(Cooldown::Wheel),
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

/// Random pause between actions (human-like behaviour).
pub async fn human_pause() {
    tokio::time::sleep(Duration::from_millis(fastrand::u64(2500..7000))).await;
}

/// Sends the command if it is allowed, then waits a moment.
pub async fn send(session: &mut SimpleSession, cmd: Command) -> Result<&mut GameState, SFError> {
    send_raw(session, cmd).await?;
    session.game_state_mut().ok_or(SFError::EmptyResponse)
}

/// Guild commands sf-api does not know (`Command::Custom`, captured from the browser, docs/guild.md).
/// None of them spends mushrooms; leaving the guild only with our own player id (never kicking anyone else).
fn custom_allowed(gs: Option<&GameState>, cmd_name: &str, arguments: &[String]) -> bool {
    match (cmd_name, arguments) {
        ("GroupJoinList", [page]) => page.parse::<u32>().is_ok(),
        ("GroupJoin", [_, suffix]) => suffix == "int",
        ("GroupRemoveMember", [id]) => gs.is_some_and(|gs| *id == gs.character.player_id.to_string()),
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
    if matches!(cmd, Command::BuyMount { .. }) && !session.game_state().is_some_and(crate::stable::needs_mount) {
        return Err(SFError::InvalidRequest("character already has a mount, buying would waste mushrooms"));
    }
    // How many mushrooms this command may spend (only renting a mount, exactly its price)
    let allowed_spend = match &cmd {
        Command::BuyMount { mount } => u32::from(mount.cost().mushrooms),
        _ => 0,
    };

    let mushrooms_before = session.game_state().map(|gs| gs.character.mushrooms);
    let res = session.send_raw(cmd).await;
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
