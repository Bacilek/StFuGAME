//! Tavern: expeditions with choices (crossroads, boss, rewards, waiting).
//! Strategy: secure 40 heroism (maximum reward) as soon as possible, then farm keys and chests.

use std::time::Duration;

use chrono::{DateTime, Local};
use sf_api::{
    command::{Command, TimeSkip},
    error::SFError,
    gamestate::{
        rewards::{Reward, RewardType, TaskType},
        tavern::{
            AvailableExpedition, AvailableTasks, CurrentAction, Expedition, ExpeditionEncounter, ExpeditionStage,
            ExpeditionThing, Location,
        },
    },
};

use crate::session::SimpleSession;

use crate::{
    journal::Journal,
    missions::{self, Bonus},
    safe,
};

/// From this heroism on the reward is maximal, we need no more.
const MAX_HEROISM: i32 = 40;
/// Bonus for an item we hold a wanted poster (bounty) for.
const BOUNTY_BONUS: i32 = 10;
/// An expedition always has 10 rounds (crossroads).
const LAST_FLOOR: u8 = 10;
/// Average heroism one round yields. A round spent on preparation (a chain step)
/// therefore "costs" what we would otherwise have collected. Tune from the journal.
const OPPORTUNITY_COST: f64 = 4.0;
/// Estimated bonus for the target item of an unknown mission (until it is mapped).
const UNKNOWN_TARGET_GUESS: i32 = 5;
/// The most heroism realistically obtainable in one round (poster + dummy ~13,
/// winner's podium 15). If even that cannot reach 40, chasing points is pointless.
const MAX_GAIN_PER_FLOOR: i32 = 12;
/// Conservative safety margin before the midnight Thirst-for-Adventure reset (user 2026-10-08):
/// if normal waiting would leave less than this much real time before the reset, use an
/// hourglass instead of risking the ALU (and the in-progress expedition) getting wiped at
/// midnight. What exactly happens to an expedition right at the reset is not known/verified.
const MIDNIGHT_SAFETY_MARGIN_SEC: u64 = 15 * 60;

/// True when waiting out the current expedition stage normally would not leave enough real
/// time before the midnight Thirst-for-Adventure reset (see `MIDNIGHT_SAFETY_MARGIN_SEC`).
fn should_skip_wait_with_glass(busy_until: DateTime<Local>, now: DateTime<Local>) -> bool {
    let wait_secs = u64::try_from((busy_until - now).num_seconds()).unwrap_or(0);
    let midnight_secs = crate::guard::secs_until_midnight(now);
    midnight_secs <= wait_secs + MIDNIGHT_SAFETY_MARGIN_SEC
}
/// Safety limit against an endless loop.
const MAX_STEPS: u32 = 300;

/// Picks an expedition: the shortest (cheapest in Thirst for Adventure). At equal length the one where
/// 40 heroism is easiest; an unknown mission (or one with an unverified bonus) wins ties so we can map it.
/// A Gleeman/event task "travel to <location>" comes first (user 2026-10-07): an expedition through a wanted
/// location wins even when it is longer.
fn choose_expedition(list: &[AvailableExpedition], thirst: u32, wanted: &[Location]) -> Option<usize> {
    let visits = |e: &AvailableExpedition| wanted.contains(&e.location_1) || wanted.contains(&e.location_2);
    // Unknown mission or unverified bonus = we want to map it, it wins ties
    let ease = |e: &AvailableExpedition| match missions::for_target(e.target) {
        Some(m) if m.bonus_known => m.ease(),
        _ => f64::INFINITY,
    };
    let mut best: Option<usize> = None;
    for (i, e) in list.iter().enumerate().filter(|(_, e)| e.thirst_for_adventure_sec <= thirst) {
        let better = match best {
            None => true,
            Some(b) => {
                let b = &list[b];
                (visits(e) && !visits(b))
                    || (visits(e) == visits(b) && e.thirst_for_adventure_sec < b.thirst_for_adventure_sec)
                    || (visits(e) == visits(b)
                        && e.thirst_for_adventure_sec == b.thirst_for_adventure_sec
                        && ease(e) > ease(b))
            }
        };
        if better {
            best = Some(i);
        }
    }
    best
}

fn has(exp: &Expedition, t: ExpeditionThing) -> bool {
    exp.items.iter().flatten().any(|i| *i == t)
}

/// How many crossroads remain AFTER the current pick.
fn floors_after(exp: &Expedition) -> u8 {
    LAST_FLOOR.saturating_sub(exp.current_floor)
}

fn target_done(exp: &Expedition) -> bool {
    exp.target_current >= exp.target_amount
}

/// Projected heroism at the end of the expedition if we collect nothing more:
/// current + per-item bonuses (credited at the end) - penalty for an unfinished task.
fn projected_heroism(exp: &Expedition) -> i32 {
    // After the pick in the last round the server has already credited bonuses and penalties (verified 2026-10-07)
    if exp.current_floor >= LAST_FLOOR && !matches!(exp.current_stage(), ExpeditionStage::Encounters(_)) {
        return exp.heroism;
    }
    let mut p = exp.heroism;
    if let Some(m) = missions::for_target(exp.target_thing) {
        if let Bonus::PerItem(b) = m.bonus {
            p += b * i32::from(exp.target_current);
        }
        if !target_done(exp) {
            p -= m.fail_penalty;
        }
    }
    p
}

