//! Hall of Fame hunt for Gleeman/event fight tasks (user 2026-10-07): "win N fights against <class>",
//! "win fights bare-handed" (weapon off), "win fights without a chest plate" (chest plate off).
//! Hall of Fame fights do not count towards the 10 Arena wins for XP, but the tasks come first (user 2026-10-10): the hunt
//! runs BEFORE the Arena XP fights whenever such a task is open and the Arena is free (same cooldown). Opponent: from the
//! very bottom of the Hall of Fame upwards (level 1 players live there), lowest honor, wanted class, clearly weaker.

use sf_api::{
    command::{AttributeType, Command},
    gamestate::{GameState, character::Class, items::EquipmentSlot, rewards::TaskType},
};

use crate::{
    arena::{self, power},
    safe,
    session::SimpleSession,
    tavern::Outcome,
};

/// How many Hall of Fame pages (51 players each) to look through, starting at the very bottom and going up.
const BOTTOM_PAGES: u32 = 6;
/// How many of the lowest-honor players of one page to inspect (each one is a `ViewPlayer`).
const INSPECT: usize = 3;
/// The scrapbook hunt looks further: more pages and more players per page (each one is a `ViewPlayer`).
const SCRAPBOOK_PAGES: u32 = 10;
const SCRAPBOOK_INSPECT: usize = 6;
/// Task hunts that ride along with the scrapbook: players inspected per page and pages scanned for a missing item.
const COMBINED_INSPECT: usize = 5;
const COMBINED_PAGES: u32 = 3;
/// After a scrapbook search that found nobody, wait this long before the next one.
const SCRAPBOOK_RETRY: chrono::Duration = chrono::Duration::minutes(30);

static NEXT_SCRAPBOOK: crate::ctx::PerChar<Option<chrono::DateTime<chrono::Local>>> = crate::ctx::PerChar::new();

/// A spare Arena fight for the scrapbook: all XP wins of the day done, the Arena free, a scrapbook that is not complete,
/// and the last fruitless search long enough ago.
fn scrapbook_due(gs: &GameState) -> bool {
    let incomplete = gs.character.scrapbook.as_ref().is_some_and(|sb| {
        let s = crate::scrapbook::stats(sb);
        s.owned < s.total
    });
    incomplete
        && usize::from(gs.arena.fights_for_xp) >= arena::MAX_WINS_PER_DAY
        && !arena::rumble_unlimited(gs)
        && NEXT_SCRAPBOOK.lock().is_ok_and(|n| n.is_none_or(|t| chrono::Local::now() >= t))
}

fn scrapbook_wanted(gs: &GameState) -> bool {
    scrapbook_due(gs) && safe::arena_is_free(gs)
}

/// Is any hunt (fight task, attack-me, scrapbook) waiting for the Arena cooldown? The main loop then wakes up when the
/// cooldown ends instead of idling for 30 minutes (found 2026-10-12: with the 10 XP wins done the Arena timer was ignored and
/// the event hunts ran once every 30 min).
pub fn wants_arena(gs: &GameState) -> bool {
    !wanted_hunts(gs).is_empty() || crate::attack_me::target().is_some() || scrapbook_due(gs)
}

/// What the fight has to look like.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Hunt {
    Class(Class),
    /// Fight with these equipment slots empty
    Without(EquipmentSlot),
    /// Fight with every epic/legendary item taken off (user 2026-10-10)
    NoEpics,
    /// Any clearly weaker Hall of Fame opponent: "win N fights in the Hall of Fame" and "win back to back"
    /// (Ruffian event, user 2026-10-12). Back to back = the very weakest ones, a loss would reset the streak.
    Any { streak: bool },
    /// Spare Arena fight (all 10 XP wins done, no task hunt open): an opponent wearing items our scrapbook still lacks
    /// (user 2026-10-12). Tasks and events always go first.
    Scrapbook,
    /// "Attack me" mode: the user's main character is the opponent, ahead of everything else
    AttackMe,
}

