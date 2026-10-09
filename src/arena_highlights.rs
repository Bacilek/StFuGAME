//! Arena "highlight" detection: after a win, look for a new **personal best** (per character, per category –
//! never compared across characters) in the raw per-round combat log (`fight.r`), which sf-api does not parse
//! for `fightversion > 1` (the server's current format – see `docs/arena-highlights.md` for the reverse-
//! engineered field layout). Only a fight that beats this character's own previous record in at least one
//! category gets marked via `PlayerCombatLogMark` (sf-api does not know this command either, same
//! `Command::Custom` + base64-params mechanism as guild/daily, see `src/guild.rs`) so it shows up in Quarter →
//! Mail, like manually clicking "save" on a fight in-game.
//! User's rule (2026-10-09, docs/precedents.md): thresholds high on purpose (not "a decent fight", a genuinely
//! rare one), and at most one saved fight per character per category – a paladin can have one saved for its best
//! block streak AND one for its best heal count, a plague doctor one for evades and one for poison damage, etc.,
//! but a later, better fight of the *same* category should replace needing to keep the old one around (we can't
//! un-mark the old one yet, see docs/arena-highlights.md's open question) rather than accumulate dozens of
//! "pretty good" fights per character over the whole challenge.
//! Format and category thresholds reverse-engineered/tuned from the user's own saved fights (2026-10-09). Not yet
//! verified live – see docs/arena-highlights.md.

use std::collections::HashMap;

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

/// One measured category from a fight. `lower_is_better` (only `low_hp`) means a *smaller* value is the more
/// impressive one (closer to death); every other category is "bigger is better".
struct Metric {
    category: &'static str,
    value: f64,
    threshold: f64,
    lower_is_better: bool,
    describe: fn(f64) -> String,
}

fn beats_threshold(m: &Metric) -> bool {
    if m.lower_is_better { m.value <= m.threshold } else { m.value >= m.threshold }
}

fn beats_record(m: &Metric, prev: f64) -> bool {
    if m.lower_is_better { m.value < prev } else { m.value > prev }
}

