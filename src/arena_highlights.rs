//! Arena "highlight" detection: after a win, score how impressive the fight was from the raw per-round combat
//! log (`fight.r`), which sf-api does not parse for `fightversion > 1` (the server's current format – see
//! `docs/arena-highlights.md` for the reverse-engineered field layout and scoring). If the score crosses a
//! threshold, mark the fight with `PlayerCombatLogMark` (sf-api does not know this command either, same
//! `Command::Custom` + base64-params mechanism as guild/daily, see `src/guild.rs`) so it shows up in Quarter →
//! Mail, like manually clicking "save" on a fight in-game.
//! Format and scoring reverse-engineered from the user's own saved fights (2026-10-09). Not yet verified live:
//! does marking actually work, and are the point thresholds reasonable? Tune `MARK_THRESHOLD` and the per-signal
//! points after seeing real scores in `logs/progress.log`.

use chrono::Local;
use sf_api::{command::Command, gamestate::GameState};

use crate::{safe, session::SimpleSession};

fn custom(cmd_name: &str, arguments: &[&str]) -> Command {
    Command::Custom { cmd_name: cmd_name.to_string(), arguments: arguments.iter().map(|a| (*a).to_string()).collect() }
}

/// One parsed round from `fight.r`. Field meanings: see `docs/arena-highlights.md`.
#[derive(Debug, Clone, Copy)]
struct Round {
    actor: i64,
    typ: i64,
    result: i64,
    own_life: i64,
    target_life: i64,
}

/// `type` field values we understand (docs/arena-highlights.md has the full table and examples).
const TYPE_CRIT: i64 = 1;
const TYPE_SUMMON: i64 = 11;
const TYPE_COMPANION_BIG_HIT: i64 = 15;
/// `result` field values (always refers to the outcome on the *defending* side, whichever row it is attached to).
const RESULT_BLOCKED: i64 = 3;
const RESULT_EVADED: i64 = 4;
const RESULT_BLOCKED_HEALED: i64 = 6;

/// Parses the `fight.r` rounds out of the raw server response. Each round is 7 fixed fields
/// (`actor/stance/type/result/f4/own_life/target_life`) followed by a variable-length tail we don't need; the
/// next round is recognised by `actor` being one of the two fighter ids again.
fn parse_rounds(raw: &str, id_a: i64, id_b: i64) -> Vec<Round> {
    let Some(section) = raw.split("fight.r:").nth(1) else { return Vec::new() };
    let section = section.split('&').next().unwrap_or("");
    let toks: Vec<i64> = section.split('/').filter_map(|s| s.parse().ok()).collect();
    let mut rounds = Vec::new();
    let mut i = 0;
    while i + 7 <= toks.len() {
        let actor = toks[i];
        if actor != id_a && actor != id_b {
            break; // unexpected layout, stop rather than misparse the rest
        }
        rounds.push(Round { actor, typ: toks[i + 2], result: toks[i + 3], own_life: toks[i + 5], target_life: toks[i + 6] });
        let mut j = i + 7;
        while j < toks.len() && toks[j] != id_a && toks[j] != id_b {
            j += 1;
        }
        i = j;
    }
    rounds
}

/// Longest run of consecutive `true`s.
fn longest_run(items: &[bool]) -> u32 {
    let (mut best, mut cur) = (0u32, 0u32);
    for &b in items {
        cur = if b { cur + 1 } else { 0 };
        best = best.max(cur);
    }
    best
}

/// Longest run of consecutive rounds (in raw order) acted by `actor_id` – catches "several actions in one go"
/// (berserker rage, golem/wolf + owner, assassin's second weapon) regardless of class, see docs/arena-highlights.md.
fn longest_actor_run(rounds: &[Round], actor_id: i64) -> u32 {
    longest_run(&rounds.iter().map(|r| r.actor == actor_id).collect::<Vec<_>>())
}

pub struct Highlight {
    pub points: u32,
    pub reasons: Vec<String>,
}

/// Score to mark a fight. Unverified (docs/arena-highlights.md) – tune after seeing real scores logged.
const MARK_THRESHOLD: u32 = 5;

