//! A small stale-while-refresh cache for GitHub lists (#11057): an entry is
//! fresh for [`FRESH`] and kept for [`KEEP`]. A fresh entry is served as
//! is. A kept but stale one is served at once while one background read
//! refreshes it. A missing one is read now, inside a spawned task, so a
//! caller that gives up still fills the cache. Failed reads are never
//! cached, and a failed refresh keeps the stale entry.
//!
//! Entries live in groups (one account's repository pages, one person's
//! branch lists) that are dropped together.
//!
//! Every GitHub token (and the anonymous one per server address) has its
//! own hourly budget; caching lists keeps repeated page renders and filter
//! submits from spending it.

use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a list is served without asking GitHub again.
pub const FRESH: Duration = Duration::from_secs(5 * 60);
/// How long a list is kept to serve while it refreshes, or while GitHub
/// is limiting requests.
pub const KEEP: Duration = Duration::from_secs(60 * 60);
/// The most entries one cache holds; the oldest go first.
const MAX_ENTRIES: usize = 4096;

struct Entry<V> {
    value: V,
    at: Instant,
    refreshing: bool,
}

/// One group's entries.
struct Group<P, V> {
    entries: HashMap<P, Entry<V>>,
    /// Bumped when the group is dropped, so a read that started before
    /// can't put back what was just dropped.
    epoch: u64,
}

impl<P, V> Default for Group<P, V> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            epoch: 0,
        }
    }
}

/// A stale-while-refresh cache of `V` by group `G` and item `P`.
pub struct Lists<G, P, V> {
    inner: Mutex<HashMap<G, Group<P, V>>>,
}

impl<G, P, V> Default for Lists<G, P, V> {
    fn default() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }
}

enum Found<V> {
    Fresh(V),
    /// Stale: serve it, and refresh when `refresh`.
    Stale(V, bool),
    Missing,
}