/// Measures every category for a just-won Arena fight. Empty if there is nothing to measure: no fight, no raw
/// rounds we can parse, or the fight was lost (the user's examples were all about winning, see
/// docs/precedents.md – losses are never saved regardless of how impressive).
fn measure(gs: &GameState, raw: &str) -> Vec<Metric> {
    let Some(fight) = gs.last_fight.as_ref() else { return Vec::new() };
    if !fight.has_player_won {
        return Vec::new();
    }
    let Some(single) = fight.fights.first() else { return Vec::new() };
    let (Some(a), Some(b)) = (single.fighter_a.as_ref(), single.fighter_b.as_ref()) else { return Vec::new() };
    let own_id = i64::from(gs.character.player_id);
    let (own, opp) = if a.id == own_id { (a, b) } else { (b, a) };

    let rounds = parse_rounds(raw, a.id, b.id);
    if rounds.is_empty() {
        return Vec::new();
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

    // Walk the fight tracking both fighters' life to get: how many times we revived (life <= 0, then positive
    // again), how low our life got (excluding the very last round, which for a win is just the kill shot), and
    // the biggest single hit we landed (damage as a fraction of the opponent's max life).
    let own_max_life = i64::from(own.life).max(1);
    let opp_max_life = i64::from(opp.life).max(1);
    let mut life = [own_max_life, opp_max_life]; // index 0 = us, 1 = opponent
    let idx = |id: i64| usize::from(id != own_id);
    let mut revive_count = 0u32;
    let mut seen_down = false;
    let mut low_point = own_max_life;
    let mut best_hit_ratio = 0.0f64;
    for (i, r) in rounds.iter().enumerate() {
        let defender = 1 - idx(r.actor);
        let dmg = life[defender] - r.target_life;
        if r.actor == own_id && dmg > 0 {
            best_hit_ratio = best_hit_ratio.max(dmg as f64 / opp_max_life as f64);
        }
        life[defender] = r.target_life;
        life[idx(r.actor)] = r.own_life;
        if i + 1 < rounds.len() {
            // not the final, fatal round
            if life[0] > 0 {
                low_point = low_point.min(life[0]);
            }
        }
        if life[0] <= 0 {
            seen_down = true;
        } else if seen_down {
            revive_count += 1;
            seen_down = false;
        }
    }

    let level_gap = f64::from(opp.level).max(0.0) - f64::from(own.level);
    let strength_ratio = {
        let own_s = crate::arena::strength(own.class, |a| f64::from(own.attributes[a]));
        let opp_s = crate::arena::strength(opp.class, |a| f64::from(opp.attributes[a]));
        if own_s > 0.0 { opp_s / own_s } else { 1.0 }
    };
    let low_hp_ratio = low_point as f64 / own_max_life as f64;

    vec![
        Metric {
            category: "crit_streak",
            value: f64::from(crit_streak),
            threshold: 4.0,
            lower_is_better: false,
            describe: |v| format!("{v:.0}x crit v řadě"),
        },
        Metric {
            category: "block_streak",
            value: f64::from(block_streak),
            threshold: 5.0,
            lower_is_better: false,
            describe: |v| format!("{v:.0}x blok v řadě"),
        },
        Metric {
            category: "evade_streak",
            value: f64::from(evade_streak),
            threshold: 5.0,
            lower_is_better: false,
            describe: |v| format!("{v:.0}x výhyb v řadě"),
        },
        Metric {
            category: "heal_blocks",
            value: heal_blocks as f64,
            threshold: 4.0,
            lower_is_better: false,
            describe: |v| format!("{v:.0}x blok+heal"),
        },
        Metric {
            category: "summons",
            value: summons as f64,
            threshold: 3.0,
            lower_is_better: false,
            describe: |v| format!("{v:.0}x vyvolání společníka"),
        },
        Metric {
            category: "companion_big_hits",
            value: companion_big_hits as f64,
            threshold: 2.0,
            lower_is_better: false,
            describe: |v| format!("{v:.0}x silný zásah společníka"),
        },
        Metric {
            category: "combo_run",
            value: f64::from(combo_run),
            threshold: 4.0,
            lower_is_better: false,
            describe: |v| format!("{v:.0}x akce v řadě (combo)"),
        },
        Metric {
            category: "revives",
            value: f64::from(revive_count),
            threshold: 1.0,
            lower_is_better: false,
            describe: |v| format!("{v:.0}x oživení"),
        },
        Metric {
            category: "low_hp",
            value: low_hp_ratio,
            threshold: 0.05,
            lower_is_better: true,
            describe: |v| format!("přežití na {:.1} % života", v * 100.0),
        },
        Metric {
            category: "level_gap",
            value: level_gap,
            threshold: 6.0,
            lower_is_better: false,
            describe: |v| format!("soupeř o {v:.0} levelů výš"),
        },
        Metric {
            category: "strength_ratio",
            value: strength_ratio,
            threshold: 1.6,
            lower_is_better: false,
            describe: |v| format!("soupeř ~{:.0}% silnější", (v - 1.0) * 100.0),
        },
        Metric {
            category: "big_hit",
            value: best_hit_ratio,
            threshold: 0.35,
            lower_is_better: false,
            describe: |v| format!("jedna rána za {:.0} % soupeřova života", v * 100.0),
        },
    ]
}

/// `roster/<character>/arena_highlights.json`: this character's best value ever seen per category (never
/// compared across characters – each has their own file and their own records).
fn records_path() -> std::path::PathBuf {
    std::path::Path::new("roster").join(crate::ctx::name()).join("arena_highlights.json")
}

fn load_records() -> HashMap<String, f64> {
    std::fs::read_to_string(records_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_records(records: &HashMap<String, f64>) {
    let path = records_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(records) {
        let _ = std::fs::write(path, s);
    }
}

/// Call after a won Arena fight (`raw` = the raw response of the `Fight` command). Marks it via
/// `PlayerCombatLogMark` (so it shows up in Quarter → Mail) only if it beats this character's own previous best
/// in at least one category – best-effort on the marking itself: if the fight cannot be found in
/// `gs.mail.combat_log` yet (not verified whether the server includes it right away, see
/// docs/arena-highlights.md), it just logs the miss instead of marking anything, but the new record is still
/// saved either way (no point re-detecting the same record on a later fight that doesn't beat it).
pub async fn maybe_mark(session: &mut SimpleSession, raw: &str, opponent: &str) {
    let Some((new_records, msg_id)) = session.game_state().map(|gs| {
        let records = load_records();
        let new_records: Vec<(String, f64, String)> = measure(gs, raw)
            .into_iter()
            .filter(beats_threshold)
            .filter(|m| records.get(m.category).is_none_or(|&prev| beats_record(m, prev)))
            .map(|m| (m.category.to_string(), m.value, (m.describe)(m.value)))
            .collect();
        let msg_id = gs
            .mail
            .combat_log
            .iter()
            .filter(|e| e.player_name == opponent)
            .max_by_key(|e| e.time)
            .filter(|e| (Local::now() - e.time).num_minutes() < 5)
            .map(|e| e.msg_id);
        (new_records, msg_id)
    }) else {
        return;
    };
    if new_records.is_empty() {
        return;
    }
    let descriptions: Vec<&str> = new_records.iter().map(|(_, _, d)| d.as_str()).collect();
    report!("[arena] New personal best: {}", descriptions.join(", "));

    let mut records = load_records();
    for (category, value, _) in &new_records {
        records.insert(category.clone(), *value);
    }
    save_records(&records);

    let Some(msg_id) = msg_id else {
        report!("[arena] New record but no matching combat log entry to mark yet, skipping the save");
        return;
    };
    report!("[arena] Marking fight {msg_id} as saved");
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

    #[test]
    fn record_keeping_is_strictly_better_only() {
        // bigger-is-better category: equal or smaller does not count as a new record
        let m = Metric { category: "x", value: 5.0, threshold: 3.0, lower_is_better: false, describe: |v| v.to_string() };
        assert!(beats_record(&m, 4.0));
        assert!(!beats_record(&m, 5.0));
        assert!(!beats_record(&m, 6.0));
        // lower-is-better category (low_hp): smaller beats a bigger previous record
        let m = Metric { category: "low_hp", value: 0.02, threshold: 0.05, lower_is_better: true, describe: |v| v.to_string() };
        assert!(beats_record(&m, 0.04));
        assert!(!beats_record(&m, 0.01));
    }
}
