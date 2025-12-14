//! Scheduler - frame-based update scheduling.
//!
//! The scheduler orchestrates the update cycle through distinct phases:
//! Update → Build → Layout → Paint → Render

use crate::effect::flush_effects;
use std::time::{Duration, Instant};

/// The current phase of the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePhase {
    /// Idle - waiting for next frame.
    Idle,
    /// Update - process signals, run effects.
    Update,
    /// Build - update widget tree.
    Build,
    /// Layout - compute positions and sizes.
    Layout,
    /// Paint - generate display list.
    Paint,
    /// Render - submit to GPU.
    Render,
}

/// Frame statistics for performance monitoring.
#[derive(Debug, Clone, Default)]
pub struct FrameStats {
    /// Total frame time.
    pub total_ms: f64,
    /// Update phase time.
    pub update_ms: f64,
    /// Build phase time.
    pub build_ms: f64,
    /// Layout phase time.
    pub layout_ms: f64,
    /// Paint phase time.
    pub paint_ms: f64,
    /// Render phase time.
    pub render_ms: f64,
    /// Number of effects run.
    pub effects_run: usize,
    /// Number of layouts computed.
    pub layouts_computed: usize,
}

/// Callback types for each phase.
pub struct PhaseCallbacks {
    /// Called during build phase.
    pub on_build: Option<Box<dyn FnMut()>>,
    /// Called during layout phase.
    pub on_layout: Option<Box<dyn FnMut()>>,
    /// Called during paint phase.
    pub on_paint: Option<Box<dyn FnMut()>>,
    /// Called during render phase.
    pub on_render: Option<Box<dyn FnMut()>>,
}

impl Default for PhaseCallbacks {
    fn default() -> Self {
        Self {
            on_build: None,
            on_layout: None,
            on_paint: None,
            on_render: None,
        }
    }
}

/// The frame scheduler.
pub struct Scheduler {
    /// Current phase.
    phase: FramePhase,
    /// Frame count.
    frame_count: u64,
    /// Target frame duration (for frame pacing).
    target_frame_time: Duration,
    /// Last frame's stats.
    last_stats: FrameStats,
    /// Phase callbacks.
    callbacks: PhaseCallbacks,
    /// Whether a frame is requested.
    frame_requested: bool,
}

impl Scheduler {
    /// Create a new scheduler.
    pub fn new() -> Self {
        Self {
            phase: FramePhase::Idle,
            frame_count: 0,
            target_frame_time: Duration::from_secs_f64(1.0 / 60.0), // 60 FPS
            last_stats: FrameStats::default(),
            callbacks: PhaseCallbacks::default(),
            frame_requested: false,
        }
    }

    /// Create a scheduler with a target FPS.
    pub fn with_target_fps(fps: u32) -> Self {
        Self {
            target_frame_time: Duration::from_secs_f64(1.0 / fps as f64),
            ..Self::new()
        }
    }

    /// Get the current phase.
    pub fn phase(&self) -> FramePhase {
        self.phase
    }

    /// Get the frame count.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get the last frame's stats.
    pub fn last_stats(&self) -> &FrameStats {
        &self.last_stats
    }

    /// Request a frame.
    pub fn request_frame(&mut self) {
        self.frame_requested = true;
    }

    /// Check if a frame is requested.
    pub fn is_frame_requested(&self) -> bool {
        self.frame_requested
    }

    /// Set the build callback.
    pub fn set_build_callback<F: FnMut() + 'static>(&mut self, callback: F) {
        self.callbacks.on_build = Some(Box::new(callback));
    }

    /// Set the layout callback.
    pub fn set_layout_callback<F: FnMut() + 'static>(&mut self, callback: F) {
        self.callbacks.on_layout = Some(Box::new(callback));
    }

    /// Set the paint callback.
    pub fn set_paint_callback<F: FnMut() + 'static>(&mut self, callback: F) {
        self.callbacks.on_paint = Some(Box::new(callback));
    }

    /// Set the render callback.
    pub fn set_render_callback<F: FnMut() + 'static>(&mut self, callback: F) {
        self.callbacks.on_render = Some(Box::new(callback));
    }

    /// Run a single frame.
    pub fn run_frame(&mut self) -> FrameStats {
        let frame_start = Instant::now();
        let mut stats = FrameStats::default();

        // Update phase - run effects
        self.phase = FramePhase::Update;
        let update_start = Instant::now();
        flush_effects();
        stats.update_ms = update_start.elapsed().as_secs_f64() * 1000.0;

        // Build phase
        self.phase = FramePhase::Build;
        let build_start = Instant::now();
        if let Some(ref mut callback) = self.callbacks.on_build {
            callback();
        }
        stats.build_ms = build_start.elapsed().as_secs_f64() * 1000.0;

        // Layout phase
        self.phase = FramePhase::Layout;
        let layout_start = Instant::now();
        if let Some(ref mut callback) = self.callbacks.on_layout {
            callback();
        }
        stats.layout_ms = layout_start.elapsed().as_secs_f64() * 1000.0;

        // Paint phase
        self.phase = FramePhase::Paint;
        let paint_start = Instant::now();
        if let Some(ref mut callback) = self.callbacks.on_paint {
            callback();
        }
        stats.paint_ms = paint_start.elapsed().as_secs_f64() * 1000.0;

        // Render phase
        self.phase = FramePhase::Render;
        let render_start = Instant::now();
        if let Some(ref mut callback) = self.callbacks.on_render {
            callback();
        }
        stats.render_ms = render_start.elapsed().as_secs_f64() * 1000.0;

        // Done
        self.phase = FramePhase::Idle;
        stats.total_ms = frame_start.elapsed().as_secs_f64() * 1000.0;

        self.frame_count += 1;
        self.frame_requested = false;
        self.last_stats = stats.clone();

        stats
    }

    /// Get the target frame time.
    pub fn target_frame_time(&self) -> Duration {
        self.target_frame_time
    }

    /// Set the target FPS.
    pub fn set_target_fps(&mut self, fps: u32) {
        self.target_frame_time = Duration::from_secs_f64(1.0 / fps as f64);
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_scheduler_phases() {
        let mut scheduler = Scheduler::new();

        assert_eq!(scheduler.phase(), FramePhase::Idle);

        scheduler.run_frame();

        assert_eq!(scheduler.phase(), FramePhase::Idle);
        assert_eq!(scheduler.frame_count(), 1);
    }

    #[test]
    fn test_scheduler_callbacks() {
        let build_called = Arc::new(AtomicUsize::new(0));
        let layout_called = Arc::new(AtomicUsize::new(0));

        let mut scheduler = Scheduler::new();

        let build_clone = build_called.clone();
        scheduler.set_build_callback(move || {
            build_clone.fetch_add(1, Ordering::SeqCst);
        });

        let layout_clone = layout_called.clone();
        scheduler.set_layout_callback(move || {
            layout_clone.fetch_add(1, Ordering::SeqCst);
        });

        scheduler.run_frame();

        assert_eq!(build_called.load(Ordering::SeqCst), 1);
        assert_eq!(layout_called.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_frame_stats() {
        let mut scheduler = Scheduler::new();
        let stats = scheduler.run_frame();

        assert!(stats.total_ms >= 0.0);
        assert!(stats.update_ms >= 0.0);
    }
}
