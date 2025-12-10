//! Event Scheduler for the Unit runtime
//!
//! The scheduler controls event processing based on graph lifecycle.
//! When paused, events are buffered. When playing, events are processed
//! immediately or on the next tick.

use crate::event::{EventBus, RuntimeEvent};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Scheduler mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerMode {
    /// Events are buffered, not processed
    Paused,
    /// Events are processed immediately
    Playing,
    /// Single-step mode: process one event then pause
    Stepping,
}

/// Event scheduler with buffering support
///
/// The scheduler wraps an EventBus and adds lifecycle-aware
/// event processing. When paused, events are buffered and
/// released when playback resumes.
pub struct EventScheduler {
    /// The underlying event bus
    bus: Arc<EventBus>,
    /// Buffer for events received while paused
    buffer: Mutex<VecDeque<RuntimeEvent>>,
    /// Current scheduler mode
    mode: Mutex<SchedulerMode>,
    /// Statistics
    stats: Mutex<SchedulerStats>,
}

/// Scheduler statistics
#[derive(Debug, Clone, Default)]
pub struct SchedulerStats {
    /// Total events processed
    pub total_processed: usize,
    /// Total events buffered (then released)
    pub total_buffered: usize,
    /// Current buffer size
    pub buffer_size: usize,
    /// Events dropped (buffer overflow)
    pub dropped: usize,
}

impl EventScheduler {
    /// Maximum buffer size before dropping events
    pub const MAX_BUFFER_SIZE: usize = 10000;

    /// Create a new scheduler with the given event bus
    pub fn new(bus: Arc<EventBus>) -> Self {
        Self {
            bus,
            buffer: Mutex::new(VecDeque::new()),
            mode: Mutex::new(SchedulerMode::Paused),
            stats: Mutex::new(SchedulerStats::default()),
        }
    }

    /// Create a new scheduler with a fresh event bus
    pub fn with_new_bus() -> Self {
        Self::new(Arc::new(EventBus::new()))
    }

    /// Get a reference to the event bus
    pub fn bus(&self) -> &Arc<EventBus> {
        &self.bus
    }

    /// Get current mode
    pub fn mode(&self) -> SchedulerMode {
        *self.mode.lock().unwrap()
    }

    /// Check if playing
    pub fn is_playing(&self) -> bool {
        self.mode() == SchedulerMode::Playing
    }

    /// Check if paused
    pub fn is_paused(&self) -> bool {
        self.mode() == SchedulerMode::Paused
    }

    /// Start playback
    ///
    /// Flushes buffered events to the bus and begins processing.
    pub fn play(&self) {
        // Flush buffer to bus
        self.flush_buffer();

        // Set mode to playing
        *self.mode.lock().unwrap() = SchedulerMode::Playing;
    }

    /// Pause playback
    ///
    /// Events will be buffered until play() is called.
    pub fn pause(&self) {
        *self.mode.lock().unwrap() = SchedulerMode::Paused;
    }

    /// Enter stepping mode
    pub fn step_mode(&self) {
        *self.mode.lock().unwrap() = SchedulerMode::Stepping;
    }

    /// Emit an event
    ///
    /// If playing, the event goes to the bus.
    /// If paused, the event is buffered.
    pub fn emit(&self, event: RuntimeEvent) {
        let mode = self.mode();

        match mode {
            SchedulerMode::Playing | SchedulerMode::Stepping => {
                self.bus.emit(event);
            }
            SchedulerMode::Paused => {
                self.buffer_event(event);
            }
        }
    }

    /// Process events
    ///
    /// Returns the number of events processed.
    /// In stepping mode, processes one event then pauses.
    pub fn tick(&self) -> usize {
        let mode = self.mode();

        match mode {
            SchedulerMode::Playing => self.bus.process(),
            SchedulerMode::Stepping => {
                let count = self.bus.process();
                if count > 0 {
                    *self.mode.lock().unwrap() = SchedulerMode::Paused;
                }
                count
            }
            SchedulerMode::Paused => 0,
        }
    }

    /// Process all pending events (regardless of mode)
    ///
    /// Use with caution - bypasses pause state.
    pub fn flush(&self) -> usize {
        self.flush_buffer();
        self.bus.process()
    }

    /// Get current statistics
    pub fn stats(&self) -> SchedulerStats {
        let mut stats = self.stats.lock().unwrap().clone();
        stats.buffer_size = self.buffer.lock().unwrap().len();
        stats
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        *self.stats.lock().unwrap() = SchedulerStats::default();
    }

