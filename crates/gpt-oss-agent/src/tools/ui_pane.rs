//! UI Pane manipulation tool for agent-controlled UI
//!
//! This tool allows agents to:
//! - Move panes around the screen
//! - Change pane priorities (focus/z-order)
//! - Trigger animations (slide in, fade, glow)
//! - Open/close/minimize panes
//! - Set attention indicators

use super::{Tool, ToolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Position on screen
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

/// Size of a pane
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// Pane visibility state
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PaneState {
    Open,
    Minimized,
    Closed,
    SlideIn,
    SlideOut,
    FadeIn,
    FadeOut,
}

/// Frame style for the pane border
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum FrameStyle {
    #[default]
    Corners,
    Lines,
    Octagon,
    Underline,
    Nefrex,
    Kranox,
}

/// Priority level for pane attention
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Priority {
    Background = 0,
    #[default]
    Normal = 1,
    Elevated = 2,
    Urgent = 3,
    Critical = 4,
}

/// Animation type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Animation {
    None,
    SlideFromLeft { duration_ms: u32 },
    SlideFromRight { duration_ms: u32 },
    SlideFromTop { duration_ms: u32 },
    SlideFromBottom { duration_ms: u32 },
    FadeIn { duration_ms: u32 },
    FadeOut { duration_ms: u32 },
    Pulse { count: u32, duration_ms: u32 },
    Glow { color: String, duration_ms: u32 },
    Shake { intensity: f32, duration_ms: u32 },
}

/// A UI pane that the agent can manipulate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pane {
    pub id: String,
    pub title: String,
    pub position: Position,
    pub size: Size,
    pub state: PaneState,
    pub priority: Priority,
    pub z_index: i32,
    pub frame_style: FrameStyle,
    pub glow_color: Option<String>,
    pub animation_progress: f32,
    pub content_type: String,
}

impl Pane {
    pub fn new(id: &str, title: &str) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            position: Position { x: 100.0, y: 100.0 },
            size: Size {
                width: 400.0,
                height: 300.0,
            },
            state: PaneState::Open,
            priority: Priority::Normal,
            z_index: 0,
            frame_style: FrameStyle::Corners,
            glow_color: None,
            animation_progress: 1.0,
            content_type: "generic".to_string(),
        }
    }
}

/// UI Pane Manager - tracks all panes and their state
#[derive(Debug, Default)]
pub struct PaneManager {
    panes: HashMap<String, Pane>,
    focus_stack: Vec<String>,
    next_z_index: i32,
}

impl PaneManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_pane(&mut self, pane: Pane) {
        let id = pane.id.clone();
        self.panes.insert(id.clone(), pane);
        self.focus_stack.push(id);
    }

    pub fn get_pane(&self, id: &str) -> Option<&Pane> {
        self.panes.get(id)
    }

    pub fn get_pane_mut(&mut self, id: &str) -> Option<&mut Pane> {
        self.panes.get_mut(id)
    }

    pub fn list_panes(&self) -> Vec<&Pane> {
        self.panes.values().collect()
    }

    pub fn bring_to_front(&mut self, id: &str) {
        if let Some(pane) = self.panes.get_mut(id) {
            self.next_z_index += 1;
            pane.z_index = self.next_z_index;
            self.focus_stack.retain(|x| x != id);
            self.focus_stack.push(id.to_string());
        }
    }

    pub fn get_focused_pane(&self) -> Option<&str> {
        self.focus_stack.last().map(|s| s.as_str())
    }
}

/// Tool parameters for UI pane operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum UiPaneAction {
    /// List all panes
    ListPanes,

    /// Create a new pane
    CreatePane {
        id: String,
        title: String,
        #[serde(default)]
        position: Option<Position>,
        #[serde(default)]
        size: Option<Size>,
        #[serde(default)]
        content_type: Option<String>,
    },

    /// Move a pane to a new position
    MovePane {
        id: String,
        position: Position,
        #[serde(default)]
        animate: bool,
    },

    /// Resize a pane
    ResizePane {
        id: String,
        size: Size,
        #[serde(default)]
        animate: bool,
    },

    /// Set pane priority (affects visual prominence)
    SetPriority { id: String, priority: Priority },

    /// Bring pane to front (focus)
    Focus { id: String },

    /// Change pane state (open, close, minimize)
    SetState { id: String, state: PaneState },

    /// Set frame style
    SetFrameStyle { id: String, style: FrameStyle },

    /// Trigger animation on a pane
    Animate { id: String, animation: Animation },

    /// Set glow color for attention
    SetGlow { id: String, color: Option<String> },

    /// Close and remove a pane
    ClosePane { id: String },

    /// Get attention - flash/animate the most important pane
    RequestAttention { id: String, message: Option<String> },
}

