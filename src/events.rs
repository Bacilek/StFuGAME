//! The Exceptional XP event (the only one that costs mushrooms) switched on BY HAND in the app's "Events" tab (user 2026-10-16), each with an end time.
//!
//! Why by hand: the XP event lets the bot drink beers for mushrooms (the premium currency); a forgotten switch must never
//! keep spending them. So an event counts as active for the bot only while ALL of these hold (`tasks::event_active`):
//! the user switched it on here, its end time lies in the future, the server's own event list contains it and it is
//! Friday–Sunday. Everything fails closed: a missing/unreadable/garbled file, an unparsable time or a past end time means
//! "not active". There is no timer that has to fire: the end time is compared with the clock at every single decision
//! (and again in `safe.rs` right before `BuyBeer`), so an expired event stops mattering at that very second whether or
//! not the app window is open. The end time is also clamped to the coming Sunday 23:59:59 when saving.
//!
//! File `roster/events.json` (local only): `{"ExceptionalXPEvent": "2026-10-18T23:59:00+02:00", …}`; no entry = off.

use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, Duration, Local, NaiveTime};
use sf_api::gamestate::rewards::Event;

const FILE: &str = "roster/events.json";

/// Events the user can switch on: (event, name in the game, what the bot does differently).
pub const EVENTS: [(Event, &str, &str); 1] = [(Event::ExceptionalXPEvent, "Exceptional XP", "Beers for mushrooms in the Tavern (up to 10 a day)")];

fn key(event: Event) -> String {
    format!("{event:?}")
}

/// The stored end times (empty on any problem: fail closed).
fn read() -> BTreeMap<String, DateTime<Local>> {
    let Ok(text) = std::fs::read_to_string(FILE) else { return BTreeMap::new() };
    let Ok(raw) = serde_json::from_str::<BTreeMap<String, String>>(&text) else { return BTreeMap::new() };
    raw.into_iter()
        .filter_map(|(k, v)| Some((k, DateTime::parse_from_rfc3339(&v).ok()?.with_timezone(&Local))))
        .collect()
}

/// Switched on by hand and not yet past its end time?
fn active_at(map: &BTreeMap<String, DateTime<Local>>, event: Event, now: DateTime<Local>) -> bool {
    map.get(&key(event)).is_some_and(|until| *until > now)
}

/// May the bot treat this event as running as far as the user's own switch is concerned? (Read fresh every call.)
pub fn manual_active(event: Event) -> bool {
    active_at(&read(), event, Local::now())
}

/// The end time to store: nothing when it is not in the future, never later than the coming Sunday 23:59:59 (the events
/// run Friday 00:01 – Sunday 23:59, so a longer time can only be a slip).
fn clamp_until(until: DateTime<Local>, now: DateTime<Local>) -> Option<DateTime<Local>> {
    if until <= now {
        return None;
    }
    let to_sunday = i64::from(6 - now.weekday().num_days_from_monday());
    let sunday = (now.date_naive() + Duration::days(to_sunday)).and_time(NaiveTime::from_hms_opt(23, 59, 59)?);
    let max = sunday.and_local_timezone(Local).latest()?;
    Some(until.min(max))
}

/// Saves what the Events tab sent: `{"ExceptionalXPEvent": "<RFC 3339 time>" | null, …}`. Replaces the whole file, so an
/// event missing from the message is off.
pub fn set_from_ui(v: &serde_json::Value) {
    let now = Local::now();
    let mut out = BTreeMap::new();
    for (event, name, _) in EVENTS {
        let Some(s) = v[key(event)].as_str() else { continue };
        let Some(until) = DateTime::parse_from_rfc3339(s).ok().map(|t| t.with_timezone(&Local)) else {
            report!("[events] {name}: could not read the end time {s:?}, left off");
            continue;
        };
        match clamp_until(until, now) {
            Some(u) => {
                if u != until {
                    report!("[events] {name}: end time {until} is past the coming Sunday, using {u}");
                }
                out.insert(key(event), u.to_rfc3339());
            }
            None => report!("[events] {name}: end time {until} is not in the future, left off"),
        }
    }
    let before = read();
    for (event, name, _) in EVENTS {
        let (was, is) = (active_at(&before, event, now), out.contains_key(&key(event)));
        if was != is || (is && before.get(&key(event)).map(|t| t.to_rfc3339()) != out.get(&key(event)).cloned()) {
            match out.get(&key(event)) {
                Some(t) => report!("[events] {name}: ON until {t}"),
                None => report!("[events] {name}: OFF"),
            }
        }
    }
    let _ = std::fs::create_dir_all("roster");
    let tmp = format!("{FILE}.tmp");
    // Write + rename: a crash in the middle never leaves half a file (which would read as "all off" anyway)
    if serde_json::to_string_pretty(&out).is_ok_and(|s| std::fs::write(&tmp, s).is_ok()) && std::fs::rename(&tmp, FILE).is_err() {
        report!("[events] Could not save {FILE}");
    }
}

