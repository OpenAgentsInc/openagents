//! History: Undo/redo command history for graph operations
//!
//! Implements the Command pattern for reversible graph operations.
//! Each command knows how to execute and undo itself.

use std::fmt;

/// A command that can be executed and undone
pub trait Command: fmt::Debug + Send {
    /// Execute the command
    fn execute(&mut self, ctx: &mut dyn CommandContext);

    /// Undo the command
    fn undo(&mut self, ctx: &mut dyn CommandContext);

    /// Get a description for the undo/redo menu
    fn description(&self) -> &str;

    /// Whether this command can be merged with the previous one
    /// (e.g., multiple small moves while dragging)
    fn can_merge(&self, _other: &dyn Command) -> bool {
        false
    }

    /// Merge with another command (only called if can_merge returns true)
    fn merge(&mut self, _other: Box<dyn Command>) {
        // Default: don't merge
    }
}

/// Context provided to commands for execution
pub trait CommandContext {
    /// Add a node to the graph
    fn add_node(&mut self, id: &str, node_type: &str, x: f32, y: f32);

    /// Remove a node from the graph
    fn remove_node(&mut self, id: &str);

    /// Move a node to a new position
    fn move_node(&mut self, id: &str, x: f32, y: f32);

    /// Add a connection between pins
    fn add_connection(&mut self, from_unit: &str, from_pin: &str, to_unit: &str, to_pin: &str);

    /// Remove a connection
    fn remove_connection(&mut self, from_unit: &str, from_pin: &str, to_unit: &str, to_pin: &str);

    /// Set a pin's constant value
    fn set_pin_value(&mut self, unit_id: &str, pin_name: &str, value: &str);
}

/// Command history with undo/redo stacks
pub struct CommandHistory {
    /// Undo stack (most recent at end)
    undo_stack: Vec<Box<dyn Command>>,
    /// Redo stack (most recent at end)
    redo_stack: Vec<Box<dyn Command>>,
    /// Maximum history size
    max_size: usize,
    /// Whether history is currently executing (to prevent nested recording)
    executing: bool,
}

impl CommandHistory {
    /// Create a new history with default max size
    pub fn new() -> Self {
        Self::with_max_size(100)
    }

    /// Create a new history with specified max size
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size,
            executing: false,
        }
    }

    /// Execute a command and add it to history
    pub fn execute(&mut self, mut command: Box<dyn Command>, ctx: &mut dyn CommandContext) {
        if self.executing {
            return;
        }

        self.executing = true;
        command.execute(ctx);
        self.executing = false;

        // Try to merge with previous command
        if let Some(last) = self.undo_stack.last_mut() {
            if last.can_merge(command.as_ref()) {
                last.merge(command);
                return;
            }
        }

        // Add to undo stack
        self.undo_stack.push(command);

        // Clear redo stack (new action invalidates redo history)
        self.redo_stack.clear();

        // Trim if over max size
        while self.undo_stack.len() > self.max_size {
            self.undo_stack.remove(0);
        }
    }

    /// Undo the last command
    pub fn undo(&mut self, ctx: &mut dyn CommandContext) -> bool {
        if self.executing {
            return false;
        }

        if let Some(mut command) = self.undo_stack.pop() {
            self.executing = true;
            command.undo(ctx);
            self.executing = false;
            self.redo_stack.push(command);
            true
        } else {
            false
        }
    }

    /// Redo the last undone command
    pub fn redo(&mut self, ctx: &mut dyn CommandContext) -> bool {
        if self.executing {
            return false;
        }

        if let Some(mut command) = self.redo_stack.pop() {
            self.executing = true;
            command.execute(ctx);
            self.executing = false;
            self.undo_stack.push(command);
            true
        } else {
            false
        }
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Get description of next undo action
    pub fn undo_description(&self) -> Option<&str> {
        self.undo_stack.last().map(|c| c.description())
    }

    /// Get description of next redo action
    pub fn redo_description(&self) -> Option<&str> {
        self.redo_stack.last().map(|c| c.description())
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Get undo stack size
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get redo stack size
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CommandHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandHistory")
            .field("undo_count", &self.undo_stack.len())
            .field("redo_count", &self.redo_stack.len())
            .field("max_size", &self.max_size)
            .finish()
    }
}

// ========== Common Commands ==========

/// Move node command
#[derive(Debug)]
pub struct MoveNodeCommand {
    node_id: String,
    old_x: f32,
    old_y: f32,
    new_x: f32,
    new_y: f32,
}