/// Scores how "cool" a just-won Arena fight was. `None` if there is nothing to score: no fight, no raw rounds we
/// can parse, or the fight was lost (the user's examples were all about winning, see docs/precedents.md).
fn evaluate(gs: &GameState, raw: &str) -> Option<Highlight> {
    let fight = gs.last_fight.as_ref()?;
    if !fight.has_player_won {
        return None;
    }
    let single = fight.fights.first()?;
    let (a, b) = (single.fighter_a.as_ref()?, single.fighter_b.as_ref()?);
    let own_id = i64::from(gs.character.player_id);
    let (own, opp) = if a.id == own_id { (a, b) } else { (b, a) };

    let rounds = parse_rounds(raw, a.id, b.id);
    if rounds.is_empty() {
        return None;
    }

    let own_rounds: Vec<&Round> = rounds.iter().filter(|r| r.actor == own_id).collect();
    let opp_rounds: Vec<&Round> = rounds.iter().filter(|r| r.actor == opp.id).collect();

    let crit_streak = longest_run(&own_rounds.iter().map(|r| r.typ == TYPE_CRIT).collect::<Vec<_>>());
    let block_streak =
        longest_run(&opp_rounds.iter().map(|r| r.result == RESULT_BLOCKED || r.result == RESULT_BLOCKED_HEALED).collect::<Vec<_>>());
    let evade_streak = longest_run(&opp_rounds.iter().map(|r| r.result == RESULT_EVADED).collect::<Vec<_>>());
    let heal_blocks = opp_rounds.iter().filter(|r| r.result == RESULT_BLOCKED_HEALED).count();
    let summons = own_rounds.iter().filter(|r| r.typ == TYPE_SUMMON).count();
    let companion_big_hits = own_rounds.iter().filter(|r| r.typ == TYPE_COMPANION_BIG_HIT).count();
    let combo_run = longest_actor_run(&rounds, own_id);

    // Revive: our life reads <= 0 at some point but we keep acting afterwards.
    let mut seen_down = false;
    let mut revived = false;
    for r in &rounds {
        let our_life_here =
            if r.actor == own_id { Some(r.own_life) } else if r.actor == opp.id { Some(r.target_life) } else { None };
        if let Some(life) = our_life_here {
            if life <= 0 {
                seen_down = true;
            } else if seen_down {
                revived = true;
            }
        }
    }

    // How low our life got relative to max, ignoring the very last round (just the kill shot).
    let own_max_life = i64::from(own.life).max(1);
    let low_point = rounds[..rounds.len().saturating_sub(1)]
        .iter()
        .filter_map(|r| if r.actor == own_id { Some(r.own_life) } else if r.actor == opp.id { Some(r.target_life) } else { None })
        .filter(|&l| l > 0)
        .min();

    let level_gap = i64::from(opp.level).saturating_sub(i64::from(own.level));
    let strength_ratio = {
        let own_s = crate::arena::strength(own.class, |a| f64::from(own.attributes[a]));
        let opp_s = crate::arena::strength(opp.class, |a| f64::from(opp.attributes[a]));
        if own_s > 0.0 { opp_s / own_s } else { 1.0 }
    };

    let mut points = 0u32;
    let mut reasons = Vec::new();
    if crit_streak >= 3 {
        points += 3;
        reasons.push(format!("{crit_streak}x crit v řadě"));
    }
    if block_streak >= 3 {
        points += 2;
        reasons.push(format!("{block_streak}x blok v řadě"));
    }
    if evade_streak >= 3 {
        points += 2;
        reasons.push(format!("{evade_streak}x výhyb v řadě"));
    }
    if heal_blocks >= 3 {
        points += 2;
        reasons.push(format!("{heal_blocks}x blok+heal"));
    }
    if summons >= 2 {
        points += 1;
        reasons.push(format!("{summons}x vyvolání společníka"));
    }
    if companion_big_hits >= 2 {
        points += 2;
        reasons.push(format!("{companion_big_hits}x silný zásah společníka"));
    }
    if combo_run >= 3 {
        points += 3;
        reasons.push(format!("{combo_run}x akce v řadě (combo)"));
    }
    if revived {
        points += 4;
        reasons.push("oživení uprostřed zápasu".to_string());
    }
    if let Some(low) = low_point {
        let ratio = low as f64 / own_max_life as f64;
        if ratio <= 0.1 {
            points += 3;
            reasons.push(format!("přežití na {:.0} % života", ratio * 100.0));
        }
    }
    if level_gap >= 5 {
        points += 2;
        reasons.push(format!("soupeř o {level_gap} levelů výš"));
    }
    if strength_ratio >= 1.5 {
        points += 3;
        reasons.push(format!("soupeř ~{:.0}% silnější", (strength_ratio - 1.0) * 100.0));
    }

    if reasons.is_empty() { None } else { Some(Highlight { points, reasons }) }
}