/// How much this pick immediately changes the projected final heroism.
fn immediate_gain(exp: &Expedition, enc: &ExpeditionEncounter) -> i32 {
    let mut g = enc.heroism;
    if let Some(poster) = enc.typ.required_bounty()
        && has(exp, poster)
    {
        g += BOUNTY_BONUS;
    }
    if enc.typ == exp.target_thing && missions::for_target(exp.target_thing).is_none() {
        g += UNKNOWN_TARGET_GUESS;
    }
    if enc.typ == exp.target_thing
        && let Some(m) = missions::for_target(exp.target_thing)
    {
        match m.bonus {
            Bonus::PerItem(b) => g += b,
            Bonus::OnComplete(b) => {
                if !target_done(exp) && exp.target_current + 1 >= exp.target_amount {
                    g += b + m.fail_penalty;
                }
            }
        }
    }
    g
}

/// Chance of completing `steps` more required steps with `floors` crossroads left.
fn feasibility(steps: u8, floors: u8) -> f64 {
    if steps == 0 {
        1.0
    } else if steps > floors {
        0.0
    } else if floors >= 2 * steps {
        0.7
    } else {
        0.35
    }
}

/// Future value of a pick: what it unlocks (poster, chain step) or brings closer
/// (counting task), weighted by the chance we make it in time. A step that cannot be completed is worth 0.
fn future_value(exp: &Expedition, enc: &ExpeditionEncounter) -> f64 {
    let floors = floors_after(exp);
    let t = enc.typ;

    if t.is_bounty_for().is_some() {
        return if has(exp, t) { 0.0 } else { f64::from(BOUNTY_BONUS) * feasibility(1, floors) };
    }
    let Some((m, idx)) = missions::chain_position(t) else {
        return unknown_chain_value(exp, t, floors);
    };
    let is_target = m.target() == exp.target_thing;
    // Items of foreign cycles: they give nothing at the end, only the immediate gain counts
    if !is_target {
        return 0.0;
    }
    let last_idx = m.chain.len() - 1;

    // Task with a count (e.g. 3× toilet paper): each piece brings us closer to completion
    if idx == last_idx {
        if is_target
            && let Bonus::OnComplete(b) = m.bonus
            && !target_done(exp)
        {
            let steps = exp.target_amount.saturating_sub(exp.target_current + 1);
            if steps > 0 {
                let payoff = f64::from(b + m.fail_penalty) - OPPORTUNITY_COST * f64::from(steps);
                return feasibility(steps, floors) * payoff.max(0.0);
            }
        }
        return 0.0;
    }

    // Chain step: only worth it if we do not have it yet and the chain can still be completed
    let held = m.chain.iter().rposition(|(c, _)| has(exp, *c));
    if held.is_some_and(|h| h >= idx) {
        return 0.0;
    }
    let steps = u8::try_from(last_idx - idx).unwrap_or(u8::MAX);
    let rest: i32 = m.chain[idx + 1..].iter().map(|(_, h)| h).sum();
    let bonus = match (is_target, m.bonus) {
        (false, _) => 0,
        (true, Bonus::PerItem(b)) => b,
        (true, Bonus::OnComplete(b)) if !target_done(exp) => b + m.fail_penalty,
        (true, Bonus::OnComplete(_)) => 0,
    };
    let payoff = f64::from(rest + bonus) - OPPORTUNITY_COST * f64::from(steps);
    feasibility(steps, floors) * payoff.max(0.0)
}

/// Estimate for a step of an unmapped mission. Items of one mission have consecutive sf-api numbers
/// within the same ten (e.g. Mugs 151 → DraftBeer 152 → Barkeeper 153).
fn unknown_chain_value(exp: &Expedition, t: ExpeditionThing, floors: u8) -> f64 {
    let target = exp.target_thing;
    if missions::for_target(target).is_some() || has(exp, t) {
        return 0.0;
    }
    let (t_id, target_id) = (t as i64, target as i64);
    if t_id / 10 != target_id / 10 || t_id >= target_id {
        return 0.0;
    }
    let steps = u8::try_from(target_id - t_id).unwrap_or(u8::MAX);
    let payoff = f64::from(UNKNOWN_TARGET_GUESS * 2) - OPPORTUNITY_COST * f64::from(steps - 1);
    feasibility(steps, floors) * payoff.max(0.0)
}

/// How much heroism should rise right after the pick: encounter points + poster + one-time completion bonus.
/// (Per-item bonuses come at the end and do not belong here.)
fn expected_now(exp: &Expedition, enc: &ExpeditionEncounter) -> i32 {
    let mut g = enc.heroism;
    if let Some(poster) = enc.typ.required_bounty()
        && has(exp, poster)
    {
        g += BOUNTY_BONUS;
    }
    if enc.typ == exp.target_thing
        && let Some(m) = missions::for_target(exp.target_thing)
        && let Bonus::OnComplete(b) = m.bonus
        && !target_done(exp)
        && exp.target_current + 1 >= exp.target_amount
    {
        g += b;
    }
    g
}

/// How much heroism should change at the end (right after the pick in round 10): per-item bonuses,
/// or the penalty for an unfinished task.
fn expected_end_change(exp: &Expedition) -> i32 {
    let Some(m) = missions::for_target(exp.target_thing) else { return 0 };
    let mut d = 0;
    if let Bonus::PerItem(b) = m.bonus {
        d += b * i32::from(exp.target_current);
    }
    if !target_done(exp) {
        d -= m.fail_penalty;
    }
    d
}

