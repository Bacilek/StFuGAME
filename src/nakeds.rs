//! Naked-player list for Underworld attacks (user 2026-11-10): ONE scanner character (`SCANNER`) looks through the Hall of Fame
//! and keeps, for every level that is a multiple of 5 (5, 10 … 200), the weakest player WITHOUT any equipment it has seen.
//! Result: `roster/nakeds.json` (state, read by the bot) + `roster/nakeds.md` (readable overview), both local and rewritten daily.
//!
//! Rules (user): the list is only completed, never improved on purpose. Each day the saved players are re-checked (still a
//! multiple of 5 in 5..=200 and still naked → kept, a different multiple of 5 → moved there if that is better); only levels
//! WITHOUT an entry are searched, and a weaker naked player replaces a stronger one only when it turns up by chance.
//!
//! Finding the level without reading the whole Hall of Fame: the ranking is by honor, which follows the level closely, so the
//! page where level L ends is found by bisection (`Hof::last_page_with`), and the pages already read are cached for the other
//! levels. The page found is remembered (`hints`) and the next day's search starts there (exponential search around the hint).
//! The weakest players of a level sit at the END of its rank range, so the search reads the pages from there upwards and views
//! the lowest-honor players first. Every request goes through `safe::send` (human pauses of 2.5-7 s).

