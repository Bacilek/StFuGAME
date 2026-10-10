//! Goblin Gleeman (daily tasks) and event tasks: claim the reward chests and actively do the tasks that cost
//! only gold or nothing (user 2026-10-07). Never anything with mushrooms (beer, paid wheel spins, hourglasses, skips).
//! - chests: claimed as soon as there are enough points,
//! - Upgrade guild skill: the cheaper of Treasure/Instructor (Instructor on a tie), gold only,
//! - Increase attribute X / any attribute / spend gold on upgrades: buy attributes for gold (the best weight-per-gold
//!   one from `attributes.rs` when the task lets us choose freely), plus the same choice for genuinely surplus gold
//!   (`buy_surplus_attributes`, no task needed, above 5× the shop reserve),
//! - Win the shell game: smallest bet, only after the Tavern and the shops (gold is low then) and only when its points
//!   are needed to reach a chest that the remaining "natural" tasks (Arena, Dungeons, City Guard) would not reach.
//!
//! Shop purchases for tasks are in `shops.rs`, the Training Camp preference in `dungeons.rs`.

use std::time::{Duration, Instant};

use sf_api::{
    command::{AttributeType, Command, FortunePayment},
    gamestate::{
        GameState,
        guild::GuildSkill,
        rewards::{RewardChest, RewardType, Task, TaskType},
        tavern::{AvailableTasks, CurrentAction, Location},
    },
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

/// Safety limit on actions of one kind in one pass.
const MAX_ACTIONS: usize = 20;
/// Shell game: only with at least 5 gold, bet at most 1/10 of the gold (game rules, user 2026-10-07).
pub const GAMBLE_MIN_SILVER: u64 = 500;
/// Smallest possible bet (in silver). Verify on the server.
const GAMBLE_BET: u64 = 1;

/// Open (uncompleted) daily and event tasks.
pub fn open_tasks(gs: &GameState) -> impl Iterator<Item = &Task> {
    let t = &gs.specials.tasks;
    t.daily.tasks.iter().chain(t.event.tasks.iter()).filter(|t| !t.is_completed())
}

/// How many more of the given task are needed (the biggest remaining count among matching open tasks).
pub fn remaining(gs: &GameState, pred: impl Fn(TaskType) -> bool) -> u64 {
    open_tasks(gs).filter(|t| pred(t.typ)).map(|t| t.target - t.current).max().unwrap_or(0)
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[tasks] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

macro_rules! send_or_return {
    ($session:expr, $cmd:expr) => {
        if let Err(e) = safe::send($session, $cmd).await {
            return fail(&e);
        }
    };
}

/// Claims every chest (daily, event) that has enough points.
async fn claim_chests(session: &mut SimpleSession) -> Outcome {
    for _ in 0..6 {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let t = &gs.specials.tasks;
        let (cmd, what, points) = if let Some(pos) = (0..3).find(|&i| t.daily.can_open_chest(i)) {
            (Command::CollectDailyQuestReward { pos }, format!("daily chest {}", pos + 1), t.daily.earned_points())
        } else if let Some(pos) =
            (!t.event.tasks.is_empty()).then(|| (0..3).find(|&i| t.event.can_open_chest(i))).flatten()
        {
            (Command::CollectEventTaskReward { pos }, format!("event chest {}", pos + 1), t.event.earned_points())
        } else {
            return Outcome::Done;
        };
        report!("[tasks] Claiming the {what} ({points} points)");
        send_or_return!(session, cmd);
    }
    Outcome::Done
}

/// Guild skill to upgrade for a task: the cheaper of Treasure/Instructor, Instructor on a tie; gold only and affordable.
pub fn guild_skill_to_upgrade(gs: &GameState) -> Option<(GuildSkill, u16)> {
    let g = gs.guild.as_ref()?;
    [(GuildSkill::Instructor, g.own_instructor_skill), (GuildSkill::Treasure, g.own_treasure_skill)]
        .into_iter()
        .filter(|(s, _)| {
            g.upgrade_price[*s].mushrooms == 0
                && g.upgrade_price[*s].silver + crate::shops::reserve() <= gs.character.silver
        })
        .min_by_key(|(s, _)| g.upgrade_price[*s].silver)
}

async fn upgrade_guild(session: &mut SimpleSession) -> Outcome {
    for _ in 0..MAX_ACTIONS {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        if remaining(gs, |t| t == TaskType::UpgradeAnyGuildSkill) == 0 {
            return Outcome::Done;
        }
        let Some((skill, current)) = guild_skill_to_upgrade(gs) else { return Outcome::Done };
        let price = gs.guild.as_ref().map_or(0, |g| g.upgrade_price[skill].silver);
        report!("[tasks] Upgrading the guild {skill:?} {current} → {} for {}", current + 1, crate::report::gold(price));
        send_or_return!(session, Command::GuildIncreaseSkill { skill, current });
        send_or_return!(session, Command::Update);
    }
    Outcome::Done
}

/// Attribute to buy for an open task, if any. A task that lets us pick freely (`UpgradeAnyAttribute`/
/// `SpendGoldOnUpgrades`) picks whichever attribute is currently the best weight-per-gold (user 2026-10-09),
/// not always the main one.
fn attribute_for_task(gs: &GameState) -> Option<AttributeType> {
    open_tasks(gs).find_map(|t| match t.typ {
        TaskType::Upgrade(a) => Some(a),
        TaskType::UpgradeAnyAttribute | TaskType::SpendGoldOnUpgrades => {
            Some(crate::attributes::best_attribute(gs.character.class))
        }
        _ => None,
    })
}

pub async fn buy_attributes(session: &mut SimpleSession) -> Outcome {
    // The price of the next point is not known in advance: estimate it by the last one paid
    let mut last_price = 0;
    for _ in 0..MAX_ACTIONS {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let Some(attribute) = attribute_for_task(gs) else { return Outcome::Done };
        let reserve = crate::shops::reserve();
        if gs.character.silver <= reserve + last_price {
            report!(
                "[tasks] Not buying {attribute:?}: gold {} would drop below the shop reserve {}",
                crate::report::gold(gs.character.silver),
                crate::report::gold(reserve)
            );
            return Outcome::Done;
        }
        let current = gs.character.attribute_basis[attribute];
        let silver = gs.character.silver;
        send_or_return!(session, Command::UpgradeSkill { attribute, next_attribute: current + 1 });
        let Some(gs) = session.game_state() else { return Outcome::Done };
        if gs.character.attribute_basis[attribute] == current {
            report!("[tasks] The attribute did not increase (not enough gold?), stopping");
            return Outcome::Done;
        }
        last_price = silver.saturating_sub(gs.character.silver);
        crate::attributes::record_purchase(attribute, last_price as u32);
        report!("[tasks] Buying {attribute:?} {current} → {} for a task, paid {}", current + 1, crate::report::gold(last_price));
    }
    Outcome::Done
}

/// Gold above this multiple of the shop reserve is "a lot" (user 2026-10-09): rather than let it just pile up
/// (spinning the shop burns gold on the buy/sell spread), spend the surplus on whichever attribute is currently
/// the best weight-per-gold. No task needed - unlike `buy_attributes`, this never runs just because a task asks
/// for it, only when gold is genuinely abundant.
const SURPLUS_RESERVE_MULTIPLE: u64 = 5;

/// Spends gold on attributes once it piles up far above the shop reserve, even with no open attribute task.
/// Keeps at least `SURPLUS_RESERVE_MULTIPLE × reserve` in gold, same floor logic as `buy_attributes`.
pub async fn buy_surplus_attributes(session: &mut SimpleSession) -> Outcome {
    let mut last_price = 0;
    for _ in 0..MAX_ACTIONS {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let floor = SURPLUS_RESERVE_MULTIPLE * crate::shops::reserve();
        if gs.character.silver <= floor + last_price {
            return Outcome::Done;
        }
        let attribute = crate::attributes::best_attribute(gs.character.class);
        let current = gs.character.attribute_basis[attribute];
        let silver = gs.character.silver;
        send_or_return!(session, Command::UpgradeSkill { attribute, next_attribute: current + 1 });
        let Some(gs) = session.game_state() else { return Outcome::Done };
        if gs.character.attribute_basis[attribute] == current {
            report!("[tasks] Surplus attribute buy did not increase the attribute (not enough gold?), stopping");
            return Outcome::Done;
        }
        last_price = silver.saturating_sub(gs.character.silver);
        crate::attributes::record_purchase(attribute, last_price as u32);
        report!(
            "[tasks] Buying {attribute:?} {current} → {} from surplus gold (above {}× reserve), paid {}",
            current + 1,
            SURPLUS_RESERVE_MULTIPLE,
            crate::report::gold(last_price)
        );
    }
    Outcome::Done
}

fn is_attribute_task(t: TaskType) -> bool {
    matches!(t, TaskType::Upgrade(_) | TaskType::UpgradeAnyAttribute | TaskType::SpendGoldOnUpgrades)
}

/// Do the attribute tasks help to a better chest? (user 2026-10-07: then they go before spinning the shops)
/// Yes when some unopened chest is above earned + natural points and within reach with the attribute tasks
/// (plus the costly tasks we could afford).
fn attributes_help(tasks: &[Task], chests: &[RewardChest], means: Means) -> bool {
    let open = |pred: &dyn Fn(TaskType) -> bool| -> u32 {
        tasks.iter().filter(|t| !t.is_completed() && pred(t.typ)).map(|t| t.point_reward).sum()
    };
    let attr = open(&is_attribute_task);
    if attr == 0 {
        return false;
    }
    let earned: u32 = tasks.iter().filter(|t| t.is_completed()).map(|t| t.point_reward).sum();
    let expected = earned + natural_points(tasks);
    let extras = open(&|t| {
        (t == TaskType::DefeatGambler && means.gamble)
            || (t == TaskType::SpinWheelOfFortune && means.wheel_spins > 0)
            || (t == TaskType::DrinkBeer && means.beers > 0)
    });
    chests.iter().any(|c| !c.opened && c.required_points > expected && c.required_points <= expected + attr + extras)
}

/// Attribute tasks needed for a better chest (daily or event)?
pub fn attributes_needed(gs: &GameState) -> bool {
    let t = &gs.specials.tasks;
    let means = Means::of(gs);
    attributes_help(&t.daily.tasks, &t.daily.rewards, means) || attributes_help(&t.event.tasks, &t.event.rewards, means)
}

/// Points of open tasks that will most likely complete on their own today (Arena, City Guard – the bot retries
/// both every cooldown with no cap on attempts, so 10 Arena wins and the day's City Guard hours reliably land
/// eventually). **Not** dungeon tasks (`FightInDungeons`/`DefeatMonstersLightDungeon`) any more (found 2026-10-08,
/// live): those are capped at 1 real attempt/hour with a real win/loss outcome (and Training Camp's difficulty
/// climbs with each win), so "will probably finish today" is often wrong – assuming it anyway made `plan()`
/// think an unopened chest was already covered by "natural" progress and skip beer, even when the character was
/// stuck at e.g. 6/10 with no realistic chance of reaching 10/10 before midnight. Beer (and the other costly
/// tasks) now only ever gets credited for dungeon points once the task is *actually* done, not assumed.
fn natural_points(tasks: &[Task]) -> u32 {
    tasks
        .iter()
        .filter(|t| !t.is_completed())
        .filter(|t| matches!(t.typ, TaskType::WinFightsInArena | TaskType::CityGuardHours | TaskType::EarnMoneyCityGuard))
        .map(|t| t.point_reward)
        .sum()
}

/// Costly tasks: done only when they are needed for a chest (user 2026-10-07).
/// Shell game = a few silver, Wheel of Fortune beyond the free spin = lucky coins, beer = mushrooms.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Extra {
    Gamble,
    Wheel,
    Beer,
}

impl Extra {
    /// How much we dislike spending on it (per unit): gold < lucky coins < mushrooms.
    fn weight(self) -> u64 {
        match self {
            Extra::Gamble => 1,
            Extra::Wheel => 10,
            Extra::Beer => 100,
        }
    }

    fn task(self) -> TaskType {
        match self {
            Extra::Gamble => TaskType::DefeatGambler,
            Extra::Wheel => TaskType::SpinWheelOfFortune,
            Extra::Beer => TaskType::DrinkBeer,
        }
    }
}

/// What we can afford for the costly tasks right now.
#[derive(Clone, Copy, Debug, Default)]
pub struct Means {
    /// At least 5 gold for the shell game
    pub gamble: bool,
    /// Wheel spins we can pay with lucky coins (+ the free one if still available)
    pub wheel_spins: u64,
    /// Beers still allowed today
    pub beers: u64,
}

impl Means {
    pub fn of(gs: &GameState) -> Self {
        let w = &gs.specials.wheel;
        Means {
            gamble: gs.character.silver >= GAMBLE_MIN_SILVER,
            wheel_spins: u64::from(w.lucky_coins) / LUCKY_COINS_PER_SPIN + u64::from(safe::wheel_is_free(gs)),
            beers: u64::from(gs.tavern.beer_max.saturating_sub(gs.tavern.beer_drunk)),
        }
    }
}

/// One beer costs one mushroom.
pub const BEER_MUSHROOMS: u32 = 1;
/// One Wheel of Fortune spin costs 10 lucky coins (user 2026-10-07; an ad at Dr. Abawuwu gives 3).
const LUCKY_COINS_PER_SPIN: u64 = 10;

/// Amount of one reward type in a chest.
fn chest_amount(c: &RewardChest, typ: RewardType) -> u64 {
    c.rewards.iter().filter(|r| r.typ == typ).map(|r| r.amount).sum()
}

fn chest_mushrooms(c: &RewardChest) -> u64 {
    chest_amount(c, RewardType::Mushrooms)
}

/// Is the chest worth what the chosen costly tasks spend? (user 2026-10-07: look at the chest rewards)
/// Beers (mushrooms): at least as many mushrooms in the chest. Lucky coins: the chest has mushrooms, or at least as
/// many lucky coins back. The shell game costs a few silver: any chest.
fn chest_worth(c: &RewardChest, beers: u64, spins: u64) -> bool {
    let mushrooms = chest_mushrooms(c);
    (beers == 0 || mushrooms >= beers * u64::from(BEER_MUSHROOMS))
        && (spins == 0 || mushrooms > 0 || chest_amount(c, RewardType::LuckyCoins) >= spins * LUCKY_COINS_PER_SPIN)
}

/// Which costly tasks to do for this task list: the combination that reaches the highest chest that earned +
/// "natural" points (Arena, Dungeons, City Guard) would not reach, and the cheapest one for that chest.
/// Beers only for a chest with at least as many mushrooms as the beers cost (10 beers for 10 mushrooms + the Thirst
/// for Adventure is fine, otherwise not). Nothing when the chest is reachable without them.
pub fn plan(tasks: &[Task], chests: &[RewardChest], means: Means) -> Vec<Extra> {
    let earned: u32 = tasks.iter().filter(|t| t.is_completed()).map(|t| t.point_reward).sum();
    let expected = earned + natural_points(tasks);
    // (extra, points, units still needed)
    let options: Vec<(Extra, u32, u64)> = [Extra::Gamble, Extra::Wheel, Extra::Beer]
        .into_iter()
        .filter_map(|e| {
            let t = tasks.iter().find(|t| t.typ == e.task() && !t.is_completed())?;
            let left = t.target - t.current;
            let ok = match e {
                Extra::Gamble => means.gamble,
                Extra::Wheel => left <= means.wheel_spins,
                Extra::Beer => left <= means.beers,
            };
            ok.then_some((e, t.point_reward, left))
        })
        .collect();

    // Best = (highest chest, lowest cost)
    let mut best: Option<(u32, u64, Vec<Extra>)> = None;
    for mask in 1u32..(1 << options.len()) {
        let chosen: Vec<_> = options.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, o)| *o).collect();
        let points: u32 = chosen.iter().map(|o| o.1).sum();
        let cost: u64 = chosen.iter().map(|o| o.0.weight() * o.2).sum();
        let beers = chosen.iter().find(|o| o.0 == Extra::Beer).map_or(0, |o| o.2);
        let spins = chosen.iter().find(|o| o.0 == Extra::Wheel).map_or(0, |o| o.2);
        let reached = chests
            .iter()
            .filter(|c| !c.opened && c.required_points > expected && c.required_points <= expected + points)
            .filter(|c| chest_worth(c, beers, spins))
            .map(|c| c.required_points)
            .max();
        let Some(req) = reached else { continue };
        let better = best.as_ref().is_none_or(|(r, c, _)| req > *r || (req == *r && cost < *c));
        if better {
            best = Some((req, cost, chosen.iter().map(|o| o.0).collect()));
        }
    }
    best.map(|b| b.2).unwrap_or_default()
}

