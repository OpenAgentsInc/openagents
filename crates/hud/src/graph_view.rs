//! GraphView: Full graph canvas with physics-based layout
//!
//! Renders a complete Unit graph with:
//! - Physics simulation for force-directed layout
//! - Pan and zoom navigation
//! - Node selection and dragging
//! - Connection rendering with bezier curves
//! - Animation loop for smooth physics updates

use gpui::{
    App, Context, Entity, EventEmitter, Focusable, FocusHandle, Hsla,
    MouseButton, MouseMoveEvent, MouseUpEvent, PathBuilder, Render, ScrollWheelEvent, Window,
    canvas, div, point, prelude::*, px,
};
use std::collections::HashMap;
use std::time::Duration;
use theme_oa::hud;

use unit::{Point as UnitPoint, Graph, SimConnection, SimNode, SimulationConfig, tick, should_stop, reheat};

use crate::actions::{DeselectAll, ResetView, SelectAll, ZoomIn, ZoomOut, ZoomToFit};
use crate::apm_widget::{ApmWidget, ApmState, ApmSnapshot, ApmComparison};
use crate::connection::{Connection, ConnectionState, ConnectionStyle};
use crate::pin_view::PinDirection;
use crate::selection::{SelectionManager, SelectionRect};
use crate::unit_view::{UnitSnapshot, UnitStyle, UnitView};

/// Graph view style configuration
#[derive(Debug, Clone)]
pub struct GraphStyle {
    /// Background color
    pub background: Hsla,
    /// Grid line color (if enabled)
    pub grid_color: Hsla,
    /// Grid spacing
    pub grid_spacing: f32,
    /// Whether to show grid
    pub show_grid: bool,
    /// Unit style
    pub unit_style: UnitStyle,
    /// Connection style
    pub connection_style: ConnectionStyle,
    /// Minimum zoom level
    pub min_zoom: f32,
    /// Maximum zoom level
    pub max_zoom: f32,
}

impl Default for GraphStyle {
    fn default() -> Self {
        Self {
            background: hud::GRAPH_BG,
            grid_color: hud::GRID,
            grid_spacing: 50.0,
            show_grid: true,
            unit_style: UnitStyle::default(),
            connection_style: ConnectionStyle::default(),
            min_zoom: 0.25,
            max_zoom: 4.0,
        }
    }
}

/// Events emitted by GraphView
#[derive(Debug, Clone)]
pub enum GraphEvent {
    /// A unit was selected
    UnitSelected { id: String },
    /// Multiple units were selected
    MultipleSelected { ids: Vec<String> },
    /// Selection was cleared
    SelectionCleared,
    /// A unit was moved
    UnitMoved { id: String, x: f32, y: f32 },
    /// Multiple units were moved
    MultipleUnitsMoved { ids: Vec<String> },
    /// Connection drag started
    ConnectionDragStarted {
        unit_id: String,
        pin_name: String,
        direction: PinDirection,
    },
    /// Connection was created
    ConnectionCreated {
        from_unit: String,
        from_pin: String,
        to_unit: String,
        to_pin: String,
    },
    /// Physics simulation settled
    SimulationSettled,
}

/// State for dragging operations
#[derive(Debug, Clone)]
enum DragState {
    /// No drag in progress
    None,
    /// Panning the canvas
    Panning { start_x: f32, start_y: f32 },
    /// Dragging a unit (or multiple selected units)
    DraggingUnit {
        unit_id: String,
        offset_x: f32,
        offset_y: f32,
    },
    /// Rubber band selection
    RubberBand {
        rect: SelectionRect,
        /// Whether to add to existing selection (shift held)
        additive: bool,
    },
}

/// Node in the graph view (visual + physics)
struct GraphNode {
    /// Unit view entity
    view: Entity<UnitView>,
    /// Position x
    x: f32,
    /// Position y
    y: f32,
    /// Velocity x
    vx: f32,
    /// Velocity y
    vy: f32,
    /// Width
    width: f32,
    /// Height
    height: f32,
    /// Whether node is fixed (being dragged)
    is_dragged: bool,
}

/// GPUI Entity for rendering a complete graph
pub struct GraphView {
    /// The underlying Unit graph
    graph: Option<Graph>,
    /// Visual nodes
    nodes: HashMap<String, GraphNode>,
    /// Simulation connections
    sim_connections: Vec<SimConnection>,
    /// Physics configuration
    sim_config: SimulationConfig,
    /// Camera pan offset x
    pan_x: f32,
    /// Camera pan offset y
    pan_y: f32,
    /// Zoom level
    zoom: f32,
    /// Current drag state
    drag_state: DragState,
    /// Selection manager for multi-select
    selection: SelectionManager,
    /// Visual style
    style: GraphStyle,
    /// Whether simulation is running
    simulating: bool,

    // =========================================================================
    // HUD State (for testing and message handling)
    // =========================================================================

