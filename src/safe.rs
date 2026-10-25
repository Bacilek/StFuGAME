//! The only place commands are sent to the server through.
//! Makes sure the bot never spends mushrooms and adds random pauses between actions.

use std::time::Duration;

use chrono::{DateTime, Local};
use sf_api::{
    command::{BlacksmithAction, Command, FortunePayment, TimeSkip},
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
            | Command::ViewGuild { .. }
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
            // Ident 20/1 = Pets themselves (user capture 2026-10-24: `unlockfeature:20/1/` pending at login, free), see pets::unlock_pets
            | Command::UnlockFeature { unlockable: sf_api::gamestate::unlockables::Unlockable { main_ident: 20, sub_ident: 1 } }
            // Ident 21 = a new pet after an egg was picked (sub ident = pet id 1..=100, user 2026-10-23: the game client sends it when
            // the habitat is opened; free), see pets::unlock_new
            | Command::UnlockFeature { unlockable: sf_api::gamestate::unlockables::Unlockable { main_ident: 21, sub_ident: 1..=100 } }
            | Command::FightDungeon { use_mushroom: false, .. }
            // Pets (user 2026-10-23, src/pets.rs): feeding costs only fruits (checked in `send_raw`: an owned pet), the habitat dungeon
            // fight is free once an hour (`Cooldown::PetDungeon`), never with `use_mush`
            | Command::PetFeed { .. }
            | Command::FightPetDungeon { use_mush: false, .. }
            // Pet PvP (user 2026-10-24, `pets::pvp`): no mushroom parameter, so it is guarded by `pet_pvp_ok` (free time passed, the offered
            // opponent, a habitat that has not fought today and owns a pet)
            | Command::FightPetOpponent { .. }
            | Command::SellShop { .. }
            // The witch's cauldron takes the place of a shop sale for the item type she wants today (checked in `send_raw`)
            | Command::WitchDropCauldron { .. }
            // The Blacksmith takes a backpack item apart instead of a shop sale (user 2026-10-24, src/smith.rs); free. Socket upgrades,
            // gem extraction and item upgrades (and every `...WithMushrooms` variant) stay forbidden; checked in `send_raw` (`dismantle_ok`)
            | Command::Blacksmith { action: BlacksmithAction::Dismantle, .. }
            // Toilet of the Arcane Gods (user 2026-10-25, src/toilet.rs): costs no mushrooms in the captures; the sacrifice only for a
            // backpack epic / gem (`toilet_drop_ok`), the flush only with enough mana and a free slot (`toilet_flush_ok`)
            | Command::ToiletDrop { .. }
            | Command::ToiletFlush
            | Command::BuyShop { .. }
            | Command::Equip { .. }
            | Command::CollectDailyQuestReward { .. }
            | Command::CollectEventTaskReward { .. }
            | Command::UpgradeSkill { .. }
            | Command::GuildIncreaseSkill { skill: GuildSkill::Treasure | GuildSkill::Instructor, .. }
            | Command::GambleSilver { .. }
            // Beer only during the Exceptional XP event (user 2026-10-16), see tasks::beer_justified
            | Command::BuyBeer
            | Command::HallOfFamePage { .. }
            // Read-only refresh of the scrapbook (user 2026-10-12, scrapbook hunt)
            | Command::ViewScrapbook
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
            // Fortress (user 2026-10-11, src/fortress.rs): only the agreed buildings, never mushrooms
            | Command::FortressBuild { f_type: sf_api::gamestate::fortress::FortressBuildingType::Fortress | sf_api::gamestate::fortress::FortressBuildingType::LaborersQuarters | sf_api::gamestate::fortress::FortressBuildingType::WoodcuttersHut | sf_api::gamestate::fortress::FortressBuildingType::Quarry | sf_api::gamestate::fortress::FortressBuildingType::Treasury | sf_api::gamestate::fortress::FortressBuildingType::GemMine | sf_api::gamestate::fortress::FortressBuildingType::Barracks | sf_api::gamestate::fortress::FortressBuildingType::Academy }
            | Command::FortressBuildFinish { f_type: sf_api::gamestate::fortress::FortressBuildingType::Fortress | sf_api::gamestate::fortress::FortressBuildingType::LaborersQuarters | sf_api::gamestate::fortress::FortressBuildingType::WoodcuttersHut | sf_api::gamestate::fortress::FortressBuildingType::Quarry | sf_api::gamestate::fortress::FortressBuildingType::Treasury | sf_api::gamestate::fortress::FortressBuildingType::GemMine | sf_api::gamestate::fortress::FortressBuildingType::Barracks | sf_api::gamestate::fortress::FortressBuildingType::Academy, mushrooms: 0 }
            | Command::FortressGather { .. }
            // Hall of Knights (user 2026-10-12): instant, only while no build runs; `send_raw` checks `fortress::hok_affordable`
            | Command::FortressUpgradeHallOfKnights
            // Soldiers (user 2026-10-12, src/fortress.rs `military`): train soldiers only, attack with exactly ONE soldier, counter-attack
            // targets. the paid `FortressNewEnemy` stays forbidden. None of these takes mushrooms.
            | Command::FortressBuildUnit { unit: sf_api::gamestate::fortress::FortressUnitType::Soldier, .. }
            | Command::FortressAttack { soldiers: 1 | 2 }
            | Command::FortressSetCAEnemy { .. }
            // The FREE change of the opponent only (user 2026-10-12, once per 5 min); `send_raw` checks `reroll_is_free`. The paid one
            // (gold or a mushroom) stays forbidden.
            | Command::FortressNewEnemy { use_mushroom: false }
            // Gem Mine (user 2026-10-12, src/gems.rs): start a search, collect it WITHOUT mushrooms, socket a gem (the move itself is
            // checked in `send_raw`: only a Gem from the backpack onto an equipped item that has a gem slot)
            | Command::FortressGemStoneSearch
            | Command::FortressGemStoneSearchFinish { mushrooms: 0 }
            | Command::PlayerItemMove {
                from: PlayerItemPosition { place: PlayerItemPlace::MainInventory | PlayerItemPlace::ExtendedInventory, .. },
                to: PlayerItemPosition { place: PlayerItemPlace::Equipment, .. },
                ..
            }
            // a gem onto a stored backpack item that has an empty gem slot (`gem_socket_ok` checks it)
            | Command::PlayerItemMove {
                from: PlayerItemPosition { place: PlayerItemPlace::MainInventory | PlayerItemPlace::ExtendedInventory, .. },
                to: PlayerItemPosition { place: PlayerItemPlace::MainInventory | PlayerItemPlace::ExtendedInventory, .. },
                ..
            }
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
    /// Free change of the fortress attack opponent (once per ~5 min; otherwise it costs gold, user 2026-10-12)
    Reroll,
    /// Habitat dungeon fight with a pet, free once an hour for all habitats together (user 2026-10-21)
    PetDungeon,
    /// Pet PvP fight against the offered player, free again 15 min after the last one (user 2026-10-24; `petsSave` 232)
    PetPvp,
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
        Command::FightPetDungeon { .. } => Some(Cooldown::PetDungeon),
        Command::FightPetOpponent { .. } => Some(Cooldown::PetPvp),
        // Only the free spin has a cooldown; lucky-coin spins are guarded by `tasks::lucky_spin_justified`
        Command::SpinWheelOfFortune { payment: FortunePayment::FreeTurn } => Some(Cooldown::Wheel),
        Command::FortressNewEnemy { use_mushroom: false } => Some(Cooldown::Reroll),
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

/// Is the free change of the fortress attack opponent available (the server-given time has passed and is newer than our last change)?
pub fn reroll_is_free(gs: &GameState) -> bool {
    gs.fortress.as_ref().is_some_and(|f| f.attack_free_reroll.is_some() && cooldown_free(Cooldown::Reroll, f.attack_free_reroll))
}

/// Is the pets' habitat dungeon free? `gs.pets.next_free_exploration` comes with every `Update` and fight response.
pub fn pet_dungeon_is_free(gs: &GameState) -> bool {
    gs.pets.as_ref().is_some_and(|p| p.next_free_exploration.is_some() && cooldown_free(Cooldown::PetDungeon, p.next_free_exploration))
}

/// May this pet PvP fight start? Raw `petsSave`: the opponent offered is index 231, the next free fight 232, the habitat's "fought today"
/// flag 223 + habitat; the habitat must own a pet (an empty one would just be a sure loss). Otherwise it would cost a mushroom.
fn pet_pvp_ok(nums: Option<&[i64]>, habitat: sf_api::gamestate::unlockables::HabitatType, opponent: u32) -> bool {
    let Some(n) = nums else { return false };
    let h = habitat as usize;
    n.len() > 240
        && opponent > 0
        && n[231] == i64::from(opponent)
        && n[223 + h] == 0
        && (h * 20 + 2..h * 20 + 22).any(|i| n[i] > 0)
        && cooldown_free(Cooldown::PetPvp, crate::pets::pvp_free_at(n))
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

/// A gem from the backpack onto an equipped item that has a gem slot.
fn gem_socket_ok(gs: &GameState, to: PlayerItemPosition, item_ident: ItemCommandIdent) -> bool {
    use sf_api::gamestate::items::ItemType;
    let is_gem = gs.character.inventory.backpack.iter().flatten().any(|i| i.command_ident() == item_ident && matches!(i.typ, ItemType::Gem(_)));
    let has_slot = if to.place == PlayerItemPlace::Equipment {
        gs.character.equipment.0.iter().any(|(slot, item)| {
            slot as usize - 1 == to.position && item.as_ref().is_some_and(|i| i.gem_slot.is_some())
        })
    } else {
        // a stored backpack item with a still EMPTY gem slot
        gs.character.inventory.iter().any(|(pos, item)| {
            PlayerItemPosition::from(pos) == to
                && item.is_some_and(|i| matches!(i.gem_slot, Some(sf_api::gamestate::items::GemSlot::Empty)))
        })
    };
    is_gem && has_slot
}

/// Guild skill upgrade only for gold (from higher levels it also costs mushrooms) and only at the current level.
fn guild_upgrade_ok(gs: &GameState, skill: GuildSkill, current: u16) -> bool {
    gs.guild.as_ref().is_some_and(|g| {
        let own = if skill == GuildSkill::Instructor { g.own_instructor_skill } else { g.own_treasure_skill };
        g.upgrade_price[skill].mushrooms == 0 && g.upgrade_price[skill].silver <= gs.character.silver && own == current
    }) || crate::guild::six_price_ok(gs, skill, current)
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
/// The cauldron takes only a BACKPACK item (never a worn one) of the type the witch wants today, never an epic (user 2026-10-19).
fn witch_drop_ok(gs: &GameState, pos: PlayerItemPosition) -> bool {
    let Some(w) = gs.witch.as_ref() else { return false };
    let Some(wanted) = w.required_item else { return false };
    gs.character.level >= 66
        && gs.character.inventory.iter().any(|(bp, item)| {
            PlayerItemPosition::from(bp) == pos
                && item.is_some_and(|i| !i.is_unique() && i.typ.equipment_slot() == Some(wanted))
        })
}

/// The Blacksmith dismantles only a BACKPACK equipment item (never a worn one, never a unique), from character level 90 (user 2026-10-24).
fn dismantle_ok(gs: &GameState, pos: PlayerItemPosition, ident: ItemCommandIdent) -> bool {
    gs.character.level >= crate::smith::MIN_LEVEL
        && gs.character.inventory.iter().any(|(bp, item)| {
            PlayerItemPosition::from(bp) == pos
                && item.is_some_and(|i| i.command_ident() == ident && !i.is_unique() && i.typ.equipment_slot().is_some())
        })
}

/// The toilet takes only a BACKPACK epic equipment piece or gem (never a potion or a plain item), see `toilet::throwable`.
fn toilet_drop_ok(gs: &GameState, pos: PlayerItemPosition) -> bool {
    gs.tavern.toilet.is_some()
        && gs.character.inventory.iter().any(|(bp, item)| PlayerItemPosition::from(bp) == pos && item.is_some_and(crate::toilet::throwable))
}

/// A flush only when the toilet has the mana for it and the new item has a backpack slot.
fn toilet_flush_ok(gs: &GameState) -> bool {
    gs.tavern.toilet.as_ref().is_some_and(|t| t.mana_total > 0 && t.mana_currently >= t.mana_total)
        && gs.character.inventory.free_slot().is_some()
}

/// Highest enchantment price the witch may charge (silver = copper units, 10 000 gold).
pub const WITCH_MAX_PRICE: u64 = 1_000_000;

/// Price of the next enchantment in the raw `witchshop` (sf-api's own witch state was missing for Filminy/Chlamydie on 2026-10-22 and
/// the enchanting was refused as "not whitelisted"), so the raw number counts too.
fn raw_witch_price(session: &SimpleSession) -> Option<u64> {
    session.witch_shop().and_then(crate::witch::parse).map(|w| w.price_silver)
}

fn custom_allowed(gs: Option<&GameState>, raw_price: Option<u64>, cmd_name: &str, arguments: &[String]) -> bool {
    match (cmd_name, arguments) {
        ("GroupJoinList", [page]) => page.parse::<u32>().is_ok(),
        ("GroupJoin", [_, suffix]) => suffix == "int",
        ("GroupRemoveMember", [id]) => gs.is_some_and(|gs| *id == gs.character.player_id.to_string()),
        ("ShopCatalog", [shop, mid, page]) => shop == "1" && mid.is_empty() && page == "1",
        ("ShopCheckout", [shop, identifier, suffix]) => shop == "1" && suffix.is_empty() && !identifier.is_empty(),
        // Witch enchanting (user 2026-10-19): the character's OWN worn item (flag "1", never a companion), the ident is the 1-based position
        // of the enchantment in the `witchshop` list (1..=9), costs only the gold price the server announces (at most `WITCH_MAX_PRICE`)
        ("PlayerWitchEnchantItem", [ident, own]) => {
            own == "1"
                && ident.parse::<u8>().is_ok_and(|i| (1..=9).contains(&i))
                && gs.is_some_and(|gs| {
                    gs.character.level >= 66
                        && gs.witch.as_ref().map(|w| w.enchantment_price).into_iter().chain(raw_price).any(|price| {
                            price > 0 && price <= WITCH_MAX_PRICE && price <= gs.character.silver
                        })
                })
        }
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
        Command::Custom { cmd_name, arguments } => custom_allowed(session.game_state(), raw_witch_price(session), cmd_name, arguments),
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
        Some(Cooldown::PetDungeon) if !session.game_state().is_some_and(pet_dungeon_is_free) => {
            return Err(SFError::InvalidRequest("the pets' habitat dungeon is on cooldown"));
        }
        Some(Cooldown::PetPvp) if !matches!(&cmd, Command::FightPetOpponent { habitat, opponent_id } if pet_pvp_ok(session.pets_save(), *habitat, *opponent_id)) => {
            return Err(SFError::InvalidRequest("pet PvP: on cooldown, not the offered opponent, or the habitat already fought today"));
        }
        Some(Cooldown::Wheel) if !session.game_state().is_some_and(wheel_is_free) => {
            return Err(SFError::InvalidRequest("no free Wheel of Fortune spin available"));
        }
        Some(Cooldown::Reroll) if !session.game_state().is_some_and(reroll_is_free) => {
            return Err(SFError::InvalidRequest("the free change of the fortress opponent is not available, it would cost gold"));
        }
        _ => {}
    }
    // Feeding: only an owned pet (level > 0 in the raw `petsSave`, index = pet id + 1) and one of the 5 fruit kinds
    if let Command::PetFeed { pet_id, fruit_idx } = &cmd
        && !(*fruit_idx <= 5 && session.pets_save().is_some_and(|n| (1..=100).contains(pet_id) && n.get(*pet_id as usize + 1).is_some_and(|l| *l > 0)))
    {
        return Err(SFError::InvalidRequest("pets: only an owned pet may be fed"));
    }
    if let Command::WitchDropCauldron { item_pos } = &cmd
        && !session.game_state().is_some_and(|gs| witch_drop_ok(gs, *item_pos))
    {
        return Err(SFError::InvalidRequest("cauldron: not a plain backpack item of the type the witch wants today"));
    }
    if let Command::ToiletDrop { item_pos } = &cmd
        && !session.game_state().is_some_and(|gs| toilet_drop_ok(gs, *item_pos))
    {
        return Err(SFError::InvalidRequest("toilet: only a backpack epic or gem may be sacrificed"));
    }
    if matches!(cmd, Command::ToiletFlush) && !session.game_state().is_some_and(toilet_flush_ok) {
        return Err(SFError::InvalidRequest("toilet: not enough mana or no free backpack slot"));
    }
    if let Command::Blacksmith { item_pos, item_ident, .. } = &cmd
        && !session.game_state().is_some_and(|gs| dismantle_ok(gs, *item_pos, *item_ident))
    {
        return Err(SFError::InvalidRequest("blacksmith: only a backpack equipment item may be dismantled, from level 90"));
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
    // Fortress buildings: the Gem Mine only to level 1 until character level 125 (user 2026-10-11), see fortress::build_ok
    if let Command::FortressBuild { f_type } | Command::FortressBuildFinish { f_type, .. } = &cmd
        && !session.game_state().is_some_and(|gs| crate::fortress::build_ok(gs, *f_type))
    {
        return Err(SFError::InvalidRequest("this fortress building may not be built now"));
    }
    if matches!(cmd, Command::FortressUpgradeHallOfKnights) && !session.game_state().is_some_and(crate::fortress::hok_affordable) {
        return Err(SFError::InvalidRequest("the Hall of Knights is not affordable now or a build is running"));
    }
    // Moves out of the backpack are only for socketing a gem into an item with a gem slot, worn or stored (src/gems.rs); equipping is `Command::Equip`
    if let Command::PlayerItemMove { from, to, item_ident } = &cmd
        && from.place != PlayerItemPlace::Equipment
        && !session.game_state().is_some_and(|gs| gem_socket_ok(gs, *to, *item_ident))
    {
        return Err(SFError::InvalidRequest("only a gem may be moved onto an item with a gem slot"));
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
        return Err(SFError::InvalidRequest("beer costs a mushroom and is only allowed during the Exceptional XP event"));
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
        Command::GuildIncreaseSkill { skill, current } => session
            .game_state()
            .filter(|gs| crate::guild::six_price_ok(gs, *skill, *current))
            .and_then(|gs| gs.guild.as_ref())
            .map_or(0, |g| u32::from(g.upgrade_price[*skill].mushrooms)),
        _ => 0,
    };

    let mushrooms_before = session.game_state().map(|gs| gs.character.mushrooms);
    let silver_before = session.game_state().map(|gs| gs.character.silver);
    let xp_before = session.game_state().map(|gs| (gs.character.level, gs.character.experience, gs.character.next_level_xp));
    let desc = crate::stuck::describe(&cmd);
    let res = session.send_raw(cmd).await;
    match &res {
        Ok(_) => crate::stuck::after(&desc, true, None),
        Err(e) if !crate::tavern::is_session_error(e) => crate::stuck::after(&desc, false, Some(&e.to_string())),
        Err(_) => {}
    }
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
        Command::Custom { cmd_name, arguments } => custom_allowed(session.game_state(), raw_witch_price(session), cmd_name, arguments),
        _ => false,
    };
    if !custom_ok {
        return Err(SFError::InvalidRequest("command is not whitelisted (mushroom protection)"));
    }
    let res = session.send_raw_only(cmd).await;
    human_pause().await;
    res
}

#[cfg(test)]
mod fortress_tests {
    use super::*;
    use sf_api::gamestate::fortress::{FortressBuildingType as B, FortressResourceType as R};

    #[test]
    fn fortress_commands_never_cancel_or_pay_with_mushrooms() {
        // allowed: the agreed buildings, finishing with 0 mushrooms, collecting
        assert!(is_allowed(&Command::FortressBuild { f_type: B::Fortress }));
        assert!(is_allowed(&Command::FortressBuildFinish { f_type: B::Quarry, mushrooms: 0 }));
        assert!(is_allowed(&Command::FortressGather { resource: R::Wood }));
        // never: skipping with mushrooms, cancelling, the Wall, buildings nobody agreed to, gem search finish
        assert!(!is_allowed(&Command::FortressBuildFinish { f_type: B::Fortress, mushrooms: 1 }));
        assert!(!is_allowed(&Command::FortressBuildCancel { f_type: B::Fortress }));
        assert!(!is_allowed(&Command::FortressBuild { f_type: B::Wall }));
        // the Gem Mine passes the whitelist but `fortress::build_ok` limits it to level 1 below character level 125
        assert!(is_allowed(&Command::FortressBuild { f_type: B::GemMine }));
        assert!(is_allowed(&Command::FortressBuild { f_type: B::Barracks }));
        assert!(is_allowed(&Command::FortressBuild { f_type: B::Academy }));
        assert!(!is_allowed(&Command::FortressBuild { f_type: B::Smithy }));
        assert!(!is_allowed(&Command::FortressGemStoneSearchFinish { mushrooms: 1 }));
        assert!(is_allowed(&Command::FortressUpgradeHallOfKnights)); // guarded by `hok_affordable`
        // soldiers: train soldiers, one-soldier attacks, counter-attack targets; never a reroll, never other units or bigger armies
        use sf_api::gamestate::fortress::FortressUnitType as U;
        assert!(is_allowed(&Command::FortressBuildUnit { unit: U::Soldier, count: 3 }));
        assert!(!is_allowed(&Command::FortressBuildUnit { unit: U::Archer, count: 1 }));
        assert!(is_allowed(&Command::FortressAttack { soldiers: 1 }));
        assert!(is_allowed(&Command::FortressAttack { soldiers: 2 })); // user 2026-10-14: 1-2 soldiers per raid
        assert!(!is_allowed(&Command::FortressAttack { soldiers: 3 }));
        assert!(is_allowed(&Command::FortressSetCAEnemy { msg_id: 5 }));
        assert!(is_allowed(&Command::FortressNewEnemy { use_mushroom: false })); // free change only, guarded by `reroll_is_free`
        assert!(!is_allowed(&Command::FortressNewEnemy { use_mushroom: true }));
        // gems: start and collect (only without mushrooms)
        assert!(is_allowed(&Command::FortressGemStoneSearch));
        assert!(is_allowed(&Command::FortressGemStoneSearchFinish { mushrooms: 0 }));
    }
}

#[cfg(test)]
mod blacksmith_tests {
    use super::*;

    /// Only dismantling is whitelisted: no gem socket upgrades, extraction, item upgrades or anything paid with mushrooms.
    #[test]
    fn only_dismantling_passes() {
        let pos = PlayerItemPosition { place: PlayerItemPlace::MainInventory, position: 0 };
        let ident = crate::smith::test_item(sf_api::gamestate::items::ItemType::Belt, 1).command_ident();
        for (action, ok) in [
            (BlacksmithAction::Dismantle, true),
            (BlacksmithAction::SocketUpgrade, false),
            (BlacksmithAction::SocketUpgradeWithMushrooms, false),
            (BlacksmithAction::GemExtract, false),
            (BlacksmithAction::GemExtractWithMushrooms, false),
            (BlacksmithAction::Upgrade, false),
        ] {
            assert_eq!(is_allowed(&Command::Blacksmith { item_pos: pos, action, item_ident: ident }), ok, "{action:?}");
        }
    }
}

#[cfg(test)]
mod guild_six_tests {
    use super::*;

    #[test]
    fn only_instructor_and_treasure_may_be_upgraded() {
        assert!(is_allowed(&Command::GuildIncreaseSkill { skill: GuildSkill::Instructor, current: 5 }));
        assert!(is_allowed(&Command::GuildIncreaseSkill { skill: GuildSkill::Treasure, current: 5 }));
        // the Pet skill is never bought (not part of the "6/6" the guilds ask for)
        assert!(!is_allowed(&Command::GuildIncreaseSkill { skill: GuildSkill::Pet, current: 5 }));
        // 6 -> 7 is the second priced step; the price caps are checked by `guild::six_price_ok`
        assert!(is_allowed(&Command::GuildIncreaseSkill { skill: GuildSkill::Instructor, current: 6 }));
    }
}

#[cfg(test)]
mod witch_tests {
    use super::*;

    fn args(a: &str, b: &str) -> Vec<String> {
        vec![a.to_string(), b.to_string()]
    }

    /// The enchant request passes only for the own item (flag 1), idents 1..=9, level 66+ and a price the server announced and we can pay.
    #[test]
    fn enchant_needs_level_price_and_the_own_flag() {
        let mut gs = GameState::default();
        assert!(!custom_allowed(Some(&gs), None, "PlayerWitchEnchantItem", &args("2", "1")), "no witch yet");
        gs.character.level = 67;
        gs.character.silver = 300_000;
        gs.witch = Some(Default::default());
        assert!(!custom_allowed(Some(&gs), None, "PlayerWitchEnchantItem", &args("2", "1")), "no price known");
        gs.witch.as_mut().unwrap().enchantment_price = 194_000;
        assert!(custom_allowed(Some(&gs), None, "PlayerWitchEnchantItem", &args("2", "1")));
        assert!(!custom_allowed(Some(&gs), None, "PlayerWitchEnchantItem", &args("2", "101")), "a companion is not allowed");
        assert!(!custom_allowed(Some(&gs), None, "PlayerWitchEnchantItem", &args("10", "1")));
        assert!(!custom_allowed(Some(&gs), None, "PlayerWitchEnchantItem", &args("0", "1")));
        gs.character.silver = 100_000;
        assert!(!custom_allowed(Some(&gs), None, "PlayerWitchEnchantItem", &args("2", "1")), "not enough gold");
        gs.character.silver = 5_000_000;
        gs.witch.as_mut().unwrap().enchantment_price = WITCH_MAX_PRICE + 1;
        assert!(!custom_allowed(Some(&gs), None, "PlayerWitchEnchantItem", &args("2", "1")), "price above the cap");
    }
}