impl<G, P, V> Lists<G, P, V>
where
    G: Clone + Eq + Hash + Send + 'static,
    P: Clone + Eq + Hash + Send + 'static,
    V: Clone + Send + 'static,
{
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<G, Group<P, V>>> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn find(&self, group: &G, item: &P, may_refresh: bool) -> (Found<V>, u64) {
        let mut groups = self.lock();
        let Some(found) = groups.get_mut(group) else {
            return (Found::Missing, 0);
        };
        let epoch = found.epoch;
        let answer = match found.entries.get_mut(item) {
            Some(entry) if entry.at.elapsed() < FRESH => Found::Fresh(entry.value.clone()),
            Some(entry) if entry.at.elapsed() < KEEP => {
                let refresh = may_refresh && !entry.refreshing;
                if refresh {
                    entry.refreshing = true;
                }
                Found::Stale(entry.value.clone(), refresh)
            }
            Some(_) => {
                found.entries.remove(item);
                Found::Missing
            }
            None => Found::Missing,
        };
        (answer, epoch)
    }

    /// Keep `value`, unless the group was dropped since `epoch`.
    fn put(&self, group: G, item: P, value: V, epoch: u64) {
        let mut groups = self.lock();
        if groups.get(&group).map_or(0, |found| found.epoch) != epoch {
            return;
        }
        let total: usize = groups.values().map(|found| found.entries.len()).sum();
        if total >= MAX_ENTRIES {
            let oldest = groups
                .iter()
                .flat_map(|(g, found)| found.entries.iter().map(move |(p, e)| (g, p, e.at)))
                .min_by_key(|(_, _, at)| *at)
                .map(|(g, p, _)| (g.clone(), p.clone()));
            if let Some((g, p)) = oldest
                && let Some(found) = groups.get_mut(&g)
            {
                found.entries.remove(&p);
            }
        }
        if groups.len() >= MAX_ENTRIES {
            // Groups with nothing kept are forgotten (an epoch read before
            // then no longer matches, so that read just isn't kept).
            groups.retain(|_, found| !found.entries.is_empty());
        }
        groups.entry(group).or_default().entries.insert(
            item,
            Entry {
                value,
                at: Instant::now(),
                refreshing: false,
            },
        );
    }

    fn refresh_failed(&self, group: &G, item: &P) {
        if let Some(entry) = self
            .lock()
            .get_mut(group)
            .and_then(|found| found.entries.get_mut(item))
        {
            entry.refreshing = false;
        }
    }

    /// The list for `group` and `item`: from the cache when kept, else
    /// from `read`. With `may_refresh` false (GitHub's budget is nearly
    /// spent) a stale entry is served without reading again. `lost` is the
    /// error when the read's task ends without an answer.
    pub async fn get<F, Fut, E>(
        &'static self,
        group: G,
        item: P,
        may_refresh: bool,
        read: F,
        lost: E,
    ) -> Result<V, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, E>> + Send + 'static,
        E: Send + 'static,
    {
        match self.find(&group, &item, may_refresh) {
            (Found::Fresh(value) | Found::Stale(value, false), _) => Ok(value),
            (Found::Stale(value, true), epoch) => {
                let future = read();
                tokio::spawn(async move {
                    match future.await {
                        Ok(fresh) => self.put(group, item, fresh, epoch),
                        Err(_) => self.refresh_failed(&group, &item),
                    }
                });
                Ok(value)
            }
            (Found::Missing, epoch) => {
                let future = read();
                let task = tokio::spawn(async move {
                    let answer = future.await;
                    if let Ok(value) = &answer {
                        self.put(group, item, value.clone(), epoch);
                    }
                    answer
                });
                task.await.unwrap_or(Err(lost))
            }
        }
    }

    /// Drop everything kept for `group`.
    pub fn remove(&self, group: &G) {
        let mut groups = self.lock();
        let found = groups.entry(group.clone()).or_default();
        found.entries.clear();
        found.epoch = found.epoch.wrapping_add(1);
    }

    /// Make the entries of the groups that match older by `by` (tests:
    /// step past [`FRESH`] or [`KEEP`] without waiting).
    #[doc(hidden)]
    pub fn age_where(&self, by: Duration, matches: impl Fn(&G) -> bool) {
        for (group, found) in self.lock().iter_mut() {
            if matches(group) {
                for entry in found.entries.values_mut() {
                    entry.at = entry.at.checked_sub(by).unwrap_or(entry.at);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static READS: AtomicUsize = AtomicUsize::new(0);

    async fn read(value: u32) -> Result<u32, &'static str> {
        READS.fetch_add(1, Ordering::SeqCst);
        Ok(value)
    }

    type Cache = Lists<&'static str, &'static str, u32>;

    async fn get(
        cache: &'static Cache,
        item: &'static str,
        value: u32,
    ) -> Result<u32, &'static str> {
        cache.get("g", item, true, || read(value), "lost").await
    }

    #[tokio::test]
    async fn fresh_then_stale_while_refreshing_then_gone() {
        static CACHE: LazyLock<Cache> = LazyLock::new(Lists::new);
        let before = READS.load(Ordering::SeqCst);
        assert_eq!(get(&CACHE, "a", 1).await, Ok(1));
        assert_eq!(get(&CACHE, "a", 2).await, Ok(1));
        assert_eq!(READS.load(Ordering::SeqCst) - before, 1);

        // Stale: the old value at once, the new one after the refresh.
        CACHE.age_where(FRESH, |_| true);
        assert_eq!(get(&CACHE, "a", 3).await, Ok(1));
        for _ in 0..200 {
            let now = CACHE.get("g", "a", false, || read(9), "lost").await;
            if now == Ok(3) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(get(&CACHE, "a", 4).await, Ok(3));

        // A failed refresh keeps the stale value; a failed read isn't kept.
        CACHE.age_where(FRESH, |_| true);
        let failed = CACHE
            .get("g", "a", true, || async { Err("limited") }, "lost")
            .await;
        assert_eq!(failed, Ok(3));
        let failed = CACHE
            .get("g", "b", true, || async { Err("limited") }, "lost")
            .await;
        assert_eq!(failed, Err("limited"));
        assert_eq!(get(&CACHE, "b", 7).await, Ok(7));

        // Past KEEP it is read again; dropping the group drops it.
        CACHE.age_where(KEEP, |_| true);
        assert_eq!(get(&CACHE, "a", 5).await, Ok(5));
        CACHE.remove(&"g");
        assert_eq!(get(&CACHE, "a", 6).await, Ok(6));
    }
}
