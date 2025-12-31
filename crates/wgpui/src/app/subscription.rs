//! Subscription management for reactive state updates.
//!
//! Subscriptions are handles that automatically unsubscribe when dropped,
//! ensuring clean resource management for observers and event handlers.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// A handle to a subscription. When dropped, the subscription is cancelled.
#[must_use]
pub struct Subscription {
    unsubscribe: Option<Box<dyn FnOnce() + 'static>>,
}

impl Subscription {
    /// Creates a new subscription with the given unsubscribe callback.
    pub fn new(unsubscribe: impl FnOnce() + 'static) -> Self {
        Self {
            unsubscribe: Some(Box::new(unsubscribe)),
        }
    }

    /// Detaches the subscription, preventing automatic unsubscription on drop.
    /// The subscription will persist until the emitter is dropped.
    pub fn detach(mut self) {
        self.unsubscribe.take();
    }

    /// Joins two subscriptions into one. When the combined subscription is
    /// dropped or detached, both interior subscriptions are affected.
    pub fn join(mut a: Self, mut b: Self) -> Self {
        let a_unsub = a.unsubscribe.take();
        let b_unsub = b.unsubscribe.take();
        Self {
            unsubscribe: Some(Box::new(move || {
                if let Some(unsub) = a_unsub {
                    unsub();
                }
                if let Some(unsub) = b_unsub {
                    unsub();
                }
            })),
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(unsubscribe) = self.unsubscribe.take() {
            unsubscribe();
        }
    }
}

impl std::fmt::Debug for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Subscription").finish()
    }
}

struct Subscriber<Callback> {
    active: Rc<Cell<bool>>,
    callback: Callback,
}

struct SubscriberSetState<EmitterKey, Callback> {
    subscribers: BTreeMap<EmitterKey, Option<BTreeMap<usize, Subscriber<Callback>>>>,
    dropped_subscribers: BTreeSet<(EmitterKey, usize)>,
    next_subscriber_id: usize,
}

/// A collection of subscribers keyed by emitter.
///
/// This is used internally to manage observers and event subscriptions.
pub(crate) struct SubscriberSet<EmitterKey, Callback>(
    Rc<RefCell<SubscriberSetState<EmitterKey, Callback>>>,
);

impl<EmitterKey, Callback> Clone for SubscriberSet<EmitterKey, Callback> {
    fn clone(&self) -> Self {
        SubscriberSet(self.0.clone())
    }
}

impl<EmitterKey, Callback> SubscriberSet<EmitterKey, Callback>
where
    EmitterKey: 'static + Ord + Clone + std::fmt::Debug,
    Callback: 'static,
{
    /// Creates a new empty subscriber set.
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(SubscriberSetState {
            subscribers: Default::default(),
            dropped_subscribers: Default::default(),
            next_subscriber_id: 0,
        })))
    }

    /// Inserts a new subscription for the given emitter key.
    ///
    /// Returns a tuple of (Subscription, activate_fn). The subscription is
    /// initially inert - call the activate function to make it active.
    pub fn insert(
        &self,
        emitter_key: EmitterKey,
        callback: Callback,
    ) -> (Subscription, impl FnOnce()) {
        let active = Rc::new(Cell::new(false));
        let mut lock = self.0.borrow_mut();
        let subscriber_id = lock.next_subscriber_id;
        lock.next_subscriber_id += 1;

        lock.subscribers
            .entry(emitter_key.clone())
            .or_default()
            .get_or_insert_with(Default::default)
            .insert(
                subscriber_id,
                Subscriber {
                    active: active.clone(),
                    callback,
                },
            );

        let this = self.0.clone();
        let emitter_key_clone = emitter_key.clone();

        let subscription = Subscription::new(move || {
            let mut lock = this.borrow_mut();
            let Some(subscribers) = lock.subscribers.get_mut(&emitter_key_clone) else {
                return;
            };

            if let Some(subscribers) = subscribers {
                subscribers.remove(&subscriber_id);
                if subscribers.is_empty() {
                    lock.subscribers.remove(&emitter_key_clone);
                }
                return;
            }

            lock.dropped_subscribers
                .insert((emitter_key_clone, subscriber_id));
        });

        (subscription, move || active.set(true))
    }

    /// Removes all subscribers for the given emitter and returns their callbacks.
    pub fn remove(&self, emitter: &EmitterKey) -> impl IntoIterator<Item = Callback> {
        let subscribers = self.0.borrow_mut().subscribers.remove(emitter);
        subscribers
            .unwrap_or_default()
            .map(|s| s.into_values())
            .into_iter()
            .flatten()
            .filter_map(|subscriber| {
                if subscriber.active.get() {
                    Some(subscriber.callback)
                } else {
                    None
                }
            })
    }

    /// Calls the given function for each active subscriber to the emitter.
    /// If the function returns false, the subscriber is removed.
    pub fn retain<F>(&self, emitter: &EmitterKey, mut f: F)
    where
        F: FnMut(&mut Callback) -> bool,
    {
        let Some(mut subscribers) = self
            .0
            .borrow_mut()
            .subscribers
            .get_mut(emitter)
            .and_then(|s| s.take())
        else {
            return;
        };

        subscribers.retain(|_, subscriber| {
            if subscriber.active.get() {
                f(&mut subscriber.callback)
            } else {
                true
            }
        });

        let mut lock = self.0.borrow_mut();

        if let Some(Some(new_subscribers)) = lock.subscribers.remove(emitter) {
            subscribers.extend(new_subscribers);
        }

        let dropped = std::mem::take(&mut lock.dropped_subscribers);
        let mut remaining_dropped = BTreeSet::new();
        for (dropped_emitter, dropped_id) in dropped {
            if dropped_emitter == *emitter {
                subscribers.remove(&dropped_id);
            } else {
                remaining_dropped.insert((dropped_emitter, dropped_id));
            }
        }
        lock.dropped_subscribers = remaining_dropped;

        if !subscribers.is_empty() {
            lock.subscribers.insert(emitter.clone(), Some(subscribers));
        }
    }
}

