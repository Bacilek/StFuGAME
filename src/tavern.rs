//! Hospoda: expedice s výběry (rozcestí, boss, odměny, čekání).
//! Strategie: co nejdřív si zajistit 40 hrdinství (maximální odměna) a pak farmit klíče a truhly.

use std::time::Duration;

use chrono::Local;
use sf_api::{
    SimpleSession,
    command::Command,
    error::SFError,
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
/// Nejvíc hrdinství, které se dá realisticky získat za jedno kolo (plakát + kostlivec ~13,
/// stupně vítězů 15). Když ani s tím 40 nedosáhneme, nemá smysl body honit.
const MAX_GAIN_PER_FLOOR: i32 = 12;
/// Pojistka proti nekonečné smyčce.
const MAX_STEPS: u32 = 300;

/// Vybere expedici: nejkratší (nejlevnější v ALU). Při stejné délce ta, kde je nejsnazší
/// získat 40 hrdinství; neznámá mise (nebo s neověřeným bonusem) má při shodě přednost, abychom ji zmapovali.
fn choose_expedition(list: &[AvailableExpedition], thirst: u32) -> Option<usize> {
    // Neznámá mise nebo neověřený bonus = chceme zmapovat, při shodě má přednost
    let ease = |e: &AvailableExpedition| match missions::for_target(e.target) {
        Some(m) if m.bonus_known => m.ease(),
        _ => f64::INFINITY,
    };
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
    // Po výběru v posledním kole už server bonusy i tresty připsal (ověřeno 2026-10-07)
    if exp.current_floor >= LAST_FLOOR && !matches!(exp.current_stage(), ExpeditionStage::Encounters(_)) {
        return exp.heroism;
    }
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
        return unknown_chain_value(exp, t, floors);
    };
    let is_target = m.target() == exp.target_thing;
    // Předměty cizích cyklů: za ně na konci nic nedostaneme, počítá se jen okamžitý zisk
    if !is_target {
        return 0.0;
    }
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

/// Odhad pro krok nezmapované mise. Předměty jedné mise mají v sf-api čísla po sobě
/// ve stejné desítce (např. Mugs 151 → DraftBeer 152 → Barkeeper 153).
fn unknown_chain_value(exp: &Expedition, t: ExpeditionThing, floors: u8) -> f64 {
    let target = exp.target_thing;
    if missions::for_target(target).is_some() || has(exp, t) {
        return 0.0;
    }
    let (t_id, target_id) = (t as i64, target as i64);
    if t_id / 10 != target_id / 10 || t_id >= target_id {
        return 0.0;
    }
    let steps = u8::try_from(target_id - t_id).unwrap_or(u8::MAX);
    let payoff = f64::from(UNKNOWN_TARGET_GUESS * 2) - OPPORTUNITY_COST * f64::from(steps - 1);
    feasibility(steps, floors) * payoff.max(0.0)
}

/// O kolik hrdinství má vzrůst hned po výběru: body setkání + plakát + jednorázový bonus při splnění.
/// (Bonusy „za kus“ přijdou až na konci, sem nepatří.)
fn expected_now(exp: &Expedition, enc: &ExpeditionEncounter) -> i32 {
    let mut g = enc.heroism;
    if let Some(poster) = enc.typ.required_bounty()
        && has(exp, poster)
    {
        g += BOUNTY_BONUS;
    }
    if enc.typ == exp.target_thing
        && let Some(m) = missions::for_target(exp.target_thing)
        && let Bonus::OnComplete(b) = m.bonus
        && !target_done(exp)
        && exp.target_current + 1 >= exp.target_amount
    {
        g += b;
    }
    g
}

/// O kolik se má hrdinství změnit na konci (hned po výběru v 10. kole): bonusy „za kus“,
/// nebo trest za nesplněný úkol.
fn expected_end_change(exp: &Expedition) -> i32 {
    let Some(m) = missions::for_target(exp.target_thing) else { return 0 };
    let mut d = 0;
    if let Bonus::PerItem(b) = m.bonus {
        d += b * i32::from(exp.target_current);
    }
    if !target_done(exp) {
        d -= m.fail_penalty;
    }
    d
}

/// Popis možnosti pro výpis: body ze serveru, celkový zisk (je-li jiný) a budoucí hodnota (je-li nějaká).
fn describe_option(exp: &Expedition, e: &ExpeditionEncounter) -> String {
    let gain = immediate_gain(exp, e);
    let future = future_value(exp, e);
    let mut s = format!("{:?}({:+}", e.typ, e.heroism);
    if gain != e.heroism {
        s += &format!(" ⇒ {gain:+}");
    }
    if future > 0.0 {
        s += &format!(", budoucí {future:.1}");
    }
    s + ")"
}

/// Zapíše výsledek ověření dat misí. Nesoulad se hned vypíše.
fn check(entry: &mut crate::journal::Entry, ok: bool, what: &str) {
    if ok {
        entry.checks.push(format!("OK {what}"));
    } else {
        report!("[kontrola] NESEDÍ: {what}");
        entry.checks.push(format!("NESEDÍ {what}"));
    }
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

    // 40 už nestihneme: body nepomůžou, ber aspoň truhlu/klíč
    let floors_incl_this = i32::from(floors_after(exp)) + 1;
    if projected + floors_incl_this * MAX_GAIN_PER_FLOOR < MAX_HEROISM {
        if has(exp, ExpeditionThing::Key)
            && let Some(i) = encs.iter().position(|e| e.typ == ExpeditionThing::Suitcase)
        {
            return i;
        }
        if floors_after(exp) > 0
            && let Some(i) = encs.iter().position(|e| e.typ == ExpeditionThing::Key)
        {
            return i;
        }
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

/// Expedice za nejvýš tolik ALU je „zbytková“: zlata dá málo, přesýpacích hodin stejně.
const REDUCED_EXPEDITION_SEC: u32 = 3 * 60;

/// Vybere odměnu: houby > zlato > přesýpací hodiny > cokoli.
/// U zbytkové expedice (za zbytek ALU) houby > přesýpací hodiny > zlato.
fn choose_reward(rewards: &[Reward], reduced: bool) -> usize {
    let rank = |t: &RewardType| match t {
        RewardType::Mushrooms => 0,
        RewardType::Silver if reduced => 2,
        RewardType::Silver => 1,
        RewardType::QuicksandGlass if reduced => 1,
        RewardType::QuicksandGlass => 2,
        _ => 3,
    };
    rewards
        .iter()
        .enumerate()
        .min_by_key(|(i, r)| (rank(&r.typ), *i))
        .map_or(0, |(i, _)| i)
}

/// Je expedice po čekání (doprava po bossovi) a čeká na „pokračovat“?
/// sf-api v tu chvíli ukazuje starou nabídku rozcestí; novou server pošle až po pokračování.
/// Pole `floor_stage` (4 = čekání) není veřejné, čteme ho přes serde.
fn is_after_wait(exp: &Expedition) -> bool {
    serde_json::to_value(exp)
        .ok()
        .and_then(|v| v.get("floor_stage")?.as_i64())
        == Some(4)
}

/// Jak skončil běh hospody.
pub enum Outcome {
    /// Hotovo nebo chyba, kterou opakování nevyřeší.
    Done,
    /// Server zneplatnil session, je potřeba se znovu přihlásit přes účet.
    SessionLost,
}

/// Server zneplatnil session (sf-api se při dalším příkazu přihlásí znovu).
pub fn is_session_error(e: &SFError) -> bool {
    matches!(e, SFError::ServerError(msg) if msg.contains("sessionid"))
}

fn mission_name(t: ExpeditionThing) -> String {
    match missions::for_target(t) {
        None => format!("{t:?} (neznámá mise)"),
        Some(m) if !m.bonus_known => format!("{} (bonus neověřený)", m.name),
        Some(m) => m.name.to_string(),
    }
}

/// Odehraje expedice, dokud je ALU. Čekání vždy vyčká, nikdy nepřeskakuje.
pub async fn run(session: &mut SimpleSession, journal: &mut Journal) -> Outcome {
    let mut unknown_in_row = 0;
    // sf-api obnoví stav expedice, jen když ho server pošle. Po každém herním příkazu proto
    // stáhneme čerstvý stav a nikdy nevybíráme dvakrát ze stejné (zastaralé) nabídky.
    let mut refresh_pending = true;
    let mut last_offer: Option<Vec<String>> = None;
    let mut stale_tries = 0;
    // Po dokončení expedice vrátíme řízení (mezi expedicemi se dá stihnout aréna)
    let mut played = false;
    // Aréna během expedice: zkoušet nejvýš jednou za minutu (kdyby boj z nějakého důvodu neproběhl)
    let mut last_arena_try: Option<std::time::Instant> = None;
    let mut last_dungeon_try: Option<std::time::Instant> = None;
    // Expedice spuštěná za zbytek ALU (po restartu bota neznámé, pak se bere jako plná)
    let mut reduced_expedition = false;
    // Ověřování dat misí za běhu: (očekávané hrdinství, popis)
    let mut pending_check: Option<(i32, String)> = None;
    // Nesoulady s tabulkou hlásit jen jednou za běh
    let mut reported = std::collections::HashSet::new();
    // Před posledním bossem: (hrdinství, očekávaná změna na konci, popis)
    let mut end_check: Option<(i32, i32, String)> = None;

    for _ in 0..MAX_STEPS {
        let Some(gs) = session.game_state() else {
            report!("[hospoda] Chybí stav hry, končím");
            return Outcome::Done;
        };
        // Nové předměty (z truhel, boje): nasadit lepší, prodat horší. Bez akce nic neposílá.
        if !refresh_pending {
            let before = gs.character.inventory.count_free_slots();
            if let Outcome::SessionLost = crate::inventory::manage(session).await {
                return Outcome::SessionLost;
            }
            let Some(gs_now) = session.game_state() else { continue };
            if gs_now.character.inventory.count_free_slots() != before {
                refresh_pending = true;
                continue;
            }
        }
        let Some(gs) = session.game_state() else { continue };
        if crate::arena::ready(gs) && last_arena_try.is_none_or(|t| t.elapsed().as_secs() >= 60) {
            last_arena_try = Some(std::time::Instant::now());
            if let Outcome::SessionLost = crate::arena::run(session).await {
                return Outcome::SessionLost;
            }
            refresh_pending = true;
            continue;
        }
        if crate::dungeons::secs_until_ready(gs) == 0
            && last_dungeon_try.is_none_or(|t| t.elapsed().as_secs() >= crate::dungeons::RETRY_SEC)
        {
            last_dungeon_try = Some(std::time::Instant::now());
            if let Outcome::SessionLost = crate::dungeons::run(session).await {
                return Outcome::SessionLost;
            }
            refresh_pending = true;
            continue;
        }
        let tavern = &gs.tavern;

        let cmd = if let Some(exp) = tavern.expeditions.active() {
            played = true;
            let entry = journal.entry(&mission_name(exp.target_thing));
            entry.heroism = exp.heroism;
            entry.projected = projected_heroism(exp);
            entry.target_current = exp.target_current;
            entry.target_amount = exp.target_amount;

            let stage = exp.current_stage();
            // Po každém herním příkazu nejdřív čerstvý stav, ať nerozhodujeme podle starých dat
            // (např. „pokračovat“ u bosse je pro server výběr odměny č. 1)
            if refresh_pending && !matches!(stage, ExpeditionStage::Waiting { .. }) {
                refresh_pending = false;
                if let Err(e) = safe::send(session, Command::Update).await {
                    report!("[hospoda] Chyba: {e}");
                    return if is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
                }
                continue;
            }
            if let Some((expected, what)) = pending_check.take() {
                check(entry, expected == exp.heroism, &format!("{what}: čekal jsem {expected}, server má {}", exp.heroism));
            }
            match stage {
                ExpeditionStage::Encounters(_) if is_after_wait(exp) => {
                    unknown_in_row = 0;
                    last_offer = None;
                    stale_tries = 0;
                    report!("[hospoda] Čekání skončilo, pokračuji do další části expedice");
                    Command::ExpeditionContinue
                }
                ExpeditionStage::Encounters(encs) if !encs.is_empty() => {
                    unknown_in_row = 0;
                    let offer: Vec<String> = encs.iter().map(|e| format!("{:?}{}", e.typ, e.heroism)).collect();
                    if last_offer.as_ref() == Some(&offer) && stale_tries < 2 {
                        stale_tries += 1;
                        report!("[hospoda] Nabídka je stejná jako minule, obnovuji stav ({stale_tries}. pokus)");
                        if let Err(e) = safe::send(session, Command::Update).await {
                            report!("[hospoda] Chyba: {e}");
                            return if is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
                        }
                        continue;
                    }
                    if stale_tries >= 2 {
                        report!("[hospoda] Nabídka zůstala stejná i po obnovení, beru ji jako skutečnou");
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
                    for e in &encs {
                        if let Some((m, idx)) = missions::chain_position(e.typ) {
                            let table = m.chain[idx].1;
                            if table != e.heroism && reported.insert(e.typ as i64) {
                                check(entry, false, &format!("{:?} má v tabulce {table:+}, server ukazuje {:+}", e.typ, e.heroism));
                            }
                        }
                    }
                    let opts: Vec<String> = encs.iter().map(|e| describe_option(exp, e)).collect();
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
                    let is_resource = |t: ExpeditionThing| matches!(t, ExpeditionThing::Key | ExpeditionThing::Suitcase);
                    if projected_heroism(exp) >= MAX_HEROISM && encs.iter().any(|e| is_resource(e.typ)) && !is_resource(picked) {
                        entry.declined_resources += 1;
                    }
                    match picked {
                        ExpeditionThing::Key => entry.keys += 1,
                        ExpeditionThing::Suitcase => entry.chests += 1,
                        _ => {}
                    }
                    last_offer = Some(offer);
                    let end_change = if exp.current_floor >= LAST_FLOOR {
                        let mut after = exp.clone();
                        if picked == exp.target_thing {
                            after.target_current += 1;
                        }
                        expected_end_change(&after)
                    } else {
                        0
                    };
                    pending_check = Some((
                        exp.heroism + expected_now(exp, &encs[pos]) + end_change,
                        format!("hrdinství po výběru {picked:?} v kole {}", exp.current_floor),
                    ));
                    Command::ExpeditionPickEncounter { pos }
                }
                ExpeditionStage::Boss(_) => {
                    unknown_in_row = 0;
                    if exp.current_floor >= LAST_FLOOR {
                        // Bonusy za kus/trest už server připsal po výběru v 10. kole, po bossovi se nic měnit nemá
                        end_check = Some((
                            exp.heroism,
                            0,
                            format!("změna hrdinství po posledním bossovi ({})", mission_name(exp.target_thing)),
                        ));
                    }
                    report!("[hospoda] Boss, bojuji");
                    Command::ExpeditionContinue
                }
                ExpeditionStage::Rewards(rewards) if !rewards.is_empty() => {
                    unknown_in_row = 0;
                    let pos = choose_reward(&rewards, reduced_expedition);
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
                    // Když se mezitím uvolní aréna, probudit se dřív a zabojovat
                    let mut sleep = secs + extra;
                    if let Some(arena) = crate::arena::secs_until_ready(gs)
                        && arena + 5 < sleep
                    {
                        sleep = arena + fastrand::u64(5..20);
                        report!("[hospoda] Během čekání se uvolní aréna, vzbudím se za {sleep} s");
                    }
                    let dungeon = crate::dungeons::secs_until_ready(gs);
                    if dungeon + 5 < sleep {
                        sleep = dungeon + fastrand::u64(5..20);
                        report!("[hospoda] Během čekání se uvolní podzemí, vzbudím se za {sleep} s");
                    }
                    tokio::time::sleep(Duration::from_secs(sleep)).await;
                    Command::Update
                }
                _ => {
                    unknown_in_row += 1;
                    if unknown_in_row > 2 {
                        report!("[hospoda] Neznámý stav expedice, končím");
                        return Outcome::Done;
                    }
                    Command::Update
                }
            }
        } else {
            if let Some((before, delta, what)) = end_check.take() {
                let raw = serde_json::to_value(&tavern.expeditions).ok();
                let after = raw.as_ref().and_then(|v| v.get("active")?.get("heroism")?.as_i64());
                if let (Some(after), Some(entry)) = (after, journal.current.as_mut()) {
                    let after = i32::try_from(after).unwrap_or(i32::MIN);
                    check(entry, after - before == delta, &format!("{what}: čekal jsem {delta:+}, server {:+}", after - before));
                    entry.heroism = after;
                    entry.projected = after;
                }
            }
            journal.finish();
            match tavern.current_action {
                // Poslední časovač doběhl, expedici je potřeba uzavřít
                CurrentAction::Expedition => {
                    unknown_in_row += 1;
                    if unknown_in_row > 2 {
                        report!("[hospoda] Expedici se nedaří uzavřít, končím");
                        return Outcome::Done;
                    }
                    // Diagnostika: co server o expedici ví (hlavně jestli nečeká výběr odměny)
                    let raw = serde_json::to_value(&tavern.expeditions).ok();
                    let active = raw.as_ref().and_then(|v| v.get("active"));
                    let field = |k: &str| active.and_then(|a| a.get(k)).map(ToString::to_string).unwrap_or_default();
                    report!(
                        "[hospoda] Stav před uzavřením: floor_stage {}, kolo {}, odměny {}",
                        field("floor_stage"),
                        field("current_floor"),
                        field("rewards")
                    );
                    // „Pokračovat“ je pro server výběr odměny č. 1. Když sf-api ukazuje odměny (třeba
                    // zastaralé po 1. bossovi), vybereme podle priorit – horší to být nemůže.
                    let rewards: Vec<Reward> = active
                        .and_then(|a| a.get("rewards").cloned())
                        .and_then(|r| serde_json::from_value(r).ok())
                        .unwrap_or_default();
                    report!("[hospoda] Uzavírám dokončenou expedici");
                    if rewards.is_empty() {
                        Command::ExpeditionContinue
                    } else {
                        Command::ExpeditionPickReward { pos: choose_reward(&rewards, reduced_expedition) }
                    }
                }
                CurrentAction::Idle if played => return Outcome::Done,
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
                            return Outcome::Done;
                        };
                        let e = &list[pos];
                        report!(
                            "[hospoda] Startuji expedici: {}, {} min ALU, speciál {:?}",
                            mission_name(e.target),
                            e.thirst_for_adventure_sec / 60,
                            e.special
                        );
                        unknown_in_row = 0;
                        played = true;
                        reduced_expedition = e.thirst_for_adventure_sec <= REDUCED_EXPEDITION_SEC;
                        if reduced_expedition {
                            report!("[hospoda] Zbytková expedice: u odměn dávám přednost přesýpacím hodinám před zlatem");
                        }
                        Command::ExpeditionStart { pos }
                    }
                    AvailableTasks::Quests(_) => {
                        report!("[hospoda] Expedice nejsou dostupné (jen klasické questy), zatím nepodporuji");
                        return Outcome::Done;
                    }
                },
                other => {
                    report!("[hospoda] Postava je zaneprázdněná ({other:?}), hospodu přeskakuji");
                    return Outcome::Done;
                }
            }
        };

        refresh_pending = !matches!(cmd, Command::Update);
        if let Err(e) = safe::send(session, cmd).await {
            report!("[hospoda] Chyba: {e}");
            return if is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
        }
    }
    report!("[hospoda] Dosažen limit kroků, končím");
    Outcome::Done
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
        assert_eq!(choose_expedition(&[avail(Cake, 20), avail(Barkeeper, 20)], 6000), Some(1));
        // stejná délka: mise s neověřeným bonusem má také přednost
        assert_eq!(choose_expedition(&[avail(Cake, 20), avail(Klaus, 20)], 6000), Some(1));
        // na delší nestačí ALU
        assert_eq!(choose_expedition(&[avail(Cake, 20)], 600), None);
    }

    #[test]
    fn unknown_mission_chain_progresses() {
        let e = exp(Barkeeper, 0, 1, 0, 3);
        assert!(future_value(&e, &enc(Mugs, 0)) > 0.0);
        assert_eq!(choose_encounter(&e, &[enc(Dummy2, 2), enc(Mugs, 0)]), 1);
        // jiná skupina nic nedostane
        assert_eq!(future_value(&e, &enc(FishingRod, 0)), 0.0);
    }

    /// Čísla ověřená v běhu 2026-10-07.
    #[test]
    fn expectations_match_observed_runs() {
        // jednorožec: 8 → 25 (7 + bonus 10 hned při splnění)
        let e = with(exp(Unicorn, 0, 1, 8, 6), &[UnicornHorn, Donkey, Rainbow]);
        assert_eq!(e.heroism + expected_now(&e, &enc(Unicorn, 7)), 25);
        // kostlivec s plakátem: 25 → 37
        let e = with(exp(Unicorn, 1, 1, 25, 7), &[DummyBounty]);
        assert_eq!(e.heroism + expected_now(&e, &enc(Dummy2, 2)), 37);
        // draci: na konci +5 × 2
        assert_eq!(expected_end_change(&exp(Dragon, 2, 2, 36, 10)), 10);
        // nesplněný papír: −5 na konci, splnění +20 hned
        assert_eq!(expected_end_change(&exp(ToiletPaper, 2, 3, 30, 10)), -5);
        assert_eq!(expected_now(&exp(ToiletPaper, 2, 3, 30, 8), &enc(ToiletPaper, 0)), 20);
    }

    /// Barkeeper 2026-10-07, kolo 9: hrdinství 13, máme klíč, nabídka truhla / kostlivec +3.
    #[test]
    fn chest_when_40_unreachable() {
        let e = with(exp(Barkeeper, 0, 1, 13, 9), &[Key]);
        assert_eq!(choose_encounter(&e, &[enc(Dummy3, 3), enc(Suitcase, 0), enc(Dummy1, 1)]), 1);
        // na začátku expedice ještě body honíme
        let e = with(exp(Barkeeper, 0, 1, 13, 4), &[Key]);
        assert_eq!(choose_encounter(&e, &[enc(Dummy3, 3), enc(Suitcase, 0)]), 0);
    }

    /// Předměty cizích cyklů jen podle okamžitého zisku (pravidlo uživatele 2026-10-07).
    #[test]
    fn foreign_quest_items_only_for_instant_gain() {
        // mise drak: táborák (+3, cizí cyklus) je víc než slabý kostlivec (+1) → bereme
        let e = exp(Dragon, 0, 1, 0, 3);
        assert_eq!(choose_encounter(&e, &[enc(Dummy1, 1), enc(CampFire, 3)]), 1);
        // malá překážka (−1) kvůli cizím stupňům vítězů ne, radši klíč (0)
        let e = exp(Unicorn, 0, 1, 1, 4);
        assert_eq!(choose_encounter(&e, &[enc(SmallHurdle, -1), enc(Key, 0)]), 1);
        // čarodějnice (−5) v cizí misi ne, kostlivec (+2)
        let e = exp(BrokenSword, 2, 97, 3, 6);
        assert_eq!(choose_encounter(&e, &[enc(Dummy2, 2), enc(Girl, -5)]), 0);
    }

    #[test]
    fn detects_after_wait() {
        let e = Expedition::default();
        assert!(!is_after_wait(&e));
        let mut v = serde_json::to_value(&e).unwrap();
        v["floor_stage"] = serde_json::json!(4);
        let e: Expedition = serde_json::from_value(v).unwrap();
        assert!(is_after_wait(&e));
    }

    #[test]
    fn reward_order() {
        let r = |typ| Reward { typ, amount: 1 };
        assert_eq!(choose_reward(&[r(RewardType::QuicksandGlass), r(RewardType::Silver), r(RewardType::Mushrooms)], false), 2);
        assert_eq!(choose_reward(&[r(RewardType::QuicksandGlass), r(RewardType::Silver)], false), 1);
        assert_eq!(choose_reward(&[r(RewardType::XP), r(RewardType::QuicksandGlass)], false), 1);
        // zbytková expedice: hodiny před zlatem, houby pořád první
        assert_eq!(choose_reward(&[r(RewardType::Silver), r(RewardType::QuicksandGlass)], true), 1);
        assert_eq!(choose_reward(&[r(RewardType::QuicksandGlass), r(RewardType::Mushrooms)], true), 1);
    }
}
