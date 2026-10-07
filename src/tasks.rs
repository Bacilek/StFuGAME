//! Goblin Gleeman (daily tasks) and event tasks: claim the reward chests and actively do the tasks that cost
//! only gold or nothing (user 2026-10-07). Never anything with mushrooms (beer, paid wheel spins, hourglasses, skips).
//! - chests: claimed as soon as there are enough points,
//! - Upgrade guild skill: the cheaper of Treasure/Instructor (Instructor on a tie), gold only,
//! - Increase attribute X / any attribute / spend gold on upgrades: buy attributes for gold (main attribute when any),
//! - Win the shell game: smallest bet, only after the Tavern and the shops (gold is low then) and only when its points
//!   are needed to reach a chest that the remaining "natural" tasks (Arena, Dungeons, City Guard) would not reach.
//!
//! Shop purchases for tasks are in `shops.rs`, the Training Camp preference in `dungeons.rs`.

use sf_api::{
    command::{AttributeType, Command},
    gamestate::{
        GameState,
        guild::GuildSkill,
        rewards::{RewardChest, RewardType, Task, TaskType},
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
        } else if let Some(pos) = (0..3).find(|&i| t.event.can_open_chest(i)) {
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

/// Attribute to buy for an open task, if any.
fn attribute_for_task(gs: &GameState) -> Option<AttributeType> {
    open_tasks(gs).find_map(|t| match t.typ {
        TaskType::Upgrade(a) => Some(a),
        TaskType::UpgradeAnyAttribute | TaskType::SpendGoldOnUpgrades => Some(gs.character.class.main_attribute()),
        _ => None,
    })
}

async fn buy_attributes(session: &mut SimpleSession) -> Outcome {
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
        report!("[tasks] Buying {attribute:?} {current} → {} for a task", current + 1);
        send_or_return!(session, Command::UpgradeSkill { attribute, next_attribute: current + 1 });
        let Some(gs) = session.game_state() else { return Outcome::Done };
        if gs.character.attribute_basis[attribute] == current {
            report!("[tasks] The attribute did not increase (not enough gold?), stopping");
            return Outcome::Done;
        }
        last_price = silver.saturating_sub(gs.character.silver);
        report!("[tasks] Paid {}", crate::report::gold(last_price));
    }
    Outcome::Done
}

/// Points of open tasks that will most likely complete on their own today (Arena, Dungeons, City Guard).
fn natural_points(tasks: &[Task]) -> u32 {
    tasks
        .iter()
        .filter(|t| !t.is_completed())
        .filter(|t| {
            matches!(
                t.typ,
                TaskType::WinFightsInArena
                    | TaskType::FightInDungeons
                    | TaskType::DefeatMonstersLightDungeon(_)
                    | TaskType::CityGuardHours
                    | TaskType::EarnMoneyCityGuard
            )
        })
        .map(|t| t.point_reward)
        .sum()
}

/// Is the shell game worth it for this task list? Only when its points reach a chest that would not be reached
/// otherwise (earned + natural tasks).
pub fn gamble_needed(tasks: &[Task], chests: &[RewardChest]) -> bool {
    let Some(gamble) = tasks.iter().find(|t| t.typ == TaskType::DefeatGambler && !t.is_completed()) else {
        return false;
    };
    let earned: u32 = tasks.iter().filter(|t| t.is_completed()).map(|t| t.point_reward).sum();
    let expected = earned + natural_points(tasks);
    chests
        .iter()
        .any(|c| !c.opened && c.required_points > expected && c.required_points <= expected + gamble.point_reward)
}

/// One beer costs one mushroom.
pub const BEER_MUSHROOMS: u32 = 1;

/// Mushrooms in a chest's rewards.
fn chest_mushrooms(c: &RewardChest) -> u64 {
    c.rewards.iter().filter(|r| r.typ == RewardType::Mushrooms).map(|r| r.amount).sum()
}

/// Beer for a task (exception to the mushroom rule, user 2026-10-07): only when drinking is the last missing step
/// to a chest with more mushrooms than the beer costs, i.e. earned + "natural" points (Arena, Dungeons, City Guard)
/// do not reach that chest and the beer's points do. Called only after everything else for the day was done.
fn beer_opens_chest(tasks: &[Task], chests: &[RewardChest]) -> bool {
    let Some(beer) = tasks.iter().find(|t| t.typ == TaskType::DrinkBeer && !t.is_completed()) else {
        return false;
    };
    let earned: u32 = tasks.iter().filter(|t| t.is_completed()).map(|t| t.point_reward).sum();
    let expected = earned + natural_points(tasks);
    chests.iter().any(|c| {
        !c.opened
            && c.required_points > expected
            && c.required_points <= expected + beer.point_reward
            && chest_mushrooms(c) > u64::from(BEER_MUSHROOMS)
    })
}

/// May the bot drink a beer now? (Also checked by `safe.rs` before `BuyBeer`.)
pub fn beer_justified(gs: &GameState) -> bool {
    let t = &gs.specials.tasks;
    gs.tavern.beer_drunk < gs.tavern.beer_max
        && (beer_opens_chest(&t.daily.tasks, &t.daily.rewards) || beer_opens_chest(&t.event.tasks, &t.event.rewards))
}

async fn drink_beer(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    if !beer_justified(gs) {
        return Outcome::Done;
    }
    report!(
        "[tasks] Drinking a beer ({BEER_MUSHROOMS} mushroom): it is the last missing task for a chest with mushrooms"
    );
    send_or_return!(session, Command::BuyBeer);
    claim_chests(session).await
}

async fn gamble(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let t = &gs.specials.tasks;
    if !gamble_needed(&t.daily.tasks, &t.daily.rewards) && !gamble_needed(&t.event.tasks, &t.event.rewards) {
        return Outcome::Done;
    }
    let start = gs.character.silver;
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
        let res = session.game_state().and_then(|gs| gs.tavern.gamble_result);
        report!("[tasks] Shell game ({left} wins to go), bet {}: {res:?}", crate::report::gold(GAMBLE_BET));
    }
    if let Some(gs) = session.game_state() {
        let diff = i128::from(gs.character.silver) - i128::from(start);
        report!("[tasks] Shell game total: {}{}", if diff < 0 { "-" } else { "+" }, crate::report::gold(diff.unsigned_abs() as u64));
    }
    Outcome::Done
}