/// Costly tasks planned for the daily and the event list together.
pub fn planned(gs: &GameState) -> Vec<Extra> {
    let t = &gs.specials.tasks;
    let means = Means::of(gs);
    let mut v = plan(&t.daily.tasks, &t.daily.rewards, means);
    v.extend(plan(&t.event.tasks, &t.event.rewards, means));
    // Shell game only ever spends gold (never mushrooms/lucky coins), so – unlike Wheel/Beer – there's no reason
    // to wait for a chest that's "worth" it: finish it whenever it's open and affordable (user 2026-10-08:
    // "goldy jsou postradatelnější než houby").
    let gambling_open = gambling_is_open(&t.daily.tasks) || gambling_is_open(&t.event.tasks);
    if means.gamble && gambling_open && !v.contains(&Extra::Gamble) {
        v.push(Extra::Gamble);
    }
    v
}

fn gambling_is_open(tasks: &[Task]) -> bool {
    tasks.iter().any(|task| task.typ == TaskType::DefeatGambler && !task.is_completed())
}

/// Bonus Thirst for Adventure of one beer, as far as we assume (NOT verified live yet): the expedition we want the
/// beer for must cost at most this much.
const BEER_ALU_SEC: u32 = 20 * 60;

/// "Last chest" travel beer (user 2026-10-10, Mrožik missed a 125 g chest by one BustedLands visit with 0 Thirst for
/// Adventure): ONE beer (1 mushroom) is allowed when the highest unopened Gleeman chest (daily or event) lacks only
/// one `TravelTo(location)` visit, the Tavern is out of Thirst for Adventure, one of the expeditions on offer passes
/// that location (and costs at most one beer's worth), and nothing free (gold-only tasks, Arena, City Guard) would
/// reach the chest anyway. `offered` = (location 1, location 2, Thirst for Adventure cost) of each expedition.
pub fn travel_beer_needed(
    tasks: &[Task],
    chests: &[RewardChest],
    gamble_possible: bool,
    thirst_now: u32,
    offered: &[(Location, Location, u32)],
) -> bool {
    if offered.iter().any(|o| o.2 <= thirst_now) {
        return false; // can still start an expedition without a beer
    }
    let Some(last) = chests.iter().max_by_key(|c| c.required_points) else { return false };
    if last.opened {
        return false;
    }
    let earned: u32 = tasks.iter().filter(|t| t.is_completed()).map(|t| t.point_reward).sum();
    // Free ways to the chest: Arena/City Guard, the shell game (gold), attribute tasks (gold)
    let free: u32 = natural_points(tasks)
        + tasks
            .iter()
            .filter(|t| !t.is_completed())
            .filter(|t| (t.typ == TaskType::DefeatGambler && gamble_possible) || is_attribute_task(t.typ))
            .map(|t| t.point_reward)
            .sum::<u32>();
    if earned + free >= last.required_points {
        return false;
    }
    tasks.iter().filter(|t| !t.is_completed()).any(|t| {
        let TaskType::TravelTo(place) = t.typ else { return false };
        t.target - t.current == 1
            && earned + free + t.point_reward >= last.required_points
            && offered.iter().any(|o| (o.0 == place || o.1 == place) && o.2 <= BEER_ALU_SEC)
    })
}

