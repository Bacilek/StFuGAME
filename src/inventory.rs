//! Správa inventáře: každý předmět na nasazení v batohu porovná s tím, co má postava na sobě.
//! Lepší nasadí, horší prodá. Epické předměty se nikdy neprodávají (necháváme si je na později).
//! „Lepší“ = vyšší hodnota podle vzorce z arény (100 % hlavní + 80 % CON + 40 % LCK + 10 % vedlejší),
//! u zbraní podle očekávaného poškození (viz `weapon_value`).

use sf_api::{
    SimpleSession,
    command::{AttributeType, Command},
    gamestate::{
        GameState,
        character::Class,
        items::{BagPosition, EquipmentSlot, Item, ItemType},
    },
};

use crate::{arena::strength, safe, tavern::Outcome};

/// Pojistka proti nekonečné smyčce v jednom průchodu.
const MAX_ACTIONS: usize = 30;

/// Hodnota předmětu pro naši třídu.
pub fn score(class: Class, item: &Item) -> f64 {
    strength(class, |a: AttributeType| f64::from(item.attributes[a]))
}

/// Hodnota zbraně = průměrné poškození × (1 + M / 20).
/// Ze simulace boje v sf-api (simulate/damage.rs): poškození úderu = zbraň × (1 + A / 10),
/// kde A = max(M / 2, M − M_soupeře / 2); proti stejně silnému soupeři A = M / 2.
/// M = celkový hlavní atribut postavy, když má nasazenou právě tuhle zbraň.
pub fn weapon_value(avg_damage: f64, main_attr_with_weapon: f64) -> f64 {
    avg_damage * (1.0 + main_attr_with_weapon / 20.0)
}

/// Hodnota předmětu pro naši postavu: zbraně podle poškození, ostatní podle atributů.
fn value(gs: &GameState, item: &Item) -> f64 {
    let ch = &gs.character;
    let ItemType::Weapon { min_dmg, max_dmg } = item.typ else {
        return score(ch.class, item);
    };
    let main = ch.class.main_attribute();
    let total = f64::from(ch.attribute_basis[main] + ch.attribute_additions[main]);
    let equipped = ch.equipment.0[EquipmentSlot::Weapon].as_ref().map_or(0.0, |w| f64::from(w.attributes[main]));
    let with_this = total - equipped + f64::from(item.attributes[main]);
    weapon_value(f64::from(min_dmg + max_dmg) / 2.0, with_this)
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
        let new = value(gs, item);
        if !item.can_be_equipped_by(class) {
            if !item.is_epic() {
                return Some((Action::Sell { pos }, format!("prodávám {} (pro jinou třídu)", describe(item))));
            }
            continue;
        }
        let current = gs.character.equipment.0[slot].as_ref();
        let cur = current.map_or(-1.0, |i| value(gs, i));
        if new > cur {
            let what = match current {
                Some(c) => format!("nasazuji {} (hodnota {new:.1} > {cur:.1} u {})", describe(item), describe(c)),
                None => format!("nasazuji {} do prázdného slotu {slot:?} (hodnota {new:.1})", describe(item)),
            };
            return Some((Action::Equip { pos, slot }, what));
        }
        if !item.is_epic() {
            return Some((
                Action::Sell { pos },
                format!("prodávám {} (hodnota {new:.1} ≤ {cur:.1} nasazeného)", describe(item)),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-07: zbraň 13–15 vs 9–15. Vyšší poškození vyhraje, pokud nová nepřidá hodně hlavního atributu.
    #[test]
    fn weapon_damage_matters() {
        // stejný hlavní atribut postavy: 14 × 3,5 > 12 × 3,5
        assert!(weapon_value(14.0, 50.0) > weapon_value(12.0, 50.0));
        // nová přidá +6 hlavního atributu: 12 × (1 + 56/20) = 45,6 < 14 × (1 + 50/20) = 49
        assert!(weapon_value(14.0, 50.0) > weapon_value(12.0, 56.0));
        // při malém atributu postavy rozhoduje víc atribut: 12 × (1 + 16/20) = 21,6 > 14 × (1 + 10/20) = 21
        assert!(weapon_value(12.0, 16.0) > weapon_value(14.0, 10.0));
    }
}
