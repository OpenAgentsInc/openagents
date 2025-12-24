use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimatorState {
    Entered,
    Entering,
    Exiting,
    Exited,
}

impl Default for AnimatorState {
    fn default() -> Self {
        AnimatorState::Exited
    }
}

/// Orchestration strategy for child nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimatorManagerKind {
    Parallel,
    Stagger,
    Sequence,
    Switch,
}

impl Default for AnimatorManagerKind {
    fn default() -> Self {
        AnimatorManagerKind::Parallel
    }
}

/// Timing configuration for a single node.
#[derive(Debug, Clone, Copy)]
pub struct AnimatorTiming {
    pub enter: Duration,
    pub exit: Duration,
    pub delay: Duration,
}

impl AnimatorTiming {
    pub const fn new(enter: Duration, exit: Duration, delay: Duration) -> Self {
        Self { enter, exit, delay }
    }
}

impl Default for AnimatorTiming {
    fn default() -> Self {
        Self {
            enter: Duration::from_millis(200),
            exit: Duration::from_millis(200),
            delay: Duration::ZERO,
        }
    }
}

/// Node settings used by managers and timing.
#[derive(Debug, Clone, Copy)]
pub struct AnimatorSettings {
    pub timing: AnimatorTiming,
    pub stagger: Duration,
}

impl AnimatorSettings {
    pub const fn new(timing: AnimatorTiming, stagger: Duration) -> Self {
        Self { timing, stagger }
    }
}

impl Default for AnimatorSettings {
    fn default() -> Self {
        Self {
            timing: AnimatorTiming::default(),
            stagger: Duration::from_millis(50),
        }
    }
}

/// Messages sent from child nodes to a parent.
#[derive(Debug, Clone, Copy)]
pub enum AnimatorMessage {
    StateChanged { id: AnimatorId, state: AnimatorState },
}

#[derive(Debug, Clone, Copy)]
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
    ChildAction { id: AnimatorId, command: AnimatorCommand },
}

#[derive(Debug)]
struct AnimatorChild {
    id: AnimatorId,
    command_tx: UnboundedSender<AnimatorCommand>,
    state: AnimatorState,
    timing: AnimatorTiming,
}

/// Animator node with channel-based parent/child communication.
#[derive(Debug)]
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
}

impl AnimatorNode {
    /// Create a root animator node without a parent.
    pub fn new_root(settings: AnimatorSettings, manager: AnimatorManagerKind) -> Self {
        let (children_tx, children_rx) = unbounded_channel();
        let (_command_tx, command_rx) = unbounded_channel();

        Self {
            id: AnimatorId::next(),
            state: AnimatorState::Exited,
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
            state: AnimatorState::Exited,
            timing: settings.timing,
        });

        Self {
            id: child_id,
            state: AnimatorState::Exited,
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
        self.settings
    }

    /// Return a child's last known state.
    pub fn child_state(&self, id: AnimatorId) -> Option<AnimatorState> {
        self.children.iter().find(|child| child.id == id).map(|child| child.state)
    }

    /// Trigger enter using the current time.
    pub fn enter(&mut self) {
        self.enter_at(Instant::now());
    }

    /// Trigger enter at a specific time.
    pub fn enter_at(&mut self, now: Instant) {
        self.handle_command(AnimatorCommand::Enter, now);
    }

    /// Trigger exit using the current time.
    pub fn exit(&mut self) {
        self.exit_at(Instant::now());
    }