static TRAVEL_BEER_DAY: crate::ctx::PerChar<Option<chrono::NaiveDate>> = crate::ctx::PerChar::new();

fn travel_beer_used_today() -> bool {
    TRAVEL_BEER_DAY.lock().is_ok_and(|d| *d == Some(chrono::Local::now().date_naive()))
}

/// `travel_beer_needed` for the daily and the event task list of the current game state (once per day).
fn travel_beer_justified(gs: &GameState) -> bool {
    if travel_beer_used_today() || gs.tavern.beer_drunk >= gs.tavern.beer_max || gs.tavern.current_action != CurrentAction::Idle {
        return false;
    }
    let AvailableTasks::Expeditions(list) = gs.tavern.available_tasks() else { return false };
    let offered: Vec<_> = list.iter().map(|e| (e.location_1, e.location_2, e.thirst_for_adventure_sec)).collect();
    let t = &gs.specials.tasks;
    let gamble = Means::of(gs).gamble;
    let thirst = gs.tavern.thirst_for_adventure_sec;
    travel_beer_needed(&t.daily.tasks, &t.daily.rewards, gamble, thirst, &offered)
        || travel_beer_needed(&t.event.tasks, &t.event.rewards, gamble, thirst, &offered)
}

/// May the bot drink a beer now? (Also checked by `safe.rs` before `BuyBeer`.)
pub fn beer_justified(gs: &GameState) -> bool {
    planned(gs).contains(&Extra::Beer) || travel_beer_justified(gs)
}