/// UI Pane Tool
pub struct UiPaneTool {
    manager: Arc<RwLock<PaneManager>>,
}

impl UiPaneTool {
    pub fn new(manager: Arc<RwLock<PaneManager>>) -> Self {
        Self { manager }
    }

    fn execute_action(&self, action: UiPaneAction) -> crate::Result<ToolResult> {
        let mut manager = self.manager.write().unwrap();

        match action {
            UiPaneAction::ListPanes => {
                let panes: Vec<_> = manager
                    .list_panes()
                    .iter()
                    .map(|p| {
                        serde_json::json!({
                            "id": p.id,
                            "title": p.title,
                            "position": p.position,
                            "size": p.size,
                            "state": p.state,
                            "priority": p.priority,
                            "z_index": p.z_index,
                        })
                    })
                    .collect();

                Ok(ToolResult {
                    success: true,
                    output: serde_json::to_string_pretty(&panes).unwrap(),
                    error: None,
                })
            }

            UiPaneAction::CreatePane {
                id,
                title,
                position,
                size,
                content_type,
            } => {
                let mut pane = Pane::new(&id, &title);
                if let Some(pos) = position {
                    pane.position = pos;
                }
                if let Some(sz) = size {
                    pane.size = sz;
                }
                if let Some(ct) = content_type {
                    pane.content_type = ct;
                }
                pane.state = PaneState::SlideIn;
                manager.add_pane(pane.clone());

                Ok(ToolResult {
                    success: true,
                    output: format!(
                        "Created pane '{}' ({}) at ({}, {})",
                        id, title, pane.position.x, pane.position.y
                    ),
                    error: None,
                })
            }

            UiPaneAction::MovePane {
                id,
                position,
                animate,
            } => {
                if let Some(pane) = manager.get_pane_mut(&id) {
                    let old_pos = pane.position;
                    pane.position = position;
                    if animate {
                        pane.animation_progress = 0.0;
                    }

                    Ok(ToolResult {
                        success: true,
                        output: format!(
                            "Moved pane '{}' from ({}, {}) to ({}, {}){}",
                            id,
                            old_pos.x,
                            old_pos.y,
                            position.x,
                            position.y,
                            if animate { " (animated)" } else { "" }
                        ),
                        error: None,
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Pane '{}' not found", id)),
                    })
                }
            }

            UiPaneAction::ResizePane { id, size, animate } => {
                if let Some(pane) = manager.get_pane_mut(&id) {
                    let old_size = pane.size;
                    pane.size = size;
                    if animate {
                        pane.animation_progress = 0.0;
                    }

                    Ok(ToolResult {
                        success: true,
                        output: format!(
                            "Resized pane '{}' from {}x{} to {}x{}{}",
                            id,
                            old_size.width,
                            old_size.height,
                            size.width,
                            size.height,
                            if animate { " (animated)" } else { "" }
                        ),
                        error: None,
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Pane '{}' not found", id)),
                    })
                }
            }

            UiPaneAction::SetPriority { id, priority } => {
                if let Some(pane) = manager.get_pane_mut(&id) {
                    let old_priority = pane.priority;
                    pane.priority = priority;

                    if priority >= Priority::Urgent {
                        pane.glow_color = Some("#ff6600".to_string());
                    } else if priority == Priority::Elevated {
                        pane.glow_color = Some("#00a8ff".to_string());
                    }

                    Ok(ToolResult {
                        success: true,
                        output: format!(
                            "Changed priority of '{}' from {:?} to {:?}",
                            id, old_priority, priority
                        ),
                        error: None,
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Pane '{}' not found", id)),
                    })
                }
            }

            UiPaneAction::Focus { id } => {
                manager.bring_to_front(&id);
                if let Some(pane) = manager.get_pane(&id) {
                    Ok(ToolResult {
                        success: true,
                        output: format!("Focused pane '{}' (z-index: {})", id, pane.z_index),
                        error: None,
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Pane '{}' not found", id)),
                    })
                }
            }

            UiPaneAction::SetState { id, state } => {
                if let Some(pane) = manager.get_pane_mut(&id) {
                    let old_state = pane.state;
                    pane.state = state;

                    Ok(ToolResult {
                        success: true,
                        output: format!(
                            "Changed state of '{}' from {:?} to {:?}",
                            id, old_state, state
                        ),
                        error: None,
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Pane '{}' not found", id)),
                    })
                }
            }

            UiPaneAction::SetFrameStyle { id, style } => {
                if let Some(pane) = manager.get_pane_mut(&id) {
                    pane.frame_style = style;

                    Ok(ToolResult {
                        success: true,
                        output: format!("Set frame style of '{}' to {:?}", id, style),
                        error: None,
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Pane '{}' not found", id)),
                    })
                }
            }

            UiPaneAction::Animate { id, animation } => {
                if let Some(pane) = manager.get_pane_mut(&id) {
                    pane.animation_progress = 0.0;

                    Ok(ToolResult {
                        success: true,
                        output: format!("Triggered {:?} animation on '{}'", animation, id),
                        error: None,
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Pane '{}' not found", id)),
                    })
                }
            }

            UiPaneAction::SetGlow { id, color } => {
                if let Some(pane) = manager.get_pane_mut(&id) {
                    pane.glow_color = color.clone();

                    Ok(ToolResult {
                        success: true,
                        output: format!(
                            "Set glow on '{}' to {:?}",
                            id,
                            color.unwrap_or_else(|| "none".to_string())
                        ),
                        error: None,
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Pane '{}' not found", id)),
                    })
                }
            }

            UiPaneAction::ClosePane { id } => {
                if let Some(pane) = manager.get_pane_mut(&id) {
                    pane.state = PaneState::SlideOut;

                    Ok(ToolResult {
                        success: true,
                        output: format!("Closing pane '{}' (slide out)", id),
                        error: None,
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Pane '{}' not found", id)),
                    })
                }
            }

            UiPaneAction::RequestAttention { id, message } => {
                if let Some(pane) = manager.get_pane_mut(&id) {
                    pane.priority = Priority::Urgent;
                    pane.glow_color = Some("#ff0000".to_string());
                    manager.bring_to_front(&id);

                    Ok(ToolResult {
                        success: true,
                        output: format!(
                            "Attention requested on '{}': {}",
                            id,
                            message.unwrap_or_else(|| "Look here!".to_string())
                        ),
                        error: None,
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Pane '{}' not found", id)),
                    })
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl Tool for UiPaneTool {
    async fn execute(&self, params: serde_json::Value) -> crate::Result<ToolResult> {
        let action: UiPaneAction = serde_json::from_value(params).map_err(|e| {
            crate::GptOssAgentError::ToolError(format!("Invalid parameters: {}", e))
        })?;

        self.execute_action(action)
    }

    fn name(&self) -> &str {
        "ui_pane"
    }

    fn description(&self) -> &str {
        "Manipulate UI panes - move, resize, animate, focus, and control visibility"
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "ListPanes", "CreatePane", "MovePane", "ResizePane",
                        "SetPriority", "Focus", "SetState", "SetFrameStyle",
                        "Animate", "SetGlow", "ClosePane", "RequestAttention"
                    ]
                },
                "id": { "type": "string" },
                "title": { "type": "string" },
                "position": {
                    "type": "object",
                    "properties": {
                        "x": { "type": "number" },
                        "y": { "type": "number" }
                    }
                },
                "size": {
                    "type": "object",
                    "properties": {
                        "width": { "type": "number" },
                        "height": { "type": "number" }
                    }
                },
                "priority": {
                    "type": "string",
                    "enum": ["Background", "Normal", "Elevated", "Urgent", "Critical"]
                },
                "state": {
                    "type": "string",
                    "enum": ["Open", "Minimized", "Closed", "SlideIn", "SlideOut", "FadeIn", "FadeOut"]
                },
                "style": {
                    "type": "string",
                    "enum": ["Corners", "Lines", "Octagon", "Underline", "Nefrex", "Kranox"]
                },
                "animation": {
                    "type": "object",
                    "description": "Animation type with parameters"
                },
                "animate": { "type": "boolean" },
                "color": { "type": "string" },
                "message": { "type": "string" }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pane_creation() {
        let pane = Pane::new("test", "Test Pane");
        assert_eq!(pane.id, "test");
        assert_eq!(pane.title, "Test Pane");
        assert_eq!(pane.priority, Priority::Normal);
    }

    #[test]
    fn test_pane_manager() {
        let mut manager = PaneManager::new();
        manager.add_pane(Pane::new("pane1", "First"));
        manager.add_pane(Pane::new("pane2", "Second"));

        assert_eq!(manager.list_panes().len(), 2);
        assert_eq!(manager.get_focused_pane(), Some("pane2"));

        manager.bring_to_front("pane1");
        assert_eq!(manager.get_focused_pane(), Some("pane1"));
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::Urgent);
        assert!(Priority::Urgent > Priority::Elevated);
        assert!(Priority::Elevated > Priority::Normal);
        assert!(Priority::Normal > Priority::Background);
    }

    #[tokio::test]
    async fn test_ui_pane_tool_create() {
        let manager = Arc::new(RwLock::new(PaneManager::new()));
        let tool = UiPaneTool::new(manager.clone());

        let result = tool
            .execute(serde_json::json!({
                "action": "CreatePane",
                "id": "editor",
                "title": "Code Editor"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("Created pane 'editor'"));

        let mgr = manager.read().unwrap();
        assert!(mgr.get_pane("editor").is_some());
    }

    #[tokio::test]
    async fn test_ui_pane_tool_move() {
        let manager = Arc::new(RwLock::new(PaneManager::new()));
        {
            let mut mgr = manager.write().unwrap();
            mgr.add_pane(Pane::new("test", "Test"));
        }

        let tool = UiPaneTool::new(manager.clone());

        let result = tool
            .execute(serde_json::json!({
                "action": "MovePane",
                "id": "test",
                "position": { "x": 200.0, "y": 300.0 },
                "animate": true
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("animated"));

        let mgr = manager.read().unwrap();
        let pane = mgr.get_pane("test").unwrap();
        assert_eq!(pane.position.x, 200.0);
        assert_eq!(pane.position.y, 300.0);
    }

    #[tokio::test]
    async fn test_ui_pane_tool_priority() {
        let manager = Arc::new(RwLock::new(PaneManager::new()));
        {
            let mut mgr = manager.write().unwrap();
            mgr.add_pane(Pane::new("alert", "Alert"));
        }

        let tool = UiPaneTool::new(manager.clone());

        let result = tool
            .execute(serde_json::json!({
                "action": "SetPriority",
                "id": "alert",
                "priority": "Urgent"
            }))
            .await
            .unwrap();

        assert!(result.success);

        let mgr = manager.read().unwrap();
        let pane = mgr.get_pane("alert").unwrap();
        assert_eq!(pane.priority, Priority::Urgent);
        assert!(
            pane.glow_color.is_some(),
            "Urgent priority should auto-enable glow"
        );
    }

    #[tokio::test]
    async fn test_ui_pane_tool_resize() {
        let manager = Arc::new(RwLock::new(PaneManager::new()));
        {
            let mut mgr = manager.write().unwrap();
            mgr.add_pane(Pane::new("test", "Test"));
        }

        let tool = UiPaneTool::new(manager.clone());

        let result = tool
            .execute(serde_json::json!({
                "action": "ResizePane",
                "id": "test",
                "size": { "width": 600.0, "height": 400.0 }
            }))
            .await
            .unwrap();

        assert!(result.success);

        let mgr = manager.read().unwrap();
        let pane = mgr.get_pane("test").unwrap();
        assert_eq!(pane.size.width, 600.0);
        assert_eq!(pane.size.height, 400.0);
    }

    #[tokio::test]
    async fn test_ui_pane_tool_set_state() {
        let manager = Arc::new(RwLock::new(PaneManager::new()));
        {
            let mut mgr = manager.write().unwrap();
            mgr.add_pane(Pane::new("test", "Test"));
        }

        let tool = UiPaneTool::new(manager.clone());

        let result = tool
            .execute(serde_json::json!({
                "action": "SetState",
                "id": "test",
                "state": "Minimized"
            }))
            .await
            .unwrap();

        assert!(result.success);

        let mgr = manager.read().unwrap();
        let pane = mgr.get_pane("test").unwrap();
        assert_eq!(pane.state, PaneState::Minimized);
    }

    #[tokio::test]
    async fn test_ui_pane_tool_frame_style() {
        let manager = Arc::new(RwLock::new(PaneManager::new()));
        {
            let mut mgr = manager.write().unwrap();
            mgr.add_pane(Pane::new("test", "Test"));
        }

        let tool = UiPaneTool::new(manager.clone());

        let result = tool
            .execute(serde_json::json!({
                "action": "SetFrameStyle",
                "id": "test",
                "style": "Kranox"
            }))
            .await
            .unwrap();

        assert!(result.success);

        let mgr = manager.read().unwrap();
        let pane = mgr.get_pane("test").unwrap();
        assert_eq!(pane.frame_style, FrameStyle::Kranox);
    }

    #[tokio::test]
    async fn test_ui_pane_tool_glow() {
        let manager = Arc::new(RwLock::new(PaneManager::new()));
        {
            let mut mgr = manager.write().unwrap();
            mgr.add_pane(Pane::new("test", "Test"));
        }

        let tool = UiPaneTool::new(manager.clone());

        let result = tool
            .execute(serde_json::json!({
                "action": "SetGlow",
                "id": "test",
                "color": "#00ff00"
            }))
            .await
            .unwrap();

        assert!(result.success);

        let mgr = manager.read().unwrap();
        let pane = mgr.get_pane("test").unwrap();
        assert_eq!(pane.glow_color, Some("#00ff00".to_string()));
    }

    #[tokio::test]
    async fn test_ui_pane_tool_list() {
        let manager = Arc::new(RwLock::new(PaneManager::new()));
        {
            let mut mgr = manager.write().unwrap();
            mgr.add_pane(Pane::new("pane1", "First"));
            mgr.add_pane(Pane::new("pane2", "Second"));
        }

        let tool = UiPaneTool::new(manager.clone());

        let result = tool
            .execute(serde_json::json!({
                "action": "ListPanes"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("pane1"));
        assert!(result.output.contains("pane2"));
    }

    #[tokio::test]
    async fn test_ui_pane_tool_close() {
        let manager = Arc::new(RwLock::new(PaneManager::new()));
        {
            let mut mgr = manager.write().unwrap();
            mgr.add_pane(Pane::new("test", "Test"));
        }

        let tool = UiPaneTool::new(manager.clone());

        let result = tool
            .execute(serde_json::json!({
                "action": "ClosePane",
                "id": "test"
            }))
            .await
            .unwrap();

        assert!(result.success);

        let mgr = manager.read().unwrap();
        let pane = mgr.get_pane("test").unwrap();
        assert_eq!(pane.state, PaneState::SlideOut);
    }

    #[tokio::test]
    async fn test_ui_pane_tool_focus() {
        let manager = Arc::new(RwLock::new(PaneManager::new()));
        {
            let mut mgr = manager.write().unwrap();
            mgr.add_pane(Pane::new("pane1", "First"));
            mgr.add_pane(Pane::new("pane2", "Second"));
        }

        let tool = UiPaneTool::new(manager.clone());

        let result = tool
            .execute(serde_json::json!({
                "action": "Focus",
                "id": "pane1"
            }))
            .await
            .unwrap();

        assert!(result.success);

        let mgr = manager.read().unwrap();
        assert_eq!(mgr.get_focused_pane(), Some("pane1"));
    }

    #[tokio::test]
    async fn test_ui_pane_tool_request_attention() {
        let manager = Arc::new(RwLock::new(PaneManager::new()));
        {
            let mut mgr = manager.write().unwrap();
            mgr.add_pane(Pane::new("alert", "Alert"));
        }

        let tool = UiPaneTool::new(manager.clone());

        let result = tool
            .execute(serde_json::json!({
                "action": "RequestAttention",
                "id": "alert",
                "message": "Error detected!"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("Error detected!"));

        let mgr = manager.read().unwrap();
        let pane = mgr.get_pane("alert").unwrap();
        assert_eq!(pane.priority, Priority::Urgent);
        assert_eq!(pane.glow_color, Some("#ff0000".to_string()));
    }

    #[tokio::test]
    async fn test_ui_pane_tool_not_found() {
        let manager = Arc::new(RwLock::new(PaneManager::new()));
        let tool = UiPaneTool::new(manager.clone());

        let result = tool
            .execute(serde_json::json!({
                "action": "MovePane",
                "id": "nonexistent",
                "position": { "x": 0.0, "y": 0.0 }
            }))
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_ui_pane_tool_animate() {
        let manager = Arc::new(RwLock::new(PaneManager::new()));
        {
            let mut mgr = manager.write().unwrap();
            mgr.add_pane(Pane::new("test", "Test"));
        }

        let tool = UiPaneTool::new(manager.clone());

        let result = tool
            .execute(serde_json::json!({
                "action": "Animate",
                "id": "test",
                "animation": { "Pulse": { "count": 3, "duration_ms": 500 } }
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("animation"));
    }
}
