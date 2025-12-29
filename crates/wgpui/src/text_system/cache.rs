//! Line layout caching for performance.
//!
//! This module provides caching infrastructure to avoid re-shaping
//! text that hasn't changed between frames.

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::{Arc, Mutex, RwLock};

use smallvec::SmallVec;

use super::{FontRun, LineLayout, WrappedLineLayout};

/// Cache for line layouts.
///
/// Uses a double-buffered approach where layouts from the previous
/// frame can be reused if the text and styling haven't changed.
pub struct LineLayoutCache {
    previous_frame: Mutex<FrameCache>,
    current_frame: RwLock<FrameCache>,
}

#[derive(Default)]
struct FrameCache {
    /// Cached line layouts keyed by content.
    lines: HashMap<Arc<CacheKey>, Arc<LineLayout>>,
    /// Cached wrapped line layouts.
    wrapped_lines: HashMap<Arc<CacheKey>, Arc<WrappedLineLayout>>,
    /// Keys used in the current frame (for tracking reuse).
    used_lines: Vec<Arc<CacheKey>>,
    /// Keys for wrapped lines used in the current frame.
    used_wrapped_lines: Vec<Arc<CacheKey>>,
}

/// Index into the layout cache for a frame.
#[derive(Clone, Default)]
pub struct LineLayoutIndex {
    lines_index: usize,
    wrapped_lines_index: usize,
}