/// May the bot spin the Wheel of Fortune for lucky coins now? (Also checked by `safe.rs`.)
pub fn lucky_spin_justified(gs: &GameState) -> bool {
    planned(gs).contains(&Extra::Wheel)
}

async fn drink_beer(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    if !beer_justified(gs) {
        return Outcome::Done;
    }
    let left = remaining(gs, |t| t == TaskType::DrinkBeer);
    if !planned(gs).contains(&Extra::Beer) {
        report!("[tasks] Drinking ONE beer ({BEER_MUSHROOMS} mushroom): the last chest lacks one visit that an offered expedition provides");
        if let Ok(mut d) = TRAVEL_BEER_DAY.lock() {
            *d = Some(chrono::Local::now().date_naive());
        }
    } else {
        report!("[tasks] Drinking a beer ({BEER_MUSHROOMS} mushroom, {left} to go): needed for a chest with enough mushrooms");
    }
    send_or_return!(session, Command::BuyBeer);
    claim_chests(session).await
}

async fn lucky_spins(session: &mut SimpleSession) -> Outcome {
    let mut spins = 0;
    for _ in 0..MAX_ACTIONS {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        if !lucky_spin_justified(gs) || remaining(gs, |t| t == TaskType::SpinWheelOfFortune) == 0 {
            break;
        }
        // The free spin is done by daily.rs; here only lucky coins
        if safe::wheel_is_free(gs) {
            break;
        }
        send_or_return!(session, Command::SpinWheelOfFortune { payment: FortunePayment::LuckyCoins });
        spins += 1;
    }
    if spins > 0 {
        report!("[tasks] Wheel of Fortune: {spins} lucky-coin spin(s) for a chest");
    }
    claim_chests(session).await
}

