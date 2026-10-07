//! Hospoda: expedice s výběry (rozcestí, boss, odměny, čekání).

use std::time::Duration;

use chrono::Local;
use sf_api::{
    SimpleSession,
    command::Command,
    gamestate::{
        rewards::{Reward, RewardType},
        tavern::{
            AvailableExpedition, AvailableTasks, CurrentAction, Expedition, ExpeditionEncounter, ExpeditionStage,
            ExpeditionThing,
        },
    },
};

use crate::safe;

/// Od tohoto hrdinství je odměna maximální, další už nepotřebujeme.
const MAX_HEROISM: i32 = 40;
/// Bonus za věc, ke které máme plakát „hledá se“ (wanted).
const BOUNTY_BONUS: i32 = 10;
/// Pojistka proti nekonečné smyčce.
const MAX_STEPS: u32 = 300;

/// Vybere expedici: přednostně se speciální odměnou (vejce, denní úkol), jinak nejlevnější v ALU.
/// Vrací jen expedice, na které máme dost ALU.
fn choose_expedition(list: &[AvailableExpedition], thirst: u32) -> Option<usize> {
    list.iter()
        .enumerate()
        .filter(|(_, e)| e.thirst_for_adventure_sec <= thirst)
        .min_by_key(|(_, e)| (e.special.is_none(), e.thirst_for_adventure_sec))
        .map(|(i, _)| i)
}

/// Poslední kolo expedice. V něm nemá smysl brát „přípravné“ věci.
const LAST_FLOOR: u8 = 10;
/// Bonus na konci expedice za každý kus rozbitého meče, když je cílem.
const BROKEN_SWORD_END_BONUS: i32 = 8;

fn has(exp: &Expedition, t: ExpeditionThing) -> bool {
    exp.items.iter().flatten().any(|i| *i == t)
}

/// Přípravná věc: sama nic nedá (nebo ubere), ale odemkne bonus později.
/// Plakát (+10 k hledanému), klíč (k truhle), princezna (`Bait`, odemkne draka +10).
fn is_setup(t: ExpeditionThing) -> bool {
    t.is_bounty_for().is_some() || matches!(t, ExpeditionThing::Key | ExpeditionThing::Bait)
}

/// Budoucí hodnota přípravné věci (0, pokud ji už máme nebo je poslední kolo).
fn future_value(exp: &Expedition, t: ExpeditionThing) -> i32 {
    if exp.current_floor >= LAST_FLOOR || has(exp, t) {
        return 0;
    }
    if t.is_bounty_for().is_some() || t == ExpeditionThing::Bait {
        BOUNTY_BONUS
    } else {
        0
    }
}

/// Hodnota setkání v hrdinství: základ + bonus za plakát + budoucí hodnota přípravné věci.
fn encounter_value(exp: &Expedition, enc: &ExpeditionEncounter) -> i32 {
    let bonus = match enc.typ.required_bounty() {
        Some(poster) if has(exp, poster) => BOUNTY_BONUS,
        _ => 0,
    };
    enc.heroism + bonus + future_value(exp, enc.typ)
}

/// Hrdinství, které budeme mít na konci, včetně bonusů připsaných až na konci.
fn projected_heroism(exp: &Expedition) -> i32 {
    let end_bonus = match exp.target_thing {
        ExpeditionThing::BrokenSword => BROKEN_SWORD_END_BONUS * i32::from(exp.target_current),
        _ => 0,
    };
    exp.heroism + end_bonus
}

/// Vybere setkání na rozcestí:
/// 0. v posledním kole se neberou přípravné věci (plakát, klíč, princezna), pokud je jiná možnost,
/// 1. cílový předmět expedice, dokud není úkol splněný,
/// 2. když je úkol splněný a hrdinství (vč. bonusů na konci) je 40+: klíč nebo truhla,
/// 3. jinak nejvyšší hodnota (plakát/princezna se počítají jako +10 do budoucna).
fn choose_encounter(exp: &Expedition, encs: &[ExpeditionEncounter]) -> usize {
    let last = exp.current_floor >= LAST_FLOOR;
    let allowed = |e: &ExpeditionEncounter| !(last && is_setup(e.typ));
    let any_allowed = encs.iter().any(allowed);
    let ok = |e: &ExpeditionEncounter| !any_allowed || allowed(e);

    let find = |pred: &dyn Fn(&ExpeditionEncounter) -> bool| encs.iter().position(|e| ok(e) && pred(e));

    let target_done = exp.target_current >= exp.target_amount;
    if !target_done && let Some(i) = find(&|e| e.typ == exp.target_thing) {
        return i;
    }
    if target_done && projected_heroism(exp) >= MAX_HEROISM {
        let need_key = !has(exp, ExpeditionThing::Key);
        let key_or_chest =
            |e: &ExpeditionEncounter| (need_key && e.typ == ExpeditionThing::Key) || e.typ == ExpeditionThing::Suitcase;
        if let Some(i) = find(&key_or_chest) {
            return i;
        }
    }
    encs.iter()
        .enumerate()
        .filter(|(_, e)| ok(e))
        .max_by_key(|(i, e)| (encounter_value(exp, e), std::cmp::Reverse(*i)))
        .map_or(0, |(i, _)| i)
}

