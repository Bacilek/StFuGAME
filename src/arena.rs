//! Aréna: když postava nic nedělá a aréna je mimo cooldown, vyzve nejslabšího ze 3 soupeřů.
//! Síla = 100 % hlavní atribut + 80 % odolnost + 40 % štěstí + 10 % každý vedlejší atribut.

use sf_api::{
    SimpleSession,
    command::{AttributeType, Command},
    gamestate::{character::Class, social::OtherPlayer, tavern::CurrentAction},
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
    if gs.tavern.current_action != CurrentAction::Idle {
        return Outcome::Done;
    }
    if !safe::arena_is_free(gs) {
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
    let gs = match safe::send(session, Command::Fight { name, use_mushroom: false }).await {
        Ok(gs) => gs,
        Err(e) => return chyba(&e),
    };
    match &gs.last_fight {
        Some(f) => report!(
            "[aréna] {}: čest {:+}, stříbro {:+}, xp +{}",
            if f.has_player_won { "Výhra" } else { "Prohra" },
            f.honor_change,
            f.silver_change,
            f.xp_change
        ),
        None => report!("[aréna] Boj proběhl, výsledek server neposlal"),
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