async fn gamble(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    if !planned(gs).contains(&Extra::Gamble) {
        return Outcome::Done;
    }
    let start = gs.character.silver;
    let mut bets = 0;
    for _ in 0..MAX_ACTIONS * 2 {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let left = remaining(gs, |t| t == TaskType::DefeatGambler);
        if left == 0 {
            break;
        }
        if gs.character.silver < GAMBLE_MIN_SILVER {
            report!("[tasks] Shell game: less than 5 g, stopping");
            break;
        }
        send_or_return!(session, Command::GambleSilver { amount: GAMBLE_BET });
        bets += 1;
    }
    if let Some(gs) = session.game_state()
        && bets > 0
    {
        let diff = i128::from(gs.character.silver) - i128::from(start);
        report!(
            "[tasks] Shell game: {bets} bet(s), total {}{}",
            if diff < 0 { "-" } else { "+" },
            crate::report::gold(diff.unsigned_abs() as u64)
        );
    }
    claim_chests(session).await
}

/// How often `log_tasks` repeats the task/chest overview while anything is still unclaimed today (user
/// 2026-10-08: once a day was too stale to tell whether a chest's points are actually within reach – e.g.
/// whether the Mushroom Dealer welcome pack's task credit actually registered).
const TASK_LOG_EVERY: Duration = Duration::from_secs(15 * 60);