    /// Trigger exit at a specific time.
    pub fn exit_at(&mut self, now: Instant) {
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

        if self.switch_active == Some(id) && self.switch_pending.is_none() {
            self.schedule_child_action(id, now, AnimatorCommand::Enter);
            self.exit_other_children(id, now);
            return;
        }

        if self.switch_active.is_none() {
            self.switch_active = Some(id);
            self.schedule_child_action(id, now, AnimatorCommand::Enter);
            self.exit_other_children(id, now);
            return;
        }

        self.switch_pending = Some(id);
        if let Some(active) = self.switch_active {
            if active != id {
                self.schedule_child_action(active, now, AnimatorCommand::Exit);
            }
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
                self.set_state(AnimatorState::Entering);
                self.schedule_self_action(
                    now + self.settings.timing.enter,
                    AnimatorAction::EnterComplete,
                );
                self.handle_parent_enter(now);
            }
            AnimatorCommand::Exit => {
                if matches!(self.state, AnimatorState::Exited | AnimatorState::Exiting) {
                    return;
                }
                self.cancel_self_actions();
                self.set_state(AnimatorState::Exiting);
                self.schedule_self_action(
                    now + self.settings.timing.exit,
                    AnimatorAction::ExitComplete,
                );
                self.handle_parent_exit(now);
            }
        }
    }

    fn handle_self_action(&mut self, action: AnimatorAction, _now: Instant) {
        match action {
            AnimatorAction::EnterComplete => self.set_state(AnimatorState::Entered),
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
        }
    }

    fn handle_parent_enter(&mut self, now: Instant) {
        match self.manager {
            AnimatorManagerKind::Parallel => {
                let children: Vec<(AnimatorId, AnimatorTiming)> = self
                    .children
                    .iter()
                    .map(|child| (child.id, child.timing))
                    .collect();
                for (id, timing) in children {
                    self.schedule_child_action(id, now + timing.delay, AnimatorCommand::Enter);
                }
            }
            AnimatorManagerKind::Stagger => {
                let stagger = self.settings.stagger;
                let children: Vec<(AnimatorId, AnimatorTiming)> = self
                    .children
                    .iter()
                    .map(|child| (child.id, child.timing))
                    .collect();
                for (index, (id, timing)) in children.iter().enumerate() {
                    let offset = scaled_duration(stagger, index);
                    self.schedule_child_action(
                        *id,
                        now + offset + timing.delay,
                        AnimatorCommand::Enter,
                    );
                }
            }
            AnimatorManagerKind::Sequence => {
                let mut cursor = now;
                let children: Vec<(AnimatorId, AnimatorTiming)> = self
                    .children
                    .iter()
                    .map(|child| (child.id, child.timing))
                    .collect();
                for (id, timing) in children {
                    cursor += timing.delay;
                    self.schedule_child_action(id, cursor, AnimatorCommand::Enter);
                    cursor += timing.enter + self.settings.stagger;
                }
            }
            AnimatorManagerKind::Switch => {
                let first_child = self.children.first().map(|child| child.id);
                let active = self.switch_active.or(first_child);
                if let Some(active_id) = active {
                    self.switch_active = Some(active_id);
                    self.switch_pending = None;
                    self.schedule_child_action(active_id, now, AnimatorCommand::Enter);
                    self.exit_other_children(active_id, now);
                }
            }
        }
    }

    fn handle_parent_exit(&mut self, now: Instant) {
        let children: Vec<AnimatorId> = self.children.iter().map(|child| child.id).collect();
        for id in children {
            self.schedule_child_action(id, now, AnimatorCommand::Exit);
        }
    }

    fn handle_switch_child_state(&mut self, id: AnimatorId, state: AnimatorState, now: Instant) {
        if state == AnimatorState::Exited && self.switch_active == Some(id) {
            if let Some(pending) = self.switch_pending.take() {
                self.switch_active = Some(pending);
                self.schedule_child_action(pending, now, AnimatorCommand::Enter);
                self.exit_other_children(pending, now);
            } else {
                self.switch_active = None;
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

    fn schedule_child_action(&mut self, id: AnimatorId, due: Instant, command: AnimatorCommand) {
        self.scheduled.push(ScheduledAction {
            due,
            action: ScheduledActionKind::ChildAction { id, command },
        });
    }

    fn send_child_command(&self, id: AnimatorId, command: AnimatorCommand) {
        if let Some(child) = self.children.iter().find(|child| child.id == id) {
            let _ = child.command_tx.send(command);
        }
    }

    fn cancel_self_actions(&mut self) {
        self.scheduled.retain(|action| !matches!(action.action, ScheduledActionKind::SelfAction(_)));
    }

    fn set_state(&mut self, state: AnimatorState) {
        if self.state == state {
            return;
        }
        self.state = state;
        if let Some(parent_tx) = &self.parent_tx {
            let _ = parent_tx.send(AnimatorMessage::StateChanged { id: self.id, state });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animator_state_transitions() {
        let settings = AnimatorSettings::new(
            AnimatorTiming::new(Duration::from_millis(10), Duration::from_millis(10), Duration::ZERO),
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
            AnimatorTiming::new(Duration::from_millis(5), Duration::from_millis(5), Duration::ZERO),
            Duration::ZERO,
        );
        let mut root = AnimatorNode::new_root(settings, AnimatorManagerKind::Parallel);
        let child1 = root.add_child(settings, AnimatorManagerKind::Parallel);
        let child2 = root.add_child(settings, AnimatorManagerKind::Parallel);
        let mut child1 = child1;
        let mut child2 = child2;
        let base = Instant::now();

        root.enter_at(base);
        root.tick_at(base);
        child1.tick_at(base);
        child2.tick_at(base);

        assert_eq!(child1.state(), AnimatorState::Entering);
        assert_eq!(child2.state(), AnimatorState::Entering);

        child1.tick_at(base + Duration::from_millis(5));
        child2.tick_at(base + Duration::from_millis(5));
        root.tick_at(base + Duration::from_millis(5));

        assert_eq!(root.child_state(child1.id()).unwrap(), AnimatorState::Entered);
        assert_eq!(root.child_state(child2.id()).unwrap(), AnimatorState::Entered);
    }

    #[test]
    fn test_animator_stagger_manager() {
        let settings = AnimatorSettings::new(
            AnimatorTiming::new(Duration::from_millis(5), Duration::from_millis(5), Duration::ZERO),
            Duration::from_millis(10),
        );
        let mut root = AnimatorNode::new_root(settings, AnimatorManagerKind::Stagger);
        let mut child1 = root.add_child(settings, AnimatorManagerKind::Parallel);
        let mut child2 = root.add_child(settings, AnimatorManagerKind::Parallel);
        let mut child3 = root.add_child(settings, AnimatorManagerKind::Parallel);
        let base = Instant::now();

        root.enter_at(base);
        root.tick_at(base);
        child1.tick_at(base);
        child2.tick_at(base);
        child3.tick_at(base);

        assert_eq!(child1.state(), AnimatorState::Entering);
        assert_eq!(child2.state(), AnimatorState::Exited);
        assert_eq!(child3.state(), AnimatorState::Exited);

        let t1 = base + Duration::from_millis(10);
        root.tick_at(t1);
        child2.tick_at(t1);
        assert_eq!(child2.state(), AnimatorState::Entering);

        let t2 = base + Duration::from_millis(20);
        root.tick_at(t2);
        child3.tick_at(t2);
        assert_eq!(child3.state(), AnimatorState::Entering);
    }

    #[test]
    fn test_animator_sequence_manager() {
        let settings = AnimatorSettings::new(
            AnimatorTiming::new(Duration::from_millis(10), Duration::from_millis(10), Duration::ZERO),
            Duration::from_millis(5),
        );
        let mut root = AnimatorNode::new_root(settings, AnimatorManagerKind::Sequence);
        let mut child1 = root.add_child(settings, AnimatorManagerKind::Parallel);
        let mut child2 = root.add_child(settings, AnimatorManagerKind::Parallel);
        let base = Instant::now();

        root.enter_at(base);
        root.tick_at(base);
        child1.tick_at(base);
        child2.tick_at(base);

        assert_eq!(child1.state(), AnimatorState::Entering);
        assert_eq!(child2.state(), AnimatorState::Exited);

        let t1 = base + Duration::from_millis(15);
        root.tick_at(t1);
        child2.tick_at(t1);
        assert_eq!(child2.state(), AnimatorState::Entering);
    }

    #[test]
    fn test_animator_switch_manager() {
        let settings = AnimatorSettings::new(
            AnimatorTiming::new(Duration::from_millis(5), Duration::from_millis(5), Duration::ZERO),
            Duration::ZERO,
        );
        let mut root = AnimatorNode::new_root(settings, AnimatorManagerKind::Switch);
        let mut child1 = root.add_child(settings, AnimatorManagerKind::Parallel);
        let mut child2 = root.add_child(settings, AnimatorManagerKind::Parallel);
        let base = Instant::now();

        root.enter_at(base);
        root.tick_at(base);
        child1.tick_at(base);
        child2.tick_at(base);
        assert_eq!(child1.state(), AnimatorState::Entering);
        assert_eq!(child2.state(), AnimatorState::Exited);

        let switch_time = base + Duration::from_millis(10);
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
}