impl Hunt {
    /// Required opponent power relative to ours: without a weapon we are much weaker.
    fn max_ratio(self) -> f64 {
        match self {
            Hunt::Class(_) => 0.6,
            Hunt::Without(EquipmentSlot::Weapon) => 0.3,
            Hunt::Without(_) | Hunt::NoEpics => 0.5,
            Hunt::Any { streak: true } => 0.3,
            Hunt::Any { streak: false } => 0.6,
            Hunt::Scrapbook => 0.6,
            Hunt::AttackMe => 1.0,
        }
    }
}

/// Event hunts count as urgent when the event ends within this many hours; before that the Arena XP wins go first.
const EVENT_URGENT_HOURS: i64 = 24;

/// Every kind of hunt an open task asks for, in task order – daily tasks first, then event tasks (the first one that
/// fits in the backpack is fought). Event tasks step aside for the Arena XP wins while those are still due, unless the
/// event ends soon (user 2026-10-12: "if the event tasks are not burning, Arena XP comes first").
fn wanted_hunts(gs: &GameState) -> Vec<Hunt> {
    let t = &gs.specials.tasks;
    let urgent = t.event.end.is_some_and(|e| (e - chrono::Local::now()).num_hours() < EVENT_URGENT_HOURS);
    let event_goes = urgent || !arena::ready(gs);
    // A task list whose chests are ALL opened has nothing more to give (user 2026-10-12: Mrožik and Květoš had the last event
    // chest, so the remaining event quest need not be done): its tasks are not hunted any more.
    let daily_open = t.daily.rewards.iter().any(|c| !c.opened);
    let event_open = t.event.rewards.iter().any(|c| !c.opened);
    let daily = t.daily.tasks.iter().filter(|_| daily_open);
    let event = t.event.tasks.iter().filter(|_| event_goes && event_open);
    daily.chain(event).filter(|t| !t.is_completed()).filter_map(|t| match t.typ {
        TaskType::WinFightsAgainst(c) => Some(Hunt::Class(c)),
        TaskType::WinFightsBareHands => Some(Hunt::Without(EquipmentSlot::Weapon)),
        TaskType::WinFightsNoChestplate => Some(Hunt::Without(EquipmentSlot::BreastPlate)),
        TaskType::WinFightsNoEpicsLegendaries => Some(Hunt::NoEpics),
        TaskType::WinFightsInHoF => Some(Hunt::Any { streak: false }),
        TaskType::WinFightsBackToBack => Some(Hunt::Any { streak: true }),
        _ => None,
    })
    .collect()
}

