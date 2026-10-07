//! Hospoda: expedice s výběry (rozcestí, boss, odměny, čekání).
//! Strategie: co nejdřív si zajistit 40 hrdinství (maximální odměna) a pak farmit klíče a truhly.

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

use crate::{
    journal::Journal,
    missions::{self, Bonus},
    safe,
};

/// Od tohoto hrdinství je odměna maximální, další už nepotřebujeme.
const MAX_HEROISM: i32 = 40;
/// Bonus za věc, ke které máme plakát „hledá se“ (wanted).
const BOUNTY_BONUS: i32 = 10;
/// Expedice má vždy 10 kol (rozcestí).
const LAST_FLOOR: u8 = 10;
/// Kolik hrdinství v průměru dá jedno kolo. Kolo strávené na přípravě (krok řetězu)
/// tedy „stojí“ tolik, kolik bychom jinak sebrali. Ladit podle deníku.
const OPPORTUNITY_COST: f64 = 4.0;
/// Odhad bonusu za cílový předmět neznámé mise (než ji zmapujeme).
const UNKNOWN_TARGET_GUESS: i32 = 5;
/// Pojistka proti nekonečné smyčce.
const MAX_STEPS: u32 = 300;

/// Vybere expedici: nejkratší (nejlevnější v ALU). Při stejné délce ta, kde je nejsnazší
/// získat 40 hrdinství; neznámá mise má při shodě přednost, abychom ji zmapovali.
fn choose_expedition(list: &[AvailableExpedition], thirst: u32) -> Option<usize> {
    let ease = |e: &AvailableExpedition| missions::for_target(e.target).map_or(f64::INFINITY, |m| m.ease());
    let mut best: Option<usize> = None;
    for (i, e) in list.iter().enumerate().filter(|(_, e)| e.thirst_for_adventure_sec <= thirst) {
        let better = match best {
            None => true,
            Some(b) => {
                let b = &list[b];
                e.thirst_for_adventure_sec < b.thirst_for_adventure_sec
                    || (e.thirst_for_adventure_sec == b.thirst_for_adventure_sec && ease(e) > ease(b))
            }
        };
        if better {
            best = Some(i);
        }
    }
    best
}

fn has(exp: &Expedition, t: ExpeditionThing) -> bool {
    exp.items.iter().flatten().any(|i| *i == t)
}

/// Kolik rozcestí ještě zbývá PO aktuálním výběru.
fn floors_after(exp: &Expedition) -> u8 {
    LAST_FLOOR.saturating_sub(exp.current_floor)
}

fn target_done(exp: &Expedition) -> bool {
    exp.target_current >= exp.target_amount
}

/// Odhad hrdinství na konci expedice, pokud už nic dalšího nesebereme:
/// aktuální + bonusy „za kus“ (připíšou se na konci) - trest za nesplněný úkol.
fn projected_heroism(exp: &Expedition) -> i32 {
    let mut p = exp.heroism;
    if let Some(m) = missions::for_target(exp.target_thing) {
        if let Bonus::PerItem(b) = m.bonus {
            p += b * i32::from(exp.target_current);
        }
        if !target_done(exp) {
            p -= m.fail_penalty;
        }
    }
    p
}

/// O kolik se změní odhad konečného hrdinství hned tímto výběrem.
fn immediate_gain(exp: &Expedition, enc: &ExpeditionEncounter) -> i32 {
    let mut g = enc.heroism;
    if let Some(poster) = enc.typ.required_bounty()
        && has(exp, poster)
    {
        g += BOUNTY_BONUS;
    }
    if enc.typ == exp.target_thing && missions::for_target(exp.target_thing).is_none() {
        g += UNKNOWN_TARGET_GUESS;
    }
    if enc.typ == exp.target_thing
        && let Some(m) = missions::for_target(exp.target_thing)
    {
        match m.bonus {
            Bonus::PerItem(b) => g += b,
            Bonus::OnComplete(b) => {
                if !target_done(exp) && exp.target_current + 1 >= exp.target_amount {
                    g += b + m.fail_penalty;
                }
            }
        }
    }
    g
}

/// Šance, že stihneme dalších `steps` potřebných kroků, když zbývá `floors` rozcestí.
fn feasibility(steps: u8, floors: u8) -> f64 {
    if steps == 0 {
        1.0
    } else if steps > floors {
        0.0
    } else if floors >= 2 * steps {
        0.7
    } else {
        0.35
    }
}

