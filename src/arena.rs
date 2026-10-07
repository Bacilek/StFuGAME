//! Aréna: kdykoli je mimo cooldown (i během expedice), vyzve nejslabšího ze 3 soupeřů.
//! Nejvýš 10 výher denně, pak už nejsou odměny. Řídí se počítadlem serveru `fights_for_xp`
//! (počet dnešních výher za xp, 0–10), takže den resetuje server.
//! Síla = 100 % hlavní atribut + 80 % odolnost + 40 % štěstí + 10 % každý vedlejší atribut.

use std::{fs::OpenOptions, io::Write};

use chrono::Local;
use serde_json::json;
use sf_api::{
    SimpleSession,
    command::{AttributeType, Command},
    gamestate::{GameState, character::Class, social::OtherPlayer},
};

use crate::{safe, tavern::Outcome};

/// Celkový atribut soupeře (základ + vybavení a bonusy + mazlíčci).
fn total(p: &OtherPlayer, a: AttributeType) -> f64 {
    f64::from(p.attribute_basis[a] + p.attribute_additions[a] + p.attribute_pet_bonus[a])
}

/// Síla soupeře podle vzorce uživatele.
pub fn strength(class: Class, stat: impl Fn(AttributeType) -> f64) -> f64 {
    let main = class.main_attribute();
    let side: f64 = [AttributeType::Strength, AttributeType::Dexterity, AttributeType::Intelligence]
        .into_iter()
        .filter(|a| *a != main)
        .map(&stat)
        .sum();
    stat(main) + 0.8 * stat(AttributeType::Constitution) + 0.4 * stat(AttributeType::Luck) + 0.1 * side
}

/// Po tolika výhrách za den už aréna nedává odměny.
const MAX_WINS_PER_DAY: usize = 10;
const LOG: &str = "logs/arena.jsonl";

/// Počet dnešních výher podle logu arény (jen pro kontrolu proti `fights_for_xp`).
pub fn wins_today() -> usize {
    let today = Local::now().format("%Y-%m-%d").to_string();
    std::fs::read_to_string(LOG)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["time"].as_str().is_some_and(|t| t.starts_with(&today)) && v["won"] == true)
        .count()
}

/// Za kolik sekund bude aréna volná (s rezervou), pokud ještě má smysl (méně než 10 výher).
pub fn secs_until_ready(gs: &GameState) -> Option<u64> {
    if usize::from(gs.arena.fights_for_xp) >= MAX_WINS_PER_DAY {
        return None;
    }
    let free_at = gs.arena.next_free_fight.map_or_else(Local::now, |t| t + chrono::Duration::seconds(safe::ARENA_SAFETY_SEC));
    Some(u64::try_from((free_at - Local::now()).num_seconds()).unwrap_or(0))
}

/// Má smysl jít do arény? Mimo cooldown a ještě ne 10 výher dnes.
pub fn ready(gs: &GameState) -> bool {
    safe::arena_is_free(gs) && usize::from(gs.arena.fights_for_xp) < MAX_WINS_PER_DAY
}

fn log_fight(line: &serde_json::Value) {
    let res = std::fs::create_dir_all("logs").and_then(|()| {
        let mut f = OpenOptions::new().create(true).append(true).open(LOG)?;
        writeln!(f, "{line}")
    });
    if let Err(err) = res {
        report!("[aréna] Zápis do {LOG} selhal: {err}");
    }
}

