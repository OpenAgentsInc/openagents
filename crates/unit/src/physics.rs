//! Physics simulation for force-directed graph layout
//!
//! Implements a simple force-directed layout algorithm with:
//! - Repulsive forces between all nodes (like electric charges)
//! - Attractive forces along connections (like springs)
//! - Velocity damping and alpha cooling

use crate::geometry::{Point, Thing, surface_distance};

/// Simulation node
#[derive(Debug, Clone)]
pub struct SimNode {
    /// Unique identifier
    pub id: String,
    /// Current position
    pub position: Point,
    /// Current velocity
    pub velocity: Point,
    /// Accumulated acceleration (reset each frame)
    pub acceleration: Point,
    /// Fixed position (if Some, node doesn't move)
    pub fixed: Option<Point>,
    /// Node shape/size for collision
    pub thing: Thing,
}

impl SimNode {
    /// Create a new simulation node
    pub fn new(id: impl Into<String>, x: f64, y: f64, radius: f64) -> Self {
        Self {
            id: id.into(),
            position: Point::new(x, y),
            velocity: Point::zero(),
            acceleration: Point::zero(),
            fixed: None,
            thing: Thing::circle(x, y, radius),
        }
    }

    /// Create a node with a rectangle shape
    pub fn new_rect(id: impl Into<String>, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            id: id.into(),
            position: Point::new(x, y),
            velocity: Point::zero(),
            acceleration: Point::zero(),
            fixed: None,
            thing: Thing::rect(x, y, width, height),
        }
    }

    /// Fix the node at its current position
    pub fn fix(&mut self) {
        self.fixed = Some(self.position);
    }

    /// Unfix the node
    pub fn unfix(&mut self) {
        self.fixed = None;
    }

    /// Check if node is fixed
    pub fn is_fixed(&self) -> bool {
        self.fixed.is_some()
    }

    /// Apply a force to the node
    pub fn apply_force(&mut self, force: Point) {
        self.acceleration = self.acceleration + force;
    }

    /// Update thing position to match node position
    fn sync_thing(&mut self) {
        self.thing.x = self.position.x;
        self.thing.y = self.position.y;
    }
}

/// Connection between two nodes
#[derive(Debug, Clone)]
pub struct SimConnection {
    /// Source node ID
    pub from: String,
    /// Target node ID
    pub to: String,
    /// Optional connection strength (defaults to 1.0)
    pub strength: f64,
}

impl SimConnection {
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            strength: 1.0,
        }
    }

    pub fn with_strength(mut self, strength: f64) -> Self {
        self.strength = strength;
        self
    }
}

/// Simulation configuration
#[derive(Debug, Clone)]
pub struct SimulationConfig {
    /// Strength of repulsive force between nodes
    pub repulsion_strength: f64,
    /// Target distance for connected nodes
    pub link_distance: f64,
    /// Strength of link attraction
    pub link_strength: f64,
    /// Velocity damping (0-1, lower = more damping)
    pub damping: f64,
    /// Current simulation temperature (decreases over time)
    pub alpha: f64,
    /// Target alpha (usually 0)
    pub alpha_target: f64,
    /// How fast alpha decays (0-1)
    pub alpha_decay: f64,
    /// Minimum alpha before simulation stops
    pub alpha_min: f64,
    /// Center gravity strength
    pub center_strength: f64,
    /// Center point
    pub center: Point,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            repulsion_strength: 300.0,
            link_distance: 100.0,
            link_strength: 0.5,
            damping: 0.9,
            alpha: 1.0,
            alpha_target: 0.0,
            alpha_decay: 0.02,
            alpha_min: 0.001,
            center_strength: 0.05,
            center: Point::zero(),
        }
    }
}

impl SimulationConfig {
    /// Create config optimized for small graphs
    pub fn small_graph() -> Self {
        Self {
            repulsion_strength: 200.0,
            link_distance: 80.0,
            ..Default::default()
        }
    }

    /// Create config optimized for large graphs
    pub fn large_graph() -> Self {
        Self {
            repulsion_strength: 500.0,
            link_distance: 150.0,
            alpha_decay: 0.01,
            ..Default::default()
        }
    }
}

