//! Tells the platform host when the app packet changed, so it asks for a
//! new one at once instead of on a timer.
//!
//! Background work that changes what a screen shows (a transcript page, a
//! streamed reply, a chat list, a computer's task summary) calls [`ring`].
//! The host keeps one thread in [`wait`] (`openagents_mobile_wait`, or
//! `OpenAgentsNative.waitChange` on Android), which returns as soon as the
//! change count moves; the host then asks for a packet. While the Coder tab
//! shows a live chat, [`wait`] also moves the count once a second on its
//! own ([`TICK`]), so the tab reads its running chat and the computers'
//! state as the hosts' one-second timers did.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How often a live, shown Coder chat asks for a packet with nothing rung.
pub const TICK: Duration = Duration::from_secs(1);
/// The longest one [`wait`] blocks.
pub const MAX_WAIT: Duration = Duration::from_secs(60);

static CHANGES: Mutex<u64> = Mutex::new(0);
static CHANGED: Condvar = Condvar::new();
/// The last packet's Coder tab changes on its own (`coder_live`).
static LIVE: AtomicBool = AtomicBool::new(false);
/// The host shows the Coder tab.
static SHOWN: AtomicBool = AtomicBool::new(false);
/// The app is in the foreground.
static ACTIVE: AtomicBool = AtomicBool::new(true);
/// The Computers service changed, or a tick asked for its state: the next
/// snapshot reloads it.
static COMPUTERS: AtomicBool = AtomicBool::new(false);

fn changes() -> MutexGuard<'static, u64> {
    CHANGES.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn bump(count: &mut u64) {
    *count = count.wrapping_add(1);
    CHANGED.notify_all();
}

/// Something a screen shows changed.
pub fn ring() {
    bump(&mut changes());
}

/// The Computers service's state changed: ring, and have the next snapshot
/// reload it.
pub fn computers() {
    COMPUTERS.store(true, Ordering::Release);
    ring();
}

/// Whether the Computers state should be reloaded now; clears the mark.
pub fn take_computers() -> bool {
    COMPUTERS.swap(false, Ordering::AcqRel)
}

/// Leave the Computers reload for a later snapshot.
pub fn keep_computers() {
    COMPUTERS.store(true, Ordering::Release);
}

/// Record whether the packet just built has a live Coder chat.
pub fn set_live(live: bool) {
    LIVE.store(live, Ordering::Release);
}

/// Record whether the host shows the Coder tab.
pub fn set_shown(shown: bool) {
    SHOWN.store(shown, Ordering::Release);
    if shown {
        ring();
    }
}

/// Record whether the app is in the foreground.
pub fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::Release);
}

fn ticking() -> bool {
    LIVE.load(Ordering::Acquire) && SHOWN.load(Ordering::Acquire) && ACTIVE.load(Ordering::Acquire)
}

/// The change count: `seen` when nothing changed within `limit`, else the
/// count now. While the Coder tab shows a live chat, a [`TICK`] with
/// nothing rung moves the count too and marks the Computers state for a
/// reload. Call it from a thread of its own; any number may wait at once.
pub fn wait(seen: u64, limit: Duration) -> u64 {
    let limit = limit.min(MAX_WAIT);
    let started = Instant::now();
    let mut count = changes();
    loop {
        if *count != seen {
            return *count;
        }
        let elapsed = started.elapsed();
        let tick = ticking();
        if tick && elapsed >= TICK.min(limit) {
            COMPUTERS.store(true, Ordering::Release);
            bump(&mut count);
            return *count;
        }
        if elapsed >= limit {
            return *count;
        }
        // Look again every quarter tick, to see whether a chat became live.
        let slice = (limit - elapsed).min(TICK / 4);
        count = CHANGED
            .wait_timeout(count, slice)
            .unwrap_or_else(|poison| poison.into_inner())
            .0;
    }
}

/// The change count now.
#[cfg(any(test, feature = "test-support"))]
pub fn count() -> u64 {
    *changes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ring_ends_a_wait_at_once() {
        let seen = count();
        let waiter = std::thread::spawn(move || {
            let started = Instant::now();
            (wait(seen, Duration::from_secs(10)), started.elapsed())
        });
        std::thread::sleep(Duration::from_millis(50));
        ring();
        let (now, took) = waiter.join().unwrap();
        assert_ne!(now, seen);
        assert!(took < Duration::from_secs(2), "{took:?}");
        // A count already moved returns without waiting.
        let started = Instant::now();
        assert_ne!(wait(seen, Duration::from_secs(10)), seen);
        assert!(started.elapsed() < Duration::from_millis(100));
    }
}