impl LineLayoutCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            previous_frame: Mutex::new(FrameCache::default()),
            current_frame: RwLock::new(FrameCache::default()),
        }
    }

    /// Get the current layout index.
    ///
    /// This can be used to track which layouts were added during
    /// a particular operation.
    pub fn layout_index(&self) -> LineLayoutIndex {
        let frame = self.current_frame.read().unwrap();
        LineLayoutIndex {
            lines_index: frame.used_lines.len(),
            wrapped_lines_index: frame.used_wrapped_lines.len(),
        }
    }

    /// Reuse layouts from the previous frame within the given range.
    ///
    /// This is useful when re-rendering a portion of the UI that
    /// hasn't changed.
    pub fn reuse_layouts(&self, range: Range<LineLayoutIndex>) {
        let mut previous_frame = self.previous_frame.lock().unwrap();
        let mut current_frame = self.current_frame.write().unwrap();

        // Collect keys to reuse first (to avoid borrow issues)
        let line_keys: Vec<_> = previous_frame.used_lines
            [range.start.lines_index..range.end.lines_index]
            .to_vec();
        let wrapped_keys: Vec<_> = previous_frame.used_wrapped_lines
            [range.start.wrapped_lines_index..range.end.wrapped_lines_index]
            .to_vec();

        // Reuse line layouts
        for key in line_keys {
            if let Some((key, layout)) = previous_frame.lines.remove_entry(&key) {
                current_frame.lines.insert(key.clone(), layout);
                current_frame.used_lines.push(key);
            }
        }

        // Reuse wrapped line layouts
        for key in wrapped_keys {
            if let Some((key, layout)) = previous_frame.wrapped_lines.remove_entry(&key) {
                current_frame.wrapped_lines.insert(key.clone(), layout);
                current_frame.used_wrapped_lines.push(key);
            }
        }
    }

    /// Truncate layouts to the given index.
    ///
    /// Removes layouts added after the given index.
    pub fn truncate_layouts(&self, index: LineLayoutIndex) {
        let mut current_frame = self.current_frame.write().unwrap();
        current_frame.used_lines.truncate(index.lines_index);
        current_frame
            .used_wrapped_lines
            .truncate(index.wrapped_lines_index);
    }

    /// Finish the current frame.
    ///
    /// Swaps current and previous frame caches, clearing the
    /// current frame for the next render.
    pub fn finish_frame(&self) {
        let mut prev_frame = self.previous_frame.lock().unwrap();
        let mut curr_frame = self.current_frame.write().unwrap();

        // Swap frames
        std::mem::swap(&mut *prev_frame, &mut *curr_frame);

        // Clear new current frame
        curr_frame.lines.clear();
        curr_frame.wrapped_lines.clear();
        curr_frame.used_lines.clear();
        curr_frame.used_wrapped_lines.clear();
    }

    /// Get or compute a line layout.
    ///
    /// If a cached layout exists for the given parameters, it's returned.
    /// Otherwise, the `compute` function is called to create a new layout.
    pub fn layout_line<F>(
        &self,
        text: &str,
        font_size: f32,
        runs: &[FontRun],
        compute: F,
    ) -> Arc<LineLayout>
    where
        F: FnOnce(&str, f32, &[FontRun]) -> LineLayout,
    {
        let key_ref = CacheKeyRef {
            text,
            font_size,
            runs,
            wrap_width: None,
        };

        // Try current frame cache first
        {
            let current_frame = self.current_frame.read().unwrap();
            if let Some(layout) = current_frame.lines.get(&key_ref as &dyn AsCacheKeyRef) {
                return layout.clone();
            }
        }

        // Try previous frame cache
        let from_previous = self
            .previous_frame
            .lock()
            .unwrap()
            .lines
            .remove_entry(&key_ref as &dyn AsCacheKeyRef);

        if let Some((key, layout)) = from_previous {
            let mut current_frame = self.current_frame.write().unwrap();
            current_frame.lines.insert(key.clone(), layout.clone());
            current_frame.used_lines.push(key);
            return layout;
        }

        // Compute new layout
        let layout = Arc::new(compute(text, font_size, runs));
        let key = Arc::new(CacheKey {
            text: text.to_string(),
            font_size,
            runs: SmallVec::from(runs),
            wrap_width: None,
        });

        let mut current_frame = self.current_frame.write().unwrap();
        current_frame.lines.insert(key.clone(), layout.clone());
        current_frame.used_lines.push(key);

        layout
    }

    /// Get or compute a wrapped line layout.
    pub fn layout_wrapped_line<F>(
        &self,
        text: &str,
        font_size: f32,
        runs: &[FontRun],
        wrap_width: Option<f32>,
        compute: F,
    ) -> Arc<WrappedLineLayout>
    where
        F: FnOnce(&str, f32, &[FontRun], Option<f32>) -> WrappedLineLayout,
    {
        let key_ref = CacheKeyRef {
            text,
            font_size,
            runs,
            wrap_width,
        };

        // Try current frame cache first
        {
            let current_frame = self.current_frame.read().unwrap();
            if let Some(layout) = current_frame
                .wrapped_lines
                .get(&key_ref as &dyn AsCacheKeyRef)
            {
                return layout.clone();
            }
        }

        // Try previous frame cache
        let from_previous = self
            .previous_frame
            .lock()
            .unwrap()
            .wrapped_lines
            .remove_entry(&key_ref as &dyn AsCacheKeyRef);

        if let Some((key, layout)) = from_previous {
            let mut current_frame = self.current_frame.write().unwrap();
            current_frame
                .wrapped_lines
                .insert(key.clone(), layout.clone());
            current_frame.used_wrapped_lines.push(key);
            return layout;
        }

        // Compute new layout
        let layout = Arc::new(compute(text, font_size, runs, wrap_width));
        let key = Arc::new(CacheKey {
            text: text.to_string(),
            font_size,
            runs: SmallVec::from(runs),
            wrap_width,
        });

        let mut current_frame = self.current_frame.write().unwrap();
        current_frame
            .wrapped_lines
            .insert(key.clone(), layout.clone());
        current_frame.used_wrapped_lines.push(key);

        layout
    }

    /// Clear all cached layouts.
    pub fn clear(&self) {
        self.previous_frame.lock().unwrap().clear();
        self.current_frame.write().unwrap().clear();
    }

    /// Get statistics about the cache.
    pub fn stats(&self) -> CacheStats {
        let current = self.current_frame.read().unwrap();
        let previous = self.previous_frame.lock().unwrap();
        CacheStats {
            current_lines: current.lines.len(),
            current_wrapped_lines: current.wrapped_lines.len(),
            previous_lines: previous.lines.len(),
            previous_wrapped_lines: previous.wrapped_lines.len(),
        }
    }
}

