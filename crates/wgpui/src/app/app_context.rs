//! Application context for entity operations.

use std::{
    borrow::{Borrow, BorrowMut},
    future::Future,
    ops,
};

use super::{
    entity_map::{Entity, EntityId, EntityMap, WeakEntity},
    subscription::{SubscriberSet, Subscription},
};
use crate::r#async::{BackgroundExecutor, ForegroundExecutor, Task};

type ObserverCallback = Box<dyn FnMut(&mut App) -> bool + 'static>;
type ReleaseCallback = Box<dyn FnOnce(&mut dyn std::any::Any, &mut App) + 'static>;
type DeferredCallback = Box<dyn FnOnce(&mut App) + 'static>;

pub struct App {
    pub(crate) entities: EntityMap,
    pub(crate) observers: SubscriberSet<EntityId, ObserverCallback>,
    pub(crate) release_listeners: SubscriberSet<EntityId, ReleaseCallback>,
    pending_notifications: Vec<EntityId>,
    deferred: Vec<DeferredCallback>,
    flushing: bool,
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
}

impl App {
    pub fn new() -> Self {
        Self {
            entities: EntityMap::new(),
            observers: SubscriberSet::new(),
            release_listeners: SubscriberSet::new(),
            pending_notifications: Vec::new(),
            deferred: Vec::new(),
            flushing: false,
            background_executor: BackgroundExecutor::new(),
            foreground_executor: ForegroundExecutor::new(),
        }
    }

    pub fn new_entity<T: 'static>(
        &mut self,
        build: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        let slot = self.entities.reserve::<T>();
        let entity: Entity<T> = (*slot).clone();

        let mut cx = Context::new(self, entity.downgrade());
        let state = build(&mut cx);

        self.entities.insert(slot, state)
    }

    pub fn update_entity<T: 'static, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        let mut lease = self.entities.lease(handle);
        let weak = handle.downgrade();
        let mut cx = Context::new(self, weak);
        let result = update(&mut lease, &mut cx);
        self.entities.end_lease(lease);
        result
    }

    pub fn read_entity<T: 'static>(&self, handle: &Entity<T>) -> &T {
        self.entities.read(handle)
    }

    pub fn notify(&mut self, entity_id: EntityId) {
        if !self.pending_notifications.contains(&entity_id) {
            self.pending_notifications.push(entity_id);
        }

        if !self.flushing {
            self.flush_effects();
        }
    }

    pub fn observe<T: 'static>(
        &mut self,
        entity: &Entity<T>,
        mut callback: impl FnMut(Entity<T>, &mut App) + 'static,
    ) -> Subscription {
        let entity_id = entity.entity_id();
        let entity_clone = entity.clone();
        let (subscription, activate) = self.observers.insert(
            entity_id,
            Box::new(move |cx| {
                callback(entity_clone.clone(), cx);
                true
            }),
        );
        activate();
        subscription
    }

    pub(crate) fn observe_internal(
        &mut self,
        entity_id: EntityId,
        callback: impl FnMut(&mut App) -> bool + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.observers.insert(entity_id, Box::new(callback));
        activate();
        subscription
    }

    pub fn defer(&mut self, callback: impl FnOnce(&mut App) + 'static) {
        self.deferred.push(Box::new(callback));
        if !self.flushing {
            self.flush_effects();
        }
    }

    fn flush_effects(&mut self) {
        self.flushing = true;

        loop {
            while let Some(entity_id) = self.pending_notifications.pop() {
                let observers = self.observers.clone();
                observers.retain(&entity_id, |callback| callback(self));
            }

            let dropped = self.entities.take_dropped();
            for (entity_id, mut entity) in dropped {
                let callbacks: Vec<_> = self
                    .release_listeners
                    .remove(&entity_id)
                    .into_iter()
                    .collect();
                for callback in callbacks {
                    callback(&mut *entity, self);
                }
            }

            while let Some(callback) = self.deferred.pop() {
                callback(self);
            }

            if self.pending_notifications.is_empty() && self.deferred.is_empty() {
                break;
            }
        }

        self.flushing = false;
    }

    pub fn background_executor(&self) -> &BackgroundExecutor {
        &self.background_executor
    }

    pub fn foreground_executor(&self) -> &ForegroundExecutor {
        &self.foreground_executor
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Context<'a, T> {
    app: &'a mut App,
    entity_state: WeakEntity<T>,
}

impl<'a, T> ops::Deref for Context<'a, T> {
    type Target = App;

    fn deref(&self) -> &Self::Target {
        self.app
    }
}

impl<'a, T> ops::DerefMut for Context<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.app
    }
}