/// Logs the open tasks and chests (to see what the Gleeman wants and how close each chest is) – every
/// `TASK_LOG_EVERY` while anything today is still unclaimed, then stops until tomorrow's reset.
fn log_tasks(gs: &GameState) {
    let t = &gs.specials.tasks;
    // Nothing left to claim today: stop logging until the lists reset tomorrow, no point repeating a static view.
    if t.daily.rewards.iter().chain(&t.event.rewards).all(|c| c.opened) {
        return;
    }
    static LAST: crate::ctx::PerChar<Option<Instant>> = crate::ctx::PerChar::new();
    let Ok(mut last) = LAST.lock() else { return };
    if last.is_some_and(|i| i.elapsed() < TASK_LOG_EVERY) {
        return;
    }
    *last = Some(Instant::now());
    // Skip the event section entirely while the server hasn't sent a real event task list yet (e.g. right after
    // a new event theme starts but its tasks haven't synced) – the reward-chest array still exists but with
    // zeroed thresholds, which otherwise prints a misleading "0 points" line forever (see claim_chests).
    let mut sections = vec![("daily", &t.daily.tasks, &t.daily.rewards)];
    if !t.event.tasks.is_empty() {
        sections.push(("event", &t.event.tasks, &t.event.rewards));
    }
    for (name, tasks, chests) in sections {
        let list: Vec<String> =
            tasks.iter().map(|t| format!("{:?} {}/{} ({} p)", t.typ, t.current, t.target, t.point_reward)).collect();
        report!("[tasks] {name} tasks: {}", list.join(", "));
        for (i, c) in chests.iter().enumerate() {
            let rewards: Vec<String> = c
                .rewards
                .iter()
                .map(|r| crate::report::reward(&format!("{:?}", r.typ), i64::try_from(r.amount).unwrap_or(i64::MAX)))
                .collect();
            report!(
                "[tasks] {name} chest {} ({} points{}): {}",
                i + 1,
                c.required_points,
                if c.opened { ", opened" } else { "" },
                rewards.join(", ")
            );
        }
    }
}