/// Option description for the log: server points, total gain (if different) and future value (if any).
fn describe_option(exp: &Expedition, e: &ExpeditionEncounter) -> String {
    let gain = immediate_gain(exp, e);
    let future = future_value(exp, e);
    let mut s = format!("{:?}({:+}", e.typ, e.heroism);
    if gain != e.heroism {
        s += &format!(" ⇒ {gain:+}");
    }
    if future > 0.0 {
        s += &format!(", future {future:.1}");
    }
    s + ")"
}

/// Records a mission data check result. A mismatch is reported right away.
fn check(entry: &mut crate::journal::Entry, ok: bool, what: &str) {
    if ok {
        entry.checks.push(format!("OK {what}"));
    } else {
        report!("[check] MISMATCH: {what}");
        entry.checks.push(format!("MISMATCH {what}"));
    }
}

/// Picks an encounter at the crossroads.
///
/// When 40 is secured (including bonuses and penalties at the end): only takes what does not
/// drop us below 40, preferably a chest (if we hold a key), otherwise a key (if a round is left for a chest).
/// Otherwise: the highest sum of immediate gain and future value.
fn choose_encounter(exp: &Expedition, encs: &[ExpeditionEncounter]) -> usize {
    let projected = projected_heroism(exp);

    if projected >= MAX_HEROISM {
        let safe: Vec<usize> = (0..encs.len())
            .filter(|&i| projected + immediate_gain(exp, &encs[i]) >= MAX_HEROISM)
            .collect();
        let pick = |t: ExpeditionThing| safe.iter().copied().find(|&i| encs[i].typ == t);
        if has(exp, ExpeditionThing::Key)
            && let Some(i) = pick(ExpeditionThing::Suitcase)
        {
            return i;
        }
        if floors_after(exp) > 0
            && let Some(i) = pick(ExpeditionThing::Key)
        {
            return i;
        }
        if let Some(&i) = safe.iter().max_by_key(|&&i| (immediate_gain(exp, &encs[i]), std::cmp::Reverse(i))) {
            return i;
        }
        // Everything drops us below 40: take the smallest loss
        return (0..encs.len())
            .max_by_key(|&i| (immediate_gain(exp, &encs[i]), std::cmp::Reverse(i)))
            .unwrap_or(0);
    }

    // 40 is out of reach: points will not help, take at least a chest/key
    let floors_incl_this = i32::from(floors_after(exp)) + 1;
    if projected + floors_incl_this * MAX_GAIN_PER_FLOOR < MAX_HEROISM {
        if has(exp, ExpeditionThing::Key)
            && let Some(i) = encs.iter().position(|e| e.typ == ExpeditionThing::Suitcase)
        {
            return i;
        }
        if floors_after(exp) > 0
            && let Some(i) = encs.iter().position(|e| e.typ == ExpeditionThing::Key)
        {
            return i;
        }
    }

    let score = |e: &ExpeditionEncounter| f64::from(immediate_gain(exp, e)) + future_value(exp, e);
    let mut best = 0;
    for (i, e) in encs.iter().enumerate() {
        if score(e) > score(&encs[best]) {
            best = i;
        }
    }
    best
}

/// An expedition costing at most this much Thirst for Adventure is a "leftover" one: little gold, but the same hourglasses.
const REDUCED_EXPEDITION_SEC: u32 = 3 * 60;

/// Picks a reward: mushrooms > gold > hourglasses > anything.
/// For a leftover expedition (the rest of the Thirst for Adventure) mushrooms > hourglasses > gold.
fn choose_reward(rewards: &[Reward], reduced: bool) -> usize {
    let rank = |t: &RewardType| match t {
        RewardType::Mushrooms => 0,
        RewardType::Silver if reduced => 2,
        RewardType::Silver => 1,
        RewardType::QuicksandGlass if reduced => 1,
        RewardType::QuicksandGlass => 2,
        _ => 3,
    };
    rewards
        .iter()
        .enumerate()
        .min_by_key(|(i, r)| (rank(&r.typ), *i))
        .map_or(0, |(i, _)| i)
}

/// Is the expedition past the waiting (travel after the boss) and waiting for "continue"?
/// sf-api then shows the old crossroads offer; the server sends the new one only after continuing.
/// The `floor_stage` field (4 = waiting) is not public, we read it via serde.
fn is_after_wait(exp: &Expedition) -> bool {
    serde_json::to_value(exp)
        .ok()
        .and_then(|v| v.get("floor_stage")?.as_i64())
        == Some(4)
}

/// How the Tavern run ended.
pub enum Outcome {
    /// Done, or an error that retrying will not fix.
    Done,
    /// The server invalidated the session, a new login via the account is needed.
    SessionLost,
}

/// The server invalidated the session (sf-api logs in again with the next command).
pub fn is_session_error(e: &SFError) -> bool {
    matches!(e, SFError::ServerError(msg) if msg.contains("sessionid"))
}

fn mission_name(t: ExpeditionThing) -> String {
    match missions::for_target(t) {
        None => format!("{t:?} (unknown mission)"),
        Some(m) if !m.bonus_known => format!("{} (bonus unverified)", m.name),
        Some(m) => m.name.to_string(),
    }
}