/// Apply forces to all nodes
///
/// This calculates:
/// 1. Repulsive forces between all node pairs
/// 2. Attractive forces along connections
/// 3. Center gravity
pub fn apply_forces(
    nodes: &mut [SimNode],
    connections: &[SimConnection],
    config: &SimulationConfig,
) {
    let alpha = config.alpha;
    if alpha <= 0.0 {
        return;
    }

    // Reset accelerations
    for node in nodes.iter_mut() {
        node.acceleration = Point::zero();
    }

    // Apply repulsive forces between all node pairs
    let n = nodes.len();
    for i in 0..n {
        for j in (i + 1)..n {
            // Calculate surface distance
            let result = surface_distance(&nodes[i].thing, &nodes[j].thing);
            let d = result.center_distance.max(1.0); // Avoid division by zero
            let u = result.direction;

            // Repulsive force (inverse square law)
            let force_magnitude = config.repulsion_strength * alpha / (d * d);
            let force = u * force_magnitude;

            // Apply equal and opposite forces
            // Note: We need to do this carefully to avoid borrow issues
            let (left, right) = nodes.split_at_mut(j);
            left[i].apply_force(force * -1.0);
            right[0].apply_force(force);
        }
    }

    // Build ID -> index map for connections (rebuild to avoid borrow issues)
    let id_to_idx: std::collections::HashMap<String, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.clone(), i))
        .collect();

    // Apply attractive forces along connections
    for conn in connections {
        let from_idx = match id_to_idx.get(&conn.from) {
            Some(&idx) => idx,
            None => continue,
        };
        let to_idx = match id_to_idx.get(&conn.to) {
            Some(&idx) => idx,
            None => continue,
        };

        if from_idx == to_idx {
            continue;
        }

        // Calculate distance and direction
        let from_pos = nodes[from_idx].position;
        let to_pos = nodes[to_idx].position;
        let d = from_pos.distance_to(&to_pos).max(1.0);
        let u = from_pos.unit_vector_to(&to_pos);

        // Spring force (Hooke's law)
        let displacement = d - config.link_distance;
        let force_magnitude = displacement * config.link_strength * conn.strength * alpha;
        let force = u * force_magnitude;

        // Apply to both nodes
        let (min_idx, max_idx) = if from_idx < to_idx {
            (from_idx, to_idx)
        } else {
            (to_idx, from_idx)
        };
        let (left, right) = nodes.split_at_mut(max_idx);
        left[min_idx].apply_force(if from_idx < to_idx {
            force
        } else {
            force * -1.0
        });
        right[0].apply_force(if from_idx < to_idx {
            force * -1.0
        } else {
            force
        });
    }

    // Apply center gravity
    if config.center_strength > 0.0 {
        for node in nodes.iter_mut() {
            let to_center = node.position.unit_vector_to(&config.center);
            let d = node.position.distance_to(&config.center);
            let force = to_center * (d * config.center_strength * alpha);
            node.apply_force(force);
        }
    }
}

/// Integrate positions using Euler integration
///
/// Updates velocity based on acceleration, then position based on velocity.
/// Applies damping and respects fixed nodes.
pub fn integrate(nodes: &mut [SimNode], dt: f64, config: &SimulationConfig) {
    for node in nodes.iter_mut() {
        if node.is_fixed() {
            // Fixed nodes don't move
            if let Some(fixed_pos) = node.fixed {
                node.position = fixed_pos;
                node.velocity = Point::zero();
            }
            continue;
        }

        // Update velocity: v += a * dt
        node.velocity = node.velocity + node.acceleration * dt;

        // Apply damping
        node.velocity = node.velocity * config.damping;

        // Update position: p += v * dt
        node.position = node.position + node.velocity * dt;

        // Sync thing position
        node.sync_thing();

        // Clear acceleration for next frame
        node.acceleration = Point::zero();
    }
}

/// Check if simulation should stop
pub fn should_stop(config: &SimulationConfig) -> bool {
    config.alpha < config.alpha_min
}

/// Update alpha (cooling)
pub fn cool(config: &mut SimulationConfig) {
    config.alpha += (config.alpha_target - config.alpha) * config.alpha_decay;
}

/// Reset simulation to initial temperature
pub fn reheat(config: &mut SimulationConfig) {
    config.alpha = 1.0;
}

