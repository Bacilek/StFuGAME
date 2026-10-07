//! Správa inventáře: každý předmět na nasazení v batohu porovná s tím, co má postava na sobě.
//! Lepší nasadí, horší prodá. Epické předměty se nikdy neprodávají (necháváme si je na později).
//! „Lepší“ = vyšší hodnota podle vzorce z arény (100 % hlavní + 80 % CON + 40 % LCK + 10 % vedlejší).

use sf_api::{
    SimpleSession,
    command::{AttributeType, Command},
    gamestate::{
        GameState,
        character::Class,
        items::{BagPosition, EquipmentSlot, Item},
    },
};

use crate::{arena::strength, safe, tavern::Outcome};

/// Pojistka proti nekonečné smyčce v jednom průchodu.
const MAX_ACTIONS: usize = 30;

/// Hodnota předmětu pro naši třídu.
pub fn score(class: Class, item: &Item) -> f64 {
    strength(class, |a: AttributeType| f64::from(item.attributes[a]))
}

#[derive(Debug)]
enum Action {
    Equip { pos: BagPosition, slot: EquipmentSlot },
    Sell { pos: BagPosition },
}

fn describe(item: &Item) -> String {
    format!("{:?}{}", item.typ, if item.is_epic() { " (epický)" } else { "" })
}

/// Další krok správy inventáře, nebo None, když není co dělat.
fn next_action(gs: &GameState) -> Option<(Action, String)> {
    let class = gs.character.class;
    for (pos, item) in gs.character.inventory.iter() {
        let Some(item) = item else { continue };
        let Some(slot) = item.typ.equipment_slot() else { continue };
        if item.is_unique() {
            continue;
        }
        let new = score(class, item);
        if !item.can_be_equipped_by(class) {
            if !item.is_epic() {
                return Some((Action::Sell { pos }, format!("prodávám {} (pro jinou třídu)", describe(item))));
            }
            continue;
        }
        let current = gs.character.equipment.0[slot].as_ref();
        let cur = current.map_or(-1.0, |i| score(class, i));
        if new > cur {
            let what = match current {
                Some(c) => format!("nasazuji {} (hodnota {new:.0} > {cur:.0} u {})", describe(item), describe(c)),
                None => format!("nasazuji {} do prázdného slotu {slot:?} (hodnota {new:.0})", describe(item)),
            };
            return Some((Action::Equip { pos, slot }, what));
        }
        if !item.is_epic() {
            return Some((
                Action::Sell { pos },
                format!("prodávám {} (hodnota {new:.0} ≤ {cur:.0} nasazeného)", describe(item)),
            ));
        }
    }
    None
}

fn chyba(e: &sf_api::error::SFError) -> Outcome {
    report!("[inventář] Chyba: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Projde inventář a provede všechna nasazení/prodeje. Když není co dělat, nic neposílá.
pub async fn manage(session: &mut SimpleSession) -> Outcome {
    for _ in 0..MAX_ACTIONS {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let Some((action, what)) = next_action(gs) else { return Outcome::Done };
        let pos_idx = match &action {
            Action::Equip { pos, .. } | Action::Sell { pos } => pos.backpack_pos(),
        };
        let Some(Some(item)) = gs.character.inventory.backpack.get(pos_idx) else { return Outcome::Done };
        let item_ident = item.command_ident();
        let price = item.price;

        report!("[inventář] {what}");
        let cmd = match action {
            Action::Equip { pos, slot } => Command::Equip { from_pos: pos.into(), to_slot: slot, item_ident },
            Action::Sell { pos } => Command::SellShop { item_pos: pos.into(), item_ident },
        };
        let selling = matches!(cmd, Command::SellShop { .. });
        if let Err(e) = safe::send(session, cmd).await {
            return chyba(&e);
        }
        if selling {
            report!("[inventář] Prodáno za {} g {} s", price / 100, price % 100);
        }
        // Rozhodovat vždy podle čerstvého stavu
        if let Err(e) = safe::send(session, Command::Update).await {
            return chyba(&e);
        }
    }
    report!("[inventář] Dosažen limit akcí v jednom průchodu");
    Outcome::Done
}