/// Plays expeditions while there is Thirst for Adventure. Always waits, never skips.
pub async fn run(session: &mut SimpleSession, journal: &mut Journal) -> Outcome {
    let mut unknown_in_row = 0;
    // sf-api refreshes the expedition state only when the server sends it. So after every game command
    // we download a fresh state and never pick twice from the same (stale) offer.
    let mut refresh_pending = true;
    let mut last_offer: Option<Vec<String>> = None;
    let mut stale_tries = 0;
    // After finishing an expedition we return control (the Arena fits in between expeditions)
    let mut played = false;
    // Arena during an expedition: try at most once a minute (in case a fight did not happen for some reason)
    let mut last_arena_try: Option<std::time::Instant> = None;
    let mut last_dungeon_try: Option<std::time::Instant> = None;
    // Expedition started with the leftover Thirst for Adventure (unknown after a bot restart, then treated as full)
    let mut reduced_expedition = false;
    // Mission data checks during the run: (expected heroism, description)
    let mut pending_check: Option<(i32, String)> = None;
    // Report table mismatches only once per run
    let mut reported = std::collections::HashSet::new();
    // Before the last boss: (heroism, expected change at the end, description)
    let mut end_check: Option<(i32, i32, String)> = None;

    for _ in 0..MAX_STEPS {
        let Some(gs) = session.game_state() else {
            report!("[tavern] Game state missing, stopping");
            return Outcome::Done;
        };
        // New items (from chests, fights): equip better, sell worse. Sends nothing without an action.
        if !refresh_pending {
            let before = gs.character.inventory.count_free_slots();
            if let Outcome::SessionLost = crate::inventory::manage(session).await {
                return Outcome::SessionLost;
            }
            let Some(gs_now) = session.game_state() else { continue };
            if gs_now.character.inventory.count_free_slots() != before {
                refresh_pending = true;
                continue;
            }
        }
        let Some(gs) = session.game_state() else { continue };
        // A mount before a new expedition (bought only now, when it is really needed)
        let about_to_start = !played
            && gs.tavern.current_action == CurrentAction::Idle
            && gs.tavern.expeditions.active().is_none()
            && gs.tavern.thirst_for_adventure_sec > 0;
        if about_to_start && crate::stable::needs_mount(gs) {
            if let Outcome::SessionLost = crate::stable::ensure_mount(session).await {
                return Outcome::SessionLost;
            }
            let Some(gs) = session.game_state() else { continue };
            if !crate::stable::needs_mount(gs) {
                continue;
            }
        }
        let Some(gs) = session.game_state() else { continue };
        if crate::arena::ready(gs) && last_arena_try.is_none_or(|t| t.elapsed().as_secs() >= 60) {
            last_arena_try = Some(std::time::Instant::now());
            if let Outcome::SessionLost = crate::arena::run(session).await {
                return Outcome::SessionLost;
            }
            refresh_pending = true;
            continue;
        }
        if crate::dungeons::secs_until_ready(gs) == 0
            && last_dungeon_try.is_none_or(|t| t.elapsed().as_secs() >= crate::dungeons::RETRY_SEC)
        {
            last_dungeon_try = Some(std::time::Instant::now());
            if let Outcome::SessionLost = crate::dungeons::run(session).await {
                return Outcome::SessionLost;
            }
            refresh_pending = true;
            continue;
        }
        let tavern = &gs.tavern;

        let cmd = if let Some(exp) = tavern.expeditions.active() {
            played = true;
            let entry = journal.entry(&mission_name(exp.target_thing));
            entry.heroism = exp.heroism;
            entry.projected = projected_heroism(exp);
            entry.target_current = exp.target_current;
            entry.target_amount = exp.target_amount;

            let stage = exp.current_stage();
            // After every game command first a fresh state, so we never decide on stale data
            // (e.g. "continue" at the boss means "pick reward 1" to the server)
            if refresh_pending && !matches!(stage, ExpeditionStage::Waiting { .. }) {
                refresh_pending = false;
                if let Err(e) = safe::send(session, Command::Update).await {
                    report!("[tavern] Error: {e}");
                    return if is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
                }
                continue;
            }
            if let Some((expected, what)) = pending_check.take() {
                check(entry, expected == exp.heroism, &format!("{what}: expected {expected}, server has {}", exp.heroism));
            }
            match stage {
                ExpeditionStage::Encounters(_) if is_after_wait(exp) => {
                    unknown_in_row = 0;
                    last_offer = None;
                    stale_tries = 0;
                    report!("[tavern] Waiting over, continuing to the next part of the expedition");
                    Command::ExpeditionContinue
                }
                ExpeditionStage::Encounters(encs) if !encs.is_empty() => {
                    unknown_in_row = 0;
                    let offer: Vec<String> = encs.iter().map(|e| format!("{:?}{}", e.typ, e.heroism)).collect();
                    if last_offer.as_ref() == Some(&offer) && stale_tries < 2 {
                        stale_tries += 1;
                        report!("[tavern] The offer is the same as last time, refreshing the state (attempt {stale_tries})");
                        if let Err(e) = safe::send(session, Command::Update).await {
                            report!("[tavern] Error: {e}");
                            return if is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
                        }
                        continue;
                    }
                    if stale_tries >= 2 {
                        report!("[tavern] The offer stayed the same after refreshing, treating it as real");
                    }
                    stale_tries = 0;
                    for u in encs.iter().filter(|e| !missions::is_known(e.typ)) {
                        let note = format!("{:?}({:+})", u.typ, u.heroism);
                        report!("[tavern] Unmapped encounter: {note}");
                        if !entry.unmapped.contains(&note) {
                            entry.unmapped.push(note);
                        }
                    }
                    let pos = choose_encounter(exp, &encs);
                    for e in &encs {
                        if let Some((m, idx)) = missions::chain_position(e.typ) {
                            let table = m.chain[idx].1;
                            if table != e.heroism && reported.insert(e.typ as i64) {
                                check(entry, false, &format!("{:?} is {table:+} in the table, the server shows {:+}", e.typ, e.heroism));
                            }
                        }
                    }
                    let opts: Vec<String> = encs.iter().map(|e| describe_option(exp, e)).collect();
                    let picked = encs[pos].typ;
                    report!(
                        "[tavern] Round {}/{LAST_FLOOR}, heroism {} (projected end {}), {} {}/{} | {} → taking {picked:?}",
                        exp.current_floor,
                        exp.heroism,
                        projected_heroism(exp),
                        mission_name(exp.target_thing),
                        exp.target_current,
                        exp.target_amount,
                        opts.join(", "),
                    );
                    entry.picks.push(format!("{}: [{}] → {picked:?}", exp.current_floor, opts.join(", ")));
                    let is_resource = |t: ExpeditionThing| matches!(t, ExpeditionThing::Key | ExpeditionThing::Suitcase);
                    if projected_heroism(exp) >= MAX_HEROISM && encs.iter().any(|e| is_resource(e.typ)) && !is_resource(picked) {
                        entry.declined_resources += 1;
                    }
                    match picked {
                        ExpeditionThing::Key => entry.keys += 1,
                        ExpeditionThing::Suitcase => entry.chests += 1,
                        _ => {}
                    }
                    last_offer = Some(offer);
                    let end_change = if exp.current_floor >= LAST_FLOOR {
                        let mut after = exp.clone();
                        if picked == exp.target_thing {
                            after.target_current += 1;
                        }
                        expected_end_change(&after)
                    } else {
                        0
                    };
                    pending_check = Some((
                        exp.heroism + expected_now(exp, &encs[pos]) + end_change,
                        format!("heroism after picking {picked:?} in round {}", exp.current_floor),
                    ));
                    Command::ExpeditionPickEncounter { pos }
                }
                ExpeditionStage::Boss(_) => {
                    unknown_in_row = 0;
                    if exp.current_floor >= LAST_FLOOR {
                        // Per-item bonuses/penalty were credited after the pick in round 10, nothing should change after the boss
                        end_check = Some((
                            exp.heroism,
                            0,
                            format!("heroism change after the last boss ({})", mission_name(exp.target_thing)),
                        ));
                    }
                    report!("[tavern] Boss, fighting");
                    Command::ExpeditionContinue
                }
                ExpeditionStage::Rewards(rewards) if !rewards.is_empty() => {
                    unknown_in_row = 0;
                    let pos = choose_reward(&rewards, reduced_expedition);
                    let reward_text = |r: &Reward| crate::report::reward(&format!("{:?}", r.typ), i64::try_from(r.amount).unwrap_or(i64::MAX));
                    let opts: Vec<String> = rewards.iter().map(reward_text).collect();
                    report!("[tavern] Rewards: {} → taking {:?}", opts.join(", "), rewards[pos].typ);
                    entry.rewards.push(reward_text(&rewards[pos]));
                    Command::ExpeditionPickReward { pos }
                }
                ExpeditionStage::Waiting { busy_until, .. } => {
                    unknown_in_row = 0;
                    let secs = u64::try_from((busy_until - Local::now()).num_seconds()).unwrap_or(0);
                    // Safety valve (user 2026-10-08): normal waiting would leave too little real
                    // time before the midnight Thirst-for-Adventure reset to spend it in the
                    // Tavern (typically bonus ALU from a beer drunk late in the day, near/during
                    // a City Guard shift) – use an hourglass instead of risking it (and this
                    // expedition) getting wiped at the reset. Never a mushroom skip.
                    if should_skip_wait_with_glass(busy_until, Local::now()) && gs.tavern.quicksand_glasses > 0 {
                        report!(
                            "[tavern] Using an hourglass to avoid wasting bonus Thirst for Adventure before midnight ({} left)",
                            gs.tavern.quicksand_glasses
                        );
                        Command::ExpeditionSkipWait { typ: TimeSkip::Glass }
                    } else {
                        let extra = fastrand::u64(5..30);
                        report!(
                            "[tavern] Waiting until {} ({} min {} s)",
                            busy_until.format("%H:%M:%S"),
                            secs / 60,
                            secs % 60
                        );
                        // If the Arena becomes free in the meantime, wake up earlier and fight
                        let mut sleep = secs + extra;
                        if let Some(arena) = crate::arena::secs_until_ready(gs)
                            && arena + 5 < sleep
                        {
                            sleep = arena + fastrand::u64(5..20);
                            report!("[tavern] The Arena becomes free while waiting, waking up in {sleep} s");
                        }
                        let dungeon = crate::dungeons::secs_until_ready(gs);
                        if dungeon + 5 < sleep {
                            sleep = dungeon + fastrand::u64(5..20);
                            report!("[tavern] The Dungeons become free while waiting, waking up in {sleep} s");
                        }
                        tokio::time::sleep(Duration::from_secs(sleep)).await;
                        Command::Update
                    }
                }
                _ => {
                    unknown_in_row += 1;
                    if unknown_in_row > 2 {
                        report!("[tavern] Unknown expedition state, stopping");
                        return Outcome::Done;
                    }
                    Command::Update
                }
            }
        } else {
            if let Some((before, delta, what)) = end_check.take() {
                let raw = serde_json::to_value(&tavern.expeditions).ok();
                let after = raw.as_ref().and_then(|v| v.get("active")?.get("heroism")?.as_i64());
                if let (Some(after), Some(entry)) = (after, journal.current.as_mut()) {
                    let after = i32::try_from(after).unwrap_or(i32::MIN);
                    check(entry, after - before == delta, &format!("{what}: expected {delta:+}, server {:+}", after - before));
                    entry.heroism = after;
                    entry.projected = after;
                }
            }
            journal.finish();
            match tavern.current_action {
                // The last timer ran out, the expedition needs to be closed
                CurrentAction::Expedition => {
                    unknown_in_row += 1;
                    if unknown_in_row > 2 {
                        report!("[tavern] Could not close the expedition, stopping");
                        return Outcome::Done;
                    }
                    // Diagnostics: what the server knows about the expedition (mainly whether a reward choice is pending)
                    let raw = serde_json::to_value(&tavern.expeditions).ok();
                    let active = raw.as_ref().and_then(|v| v.get("active"));
                    let field = |k: &str| active.and_then(|a| a.get(k)).map(ToString::to_string).unwrap_or_default();
                    report!(
                        "[tavern] State before closing: floor_stage {}, round {}, rewards {}",
                        field("floor_stage"),
                        field("current_floor"),
                        field("rewards")
                    );
                    // "Continue" means "pick reward 1" to the server. If sf-api shows rewards (maybe
                    // stale ones from the 1st boss), pick by priority – it cannot be worse.
                    let rewards: Vec<Reward> = active
                        .and_then(|a| a.get("rewards").cloned())
                        .and_then(|r| serde_json::from_value(r).ok())
                        .unwrap_or_default();
                    report!("[tavern] Closing the finished expedition");
                    if rewards.is_empty() {
                        Command::ExpeditionContinue
                    } else {
                        Command::ExpeditionPickReward { pos: choose_reward(&rewards, reduced_expedition) }
                    }
                }
                CurrentAction::Idle if played => return Outcome::Done,
                CurrentAction::Idle => match tavern.available_tasks() {
                    AvailableTasks::Expeditions(list) => {
                        for e in list.iter().filter(|e| missions::for_target(e.target).is_none()) {
                            report!("[tavern] Unmapped mission on offer with target {:?}", e.target);
                        }
                        let thirst = tavern.thirst_for_adventure_sec;
                        let wanted: Vec<Location> = session
                            .game_state()
                            .map(|gs| {
                                crate::tasks::open_tasks(gs)
                                    .filter_map(|t| match t.typ {
                                        TaskType::TravelTo(l) => Some(l),
                                        _ => None,
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let Some(pos) = choose_expedition(list, thirst, &wanted) else {
                            report!(
                                "[tavern] No expedition affordable with the Thirst for Adventure ({} min), done",
                                thirst / 60
                            );
                            return Outcome::Done;
                        };
                        let e = &list[pos];
                        report!(
                            "[tavern] Starting expedition: {}, {} min Thirst for Adventure, special {:?}, {:?} → {:?}",
                            mission_name(e.target),
                            e.thirst_for_adventure_sec / 60,
                            e.special,
                            e.location_1,
                            e.location_2
                        );
                        if !wanted.is_empty() {
                            report!("[tavern] Task wants locations {wanted:?}");
                        }
                        unknown_in_row = 0;
                        played = true;
                        reduced_expedition = e.thirst_for_adventure_sec <= REDUCED_EXPEDITION_SEC;
                        if reduced_expedition {
                            report!("[tavern] Leftover expedition: preferring hourglasses over gold for rewards");
                        }
                        Command::ExpeditionStart { pos }
                    }
                    AvailableTasks::Quests(_) => {
                        report!("[tavern] Expeditions not available (only classic quests), not supported yet");
                        return Outcome::Done;
                    }
                },
                other => {
                    report!("[tavern] The character is busy ({other:?}), skipping the Tavern");
                    return Outcome::Done;
                }
            }
        };

        refresh_pending = !matches!(cmd, Command::Update);
        if let Err(e) = safe::send(session, cmd).await {
            report!("[tavern] Error: {e}");
            return if is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
        }
    }
    report!("[tavern] Step limit reached, stopping");
    Outcome::Done
}

#[cfg(test)]
mod tests {
    use super::*;
    use ExpeditionThing::*;

    fn exp(target: ExpeditionThing, cur: u8, amount: u8, heroism: i32, floor: u8) -> Expedition {
        let mut e = Expedition::default();
        e.target_thing = target;
        e.target_current = cur;
        e.target_amount = amount;
        e.heroism = heroism;
        e.current_floor = floor;
        e
    }

    fn with(mut e: Expedition, items: &[ExpeditionThing]) -> Expedition {
        for (slot, t) in e.items.iter_mut().zip(items) {
            *slot = Some(*t);
        }
        e
    }

    fn enc(typ: ExpeditionThing, heroism: i32) -> ExpeditionEncounter {
        ExpeditionEncounter { typ, heroism }
    }

    #[test]
    fn target_item_beats_small_points() {
        // Dragon: +10 and +5 at the end
        let e = with(exp(Dragon, 0, 2, 0, 3), &[Bait]);
        assert_eq!(choose_encounter(&e, &[enc(Cake, 5), enc(Dragon, 10)]), 1);
    }

    #[test]
    fn poster_worth_ten_early() {
        let e = exp(Cake, 1, 1, 5, 2);
        assert_eq!(choose_encounter(&e, &[enc(Dummy1, 2), enc(UnicornBounty, 0)]), 1);
        assert_eq!(choose_encounter(&e, &[enc(Phoenix, 12), enc(UnicornBounty, 0)]), 0);
    }

    #[test]
    fn bounty_bonus_applies() {
        let e = with(exp(Cake, 1, 1, 5, 4), &[DummyBounty]);
        assert_eq!(choose_encounter(&e, &[enc(Cake, 5), enc(Dummy1, 2)]), 1);
    }

    /// Example 1: at 40 we do not take a hurdle that would drop us below 40.
    #[test]
    fn secured_never_drops_below_40() {
        let e = exp(Cake, 1, 1, 40, 5);
        assert_eq!(choose_encounter(&e, &[enc(SmallHurdle, -1), enc(Dummy1, 2)]), 1);
    }

    /// Example 2: in the last round we do not start a chain that cannot be completed.
    #[test]
    fn no_unfinishable_chain_on_last_floor() {
        let e = exp(WinnersPodium, 0, 1, 38, 10);
        assert_eq!(choose_encounter(&e, &[enc(SmallHurdle, -1), enc(Key, 0)]), 1);
        assert_eq!(choose_encounter(&e, &[enc(UnicornBounty, 0), enc(CampFire, 3)]), 1);
    }

    /// Example 3: round 9, wanted dummy (+2 +10) vs. the target burnt-out campfire (0 +4).
    #[test]
    fn wanted_beats_weak_target() {
        let e = with(exp(BurntCampfire, 0, 1, 20, 9), &[DummyBounty, Phoenix]);
        assert_eq!(choose_encounter(&e, &[enc(BurntCampfire, 0), enc(Dummy1, 2)]), 1);
    }

    #[test]
    fn princess_early_not_late() {
        let early = exp(Dragon, 0, 1, 0, 2);
        assert_eq!(choose_encounter(&early, &[enc(Dummy1, 2), enc(Bait, -2)]), 1);
        let late = exp(Dragon, 0, 1, 0, 10);
        assert_eq!(choose_encounter(&late, &[enc(Dummy1, 2), enc(Bait, -2)]), 0);
    }

    #[test]
    fn keys_and_chests_after_secured() {
        let e = exp(Cake, 1, 1, 42, 5);
        assert_eq!(choose_encounter(&e, &[enc(Dragon, 10), enc(Key, 0)]), 1);
        let e = with(exp(Cake, 1, 1, 42, 6), &[Key]);
        assert_eq!(choose_encounter(&e, &[enc(Key, 0), enc(Suitcase, 0)]), 1);
        // last round: a key makes no sense any more
        let e = exp(Cake, 1, 1, 42, 10);
        assert_eq!(choose_encounter(&e, &[enc(Key, 0), enc(Dummy1, 2)]), 1);
    }

    #[test]
    fn per_item_bonus_counts_to_40() {
        // Podium: 25 + 2 × 10 at the end = 45 → already secured, take the key
        let e = exp(WinnersPodium, 2, 2, 25, 6);
        assert_eq!(projected_heroism(&e), 45);
        assert_eq!(choose_encounter(&e, &[enc(Dummy1, 2), enc(Key, 0)]), 1);
    }

    #[test]
    fn sanitary_fail_penalty() {
        // unfinished paper: projection 42 - 5 = 37 → not secured yet
        let e = exp(ToiletPaper, 2, 3, 42, 7);
        assert_eq!(projected_heroism(&e), 37);
        // the third paper completes the task: 0 + 20 + 5
        assert_eq!(choose_encounter(&e, &[enc(Key, 0), enc(ToiletPaper, 0)]), 1);
    }

    fn avail(target: ExpeditionThing, min: u32) -> AvailableExpedition {
        AvailableExpedition {
            target,
            thirst_for_adventure_sec: min * 60,
            location_1: Default::default(),
            location_2: Default::default(),
            special: None,
        }
    }

    #[test]
    fn expedition_shortest_then_easiest() {
        // shorter wins even over easier
        assert_eq!(choose_expedition(&[avail(Cake, 20), avail(BurntCampfire, 15)], 6000, &[]), Some(1));
        // same length: suckling pig (8/round) is easier than the fire (4/round)
        assert_eq!(choose_expedition(&[avail(BurntCampfire, 20), avail(Cake, 20)], 6000, &[]), Some(1));
        // same length: an unmapped mission wins
        assert_eq!(choose_expedition(&[avail(Cake, 20), avail(Merman, 20)], 6000, &[]), Some(1));
        // same length: a mission with an unverified bonus also wins
        assert_eq!(choose_expedition(&[avail(Cake, 20), avail(Klaus, 20)], 6000, &[]), Some(1));
        // not enough Thirst for Adventure for the longer one
        assert_eq!(choose_expedition(&[avail(Cake, 20)], 600, &[]), None);
    }

    #[test]
    fn task_location_beats_shorter() {
        let mut far = avail(Cake, 20);
        far.location_2 = Location::SkullIsland;
        let list = [avail(Cake, 15), far];
        assert_eq!(choose_expedition(&list, 6000, &[Location::SkullIsland]), Some(1));
        assert_eq!(choose_expedition(&list, 6000, &[]), Some(0));
        // the wanted one must still be affordable
        assert_eq!(choose_expedition(&list, 900, &[Location::SkullIsland]), Some(0));
    }

    #[test]
    fn unknown_mission_chain_progresses() {
        let e = exp(Barkeeper, 0, 1, 0, 3);
        assert!(future_value(&e, &enc(Mugs, 0)) > 0.0);
        assert_eq!(choose_encounter(&e, &[enc(Dummy2, 2), enc(Mugs, 0)]), 1);
        // another group gets nothing
        assert_eq!(future_value(&e, &enc(FishingRod, 0)), 0.0);
    }

    /// Numbers verified in the 2026-10-07 run.
    #[test]
    fn expectations_match_observed_runs() {
        // unicorn: 8 → 25 (7 + bonus 10 right on completion)
        let e = with(exp(Unicorn, 0, 1, 8, 6), &[UnicornHorn, Donkey, Rainbow]);
        assert_eq!(e.heroism + expected_now(&e, &enc(Unicorn, 7)), 25);
        // dummy with a poster: 25 → 37
        let e = with(exp(Unicorn, 1, 1, 25, 7), &[DummyBounty]);
        assert_eq!(e.heroism + expected_now(&e, &enc(Dummy2, 2)), 37);
        // dragons: +5 × 2 at the end
        assert_eq!(expected_end_change(&exp(Dragon, 2, 2, 36, 10)), 10);
        // unfinished paper: −5 at the end, completion +20 right away
        assert_eq!(expected_end_change(&exp(ToiletPaper, 2, 3, 30, 10)), -5);
        assert_eq!(expected_now(&exp(ToiletPaper, 2, 3, 30, 8), &enc(ToiletPaper, 0)), 20);
    }

    /// Barkeeper 2026-10-07, round 9: heroism 13, we hold a key, offer chest / dummy +3.
    #[test]
    fn chest_when_40_unreachable() {
        let e = with(exp(Barkeeper, 0, 1, 13, 9), &[Key]);
        assert_eq!(choose_encounter(&e, &[enc(Dummy3, 3), enc(Suitcase, 0), enc(Dummy1, 1)]), 1);
        // at the start of the expedition we still chase points
        let e = with(exp(Barkeeper, 0, 1, 13, 4), &[Key]);
        assert_eq!(choose_encounter(&e, &[enc(Dummy3, 3), enc(Suitcase, 0)]), 0);
    }

    /// Items of foreign cycles only by immediate gain (user rule 2026-10-07).
    #[test]
    fn foreign_quest_items_only_for_instant_gain() {
        // dragon mission: campfire (+3, foreign cycle) beats a weak dummy (+1) → take it
        let e = exp(Dragon, 0, 1, 0, 3);
        assert_eq!(choose_encounter(&e, &[enc(Dummy1, 1), enc(CampFire, 3)]), 1);
        // no small hurdle (−1) for a foreign winner's podium, rather the key (0)
        let e = exp(Unicorn, 0, 1, 1, 4);
        assert_eq!(choose_encounter(&e, &[enc(SmallHurdle, -1), enc(Key, 0)]), 1);
        // no witch (−5) in a foreign mission, the dummy (+2)
        let e = exp(BrokenSword, 2, 97, 3, 6);
        assert_eq!(choose_encounter(&e, &[enc(Dummy2, 2), enc(Girl, -5)]), 0);
    }

    #[test]
    fn skips_wait_with_glass_only_when_midnight_is_close() {
        use chrono::TimeZone;
        let at = |h: u32, m: u32| Local.with_ymd_and_hms(2026, 10, 7, h, m, 0).unwrap();
        // Plenty of time: 1h wait at 20:00, 4h to midnight – no need for an hourglass.
        assert!(!should_skip_wait_with_glass(at(21, 0), at(20, 0)));
        // 50 min wait starting at 23:20: ends 00:10, already past midnight – skip.
        assert!(should_skip_wait_with_glass(at(0, 10) + chrono::Duration::days(1), at(23, 20)));
        // 10 min wait starting at 23, 50 min to midnight: plenty of margin even after waiting.
        assert!(!should_skip_wait_with_glass(at(23, 10), at(23, 0)));
        // 10 min wait starting at 23:50: ends 00:00, 0 min margin – skip.
        assert!(should_skip_wait_with_glass(at(0, 0) + chrono::Duration::days(1), at(23, 50)));
    }

    #[test]
    fn detects_after_wait() {
        let e = Expedition::default();
        assert!(!is_after_wait(&e));
        let mut v = serde_json::to_value(&e).unwrap();
        v["floor_stage"] = serde_json::json!(4);
        let e: Expedition = serde_json::from_value(v).unwrap();
        assert!(is_after_wait(&e));
    }

    #[test]
    fn reward_order() {
        let r = |typ| Reward { typ, amount: 1 };
        assert_eq!(choose_reward(&[r(RewardType::QuicksandGlass), r(RewardType::Silver), r(RewardType::Mushrooms)], false), 2);
        assert_eq!(choose_reward(&[r(RewardType::QuicksandGlass), r(RewardType::Silver)], false), 1);
        assert_eq!(choose_reward(&[r(RewardType::XP), r(RewardType::QuicksandGlass)], false), 1);
        // leftover expedition: hourglasses before gold, mushrooms still first
        assert_eq!(choose_reward(&[r(RewardType::Silver), r(RewardType::QuicksandGlass)], true), 1);
        assert_eq!(choose_reward(&[r(RewardType::QuicksandGlass), r(RewardType::Mushrooms)], true), 1);
    }
}
