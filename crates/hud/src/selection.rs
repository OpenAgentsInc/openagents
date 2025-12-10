//! Selection: Multi-select management for graph nodes
//!
//! Provides selection state tracking with support for:
//! - Single selection
//! - Multi-select with Shift+click
//! - Rubber band selection
//! - Select all / clear

use std::collections::HashSet;

/// Manages selection state for graph elements
#[derive(Debug, Clone, Default)]
pub struct SelectionManager {
    /// Currently selected node IDs
    selected: HashSet<String>,
    /// Primary selection (last selected, for property panel)
    primary: Option<String>,
}

impl SelectionManager {
    /// Create a new empty selection manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Select a single node, clearing previous selection
    pub fn select(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.selected.clear();
        self.selected.insert(id.clone());
        self.primary = Some(id);
    }

    /// Toggle selection of a node (for Shift+click)
    pub fn toggle(&mut self, id: impl Into<String>) {
        let id = id.into();
        if self.selected.contains(&id) {
            self.selected.remove(&id);
            // Update primary if we removed it
            if self.primary.as_ref() == Some(&id) {
                self.primary = self.selected.iter().next().cloned();
            }
        } else {
            self.selected.insert(id.clone());
            self.primary = Some(id);
        }
    }

    /// Add to selection without clearing (for Shift+click or rubber band)
    pub fn add(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.selected.insert(id.clone());
        if self.primary.is_none() {
            self.primary = Some(id);
        }
    }

    /// Add multiple nodes to selection
    pub fn add_many(&mut self, ids: impl IntoIterator<Item = impl Into<String>>) {
        for id in ids {
            self.add(id);
        }
    }

    /// Remove a node from selection
    pub fn remove(&mut self, id: &str) {
        self.selected.remove(id);
        if self.primary.as_ref().map(|s| s.as_str()) == Some(id) {
            self.primary = self.selected.iter().next().cloned();
        }
    }

    /// Clear all selection
    pub fn clear(&mut self) {
        self.selected.clear();
        self.primary = None;
    }

    /// Set selection to exactly these nodes
    pub fn set(&mut self, ids: impl IntoIterator<Item = impl Into<String>>) {
        self.selected.clear();
        for id in ids {
            let id = id.into();
            self.selected.insert(id.clone());
            if self.primary.is_none() {
                self.primary = Some(id);
            }
        }
    }

    /// Check if a node is selected
    pub fn is_selected(&self, id: &str) -> bool {
        self.selected.contains(id)
    }

    /// Check if selection is empty
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// Get the number of selected nodes
    pub fn count(&self) -> usize {
        self.selected.len()
    }

    /// Get the primary (last selected) node
    pub fn primary(&self) -> Option<&str> {
        self.primary.as_deref()
    }

    /// Get all selected node IDs
    pub fn selected(&self) -> &HashSet<String> {
        &self.selected
    }

    /// Get selected IDs as a vector (for iteration)
    pub fn selected_vec(&self) -> Vec<String> {
        self.selected.iter().cloned().collect()
    }

    /// Select all nodes from a list
    pub fn select_all(&mut self, all_ids: impl IntoIterator<Item = impl Into<String>>) {
        self.set(all_ids);
    }
}

/// Rectangle for rubber band selection
#[derive(Debug, Clone, Copy)]
pub struct SelectionRect {
    /// Start x (where mouse down occurred)
    pub start_x: f32,
    /// Start y
    pub start_y: f32,
    /// Current x (where mouse is now)
    pub end_x: f32,
    /// Current y
    pub end_y: f32,
}

impl SelectionRect {
    /// Create a new selection rectangle
    pub fn new(start_x: f32, start_y: f32) -> Self {
        Self {
            start_x,
            start_y,
            end_x: start_x,
            end_y: start_y,
        }
    }

    /// Update the end point
    pub fn update(&mut self, x: f32, y: f32) {
        self.end_x = x;
        self.end_y = y;
    }