fn chyba(e: &sf_api::error::SFError) -> Outcome {
    report!("[aréna] Chyba: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Jeden boj v aréně, pokud je to teď možné. Jinak nic nedělá.
pub async fn run(session: &mut SimpleSession) -> Outcome {
    // Rozhoduje se vždy podle čerstvého stavu (hlavně cooldown)
    if let Err(e) = safe::send(session, Command::Update).await {
        return chyba(&e);
    }
    let Some(gs) = session.game_state() else { return Outcome::Done };
    if !ready(gs) {
        return Outcome::Done;
    }

    let mut ids = gs.arena.enemy_ids;
    if ids.iter().all(|&id| id == 0) {
        match safe::send(session, Command::CheckArena).await {
            Ok(gs) => ids = gs.arena.enemy_ids,
            Err(e) => return chyba(&e),
        }
    }

    // Načíst staty soupeřů
    let mut best: Option<(f64, String)> = None;
    for id in ids.into_iter().filter(|&id| id != 0) {
        let gs = match safe::send(session, Command::ViewPlayer { ident: id.to_string() }).await {
            Ok(gs) => gs,
            Err(e) => return chyba(&e),
        };
        let Some(p) = gs.lookup.lookup_pid(id) else {
            report!("[aréna] Soupeře {id} se nepodařilo načíst");
            continue;
        };
        let s = strength(p.class, |a| total(p, a));
        report!("[aréna] Soupeř {} (lvl {}, {:?}): síla {s:.0}", p.name, p.level, p.class);
        if best.as_ref().is_none_or(|(b, _)| s < *b) {
            best = Some((s, p.name.clone()));
        }
    }
    let Some((s, name)) = best else {
        report!("[aréna] Žádný soupeř k dispozici");
        return Outcome::Done;
    };

    // Načítání soupeřů trvalo, cooldown ověřujeme znovu (safe::send ho hlídá taky)
    let still_free = session.game_state().is_some_and(safe::arena_is_free);
    if !still_free {
        report!("[aréna] Aréna mezitím není volná, boj ruším");
        return Outcome::Done;
    }
    report!("[aréna] Vyzývám nejslabšího: {name} (síla {s:.0})");
    let opponent = name.clone();
    let gs = match safe::send(session, Command::Fight { name, use_mushroom: false }).await {
        Ok(gs) => gs,
        Err(e) => return chyba(&e),
    };
    let fights_for_xp = gs.arena.fights_for_xp;
    match &gs.last_fight {
        Some(f) => {
            report!(
                "[aréna] {}: čest {:+}, stříbro {:+}, xp +{} (server fights_for_xp {fights_for_xp})",
                if f.has_player_won { "Výhra" } else { "Prohra" },
                f.honor_change,
                f.silver_change,
                f.xp_change
            );
            log_fight(&json!({
                "time": Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                "opponent": opponent,
                "strength": s.round(),
                "won": f.has_player_won,
                "honor": f.honor_change,
                "silver": f.silver_change,
                "xp": f.xp_change,
                "fights_for_xp": fights_for_xp,
            }));
        }
        None => report!("[aréna] Boj proběhl, výsledek server neposlal"),
    }
    let wins = usize::from(fights_for_xp);
    if wins >= MAX_WINS_PER_DAY {
        report!("[aréna] Dnes {wins} výher za xp, aréna do zítřka stojí");
    } else {
        report!("[aréna] Dnes výher za xp: {wins}/{MAX_WINS_PER_DAY} (podle mého logu {})", wins_today());
    }
    if let Some(next) = gs.arena.next_free_fight {
        report!("[aréna] Další volný boj v {}", next.format("%H:%M:%S"));
    }
    Outcome::Done
}

#[cfg(test)]
mod tests {
    use super::*;
    use AttributeType::*;

    fn stats(str: f64, dex: f64, int: f64, con: f64, lck: f64) -> impl Fn(AttributeType) -> f64 {
        move |a| match a {
            Strength => str,
            Dexterity => dex,
            Intelligence => int,
            Constitution => con,
            Luck => lck,
        }
    }

    #[test]
    fn scout_formula() {
        // lučištník: 100 % DEX + 80 % CON + 40 % LCK + 10 % STR + 10 % INT
        let s = strength(Class::Scout, stats(100.0, 1000.0, 200.0, 500.0, 300.0));
        assert!((s - (1000.0 + 400.0 + 120.0 + 10.0 + 20.0)).abs() < 1e-9);
    }

    #[test]
    fn warrior_and_mage_main_stat() {
        let w = strength(Class::Warrior, stats(1000.0, 0.0, 0.0, 0.0, 0.0));
        let m = strength(Class::Mage, stats(1000.0, 0.0, 0.0, 0.0, 0.0));
        assert!((w - 1000.0).abs() < 1e-9);
        assert!((m - 100.0).abs() < 1e-9);
    }
}
