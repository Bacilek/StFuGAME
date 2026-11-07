//! One-shot probe of the toilet's Wash cycle (user 2026-11-07, `docs/tower.md`): the client sends an EMPTY `PlayerToilettWash:` and no
//! other request but `Poll`, so how the server learns which item to wash is unknown. Runs once for a character that has the marker file
//! `roster/<character>/wash_probe.todo` (the file is deleted first, so it never repeats). Tries a bare Wash, then `ToiletDrop` + Wash on
//! a backpack epic, and writes every answer to the log. Neither command costs mushrooms; no flush is sent here.

use sf_api::{
    command::Command,
    gamestate::GameState,
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

fn marker() -> std::path::PathBuf {
    std::path::Path::new("roster").join(crate::ctx::name()).join("wash_probe.todo")
}

/// Backpack slots as (position, "type, washed?") for the log.
fn washed_slots(gs: &GameState) -> Vec<String> {
    gs.character
        .inventory
        .iter()
        .filter_map(|(bp, item)| item.filter(|i| i.is_washed).map(|i| format!("{:?}={:?}", sf_api::gamestate::items::PlayerItemPosition::from(bp), i.typ)))
        .collect()
}

fn short(s: &str) -> String {
    let head: String = s.chars().take(160).collect();
    format!("{head} ({} chars)", s.len())
}

fn wash_cmd() -> Command {
    Command::Custom { cmd_name: "PlayerToilettWash".to_string(), arguments: Vec::new() }
}

pub async fn run(session: &mut SimpleSession) -> Outcome {
    if !marker().exists() {
        return Outcome::Done;
    }
    let Some(gs) = session.game_state() else { return Outcome::Done };
    if !crate::toilet::unlocked(gs) {
        return Outcome::Done;
    }
    let pick = gs
        .character
        .inventory
        .iter()
        .find(|(_, item)| item.is_some_and(|i| i.is_epic() && !i.is_washed && crate::toilet::throwable(i) && i.typ.equipment_slot().is_some()))
        .map(|(bp, item)| (bp, item.map(|i| format!("{:?}", i.typ)).unwrap_or_default()));
    let Some((bp, what)) = pick else {
        report!("[wash-probe] No unwashed backpack epic, waiting");
        return Outcome::Done;
    };
    let _ = std::fs::remove_file(marker());
    let pos = sf_api::gamestate::items::PlayerItemPosition::from(bp);
    report!("[wash-probe] Start: epic {what} at {pos:?}, already washed: {:?}", washed_slots(gs));

    // A: bare Wash
    match safe::send_raw(session, wash_cmd()).await {
        Ok(raw) => report!("[wash-probe] A bare Wash -> OK {}", short(&raw)),
        Err(e) => report!("[wash-probe] A bare Wash -> {e}"),
    }
    if let Err(e) = safe::send(session, Command::Update).await {
        report!("[wash-probe] Update failed: {e}");
        return Outcome::Done;
    }
    let after_a = session.game_state().map(washed_slots).unwrap_or_default();
    report!("[wash-probe] After A washed: {after_a:?}");
    if after_a.iter().any(|s| s.contains(&what)) {
        report!("[wash-probe] A washed the item without a Load");
        return Outcome::Done;
    }

    // B: ToiletDrop (puts the item in the toilet, nothing is lost until a flush) then Wash
    match safe::send_raw(session, Command::ToiletDrop { item_pos: pos }).await {
        Ok(raw) => report!("[wash-probe] B Load -> OK {}", short(&raw)),
        Err(e) => report!("[wash-probe] B Load -> {e}"),
    }
    match safe::send_raw(session, wash_cmd()).await {
        Ok(raw) => report!("[wash-probe] B Wash -> OK {}", short(&raw)),
        Err(e) => report!("[wash-probe] B Wash -> {e}"),
    }
    if let Err(e) = safe::send(session, Command::Update).await {
        report!("[wash-probe] Update failed: {e}");
        return Outcome::Done;
    }
    let now = session.game_state().map(washed_slots).unwrap_or_default();
    report!("[wash-probe] After B washed: {now:?}");
    Outcome::Done
}