/// Our own power by the Arena formula.
pub fn own_power(gs: &GameState) -> f64 {
    let ch = &gs.character;
    power(ch.class, |a: AttributeType| f64::from(ch.attribute_basis[a] + ch.attribute_additions[a]))
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[hunt] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Finds a clearly weaker opponent for the hunt. Returns the name.
async fn find_opponent(session: &mut SimpleSession, hunt: Hunt) -> Result<Option<String>, Outcome> {
    // "Attack me" mode (user 2026-10-12): look the user's main up and attack him, so his honor and rank sink
    if hunt == Hunt::AttackMe {
        let Some(target) = crate::attack_me::target() else { return Ok(None) };
        let gs = safe::send(session, Command::ViewPlayer { ident: target.to_string() }).await.map_err(|e| fail(&e))?;
        let Some(p) = gs.lookup.lookup_name(target) else {
            report!("[hunt] Attack-me: {target} not found");
            return Ok(None);
        };
        report!("[hunt] Attack-me: {target} (rank {}, honor {}, lvl {}, {:?})", p.rank, p.honor, p.level, p.class);
        return Ok(Some(target.to_string()));
    }
    let Some(gs) = session.game_state() else { return Err(Outcome::Done) };
    let (our_rank, our_level, ours) = (gs.character.rank, u32::from(gs.character.level), own_power(gs));
    let mut total = gs.hall_of_fames.players_total;
    let limit = ours * hunt.max_ratio();
    if total <= our_rank {
        // Size of the Hall of Fame not known yet: one page near our own rank tells it
        let page = (our_rank.saturating_sub(26) / 51) as usize;
        let gs = safe::send(session, Command::HallOfFamePage { page }).await.map_err(|e| fail(&e))?;
        total = gs.hall_of_fames.players_total.max(our_rank);
    }
    // From the very bottom upwards: the lowest ranks are level 1 players, a clearly weaker opponent is always there
    // Fight tasks (win vs a class, bare hands, in the Hall of Fame…) ride along with the scrapbook (user 2026-10-12: "it combines
    // nicely"): look through a few pages and take the clearly weaker opponent wearing the most missing items. Only the
    // back-to-back streak (it must not be lost) keeps the plain weakest-first search.
    let combined = !matches!(hunt, Hunt::Scrapbook | Hunt::AttackMe | Hunt::Any { streak: true });
    let (pages, inspect) = match hunt {
        Hunt::Scrapbook => (SCRAPBOOK_PAGES, SCRAPBOOK_INSPECT),
        _ if combined => (BOTTOM_PAGES, COMBINED_INSPECT),
        _ => (BOTTOM_PAGES, INSPECT),
    };
    let mut best: Option<(usize, String, String)> = None;
    for k in 0..pages {
        let target = total.saturating_sub(25 + 51 * k);
        if target <= our_rank || target < 26 {
            break;
        }
        let page = ((target - 26) / 51) as usize;
        let gs = safe::send(session, Command::HallOfFamePage { page }).await.map_err(|e| fail(&e))?;
        let mut list: Vec<_> = gs
            .hall_of_fames
            .players
            .iter()
            .filter(|p| {
                p.level <= our_level
                    && match hunt {
                        Hunt::Class(c) => p.class == c,
                        Hunt::Without(_) | Hunt::NoEpics | Hunt::Any { .. } | Hunt::Scrapbook | Hunt::AttackMe => true,
                    }
            })
            .map(|p| (p.honor, p.name.clone()))
            .collect();
        list.sort();
        // Among the clearly weaker candidates of this page the one wearing the most items our scrapbook still lacks wins
        // (user 2026-10-12: tasks first, but the scrapbook may ride along); a scrapbook hunt needs at least one.
        for (_, name) in list.into_iter().take(inspect) {
            let gs = safe::send(session, Command::ViewPlayer { ident: name.clone() }).await.map_err(|e| fail(&e))?;
            let Some(p) = gs.lookup.lookup_name(&name) else { continue };
            let s = power(p.class, |a| arena::total(p, a));
            if s > limit {
                continue;
            }
            let missing = gs.character.scrapbook.as_ref().map_or(0, |sb| crate::scrapbook::missing_on(&p.equipment, sb));
            if hunt == Hunt::Scrapbook && missing == 0 {
                continue;
            }
            if best.as_ref().is_none_or(|b| missing > b.0) {
                let desc = format!("Found {name} (rank {}, lvl {}, {:?}): power {s:.0}, ours {ours:.0}", p.rank, p.level, p.class);
                best = Some((missing, name, desc));
            }
        }
        // Done with this page? A combined hunt keeps looking for a page with a missing item for up to COMBINED_PAGES pages
        let enough = match &best {
            None => false,
            Some((missing, ..)) => !combined || *missing > 0 || k + 1 >= COMBINED_PAGES,
        };
        if enough
            && let Some((missing, name, desc)) = best.take()
        {
            report!("[hunt] {desc}");
            if missing > 0 {
                report!("[hunt] {name} wears {missing} item(s) missing from our scrapbook");
            }
            return Ok(Some(name));
        }
    }
    Ok(best.map(|(_, name, desc)| {
        report!("[hunt] {desc}");
        name
    }))
}

/// The equipment slots to empty for the fight (`None` = nothing to take off for this kind of hunt).
/// An Assassin carries a weapon in both the Weapon and Shield slot (sf-api types the off-hand item as a Weapon too),
/// so "bare hands" must take both off, not just the main hand.
fn strip_slots(gs: Option<&GameState>, hunt: Hunt) -> Option<Vec<EquipmentSlot>> {
    let gs = gs?;
    match hunt {
        Hunt::Class(_) | Hunt::Any { .. } | Hunt::Scrapbook | Hunt::AttackMe => None,
        Hunt::Without(slot) => {
            let mut slots = vec![slot];
            if slot == EquipmentSlot::Weapon && gs.character.class == Class::Assassin {
                slots.push(EquipmentSlot::Shield);
            }
            Some(slots)
        }
        Hunt::NoEpics => Some(
            gs.character.equipment.0.iter().filter(|(_, i)| i.as_ref().is_some_and(|i| i.is_epic())).map(|(s, _)| s).collect(),
        ),
    }
}

/// Makes sure the items this hunt takes off fit into the backpack, freeing slots with potions when short (drink a
/// better/stacking one or sell the least important, `potions::make_room`). `false` = does not fit, try another hunt.
async fn ensure_room(session: &mut SimpleSession, hunt: Hunt) -> Result<bool, Outcome> {
    let Some(slots) = strip_slots(session.game_state(), hunt) else { return Ok(true) };
    let count = |s: &SimpleSession| s.game_state().map_or(0, |gs| gs.character.inventory.count_free_slots());
    let mut free = count(session);
    // E.g. an Assassin needs 2 slots for bare hands; Sanek's fight tasks stayed blocked all day with 1 free slot
    while free < slots.len() {
        if !crate::potions::make_room(session).await? {
            break;
        }
        free = count(session);
    }
    if free < slots.len() {
        let held = session.game_state().map_or_else(String::new, |gs| {
            gs.character.inventory.backpack.iter().flatten().map(crate::inventory::detail).collect::<Vec<_>>().join("; ")
        });
        report!("[hunt] {hunt:?}: backpack has {free} free slot(s), {} needed to take the items off, skipping (holding: {held})", slots.len());
        return Ok(false);
    }
    Ok(true)
}

/// Uncapped per-character journal of hunts (`roster/<character>/logs/hunts.log`): the progress log keeps only 100
/// messages, too few to judge how often the scrapbook hunt finds an opponent and how many items a win adds.
fn journal(line: &str) {
    use std::io::Write;
    let path = crate::ctx::log_path("hunts.log");
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {line}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

fn scrap_count(session: &SimpleSession) -> usize {
    session.game_state().and_then(|gs| gs.character.scrapbook.as_ref().map(|s| s.items.len())).unwrap_or(0)
}

/// One hunt fight if a fight task is open and the Arena is free (tasks go before the XP fights, user 2026-10-10).
pub async fn run(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let mut hunts = wanted_hunts(gs);
    if crate::attack_me::target().is_some() {
        hunts.insert(0, Hunt::AttackMe); // priority while the app switch is on (user 2026-10-12)
    }
    if hunts.is_empty() && scrapbook_wanted(gs) {
        hunts.push(Hunt::Scrapbook);
    }
    if hunts.is_empty() || !safe::arena_is_free(gs) {
        return Outcome::Done;
    }
    let mut chosen = None;
    for hunt in hunts {
        match ensure_room(session, hunt).await {
            Ok(true) => {
                chosen = Some(hunt);
                break;
            }
            Ok(false) => {}
            Err(o) => return o,
        }
    }
    let Some(hunt) = chosen else { return Outcome::Done };
    let name = match find_opponent(session, hunt).await {
        Ok(Some(n)) => n,
        Ok(None) => {
            report!("[hunt] {hunt:?}: nobody suitable in the Hall of Fame right now (no clearly weaker player{})", if hunt == Hunt::Scrapbook { " wearing items we lack" } else { "" });
            journal(&format!("{hunt:?} | no clearly weaker opponent"));
            if hunt == Hunt::Scrapbook
                && let Ok(mut n) = NEXT_SCRAPBOOK.lock()
            {
                *n = Some(chrono::Local::now() + SCRAPBOOK_RETRY);
            }
            return Outcome::Done;
        }
        Err(o) => return o,
    };

    // Take the item(s) off (weapon / chest plate) into free backpack slots. An Assassin carries a weapon in
    // both the Weapon and Shield slot (sf-api types the off-hand item as a Weapon too), so "bare hands" must
    // take both off, not just the main hand.
    let mut stripped: Vec<(EquipmentSlot, sf_api::gamestate::items::BagPosition, sf_api::gamestate::items::ItemCommandIdent)> =
        Vec::new();
    if let Some(slots) = strip_slots(session.game_state(), hunt) {
        // (room for all of them was made in `ensure_room` before the opponent search)
        for slot in slots {
            let Some(gs) = session.game_state() else { return Outcome::Done };
            let Some(item) = gs.character.equipment.0[slot].as_ref() else { continue };
            let Some(bag) = gs.character.inventory.free_slot() else {
                report!(
                    "[hunt] Backpack full, cannot take off the {slot:?} (backpack {} slots, {} free, {} already stripped)",
                    gs.character.inventory.backpack.len(),
                    gs.character.inventory.count_free_slots(),
                    stripped.len()
                );
                // Put back whatever was already stripped this run before giving up
                for (slot, bag, item_ident) in stripped {
                    let _ = safe::send(session, Command::Equip { from_pos: bag.into(), to_slot: slot, item_ident }).await;
                }
                return Outcome::Done;
            };
            let item_ident = item.command_ident();
            report!("[hunt] Taking off the {slot:?}");
            if let Err(e) =
                safe::send(session, Command::PlayerItemMove { from: slot.into(), to: bag.into(), item_ident }).await
            {
                return fail(&e);
            }
            stripped.push((slot, bag, item_ident));
        }
    }

    let scrap_before = scrap_count(session);
    report!("[hunt] Fighting {name}");
    let mut result = "failed";
    let session_lost = match safe::send(session, Command::Fight { name: name.clone(), use_mushroom: false }).await {
        Ok(gs) => {
            match &gs.last_fight {
                Some(f) => {
                    result = if f.has_player_won { "win" } else { "loss" };
                    report!("[hunt] {} against {name}", if f.has_player_won { "Win" } else { "Loss" });
                }
                None => {
                    result = "no result";
                    report!("[hunt] Fight done, the server sent no result");
                }
            }
            false
        }
        Err(e) => {
            report!("[hunt] Fight failed: {e}");
            crate::tavern::is_session_error(&e)
        }
    };

    let mut scrap = String::new();
    if hunt == Hunt::Scrapbook {
        if safe::send(session, Command::ViewScrapbook).await.is_ok() {
            let after = scrap_count(session);
            report!("[hunt] Scrapbook: {scrap_before} -> {after} items");
            scrap = format!(" | scrapbook {scrap_before} -> {after}");
        }
    }
    journal(&format!("{hunt:?} | vs {name} | {result}{scrap}"));

    // Always put the item(s) back on
    for (slot, bag, item_ident) in stripped {
        report!("[hunt] Putting the {slot:?} back on");
        if let Err(e) = safe::send(session, Command::Equip { from_pos: bag.into(), to_slot: slot, item_ident }).await {
            // inventory::manage will equip it on the next pass anyway (better than an empty slot)
            return fail(&e);
        }
    }
    if session_lost { Outcome::SessionLost } else { Outcome::Done }
}