impl<EmitterKey, Callback> Default for SubscriberSet<EmitterKey, Callback>
where
    EmitterKey: 'static + Ord + Clone + std::fmt::Debug,
    Callback: 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_subscription_drop() {
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        let sub = Subscription::new(move || {
            *called_clone.borrow_mut() = true;
        });

        assert!(!*called.borrow());
        drop(sub);
        assert!(*called.borrow());
    }

    #[test]
    fn test_subscription_detach() {
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        let sub = Subscription::new(move || {
            *called_clone.borrow_mut() = true;
        });

        sub.detach();
        assert!(!*called.borrow());
    }

    #[test]
    fn test_subscription_join() {
        let count = Rc::new(RefCell::new(0));
        let count1 = count.clone();
        let count2 = count.clone();

        let sub1 = Subscription::new(move || {
            *count1.borrow_mut() += 1;
        });
        let sub2 = Subscription::new(move || {
            *count2.borrow_mut() += 1;
        });

        let joined = Subscription::join(sub1, sub2);
        assert_eq!(*count.borrow(), 0);
        drop(joined);
        assert_eq!(*count.borrow(), 2);
    }

    #[test]
    fn test_subscriber_set_insert_remove() {
        let set: SubscriberSet<u32, Box<dyn FnMut()>> = SubscriberSet::new();

        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        let (sub, activate) = set.insert(
            1,
            Box::new(move || {
                *called_clone.borrow_mut() = true;
            }),
        );
        activate();

        let callbacks: Vec<_> = set.remove(&1).into_iter().collect();
        assert_eq!(callbacks.len(), 1);

        drop(sub);
    }

    #[test]
    fn test_subscriber_set_retain() {
        let set: SubscriberSet<u32, Box<dyn FnMut() -> bool>> = SubscriberSet::new();

        let count = Rc::new(RefCell::new(0));
        let count_clone = count.clone();

        let (_sub, activate) = set.insert(
            1,
            Box::new(move || {
                *count_clone.borrow_mut() += 1;
                *count_clone.borrow() < 3
            }),
        );
        activate();

        // First three calls should succeed
        for _ in 0..3 {
            set.retain(&1, |cb| cb());
        }

        assert_eq!(*count.borrow(), 3);
    }

    #[test]
    fn test_retain_handles_nested_drops() {
        let set: SubscriberSet<u32, Box<dyn FnMut() -> bool>> = SubscriberSet::new();

        let (sub1, activate1) = set.insert(1, Box::new(|| true));
        let (_sub1b, activate1b) = set.insert(1, Box::new(|| true));
        let (_sub2, activate2) = set.insert(2, Box::new(|| true));
        activate1();
        activate1b();
        activate2();

        let drop_target = RefCell::new(Some(sub1));

        set.retain(&1, |cb| {
            set.retain(&2, |_cb2| {
                if let Some(sub) = drop_target.borrow_mut().take() {
                    drop(sub);
                }
                true
            });
            cb()
        });

        let remaining: Vec<_> = set.remove(&1).into_iter().collect();
        assert_eq!(remaining.len(), 1);
    }
}