    /// Current session ID (if a session is active)
    current_session_id: Option<String>,
    /// Current APM value
    current_apm: f64,
    /// Current error message (if any)
    current_error: Option<String>,
    /// Error count for this session
    error_count: usize,
    /// Total messages received
    message_count: usize,
    /// Connection state (simulated for testing)
    is_connected: bool,

    // =========================================================================
    // APM Widget State
    // =========================================================================

    /// APM widget entity (if created)
    apm_widget: Option<Entity<ApmWidget>>,
    /// APM state (for direct access without widget)
    apm_state: ApmState,

    // =========================================================================
    // Focus & Input
    // =========================================================================

    /// Focus handle for keyboard input
    focus_handle: FocusHandle,
}

impl GraphView {
    /// Create a new empty GraphView
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            graph: None,
            nodes: HashMap::new(),
            sim_connections: Vec::new(),
            sim_config: SimulationConfig::default(),
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
            drag_state: DragState::None,
            selection: SelectionManager::new(),
            style: GraphStyle::default(),
            simulating: false,
            // HUD state
            current_session_id: None,
            current_apm: 0.0,
            current_error: None,
            error_count: 0,
            message_count: 0,
            is_connected: true,
            // APM widget state
            apm_widget: None,
            apm_state: ApmState::new(),
            // Focus handle
            focus_handle: cx.focus_handle(),
        }
    }

    /// Create a new GraphView with APM widget enabled
    pub fn new_with_apm(cx: &mut Context<Self>) -> Self {
        let apm_widget = cx.new(|cx| ApmWidget::new(cx));
        Self {
            graph: None,
            nodes: HashMap::new(),
            sim_connections: Vec::new(),
            sim_config: SimulationConfig::default(),
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
            drag_state: DragState::None,
            selection: SelectionManager::new(),
            style: GraphStyle::default(),
            simulating: false,
            // HUD state
            current_session_id: None,
            current_apm: 0.0,
            current_error: None,
            error_count: 0,
            message_count: 0,
            is_connected: true,
            // APM widget state
            apm_widget: Some(apm_widget),
            apm_state: ApmState::new(),
            // Focus handle
            focus_handle: cx.focus_handle(),
        }
    }

    /// Load a graph for visualization
    pub fn load_graph(&mut self, graph: Graph, cx: &mut Context<Self>) {
        self.nodes.clear();
        self.sim_connections.clear();

        // Create visual nodes with initial positions
        let unit_ids: Vec<String> = graph.unit_ids().iter().map(|s| s.to_string()).collect();
        let num_units = unit_ids.len();

        for (i, id) in unit_ids.iter().enumerate() {
            if let Some(unit) = graph.get_unit(id) {
                // Arrange in a circle initially
                let angle = (i as f64 / num_units.max(1) as f64) * std::f64::consts::TAU;
                let radius = 200.0;
                let x = (angle.cos() * radius) as f32;
                let y = (angle.sin() * radius) as f32;

                let snapshot = UnitSnapshot::from_unit(
                    unit,
                    point(px(x), px(y)),
                );

                let size = snapshot.calculate_size(&self.style.unit_style);
                let width: f32 = size.width.into();
                let height: f32 = size.height.into();
                let view = cx.new(|cx| UnitView::new(snapshot, cx));

                self.nodes.insert(id.clone(), GraphNode {
                    view,
                    x,
                    y,
                    vx: 0.0,
                    vy: 0.0,
                    width,
                    height,
                    is_dragged: false,
                });
            }
        }

        self.graph = Some(graph);
        self.start_simulation(cx);
    }

    /// Add a demo node for storybook/testing purposes
    pub fn add_demo_node_with_cx(&mut self, id: &str, x: f32, y: f32, cx: &mut Context<Self>) {
        use unit::{Lifecycle, PinState};
        use crate::pin_view::PinSnapshot;

        let snapshot = UnitSnapshot {
            id: id.to_string(),
            lifecycle: Lifecycle::Playing,
            inputs: vec![
                PinSnapshot {
                    name: "in".to_string(),
                    state: PinState::Valid,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Input,
                    type_name: "T".to_string(),
                },
            ],
            outputs: vec![
                PinSnapshot {
                    name: "out".to_string(),
                    state: PinState::Valid,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Output,
                    type_name: "T".to_string(),
                },
            ],
            error: None,
            position: point(px(x), px(y)),
        };

        let size = snapshot.calculate_size(&self.style.unit_style);
        let width: f32 = size.width.into();
        let height: f32 = size.height.into();
        let view = cx.new(|cx| UnitView::new(snapshot, cx));

        self.nodes.insert(id.to_string(), GraphNode {
            view,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            width,
            height,
            is_dragged: false,
        });

        // Start simulation if we have nodes
        if !self.simulating && !self.nodes.is_empty() {
            self.start_simulation(cx);
        }
    }

    /// Start the physics simulation
    pub fn start_simulation(&mut self, cx: &mut Context<Self>) {
        reheat(&mut self.sim_config);
        self.simulating = true;
        self.schedule_tick(cx);
    }

    /// Schedule the next simulation tick
    fn schedule_tick(&mut self, cx: &mut Context<Self>) {
        if !self.simulating {
            return;
        }

        cx.spawn(async move |view, cx| {
            cx.background_executor().timer(Duration::from_millis(16)).await;
            view.update(cx, |view, cx| {
                view.simulation_tick(cx);
            }).ok();
        }).detach();
    }

    /// Run one simulation step
    fn simulation_tick(&mut self, cx: &mut Context<Self>) {
        if should_stop(&self.sim_config) {
            self.simulating = false;
            cx.emit(GraphEvent::SimulationSettled);
            return;
        }

        // Build SimNodes from our nodes
        let mut sim_nodes: Vec<SimNode> = self.nodes.iter()
            .map(|(id, node)| {
                let mut sim = SimNode::new_rect(
                    id.clone(),
                    node.x as f64,
                    node.y as f64,
                    node.width as f64,
                    node.height as f64,
                );
                sim.velocity = UnitPoint::new(node.vx as f64, node.vy as f64);
                if node.is_dragged {
                    sim.fix();
                }
                sim
            })
            .collect();

        // Run physics step
        tick(&mut sim_nodes, &self.sim_connections, &mut self.sim_config);

        // Update node positions from simulation
        for sim_node in sim_nodes {
            if let Some(node) = self.nodes.get_mut(&sim_node.id) {
                if !node.is_dragged {
                    node.x = sim_node.position.x as f32;
                    node.y = sim_node.position.y as f32;
                    node.vx = sim_node.velocity.x as f32;
                    node.vy = sim_node.velocity.y as f32;

                    // Update view position
                    node.view.update(cx, |v, _| v.set_position(point(px(node.x), px(node.y))));
                }
            }
        }

        cx.notify();
        self.schedule_tick(cx);
    }

    /// Transform screen coordinates to graph coordinates
    fn screen_to_graph(&self, screen_x: f32, screen_y: f32) -> (f32, f32) {
        (
            (screen_x - self.pan_x) / self.zoom,
            (screen_y - self.pan_y) / self.zoom,
        )
    }

    /// Transform graph coordinates to screen coordinates
    fn graph_to_screen(&self, graph_x: f32, graph_y: f32) -> (f32, f32) {
        (
            graph_x * self.zoom + self.pan_x,
            graph_y * self.zoom + self.pan_y,
        )
    }

    /// Handle zooming
    fn handle_zoom(&mut self, delta: f32, center_x: f32, center_y: f32) {
        let old_zoom = self.zoom;
        self.zoom = (self.zoom * (1.0 + delta * 0.1))
            .clamp(self.style.min_zoom, self.style.max_zoom);

        // Zoom toward cursor position
        let zoom_ratio = self.zoom / old_zoom;
        self.pan_x = center_x - (center_x - self.pan_x) * zoom_ratio;
        self.pan_y = center_y - (center_y - self.pan_y) * zoom_ratio;
    }

    /// Find node under cursor
    fn hit_test(&self, screen_x: f32, screen_y: f32) -> Option<String> {
        let (graph_x, graph_y) = self.screen_to_graph(screen_x, screen_y);

        for (id, node) in &self.nodes {
            if graph_x >= node.x
                && graph_x <= node.x + node.width
                && graph_y >= node.y
                && graph_y <= node.y + node.height
            {
                return Some(id.clone());
            }
        }
        None
    }

    /// Select a single node, clearing previous selection
    fn select_node(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        // Deselect all previous
        for old_id in self.selection.selected_vec() {
            if let Some(node) = self.nodes.get(&old_id) {
                node.view.update(cx, |v, _| v.set_selected(false));
            }
        }
        self.selection.clear();

        // Select new
        if let Some(ref new_id) = id {
            self.selection.select(new_id.clone());
            if let Some(node) = self.nodes.get(new_id) {
                node.view.update(cx, |v, _| v.set_selected(true));
            }
            cx.emit(GraphEvent::UnitSelected { id: new_id.clone() });
        } else {
            cx.emit(GraphEvent::SelectionCleared);
        }
    }

    /// Toggle node selection (for Shift+click)
    fn toggle_node_selection(&mut self, id: &str, cx: &mut Context<Self>) {
        let was_selected = self.selection.is_selected(id);
        self.selection.toggle(id);

        if let Some(node) = self.nodes.get(id) {
            node.view.update(cx, |v, _| v.set_selected(!was_selected));
        }

        // Emit appropriate event
        let selected = self.selection.selected_vec();
        if selected.is_empty() {
            cx.emit(GraphEvent::SelectionCleared);
        } else if selected.len() == 1 {
            cx.emit(GraphEvent::UnitSelected { id: selected[0].clone() });
        } else {
            cx.emit(GraphEvent::MultipleSelected { ids: selected });
        }
    }

    /// Select nodes within a rectangle (for rubber band selection)
    fn select_in_rect(&mut self, rect: &SelectionRect, additive: bool, cx: &mut Context<Self>) {
        if !additive {
            // Clear previous selection
            for old_id in self.selection.selected_vec() {
                if let Some(node) = self.nodes.get(&old_id) {
                    node.view.update(cx, |v, _| v.set_selected(false));
                }
            }
            self.selection.clear();
        }

        // Find and select nodes in rectangle (in graph coordinates)
        let (min_x, min_y, max_x, max_y) = rect.bounds();
        let (graph_min_x, graph_min_y) = self.screen_to_graph(min_x, min_y);
        let (graph_max_x, graph_max_y) = self.screen_to_graph(max_x, max_y);

        for (id, node) in &self.nodes {
            // Check if node overlaps with selection rect
            let node_right = node.x + node.width;
            let node_bottom = node.y + node.height;

            if node.x < graph_max_x && node_right > graph_min_x
                && node.y < graph_max_y && node_bottom > graph_min_y
            {
                if !self.selection.is_selected(id) {
                    self.selection.add(id.clone());
                    node.view.update(cx, |v, _| v.set_selected(true));
                }
            }
        }

        // Emit event
        let selected = self.selection.selected_vec();
        if selected.is_empty() {
            cx.emit(GraphEvent::SelectionCleared);
        } else if selected.len() == 1 {
            cx.emit(GraphEvent::UnitSelected { id: selected[0].clone() });
        } else {
            cx.emit(GraphEvent::MultipleSelected { ids: selected });
        }
    }

    /// Clear all selection
    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        for id in self.selection.selected_vec() {
            if let Some(node) = self.nodes.get(&id) {
                node.view.update(cx, |v, _| v.set_selected(false));
            }
        }
        self.selection.clear();
        cx.emit(GraphEvent::SelectionCleared);
    }

    /// Get current selection
    pub fn selected(&self) -> &SelectionManager {
        &self.selection
    }

    /// Set the visual style
    pub fn set_style(&mut self, style: GraphStyle) {
        self.style = style;
    }

    // =========================================================================
    // Accessor Methods (for testing)
    // =========================================================================

    /// Get current zoom level
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Get pan X offset
    pub fn pan_x(&self) -> f32 {
        self.pan_x
    }

    /// Get pan Y offset
    pub fn pan_y(&self) -> f32 {
        self.pan_y
    }

    /// Get number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Check if a node exists
    pub fn has_node(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    /// Get all node IDs
    pub fn node_ids(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }

    /// Get selected node IDs
    pub fn selected_ids(&self) -> Vec<String> {
        self.selection.selected_vec()
    }

    /// Check if a node is selected
    pub fn is_node_selected(&self, id: &str) -> bool {
        self.selection.is_selected(id)
    }

    /// Get selection count
    pub fn selection_count(&self) -> usize {
        self.selection.count()
    }

    /// Check if simulation is running
    pub fn is_simulating(&self) -> bool {
        self.simulating
    }

    /// Get node screen position
    pub fn node_screen_position(&self, id: &str) -> Option<(f32, f32)> {
        self.nodes.get(id).map(|node| self.graph_to_screen(node.x, node.y))
    }

    /// Reset view to default zoom and pan
    pub fn reset_view(&mut self, _cx: &mut Context<Self>) {
        self.zoom = 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
    }

    /// Set zoom level (clamped to valid range)
    pub fn set_zoom(&mut self, zoom: f32, _cx: &mut Context<Self>) {
        self.zoom = zoom.clamp(0.1, 5.0);
    }

    /// Set pan offset
    pub fn set_pan(&mut self, x: f32, y: f32, _cx: &mut Context<Self>) {
        self.pan_x = x;
        self.pan_y = y;
    }

    /// Notify the view for testing purposes
    pub fn notify(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }

    // =========================================================================
    // HUD State Accessors (for testing)
    // =========================================================================

    /// Get current session ID
    pub fn current_session_id(&self) -> Option<String> {
        self.current_session_id.clone()
    }

    /// Get current APM value
    pub fn current_apm(&self) -> f64 {
        self.current_apm
    }

    /// Get current error message
    pub fn current_error(&self) -> Option<String> {
        self.current_error.clone()
    }

    /// Get error count
    pub fn error_count(&self) -> usize {
        self.error_count
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.message_count
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    // =========================================================================
    // APM Widget Accessors
    // =========================================================================

    /// Get the APM state
    pub fn apm_state(&self) -> &ApmState {
        &self.apm_state
    }

    /// Get APM widget entity (if created)
    pub fn apm_widget(&self) -> Option<&Entity<ApmWidget>> {
        self.apm_widget.as_ref()
    }

    /// Check if APM widget is visible
    pub fn is_apm_visible(&self) -> bool {
        self.apm_state.visible
    }

    /// Toggle APM widget visibility
    pub fn toggle_apm_visibility(&mut self, cx: &mut Context<Self>) {
        self.apm_state.toggle_visibility();
        if let Some(ref widget) = self.apm_widget {
            widget.update(cx, |w, cx| {
                w.toggle_visibility(cx);
            });
        }
        cx.notify();
    }

    /// Enable APM widget (creates entity if not present)
    pub fn enable_apm_widget(&mut self, cx: &mut Context<Self>) {
        if self.apm_widget.is_none() {
            let widget = cx.new(|cx| ApmWidget::new(cx));
            // Initialize with current state
            let state = self.apm_state.clone();
            widget.update(cx, |w, cx| {
                w.update_state(state, cx);
            });
            self.apm_widget = Some(widget);
            cx.notify();
        }
    }

    // =========================================================================
    // HUD Message Handling (for testing)
    // =========================================================================

    /// Handle a HUD protocol message
    ///
    /// This method processes HUD messages for testing purposes.
    /// In production, messages come via WebSocket; in tests, they're injected directly.
    pub fn handle_hud_message(&mut self, message: serde_json::Value, cx: &mut Context<Self>) {
        self.message_count += 1;

        // Extract message type
        let msg_type = message.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match msg_type {
            "session_start" => {
                if let Some(session_id) = message.get("sessionId").and_then(|v| v.as_str()) {
                    self.current_session_id = Some(session_id.to_string());
                    self.current_error = None;
                    self.error_count = 0;
                    // Reset APM state for new session
                    self.apm_state = ApmState::new();
                }
            }
            "session_complete" => {
                // Session ended, keep session_id for reference
            }
            "task_selected" => {
                // Could add a node for the task
                if let Some(task) = message.get("task") {
                    if let Some(id) = task.get("id").and_then(|v| v.as_str()) {
                        self.add_task_node(id, task.get("title").and_then(|v| v.as_str()).unwrap_or("Task"), cx);
                    }
                }
            }
            "apm_update" => {
                let session_apm = message.get("sessionAPM").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let recent_apm = message.get("recentAPM").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let total_actions = message.get("totalActions").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let duration_minutes = message.get("durationMinutes").and_then(|v| v.as_f64()).unwrap_or(0.0);

                // Sanitize APM value (legacy field)
                self.current_apm = if session_apm.is_finite() { session_apm } else { 0.0 };

                // Update APM state
                self.apm_state.update_from_message(session_apm, recent_apm, total_actions, duration_minutes);

                // Update APM widget if present
                if let Some(ref widget) = self.apm_widget {
                    widget.update(cx, |w, cx| {
                        w.handle_apm_update(session_apm, recent_apm, total_actions, duration_minutes, cx);
                    });
                }
            }
            "apm_snapshot" => {
                // Handle historical APM snapshot
                if let Some(combined) = message.get("combined") {
                    let snapshot = ApmSnapshot {
                        apm_1h: combined.get("apm1h").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        apm_6h: combined.get("apm6h").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        apm_24h: combined.get("apm1d").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        total_sessions: combined.get("totalSessions").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                        total_actions: combined.get("totalActions").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                    };
                    self.apm_state.update_snapshot(snapshot);
                }

                if let Some(comparison) = message.get("comparison") {
                    let comp = ApmComparison {
                        claude_code_apm: comparison.get("claudeCodeAPM").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        mecha_coder_apm: comparison.get("mechaCoderAPM").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        efficiency_ratio: comparison.get("efficiencyRatio").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    };
                    self.apm_state.update_comparison(comp);
                }

                // Update widget with new state
                if let Some(ref widget) = self.apm_widget {
                    let state = self.apm_state.clone();
                    widget.update(cx, |w, cx| {
                        w.update_state(state, cx);
                    });
                }
            }
            "error" => {
                if let Some(error) = message.get("error").and_then(|v| v.as_str()) {
                    self.current_error = Some(error.to_string());
                    self.error_count += 1;
                }
            }
            _ => {
                // Unknown message type - ignore silently
            }
        }

        cx.notify();
    }

    /// Handle raw/malformed message data (for error handling tests)
    pub fn handle_raw_message(&mut self, data: &str, cx: &mut Context<Self>) {
        self.message_count += 1;

        // Try to parse as JSON
        match serde_json::from_str::<serde_json::Value>(data) {
            Ok(value) => {
                self.handle_hud_message(value, cx);
            }
            Err(_) => {
                // Malformed JSON - record as error but don't crash
                self.current_error = Some(format!("Malformed message: {}", &data[..data.len().min(50)]));
                self.error_count += 1;
                cx.notify();
            }
        }
    }

    /// Handle WebSocket disconnect (for testing)
    pub fn handle_disconnect(&mut self, cx: &mut Context<Self>) {
        self.is_connected = false;
        cx.notify();
    }

    /// Handle WebSocket reconnect (for testing)
    pub fn handle_reconnect(&mut self, cx: &mut Context<Self>) {
        self.is_connected = true;
        cx.notify();
    }

    /// Add a task node (simplified for testing)
    fn add_task_node(&mut self, id: &str, title: &str, cx: &mut Context<Self>) {
        use unit::{Lifecycle, PinState};
        use crate::pin_view::PinSnapshot;

        // Don't add duplicate nodes
        if self.nodes.contains_key(id) {
            return;
        }

        // Position new nodes in a grid pattern
        let node_index = self.nodes.len();
        let x = (node_index % 3) as f32 * 200.0 - 200.0;
        let y = (node_index / 3) as f32 * 150.0;

        let snapshot = UnitSnapshot {
            id: id.to_string(),
            lifecycle: Lifecycle::Playing,
            inputs: vec![],
            outputs: vec![
                PinSnapshot {
                    name: title.to_string(),
                    state: PinState::Valid,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Output,
                    type_name: "Task".to_string(),
                },
            ],
            error: None,
            position: point(px(x), px(y)),
        };

        let size = snapshot.calculate_size(&self.style.unit_style);
        let width: f32 = size.width.into();
        let height: f32 = size.height.into();
        let view = cx.new(|cx| UnitView::new(snapshot, cx));

        self.nodes.insert(id.to_string(), GraphNode {
            view,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            width,
            height,
            is_dragged: false,
        });

        // Start simulation if not running
        if !self.simulating {
            self.start_simulation(cx);
        }
    }
}

impl EventEmitter<GraphEvent> for GraphView {}

impl Focusable for GraphView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl GraphView {
    // =========================================================================
    // Action Handlers (Keyboard Shortcuts)
    // =========================================================================

    /// Handle SelectAll action (Cmd+A)
    fn handle_select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        for (id, node) in &self.nodes {
            if !self.selection.is_selected(id) {
                self.selection.add(id.clone());
                node.view.update(cx, |v, _| v.set_selected(true));
            }
        }
        let selected = self.selection.selected_vec();
        if !selected.is_empty() {
            cx.emit(GraphEvent::MultipleSelected { ids: selected });
        }
        cx.notify();
    }

    /// Handle DeselectAll action (Escape)
    fn handle_deselect_all(&mut self, _: &DeselectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.clear_selection(cx);
    }

    /// Handle ZoomIn action (Cmd+=)
    fn handle_zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        // Zoom toward center of viewport
        let old_zoom = self.zoom;
        self.zoom = (self.zoom * 1.25).clamp(self.style.min_zoom, self.style.max_zoom);
        // Adjust pan to keep center stable (assuming 800x600 viewport)
        let center_x = 400.0;
        let center_y = 300.0;
        let zoom_ratio = self.zoom / old_zoom;
        self.pan_x = center_x - (center_x - self.pan_x) * zoom_ratio;
        self.pan_y = center_y - (center_y - self.pan_y) * zoom_ratio;
        cx.notify();
    }

    /// Handle ZoomOut action (Cmd+-)
    fn handle_zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        let old_zoom = self.zoom;
        self.zoom = (self.zoom / 1.25).clamp(self.style.min_zoom, self.style.max_zoom);
        let center_x = 400.0;
        let center_y = 300.0;
        let zoom_ratio = self.zoom / old_zoom;
        self.pan_x = center_x - (center_x - self.pan_x) * zoom_ratio;
        self.pan_y = center_y - (center_y - self.pan_y) * zoom_ratio;
        cx.notify();
    }

    /// Handle ResetView action (Cmd+0)
    fn handle_reset_view(&mut self, _: &ResetView, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom = 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        cx.notify();
    }

    /// Handle ZoomToFit action
    fn handle_zoom_to_fit(&mut self, _: &ZoomToFit, _: &mut Window, cx: &mut Context<Self>) {
        if self.nodes.is_empty() {
            return;
        }

        // Calculate bounding box of all nodes
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for node in self.nodes.values() {
            min_x = min_x.min(node.x);
            min_y = min_y.min(node.y);
            max_x = max_x.max(node.x + node.width);
            max_y = max_y.max(node.y + node.height);
        }

        // Add padding
        let padding = 50.0;
        min_x -= padding;
        min_y -= padding;
        max_x += padding;
        max_y += padding;

        // Calculate zoom to fit (assuming 800x600 viewport)
        let viewport_width = 800.0;
        let viewport_height = 600.0;
        let content_width = max_x - min_x;
        let content_height = max_y - min_y;

        let zoom_x = viewport_width / content_width;
        let zoom_y = viewport_height / content_height;
        self.zoom = zoom_x.min(zoom_y).clamp(self.style.min_zoom, self.style.max_zoom);

        // Center the content
        let center_x = (min_x + max_x) / 2.0;
        let center_y = (min_y + max_y) / 2.0;
        self.pan_x = viewport_width / 2.0 - center_x * self.zoom;
        self.pan_y = viewport_height / 2.0 - center_y * self.zoom;

        cx.notify();
    }
}

