//! GraphView: Full graph canvas with physics-based layout
//!
//! Renders a complete Unit graph with:
//! - Physics simulation for force-directed layout
//! - Pan and zoom navigation
//! - Node selection and dragging
//! - Connection rendering with bezier curves
//! - Animation loop for smooth physics updates

use gpui::{
    App, Context, Entity, EventEmitter, Hsla, MouseButton, MouseMoveEvent,
    MouseUpEvent, PathBuilder, Render, ScrollWheelEvent, Window,
    canvas, div, hsla, point, prelude::*, px,
};
use std::collections::HashMap;
use std::time::Duration;

use unit::{Point as UnitPoint, Graph, SimConnection, SimNode, SimulationConfig, tick, should_stop, reheat};

use crate::connection::{Connection, ConnectionState, ConnectionStyle};
use crate::pin_view::PinDirection;
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
            background: hsla(0.0, 0.0, 0.05, 1.0),   // Near black
            grid_color: hsla(0.0, 0.0, 0.1, 1.0),    // Subtle grid
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
    /// Selection was cleared
    SelectionCleared,
    /// A unit was moved
    UnitMoved { id: String, x: f32, y: f32 },
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
    /// Dragging a unit
    DraggingUnit {
        unit_id: String,
        offset_x: f32,
        offset_y: f32,
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
    /// Selected node ID
    selected: Option<String>,
    /// Visual style
    style: GraphStyle,
    /// Whether simulation is running
    simulating: bool,
}

impl GraphView {
    /// Create a new empty GraphView
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            graph: None,
            nodes: HashMap::new(),
            sim_connections: Vec::new(),
            sim_config: SimulationConfig::default(),
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
            drag_state: DragState::None,
            selected: None,
            style: GraphStyle::default(),
            simulating: false,
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

    /// Select a node
    fn select_node(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        // Deselect previous
        if let Some(ref old_id) = self.selected {
            if let Some(node) = self.nodes.get(old_id) {
                node.view.update(cx, |v, _| v.set_selected(false));
            }
        }

        // Select new
        if let Some(ref new_id) = id {
            if let Some(node) = self.nodes.get(new_id) {
                node.view.update(cx, |v, _| v.set_selected(true));
            }
            cx.emit(GraphEvent::UnitSelected { id: new_id.clone() });
        } else {
            cx.emit(GraphEvent::SelectionCleared);
        }

        self.selected = id;
    }
}

impl EventEmitter<GraphEvent> for GraphView {}

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

        div()
            .size_full()
            .bg(style.background)
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
                let hit = this.hit_test(pos_x, pos_y);

                if let Some(id) = hit {
                    // Start dragging node
                    this.select_node(Some(id.clone()), cx);
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
                    // Start panning
                    this.select_node(None, cx);
                    this.drag_state = DragState::Panning { start_x: pos_x, start_y: pos_y };
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
                    DragState::None => {}
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _event: &MouseUpEvent, _, cx| {
                // Release any dragged node
                if let DragState::DraggingUnit { ref unit_id, .. } = this.drag_state {
                    if let Some(node) = this.nodes.get_mut(unit_id) {
                        node.is_dragged = false;
                    }
                    // Restart simulation to settle
                    this.start_simulation(cx);
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
