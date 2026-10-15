//! Local draft, composer, and scroll interaction for server-rendered chats.
//! HTMX owns requests and streams; the server owns chat and request identity.

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(target_arch = "wasm32")]
pub use browser::start;

/// What Enter in the chat's text box does here (#11685).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnterAction {
    /// The chat is free: send now.
    Send,
    /// Ctrl+Enter (Cmd+Enter) queues for later, and Enter while a send is in
    /// flight or an answer is being written goes first in line: both are the
    /// page shell's queue, so the adapter leaves the key to it.
    LeaveToQueue,
}

/// [`EnterAction`] for Enter with `later` (Ctrl or Cmd held) while
/// `sending` (a send in flight or an answer being written).
#[must_use]
pub fn action_for_enter(later: bool, sending: bool) -> EnterAction {
    if later || sending {
        EnterAction::LeaveToQueue
    } else {
        EnterAction::Send
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_sends_when_free_and_leaves_queueing_to_the_shell() {
        assert_eq!(action_for_enter(false, false), EnterAction::Send);
        assert_eq!(action_for_enter(true, false), EnterAction::LeaveToQueue);
        assert_eq!(action_for_enter(false, true), EnterAction::LeaveToQueue);
        assert_eq!(action_for_enter(true, true), EnterAction::LeaveToQueue);
    }
}
