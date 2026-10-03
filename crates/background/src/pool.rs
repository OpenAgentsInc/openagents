//! A bounded pool for checks that mostly wait on the disk or on Git: run
//! `f` over the items several at a time and keep the items' order.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// At most this many at once: Git and directory walks on one disk stop
/// getting faster well before every core is busy.
const MOST: usize = 12;

/// `f` over `items`, several at a time, results in `items`' order. A panic
/// in `f` panics here.
pub fn map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let workers = std::thread::available_parallelism()
        .map_or(4, std::num::NonZeroUsize::get)
        .min(MOST)
        .min(items.len());
    if workers <= 1 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<R>>> = Mutex::new(items.iter().map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(index) else {
                        break;
                    };
                    let result = f(item);
                    out.lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)[index] = Some(result);
                }
            });
        }
    });
    out.into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .into_iter()
        .map(|result| result.expect("every item ran"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::map;

    #[test]
    fn keeps_order_and_runs_every_item() {
        let items: Vec<u32> = (0..100).collect();
        let doubled = map(&items, |n| {
            std::thread::sleep(std::time::Duration::from_millis(u64::from(n % 7)));
            n * 2
        });
        assert_eq!(doubled, items.iter().map(|n| n * 2).collect::<Vec<_>>());
        assert!(map(&Vec::<u32>::new(), |n| *n).is_empty());
    }
}