/// Call after a won Arena fight (`raw` = the raw response of the `Fight` command). Scores it and, past the
/// threshold, marks it via `PlayerCombatLogMark` so it shows up in Quarter → Mail – best-effort: if the fight
/// cannot be found in `gs.mail.combat_log` yet (not verified whether the server includes it right away, see
/// docs/arena-highlights.md), it just logs the miss instead of marking anything.
pub async fn maybe_mark(session: &mut SimpleSession, raw: &str, opponent: &str) {
    let Some((points, reasons, msg_id)) = session.game_state().and_then(|gs| {
        let highlight = evaluate(gs, raw)?;
        let msg_id = gs
            .mail
            .combat_log
            .iter()
            .filter(|e| e.player_name == opponent)
            .max_by_key(|e| e.time)
            .filter(|e| (Local::now() - e.time).num_minutes() < 5)
            .map(|e| e.msg_id);
        Some((highlight.points, highlight.reasons, msg_id))
    }) else {
        return;
    };
    report!("[arena] Highlight score {points}: {}", reasons.join(", "));
    if points < MARK_THRESHOLD {
        return;
    }
    let Some(msg_id) = msg_id else {
        report!("[arena] Cool fight (score {points}) but no matching combat log entry to mark yet, skipping");
        return;
    };
    report!("[arena] Marking fight {msg_id} as saved (score {points})");
    if let Err(e) = safe::send_raw_only(session, custom("PlayerCombatLogMark", &[&msg_id.to_string(), "1"])).await {
        report!("[arena] Could not mark fight {msg_id}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real response captured by the user (2026-10-09): a necromancer who summons a wolf 3 times (`type=11`
    /// at rounds 1, 12, 18) and lands 2 big companion hits (`type=15` at rounds 16, 22) – see
    /// docs/arena-highlights.md. Us = 23994, opponent = 21252 (winner).
    const WOLF_FIGHT: &str = "fight.r:21252/0/11/0/0/15180/41107/1/2/1/3/0/21252/0/12/0/0/15180/40263/1/2/1/2/0/23994/0/0/0/0/40263/14271/0/1/2/1/2/21252/0/0/3/0/14271/40263/1/2/1/2/0/21252/0/12/0/0/14271/38904/1/2/1/1/0/23994/0/0/0/0/38904/13157/0/1/2/1/1/21252/0/0/0/0/13157/37628/1/2/1/1/0/21252/0/12/0/0/13157/36354/1/2/1/0/0/23994/20/0/0/0/36354/11844/0/0/21252/0/0/6/20/11844/36835/0/0/23994/20/1/0/0/36835/9562/0/0/21252/0/11/0/20/9562/37481/1/2/2/2/0/21252/0/12/0/20/9562/30932/1/2/2/1/0/23994/20/0/0/0/30932/7897/0/1/2/2/1/21252/0/0/6/20/7897/31325/1/2/2/1/0/21252/0/15/0/20/7897/16084/1/2/2/0/0/23994/20/0/0/0/16084/6467/0/0/21252/0/11/0/20/6467/16084/1/2/2/2/0/21252/0/12/6/20/6467/18206/1/2/2/1/0/23994/20/0/0/0/18206/4630/0/1/2/2/1/21252/0/0/0/20/4630/15238/1/2/2/1/0/21252/0/15/0/20/4630/-6369/1/2/2/0/0/&winnerid:21252";

    #[test]
    fn parses_wolf_fight_rounds() {
        let rounds = parse_rounds(WOLF_FIGHT, 23994, 21252);
        assert_eq!(rounds.len(), 22);
        let summons = rounds.iter().filter(|r| r.actor == 21252 && r.typ == TYPE_SUMMON).count();
        let big_hits = rounds.iter().filter(|r| r.actor == 21252 && r.typ == TYPE_COMPANION_BIG_HIT).count();
        assert_eq!(summons, 3);
        assert_eq!(big_hits, 2);
        // Round 10 (index 9): opponent attacks, we block+heal (f3=6) – our life goes up, not down.
        let blocked_heal = &rounds[9];
        assert_eq!(blocked_heal.actor, 21252);
        assert_eq!(blocked_heal.result, RESULT_BLOCKED_HEALED);
        assert!(blocked_heal.target_life > rounds[8].own_life);
    }

    #[test]
    fn longest_run_counts_consecutive_true() {
        assert_eq!(longest_run(&[true, true, false, true, true, true, false]), 3);
        assert_eq!(longest_run(&[false, false]), 0);
        assert_eq!(longest_run(&[]), 0);
    }

    #[test]
    fn longest_actor_run_finds_multi_action_turn() {
        let rounds = parse_rounds(WOLF_FIGHT, 23994, 21252);
        // The opponent (summon + wolf attack, or wolf + own attack) goes multiple rounds in a row several times.
        assert!(longest_actor_run(&rounds, 21252) >= 2);
    }
}