/// Run one simulation tick
pub fn tick(nodes: &mut [SimNode], connections: &[SimConnection], config: &mut SimulationConfig) {
    if should_stop(config) {
        return;
    }

    apply_forces(nodes, connections, config);
    integrate(nodes, 1.0, config);
    cool(config);
}

/// Run simulation until it settles
pub fn run_until_settled(
    nodes: &mut [SimNode],
    connections: &[SimConnection],
    config: &mut SimulationConfig,
    max_iterations: usize,
) -> usize {
    let mut iterations = 0;
    while !should_stop(config) && iterations < max_iterations {
        tick(nodes, connections, config);
        iterations += 1;
    }
    iterations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sim_node_creation() {
        let node = SimNode::new("test", 100.0, 200.0, 20.0);
        assert_eq!(node.id, "test");
        assert_eq!(node.position.x, 100.0);
        assert_eq!(node.position.y, 200.0);
        assert!(!node.is_fixed());
    }

    #[test]
    fn test_sim_node_fix() {
        let mut node = SimNode::new("test", 100.0, 200.0, 20.0);
        node.fix();
        assert!(node.is_fixed());

        node.unfix();
        assert!(!node.is_fixed());
    }

    #[test]
    fn test_apply_forces_repulsion() {
        let mut nodes = vec![
            SimNode::new("a", 0.0, 0.0, 10.0),
            SimNode::new("b", 50.0, 0.0, 10.0),
        ];
        let connections = vec![];
        // Disable center gravity so we can test pure repulsion
        let config = SimulationConfig {
            center_strength: 0.0,
            ..Default::default()
        };

        apply_forces(&mut nodes, &connections, &config);

        // Nodes should be pushed apart
        assert!(nodes[0].acceleration.x < 0.0); // a pushed left
        assert!(nodes[1].acceleration.x > 0.0); // b pushed right
    }

    #[test]
    fn test_apply_forces_attraction() {
        let mut nodes = vec![
            SimNode::new("a", 0.0, 0.0, 10.0),
            SimNode::new("b", 200.0, 0.0, 10.0), // Far apart
        ];
        let connections = vec![SimConnection::new("a", "b")];
        let config = SimulationConfig {
            repulsion_strength: 0.0, // Disable repulsion for this test
            ..Default::default()
        };

        apply_forces(&mut nodes, &connections, &config);

        // Nodes should be pulled together
        assert!(nodes[0].acceleration.x > 0.0); // a pulled right
        assert!(nodes[1].acceleration.x < 0.0); // b pulled left
    }

    #[test]
    fn test_integrate() {
        let mut nodes = vec![SimNode::new("a", 0.0, 0.0, 10.0)];
        nodes[0].acceleration = Point::new(10.0, 0.0);

        let config = SimulationConfig {
            damping: 1.0, // No damping for predictable test
            ..Default::default()
        };

        integrate(&mut nodes, 1.0, &config);

        assert!(nodes[0].velocity.x > 0.0);
        assert!(nodes[0].position.x > 0.0);
    }

    #[test]
    fn test_fixed_node() {
        let mut nodes = vec![SimNode::new("a", 100.0, 100.0, 10.0)];
        nodes[0].fix();
        nodes[0].acceleration = Point::new(100.0, 100.0);

        let config = SimulationConfig::default();
        integrate(&mut nodes, 1.0, &config);

        // Fixed node shouldn't move
        assert_eq!(nodes[0].position.x, 100.0);
        assert_eq!(nodes[0].position.y, 100.0);
    }

    #[test]
    fn test_simulation_cools() {
        let mut config = SimulationConfig::default();
        assert_eq!(config.alpha, 1.0);

        for _ in 0..100 {
            cool(&mut config);
        }

        assert!(config.alpha < 0.5);
    }

    #[test]
    fn test_run_until_settled() {
        let mut nodes = vec![
            SimNode::new("a", 0.0, 0.0, 10.0),
            SimNode::new("b", 10.0, 0.0, 10.0),
        ];
        let connections = vec![];
        let mut config = SimulationConfig::default();

        let iterations = run_until_settled(&mut nodes, &connections, &mut config, 1000);

        assert!(iterations > 0);
        assert!(should_stop(&config));
    }
}