impl Default for LineLayoutCache {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameCache {
    fn clear(&mut self) {
        self.lines.clear();
        self.wrapped_lines.clear();
        self.used_lines.clear();
        self.used_wrapped_lines.clear();
    }
}

/// Statistics about the layout cache.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of line layouts in the current frame.
    pub current_lines: usize,
    /// Number of wrapped line layouts in the current frame.
    pub current_wrapped_lines: usize,
    /// Number of line layouts from the previous frame.
    pub previous_lines: usize,
    /// Number of wrapped line layouts from the previous frame.
    pub previous_wrapped_lines: usize,
}

// Cache key implementation

#[derive(Clone, Debug)]
struct CacheKey {
    text: String,
    font_size: f32,
    runs: SmallVec<[FontRun; 1]>,
    wrap_width: Option<f32>,
}

impl Eq for CacheKey {}

// Use ordered f32 comparison via to_bits
impl PartialEq for CacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.font_size.to_bits() == other.font_size.to_bits()
            && self.runs == other.runs
            && self.wrap_width.map(|w| w.to_bits()) == other.wrap_width.map(|w| w.to_bits())
    }
}

impl Hash for CacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
        self.font_size.to_bits().hash(state);
        self.runs.hash(state);
        self.wrap_width.map(|w| w.to_bits()).hash(state);
    }
}

#[derive(Copy, Clone)]
struct CacheKeyRef<'a> {
    text: &'a str,
    font_size: f32,
    runs: &'a [FontRun],
    wrap_width: Option<f32>,
}

impl PartialEq for CacheKeyRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.font_size.to_bits() == other.font_size.to_bits()
            && self.runs == other.runs
            && self.wrap_width.map(|w| w.to_bits()) == other.wrap_width.map(|w| w.to_bits())
    }
}

impl Eq for CacheKeyRef<'_> {}

impl Hash for CacheKeyRef<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
        self.font_size.to_bits().hash(state);
        self.runs.hash(state);
        self.wrap_width.map(|w| w.to_bits()).hash(state);
    }
}

impl PartialEq<CacheKeyRef<'_>> for CacheKey {
    fn eq(&self, other: &CacheKeyRef<'_>) -> bool {
        self.text == other.text
            && self.font_size.to_bits() == other.font_size.to_bits()
            && self.runs.as_slice() == other.runs
            && self.wrap_width.map(|w| w.to_bits()) == other.wrap_width.map(|w| w.to_bits())
    }
}

trait AsCacheKeyRef {
    fn as_cache_key_ref(&self) -> CacheKeyRef<'_>;
}

impl AsCacheKeyRef for CacheKey {
    fn as_cache_key_ref(&self) -> CacheKeyRef<'_> {
        CacheKeyRef {
            text: &self.text,
            font_size: self.font_size,
            runs: &self.runs,
            wrap_width: self.wrap_width,
        }
    }
}

impl AsCacheKeyRef for CacheKeyRef<'_> {
    fn as_cache_key_ref(&self) -> CacheKeyRef<'_> {
        *self
    }
}

impl PartialEq for dyn AsCacheKeyRef + '_ {
    fn eq(&self, other: &dyn AsCacheKeyRef) -> bool {
        let a = self.as_cache_key_ref();
        let b = other.as_cache_key_ref();
        a.text == b.text
            && a.font_size.to_bits() == b.font_size.to_bits()
            && a.runs == b.runs
            && a.wrap_width.map(|w| w.to_bits()) == b.wrap_width.map(|w| w.to_bits())
    }
}

impl Eq for dyn AsCacheKeyRef + '_ {}

impl Hash for dyn AsCacheKeyRef + '_ {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let r = self.as_cache_key_ref();
        r.text.hash(state);
        r.font_size.to_bits().hash(state);
        r.runs.hash(state);
        r.wrap_width.map(|w| w.to_bits()).hash(state);
    }
}

