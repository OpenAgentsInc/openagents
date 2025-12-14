//! Application state management.

use coder_domain::ids::ThreadId;
use coder_domain::{ChatView, DomainEvent};
use coder_ui_runtime::Signal;
use std::collections::HashMap;

/// Global application state.
pub struct AppState {
    /// Active chat threads.
    threads: HashMap<ThreadId, Signal<ChatView>>,

    /// Current active thread.
    active_thread: Option<ThreadId>,

    /// Whether the app is connected to the backend.
    connected: Signal<bool>,

    /// Pending events to process.
    pending_events: Vec<DomainEvent>,
}

impl AppState {
    /// Create new application state.
    pub fn new() -> Self {
        Self {
            threads: HashMap::new(),
            active_thread: None,
            connected: Signal::new(false),
            pending_events: Vec::new(),
        }
    }

    /// Get or create a chat view for a thread.
    pub fn get_or_create_thread(&mut self, thread_id: ThreadId) -> Signal<ChatView> {
        self.threads
            .entry(thread_id)
            .or_insert_with(|| Signal::new(ChatView::new(thread_id)))
            .clone()
    }

    /// Get a chat view for a thread.
    pub fn get_thread(&self, thread_id: &ThreadId) -> Option<Signal<ChatView>> {
        self.threads.get(thread_id).cloned()
    }

    /// Set the active thread.
    pub fn set_active_thread(&mut self, thread_id: Option<ThreadId>) {
        self.active_thread = thread_id;
    }

    /// Get the active thread.
    pub fn active_thread(&self) -> Option<ThreadId> {
        self.active_thread
    }

    /// Get the active thread's chat view.
    pub fn active_chat_view(&self) -> Option<Signal<ChatView>> {
        self.active_thread
            .as_ref()
            .and_then(|id| self.threads.get(id).cloned())
    }

    /// Set connection status.
    pub fn set_connected(&mut self, connected: bool) {
        self.connected.set(connected);
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        self.connected.get_untracked()
    }

    /// Get connection signal.
    pub fn connected_signal(&self) -> &Signal<bool> {
        &self.connected
    }

    /// Queue a domain event for processing.
    pub fn queue_event(&mut self, event: DomainEvent) {
        self.pending_events.push(event);
    }

    /// Process all pending events.
    pub fn process_events(&mut self) {
        let events = std::mem::take(&mut self.pending_events);

        for event in events {
            self.apply_event(&event);
        }
    }

    /// Apply a single domain event.
    fn apply_event(&mut self, event: &DomainEvent) {
        // Get thread ID from event
        let thread_id = match event {
            DomainEvent::ThreadCreated { thread_id, .. } => Some(*thread_id),
            DomainEvent::MessageAdded { thread_id, .. } => Some(*thread_id),
            DomainEvent::MessageStreaming { thread_id, .. } => Some(*thread_id),
            DomainEvent::MessageComplete { thread_id, .. } => Some(*thread_id),
            DomainEvent::ToolUseStarted { thread_id, .. } => Some(*thread_id),
            DomainEvent::ToolUseComplete { thread_id, .. } => Some(*thread_id),
            _ => None,
        };

        // Apply to the appropriate thread's chat view
        if let Some(thread_id) = thread_id {
            if let Some(view_signal) = self.threads.get(&thread_id) {
                view_signal.update(|view| {
                    view.apply(event);
                });
            }
        }
    }

    /// Create a new thread and return its ID.
    pub fn create_thread(&mut self) -> ThreadId {
        let thread_id = ThreadId::new();
        let view = ChatView::new(thread_id);
        self.threads.insert(thread_id, Signal::new(view));
        thread_id
    }

    /// Get the number of active threads.
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_creation() {
        let state = AppState::new();
        assert_eq!(state.thread_count(), 0);
        assert!(!state.is_connected());
        assert!(state.active_thread().is_none());
    }

    #[test]
    fn test_create_thread() {
        let mut state = AppState::new();

        let thread_id = state.create_thread();
        assert_eq!(state.thread_count(), 1);
        assert!(state.get_thread(&thread_id).is_some());
    }

    #[test]
    fn test_active_thread() {
        let mut state = AppState::new();

        let thread_id = state.create_thread();
        state.set_active_thread(Some(thread_id));

        assert_eq!(state.active_thread(), Some(thread_id));
        assert!(state.active_chat_view().is_some());
    }

    #[test]
    fn test_connection_status() {
        let mut state = AppState::new();

        assert!(!state.is_connected());

        state.set_connected(true);
        assert!(state.is_connected());
    }

    #[test]
    fn test_get_or_create_thread() {
        let mut state = AppState::new();

        let thread_id = ThreadId::new();

        // First call creates
        let _view1 = state.get_or_create_thread(thread_id);
        assert_eq!(state.thread_count(), 1);

        // Second call returns existing
        let _view2 = state.get_or_create_thread(thread_id);
        assert_eq!(state.thread_count(), 1);
    }
}