/// For the app window: every event with its switch state (`until_local` is "YYYY-MM-DDTHH:MM" for the datetime input).
pub fn status_json() -> serde_json::Value {
    let map = read();
    let now = Local::now();
    let rows: Vec<_> = EVENTS
        .iter()
        .map(|(event, name, rule)| {
            let until = map.get(&key(*event)).copied();
            serde_json::json!({
                "key": key(*event), "name": name, "rule": rule,
                "active": active_at(&map, *event, now),
                "until_local": until.map(|t| t.format("%Y-%m-%dT%H:%M").to_string()),
                "left_sec": until.map(|t| (t - now).num_seconds()),
            })
        })
        .collect();
    serde_json::Value::Array(rows)
}

static LAST_WARN: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// The server runs the XP event but the user has not switched it on (or it ran out): say so in the log at most every 30 min,
/// so a missing switch does not look like a broken bot (no beers are drunk then).
pub fn warn_if_unswitched(gs: &sf_api::gamestate::GameState) {
    use std::sync::atomic::Ordering;
    let xp = Event::ExceptionalXPEvent;
    if !gs.specials.events.active.contains(&xp) || manual_active(xp) {
        return;
    }
    let now = Local::now().timestamp();
    if now - LAST_WARN.load(Ordering::Relaxed) >= 1800 {
        LAST_WARN.store(now, Ordering::Relaxed);
        report!("[events] The server runs Exceptional XP but it is not switched on in the app (Events tab, or its end time has passed): no beers");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, mi, 0).unwrap()
    }

    #[test]
    fn active_only_before_the_end_time() {
        let mut map = BTreeMap::new();
        map.insert(key(Event::ExceptionalXPEvent), at(2026, 10, 18, 23, 59));
        let e = Event::ExceptionalXPEvent;
        assert!(active_at(&map, e, at(2026, 10, 18, 23, 58)));
        assert!(!active_at(&map, e, at(2026, 10, 18, 23, 59)), "the end second itself is already over");
        assert!(!active_at(&map, e, at(2026, 10, 19, 8, 0)), "Monday morning: off even if nobody switched it off");
        assert!(!active_at(&map, Event::GloriousGoldGalore, at(2026, 10, 17, 12, 0)), "not switched on");
        assert!(!active_at(&BTreeMap::new(), e, at(2026, 10, 17, 12, 0)), "empty/unreadable file = off");
    }

    #[test]
    fn end_time_is_clamped_to_the_coming_sunday() {
        let fri = at(2026, 10, 16, 12, 0);
        let sunday_end = Local.with_ymd_and_hms(2026, 10, 18, 23, 59, 59).unwrap();
        assert_eq!(clamp_until(at(2026, 10, 18, 23, 59), fri), Some(at(2026, 10, 18, 23, 59)));
        assert_eq!(clamp_until(at(2026, 10, 25, 23, 59), fri), Some(sunday_end), "a week too long");
        assert_eq!(clamp_until(at(2026, 10, 19, 6, 0), fri), Some(sunday_end), "Monday morning slip");
        assert_eq!(clamp_until(at(2026, 10, 16, 11, 0), fri), None, "in the past");
        let sun = at(2026, 10, 18, 20, 0);
        assert_eq!(clamp_until(at(2026, 10, 19, 6, 0), sun), Some(sunday_end), "on Sunday itself");
    }
}