impl<'a> Borrow<dyn AsCacheKeyRef + 'a> for Arc<CacheKey> {
    fn borrow(&self) -> &(dyn AsCacheKeyRef + 'a) {
        self.as_ref() as &dyn AsCacheKeyRef
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_layout(width: f32) -> LineLayout {
        LineLayout {
            font_size: 14.0,
            width,
            ascent: 12.0,
            descent: 4.0,
            runs: vec![],
            len: 5,
        }
    }

    #[test]
    fn test_cache_hit() {
        let cache = LineLayoutCache::new();

        // First call computes
        let layout1 = cache.layout_line("hello", 14.0, &[], |_, _, _| make_layout(50.0));
        assert_eq!(layout1.width, 50.0);

        // Second call should hit cache
        let layout2 = cache.layout_line("hello", 14.0, &[], |_, _, _| make_layout(999.0));
        assert_eq!(layout2.width, 50.0); // Same as first

        // Different text should miss cache
        let layout3 = cache.layout_line("world", 14.0, &[], |_, _, _| make_layout(60.0));
        assert_eq!(layout3.width, 60.0);
    }

    #[test]
    fn test_cache_frame_swap() {
        let cache = LineLayoutCache::new();

        // Add to current frame
        let layout1 = cache.layout_line("hello", 14.0, &[], |_, _, _| make_layout(50.0));
        assert_eq!(layout1.width, 50.0);

        // Finish frame - layout moves to previous
        cache.finish_frame();

        // Should still hit from previous frame (and move to current)
        let layout2 = cache.layout_line("hello", 14.0, &[], |_, _, _| make_layout(999.0));
        assert_eq!(layout2.width, 50.0);

        // Test eviction: finish two frames WITHOUT using the layout
        cache.finish_frame(); // layout in previous
        cache.finish_frame(); // previous dropped, layout gone

        // Now it should be gone (not used for two frames)
        let layout3 = cache.layout_line("hello", 14.0, &[], |_, _, _| make_layout(70.0));
        assert_eq!(layout3.width, 70.0);
    }

    #[test]
    fn test_wrapped_cache() {
        let cache = LineLayoutCache::new();

        let layout1 = cache.layout_wrapped_line("hello world", 14.0, &[], Some(50.0), |_, _, _, _| {
            WrappedLineLayout::default()
        });

        let layout2 = cache.layout_wrapped_line("hello world", 14.0, &[], Some(50.0), |_, _, _, _| {
            panic!("Should not be called - cache hit expected")
        });

        // Same Arc
        assert!(Arc::ptr_eq(&layout1, &layout2));
    }

    #[test]
    fn test_cache_stats() {
        let cache = LineLayoutCache::new();

        cache.layout_line("a", 14.0, &[], |_, _, _| make_layout(10.0));
        cache.layout_line("b", 14.0, &[], |_, _, _| make_layout(20.0));
        cache.layout_wrapped_line("c", 14.0, &[], None, |_, _, _, _| {
            WrappedLineLayout::default()
        });

        let stats = cache.stats();
        assert_eq!(stats.current_lines, 2);
        assert_eq!(stats.current_wrapped_lines, 1);
        assert_eq!(stats.previous_lines, 0);
    }

    #[test]
    fn test_cache_clear() {
        let cache = LineLayoutCache::new();

        cache.layout_line("hello", 14.0, &[], |_, _, _| make_layout(50.0));
        cache.clear();

        let stats = cache.stats();
        assert_eq!(stats.current_lines, 0);
        assert_eq!(stats.previous_lines, 0);
    }

    #[test]
    fn test_layout_index() {
        let cache = LineLayoutCache::new();

        let idx1 = cache.layout_index();
        assert_eq!(idx1.lines_index, 0);

        cache.layout_line("a", 14.0, &[], |_, _, _| make_layout(10.0));

        let idx2 = cache.layout_index();
        assert_eq!(idx2.lines_index, 1);
    }
}
