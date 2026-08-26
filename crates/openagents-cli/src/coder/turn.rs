//! Coder's local turn lifecycle.
//!
//! The terminal loop asks this reducer what an input means. It does not infer
//! cancellation from loading flags or from whichever task happens to exist.
//! Every runtime event carries the [`TurnId`] that produced it, and the
//! reducer rejects events as soon as cancellation starts.

/// A process-local identity for one submitted Coder turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TurnId(u64);

impl TurnId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// The local phase of the current turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnPhase {
    Idle,
    Active(TurnId),
    Canceling(TurnId),
}

/// A typed request to change the turn lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnAction {
    Start,
    RequestCancel,
    ObserveTerminal(TurnId),
    CompleteCancel(TurnId),
}

/// What the owner of the runtime task must do after applying an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnEffect {
    None,
    Started(TurnId),
    AbortTransport(TurnId),
    ReturnedIdle(TurnId),
}

/// The reducer that owns the active turn and its generation fence.
#[derive(Debug, Clone)]
pub struct TurnState {
    next: u64,
    phase: TurnPhase,
}

impl Default for TurnState {
    fn default() -> Self {
        Self {
            next: 1,
            phase: TurnPhase::Idle,
        }
    }
}

impl TurnState {
    pub fn phase(&self) -> TurnPhase {
        self.phase
    }

    /// Apply one lifecycle action.
    ///
    /// Repeated cancellation and late terminal events return [`TurnEffect::None`].
    pub fn apply(&mut self, action: TurnAction) -> TurnEffect {
        match (self.phase, action) {
            (TurnPhase::Idle, TurnAction::Start) => {
                let id = TurnId::new(self.next);
                self.next = self.next.saturating_add(1);
                self.phase = TurnPhase::Active(id);
                TurnEffect::Started(id)
            }
            (TurnPhase::Active(id), TurnAction::RequestCancel) => {
                self.phase = TurnPhase::Canceling(id);
                TurnEffect::AbortTransport(id)
            }
            (TurnPhase::Active(active), TurnAction::ObserveTerminal(seen)) if active == seen => {
                self.phase = TurnPhase::Idle;
                TurnEffect::ReturnedIdle(seen)
            }
            (TurnPhase::Canceling(active), TurnAction::CompleteCancel(seen)) if active == seen => {
                self.phase = TurnPhase::Idle;
                TurnEffect::ReturnedIdle(seen)
            }
            _ => TurnEffect::None,
        }
    }

    /// Whether an event may still mutate the frame.
    ///
    /// Cancellation closes the fence immediately. The cancellation completion
    /// action is the only accepted input after that transition.
    pub fn accepts(&self, id: TurnId) -> bool {
        self.phase == TurnPhase::Active(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_closes_the_generation_fence_immediately() {
        let mut state = TurnState::default();
        let TurnEffect::Started(first) = state.apply(TurnAction::Start) else {
            panic!("the first turn did not start");
        };

        assert_eq!(
            state.apply(TurnAction::RequestCancel),
            TurnEffect::AbortTransport(first)
        );
        assert!(!state.accepts(first));
        assert_eq!(
            state.apply(TurnAction::ObserveTerminal(first)),
            TurnEffect::None
        );
        assert_eq!(state.apply(TurnAction::RequestCancel), TurnEffect::None);
        assert_eq!(
            state.apply(TurnAction::CompleteCancel(first)),
            TurnEffect::ReturnedIdle(first)
        );
    }

    #[test]
    fn a_late_event_cannot_cross_into_the_next_turn() {
        let mut state = TurnState::default();
        let TurnEffect::Started(first) = state.apply(TurnAction::Start) else {
            panic!("the first turn did not start");
        };
        state.apply(TurnAction::RequestCancel);
        state.apply(TurnAction::CompleteCancel(first));

        let TurnEffect::Started(second) = state.apply(TurnAction::Start) else {
            panic!("the second turn did not start");
        };
        assert_ne!(first, second);
        assert!(!state.accepts(first));
        assert!(state.accepts(second));
    }

    #[test]
    fn normal_completion_returns_the_reducer_to_idle() {
        let mut state = TurnState::default();
        let TurnEffect::Started(id) = state.apply(TurnAction::Start) else {
            panic!("the turn did not start");
        };
        assert_eq!(
            state.apply(TurnAction::ObserveTerminal(id)),
            TurnEffect::ReturnedIdle(id)
        );
        assert_eq!(state.phase(), TurnPhase::Idle);
    }
}
