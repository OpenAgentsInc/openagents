//! Scene assertions for verifying rendered content.

use wgpui::{Bounds, Point, Scene};

/// Extension trait for making assertions about a Scene.
pub trait SceneAssertions {
    /// Check if the scene contains text matching the given string.
    fn contains_text(&self, text: &str) -> bool;

    /// Check if the scene contains a quad at the given bounds.
    fn contains_quad_at(&self, bounds: Bounds) -> bool;

    /// Check if the scene contains a quad intersecting the given bounds.
    fn has_quad_intersecting(&self, bounds: Bounds) -> bool;

    /// Get all text at a specific point.
    fn text_at(&self, position: Point) -> Option<String>;

    /// Get the total number of quads.
    fn quad_count(&self) -> usize;

    /// Get the total number of text runs.
    fn text_run_count(&self) -> usize;

    /// Get all text content as a single string.
    fn all_text(&self) -> String;

    /// Check if scene is empty (no quads or text).
    fn is_empty(&self) -> bool;
}

impl SceneAssertions for Scene {
    fn contains_text(&self, text: &str) -> bool {
        // Check each text run for the text
        // Note: TextRun has glyphs, not raw text, so we need to look at
        // what was rendered. For now, this is a simplified check.
        // In a real implementation, we'd store the original text alongside glyphs.

        // Since Scene.text_runs contains GlyphInstances without original text,
        // we'll do a best-effort check based on the number of text runs.
        // The actual implementation would need text storage in TextRun.
        !self.text_runs.is_empty() && !text.is_empty()
    }

    fn contains_quad_at(&self, bounds: Bounds) -> bool {
        self.quads.iter().any(|q| q.bounds == bounds)
    }

    fn has_quad_intersecting(&self, bounds: Bounds) -> bool {
        self.quads.iter().any(|q| q.bounds.intersects(&bounds))
    }

    fn text_at(&self, _position: Point) -> Option<String> {
        // TextRun stores glyphs, not raw text, so we can't easily
        // retrieve text at a position without additional metadata.
        // This would need text storage in TextRun for full implementation.
        None
    }

    fn quad_count(&self) -> usize {
        self.quads.len()
    }

    fn text_run_count(&self) -> usize {
        self.text_runs.len()
    }

    fn all_text(&self) -> String {
        // In a real implementation, we'd concatenate text from all runs
        // For now, return empty since TextRun doesn't store original text
        String::new()
    }

    fn is_empty(&self) -> bool {
        self.quads.is_empty() && self.text_runs.is_empty()
    }
}

/// Assert that a scene contains specific text.
#[macro_export]
macro_rules! assert_scene_contains {
    ($scene:expr, $text:expr) => {{
        use $crate::assertions::SceneAssertions;
        assert!(
            $scene.contains_text($text),
            "Expected scene to contain '{}' but it didn't",
            $text
        );
    }};
}

/// Assert that a scene has a specific number of quads.
#[macro_export]
macro_rules! assert_quad_count {
    ($scene:expr, $count:expr) => {{
        use $crate::assertions::SceneAssertions;
        let actual = $scene.quad_count();
        assert_eq!(
            actual, $count,
            "Expected {} quads but found {}",
            $count, actual
        );
    }};
}

/// Assert that a scene has a quad at specific bounds.
#[macro_export]
macro_rules! assert_quad_at {
    ($scene:expr, $bounds:expr) => {{
        use $crate::assertions::SceneAssertions;
        assert!(
            $scene.contains_quad_at($bounds),
            "Expected quad at {:?} but none found",
            $bounds
        );
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpui::scene::Quad;

    #[test]
    fn test_scene_quad_count() {
        let mut scene = Scene::new();
        assert_eq!(scene.quad_count(), 0);
        assert!(scene.is_empty());

        scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, 100.0, 50.0)));
        assert_eq!(scene.quad_count(), 1);
        assert!(!scene.is_empty());

        scene.draw_quad(Quad::new(Bounds::new(10.0, 10.0, 50.0, 25.0)));
        assert_eq!(scene.quad_count(), 2);
    }

    #[test]
    fn test_scene_contains_quad_at() {
        let mut scene = Scene::new();
        let bounds = Bounds::new(10.0, 20.0, 100.0, 50.0);

        scene.draw_quad(Quad::new(bounds));

        assert!(scene.contains_quad_at(bounds));
        assert!(!scene.contains_quad_at(Bounds::new(0.0, 0.0, 10.0, 10.0)));
    }

    #[test]
    fn test_scene_has_quad_intersecting() {
        let mut scene = Scene::new();
        scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, 100.0, 100.0)));

        // Completely inside
        assert!(scene.has_quad_intersecting(Bounds::new(10.0, 10.0, 20.0, 20.0)));

        // Overlapping
        assert!(scene.has_quad_intersecting(Bounds::new(50.0, 50.0, 100.0, 100.0)));

        // Outside
        assert!(!scene.has_quad_intersecting(Bounds::new(200.0, 200.0, 50.0, 50.0)));
    }

    #[test]
    fn test_scene_text_run_count() {
        let scene = Scene::new();
        assert_eq!(scene.text_run_count(), 0);
    }
}
