use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

static NEXT_ANIMATOR_ID: AtomicU64 = AtomicU64::new(1);

/// Unique identifier for animator nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimatorId(u64);

impl AnimatorId {
    fn next() -> Self {
        Self(NEXT_ANIMATOR_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Numeric identifier value.
    pub fn value(self) -> u64 {
        self.0
    }
}

/// Animator lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimatorState {
    Entered,
    Entering,
    Exiting,
    #[default]
    Exited,
}

pub type AnimatorConditionFn = Arc<dyn Fn(AnimatorId) -> bool + Send + Sync>;
pub type AnimatorTransitionFn = Arc<dyn Fn(AnimatorId, AnimatorState) + Send + Sync>;
pub type AnimatorSubscriber = Arc<dyn Fn(AnimatorId, AnimatorState) + Send + Sync>;

#[derive(Clone)]
pub enum AnimatorCondition {
    Static(bool),
    Dynamic(AnimatorConditionFn),
}

impl AnimatorCondition {
    fn allows(&self, id: AnimatorId) -> bool {
        match self {
            AnimatorCondition::Static(value) => *value,
            AnimatorCondition::Dynamic(func) => func(id),
        }
    }
}

impl From<bool> for AnimatorCondition {
    fn from(value: bool) -> Self {
        AnimatorCondition::Static(value)
    }
}

impl<F> From<F> for AnimatorCondition
where
    F: Fn(AnimatorId) -> bool + Send + Sync + 'static,
{
    fn from(func: F) -> Self {
        AnimatorCondition::Dynamic(Arc::new(func))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AnimatorSubscription(usize);

/// Orchestration strategy for child nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimatorManagerKind {
    #[default]
    Parallel,
    Stagger,
    StaggerReverse,
    Sequence,
    SequenceReverse,
    Switch,
}

/// Timing configuration for a single node.
#[derive(Debug, Clone, Copy)]
pub struct AnimatorTiming {
    pub enter: Duration,
    pub exit: Duration,
    pub delay: Duration,
    pub offset: Duration,
}

/// Partial update for animator timing values.
#[derive(Clone, Copy, Debug, Default)]
pub struct AnimatorTimingUpdate {
    pub enter: Option<Duration>,
    pub exit: Option<Duration>,
    pub delay: Option<Duration>,
    pub offset: Option<Duration>,
}

impl AnimatorTiming {
    pub const fn new(enter: Duration, exit: Duration, delay: Duration) -> Self {
        Self {
            enter,
            exit,
            delay,
            offset: Duration::ZERO,
        }
    }

    pub const fn with_offset(
        enter: Duration,
        exit: Duration,
        delay: Duration,
        offset: Duration,
    ) -> Self {
        Self {
            enter,
            exit,
            delay,
            offset,
        }
    }
}

impl Default for AnimatorTiming {
    fn default() -> Self {
        Self {
            enter: Duration::from_millis(400),
            exit: Duration::from_millis(400),
            delay: Duration::ZERO,
            offset: Duration::ZERO,
        }
    }
}

/// Node settings used by managers and timing.
#[derive(Clone)]
pub struct AnimatorSettings {
    pub timing: AnimatorTiming,
    pub stagger: Duration,
    pub active: bool,
    pub combine: bool,
    pub merge: bool,
    pub initial_state: AnimatorState,
    pub condition: Option<AnimatorCondition>,
    pub on_transition: Option<AnimatorTransitionFn>,
    pub limit: Option<f32>,
}

/// Partial update for animator settings.
#[derive(Clone, Default)]
pub struct AnimatorSettingsUpdate {
    pub timing: Option<AnimatorTimingUpdate>,
    pub stagger: Option<Duration>,
    pub active: Option<bool>,
    pub combine: Option<bool>,
    pub merge: Option<bool>,
    pub initial_state: Option<AnimatorState>,
    pub condition: Option<Option<AnimatorCondition>>,
    pub on_transition: Option<Option<AnimatorTransitionFn>>,
    pub limit: Option<Option<f32>>,
    pub manager: Option<AnimatorManagerKind>,
}

impl AnimatorSettings {
    pub const fn new(timing: AnimatorTiming, stagger: Duration) -> Self {
        Self {
            timing,
            stagger,
            active: true,
            combine: false,
            merge: false,
            initial_state: AnimatorState::Exited,
            condition: None,
            on_transition: None,
            limit: None,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn combine(mut self, combine: bool) -> Self {
        self.combine = combine;
        self
    }

    pub fn merge(mut self, merge: bool) -> Self {
        self.merge = merge;
        self
    }

    pub fn initial_state(mut self, state: AnimatorState) -> Self {
        self.initial_state = state;
        self
    }

    pub fn condition(mut self, condition: AnimatorCondition) -> Self {
        self.condition = Some(condition);
        self
    }

    pub fn on_transition(mut self, callback: AnimatorTransitionFn) -> Self {
        self.on_transition = Some(callback);
        self
    }

    pub fn limit(mut self, limit: f32) -> Self {
        self.limit = Some(limit);
        self
    }
}

impl Default for AnimatorSettings {
    fn default() -> Self {
        Self::new(AnimatorTiming::default(), Duration::from_millis(40))
    }
}

/// Messages sent from child nodes to a parent.
#[derive(Clone)]
pub enum AnimatorMessage {
    StateChanged {
        id: AnimatorId,
        state: AnimatorState,
    },
    SettingsChanged {
        id: AnimatorId,
        timing: AnimatorTiming,
        merge: bool,
        condition: Option<AnimatorCondition>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnimatorCommand {
    Enter,
    Exit,
}

#[derive(Debug, Clone, Copy)]
enum AnimatorAction {
    EnterComplete,
    ExitComplete,
}

#[derive(Debug, Clone, Copy)]
struct ScheduledAction {
    due: Instant,
    action: ScheduledActionKind,
}

#[derive(Debug, Clone, Copy)]
enum ScheduledActionKind {
    SelfAction(AnimatorAction),
    SelfCommand(AnimatorCommand),
    ChildAction {
        id: AnimatorId,
        command: AnimatorCommand,
    },
}

struct AnimatorChild {
    id: AnimatorId,
    command_tx: UnboundedSender<AnimatorCommand>,
    state: AnimatorState,
    timing: AnimatorTiming,
    merge: bool,
    condition: Option<AnimatorCondition>,
}

#[derive(Clone, Copy)]
struct AnimatorChildSnapshot {
    id: AnimatorId,
    timing: AnimatorTiming,
}

/// Animator node with channel-based parent/child communication.
pub struct AnimatorNode {
    id: AnimatorId,
    state: AnimatorState,
    settings: AnimatorSettings,
    manager: AnimatorManagerKind,
    switch_active: Option<AnimatorId>,
    switch_pending: Option<AnimatorId>,
    parent_tx: Option<UnboundedSender<AnimatorMessage>>,
    children_tx: UnboundedSender<AnimatorMessage>,
    children_rx: UnboundedReceiver<AnimatorMessage>,
    command_rx: UnboundedReceiver<AnimatorCommand>,
    children: Vec<AnimatorChild>,
    scheduled: Vec<ScheduledAction>,
    subscribers: HashMap<usize, AnimatorSubscriber>,
    next_subscriber_id: usize,
    foreign: Option<Arc<dyn Any + Send + Sync>>,
}

impl AnimatorNode {
    /// Create a root animator node without a parent.
    pub fn new_root(settings: AnimatorSettings, manager: AnimatorManagerKind) -> Self {
        let (children_tx, children_rx) = unbounded_channel();
        let (_command_tx, command_rx) = unbounded_channel();

        Self {
            id: AnimatorId::next(),
            state: settings.initial_state,
            settings,
            manager,
            switch_active: None,
            switch_pending: None,
            parent_tx: None,
            children_tx,
            children_rx,
            command_rx,
            children: Vec::new(),
            scheduled: Vec::new(),
            subscribers: HashMap::new(),
            next_subscriber_id: 0,
            foreign: None,
        }
    }

    /// Add a child node connected to this parent.
    pub fn add_child(&mut self, settings: AnimatorSettings, manager: AnimatorManagerKind) -> Self {
        let child_id = AnimatorId::next();
        let (command_tx, command_rx) = unbounded_channel();
        let (children_tx, children_rx) = unbounded_channel();

        self.children.push(AnimatorChild {
            id: child_id,
            command_tx,
            state: settings.initial_state,
            timing: settings.timing,
            merge: settings.merge,
            condition: settings.condition.clone(),
        });

        Self {
            id: child_id,
            state: settings.initial_state,
            settings,
            manager,
            switch_active: None,
            switch_pending: None,
            parent_tx: Some(self.children_tx.clone()),
            children_tx,
            children_rx,
            command_rx,
            children: Vec::new(),
            scheduled: Vec::new(),
            subscribers: HashMap::new(),
            next_subscriber_id: 0,
            foreign: None,
        }
    }

    /// Node identifier.
    pub fn id(&self) -> AnimatorId {
        self.id
    }

    /// Current animator state.
    pub fn state(&self) -> AnimatorState {
        self.state
    }

    /// Current settings.
    pub fn settings(&self) -> AnimatorSettings {
        self.settings.clone()
    }

    /// Current manager strategy.
    pub fn manager(&self) -> AnimatorManagerKind {
        self.manager
    }

    /// Update animator settings using a partial update.
    pub fn update_settings(&mut self, update: AnimatorSettingsUpdate) {
        self.update_settings_at(update, Instant::now());
    }

    /// Update animator settings using a partial update at a specific time.
    pub fn update_settings_at(&mut self, update: AnimatorSettingsUpdate, now: Instant) {
        let mut notify_parent = false;
        let mut refresh_needed = false;
        let mut active_change = None;

        if let Some(timing_update) = update.timing {
            if let Some(enter) = timing_update.enter {
                self.settings.timing.enter = enter;
                notify_parent = true;
                refresh_needed = true;
            }
            if let Some(exit) = timing_update.exit {
                self.settings.timing.exit = exit;
                notify_parent = true;
                refresh_needed = true;
            }
            if let Some(delay) = timing_update.delay {
                self.settings.timing.delay = delay;
                notify_parent = true;
                refresh_needed = true;
            }
            if let Some(offset) = timing_update.offset {
                self.settings.timing.offset = offset;
                notify_parent = true;
                refresh_needed = true;
            }
        }

        if let Some(stagger) = update.stagger {
            self.settings.stagger = stagger;
            refresh_needed = true;
        }

        if let Some(active) = update.active {
            self.settings.active = active;
            active_change = Some(active);
        }

        if let Some(combine) = update.combine {
            self.settings.combine = combine;
            refresh_needed = true;
        }

        if let Some(merge) = update.merge {
            self.settings.merge = merge;
            notify_parent = true;
            refresh_needed = true;
        }

        if let Some(initial_state) = update.initial_state {
            self.settings.initial_state = initial_state;
        }

        if let Some(condition_update) = update.condition {
            self.settings.condition = condition_update;
            notify_parent = true;
            refresh_needed = true;
        }

        if let Some(on_transition_update) = update.on_transition {
            self.settings.on_transition = on_transition_update;
        }

        if let Some(limit_update) = update.limit {
            self.settings.limit = limit_update;
            refresh_needed = true;
        }

        if let Some(manager) = update.manager
            && self.manager != manager
        {
            self.manager = manager;
            self.switch_active = None;
            self.switch_pending = None;
            refresh_needed = true;
        }

        if notify_parent {
            self.notify_parent_settings();
        }

        if let Some(active) = active_change {
            if active {
                self.enter_at(now);
            } else {
                self.exit_at(now);
            }
        }

        if refresh_needed {
            self.refresh_at(now);
        }
    }

    /// Set foreign value associated with this node.
    pub fn set_foreign<T>(&mut self, value: T)
    where
        T: Any + Send + Sync,
    {
        self.foreign = Some(Arc::new(value));
    }

    /// Clear foreign value.
    pub fn clear_foreign(&mut self) {
        self.foreign = None;
    }

    /// Read foreign value as a concrete type reference.
    pub fn foreign_as<T>(&self) -> Option<&T>
    where
        T: Any + Send + Sync,
    {
        self.foreign.as_deref()?.downcast_ref::<T>()
    }

    /// Clone foreign value as a concrete Arc when possible.
    pub fn foreign_arc<T>(&self) -> Option<Arc<T>>
    where
        T: Any + Send + Sync,
    {
        self.foreign.as_ref()?.clone().downcast::<T>().ok()
    }

    /// Return a child's last known state.
    pub fn child_state(&self, id: AnimatorId) -> Option<AnimatorState> {
        self.children
            .iter()
            .find(|child| child.id == id)
            .map(|child| child.state)
    }

    /// Subscribe to state changes for this node.
    pub fn subscribe(&mut self, subscriber: AnimatorSubscriber) -> AnimatorSubscription {
        let id = self.next_subscriber_id;
        self.next_subscriber_id += 1;
        self.subscribers.insert(id, subscriber.clone());
        subscriber(self.id, self.state);
        AnimatorSubscription(id)
    }

    /// Unsubscribe a previously registered subscriber.
    pub fn unsubscribe(&mut self, subscription: AnimatorSubscription) {
        self.subscribers.remove(&subscription.0);
    }

    /// Set active state and trigger enter/exit accordingly.
    pub fn set_active(&mut self, active: bool) {
        self.settings.active = active;
        if active {
            self.enter();
        } else {
            self.exit();
        }
    }

    /// Refresh child conditions and state transitions.
    pub fn refresh(&mut self) {
        self.refresh_at(Instant::now());
    }

    /// Refresh child conditions and state transitions at a specific time.
    pub fn refresh_at(&mut self, now: Instant) {
        let parent_entering = self.state == AnimatorState::Entering;
        let parent_entered = self.state == AnimatorState::Entered;
        if !(parent_entering || parent_entered) {
            return;
        }

        let mut actions = Vec::new();
        for child in &self.children {
            let allowed = self.child_allows(child);
            match child.state {
                AnimatorState::Entered | AnimatorState::Entering => {
                    if !allowed {
                        actions.push((child.id, AnimatorCommand::Exit));
                    }
                }
                AnimatorState::Exited | AnimatorState::Exiting => {
                    if allowed {
                        let should_enter = if parent_entering {
                            self.settings.combine || child.merge
                        } else {
                            !self.settings.combine && !child.merge
                        };
                        if should_enter {
                            actions.push((child.id, AnimatorCommand::Enter));
                        }
                    }
                }
            }
        }

        for (id, command) in actions {
            self.schedule_child_action(id, now, command);
        }
    }

    /// Trigger enter using the current time.
    pub fn enter(&mut self) {
        self.enter_at(Instant::now());
    }

    /// Trigger enter at a specific time.
    pub fn enter_at(&mut self, now: Instant) {
        self.cancel_self_commands();
        if self.settings.timing.delay > Duration::ZERO {
            self.schedule_self_command(now + self.settings.timing.delay, AnimatorCommand::Enter);
        } else {
            self.handle_command(AnimatorCommand::Enter, now);
        }
    }

    /// Trigger exit using the current time.
    pub fn exit(&mut self) {
        self.exit_at(Instant::now());
    }

    /// Trigger exit at a specific time.
    pub fn exit_at(&mut self, now: Instant) {
        self.cancel_self_commands();
        self.handle_command(AnimatorCommand::Exit, now);
    }

    /// Set the active child for switch managers.
    pub fn set_active_child(&mut self, id: AnimatorId) {
        self.set_active_child_at(id, Instant::now());
    }

    /// Set the active child for switch managers at a specific time.
    pub fn set_active_child_at(&mut self, id: AnimatorId, now: Instant) {
        if self.manager != AnimatorManagerKind::Switch {
            return;
        }

        let (allowed, due) = match self.children.iter().find(|child| child.id == id) {
            Some(child) => (
                self.child_allows(child),
                now + child.timing.delay + child.timing.offset,
            ),
            None => return,
        };
        if !allowed {
            return;
        }

        if self.switch_active == Some(id) && self.switch_pending.is_none() {
            self.schedule_child_action(id, due, AnimatorCommand::Enter);
            self.exit_other_children(id, now);
            return;
        }

        if self.switch_active.is_none() {
            self.switch_active = Some(id);
            self.schedule_child_action(id, due, AnimatorCommand::Enter);
            self.exit_other_children(id, now);
            return;
        }

        self.switch_pending = Some(id);
        if let Some(active) = self.switch_active
            && active != id
        {
            self.schedule_child_action(active, now, AnimatorCommand::Exit);
        }
        self.exit_other_children(id, now);
    }

    /// Advance the animator to the current time.
    pub fn tick(&mut self) {
        self.tick_at(Instant::now());
    }

    /// Advance the animator to a specific time.
    pub fn tick_at(&mut self, now: Instant) {
        self.drain_commands(now);
        self.drain_child_messages(now);
        self.process_scheduled(now);
    }

    fn drain_commands(&mut self, now: Instant) {
        loop {
            match self.command_rx.try_recv() {
                Ok(command) => self.handle_command(command, now),
                Err(_) => break,
            }
        }
    }

    fn drain_child_messages(&mut self, now: Instant) {
        loop {
            match self.children_rx.try_recv() {
                Ok(message) => self.handle_child_message(message, now),
                Err(_) => break,
            }
        }
    }

    fn process_scheduled(&mut self, now: Instant) {
        let mut ready = Vec::new();
        self.scheduled.retain(|action| {
            if action.due <= now {
                ready.push(*action);
                false
            } else {
                true
            }
        });

        for action in ready {
            match action.action {
                ScheduledActionKind::SelfAction(action) => self.handle_self_action(action, now),
                ScheduledActionKind::SelfCommand(command) => self.handle_command(command, now),
                ScheduledActionKind::ChildAction { id, command } => {
                    self.send_child_command(id, command);
                }
            }
        }
    }

    fn handle_command(&mut self, command: AnimatorCommand, now: Instant) {
        match command {
            AnimatorCommand::Enter => {
                if matches!(self.state, AnimatorState::Entered | AnimatorState::Entering) {
                    return;
                }
                self.cancel_self_actions();
                self.cancel_self_commands();
                self.set_state(AnimatorState::Entering);
                let enter_duration = self.enter_duration();
                self.schedule_self_action(now + enter_duration, AnimatorAction::EnterComplete);
                self.handle_parent_enter(now);
            }
            AnimatorCommand::Exit => {
                if matches!(self.state, AnimatorState::Exited | AnimatorState::Exiting) {
                    return;
                }
                self.cancel_self_actions();
                self.cancel_self_commands();
                self.set_state(AnimatorState::Exiting);
                self.schedule_self_action(
                    now + self.settings.timing.exit,
                    AnimatorAction::ExitComplete,
                );
                self.handle_parent_exit(now);
            }
        }
    }

    fn handle_self_action(&mut self, action: AnimatorAction, now: Instant) {
        match action {
            AnimatorAction::EnterComplete => {
                self.set_state(AnimatorState::Entered);
                self.handle_enter_complete(now);
            }
            AnimatorAction::ExitComplete => self.set_state(AnimatorState::Exited),
        }
    }

    fn handle_child_message(&mut self, message: AnimatorMessage, now: Instant) {
        match message {
            AnimatorMessage::StateChanged { id, state } => {
                if let Some(child) = self.children.iter_mut().find(|child| child.id == id) {
                    child.state = state;
                }

                if self.manager == AnimatorManagerKind::Switch {
                    self.handle_switch_child_state(id, state, now);
                }
            }
            AnimatorMessage::SettingsChanged {
                id,
                timing,
                merge,
                condition,
            } => {
                if let Some(child) = self.children.iter_mut().find(|child| child.id == id) {
                    child.timing = timing;
                    child.merge = merge;
                    child.condition = condition;
                }
            }
        }
    }

    fn handle_parent_enter(&mut self, now: Instant) {
        let children: Vec<AnimatorChildSnapshot> = self
            .children
            .iter()
            .filter(|child| self.child_allows(child) && (self.settings.combine || child.merge))
            .map(|child| AnimatorChildSnapshot {
                id: child.id,
                timing: child.timing,
            })
            .collect();
        if children.is_empty() {
            return;
        }

        self.schedule_children_enter(children, now);
    }

    fn handle_parent_exit(&mut self, now: Instant) {
        let children: Vec<AnimatorId> = self.children.iter().map(|child| child.id).collect();
        for id in children {
            self.schedule_child_action(id, now, AnimatorCommand::Exit);
        }
    }

    fn handle_switch_child_state(&mut self, id: AnimatorId, state: AnimatorState, now: Instant) {
        if state == AnimatorState::Exited && self.switch_active == Some(id) {
            self.switch_active = None;

            let children: Vec<AnimatorChildSnapshot> = self
                .children
                .iter()
                .filter(|child| self.child_allows(child))
                .map(|child| AnimatorChildSnapshot {
                    id: child.id,
                    timing: child.timing,
                })
                .collect();
            if let Some(child) = self.select_switch_child(&children) {
                self.schedule_child_action(child.id, now, AnimatorCommand::Enter);
                self.exit_other_children(child.id, now);
            }
        }
    }

    fn exit_other_children(&mut self, keep_id: AnimatorId, now: Instant) {
        let children: Vec<AnimatorId> = self
            .children
            .iter()
            .filter(|child| child.id != keep_id)
            .map(|child| child.id)
            .collect();
        for id in children {
            self.schedule_child_action(id, now, AnimatorCommand::Exit);
        }
    }

    fn schedule_self_action(&mut self, due: Instant, action: AnimatorAction) {
        self.scheduled.push(ScheduledAction {
            due,
            action: ScheduledActionKind::SelfAction(action),
        });
    }

    fn schedule_self_command(&mut self, due: Instant, command: AnimatorCommand) {
        self.scheduled.push(ScheduledAction {
            due,
            action: ScheduledActionKind::SelfCommand(command),
        });
    }

    fn schedule_child_action(&mut self, id: AnimatorId, due: Instant, command: AnimatorCommand) {
        self.cancel_child_actions(Some(id));
        self.scheduled.push(ScheduledAction {
            due,
            action: ScheduledActionKind::ChildAction { id, command },
        });
    }

    fn send_child_command(&self, id: AnimatorId, command: AnimatorCommand) {
        if let Some(child) = self.children.iter().find(|child| child.id == id) {
            if command == AnimatorCommand::Enter && !self.child_allows(child) {
                return;
            }
            let _ = child.command_tx.send(command);
        }
    }

    fn cancel_self_actions(&mut self) {
        self.scheduled
            .retain(|action| !matches!(action.action, ScheduledActionKind::SelfAction(_)));
    }

    fn cancel_self_commands(&mut self) {
        self.scheduled
            .retain(|action| !matches!(action.action, ScheduledActionKind::SelfCommand(_)));
    }

    fn cancel_child_actions(&mut self, id: Option<AnimatorId>) {
        self.scheduled.retain(|action| {
            !matches!(
                action.action,
                ScheduledActionKind::ChildAction {
                    id: child_id,
                    ..
                } if id.is_none_or(|target| target == child_id)
            )
        });
    }

    fn set_state(&mut self, state: AnimatorState) {
        if self.state == state {
            return;
        }
        self.state = state;
        if let Some(callback) = &self.settings.on_transition {
            callback(self.id, self.state);
        }
        for subscriber in self.subscribers.values() {
            subscriber(self.id, self.state);
        }
        if let Some(parent_tx) = &self.parent_tx {
            let _ = parent_tx.send(AnimatorMessage::StateChanged { id: self.id, state });
        }
    }

    fn notify_parent_settings(&self) {
        if let Some(parent_tx) = &self.parent_tx {
            let _ = parent_tx.send(AnimatorMessage::SettingsChanged {
                id: self.id,
                timing: self.settings.timing,
                merge: self.settings.merge,
                condition: self.settings.condition.clone(),
            });
        }
    }

    fn child_allows(&self, child: &AnimatorChild) -> bool {
        match &child.condition {
            Some(condition) => condition.allows(child.id),
            None => true,
        }
    }

    fn select_switch_child(
        &mut self,
        children: &[AnimatorChildSnapshot],
    ) -> Option<AnimatorChildSnapshot> {
        if let Some(pending) = self.switch_pending
            && let Some(child) = children.iter().find(|child| child.id == pending).copied()
        {
            self.switch_pending = None;
            self.switch_active = Some(child.id);
            return Some(child);
        }

        if let Some(active) = self.switch_active
            && let Some(child) = children.iter().find(|child| child.id == active).copied()
        {
            return Some(child);
        }

        let child = children.first().copied();
        if let Some(child) = child {
            self.switch_active = Some(child.id);
        }
        child
    }

    fn enter_duration(&self) -> Duration {
        let base = self.settings.timing.enter;
        if !self.settings.combine {
            return base;
        }

        let children: Vec<&AnimatorChild> = self
            .children
            .iter()
            .filter(|child| self.child_allows(child))
            .collect();
        if children.is_empty() {
            return base;
        }

        let duration = match self.manager {
            AnimatorManagerKind::Parallel => children
                .iter()
                .map(|child| child.timing.delay + child.timing.offset + child.timing.enter)
                .max()
                .unwrap_or(Duration::ZERO),
            AnimatorManagerKind::Stagger | AnimatorManagerKind::StaggerReverse => {
                let mut ordered = children.clone();
                if self.manager == AnimatorManagerKind::StaggerReverse {
                    ordered.reverse();
                }
                let mut max = Duration::ZERO;
                for (index, child) in ordered.iter().enumerate() {
                    let mut stagger_offset = scaled_duration(self.settings.stagger, index);
                    if let Some(limit) = self.settings.limit
                        && limit > 0.0
                    {
                        let limit_duration =
                            Duration::from_secs_f32(self.settings.stagger.as_secs_f32() * limit);
                        if stagger_offset > limit_duration {
                            stagger_offset = limit_duration;
                        }
                    }
                    let total = stagger_offset
                        + child.timing.delay
                        + child.timing.offset
                        + child.timing.enter;
                    if total > max {
                        max = total;
                    }
                }
                max
            }
            AnimatorManagerKind::Sequence | AnimatorManagerKind::SequenceReverse => {
                let mut ordered = children.clone();
                if self.manager == AnimatorManagerKind::SequenceReverse {
                    ordered.reverse();
                }
                if let Some(limit) = self.settings.limit {
                    if limit > 0.0 {
                        let slots = limit.max(1.0).round() as usize;
                        let mut lane_times = vec![Duration::ZERO; slots];
                        let mut total = Duration::ZERO;
                        for (index, child) in ordered.iter().enumerate() {
                            let lane = index % slots;
                            let start = lane_times[lane] + child.timing.offset;
                            let end = start + child.timing.enter;
                            lane_times[lane] = end;
                            let candidate = end + child.timing.delay;
                            if candidate > total {
                                total = candidate;
                            }
                        }
                        total
                    } else {
                        sequence_duration(&ordered)
                    }
                } else {
                    sequence_duration(&ordered)
                }
            }
            AnimatorManagerKind::Switch => {
                if let Some(active) = self.switch_active {
                    if let Some(child) = children.iter().find(|child| child.id == active) {
                        child.timing.delay + child.timing.offset + child.timing.enter
                    } else if let Some(child) = children.first() {
                        child.timing.delay + child.timing.offset + child.timing.enter
                    } else {
                        Duration::ZERO
                    }
                } else {
                    children
                        .first()
                        .map(|child| child.timing.delay + child.timing.offset + child.timing.enter)
                        .unwrap_or(Duration::ZERO)
                }
            }
        };

        base.max(duration)
    }

    fn handle_enter_complete(&mut self, now: Instant) {
        if self.settings.combine {
            return;
        }

        let children: Vec<AnimatorChildSnapshot> = self
            .children
            .iter()
            .filter(|child| !child.merge && self.child_allows(child))
            .map(|child| AnimatorChildSnapshot {
                id: child.id,
                timing: child.timing,
            })
            .collect();
        self.schedule_children_enter(children, now);
    }

    fn schedule_children_enter(&mut self, children: Vec<AnimatorChildSnapshot>, now: Instant) {
        match self.manager {
            AnimatorManagerKind::Parallel => {
                for child in children {
                    let due = now + child.timing.delay + child.timing.offset;
                    self.schedule_child_action(child.id, due, AnimatorCommand::Enter);
                }
            }
            AnimatorManagerKind::Stagger | AnimatorManagerKind::StaggerReverse => {
                let mut ordered = children;
                if self.manager == AnimatorManagerKind::StaggerReverse {
                    ordered.reverse();
                }
                for (index, child) in ordered.iter().enumerate() {
                    let mut stagger_offset = scaled_duration(self.settings.stagger, index);
                    if let Some(limit) = self.settings.limit
                        && limit > 0.0
                    {
                        let limit_duration =
                            Duration::from_secs_f32(self.settings.stagger.as_secs_f32() * limit);
                        if stagger_offset > limit_duration {
                            stagger_offset = limit_duration;
                        }
                    }
                    let due = now + stagger_offset + child.timing.delay + child.timing.offset;
                    self.schedule_child_action(child.id, due, AnimatorCommand::Enter);
                }
            }
            AnimatorManagerKind::Sequence | AnimatorManagerKind::SequenceReverse => {
                let mut ordered = children;
                if self.manager == AnimatorManagerKind::SequenceReverse {
                    ordered.reverse();
                }
                let limit = self.settings.limit.filter(|limit| *limit > 0.0);
                if let Some(limit) = limit {
                    let slots = limit.max(1.0).round() as usize;
                    let mut lane_times = vec![now; slots];
                    for (index, child) in ordered.iter().enumerate() {
                        let lane = index % slots;
                        let mut start = lane_times[lane] + child.timing.offset;
                        let due = start + child.timing.delay;
                        self.schedule_child_action(child.id, due, AnimatorCommand::Enter);
                        start += child.timing.enter;
                        lane_times[lane] = start;
                    }
                } else {
                    let mut cursor = now;
                    for child in ordered {
                        cursor += child.timing.offset;
                        let due = cursor + child.timing.delay;
                        self.schedule_child_action(child.id, due, AnimatorCommand::Enter);
                        cursor += child.timing.enter;
                    }
                }
            }
            AnimatorManagerKind::Switch => {
                if let Some(child) = self.select_switch_child(&children) {
                    let due = now + child.timing.delay + child.timing.offset;
                    self.schedule_child_action(child.id, due, AnimatorCommand::Enter);
                    self.exit_other_children(child.id, now);
                }
            }
        }
    }
}

fn scaled_duration(duration: Duration, factor: usize) -> Duration {
    if factor == 0 {
        Duration::ZERO
    } else {
        Duration::from_secs_f32(duration.as_secs_f32() * factor as f32)
    }
}

fn sequence_duration(children: &[&AnimatorChild]) -> Duration {
    let mut cursor = Duration::ZERO;
    let mut total = Duration::ZERO;
    for child in children {
        cursor += child.timing.offset + child.timing.enter;
        let candidate = cursor + child.timing.delay;
        if candidate > total {
            total = candidate;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animator_state_transitions() {
        let settings = AnimatorSettings::new(
            AnimatorTiming::new(
                Duration::from_millis(10),
                Duration::from_millis(10),
                Duration::ZERO,
            ),
            Duration::ZERO,
        );
        let mut node = AnimatorNode::new_root(settings, AnimatorManagerKind::Parallel);
        let base = Instant::now();

        node.enter_at(base);
        assert_eq!(node.state(), AnimatorState::Entering);

        node.tick_at(base + Duration::from_millis(10));
        assert_eq!(node.state(), AnimatorState::Entered);

        node.exit_at(base + Duration::from_millis(20));
        assert_eq!(node.state(), AnimatorState::Exiting);

        node.tick_at(base + Duration::from_millis(30));
        assert_eq!(node.state(), AnimatorState::Exited);
    }

    #[test]
    fn test_animator_parallel_manager() {
        let settings = AnimatorSettings::new(
            AnimatorTiming::new(
                Duration::from_millis(5),
                Duration::from_millis(5),
                Duration::ZERO,
            ),
            Duration::ZERO,
        );
        let mut root = AnimatorNode::new_root(settings.clone(), AnimatorManagerKind::Parallel);
        let child1 = root.add_child(settings.clone(), AnimatorManagerKind::Parallel);
        let child2 = root.add_child(settings, AnimatorManagerKind::Parallel);
        let mut child1 = child1;
        let mut child2 = child2;
        let base = Instant::now();

        root.enter_at(base);
        root.tick_at(base);
        child1.tick_at(base);
        child2.tick_at(base);

        assert_eq!(child1.state(), AnimatorState::Exited);
        assert_eq!(child2.state(), AnimatorState::Exited);

        let t1 = base + Duration::from_millis(5);
        root.tick_at(t1);
        root.tick_at(t1);
        child1.tick_at(t1);
        child2.tick_at(t1);
        assert_eq!(child1.state(), AnimatorState::Entering);
        assert_eq!(child2.state(), AnimatorState::Entering);

        let t2 = t1 + Duration::from_millis(5);
        child1.tick_at(t2);
        child2.tick_at(t2);
        root.tick_at(t2);

        assert_eq!(
            root.child_state(child1.id()).unwrap(),
            AnimatorState::Entered
        );
        assert_eq!(
            root.child_state(child2.id()).unwrap(),
            AnimatorState::Entered
        );
    }

    #[test]
    fn test_animator_stagger_manager() {
        let settings = AnimatorSettings::new(
            AnimatorTiming::new(
                Duration::from_millis(5),
                Duration::from_millis(5),
                Duration::ZERO,
            ),
            Duration::from_millis(10),
        );
        let mut root = AnimatorNode::new_root(settings.clone(), AnimatorManagerKind::Stagger);
        let mut child1 = root.add_child(settings.clone(), AnimatorManagerKind::Parallel);
        let mut child2 = root.add_child(settings.clone(), AnimatorManagerKind::Parallel);
        let mut child3 = root.add_child(settings, AnimatorManagerKind::Parallel);
        let base = Instant::now();

        root.enter_at(base);
        root.tick_at(base);
        child1.tick_at(base);
        child2.tick_at(base);
        child3.tick_at(base);

        assert_eq!(child1.state(), AnimatorState::Exited);
        assert_eq!(child2.state(), AnimatorState::Exited);
        assert_eq!(child3.state(), AnimatorState::Exited);

        let t0 = base + Duration::from_millis(5);
        root.tick_at(t0);
        root.tick_at(t0);
        child1.tick_at(t0);
        assert_eq!(child1.state(), AnimatorState::Entering);

        let t1 = t0 + Duration::from_millis(10);
        root.tick_at(t1);
        child2.tick_at(t1);
        assert_eq!(child2.state(), AnimatorState::Entering);

        let t2 = t0 + Duration::from_millis(20);
        root.tick_at(t2);
        child3.tick_at(t2);
        assert_eq!(child3.state(), AnimatorState::Entering);
    }

    #[test]
    fn test_animator_sequence_manager() {
        let settings = AnimatorSettings::new(
            AnimatorTiming::new(
                Duration::from_millis(10),
                Duration::from_millis(10),
                Duration::ZERO,
            ),
            Duration::from_millis(5),
        );
        let mut root = AnimatorNode::new_root(settings.clone(), AnimatorManagerKind::Sequence);
        let mut child1 = root.add_child(settings.clone(), AnimatorManagerKind::Parallel);
        let mut child2 = root.add_child(settings, AnimatorManagerKind::Parallel);
        let base = Instant::now();

        root.enter_at(base);
        root.tick_at(base);
        child1.tick_at(base);
        child2.tick_at(base);

        assert_eq!(child1.state(), AnimatorState::Exited);
        assert_eq!(child2.state(), AnimatorState::Exited);

        let t0 = base + Duration::from_millis(10);
        root.tick_at(t0);
        root.tick_at(t0);
        child1.tick_at(t0);
        assert_eq!(child1.state(), AnimatorState::Entering);

        let t1 = t0 + Duration::from_millis(10);
        root.tick_at(t1);
        child2.tick_at(t1);
        assert_eq!(child2.state(), AnimatorState::Entering);
    }

    #[test]
    fn test_animator_switch_manager() {
        let settings = AnimatorSettings::new(
            AnimatorTiming::new(
                Duration::from_millis(5),
                Duration::from_millis(5),
                Duration::ZERO,
            ),
            Duration::ZERO,
        );
        let mut root = AnimatorNode::new_root(settings.clone(), AnimatorManagerKind::Switch);
        let mut child1 = root.add_child(settings.clone(), AnimatorManagerKind::Parallel);
        let mut child2 = root.add_child(settings, AnimatorManagerKind::Parallel);
        let base = Instant::now();

        root.enter_at(base);
        root.tick_at(base);
        child1.tick_at(base);
        child2.tick_at(base);
        assert_eq!(child1.state(), AnimatorState::Exited);
        assert_eq!(child2.state(), AnimatorState::Exited);

        let t0 = base + Duration::from_millis(5);
        root.tick_at(t0);
        root.tick_at(t0);
        child1.tick_at(t0);
        assert_eq!(child1.state(), AnimatorState::Entering);

        let switch_time = t0 + Duration::from_millis(5);
        root.set_active_child_at(child2.id(), switch_time);
        root.tick_at(switch_time);
        child1.tick_at(switch_time);
        assert_eq!(child1.state(), AnimatorState::Exiting);

        let exit_time = switch_time + Duration::from_millis(5);
        child1.tick_at(exit_time);
        root.tick_at(exit_time);
        child2.tick_at(exit_time);
        assert_eq!(child2.state(), AnimatorState::Entering);
    }

    #[test]
    fn test_animator_merge_enters_with_parent() {
        let mut root =
            AnimatorNode::new_root(AnimatorSettings::default(), AnimatorManagerKind::Parallel);
        let child_settings = AnimatorSettings::default().merge(true);
        let mut child = root.add_child(child_settings, AnimatorManagerKind::Parallel);
        let base = Instant::now();

        root.enter_at(base);
        root.tick_at(base);
        child.tick_at(base);
        assert_eq!(child.state(), AnimatorState::Entering);
    }
}