use std::{
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use sf_api::{command::Command, gamestate::character::Class};

use crate::{arena, safe, session::SimpleSession, tavern::Outcome};

/// The character that scans (it only costs requests, the others never touch the Hall of Fame for this).
const SCANNER: &str = "TestChar1";
const STEP: u32 = 5;
const MAX_LEVEL: u32 = 200;
/// Players per Hall of Fame page.
const PAGE: usize = 51;
/// Pages read upwards from the end of a level's range, and candidates gathered from them.
const PAGES_PER_LEVEL: usize = 8;
const MAX_CANDIDATES: usize = 20;
/// Players viewed per level: without a naked one found yet / once one was found (looking for a weaker one among the lowest honors).
const INSPECT_NO_FIND: usize = 15;
const INSPECT_AFTER_FIND: usize = 6;
/// A level with nobody naked is searched again only after this many days.
const MISS_DAYS: i64 = 3;
/// One call works at most this long (the rest continues in the next main loop pass), so the character keeps playing.
const BUDGET: Duration = Duration::from_secs(12 * 60);
const JSON: &str = "roster/nakeds.json";
const MD: &str = "roster/nakeds.md";

#[derive(Serialize, Deserialize, Clone, Default)]
struct Entry {
    name: String,
    class: String,
    level: u32,
    power: f64,
    honor: u32,
    rank: u32,
    checked: String,
}

#[derive(Serialize, Deserialize, Default)]
struct Store {
    /// Day of the last scan pass; a new day starts with a re-check of the saved players.
    date: String,
    revalidated: bool,
    /// Levels already searched today
    searched: Vec<u32>,
    entries: BTreeMap<u32, Entry>,
    /// Level → day of the last fruitless search
    misses: BTreeMap<u32, String>,
    /// Level → Hall of Fame page where the level's range ended last time
    hints: BTreeMap<u32, usize>,
}

fn load() -> Store {
    std::fs::read_to_string(JSON).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save(store: &Store) {
    let _ = std::fs::create_dir_all("roster");
    if let Ok(s) = serde_json::to_string_pretty(store) {
        let _ = std::fs::write(JSON, s);
    }
    let mut md = format!("# Naked players (rewritten daily, last pass {})\n\nWeakest naked player seen per level (power = Arena formula).\n\n", store.date);
    md.push_str("| Level | Name | Class | Power | Honor | Rank | Checked |\n|---|---|---|---|---|---|---|\n");
    for level in (STEP..=MAX_LEVEL).step_by(STEP as usize) {
        match store.entries.get(&level) {
            Some(e) => md.push_str(&format!("| {level} | {} | {} | {:.0} | {} | {} | {} |\n", e.name, e.class, e.power, e.honor, e.rank, e.checked)),
            None => md.push_str(&format!("| {level} | - | | | | | {} |\n", store.misses.get(&level).map_or("not searched yet".to_string(), |d| format!("nobody found {d}")))),
        }
    }
    let _ = std::fs::write(MD, md);
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[nakeds] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// What a `ViewPlayer` tells us.
struct Seen {
    level: u32,
    honor: u32,
    rank: u32,
    class: Class,
    naked: bool,
    power: f64,
}

async fn view(session: &mut SimpleSession, name: &str) -> Result<Option<Seen>, Outcome> {
    let gs = safe::send(session, Command::ViewPlayer { ident: name.to_string() }).await.map_err(|e| fail(&e))?;
    let Some(p) = gs.lookup.lookup_name(name) else { return Ok(None) };
    Ok(Some(Seen {
        level: u32::from(p.level),
        honor: p.honor,
        rank: p.rank,
        class: p.class,
        naked: p.equipment.0.values().all(Option::is_none),
        power: arena::power(p.class, |a| arena::total(p, a)),
    }))
}

/// Puts a naked player on the list when his level is a multiple of 5 in range and the slot is empty or he is weaker than its holder.
fn offer(store: &mut Store, name: &str, s: &Seen, today: &str) -> bool {
    if !s.naked || s.level < STEP || s.level > MAX_LEVEL || s.level % STEP != 0 {
        return false;
    }
    let better = store.entries.get(&s.level).is_none_or(|e| e.name == name || s.power < e.power);
    if better {
        store.entries.insert(
            s.level,
            Entry { name: name.to_string(), class: format!("{:?}", s.class), level: s.level, power: s.power, honor: s.honor, rank: s.rank, checked: today.to_string() },
        );
    }
    better
}

#[derive(Clone)]
struct Row {
    name: String,
    level: u32,
    honor: u32,
}

/// Hall of Fame pages read in this pass.
struct Hof {
    cache: HashMap<usize, Vec<Row>>,
    total: Option<u32>,
}

impl Hof {
    async fn rows(&mut self, session: &mut SimpleSession, page: usize) -> Result<Vec<Row>, Outcome> {
        if let Some(r) = self.cache.get(&page) {
            return Ok(r.clone());
        }
        let gs = safe::send(session, Command::HallOfFamePage { page }).await.map_err(|e| fail(&e))?;
        self.total = Some(gs.hall_of_fames.players_total);
        let rows: Vec<Row> = gs.hall_of_fames.players.iter().map(|p| Row { name: p.name.clone(), level: p.level, honor: p.honor }).collect();
        self.cache.insert(page, rows.clone());
        Ok(rows)
    }

    async fn last_page(&mut self, session: &mut SimpleSession) -> Result<usize, Outcome> {
        if self.total.is_none() {
            self.rows(session, 0).await?;
        }
        Ok((self.total.unwrap_or(1).max(1) as usize - 1) / PAGE)
    }

    /// Does anybody on this page have at least this level? (Levels fall with the rank, so this is true for the first pages only.)
    async fn reaches(&mut self, session: &mut SimpleSession, page: usize, level: u32) -> Result<bool, Outcome> {
        Ok(self.rows(session, page).await?.iter().any(|r| r.level >= level))
    }

    /// The last page where somebody has at least `level` (`None` = nobody that high). Bisection, started around the
    /// remembered page when there is one.
    async fn last_page_with(&mut self, session: &mut SimpleSession, level: u32, hint: Option<usize>) -> Result<Option<usize>, Outcome> {
        let last = self.last_page(session).await?;
        let mut lo: Option<usize> = None; // known to reach the level
        let mut hi: Option<usize> = None; // known not to
        if let Some(h) = hint.map(|h| h.min(last)) {
            if self.reaches(session, h, level).await? {
                let mut cur = h;
                lo = Some(h);
                let mut step = 1;
                while cur + step <= last {
                    let n = cur + step;
                    if self.reaches(session, n, level).await? {
                        lo = Some(n);
                        cur = n;
                        step *= 2;
                    } else {
                        hi = Some(n);
                        break;
                    }
                }
            } else {
                let mut cur = h;
                hi = Some(h);
                let mut step = 1;
                loop {
                    if cur == 0 {
                        return Ok(None);
                    }
                    let n = cur.saturating_sub(step);
                    if self.reaches(session, n, level).await? {
                        lo = Some(n);
                        break;
                    }
                    hi = Some(n);
                    cur = n;
                    step *= 2;
                }
            }
        }
        let mut lo = match lo {
            Some(l) => l,
            None => {
                if !self.reaches(session, 0, level).await? {
                    return Ok(None);
                }
                0
            }
        };
        let mut hi = hi.unwrap_or(last + 1);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if self.reaches(session, mid, level).await? {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Ok(Some(lo))
    }
}

/// Searches one level: finds where its range ends, then views the lowest-honor players of that level.
async fn search(session: &mut SimpleSession, hof: &mut Hof, store: &mut Store, level: u32, today: &str) -> Result<(), Outcome> {
    let hint = store.hints.get(&level).copied();
    let Some(end) = hof.last_page_with(session, level, hint).await? else {
        report!("[nakeds] Level {level}: nobody that high in the Hall of Fame");
        store.misses.insert(level, today.to_string());
        return Ok(());
    };
    store.hints.insert(level, end);
    let last = hof.last_page(session).await?;
    // From the end of the range upwards (lowest honors first), the page after it may hold a few of the level too
    let mut page = (end + 1).min(last);
    let mut cands: Vec<(u32, String)> = Vec::new();
    for visited in 1.. {
        let rows = hof.rows(session, page).await?;
        cands.extend(rows.iter().filter(|r| r.level == level).map(|r| (r.honor, r.name.clone())));
        let above = rows.iter().map(|r| r.level).min().is_some_and(|m| m > level);
        if cands.len() >= MAX_CANDIDATES || visited >= PAGES_PER_LEVEL || page == 0 || above {
            break;
        }
        page -= 1;
    }
    cands.sort();
    let (mut inspected, mut naked) = (0, 0);
    for (_, name) in cands {
        if inspected >= if naked > 0 { INSPECT_AFTER_FIND } else { INSPECT_NO_FIND } {
            break;
        }
        inspected += 1;
        let Some(s) = view(session, &name).await? else { continue };
        if s.level != level || !s.naked {
            continue;
        }
        naked += 1;
        if offer(store, &name, &s, today) {
            report!("[nakeds] Level {level}: {name} ({:?}, power {:.0}, honor {}) is the weakest naked player so far", s.class, s.power, s.honor);
        }
    }
    if store.entries.contains_key(&level) {
        store.misses.remove(&level);
    } else {
        report!("[nakeds] Level {level}: no naked player among {inspected} viewed");
        store.misses.insert(level, today.to_string());
    }
    Ok(())
}

/// Called from the main loop; only the scanner character does anything.
pub async fn run(session: &mut SimpleSession) -> Outcome {
    if crate::ctx::name() != SCANNER || session.game_state().is_none() {
        return Outcome::Done;
    }
    match scan(session).await {
        Ok(()) => Outcome::Done,
        Err(o) => o,
    }
}

async fn scan(session: &mut SimpleSession) -> Result<(), Outcome> {
    let now = Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let mut store = load();
    if store.date != today {
        store.date = today.clone();
        store.revalidated = false;
        store.searched.clear();
    }
    let recent_miss = |store: &Store, level: u32| {
        store
            .misses
            .get(&level)
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .is_some_and(|d| (now.date_naive() - d).num_days() < MISS_DAYS)
    };
    let todo = |store: &Store| -> Vec<u32> {
        (STEP..=MAX_LEVEL)
            .step_by(STEP as usize)
            .filter(|l| !store.entries.contains_key(l) && !store.searched.contains(l) && !recent_miss(store, *l))
            .collect()
    };
    if store.revalidated && todo(&store).is_empty() {
        return Ok(());
    }
    let deadline = Instant::now() + BUDGET;

    if !store.revalidated {
        let old: Vec<Entry> = store.entries.values().cloned().collect();
        store.entries.clear();
        for e in old {
            match view(session, &e.name).await {
                Ok(Some(s)) if offer(&mut store, &e.name, &s, &today) => {
                    if s.level != e.level {
                        report!("[nakeds] {} moved from level {} to {}", e.name, e.level, s.level);
                    }
                }
                Ok(other) => {
                    let why = if other.is_none() { "gone" } else { "dressed or off the multiples of 5" };
                    report!("[nakeds] Dropped {} (level {}): {why}", e.name, e.level);
                }
                Err(o) => return Err(o), // the file still has the old list
            }
        }
        store.revalidated = true;
        save(&store);
    }

    let mut hof = Hof { cache: HashMap::new(), total: None };
    for level in todo(&store) {
        if Instant::now() >= deadline {
            report!("[nakeds] Time budget used up, the scan continues in the next pass");
            break;
        }
        search(session, &mut hof, &mut store, level, &today).await?;
        store.searched.push(level);
        save(&store);
    }
    let found = store.entries.len();
    report!("[nakeds] {found}/{} levels have a naked player", MAX_LEVEL / STEP);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(level: u32, power: f64, naked: bool) -> Seen {
        Seen { level, honor: 100, rank: 1, class: Class::Warrior, naked, power }
    }

    #[test]
    fn weaker_naked_player_replaces_the_holder_only() {
        let mut s = Store::default();
        assert!(offer(&mut s, "Ozzy", &seen(15, 137.0, true), "d"));
        assert!(!offer(&mut s, "Strong", &seen(15, 150.0, true), "d"));
        assert!(offer(&mut s, "Brag", &seen(15, 120.0, true), "d"));
        assert_eq!(s.entries[&15].name, "Brag");
        assert!(!offer(&mut s, "Dressed", &seen(15, 10.0, false), "d"));
        assert!(!offer(&mut s, "Odd", &seen(16, 10.0, true), "d"));
        assert!(!offer(&mut s, "Big", &seen(205, 10.0, true), "d"));
    }
}