    /// Buffer an event
    fn buffer_event(&self, event: RuntimeEvent) {
        let mut buffer = self.buffer.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();

        if buffer.len() >= Self::MAX_BUFFER_SIZE {
            // Drop oldest event to make room
            buffer.pop_front();
            stats.dropped += 1;
        }

        buffer.push_back(event);
        stats.total_buffered += 1;
    }

    /// Flush buffer to bus
    fn flush_buffer(&self) {
        let events: Vec<RuntimeEvent> = {
            let mut buffer = self.buffer.lock().unwrap();
            buffer.drain(..).collect()
        };

        for event in events {
            self.bus.emit(event);
        }
    }
}

impl Default for EventScheduler {
    fn default() -> Self {
        Self::with_new_bus()
    }
}

impl std::fmt::Debug for EventScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventScheduler")
            .field("mode", &self.mode())
            .field("stats", &self.stats())
            .finish()
    }
}

/// Builder for EventScheduler
pub struct SchedulerBuilder {
    bus: Option<Arc<EventBus>>,
    initial_mode: SchedulerMode,
}

impl SchedulerBuilder {
    pub fn new() -> Self {
        Self {
            bus: None,
            initial_mode: SchedulerMode::Paused,
        }
    }

    /// Use an existing event bus
    pub fn with_bus(mut self, bus: Arc<EventBus>) -> Self {
        self.bus = Some(bus);
        self
    }

    /// Start in playing mode
    pub fn playing(mut self) -> Self {
        self.initial_mode = SchedulerMode::Playing;
        self
    }

    /// Start in paused mode (default)
    pub fn paused(mut self) -> Self {
        self.initial_mode = SchedulerMode::Paused;
        self
    }

    /// Build the scheduler
    pub fn build(self) -> EventScheduler {
        let bus = self.bus.unwrap_or_else(|| Arc::new(EventBus::new()));
        let scheduler = EventScheduler::new(bus);
        *scheduler.mode.lock().unwrap() = self.initial_mode;
        scheduler
    }
}

impl Default for SchedulerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_scheduler_buffer_when_paused() {
        let scheduler = EventScheduler::default();

        // Emit while paused
        scheduler.emit(RuntimeEvent::pin_drop("a", "x"));
        scheduler.emit(RuntimeEvent::pin_drop("b", "y"));

        // Should be buffered, not in bus
        assert_eq!(scheduler.bus.pending_count(), 0);
        assert_eq!(scheduler.stats().buffer_size, 2);
    }

    #[test]
    fn test_scheduler_play_flushes_buffer() {
        let scheduler = EventScheduler::default();

        // Buffer some events
        scheduler.emit(RuntimeEvent::pin_drop("a", "x"));
        scheduler.emit(RuntimeEvent::pin_drop("b", "y"));

        // Play should flush buffer to bus
        scheduler.play();

        assert_eq!(scheduler.bus.pending_count(), 2);
        assert_eq!(scheduler.stats().buffer_size, 0);
    }

    #[test]
    fn test_scheduler_processes_when_playing() {
        let scheduler = EventScheduler::default();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        scheduler.bus.subscribe(Box::new(move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }));

        scheduler.play();
        scheduler.emit(RuntimeEvent::pin_drop("a", "x"));
        scheduler.tick();

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_scheduler_stepping_mode() {
        let scheduler = EventScheduler::default();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        scheduler.bus.subscribe(Box::new(move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }));

        // Start stepping
        scheduler.step_mode();
        scheduler.emit(RuntimeEvent::pin_drop("a", "x"));
        scheduler.emit(RuntimeEvent::pin_drop("b", "y"));

        // First tick processes and returns to paused
        scheduler.tick();
        assert_eq!(counter.load(Ordering::SeqCst), 2);
        assert!(scheduler.is_paused());
    }

    #[test]
    fn test_scheduler_buffer_overflow() {
        let scheduler = EventScheduler::default();

        // Fill buffer past max
        for i in 0..EventScheduler::MAX_BUFFER_SIZE + 100 {
            scheduler.emit(RuntimeEvent::pin_drop("unit", &format!("pin{}", i)));
        }

        let stats = scheduler.stats();
        assert_eq!(stats.buffer_size, EventScheduler::MAX_BUFFER_SIZE);
        assert_eq!(stats.dropped, 100);
    }

    #[test]
    fn test_scheduler_builder() {
        let scheduler = SchedulerBuilder::new().playing().build();
        assert!(scheduler.is_playing());

        let scheduler = SchedulerBuilder::new().paused().build();
        assert!(scheduler.is_paused());
    }
}