/// Every pass of the main loop: chests. After the Tavern and the shops (the best equipment is bought first and
/// the shop reserve is kept): guild skill, attributes, then the remaining costly tasks per `plan` (lucky-coin
/// wheel spins, beer). Shell game (gold only) runs independently of all that, any time (see `planned`).
pub async fn run(session: &mut SimpleSession, tavern_done: bool) -> Outcome {
    if let Some(gs) = session.game_state() {
        log_tasks(gs);
    }
    if let Outcome::SessionLost = claim_chests(session).await {
        return Outcome::SessionLost;
    }
    if let Outcome::SessionLost = gamble(session).await {
        return Outcome::SessionLost;
    }
    if tavern_done {
        if let Outcome::SessionLost = upgrade_guild(session).await {
            return Outcome::SessionLost;
        }
        if let Outcome::SessionLost = buy_attributes(session).await {
            return Outcome::SessionLost;
        }
        if let Outcome::SessionLost = buy_surplus_attributes(session).await {
            return Outcome::SessionLost;
        }
        // Costly tasks, cheapest first; each is done only when the plan needs it for a chest
        if let Outcome::SessionLost = lucky_spins(session).await {
            return Outcome::SessionLost;
        }
        // Beer last (one per pass: it gives Thirst for Adventure, the Tavern continues before the next one)
        return drink_beer(session).await;
    }
    Outcome::Done
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(typ: TaskType, current: u64, target: u64, point_reward: u32) -> Task {
        Task { typ, current, target, point_reward }
    }

    fn chest(required_points: u32) -> RewardChest {
        RewardChest { opened: false, required_points, rewards: Vec::new() }
    }

    fn chest_with(required_points: u32, mushrooms: u64) -> RewardChest {
        RewardChest {
            opened: false,
            required_points,
            rewards: vec![sf_api::gamestate::rewards::Reward { typ: RewardType::Mushrooms, amount: mushrooms }],
        }
    }

    const RICH: Means = Means { gamble: true, wheel_spins: 10, beers: 10 };

    #[test]
    fn attributes_before_spinning_only_when_they_help() {
        let chests = [chest(4), chest(8), chest(12)];
        // earned 7 + STR 3 = 10 → reaches chest 8 → helps
        let tasks = vec![task(TaskType::LeaseMount, 1, 1, 7), task(TaskType::Upgrade(AttributeType::Strength), 0, 5, 3)];
        assert!(attributes_help(&tasks, &chests, RICH));
        // earned 12: everything reached → no
        let tasks = vec![task(TaskType::LeaseMount, 1, 1, 12), task(TaskType::Upgrade(AttributeType::Strength), 0, 5, 3)];
        assert!(!attributes_help(&tasks, &chests, RICH));
        // earned 5 + Arena 3 = 8: the next chest 12 is out of reach even with STR → no
        let tasks = vec![
            task(TaskType::LeaseMount, 1, 1, 5),
            task(TaskType::WinFightsInArena, 0, 10, 3),
            task(TaskType::Upgrade(AttributeType::Strength), 0, 5, 3),
        ];
        assert!(!attributes_help(&tasks, &chests, RICH));
    }

    #[test]
    fn nothing_when_natural_tasks_reach_the_chest() {
        let chests = [chest(4), chest(8), chest(12)];
        let tasks = vec![
            task(TaskType::LeaseMount, 1, 1, 10),
            task(TaskType::WinFightsInArena, 5, 10, 2),
            task(TaskType::DefeatGambler, 0, 3, 1),
            task(TaskType::DrinkBeer, 0, 1, 2),
        ];
        assert!(plan(&tasks, &chests, RICH).is_empty());
    }

    #[test]
    fn gamble_only_when_it_reaches_a_chest() {
        let chests = [chest(4), chest(8), chest(12)];
        // earned 6 + Arena 2 = 8; gambling (+1) does not reach 12 → no
        let tasks = vec![
            task(TaskType::LeaseMount, 1, 1, 6),
            task(TaskType::WinFightsInArena, 5, 10, 2),
            task(TaskType::DefeatGambler, 0, 3, 1),
        ];
        assert!(plan(&tasks, &chests, RICH).is_empty());
        // earned 9 + Arena 2 = 11; gambling reaches 12 → yes
        let tasks = vec![
            task(TaskType::LeaseMount, 1, 1, 9),
            task(TaskType::WinFightsInArena, 5, 10, 2),
            task(TaskType::DefeatGambler, 0, 3, 1),
        ];
        assert_eq!(plan(&tasks, &chests, RICH), [Extra::Gamble]);
        // not enough gold → no
        assert!(plan(&tasks, &chests, Means { gamble: false, ..RICH }).is_empty());
    }

    #[test]
    fn open_dungeon_task_does_not_block_a_chest_reachable_without_it() {
        // Bug found live 2026-10-08: an open (not yet completed) dungeon task used to count its full points as
        // "natural" (assumed to finish on its own), making `plan` think the chest was already covered and skip
        // beer – even stuck at 6/10 with the 1-attempt/hour cooldown making 10/10 unrealistic before reset.
        use sf_api::gamestate::dungeons::LightDungeon;
        let chests = [chest_with(20, 10)]; // needs mushrooms in the chest for beer to count as "worth it"
        let tasks = vec![
            task(TaskType::LeaseMount, 1, 1, 18),
            task(TaskType::DefeatMonstersLightDungeon(LightDungeon::TrainingCamp), 6, 10, 3),
            task(TaskType::DrinkBeer, 0, 1, 2),
        ];
        // earned 18; the open dungeon task must NOT count toward "expected" – only beer (+2) reaches 20
        assert_eq!(plan(&tasks, &chests, RICH), [Extra::Beer]);
    }

    #[test]
    fn gambling_open_even_when_no_chest_needs_it() {
        // `plan` alone says no chest needs gambling (natural points already clear everything) – but unlike
        // Wheel/Beer, `planned` adds Gamble anyway whenever the task is open and affordable (user 2026-10-08).
        let chests = [chest(4)];
        let tasks = vec![task(TaskType::LeaseMount, 1, 1, 10), task(TaskType::DefeatGambler, 0, 3, 1)];
        assert!(plan(&tasks, &chests, RICH).is_empty());
        assert!(gambling_is_open(&tasks));
        let completed = vec![task(TaskType::DefeatGambler, 3, 3, 1)];
        assert!(!gambling_is_open(&completed));
    }

    fn chest_coins(required_points: u32, coins: u64) -> RewardChest {
        RewardChest {
            opened: false,
            required_points,
            rewards: vec![sf_api::gamestate::rewards::Reward { typ: RewardType::LuckyCoins, amount: coins }],
        }
    }

    #[test]
    fn cheapest_combination_for_the_highest_chest() {
        let chests = [chest_with(4, 0), chest_with(8, 0), chest_with(12, 10)];
        // earned 10: gamble (+1) alone is not enough, wheel (+2) is → wheel only, no beer
        let tasks = vec![
            task(TaskType::LeaseMount, 1, 1, 10),
            task(TaskType::DefeatGambler, 0, 3, 1),
            task(TaskType::SpinWheelOfFortune, 1, 5, 2),
            task(TaskType::DrinkBeer, 0, 1, 2),
        ];
        assert_eq!(plan(&tasks, &chests, RICH), [Extra::Wheel]);
        // without lucky coins: gamble + beer would give 13 ≥ 12 but beer alone (+2) is enough and cheaper than both
        let poor = Means { wheel_spins: 0, ..RICH };
        assert_eq!(plan(&tasks, &chests, poor), [Extra::Beer]);
        // earned 9: needs 3 → gamble + wheel
        let tasks = vec![
            task(TaskType::LeaseMount, 1, 1, 9),
            task(TaskType::DefeatGambler, 0, 3, 1),
            task(TaskType::SpinWheelOfFortune, 1, 5, 2),
        ];
        assert_eq!(plan(&tasks, &chests, RICH), [Extra::Gamble, Extra::Wheel]);
    }

    #[test]
    fn travel_beer_only_for_the_last_chest_when_nothing_free_helps() {
        use Location::{BustedLands, SkullIsland};
        let tasks = vec![
            task(TaskType::WinFightsInArena, 10, 10, 2),
            task(TaskType::DefeatGambler, 3, 3, 4),
            task(TaskType::TravelTo(BustedLands), 1, 2, 4),
            task(TaskType::DrinkBeer, 0, 10, 1),
        ];
        let chests = [chest(5), chest(6)];
        let offered = [(BustedLands, SkullIsland, 900), (SkullIsland, SkullIsland, 600)];
        // earned 6 (== last chest) is already enough: no beer needed
        assert!(!travel_beer_needed(&tasks, &chests, false, 0, &offered));
        let chests = [chest(5), chest(10)];
        // 6 earned, the BustedLands visit (+4) reaches the last chest (10), offered expedition passes it
        assert!(travel_beer_needed(&tasks, &chests, false, 0, &offered));
        // still enough Thirst for Adventure: no beer
        assert!(!travel_beer_needed(&tasks, &chests, false, 600, &offered));
        // no offered expedition passes the place (or too long for one beer)
        assert!(!travel_beer_needed(&tasks, &chests, false, 0, &[(SkullIsland, SkullIsland, 900)]));
        assert!(!travel_beer_needed(&tasks, &chests, false, 0, &[(BustedLands, SkullIsland, 3000)]));
        // the last chest already opened
        let mut opened = chests;
        opened[1].opened = true;
        assert!(!travel_beer_needed(&tasks, &opened, false, 0, &offered));
        // a free gold-only task (shell game, 8 points) would reach the chest anyway
        let free = vec![
            task(TaskType::WinFightsInArena, 10, 10, 2),
            task(TaskType::DefeatGambler, 0, 3, 8),
            task(TaskType::TravelTo(BustedLands), 1, 2, 4),
        ];
        assert!(!travel_beer_needed(&free, &[chest(10)], true, 0, &offered));
        // two visits missing: one beer would not finish it
        let two = vec![task(TaskType::WinFightsInArena, 10, 10, 6), task(TaskType::TravelTo(BustedLands), 0, 2, 4)];
        assert!(!travel_beer_needed(&two, &[chest(10)], false, 0, &offered));
    }

    #[test]
    fn beers_only_for_enough_mushrooms() {
        // 10 beers for a chest with 10 mushrooms: fine (+ Thirst for Adventure)
        let chests = [chest_with(4, 0), chest_with(8, 0), chest_with(12, 10)];
        let tasks = vec![task(TaskType::LeaseMount, 1, 1, 10), task(TaskType::DrinkBeer, 0, 10, 2)];
        assert_eq!(plan(&tasks, &chests, RICH), [Extra::Beer]);
        // 10 beers for 5 mushrooms: no
        let chests = [chest_with(4, 0), chest_with(8, 0), chest_with(12, 5)];
        assert!(plan(&tasks, &chests, RICH).is_empty());
        // not enough beers left today: no
        let chests = [chest_with(4, 0), chest_with(8, 0), chest_with(12, 10)];
        assert!(plan(&tasks, &chests, Means { beers: 3, ..RICH }).is_empty());
        // a chest without mushrooms is never worth a beer
        let chests = [chest_with(4, 0), chest_with(8, 0), chest_with(12, 0)];
        let tasks = vec![task(TaskType::LeaseMount, 1, 1, 10), task(TaskType::DrinkBeer, 0, 1, 2)];
        assert!(plan(&tasks, &chests, RICH).is_empty());
    }

    #[test]
    fn wheel_only_with_enough_lucky_coins() {
        let chests = [chest(4), chest(8), chest_with(12, 10)];
        let tasks = vec![task(TaskType::LeaseMount, 1, 1, 10), task(TaskType::SpinWheelOfFortune, 1, 5, 2)];
        assert!(plan(&tasks, &chests, Means { wheel_spins: 3, ..RICH }).is_empty());
        assert_eq!(plan(&tasks, &chests, Means { wheel_spins: 4, ..RICH }), [Extra::Wheel]);
        // a chest with neither mushrooms nor enough lucky coins back is not worth 4 coins
        let chests = [chest(4), chest(8), chest_coins(12, 30)];
        assert!(plan(&tasks, &chests, RICH).is_empty());
        let chests = [chest(4), chest(8), chest_coins(12, 40)];
        assert_eq!(plan(&tasks, &chests, RICH), [Extra::Wheel]);
    }
}
