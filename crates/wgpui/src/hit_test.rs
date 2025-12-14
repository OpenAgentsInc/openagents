//! Hit testing for wgpui.
//!
//! This module provides hit testing functionality to determine which
//! UI elements are under a given screen coordinate.

use crate::geometry::{Bounds, Point};

/// Unique identifier for hit-testable nodes.
pub type NodeId = u64;

/// A hit test entry representing a node that can receive input.
#[derive(Debug, Clone)]
pub struct HitTestEntry {
    /// Unique node identifier.
    pub node_id: NodeId,
    /// Bounds of the node in screen coordinates.
    pub bounds: Bounds,
    /// Z-order index (higher = on top).
    pub z_index: u32,
    /// Whether this node captures mouse events (prevents propagation).
    pub captures_mouse: bool,
    /// User data for associating custom information.
    pub user_data: u64,
}

/// Result of a hit test.
#[derive(Debug, Clone)]
pub struct Hit {
    /// Node that was hit.
    pub node_id: NodeId,
    /// Point relative to the node's origin.
    pub local_point: Point,
    /// The entry that was hit.
    pub entry: HitTestEntry,
}

/// Index for efficient hit testing.
#[derive(Default)]
pub struct HitTestIndex {
    /// Entries sorted by z-index (highest first).
    entries: Vec<HitTestEntry>,
    /// Counter for generating unique node IDs.
    next_id: NodeId,
}

impl HitTestIndex {
    /// Create a new empty hit test index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Generate a new unique node ID.
    pub fn next_node_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Reset the node ID counter (call at start of each frame).
    pub fn reset_ids(&mut self) {
        self.next_id = 0;
    }

    /// Add a hit test entry.
    pub fn add(&mut self, entry: HitTestEntry) {
        self.entries.push(entry);
    }

    /// Add a hit test entry with auto-generated ID.
    pub fn add_bounds(&mut self, bounds: Bounds, z_index: u32) -> NodeId {
        let node_id = self.next_node_id();
        self.entries.push(HitTestEntry {
            node_id,
            bounds,
            z_index,
            captures_mouse: true,
            user_data: 0,
        });
        node_id
    }

    /// Add a non-capturing entry (allows events to pass through).
    pub fn add_bounds_passthrough(&mut self, bounds: Bounds, z_index: u32) -> NodeId {
        let node_id = self.next_node_id();
        self.entries.push(HitTestEntry {
            node_id,
            bounds,
            z_index,
            captures_mouse: false,
            user_data: 0,
        });
        node_id
    }

    /// Sort entries by z-index (should be called before hit testing).
    pub fn finalize(&mut self) {
        // Sort by z-index descending (highest first)
        self.entries.sort_by(|a, b| b.z_index.cmp(&a.z_index));
    }

    /// Perform a hit test at the given point.
    ///
    /// Returns the topmost (highest z-index) entry that contains the point.
    pub fn hit_test(&self, point: Point) -> Option<Hit> {
        for entry in &self.entries {
            if entry.bounds.contains(point) {
                let local_point = Point::new(
                    point.x - entry.bounds.origin.x,
                    point.y - entry.bounds.origin.y,
                );
                return Some(Hit {
                    node_id: entry.node_id,
                    local_point,
                    entry: entry.clone(),
                });
            }
        }
        None
    }

    /// Perform a hit test and return all entries under the point.
    ///
    /// Results are sorted by z-index (highest first).
    pub fn hit_test_all(&self, point: Point) -> Vec<Hit> {
        let mut hits = Vec::new();
        for entry in &self.entries {
            if entry.bounds.contains(point) {
                let local_point = Point::new(
                    point.x - entry.bounds.origin.x,
                    point.y - entry.bounds.origin.y,
                );
                hits.push(Hit {
                    node_id: entry.node_id,
                    local_point,
                    entry: entry.clone(),
                });
            }
        }
        hits
    }

    /// Find an entry by node ID.
    pub fn find(&self, node_id: NodeId) -> Option<&HitTestEntry> {
        self.entries.iter().find(|e| e.node_id == node_id)
    }

    /// Get number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = &HitTestEntry> {
        self.entries.iter()
    }
}

/// Builder for creating hit test entries.
pub struct HitTestEntryBuilder {
    entry: HitTestEntry,
}

impl HitTestEntryBuilder {
    /// Create a new builder.
    pub fn new(node_id: NodeId, bounds: Bounds) -> Self {
        Self {
            entry: HitTestEntry {
                node_id,
                bounds,
                z_index: 0,
                captures_mouse: true,
                user_data: 0,
            },
        }
    }

    /// Set z-index.
    pub fn z_index(mut self, z: u32) -> Self {
        self.entry.z_index = z;
        self
    }

    /// Set whether this entry captures mouse events.
    pub fn captures_mouse(mut self, captures: bool) -> Self {
        self.entry.captures_mouse = captures;
        self
    }

    /// Set user data.
    pub fn user_data(mut self, data: u64) -> Self {
        self.entry.user_data = data;
        self
    }

    /// Build the entry.
    pub fn build(self) -> HitTestEntry {
        self.entry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hit_test_basic() {
        let mut index = HitTestIndex::new();

        index.add_bounds(Bounds::new(0.0, 0.0, 100.0, 100.0), 0);
        index.add_bounds(Bounds::new(50.0, 50.0, 100.0, 100.0), 1);
        index.finalize();

        // Point in overlap - should hit higher z-index
        let hit = index.hit_test(Point::new(75.0, 75.0));
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().node_id, 1);

        // Point only in first rect
        let hit = index.hit_test(Point::new(25.0, 25.0));
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().node_id, 0);

        // Point outside both
        let hit = index.hit_test(Point::new(200.0, 200.0));
        assert!(hit.is_none());
    }

    #[test]
    fn test_hit_test_all() {
        let mut index = HitTestIndex::new();

        index.add_bounds(Bounds::new(0.0, 0.0, 100.0, 100.0), 0);
        index.add_bounds(Bounds::new(50.0, 50.0, 100.0, 100.0), 1);
        index.finalize();

        // Point in overlap - should return both
        let hits = index.hit_test_all(Point::new(75.0, 75.0));
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].node_id, 1); // Higher z-index first
        assert_eq!(hits[1].node_id, 0);
    }

    #[test]
    fn test_local_point() {
        let mut index = HitTestIndex::new();

        index.add_bounds(Bounds::new(100.0, 100.0, 50.0, 50.0), 0);
        index.finalize();

        let hit = index.hit_test(Point::new(125.0, 125.0)).unwrap();
        assert_eq!(hit.local_point.x, 25.0);
        assert_eq!(hit.local_point.y, 25.0);
    }
}