impl Render for GraphView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let zoom = self.zoom;
        let pan_x = self.pan_x;
        let pan_y = self.pan_y;
        let style = self.style.clone();

        // Collect node positions for painting
        let node_positions: Vec<(String, f32, f32)> = self.nodes.iter()
            .map(|(id, node)| (id.clone(), node.x, node.y))
            .collect();

        let sim_connections = self.sim_connections.clone();

        // Capture rubber band state for painting
        let rubber_band = match &self.drag_state {
            DragState::RubberBand { rect, .. } => Some(*rect),
            _ => None,
        };

        div()
            .size_full()
            .bg(style.background)
            .track_focus(&self.focus_handle)
            // Action handlers for keyboard shortcuts
            .on_action(cx.listener(Self::handle_select_all))
            .on_action(cx.listener(Self::handle_deselect_all))
            .on_action(cx.listener(Self::handle_zoom_in))
            .on_action(cx.listener(Self::handle_zoom_out))
            .on_action(cx.listener(Self::handle_reset_view))
            .on_action(cx.listener(Self::handle_zoom_to_fit))
            .child(
                canvas(
                    move |bounds, _window, _cx| bounds,
                    move |bounds, _, window, _cx| {
                        // Paint grid
                        if style.show_grid {
                            let spacing = style.grid_spacing * zoom;
                            let offset_x = pan_x % spacing;
                            let offset_y = pan_y % spacing;
                            let color = style.grid_color;

                            let bounds_width: f32 = bounds.size.width.into();
                            let bounds_height: f32 = bounds.size.height.into();

                            let mut x = offset_x;
                            while x < bounds_width {
                                let mut builder = PathBuilder::stroke(px(1.0));
                                builder.move_to(point(px(x), px(0.0)));
                                builder.line_to(point(px(x), bounds.size.height));
                                if let Ok(path) = builder.build() {
                                    window.paint_path(path, color);
                                }
                                x += spacing;
                            }

                            let mut y = offset_y;
                            while y < bounds_height {
                                let mut builder = PathBuilder::stroke(px(1.0));
                                builder.move_to(point(px(0.0), px(y)));
                                builder.line_to(point(bounds.size.width, px(y)));
                                if let Ok(path) = builder.build() {
                                    window.paint_path(path, color);
                                }
                                y += spacing;
                            }
                        }

                        // Paint connections
                        let connection_style = &style.connection_style;
                        for sim_conn in &sim_connections {
                            if let (Some(from_pos), Some(to_pos)) = (
                                node_positions.iter().find(|(id, _, _)| id == &sim_conn.from).map(|(_, x, y)| (*x, *y)),
                                node_positions.iter().find(|(id, _, _)| id == &sim_conn.to).map(|(_, x, y)| (*x, *y)),
                            ) {
                                let screen_from_x = from_pos.0 * zoom + pan_x;
                                let screen_from_y = from_pos.1 * zoom + pan_y;
                                let screen_to_x = to_pos.0 * zoom + pan_x;
                                let screen_to_y = to_pos.1 * zoom + pan_y;

                                let conn = Connection::new(
                                    point(px(screen_from_x), px(screen_from_y)),
                                    point(px(screen_to_x), px(screen_to_y)),
                                ).with_state(ConnectionState::Active);
                                conn.paint(connection_style, window);
                            }
                        }

                        // Paint rubber band selection rectangle
                        if let Some(rect) = rubber_band {
                            let (min_x, min_y, max_x, max_y) = rect.bounds();

                            // Draw filled rectangle with low opacity
                            let fill_color = hud::RUBBER_BAND_FILL;
                            let stroke_color = hud::RUBBER_BAND_STROKE;

                            // Fill
                            let mut fill_builder = PathBuilder::fill();
                            fill_builder.move_to(point(px(min_x), px(min_y)));
                            fill_builder.line_to(point(px(max_x), px(min_y)));
                            fill_builder.line_to(point(px(max_x), px(max_y)));
                            fill_builder.line_to(point(px(min_x), px(max_y)));
                            fill_builder.close();
                            if let Ok(path) = fill_builder.build() {
                                window.paint_path(path, fill_color);
                            }

                            // Stroke
                            let mut stroke_builder = PathBuilder::stroke(px(1.0));
                            stroke_builder.move_to(point(px(min_x), px(min_y)));
                            stroke_builder.line_to(point(px(max_x), px(min_y)));
                            stroke_builder.line_to(point(px(max_x), px(max_y)));
                            stroke_builder.line_to(point(px(min_x), px(max_y)));
                            stroke_builder.close();
                            if let Ok(path) = stroke_builder.build() {
                                window.paint_path(path, stroke_color);
                            }
                        }
                    },
                )
                .size_full()
            )
            // Render unit nodes on top
            .children(self.nodes.values().map(|node| {
                let (screen_x, screen_y) = self.graph_to_screen(node.x, node.y);

                div()
                    .absolute()
                    .left(px(screen_x))
                    .top(px(screen_y))
                    .child(node.view.clone())
            }))
            // Render APM widget overlay (if enabled)
            .when_some(self.apm_widget.clone(), |this, widget| {
                this.child(widget)
            })
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                let delta = event.delta.pixel_delta(px(1.0));
                let delta_x: f32 = delta.x.into();
                let delta_y: f32 = delta.y.into();
                let pos_x: f32 = event.position.x.into();
                let pos_y: f32 = event.position.y.into();

                if event.modifiers.platform {
                    // Zoom with cmd+scroll
                    this.handle_zoom(-delta_y * 0.01, pos_x, pos_y);
                } else {
                    // Pan with scroll
                    this.pan_x += delta_x;
                    this.pan_y += delta_y;
                }
                cx.notify();
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                let pos_x: f32 = event.position.x.into();
                let pos_y: f32 = event.position.y.into();
                let shift_held = event.modifiers.shift;
                let hit = this.hit_test(pos_x, pos_y);

                if let Some(id) = hit {
                    if shift_held {
                        // Shift+click: toggle selection
                        this.toggle_node_selection(&id, cx);
                    } else if this.selection.is_selected(&id) {
                        // Clicking already selected node: prepare to drag selection
                        // Keep current selection
                    } else {
                        // Clicking unselected node: select it only
                        this.select_node(Some(id.clone()), cx);
                    }

                    // Start dragging the clicked node
                    let (graph_x, graph_y) = this.screen_to_graph(pos_x, pos_y);
                    if let Some(node) = this.nodes.get_mut(&id) {
                        let offset_x = graph_x - node.x;
                        let offset_y = graph_y - node.y;
                        node.is_dragged = true;
                        this.drag_state = DragState::DraggingUnit {
                            unit_id: id,
                            offset_x,
                            offset_y,
                        };
                    }
                } else {
                    // Clicked empty space
                    if shift_held {
                        // Shift+drag: additive rubber band selection
                        let rect = SelectionRect::new(pos_x, pos_y);
                        this.drag_state = DragState::RubberBand { rect, additive: true };
                    } else if event.modifiers.platform {
                        // Cmd+drag: pan the canvas
                        this.drag_state = DragState::Panning { start_x: pos_x, start_y: pos_y };
                    } else {
                        // Regular drag: rubber band selection (clears existing)
                        this.clear_selection(cx);
                        let rect = SelectionRect::new(pos_x, pos_y);
                        this.drag_state = DragState::RubberBand { rect, additive: false };
                    }
                }
                cx.notify();
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                let pos_x: f32 = event.position.x.into();
                let pos_y: f32 = event.position.y.into();

                match &this.drag_state {
                    DragState::Panning { start_x, start_y } => {
                        let delta_x = pos_x - start_x;
                        let delta_y = pos_y - start_y;
                        this.pan_x += delta_x;
                        this.pan_y += delta_y;
                        this.drag_state = DragState::Panning { start_x: pos_x, start_y: pos_y };
                        cx.notify();
                    }
                    DragState::DraggingUnit { unit_id, offset_x, offset_y } => {
                        let (graph_x, graph_y) = this.screen_to_graph(pos_x, pos_y);
                        let new_x = graph_x - offset_x;
                        let new_y = graph_y - offset_y;
                        let unit_id = unit_id.clone();
                        let offset_x = *offset_x;
                        let offset_y = *offset_y;

                        if let Some(node) = this.nodes.get_mut(&unit_id) {
                            node.x = new_x;
                            node.y = new_y;
                            node.view.update(cx, |v, _| v.set_position(point(px(new_x), px(new_y))));
                        }
                        this.drag_state = DragState::DraggingUnit { unit_id, offset_x, offset_y };
                        cx.notify();
                    }
                    DragState::RubberBand { rect, additive } => {
                        let mut new_rect = *rect;
                        new_rect.update(pos_x, pos_y);
                        let additive = *additive;
                        this.drag_state = DragState::RubberBand { rect: new_rect, additive };
                        cx.notify();
                    }
                    DragState::None => {}
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _event: &MouseUpEvent, _, cx| {
                // Clone data we need from drag_state before mutating
                let drag_action = match &this.drag_state {
                    DragState::DraggingUnit { unit_id, .. } => Some((true, unit_id.clone(), None)),
                    DragState::RubberBand { rect, additive } => Some((false, String::new(), Some((*rect, *additive)))),
                    DragState::Panning { .. } | DragState::None => None,
                };

                if let Some((is_unit_drag, unit_id, rubber_band)) = drag_action {
                    if is_unit_drag {
                        // Release dragged node
                        if let Some(node) = this.nodes.get_mut(&unit_id) {
                            node.is_dragged = false;
                        }
                        // Restart simulation to settle
                        this.start_simulation(cx);
                    } else if let Some((rect, additive)) = rubber_band {
                        // Finalize rubber band selection
                        this.select_in_rect(&rect, additive, cx);
                    }
                }

                this.drag_state = DragState::None;
                cx.notify();
            }))
    }
}

/// Create a GraphView entity
pub fn graph_view(cx: &mut App) -> Entity<GraphView> {
    cx.new(|cx| GraphView::new(cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_style_defaults() {
        let style = GraphStyle::default();
        assert!(style.min_zoom > 0.0);
        assert!(style.max_zoom > style.min_zoom);
    }

    #[test]
    fn test_coordinate_transform() {
        let pan_x = 100.0;
        let pan_y = 50.0;
        let zoom = 2.0f32;
        let screen_x = 200.0;
        let screen_y = 150.0;

        let graph_x = (screen_x - pan_x) / zoom;
        let graph_y = (screen_y - pan_y) / zoom;

        assert_eq!(graph_x, 50.0);
        assert_eq!(graph_y, 50.0);
    }
}