impl<'a, T: 'static> Context<'a, T> {
    pub(crate) fn new(app: &'a mut App, entity_state: WeakEntity<T>) -> Self {
        Self { app, entity_state }
    }

    pub fn entity_id(&self) -> EntityId {
        self.entity_state.entity_id
    }

    pub fn entity(&self) -> Entity<T> {
        self.weak_entity()
            .upgrade()
            .expect("The entity must be alive if we have an entity context")
    }

    pub fn weak_entity(&self) -> WeakEntity<T> {
        self.entity_state.clone()
    }

    pub fn notify(&mut self) {
        self.app.notify(self.entity_state.entity_id);
    }

    pub fn observe<W: 'static>(
        &mut self,
        entity: &Entity<W>,
        mut on_notify: impl FnMut(&mut T, Entity<W>, &mut Context<T>) + 'static,
    ) -> Subscription {
        let this = self.weak_entity();
        let entity_id = entity.entity_id();
        let entity_clone = entity.clone();
        self.app.observe_internal(entity_id, move |cx| {
            if let Some(this) = this.upgrade() {
                cx.update_entity(&this, |state, cx| {
                    on_notify(state, entity_clone.clone(), cx)
                });
                true
            } else {
                false
            }
        })
    }

    pub fn on_release(&self, on_release: impl FnOnce(&mut T, &mut App) + 'static) -> Subscription {
        let (subscription, activate) = self.app.release_listeners.insert(
            self.entity_state.entity_id,
            Box::new(move |entity, cx| {
                let entity = entity.downcast_mut::<T>().expect("invalid entity type");
                on_release(entity, cx);
            }),
        );
        activate();
        subscription
    }

    pub fn spawn<R>(&mut self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.app.background_executor.spawn(future)
    }
}

impl<T> Borrow<App> for Context<'_, T> {
    fn borrow(&self) -> &App {
        self.app
    }
}

impl<T> BorrowMut<App> for Context<'_, T> {
    fn borrow_mut(&mut self) -> &mut App {
        self.app
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Counter {
        value: i32,
    }

    struct ReleaseCounter {
        _release: Subscription,
    }

    #[test]
    fn test_new_entity() {
        let mut app = App::new();

        let entity = app.new_entity(|_cx| Counter { value: 0 });

        let counter = app.read_entity(&entity);
        assert_eq!(counter.value, 0);
    }

    #[test]
    fn test_update_entity() {
        let mut app = App::new();

        let entity = app.new_entity(|_cx| Counter { value: 0 });

        app.update_entity(&entity, |counter, _cx| {
            counter.value = 42;
        });

        let counter = app.read_entity(&entity);
        assert_eq!(counter.value, 42);
    }

    #[test]
    fn test_notify_and_observe() {
        let mut app = App::new();
        let observed_count: Rc<RefCell<i32>> = Rc::new(RefCell::new(0));
        let observed_count_clone = observed_count.clone();

        let entity = app.new_entity(|_cx| Counter { value: 0 });

        let _sub = app.observe(&entity, move |_entity, _app| {
            *(*observed_count_clone).borrow_mut() += 1;
        });

        app.update_entity(&entity, |counter, cx| {
            counter.value = 1;
            cx.notify();
        });

        assert_eq!(*(*observed_count).borrow(), 1);
    }

    #[test]
    fn test_defer_runs_during_flush() {
        let mut app = App::new();
        let ran = Rc::new(RefCell::new(false));
        let ran_clone = ran.clone();

        let entity = app.new_entity(|_cx| Counter { value: 0 });
        let _sub = app.observe(&entity, move |_entity, app| {
            let ran_clone = ran_clone.clone();
            app.defer(move |_app| {
                *(*ran_clone).borrow_mut() = true;
            });
        });

        app.notify(entity.entity_id());

        assert!(*(*ran).borrow());
    }

    #[test]
    fn test_release_notify_flushes() {
        let mut app = App::new();
        let observed_count: Rc<RefCell<i32>> = Rc::new(RefCell::new(0));
        let observed_count_clone = observed_count.clone();

        let target = app.new_entity(|_cx| Counter { value: 0 });
        let target_id = target.entity_id();
        let _sub = app.observe(&target, move |_entity, _app| {
            *(*observed_count_clone).borrow_mut() += 1;
        });

        let target_for_release = target.clone();
        let entity = app.new_entity(|cx| {
            let release = cx.on_release(move |_state: &mut ReleaseCounter, app| {
                app.notify(target_for_release.entity_id());
            });
            ReleaseCounter { _release: release }
        });

        drop(entity);
        app.notify(target_id);

        assert_eq!(*(*observed_count).borrow(), 2);
    }
}