/// Logs the open tasks once a day (to see what the Gleeman wants).
fn log_tasks(gs: &GameState) {
    use std::sync::Mutex;
    static LOGGED: Mutex<Option<chrono::NaiveDate>> = Mutex::new(None);
    let today = chrono::Local::now().date_naive();
    let Ok(mut last) = LOGGED.lock() else { return };
    if *last == Some(today) {
        return;
    }
    *last = Some(today);
    let t = &gs.specials.tasks;
    for (name, tasks, chests) in [("daily", &t.daily.tasks, &t.daily.rewards), ("event", &t.event.tasks, &t.event.rewards)] {
        let list: Vec<String> =
            tasks.iter().map(|t| format!("{:?} {}/{} ({} p)", t.typ, t.current, t.target, t.point_reward)).collect();
        let need: Vec<String> = chests.iter().map(|c| c.required_points.to_string()).collect();
        report!("[tasks] {name} tasks (chests at {} points): {}", need.join("/"), list.join(", "));
    }
}

/// Every pass of the main loop: chests. After the Tavern and the shops (the best equipment is bought first and
/// the shop reserve is kept): guild skill, attributes, shell game, beer as the last resort.
pub async fn run(session: &mut SimpleSession, tavern_done: bool) -> Outcome {
    if let Some(gs) = session.game_state() {
        log_tasks(gs);
    }
    if let Outcome::SessionLost = claim_chests(session).await {
        return Outcome::SessionLost;
    }
    if tavern_done {
        if let Outcome::SessionLost = upgrade_guild(session).await {
            return Outcome::SessionLost;
        }
        if let Outcome::SessionLost = buy_attributes(session).await {
            return Outcome::SessionLost;
        }
        if let Outcome::SessionLost = gamble(session).await {
            return Outcome::SessionLost;
        }
        // Points from the shell game may open a chest right away
        if let Outcome::SessionLost = claim_chests(session).await {
            return Outcome::SessionLost;
        }
        // Beer only as the very last resort (gives Thirst for Adventure too, the Tavern continues)
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

    #[test]
    fn beer_only_as_the_last_step_to_mushrooms() {
        let chests = [chest_with(4, 0), chest_with(8, 0), chest_with(12, 10)];
        // earned 9 + Arena 1 = 10, beer +2 → 12 with 10 mushrooms → yes
        let tasks = vec![
            task(TaskType::LeaseMount, 1, 1, 9),
            task(TaskType::WinFightsInArena, 5, 10, 1),
            task(TaskType::DrinkBeer, 0, 1, 2),
        ];
        assert!(beer_opens_chest(&tasks, &chests));
        // the Arena alone will get there → no beer
        let tasks = vec![
            task(TaskType::LeaseMount, 1, 1, 9),
            task(TaskType::WinFightsInArena, 5, 10, 3),
            task(TaskType::DrinkBeer, 0, 1, 2),
        ];
        assert!(!beer_opens_chest(&tasks, &chests));
        // even the beer is not enough → no
        let tasks = vec![task(TaskType::LeaseMount, 1, 1, 6), task(TaskType::DrinkBeer, 0, 1, 2)];
        assert!(!beer_opens_chest(&tasks, &chests));
        // a chest without mushrooms is not worth a mushroom
        let chests = [chest_with(4, 0), chest_with(8, 0), chest_with(12, 0)];
        let tasks = vec![task(TaskType::LeaseMount, 1, 1, 10), task(TaskType::DrinkBeer, 0, 1, 2)];
        assert!(!beer_opens_chest(&tasks, &chests));
    }

    #[test]
    fn gamble_only_when_it_reaches_a_chest() {
        let chests = [chest(4), chest(8), chest(12)];
        // earned 6 + Arena 2 = 8 expected; gambling (+1) does not reach 12 → no
        let tasks = vec![
            task(TaskType::LeaseMount, 1, 1, 6),
            task(TaskType::WinFightsInArena, 5, 10, 2),
            task(TaskType::DefeatGambler, 0, 3, 1),
        ];
        assert!(!gamble_needed(&tasks, &chests));
        // earned 9 + Arena 2 = 11 expected; gambling (+1) reaches 12 → yes
        let tasks = vec![
            task(TaskType::LeaseMount, 1, 1, 9),
            task(TaskType::WinFightsInArena, 5, 10, 2),
            task(TaskType::DefeatGambler, 0, 3, 1),
        ];
        assert!(gamble_needed(&tasks, &chests));
        // already done
        let tasks = vec![task(TaskType::LeaseMount, 1, 1, 11), task(TaskType::DefeatGambler, 3, 3, 1)];
        assert!(!gamble_needed(&tasks, &chests));
    }
}
