//! Full thread view section component.

use maud::{Markup, html};
use crate::acp::atoms::AgentMode;
use crate::acp::organisms::{ThreadEntry, ThreadEntryKind, ThreadControls, PlanTodo};
use crate::acp::sections::{ThreadHeader, ThreadFeedback, MessageEditor, ConnectionStatus};

/// Full thread view combining all sections.
pub struct ThreadView {
    session_id: String,
    mode: AgentMode,
    model_id: String,
    entries: Vec<ThreadEntryKind>,
    todos: Vec<PlanTodo>,
    connection_status: ConnectionStatus,
    completed: bool,
}

impl ThreadView {
    /// Create a new thread view.
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            mode: AgentMode::Code,
            model_id: String::new(),
            entries: Vec::new(),
            todos: Vec::new(),
            connection_status: ConnectionStatus::Connected,
            completed: false,
        }
    }

    /// Set the agent mode.
    pub fn mode(mut self, mode: AgentMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the model ID.
    pub fn model(mut self, model_id: impl Into<String>) -> Self {
        self.model_id = model_id.into();
        self
    }

    /// Set the thread entries.
    pub fn entries(mut self, entries: Vec<ThreadEntryKind>) -> Self {
        self.entries = entries;
        self
    }

    /// Set the plan todos.
    pub fn todos(mut self, todos: Vec<PlanTodo>) -> Self {
        self.todos = todos;
        self
    }

    /// Set the connection status.
    pub fn connection_status(mut self, status: ConnectionStatus) -> Self {
        self.connection_status = status;
        self
    }

    /// Mark thread as completed.
    pub fn completed(mut self) -> Self {
        self.completed = true;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        html! {
            div class="flex flex-col h-full" data-session-id=(self.session_id) {
                // Thread header
                (ThreadHeader::new(&self.session_id)
                    .mode(self.mode.clone())
                    .model(&self.model_id)
                    .connection_status(self.connection_status)
                    .build())

                // Thread controls (mode selector, model selector, plan)
                (ThreadControls::new(self.mode, &self.model_id, &self.session_id)
                    .todos(self.todos)
                    .build())

                // Scrollable entries area
                div class="flex-1 overflow-y-auto p-4" {
                    @for (idx, entry) in self.entries.into_iter().enumerate() {
                        (ThreadEntry::new(entry, format!("{}-{}", self.session_id, idx), idx).build())
                    }
                }

                // Feedback (if completed)
                @if self.completed {
                    (ThreadFeedback::new(&self.session_id).build())
                }

                // Message editor
                (MessageEditor::new(&self.session_id).build())
            }
        }
    }
}
