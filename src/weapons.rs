//! Weapon swap log (user 2026-10-27): every time a character puts on a better weapon, one JSON line goes to `roster/weapons.jsonl`
//! (old and new weapon with gem, damage/HP/crit and attribute totals before and after). The app's "Weapons" tab shows them, so the
//! user can check that we really swap weapons when we should and do not give up too many stats for a bit more damage.

use sf_api::gamestate::{
    GameState,
    items::{EquipmentSlot, Item, ItemType},
};

/// A swap that is about to happen: everything that must be read before the `Equip` command.
pub struct Pending {
    slot: EquipmentSlot,
    old: Option<serde_json::Value>,
    new: serde_json::Value,
    before: serde_json::Value,
    why: String,
}

/// Call before `Command::Equip`; `None` when the item is not a weapon (nothing to log).
pub fn prepare(gs: &GameState, item: &Item, slot: EquipmentSlot, why: &str) -> Option<Pending> {
    if !matches!(item.typ, ItemType::Weapon { .. }) {
        return None;
    }
    Some(Pending {
        slot,
        old: gs.character.equipment.0[slot].as_ref().map(crate::roster::item_card),
        new: crate::roster::item_card(item),
        before: crate::roster::stat_line(gs),
        why: why.to_string(),
    })
}

impl Pending {
    /// Call with the refreshed state after the `Equip` + `Update`. Nothing is written when the new weapon is not worn.
    pub fn finish(self, gs: &GameState) {
        use std::io::Write;
        let worn = gs.character.equipment.0[self.slot].as_ref().map(crate::roster::item_card);
        if worn.as_ref().map(|w| &w["d"]) != Some(&self.new["d"]) {
            return;
        }
        let line = serde_json::json!({
            "time": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            "character": crate::ctx::name(),
            "level": gs.character.level,
            "slot": format!("{:?}", self.slot),
            "old": self.old,
            "new": self.new,
            "before": self.before,
            "after": crate::roster::stat_line(gs),
            "why": self.why,
        });
        let _ = std::fs::create_dir_all("roster");
        match std::fs::OpenOptions::new().create(true).append(true).open("roster/weapons.jsonl") {
            Ok(mut f) => {
                let _ = writeln!(f, "{line}");
            }
            Err(e) => report!("[weapons] Could not log a weapon swap: {e}"),
        }
    }
}