/// Budoucí hodnota výběru: co odemkne (plakát, krok řetězu) nebo k čemu se přiblíží
/// (počítací úkol), vážená šancí, že to stihneme. Krok, který nejde dokončit, má 0.
fn future_value(exp: &Expedition, enc: &ExpeditionEncounter) -> f64 {
    let floors = floors_after(exp);
    let t = enc.typ;

    if t.is_bounty_for().is_some() {
        return if has(exp, t) { 0.0 } else { f64::from(BOUNTY_BONUS) * feasibility(1, floors) };
    }
    let Some((m, idx)) = missions::chain_position(t) else {
        return 0.0;
    };
    let is_target = m.target() == exp.target_thing;
    let last_idx = m.chain.len() - 1;

    // Úkol s počtem kusů (např. 3× toaletní papír): každý kus nás přibližuje ke splnění
    if idx == last_idx {
        if is_target
            && let Bonus::OnComplete(b) = m.bonus
            && !target_done(exp)
        {
            let steps = exp.target_amount.saturating_sub(exp.target_current + 1);
            if steps > 0 {
                let payoff = f64::from(b + m.fail_penalty) - OPPORTUNITY_COST * f64::from(steps);
                return feasibility(steps, floors) * payoff.max(0.0);
            }
        }
        return 0.0;
    }

    // Krok řetězu: má smysl, jen pokud ho ještě nemáme a řetěz jde dokončit
    let held = m.chain.iter().rposition(|(c, _)| has(exp, *c));
    if held.is_some_and(|h| h >= idx) {
        return 0.0;
    }
    let steps = u8::try_from(last_idx - idx).unwrap_or(u8::MAX);
    let rest: i32 = m.chain[idx + 1..].iter().map(|(_, h)| h).sum();
    let bonus = match (is_target, m.bonus) {
        (false, _) => 0,
        (true, Bonus::PerItem(b)) => b,
        (true, Bonus::OnComplete(b)) if !target_done(exp) => b + m.fail_penalty,
        (true, Bonus::OnComplete(_)) => 0,
    };
    let payoff = f64::from(rest + bonus) - OPPORTUNITY_COST * f64::from(steps);
    feasibility(steps, floors) * payoff.max(0.0)
}