    /// Get normalized bounds (min_x, min_y, max_x, max_y)
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        let min_x = self.start_x.min(self.end_x);
        let min_y = self.start_y.min(self.end_y);
        let max_x = self.start_x.max(self.end_x);
        let max_y = self.start_y.max(self.end_y);
        (min_x, min_y, max_x, max_y)
    }

    /// Check if a rectangle intersects with this selection rect
    pub fn intersects(&self, x: f32, y: f32, width: f32, height: f32) -> bool {
        let (min_x, min_y, max_x, max_y) = self.bounds();

        // Check if rectangles overlap
        x < max_x && x + width > min_x && y < max_y && y + height > min_y
    }

    /// Check if a point is inside the selection rect
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        let (min_x, min_y, max_x, max_y) = self.bounds();
        x >= min_x && x <= max_x && y >= min_y && y <= max_y
    }

    /// Get width of selection rect
    pub fn width(&self) -> f32 {
        (self.end_x - self.start_x).abs()
    }

    /// Get height of selection rect
    pub fn height(&self) -> f32 {
        (self.end_y - self.start_y).abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_single() {
        let mut sel = SelectionManager::new();

        sel.select("node1");
        assert!(sel.is_selected("node1"));
        assert_eq!(sel.count(), 1);
        assert_eq!(sel.primary(), Some("node1"));

        sel.select("node2");
        assert!(!sel.is_selected("node1"));
        assert!(sel.is_selected("node2"));
        assert_eq!(sel.count(), 1);
    }

    #[test]
    fn test_toggle() {
        let mut sel = SelectionManager::new();

        sel.toggle("node1");
        assert!(sel.is_selected("node1"));

        sel.toggle("node2");
        assert!(sel.is_selected("node1"));
        assert!(sel.is_selected("node2"));
        assert_eq!(sel.count(), 2);

        sel.toggle("node1");
        assert!(!sel.is_selected("node1"));
        assert!(sel.is_selected("node2"));
        assert_eq!(sel.count(), 1);
    }

    #[test]
    fn test_add_many() {
        let mut sel = SelectionManager::new();

        sel.add_many(["a", "b", "c"]);
        assert_eq!(sel.count(), 3);
        assert!(sel.is_selected("a"));
        assert!(sel.is_selected("b"));
        assert!(sel.is_selected("c"));
    }

    #[test]
    fn test_clear() {
        let mut sel = SelectionManager::new();
        sel.add_many(["a", "b"]);
        sel.clear();
        assert!(sel.is_empty());
        assert_eq!(sel.primary(), None);
    }

    #[test]
    fn test_selection_rect_bounds() {
        let rect = SelectionRect {
            start_x: 100.0,
            start_y: 100.0,
            end_x: 50.0,
            end_y: 150.0,
        };

        let (min_x, min_y, max_x, max_y) = rect.bounds();
        assert_eq!(min_x, 50.0);
        assert_eq!(min_y, 100.0);
        assert_eq!(max_x, 100.0);
        assert_eq!(max_y, 150.0);
    }

    #[test]
    fn test_selection_rect_intersects() {
        let rect = SelectionRect {
            start_x: 0.0,
            start_y: 0.0,
            end_x: 100.0,
            end_y: 100.0,
        };

        // Overlapping rectangle
        assert!(rect.intersects(50.0, 50.0, 100.0, 100.0));

        // Non-overlapping rectangle
        assert!(!rect.intersects(200.0, 200.0, 50.0, 50.0));

        // Touching edge
        assert!(!rect.intersects(100.0, 0.0, 50.0, 50.0));
    }

    #[test]
    fn test_selection_rect_contains_point() {
        let rect = SelectionRect {
            start_x: 0.0,
            start_y: 0.0,
            end_x: 100.0,
            end_y: 100.0,
        };

        assert!(rect.contains_point(50.0, 50.0));
        assert!(rect.contains_point(0.0, 0.0));
        assert!(rect.contains_point(100.0, 100.0));
        assert!(!rect.contains_point(150.0, 50.0));
    }
}