impl MoveNodeCommand {
    pub fn new(node_id: impl Into<String>, old_x: f32, old_y: f32, new_x: f32, new_y: f32) -> Self {
        Self {
            node_id: node_id.into(),
            old_x,
            old_y,
            new_x,
            new_y,
        }
    }
}

impl Command for MoveNodeCommand {
    fn execute(&mut self, ctx: &mut dyn CommandContext) {
        ctx.move_node(&self.node_id, self.new_x, self.new_y);
    }

    fn undo(&mut self, ctx: &mut dyn CommandContext) {
        ctx.move_node(&self.node_id, self.old_x, self.old_y);
    }

    fn description(&self) -> &str {
        "Move Node"
    }

    // Note: Merging is disabled for simplicity - each move is a separate undo step
    // Could be enhanced with command IDs and timestamps for drag merging
}

/// Add node command
#[derive(Debug)]
pub struct AddNodeCommand {
    node_id: String,
    node_type: String,
    x: f32,
    y: f32,
}

impl AddNodeCommand {
    pub fn new(node_id: impl Into<String>, node_type: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            node_id: node_id.into(),
            node_type: node_type.into(),
            x,
            y,
        }
    }
}

impl Command for AddNodeCommand {
    fn execute(&mut self, ctx: &mut dyn CommandContext) {
        ctx.add_node(&self.node_id, &self.node_type, self.x, self.y);
    }

    fn undo(&mut self, ctx: &mut dyn CommandContext) {
        ctx.remove_node(&self.node_id);
    }

    fn description(&self) -> &str {
        "Add Node"
    }
}

/// Delete node command
#[derive(Debug)]
pub struct DeleteNodeCommand {
    node_id: String,
    node_type: String,
    x: f32,
    y: f32,
}

impl DeleteNodeCommand {
    pub fn new(node_id: impl Into<String>, node_type: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            node_id: node_id.into(),
            node_type: node_type.into(),
            x,
            y,
        }
    }
}

impl Command for DeleteNodeCommand {
    fn execute(&mut self, ctx: &mut dyn CommandContext) {
        ctx.remove_node(&self.node_id);
    }

    fn undo(&mut self, ctx: &mut dyn CommandContext) {
        ctx.add_node(&self.node_id, &self.node_type, self.x, self.y);
    }

    fn description(&self) -> &str {
        "Delete Node"
    }
}

/// Add connection command
#[derive(Debug)]
pub struct AddConnectionCommand {
    from_unit: String,
    from_pin: String,
    to_unit: String,
    to_pin: String,
}

impl AddConnectionCommand {
    pub fn new(
        from_unit: impl Into<String>,
        from_pin: impl Into<String>,
        to_unit: impl Into<String>,
        to_pin: impl Into<String>,
    ) -> Self {
        Self {
            from_unit: from_unit.into(),
            from_pin: from_pin.into(),
            to_unit: to_unit.into(),
            to_pin: to_pin.into(),
        }
    }
}

impl Command for AddConnectionCommand {
    fn execute(&mut self, ctx: &mut dyn CommandContext) {
        ctx.add_connection(&self.from_unit, &self.from_pin, &self.to_unit, &self.to_pin);
    }

    fn undo(&mut self, ctx: &mut dyn CommandContext) {
        ctx.remove_connection(&self.from_unit, &self.from_pin, &self.to_unit, &self.to_pin);
    }

    fn description(&self) -> &str {
        "Add Connection"
    }
}

/// Delete connection command
#[derive(Debug)]
pub struct DeleteConnectionCommand {
    from_unit: String,
    from_pin: String,
    to_unit: String,
    to_pin: String,
}

impl DeleteConnectionCommand {
    pub fn new(
        from_unit: impl Into<String>,
        from_pin: impl Into<String>,
        to_unit: impl Into<String>,
        to_pin: impl Into<String>,
    ) -> Self {
        Self {
            from_unit: from_unit.into(),
            from_pin: from_pin.into(),
            to_unit: to_unit.into(),
            to_pin: to_pin.into(),
        }
    }
}

impl Command for DeleteConnectionCommand {
    fn execute(&mut self, ctx: &mut dyn CommandContext) {
        ctx.remove_connection(&self.from_unit, &self.from_pin, &self.to_unit, &self.to_pin);
    }

    fn undo(&mut self, ctx: &mut dyn CommandContext) {
        ctx.add_connection(&self.from_unit, &self.from_pin, &self.to_unit, &self.to_pin);
    }

    fn description(&self) -> &str {
        "Delete Connection"
    }
}

