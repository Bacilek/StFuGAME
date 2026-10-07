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

/// Hodnota setkání v hrdinství, včetně bonusu za plakát a hodnoty plakátu samotného.
fn encounter_value(exp: &Expedition, enc: &ExpeditionEncounter) -> i32 {
    let have = |t: ExpeditionThing| exp.items.iter().flatten().any(|i| *i == t);
    if enc.typ.is_bounty_for().is_some() {
        // Plakát dává 0, ale budoucímu „hledanému“ přidá +10
        return if have(enc.typ) { 0 } else { BOUNTY_BONUS };
    }
    let bonus = match enc.typ.required_bounty() {
        Some(poster) if have(poster) => BOUNTY_BONUS,
        _ => 0,
    };
    enc.heroism + bonus
}

/// Vybere setkání na rozcestí:
/// 1. cílový předmět expedice, dokud není úkol splněný,
/// 2. když je úkol splněný a máme 40+ hrdinství: klíč nebo truhla,
/// 3. jinak nejvyšší hrdinství (plakát se počítá jako +10).
fn choose_encounter(exp: &Expedition, encs: &[ExpeditionEncounter]) -> usize {
    let target_done = exp.target_current >= exp.target_amount;

    if !target_done && let Some(i) = encs.iter().position(|e| e.typ == exp.target_thing) {
        return i;
    }
    if target_done
        && exp.heroism >= MAX_HEROISM
        && let Some(i) = encs.iter().position(|e| matches!(e.typ, ExpeditionThing::Key | ExpeditionThing::Suitcase))
    {
        return i;
    }
    encs.iter()
        .enumerate()
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
    fn reward_order() {
        let r = |typ| Reward { typ, amount: 1 };
        assert_eq!(choose_reward(&[r(RewardType::QuicksandGlass), r(RewardType::Silver), r(RewardType::Mushrooms)]), 2);
        assert_eq!(choose_reward(&[r(RewardType::QuicksandGlass), r(RewardType::Silver)]), 1);
        assert_eq!(choose_reward(&[r(RewardType::XP), r(RewardType::QuicksandGlass)]), 1);
    }
}
