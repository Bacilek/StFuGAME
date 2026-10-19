//! Witch (unlocks at character level 66), phase 1: READ ONLY (user 2026-10-19). Parses the raw `witchshop` ourselves because sf-api
//! 0.4.4 misreads which enchantments are unlocked (see `docs/witch.md`), keeps a snapshot in `roster/<character>/witch.json` and
//! says what changed. Nothing is sent to the server here; the cauldron and the enchanting come in later phases.

use chrono::{DateTime, Local, TimeZone};

use crate::session::SimpleSession;

/// One enchantment line of the raw `witchshop`: `(item type id, enchantment id, unlock timestamp)`, timestamp 0 = locked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enchant {
    pub slot: i64,
    pub id: i64,
    pub unlocked_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitchInfo {
    /// Item type id the witch wants today (`None` = the cauldron is bubbling, nothing wanted)
    pub wanted_slot: Option<i64>,
    pub price_silver: u64,
    /// Items still missing for the next enchantment (raw numbers 2 and 3 of the response), only meaningful when something is wanted
    pub progress: (i64, i64),
    pub enchants: Vec<Enchant>,
}

/// Item type ids as the game numbers them (`EquipmentSlot::raw_id`).
pub fn slot_name(id: i64) -> &'static str {
    match id {
        1 => "Weapon",
        2 => "Shield",
        3 => "Breast plate",
        4 => "Boots",
        5 => "Gloves",
        6 => "Helmet",
        7 => "Belt",
        8 => "Amulet",
        9 => "Ring",
        10 => "Talisman",
        _ => "?",
    }
}

/// Layout (verified 2026-10-19 against the game): idx 2/3 progress, idx 5.. nine triples `(slot, enchant id, unlock timestamp)`,
/// idx 33 = 1 when the witch wants nothing, else idx 34 = the wanted item type, idx 35 = enchant price in silver.
pub fn parse(n: &[i64]) -> Option<WitchInfo> {
    if n.len() < 36 {
        return None;
    }
    let enchants = (0..9)
        .map(|i| Enchant { slot: n[5 + 3 * i], id: n[6 + 3 * i], unlocked_at: n[7 + 3 * i] })
        .collect();
    Some(WitchInfo {
        wanted_slot: (n[33] == 0).then_some(n[34]),
        price_silver: u64::try_from(n[35]).ok()?,
        progress: (n[2], n[3]),
        enchants,
    })
}

impl WitchInfo {
    pub fn unlocked(&self) -> impl Iterator<Item = &Enchant> {
        self.enchants.iter().filter(|e| e.unlocked_at != 0)
    }

    /// One line for the log: what the witch wants, the price and the unlocked enchantments.
    pub fn summary(&self) -> String {
        let wanted = self.wanted_slot.map_or_else(|| "nothing (cauldron bubbling)".to_string(), |s| slot_name(s).to_string());
        let unlocked: Vec<String> = self.unlocked().map(|e| format!("{} ({})", slot_name(e.slot), e.id)).collect();
        format!(
            "wants {wanted}, enchant price {}, unlocked {}/9: {}",
            crate::report::gold(self.price_silver),
            unlocked.len(),
            if unlocked.is_empty() { "-".to_string() } else { unlocked.join(", ") }
        )
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "wanted": self.wanted_slot.map(slot_name),
            "price_silver": self.price_silver,
            "progress": [self.progress.0, self.progress.1],
            "enchants": self.enchants.iter().map(|e| serde_json::json!({
                "item": slot_name(e.slot), "id": e.id, "unlocked": e.unlocked_at != 0,
                "unlocked_at": (e.unlocked_at != 0).then(|| ts(e.unlocked_at).map(|t| t.to_rfc3339())),
            })).collect::<Vec<_>>(),
            "updated": Local::now().to_rfc3339(),
        })
    }
}

fn ts(secs: i64) -> Option<DateTime<Local>> {
    Local.timestamp_opt(secs, 0).single()
}

/// The last summary logged (per character), so only a change is reported.
static LAST: crate::ctx::PerChar<String> = crate::ctx::PerChar::new();

/// Phase 1: from level 66 on, read the witch from the latest `witchshop`, write `roster/<character>/witch.json` and report a change.
/// Sends nothing. The `witchshop` arrives with the login; later responses refresh it only if the game sends it again.
pub fn observe(session: &SimpleSession) {
    let Some(gs) = session.game_state() else { return };
    if gs.character.level < 66 {
        return;
    }
    let Some(raw) = session.witch_shop() else { return };
    let Some(info) = parse(raw) else {
        report!("[witch] The witchshop data has an unexpected shape ({} numbers), not parsed", raw.len());
        return;
    };
    let summary = info.summary();
    let Ok(mut last) = LAST.lock() else { return };
    if *last == summary {
        return;
    }
    *last = summary.clone();
    report!("[witch] {summary}");
    let path = std::path::Path::new("roster").join(crate::ctx::name()).join("witch.json");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, info.json().to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The capture of 2026-10-19 (docs/witch.md): only the ring enchantment is unlocked, the cauldron is bubbling, price 2041 gold.
    const CAPTURE: &str = "1792368000/1792421040/1250000/157821/1/5/51/0/9/91/1792068112/6/61/0/8/81/0/10/101/0/3/31/0/4/41/0/7/71/0/1/11/0/1/1/9/204100";

    #[test]
    fn parses_the_capture() {
        let n: Vec<i64> = CAPTURE.split('/').map(|s| s.parse().unwrap()).collect();
        let w = parse(&n).expect("parses");
        assert_eq!(w.wanted_slot, None);
        assert_eq!(w.price_silver, 204_100);
        assert_eq!(w.enchants.len(), 9);
        let unlocked: Vec<_> = w.unlocked().collect();
        assert_eq!(unlocked.len(), 1);
        assert_eq!((slot_name(unlocked[0].slot), unlocked[0].id), ("Ring", 91));
    }

    #[test]
    fn short_data_is_rejected() {
        assert!(parse(&[1, 2, 3]).is_none());
    }
}