/// Batch command - executes multiple commands as one
#[derive(Debug)]
pub struct BatchCommand {
    commands: Vec<Box<dyn Command>>,
    description: String,
}

impl BatchCommand {
    pub fn new(description: impl Into<String>, commands: Vec<Box<dyn Command>>) -> Self {
        Self {
            commands,
            description: description.into(),
        }
    }
}

impl Command for BatchCommand {
    fn execute(&mut self, ctx: &mut dyn CommandContext) {
        for cmd in &mut self.commands {
            cmd.execute(ctx);
        }
    }

    fn undo(&mut self, ctx: &mut dyn CommandContext) {
        // Undo in reverse order
        for cmd in self.commands.iter_mut().rev() {
            cmd.undo(ctx);
        }
    }

    fn description(&self) -> &str {
        &self.description
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    // Mock command context for testing
    struct MockContext {
        nodes: Rc<RefCell<Vec<(String, f32, f32)>>>,
    }

    impl MockContext {
        fn new() -> Self {
            Self {
                nodes: Rc::new(RefCell::new(Vec::new())),
            }
        }
    }

    impl CommandContext for MockContext {
        fn add_node(&mut self, id: &str, _node_type: &str, x: f32, y: f32) {
            self.nodes.borrow_mut().push((id.to_string(), x, y));
        }

        fn remove_node(&mut self, id: &str) {
            self.nodes.borrow_mut().retain(|(n, _, _)| n != id);
        }

        fn move_node(&mut self, id: &str, x: f32, y: f32) {
            if let Some(node) = self.nodes.borrow_mut().iter_mut().find(|(n, _, _)| n == id) {
                node.1 = x;
                node.2 = y;
            }
        }

        fn add_connection(&mut self, _: &str, _: &str, _: &str, _: &str) {}
        fn remove_connection(&mut self, _: &str, _: &str, _: &str, _: &str) {}
        fn set_pin_value(&mut self, _: &str, _: &str, _: &str) {}
    }

    #[test]
    fn test_add_node_undo_redo() {
        let mut history = CommandHistory::new();
        let mut ctx = MockContext::new();

        let cmd = Box::new(AddNodeCommand::new("node1", "add", 100.0, 200.0));
        history.execute(cmd, &mut ctx);

        assert_eq!(ctx.nodes.borrow().len(), 1);

        history.undo(&mut ctx);
        assert_eq!(ctx.nodes.borrow().len(), 0);

        history.redo(&mut ctx);
        assert_eq!(ctx.nodes.borrow().len(), 1);
    }

    #[test]
    fn test_move_node_undo() {
        let mut history = CommandHistory::new();
        let mut ctx = MockContext::new();

        // First add a node
        ctx.add_node("node1", "test", 0.0, 0.0);

        // Then move it
        let cmd = Box::new(MoveNodeCommand::new("node1", 0.0, 0.0, 100.0, 100.0));
        history.execute(cmd, &mut ctx);

        assert_eq!(ctx.nodes.borrow()[0].1, 100.0);
        assert_eq!(ctx.nodes.borrow()[0].2, 100.0);

        history.undo(&mut ctx);
        assert_eq!(ctx.nodes.borrow()[0].1, 0.0);
        assert_eq!(ctx.nodes.borrow()[0].2, 0.0);
    }

    #[test]
    fn test_history_descriptions() {
        let mut history = CommandHistory::new();
        let mut ctx = MockContext::new();

        assert!(!history.can_undo());
        assert_eq!(history.undo_description(), None);

        let cmd = Box::new(AddNodeCommand::new("node1", "add", 0.0, 0.0));
        history.execute(cmd, &mut ctx);

        assert!(history.can_undo());
        assert_eq!(history.undo_description(), Some("Add Node"));

        history.undo(&mut ctx);
        assert!(history.can_redo());
        assert_eq!(history.redo_description(), Some("Add Node"));
    }

    #[test]
    fn test_clear_redo_on_new_action() {
        let mut history = CommandHistory::new();
        let mut ctx = MockContext::new();

        // Add, undo, then add again
        history.execute(
            Box::new(AddNodeCommand::new("node1", "add", 0.0, 0.0)),
            &mut ctx,
        );
        history.undo(&mut ctx);
        assert!(history.can_redo());

        // New action should clear redo
        history.execute(
            Box::new(AddNodeCommand::new("node2", "add", 0.0, 0.0)),
            &mut ctx,
        );
        assert!(!history.can_redo());
    }
}