/// Vybere odměnu: houby > zlato > přesýpací hodiny > cokoli.
fn choose_reward(rewards: &[Reward]) -> usize {
    let rank = |t: &RewardType| match t {
        RewardType::Mushrooms => 0,
        RewardType::Silver => 1,
        RewardType::QuicksandGlass => 2,
        _ => 3,
    };
    rewards
        .iter()
        .enumerate()
        .min_by_key(|(i, r)| (rank(&r.typ), *i))
        .map_or(0, |(i, _)| i)
}

/// Odehraje expedice, dokud je ALU. Čekání vždy vyčká, nikdy nepřeskakuje.
pub async fn run(session: &mut SimpleSession) {
    let mut unknown_in_row = 0;

    for _ in 0..MAX_STEPS {
        let Some(gs) = session.game_state() else {
            println!("[hospoda] Chybí stav hry, končím");
            return;
        };
        let tavern = &gs.tavern;

        let cmd = if let Some(exp) = tavern.expeditions.active() {
            match exp.current_stage() {
                ExpeditionStage::Encounters(encs) if !encs.is_empty() => {
                    unknown_in_row = 0;
                    let pos = choose_encounter(exp, &encs);
                    let opts: Vec<String> = encs.iter().map(|e| format!("{:?}({})", e.typ, e.heroism)).collect();
                    println!(
                        "[hospoda] Patro {}, hrdinství {}, cíl {:?} {}/{} | možnosti: {} → beru {:?}",
                        exp.current_floor,
                        exp.heroism,
                        exp.target_thing,
                        exp.target_current,
                        exp.target_amount,
                        opts.join(", "),
                        encs[pos].typ
                    );
                    Command::ExpeditionPickEncounter { pos }
                }
                ExpeditionStage::Boss(_) => {
                    unknown_in_row = 0;
                    println!("[hospoda] Boss, bojuji");
                    Command::ExpeditionContinue
                }
                ExpeditionStage::Rewards(rewards) if !rewards.is_empty() => {
                    unknown_in_row = 0;
                    let pos = choose_reward(&rewards);
                    let opts: Vec<String> = rewards.iter().map(|r| format!("{:?} x{}", r.typ, r.amount)).collect();
                    println!("[hospoda] Odměny: {} → beru {:?}", opts.join(", "), rewards[pos].typ);
                    Command::ExpeditionPickReward { pos }
                }
                ExpeditionStage::Waiting { busy_until, .. } => {
                    unknown_in_row = 0;
                    let secs = (busy_until - Local::now()).num_seconds().max(0) as u64;
                    let extra = fastrand::u64(5..30);
                    println!(
                        "[hospoda] Čekám do {} ({} min {} s)",
                        busy_until.format("%H:%M:%S"),
                        secs / 60,
                        secs % 60
                    );
                    tokio::time::sleep(Duration::from_secs(secs + extra)).await;
                    Command::Update
                }
                _ => {
                    unknown_in_row += 1;
                    if unknown_in_row > 2 {
                        println!("[hospoda] Neznámý stav expedice, končím");
                        return;
                    }
                    Command::Update
                }
            }
        } else {
            match tavern.current_action {
                // Poslední časovač doběhl, expedici je potřeba uzavřít
                CurrentAction::Expedition => {
                    unknown_in_row += 1;
                    if unknown_in_row > 2 {
                        println!("[hospoda] Expedici se nedaří uzavřít, končím");
                        return;
                    }
                    println!("[hospoda] Uzavírám dokončenou expedici");
                    Command::ExpeditionContinue
                }
                CurrentAction::Idle => match tavern.available_tasks() {
                    AvailableTasks::Expeditions(list) => {
                        let thirst = tavern.thirst_for_adventure_sec;
                        let Some(pos) = choose_expedition(list, thirst) else {
                            println!("[hospoda] Na další expedici není ALU ({} min), hotovo", thirst / 60);
                            return;
                        };
                        let e = &list[pos];
                        println!(
                            "[hospoda] Startuji expedici: cíl {:?}, {} min ALU, speciál {:?}",
                            e.target,
                            e.thirst_for_adventure_sec / 60,
                            e.special
                        );
                        unknown_in_row = 0;
                        Command::ExpeditionStart { pos }
                    }
                    AvailableTasks::Quests(_) => {
                        println!("[hospoda] Expedice nejsou dostupné (jen klasické questy), zatím nepodporuji");
                        return;
                    }
                },
                other => {
                    println!("[hospoda] Postava je zaneprázdněná ({other:?}), hospodu přeskakuji");
                    return;
                }
            }
        };

        if let Err(e) = safe::send(session, cmd).await {
            println!("[hospoda] Chyba: {e}");
            return;
        }
    }
    println!("[hospoda] Dosažen limit kroků, končím");
}

