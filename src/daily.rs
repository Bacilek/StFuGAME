//! Denní odměny: odměna za přihlášení (kalendář) a jedno volné točení kolem štěstí denně.
//! Nic jiného (pravidlo uživatele). Kolo nikdy za houby ani šťastné mince.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use chrono::Local;
use sf_api::{
    SimpleSession,
    command::{Command, FortunePayment},
};

use crate::{safe, tavern::Outcome};

/// Když se akce nepovede (stav se nezměnil), zkusit ji znovu nejdřív za tuto dobu.
const RETRY: Duration = Duration::from_secs(30 * 60);

static TRIED: Mutex<Vec<(&'static str, Instant)>> = Mutex::new(Vec::new());

/// Smí se akce zkusit? Zaznamená pokus (kvůli opakování při neúspěchu).
fn may_try(what: &'static str) -> bool {
    let Ok(mut tried) = TRIED.lock() else { return false };
    if tried.iter().any(|(w, t)| *w == what && t.elapsed() < RETRY) {
        return false;
    }
    tried.retain(|(w, _)| *w != what);
    tried.push((what, Instant::now()));
    true
}

fn chyba(e: &sf_api::error::SFError) -> Outcome {
    report!("[odměny] Chyba: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Vybere všechno, co je dnes k vybrání. Když není co, nic neposílá.
pub async fn run(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let now = Local::now();
    let specials = &gs.specials;

    // Kalendář: jedna odměna denně
    let calendar_ready = specials.calendar.next_possible.is_some_and(|t| t <= now);
    if calendar_ready && may_try("kalendář") {
        let reward = specials.calendar.rewards.get(specials.calendar.collected).map(|r| format!("{:?} x{}", r.typ, r.amount));
        report!("[odměny] Vybírám denní odměnu za přihlášení: {}", reward.unwrap_or_else(|| "?".into()));
        if let Err(e) = safe::send(session, Command::CollectCalendar).await {
            return chyba(&e);
        }
    }

    // Kolo štěstí: jen volné točení (nikdy za houby ani mince)
    if session.game_state().is_some_and(safe::wheel_is_free) && may_try("kolo") {
        report!("[odměny] Točím kolem štěstí (zdarma)");
        match safe::send(session, Command::SpinWheelOfFortune { payment: FortunePayment::FreeTurn }).await {
            Ok(gs) => report!("[odměny] Kolo štěstí: {:?}", gs.specials.wheel.result),
            Err(e) => return chyba(&e),
        }
    }

    Outcome::Done
}

/// Za kolik sekund bude něco k vybrání (kalendář nebo volné točení), pokud víme.
pub fn secs_until_ready(gs: &sf_api::gamestate::GameState) -> Option<u64> {
    let now = Local::now();
    [gs.specials.calendar.next_possible, gs.specials.wheel.next_free_spin]
        .into_iter()
        .flatten()
        .filter(|t| *t > now)
        .map(|t| u64::try_from((t - now).num_seconds()).unwrap_or(0))
        .min()
}
