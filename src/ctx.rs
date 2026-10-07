//! Several characters in one process (user 2026-10-07): every character runs in its own tokio task inside
//! `CHARACTER.scope(name, …)`. Code that keeps state between calls uses `PerChar<T>` instead of a plain
//! `Mutex<T>`: the same `.lock()` API, but every character has its own value (like a `[ThreadStatic]` per
//! character in C#).

use std::{
    ops::{Deref, DerefMut},
    sync::{Mutex, MutexGuard},
};

tokio::task_local! {
    /// Name of the character the current task plays.
    pub static CHARACTER: String;
}

/// The character of the current task ("" outside of a character task, e.g. in tests).
pub fn name() -> String {
    CHARACTER.try_with(Clone::clone).unwrap_or_default()
}

/// Log file of the current character: `logs/<character>/<file>` (`logs/<file>` outside of a character task).
pub fn log_path(file: &str) -> String {
    let n = name();
    if n.is_empty() { format!("logs/{file}") } else { format!("logs/{n}/{file}") }
}

/// "Run end of day now" from the icon menu: a request counter + a wake-up for the sleeping main loops.
pub static EOD_REQUESTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static EOD_WAKE: tokio::sync::Notify = tokio::sync::Notify::const_new();

pub fn request_end_of_day() {
    EOD_REQUESTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    EOD_WAKE.notify_waiters();
}

/// Has the current character not handled the latest manual request yet? Marks it handled.
pub fn take_end_of_day_request() -> bool {
    static SEEN: PerChar<u64> = PerChar::new();
    let now = EOD_REQUESTS.load(std::sync::atomic::Ordering::SeqCst);
    let Ok(mut seen) = SEEN.lock() else { return false };
    let new = *seen < now;
    *seen = now;
    new
}

/// A value kept separately for every character.
pub struct PerChar<T>(Mutex<Vec<(String, T)>>);

pub struct PerCharGuard<'a, T> {
    guard: MutexGuard<'a, Vec<(String, T)>>,
    index: usize,
}

impl<T: Default> PerChar<T> {
    pub const fn new() -> Self {
        Self(Mutex::new(Vec::new()))
    }

    /// The current character's value (created with `Default` on first use). Same shape as `Mutex::lock`.
    pub fn lock(&self) -> Result<PerCharGuard<'_, T>, ()> {
        let key = name();
        let mut guard = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let index = match guard.iter().position(|(k, _)| *k == key) {
            Some(i) => i,
            None => {
                guard.push((key, T::default()));
                guard.len() - 1
            }
        };
        Ok(PerCharGuard { guard, index })
    }
}

impl<T> Deref for PerCharGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard[self.index].1
    }
}

impl<T> DerefMut for PerCharGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard[self.index].1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static COUNTER: PerChar<u32> = PerChar::new();

    #[tokio::test]
    async fn every_character_has_its_own_value() {
        for (who, n) in [("A", 3), ("B", 5)] {
            CHARACTER
                .scope(who.to_string(), async move {
                    for _ in 0..n {
                        *COUNTER.lock().unwrap() += 1;
                    }
                })
                .await;
        }
        let a = CHARACTER.scope("A".to_string(), async { *COUNTER.lock().unwrap() }).await;
        let b = CHARACTER.scope("B".to_string(), async { *COUNTER.lock().unwrap() }).await;
        assert_eq!((a, b), (3, 5));
    }
}