#[cfg(test)]
mod tests {
    use super::*;
    use ExpeditionThing::*;

    fn exp(target: ExpeditionThing, cur: u8, amount: u8, heroism: i32, items: [Option<ExpeditionThing>; 4]) -> Expedition {
        let mut e = Expedition::default();
        e.current_floor = 3;
        e.target_thing = target;
        e.target_current = cur;
        e.target_amount = amount;
        e.heroism = heroism;
        e.items = items;
        e
    }

    fn enc(typ: ExpeditionThing, heroism: i32) -> ExpeditionEncounter {
        ExpeditionEncounter { typ, heroism }
    }

    #[test]
    fn target_first() {
        let e = exp(Socks, 0, 2, 0, [None; 4]);
        assert_eq!(choose_encounter(&e, &[enc(Dragon, 20), enc(Socks, 1), enc(Key, 5)]), 1);
    }

    #[test]
    fn highest_heroism_when_no_target() {
        let e = exp(Socks, 0, 2, 0, [None; 4]);
        assert_eq!(choose_encounter(&e, &[enc(Dragon, 5), enc(Unicorn, 15), enc(Key, 5)]), 1);
    }

    #[test]
    fn poster_counts_as_ten() {
        let e = exp(Socks, 2, 2, 0, [None; 4]);
        assert_eq!(choose_encounter(&e, &[enc(Dragon, 8), enc(UnicornBounty, 0)]), 1);
        assert_eq!(choose_encounter(&e, &[enc(Dragon, 12), enc(UnicornBounty, 0)]), 0);
    }

    #[test]
    fn bounty_bonus_applies() {
        let e = exp(Socks, 2, 2, 0, [Some(UnicornBounty), None, None, None]);
        assert_eq!(choose_encounter(&e, &[enc(Dragon, 15), enc(Unicorn, 10)]), 1);
    }

    #[test]
    fn keys_and_chests_after_max_heroism() {
        let e = exp(Socks, 2, 2, 40, [None; 4]);
        assert_eq!(choose_encounter(&e, &[enc(Dragon, 20), enc(Suitcase, 0)]), 1);
        // úkol nesplněný → klíč nemá přednost
        let e = exp(Socks, 1, 2, 40, [None; 4]);
        assert_eq!(choose_encounter(&e, &[enc(Dragon, 20), enc(Suitcase, 0)]), 0);
    }

    #[test]
    fn no_setup_on_last_floor() {
        let mut e = exp(Socks, 2, 2, 10, [None; 4]);
        e.current_floor = 10;
        assert_eq!(choose_encounter(&e, &[enc(UnicornBounty, 0), enc(Dragon, 3)]), 1);
        assert_eq!(choose_encounter(&e, &[enc(Bait, -2), enc(Socks, 1)]), 1);
        // po dosažení 40 v posledním kole: truhla ano, klíč ne
        e.heroism = 40;
        assert_eq!(choose_encounter(&e, &[enc(Key, 0), enc(Dragon, 3), enc(Suitcase, 0)]), 2);
        assert_eq!(choose_encounter(&e, &[enc(Key, 0), enc(Dragon, 3)]), 1);
        // když jsou všechny možnosti přípravné, vezme se aspoň něco
        assert_eq!(choose_encounter(&e, &[enc(Key, 0), enc(UnicornBounty, 0)]), 0);
    }

    #[test]
    fn princess_counts_for_dragon() {
        let e = exp(Socks, 2, 2, 0, [None; 4]);
        assert_eq!(choose_encounter(&e, &[enc(Unicorn, 5), enc(Bait, -2)]), 1);
        // princeznu už máme → jen -2
        let e = exp(Socks, 2, 2, 0, [Some(Bait), None, None, None]);
        assert_eq!(choose_encounter(&e, &[enc(Unicorn, 5), enc(Bait, -2)]), 0);
    }

    #[test]
    fn broken_sword_end_bonus_counts_to_cap() {
        // 24 + 2 meče × 8 = 40 → klíč/truhla
        let e = exp(BrokenSword, 2, 2, 24, [None; 4]);
        assert_eq!(choose_encounter(&e, &[enc(Dragon, 10), enc(Key, 0)]), 1);
        let e = exp(Socks, 2, 2, 24, [None; 4]);
        assert_eq!(choose_encounter(&e, &[enc(Dragon, 10), enc(Key, 0)]), 0);
    }

    #[test]
    fn reward_order() {
        let r = |typ| Reward { typ, amount: 1 };
        assert_eq!(choose_reward(&[r(RewardType::QuicksandGlass), r(RewardType::Silver), r(RewardType::Mushrooms)]), 2);
        assert_eq!(choose_reward(&[r(RewardType::QuicksandGlass), r(RewardType::Silver)]), 1);
        assert_eq!(choose_reward(&[r(RewardType::XP), r(RewardType::QuicksandGlass)]), 1);
    }
}
