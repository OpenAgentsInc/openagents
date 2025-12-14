//! Signal - reactive state container.
//!
//! Signals are the core primitive for reactive state. When a signal's
//! value changes, all dependent effects and memos are notified.

use crate::runtime::{with_runtime, SubscriberId};
use parking_lot::RwLock;
use smallvec::SmallVec;
use std::sync::Arc;

/// A reactive signal that holds a value and notifies subscribers on change.
pub struct Signal<T> {
    inner: Arc<SignalInner<T>>,
}

struct SignalInner<T> {
    value: RwLock<T>,
    subscribers: RwLock<SmallVec<[SubscriberId; 4]>>,
}

impl<T> Signal<T> {
    /// Create a new signal with an initial value.
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(SignalInner {
                value: RwLock::new(value),
                subscribers: RwLock::new(SmallVec::new()),
            }),
        }
    }

    /// Subscribe to this signal.
    pub(crate) fn subscribe(&self, subscriber: SubscriberId) {
        let mut subs = self.inner.subscribers.write();
        if !subs.contains(&subscriber) {
            subs.push(subscriber);
        }
    }

    /// Unsubscribe from this signal.
    pub(crate) fn unsubscribe(&self, subscriber: SubscriberId) {
        let mut subs = self.inner.subscribers.write();
        subs.retain(|s| *s != subscriber);
    }

    /// Notify all subscribers that the value changed.
    fn notify(&self) {
        let subs = self.inner.subscribers.read();
        with_runtime(|rt| {
            for &sub in subs.iter() {
                rt.queue_effect(sub);
            }
        });
    }

    /// Split into read and write handles.
    pub fn split(&self) -> (ReadSignal<T>, WriteSignal<T>) {
        (
            ReadSignal {
                inner: self.inner.clone(),
            },
            WriteSignal {
                inner: self.inner.clone(),
            },
        )
    }
}

impl<T: Clone> Signal<T> {
    /// Get the current value, tracking the dependency.
    pub fn get(&self) -> T {
        // Track dependency
        if let Some(subscriber) = with_runtime(|rt| rt.current_subscriber()) {
            self.subscribe(subscriber);
        }
        self.inner.value.read().clone()
    }

    /// Get the current value without tracking.
    pub fn get_untracked(&self) -> T {
        self.inner.value.read().clone()
    }

    /// Set a new value, notifying subscribers.
    pub fn set(&self, value: T) {
        *self.inner.value.write() = value;
        self.notify();
    }

    /// Update the value using a function.
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        {
            let mut guard = self.inner.value.write();
            f(&mut guard);
        }
        self.notify();
    }
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Read-only view of a signal.
pub struct ReadSignal<T> {
    inner: Arc<SignalInner<T>>,
}

impl<T: Clone> ReadSignal<T> {
    /// Get the current value, tracking the dependency.
    pub fn get(&self) -> T {
        // Track dependency
        if let Some(subscriber) = with_runtime(|rt| rt.current_subscriber()) {
            let mut subs = self.inner.subscribers.write();
            if !subs.contains(&subscriber) {
                subs.push(subscriber);
            }
        }
        self.inner.value.read().clone()
    }

    /// Get the current value without tracking.
    pub fn get_untracked(&self) -> T {
        self.inner.value.read().clone()
    }
}

impl<T> Clone for ReadSignal<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Write-only view of a signal.
pub struct WriteSignal<T> {
    inner: Arc<SignalInner<T>>,
}

impl<T> WriteSignal<T> {
    /// Set a new value, notifying subscribers.
    pub fn set(&self, value: T) {
        *self.inner.value.write() = value;
        let subs = self.inner.subscribers.read();
        with_runtime(|rt| {
            for &sub in subs.iter() {
                rt.queue_effect(sub);
            }
        });
    }

    /// Update the value using a function.
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        {
            let mut guard = self.inner.value.write();
            f(&mut guard);
        }
        let subs = self.inner.subscribers.read();
        with_runtime(|rt| {
            for &sub in subs.iter() {
                rt.queue_effect(sub);
            }
        });
    }
}

impl<T> Clone for WriteSignal<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Create a new signal with the given initial value.
pub fn create_signal<T>(value: T) -> Signal<T> {
    Signal::new(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_get_set() {
        let signal = create_signal(0);
        assert_eq!(signal.get_untracked(), 0);

        signal.set(42);
        assert_eq!(signal.get_untracked(), 42);
    }

    #[test]
    fn test_signal_update() {
        let signal = create_signal(10);

        signal.update(|v| *v += 5);
        assert_eq!(signal.get_untracked(), 15);
    }

    #[test]
    fn test_signal_split() {
        let signal = create_signal(100);
        let (read, write) = signal.split();

        assert_eq!(read.get_untracked(), 100);

        write.set(200);
        assert_eq!(read.get_untracked(), 200);
    }

    #[test]
    fn test_signal_clone() {
        let signal1 = create_signal(5);
        let signal2 = signal1.clone();

        signal1.set(10);
        assert_eq!(signal2.get_untracked(), 10);
    }
}