/// Vybere setkání na rozcestí.
///
/// Když je 40 zajištěno (i po započtení bonusů a trestů na konci): bere jen to, co nás
/// pod 40 nestáhne, a přednostně truhlu (máme-li klíč), jinak klíč (zbývá-li kolo na truhlu).
/// Jinak: nejvyšší součet okamžitého zisku a budoucí hodnoty.
fn choose_encounter(exp: &Expedition, encs: &[ExpeditionEncounter]) -> usize {
    let projected = projected_heroism(exp);

    if projected >= MAX_HEROISM {
        let safe: Vec<usize> = (0..encs.len())
            .filter(|&i| projected + immediate_gain(exp, &encs[i]) >= MAX_HEROISM)
            .collect();
        let pick = |t: ExpeditionThing| safe.iter().copied().find(|&i| encs[i].typ == t);
        if has(exp, ExpeditionThing::Key)
            && let Some(i) = pick(ExpeditionThing::Suitcase)
        {
            return i;
        }
        if floors_after(exp) > 0
            && let Some(i) = pick(ExpeditionThing::Key)
        {
            return i;
        }
        if let Some(&i) = safe.iter().max_by_key(|&&i| (immediate_gain(exp, &encs[i]), std::cmp::Reverse(i))) {
            return i;
        }
        // Všechno nás stáhne pod 40: vezmeme nejmenší ztrátu
        return (0..encs.len())
            .max_by_key(|&i| (immediate_gain(exp, &encs[i]), std::cmp::Reverse(i)))
            .unwrap_or(0);
    }

    let score = |e: &ExpeditionEncounter| f64::from(immediate_gain(exp, e)) + future_value(exp, e);
    let mut best = 0;
    for (i, e) in encs.iter().enumerate() {
        if score(e) > score(&encs[best]) {
            best = i;
        }
    }
    best
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

fn mission_name(t: ExpeditionThing) -> String {
    missions::for_target(t).map_or_else(|| format!("{t:?} (neznámá mise)"), |m| m.name.to_string())
}

/// Odehraje expedice, dokud je ALU. Čekání vždy vyčká, nikdy nepřeskakuje.
pub async fn run(session: &mut SimpleSession) {
    let mut unknown_in_row = 0;
    let mut journal = Journal::default();
    // sf-api obnoví nabídku rozcestí, jen když ji server pošle. Po výběru proto vždy
    // stáhneme čerstvý stav a nikdy nevybíráme dvakrát ze stejné (zastaralé) nabídky.
    let mut refresh_pending = true;
    let mut last_offer: Option<Vec<String>> = None;
    let mut stale_tries = 0;

    for _ in 0..MAX_STEPS {
        let Some(gs) = session.game_state() else {
            report!("[hospoda] Chybí stav hry, končím");
            return;
        };
        let tavern = &gs.tavern;

        let cmd = if let Some(exp) = tavern.expeditions.active() {
            let entry = journal.entry(&mission_name(exp.target_thing));
            entry.heroism = exp.heroism;
            entry.projected = projected_heroism(exp);
            entry.target_current = exp.target_current;
            entry.target_amount = exp.target_amount;

            match exp.current_stage() {
                ExpeditionStage::Encounters(encs) if !encs.is_empty() => {
                    unknown_in_row = 0;
                    let offer: Vec<String> = encs.iter().map(|e| format!("{:?}{}", e.typ, e.heroism)).collect();
                    let same_as_last = last_offer.as_ref() == Some(&offer);
                    if refresh_pending || same_as_last {
                        refresh_pending = false;
                        stale_tries += 1;
                        if stale_tries > 3 {
                            report!("[hospoda] Nabídka rozcestí se neobnovuje, končím (nechci vybírat naslepo)");
                            return;
                        }
                        if stale_tries > 1 {
                            report!("[hospoda] Nabídka je stejná jako minule, obnovuji stav ({stale_tries}. pokus)");
                        }
                        if let Err(e) = safe::send(session, Command::Update).await {
                            report!("[hospoda] Chyba: {e}");
                            return;
                        }
                        continue;
                    }
                    stale_tries = 0;
                    for u in encs.iter().filter(|e| !missions::is_known(e.typ)) {
                        let note = format!("{:?}({:+})", u.typ, u.heroism);
                        report!("[hospoda] Nezmapované setkání: {note}");
                        if !entry.unmapped.contains(&note) {
                            entry.unmapped.push(note);
                        }
                    }
                    let pos = choose_encounter(exp, &encs);
                    let opts: Vec<String> = encs
                        .iter()
                        .map(|e| {
                            format!("{:?}({:+}, budoucí {:.1})", e.typ, immediate_gain(exp, e), future_value(exp, e))
                        })
                        .collect();
                    let picked = encs[pos].typ;
                    report!(
                        "[hospoda] Kolo {}/{LAST_FLOOR}, hrdinství {} (odhad konce {}), {} {}/{} | {} → beru {picked:?}",
                        exp.current_floor,
                        exp.heroism,
                        projected_heroism(exp),
                        mission_name(exp.target_thing),
                        exp.target_current,
                        exp.target_amount,
                        opts.join(", "),
                    );
                    entry.picks.push(format!("{}: [{}] → {picked:?}", exp.current_floor, opts.join(", ")));
                    match picked {
                        ExpeditionThing::Key => entry.keys += 1,
                        ExpeditionThing::Suitcase => entry.chests += 1,
                        _ => {}
                    }
                    last_offer = Some(offer);
                    refresh_pending = true;
                    Command::ExpeditionPickEncounter { pos }
                }
                ExpeditionStage::Boss(_) => {
                    unknown_in_row = 0;
                    report!("[hospoda] Boss, bojuji");
                    Command::ExpeditionContinue
                }
                ExpeditionStage::Rewards(rewards) if !rewards.is_empty() => {
                    unknown_in_row = 0;
                    let pos = choose_reward(&rewards);
                    let opts: Vec<String> = rewards.iter().map(|r| format!("{:?} x{}", r.typ, r.amount)).collect();
                    report!("[hospoda] Odměny: {} → beru {:?}", opts.join(", "), rewards[pos].typ);
                    entry.rewards.push(format!("{:?} x{}", rewards[pos].typ, rewards[pos].amount));
                    Command::ExpeditionPickReward { pos }
                }
                ExpeditionStage::Waiting { busy_until, .. } => {
                    unknown_in_row = 0;
                    let secs = u64::try_from((busy_until - Local::now()).num_seconds()).unwrap_or(0);
                    let extra = fastrand::u64(5..30);
                    report!(
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
                        report!("[hospoda] Neznámý stav expedice, končím");
                        return;
                    }
                    Command::Update
                }
            }
        } else {
            journal.finish();
            match tavern.current_action {
                // Poslední časovač doběhl, expedici je potřeba uzavřít
                CurrentAction::Expedition => {
                    unknown_in_row += 1;
                    if unknown_in_row > 2 {
                        report!("[hospoda] Expedici se nedaří uzavřít, končím");
                        return;
                    }
                    report!("[hospoda] Uzavírám dokončenou expedici");
                    Command::ExpeditionContinue
                }
                CurrentAction::Idle => match tavern.available_tasks() {
                    AvailableTasks::Expeditions(list) => {
                        for e in list.iter().filter(|e| missions::for_target(e.target).is_none()) {
                            report!("[hospoda] Nabízí se nezmapovaná mise s cílem {:?}", e.target);
                        }
                        let thirst = tavern.thirst_for_adventure_sec;
                        let Some(pos) = choose_expedition(list, thirst) else {
                            report!(
                                "[hospoda] Žádná expedice, na kterou by stačilo ALU ({} min), hotovo",
                                thirst / 60
                            );
                            return;
                        };
                        let e = &list[pos];
                        report!(
                            "[hospoda] Startuji expedici: {}, {} min ALU, speciál {:?}",
                            mission_name(e.target),
                            e.thirst_for_adventure_sec / 60,
                            e.special
                        );
                        unknown_in_row = 0;
                        Command::ExpeditionStart { pos }
                    }
                    AvailableTasks::Quests(_) => {
                        report!("[hospoda] Expedice nejsou dostupné (jen klasické questy), zatím nepodporuji");
                        return;
                    }
                },
                other => {
                    report!("[hospoda] Postava je zaneprázdněná ({other:?}), hospodu přeskakuji");
                    return;
                }
            }
        };

        if let Err(e) = safe::send(session, cmd).await {
            report!("[hospoda] Chyba: {e}");
            return;
        }
    }
    report!("[hospoda] Dosažen limit kroků, končím");
}

#[cfg(test)]
mod tests {
    use super::*;
    use ExpeditionThing::*;

    fn exp(target: ExpeditionThing, cur: u8, amount: u8, heroism: i32, floor: u8) -> Expedition {
        let mut e = Expedition::default();
        e.target_thing = target;
        e.target_current = cur;
        e.target_amount = amount;
        e.heroism = heroism;
        e.current_floor = floor;
        e
    }

    fn with(mut e: Expedition, items: &[ExpeditionThing]) -> Expedition {
        for (slot, t) in e.items.iter_mut().zip(items) {
            *slot = Some(*t);
        }
        e
    }

    fn enc(typ: ExpeditionThing, heroism: i32) -> ExpeditionEncounter {
        ExpeditionEncounter { typ, heroism }
    }

    #[test]
    fn target_item_beats_small_points() {
        // Drak: +10 a +5 na konci
        let e = with(exp(Dragon, 0, 2, 0, 3), &[Bait]);
        assert_eq!(choose_encounter(&e, &[enc(Cake, 5), enc(Dragon, 10)]), 1);
    }

    #[test]
    fn poster_worth_ten_early() {
        let e = exp(Cake, 1, 1, 5, 2);
        assert_eq!(choose_encounter(&e, &[enc(Dummy1, 2), enc(UnicornBounty, 0)]), 1);
        assert_eq!(choose_encounter(&e, &[enc(Phoenix, 12), enc(UnicornBounty, 0)]), 0);
    }

    #[test]
    fn bounty_bonus_applies() {
        let e = with(exp(Cake, 1, 1, 5, 4), &[DummyBounty]);
        assert_eq!(choose_encounter(&e, &[enc(Cake, 5), enc(Dummy1, 2)]), 1);
    }

    /// Příklad 1: na 40 nebereme překážku, která by nás stáhla pod 40.
    #[test]
    fn secured_never_drops_below_40() {
        let e = exp(Cake, 1, 1, 40, 5);
        assert_eq!(choose_encounter(&e, &[enc(SmallHurdle, -1), enc(Dummy1, 2)]), 1);
    }

    /// Příklad 2: v posledním kole nezačínáme řetěz, který nejde dokončit.
    #[test]
    fn no_unfinishable_chain_on_last_floor() {
        let e = exp(WinnersPodium, 0, 1, 38, 10);
        assert_eq!(choose_encounter(&e, &[enc(SmallHurdle, -1), enc(Key, 0)]), 1);
        assert_eq!(choose_encounter(&e, &[enc(UnicornBounty, 0), enc(CampFire, 3)]), 1);
    }

    /// Příklad 3: kolo 9, hledaný kostlivec (+2 +10) vs. cílový uhasený oheň (0 +4).
    #[test]
    fn wanted_beats_weak_target() {
        let e = with(exp(BurntCampfire, 0, 1, 20, 9), &[DummyBounty, Phoenix]);
        assert_eq!(choose_encounter(&e, &[enc(BurntCampfire, 0), enc(Dummy1, 2)]), 1);
    }

    #[test]
    fn princess_early_not_late() {
        let early = exp(Dragon, 0, 1, 0, 2);
        assert_eq!(choose_encounter(&early, &[enc(Dummy1, 2), enc(Bait, -2)]), 1);
        let late = exp(Dragon, 0, 1, 0, 10);
        assert_eq!(choose_encounter(&late, &[enc(Dummy1, 2), enc(Bait, -2)]), 0);
    }

    #[test]
    fn keys_and_chests_after_secured() {
        let e = exp(Cake, 1, 1, 42, 5);
        assert_eq!(choose_encounter(&e, &[enc(Dragon, 10), enc(Key, 0)]), 1);
        let e = with(exp(Cake, 1, 1, 42, 6), &[Key]);
        assert_eq!(choose_encounter(&e, &[enc(Key, 0), enc(Suitcase, 0)]), 1);
        // poslední kolo: klíč už nemá smysl
        let e = exp(Cake, 1, 1, 42, 10);
        assert_eq!(choose_encounter(&e, &[enc(Key, 0), enc(Dummy1, 2)]), 1);
    }

    #[test]
    fn per_item_bonus_counts_to_40() {
        // Podium: 25 + 2 × 10 na konci = 45 → už je zajištěno, bereme klíč
        let e = exp(WinnersPodium, 2, 2, 25, 6);
        assert_eq!(projected_heroism(&e), 45);
        assert_eq!(choose_encounter(&e, &[enc(Dummy1, 2), enc(Key, 0)]), 1);
    }

    #[test]
    fn sanitary_fail_penalty() {
        // nesplněný papír: odhad 42 - 5 = 37 → ještě není zajištěno
        let e = exp(ToiletPaper, 2, 3, 42, 7);
        assert_eq!(projected_heroism(&e), 37);
        // třetí papír splní úkol: 0 + 20 + 5
        assert_eq!(choose_encounter(&e, &[enc(Key, 0), enc(ToiletPaper, 0)]), 1);
    }

    fn avail(target: ExpeditionThing, min: u32) -> AvailableExpedition {
        AvailableExpedition {
            target,
            thirst_for_adventure_sec: min * 60,
            location_1: Default::default(),
            location_2: Default::default(),
            special: None,
        }
    }

    #[test]
    fn expedition_shortest_then_easiest() {
        // kratší vyhrává i nad snazší
        assert_eq!(choose_expedition(&[avail(Cake, 20), avail(BurntCampfire, 15)], 6000), Some(1));
        // stejná délka: sele (8/kolo) je snazší než oheň (4/kolo)
        assert_eq!(choose_expedition(&[avail(BurntCampfire, 20), avail(Cake, 20)], 6000), Some(1));
        // stejná délka: nezmapovaná mise má přednost
        assert_eq!(choose_expedition(&[avail(Cake, 20), avail(Klaus, 20)], 6000), Some(1));
        // na delší nestačí ALU
        assert_eq!(choose_expedition(&[avail(Cake, 20)], 600), None);
    }

    #[test]
    fn reward_order() {
        let r = |typ| Reward { typ, amount: 1 };
        assert_eq!(choose_reward(&[r(RewardType::QuicksandGlass), r(RewardType::Silver), r(RewardType::Mushrooms)]), 2);
        assert_eq!(choose_reward(&[r(RewardType::QuicksandGlass), r(RewardType::Silver)]), 1);
        assert_eq!(choose_reward(&[r(RewardType::XP), r(RewardType::QuicksandGlass)]), 1);
    }
}
