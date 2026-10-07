//! Podzemí: kdykoli je mimo cooldown (i během expedice), jeden boj.
//! Z odemčených podzemí vybere to s protivníkem nejnižšího levelu, při podobném levelu toho slabšího.
//! Nikdy za houby, nikdy s plným inventářem.

use sf_api::{
    SimpleSession,
    command::{AttributeType, Command},
    gamestate::{
        GameState,
        dungeons::{Dungeon, DungeonProgress, LightDungeon},
    },
};

use crate::{arena::strength, safe, tavern::Outcome};

/// Jak často nejvýš zkoušet podzemí, když boj neproběhl (plný inventář apod.).
pub const RETRY_SEC: u64 = 5 * 60;

/// Levely, které bereme jako „podobné“ (pak rozhoduje síla podle statů).
const SIMILAR_LEVELS: u16 = 2;

/// Protivník v podzemí: level a síla (stejný vzorec jako v aréně).
#[derive(Debug, Clone)]
pub struct Candidate {
    pub dungeon: Dungeon,
    pub name: String,
    pub level: u16,
    pub strength: f64,
}

/// Všechna otevřená podzemí, kde se dá bojovat příkazem FightDungeon (věž má vlastní příkaz).
fn open_dungeons(gs: &GameState) -> Vec<Dungeon> {
    let is_open = |p: &DungeonProgress| matches!(p, DungeonProgress::Open { .. });
    let light = gs.dungeons.light.iter().filter(|(d, p)| *d != LightDungeon::Tower && is_open(p)).map(|(d, _)| Dungeon::Light(d));
    let shadow = gs.dungeons.shadow.iter().filter(|(_, p)| is_open(p)).map(|(d, _)| Dungeon::Shadow(d));
    light.chain(shadow).collect()
}

/// Protivníci ve všech otevřených podzemích.
pub fn candidates(gs: &GameState) -> Vec<Candidate> {
    let ch = &gs.character;
    open_dungeons(gs)
        .into_iter()
        .filter_map(|d| {
            let m = gs.dungeons.current_enemy(d)?;
            // „Zrcadlový obraz“ je v sf-api válečník s levelem 0: je to kopie naší postavy
            let c = if m.level == 0 {
                let stat = |a: AttributeType| f64::from(ch.attribute_basis[a] + ch.attribute_additions[a]);
                Candidate { dungeon: d, name: "zrcadlový obraz".into(), level: ch.level, strength: strength(ch.class, stat) }
            } else {
                let stat = |a: AttributeType| f64::from(m.attributes[a]);
                Candidate { dungeon: d, name: m.name.into(), level: m.level, strength: strength(m.class, stat) }
            };
            Some(c)
        })
        .collect()
}

/// Nejnižší level; mezi protivníky s podobným levelem (do +SIMILAR_LEVELS) ten s nejnižší silou.
pub fn choose(cands: &[Candidate]) -> Option<&Candidate> {
    let min_level = cands.iter().map(|c| c.level).min()?;
    cands
        .iter()
        .filter(|c| c.level <= min_level + SIMILAR_LEVELS)
        .min_by(|a, b| a.strength.total_cmp(&b.strength))
}

fn chyba(e: &sf_api::error::SFError) -> Outcome {
    report!("[podzemí] Chyba: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Jeden boj v podzemí, pokud je to teď možné. Jinak nic nedělá.
pub async fn run(session: &mut SimpleSession) -> Outcome {
    // Čas podzemí se obnoví jen přes UpdateDungeons (Update ani boj ho neobnoví)
    let gs = match safe::send(session, Command::UpdateDungeons).await {
        Ok(gs) => gs,
        Err(e) => return chyba(&e),
    };
    if !safe::dungeon_is_free(gs) {
        return Outcome::Done;
    }
    if gs.character.inventory.count_free_slots() == 0 {
        report!("[podzemí] Plný inventář, boj vynechávám");
        return Outcome::Done;
    }
    let cands = candidates(gs);
    let Some(pick) = choose(&cands).cloned() else {
        report!("[podzemí] Žádné otevřené podzemí");
        return Outcome::Done;
    };
    for c in &cands {
        report!("[podzemí] {:?}: {} (lvl {}, síla {:.0})", c.dungeon, c.name, c.level, c.strength);
    }
    report!("[podzemí] Bojuji: {:?} – {} (lvl {})", pick.dungeon, pick.name, pick.level);

    let gs = match safe::send(session, Command::FightDungeon { dungeon: pick.dungeon, use_mushroom: false }).await {
        Ok(gs) => gs,
        Err(e) => return chyba(&e),
    };
    match &gs.last_fight {
        Some(f) => report!(
            "[podzemí] {}: xp +{}, stříbro {:+}, předmět {}",
            if f.has_player_won { "Výhra" } else { "Prohra" },
            f.xp_change,
            f.silver_change,
            if f.item_won.is_some() { "ano" } else { "ne" }
        ),
        None => report!("[podzemí] Boj proběhl, výsledek server neposlal"),
    }
    // Nový čas cooldownu (pojistka v safe.rs bez něj další boj nepustí)
    match safe::send(session, Command::UpdateDungeons).await {
        Ok(gs) => {
            if let Some(t) = gs.dungeons.next_free_fight {
                report!("[podzemí] Další volný boj v {}", t.format("%H:%M:%S"));
            }
            Outcome::Done
        }
        Err(e) => chyba(&e),
    }
}

/// Za kolik sekund bude podzemí volné (s rezervou). Bez čerstvého stavu odhad.
pub fn secs_until_ready(gs: &GameState) -> u64 {
    let free_at = gs.dungeons.next_free_fight.map_or_else(chrono::Local::now, |t| {
        t + chrono::Duration::seconds(safe::ARENA_SAFETY_SEC)
    });
    u64::try_from((free_at - chrono::Local::now()).num_seconds()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(level: u16, strength: f64) -> Candidate {
        Candidate { dungeon: Dungeon::Light(LightDungeon::DesecratedCatacombs), name: String::new(), level, strength }
    }

    #[test]
    fn lowest_level_wins() {
        let cands = [c(20, 100.0), c(10, 900.0), c(30, 50.0)];
        assert_eq!(choose(&cands).unwrap().level, 10);
    }

    #[test]
    fn similar_level_weaker_stats() {
        let cands = [c(10, 900.0), c(11, 500.0), c(15, 100.0)];
        assert_eq!(choose(&cands).unwrap().level, 11);
    }
}
