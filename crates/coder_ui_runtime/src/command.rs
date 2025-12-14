//! Command - UI intent and side effects.
//!
//! The command bus separates UI intent from side effects,
//! enabling clean architecture and easy testing.

use coder_domain::{RunId, StepId, ThreadId};
use std::collections::VecDeque;

/// Commands representing UI intent.
#[derive(Debug, Clone)]
pub enum Command {
    // ==================
    // Platform Commands
    // ==================
    /// Copy text to clipboard.
    CopyToClipboard(String),

    /// Open a URL in the browser.
    OpenUrl(String),

    /// Set the cursor style.
    SetCursor(CursorStyle),

    /// Show a notification.
    ShowNotification {
        title: String,
        body: String,
        level: NotificationLevel,
    },

    // ==================
    // Navigation
    // ==================
    /// Navigate to a route.
    Navigate(Route),

    /// Go back in history.
    GoBack,

    /// Go forward in history.
    GoForward,

    // ==================
    // Chat Commands
    // ==================
    /// Send a message to a thread.
    SendMessage {
        thread_id: ThreadId,
        content: String,
    },

    /// Create a new thread.
    CreateThread,

    /// Delete a thread.
    DeleteThread { thread_id: ThreadId },

    // ==================
    // Run Commands
    // ==================
    /// Cancel a running workflow.
    CancelRun { run_id: RunId },

    /// Approve a step waiting for approval.
    ApproveStep { run_id: RunId, step_id: StepId },

    /// Reject a step waiting for approval.
    RejectStep {
        run_id: RunId,
        step_id: StepId,
        reason: Option<String>,
    },

    // ==================
    // Custom Commands
    // ==================
    /// Custom command with arbitrary data.
    Custom {
        name: String,
        data: serde_json::Value,
    },
}

/// Cursor styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    Default,
    Pointer,
    Text,
    Wait,
    Crosshair,
    Move,
    NotAllowed,
    Grab,
    Grabbing,
    ResizeNs,
    ResizeEw,
}

/// Notification levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// Application routes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Home/dashboard.
    Home,
    /// Chat thread.
    Thread { thread_id: ThreadId },
    /// Settings.
    Settings,
    /// Project view.
    Project { project_id: coder_domain::ProjectId },
    /// Custom route.
    Custom(String),
}

/// Result of executing a command.
#[derive(Debug)]
pub enum CommandResult {
    /// Command executed successfully.
    Success,
    /// Command is being processed asynchronously.
    Pending,
    /// Command failed.
    Error(String),
    /// Command was ignored (e.g., already in requested state).
    Ignored,
}

/// Handler for executing commands.
pub trait CommandHandler {
    /// Execute a command and return the result.
    fn execute(&mut self, command: Command) -> CommandResult;
}

/// A command bus that queues and dispatches commands.
pub struct CommandBus {
    /// Queued commands.
    queue: VecDeque<Command>,
    /// Handlers for different command types.
    handlers: Vec<Box<dyn CommandHandler + Send>>,
}

impl CommandBus {
    /// Create a new command bus.
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            handlers: Vec::new(),
        }
    }

    /// Add a handler to the command bus.
    pub fn add_handler<H: CommandHandler + Send + 'static>(&mut self, handler: H) {
        self.handlers.push(Box::new(handler));
    }

    /// Queue a command for execution.
    pub fn dispatch(&mut self, command: Command) {
        self.queue.push_back(command);
    }

    /// Process all queued commands.
    pub fn process(&mut self) -> Vec<CommandResult> {
        let mut results = Vec::new();

        while let Some(command) = self.queue.pop_front() {
            let mut handled = false;

            for handler in &mut self.handlers {
                let result = handler.execute(command.clone());
                match result {
                    CommandResult::Ignored => continue,
                    _ => {
                        results.push(result);
                        handled = true;
                        break;
                    }
                }
            }

            if !handled {
                results.push(CommandResult::Ignored);
            }
        }

        results
    }

    /// Get the number of pending commands.
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    /// Check if there are pending commands.
    pub fn has_pending(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Clear all pending commands.
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

impl Default for CommandBus {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple logging command handler for debugging.
pub struct LoggingHandler;

impl CommandHandler for LoggingHandler {
    fn execute(&mut self, command: Command) -> CommandResult {
        // In a real implementation, this would use a proper logger
        #[cfg(debug_assertions)]
        {
            eprintln!("[CommandBus] {:?}", command);
        }
        CommandResult::Ignored // Pass through to next handler
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHandler {
        handled: Vec<String>,
    }

    impl TestHandler {
        fn new() -> Self {
            Self {
                handled: Vec::new(),
            }
        }
    }

    impl CommandHandler for TestHandler {
        fn execute(&mut self, command: Command) -> CommandResult {
            match command {
                Command::CopyToClipboard(text) => {
                    self.handled.push(format!("copy:{}", text));
                    CommandResult::Success
                }
                Command::Navigate(route) => {
                    self.handled.push(format!("navigate:{:?}", route));
                    CommandResult::Success
                }
                _ => CommandResult::Ignored,
            }
        }
    }

    #[test]
    fn test_command_bus_dispatch() {
        let mut bus = CommandBus::new();

        bus.dispatch(Command::CopyToClipboard("test".to_string()));
        bus.dispatch(Command::Navigate(Route::Home));

        assert_eq!(bus.pending_count(), 2);
    }

    #[test]
    fn test_command_bus_process() {
        let mut bus = CommandBus::new();
        bus.add_handler(TestHandler::new());

        bus.dispatch(Command::CopyToClipboard("hello".to_string()));
        bus.dispatch(Command::Navigate(Route::Home));

        let results = bus.process();

        assert_eq!(results.len(), 2);
        assert!(bus.queue.is_empty());
    }

    #[test]
    fn test_command_result() {
        let success = CommandResult::Success;
        let error = CommandResult::Error("test error".to_string());

        assert!(matches!(success, CommandResult::Success));
        assert!(matches!(error, CommandResult::Error(_)));
    }
}
