//! Event injection for test execution.
//!
//! Generates synthetic InputEvents for test steps.

use crate::testing::context::ComponentRegistry;
use crate::testing::step::{ClickTarget, ElementSelector, TestStep};
use crate::{InputEvent, Key, Modifiers, MouseButton, Point};
use std::time::{Duration, Instant};

/// A timed event to be injected.
#[derive(Clone, Debug)]
pub struct TimedEvent {
    /// The event to inject.
    pub event: InputEvent,
    /// Delay before this event.
    pub delay: Duration,
}

impl TimedEvent {
    /// Create a new timed event.
    pub fn new(event: InputEvent, delay: Duration) -> Self {
        Self { event, delay }
    }

    /// Create an immediate event (no delay).
    pub fn immediate(event: InputEvent) -> Self {
        Self::new(event, Duration::ZERO)
    }
}

/// A sequence of timed events.
#[derive(Clone, Debug, Default)]
pub struct EventSequence {
    events: Vec<TimedEvent>,
}

impl EventSequence {
    /// Create a new empty sequence.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an event with delay.
    pub fn push(&mut self, event: InputEvent, delay: Duration) {
        self.events.push(TimedEvent::new(event, delay));
    }

    /// Add an immediate event.
    pub fn push_immediate(&mut self, event: InputEvent) {
        self.events.push(TimedEvent::immediate(event));
    }

    /// Get all events.
    pub fn events(&self) -> &[TimedEvent] {
        &self.events
    }

    /// Get the total duration of this sequence.
    pub fn total_duration(&self) -> Duration {
        self.events.iter().map(|e| e.delay).sum()
    }

    /// Check if the sequence is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Get the number of events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Create events for a mouse move.
    pub fn mouse_move(x: f32, y: f32) -> EventSequence {
        let mut seq = Self::new();
        seq.push_immediate(InputEvent::MouseMove { x, y });
        seq
    }

    /// Create events for a click.
    pub fn click(x: f32, y: f32, button: MouseButton) -> EventSequence {
        let mut seq = Self::new();
        seq.push_immediate(InputEvent::MouseMove { x, y });
        seq.push(
            InputEvent::MouseDown { button, x, y },
            Duration::from_millis(10),
        );
        seq.push(
            InputEvent::MouseUp { button, x, y },
            Duration::from_millis(50),
        );
        seq
    }

    /// Create events for a double-click.
    pub fn double_click(x: f32, y: f32, button: MouseButton) -> EventSequence {
        let mut seq = Self::new();
        seq.push_immediate(InputEvent::MouseMove { x, y });
        // First click
        seq.push(
            InputEvent::MouseDown { button, x, y },
            Duration::from_millis(10),
        );
        seq.push(
            InputEvent::MouseUp { button, x, y },
            Duration::from_millis(50),
        );
        // Second click
        seq.push(
            InputEvent::MouseDown { button, x, y },
            Duration::from_millis(100),
        );
        seq.push(
            InputEvent::MouseUp { button, x, y },
            Duration::from_millis(50),
        );
        seq
    }

    /// Create events for typing a character.
    pub fn type_char(c: char, delay: Duration) -> EventSequence {
        let mut seq = Self::new();
        let key = Key::Character(c.to_string());
        let modifiers = Modifiers::default();
        seq.push(
            InputEvent::KeyDown {
                key: key.clone(),
                modifiers,
            },
            delay,
        );
        seq.push(
            InputEvent::KeyUp { key, modifiers },
            Duration::from_millis(20),
        );
        seq
    }

    /// Create events for typing text.
    pub fn type_text(text: &str, delay_per_char: Option<Duration>) -> EventSequence {
        let mut seq = Self::new();
        let delay = delay_per_char.unwrap_or(Duration::from_millis(50));
        for c in text.chars() {
            let key = Key::Character(c.to_string());
            let modifiers = Modifiers::default();
            seq.push(
                InputEvent::KeyDown {
                    key: key.clone(),
                    modifiers,
                },
                delay,
            );
            seq.push(
                InputEvent::KeyUp { key, modifiers },
                Duration::from_millis(20),
            );
        }
        seq
    }

    /// Create events for a key press.
    pub fn key_press(key: Key, modifiers: Modifiers) -> EventSequence {
        let mut seq = Self::new();
        seq.push(
            InputEvent::KeyDown {
                key: key.clone(),
                modifiers,
            },
            Duration::from_millis(10),
        );
        seq.push(
            InputEvent::KeyUp { key, modifiers },
            Duration::from_millis(50),
        );
        seq
    }

    /// Create events for scrolling.
    pub fn scroll(x: f32, y: f32, dx: f32, dy: f32) -> EventSequence {
        let mut seq = Self::new();
        seq.push_immediate(InputEvent::MouseMove { x, y });
        seq.push(InputEvent::Scroll { dx, dy }, Duration::from_millis(10));
        seq
    }
}

/// Player that iterates through a sequence of timed events.
pub struct EventPlayer {
    sequence: EventSequence,
    current_index: usize,
    started_at: Option<Instant>,
    next_event_time: Duration,
}

impl EventPlayer {
    /// Create a new player for a sequence.
    pub fn new(sequence: EventSequence) -> Self {
        let next_event_time = sequence.events.first().map(|e| e.delay).unwrap_or_default();
        Self {
            sequence,
            current_index: 0,
            started_at: None,
            next_event_time,
        }
    }

    /// Start playback.
    pub fn start(&mut self) {
        self.started_at = Some(Instant::now());
    }

    /// Reset playback to the beginning.
    pub fn reset(&mut self) {
        self.current_index = 0;
        self.started_at = None;
        self.next_event_time = self
            .sequence
            .events
            .first()
            .map(|e| e.delay)
            .unwrap_or_default();
    }

    /// Check if playback is complete.
    pub fn is_complete(&self) -> bool {
        self.current_index >= self.sequence.events.len()
    }

    /// Get the next event if its time has come.
    pub fn poll(&mut self) -> Option<InputEvent> {
        let started_at = self.started_at?;
        let elapsed = started_at.elapsed();

        if self.current_index >= self.sequence.events.len() {
            return None;
        }

        if elapsed >= self.next_event_time {
            let event = self.sequence.events[self.current_index].event.clone();
            self.current_index += 1;

            // Calculate time for next event
            if let Some(next) = self.sequence.events.get(self.current_index) {
                self.next_event_time += next.delay;
            }

            return Some(event);
        }

        None
    }

    /// Get all remaining events immediately.
    pub fn drain(&mut self) -> Vec<InputEvent> {
        let events: Vec<_> = self.sequence.events[self.current_index..]
            .iter()
            .map(|e| e.event.clone())
            .collect();
        self.current_index = self.sequence.events.len();
        events
    }
}

/// Resolve a click target to screen coordinates.
pub fn resolve_click_target(target: &ClickTarget, registry: &ComponentRegistry) -> Option<Point> {
    match target {
        ClickTarget::Position(p) => Some(*p),
        ClickTarget::Element(selector) => resolve_selector_center(selector, registry),
        ClickTarget::ElementOffset { selector, offset } => {
            let bounds = resolve_selector_bounds(selector, registry)?;
            Some(Point::new(
                bounds.origin.x + offset.x,
                bounds.origin.y + offset.y,
            ))
        }
    }
}

/// Resolve a selector to the center of the element.
fn resolve_selector_center(
    selector: &ElementSelector,
    registry: &ComponentRegistry,
) -> Option<Point> {
    let bounds = resolve_selector_bounds(selector, registry)?;
    Some(bounds.center())
}

/// Resolve a selector to element bounds.
fn resolve_selector_bounds(
    selector: &ElementSelector,
    registry: &ComponentRegistry,
) -> Option<crate::Bounds> {
    match selector {
        ElementSelector::Id(id) => registry.find_by_id(*id),
        ElementSelector::Text(text) => registry.find_by_text(text),
        ElementSelector::Bounds(b) => Some(*b),
        ElementSelector::Query(query) => {
            // Parse query string
            let parsed = ElementSelector::parse(query);
            if let ElementSelector::Query(_) = &parsed {
                // Couldn't parse, try as text
                registry.find_by_text(query)
            } else {
                resolve_selector_bounds(&parsed, registry)
            }
        }
    }
}

/// Generate events for a test step.
pub fn generate_step_events(
    step: &TestStep,
    registry: &ComponentRegistry,
) -> Result<EventSequence, String> {
    match step {
        TestStep::Click { target, button } => {
            let point = resolve_click_target(target, registry)
                .ok_or_else(|| "Could not resolve click target".to_string())?;
            Ok(EventSequence::click(point.x, point.y, *button))
        }
        TestStep::DoubleClick { target, button } => {
            let point = resolve_click_target(target, registry)
                .ok_or_else(|| "Could not resolve double-click target".to_string())?;
            Ok(EventSequence::double_click(point.x, point.y, *button))
        }
        TestStep::Type {
            text,
            delay_per_char,
        } => Ok(EventSequence::type_text(text, *delay_per_char)),
        TestStep::KeyPress { key, modifiers } => {
            Ok(EventSequence::key_press(key.clone(), *modifiers))
        }
        TestStep::Scroll { target, dx, dy } => {
            let point = resolve_click_target(target, registry)
                .ok_or_else(|| "Could not resolve scroll target".to_string())?;
            Ok(EventSequence::scroll(point.x, point.y, *dx, *dy))
        }
        TestStep::MoveTo { target } => {
            let point = resolve_click_target(target, registry)
                .ok_or_else(|| "Could not resolve move target".to_string())?;
            Ok(EventSequence::mouse_move(point.x, point.y))
        }
        // Wait and assertion steps don't generate events
        TestStep::Wait { .. }
        | TestStep::WaitFor { .. }
        | TestStep::Expect { .. }
        | TestStep::ExpectText { .. }
        | TestStep::ExpectVisible { .. } => Ok(EventSequence::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NamedKey;

    #[test]
    fn test_event_sequence_click() {
        let seq = EventSequence::click(100.0, 200.0, MouseButton::Left);
        assert_eq!(seq.len(), 3); // move, down, up
        assert!(!seq.is_empty());
    }

    #[test]
    fn test_event_sequence_double_click() {
        let seq = EventSequence::double_click(100.0, 200.0, MouseButton::Left);
        assert_eq!(seq.len(), 5); // move, down, up, down, up
    }

    #[test]
    fn test_event_sequence_type_text() {
        let seq = EventSequence::type_text("hi", Some(Duration::from_millis(50)));
        assert_eq!(seq.len(), 4); // 2 chars * (down + up)
    }

    #[test]
    fn test_type_text_default_delay_and_events() {
        let seq = EventSequence::type_text("ab", None);
        assert_eq!(seq.len(), 4);

        let events = seq.events();
        assert_eq!(events[0].delay, Duration::from_millis(50));
        assert_eq!(events[1].delay, Duration::from_millis(20));
        assert_eq!(events[2].delay, Duration::from_millis(50));
        assert_eq!(events[3].delay, Duration::from_millis(20));

        match &events[0].event {
            InputEvent::KeyDown { key, modifiers } => {
                if let Key::Character(c) = key {
                    assert_eq!(c, "a");
                } else {
                    panic!("Expected KeyDown with Character");
                }
                assert!(!modifiers.shift && !modifiers.ctrl && !modifiers.alt && !modifiers.meta);
            }
            _ => panic!("Expected KeyDown event"),
        }

        match &events[1].event {
            InputEvent::KeyUp { key, .. } => {
                if let Key::Character(c) = key {
                    assert_eq!(c, "a");
                } else {
                    panic!("Expected KeyUp with Character");
                }
            }
            _ => panic!("Expected KeyUp event"),
        }
    }

    #[test]
    fn test_event_sequence_key_press() {
        let seq = EventSequence::key_press(Key::Named(NamedKey::Enter), Modifiers::default());
        assert_eq!(seq.len(), 2); // down, up
    }

    #[test]
    fn test_event_player_poll() {
        let seq = EventSequence::click(100.0, 100.0, MouseButton::Left);
        let mut player = EventPlayer::new(seq);

        assert!(!player.is_complete());
        player.start();

        // First event should be available immediately
        std::thread::sleep(Duration::from_millis(1));
        assert!(player.poll().is_some());
    }

    #[test]
    fn test_event_player_drain() {
        let seq = EventSequence::click(100.0, 100.0, MouseButton::Left);
        let mut player = EventPlayer::new(seq);

        let events = player.drain();
        assert_eq!(events.len(), 3);
        assert!(player.is_complete());
    }

    #[test]
    fn test_generate_step_events_click() {
        use crate::Bounds;
        use crate::testing::step::ClickTarget;

        let mut registry = ComponentRegistry::new();
        registry.register_id(42, Bounds::new(100.0, 100.0, 50.0, 30.0));

        let step = TestStep::Click {
            target: ClickTarget::Element(ElementSelector::Id(42)),
            button: MouseButton::Left,
        };

        let result = generate_step_events(&step, &registry);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 3);
    }

    #[test]
    fn test_generate_step_events_wait() {
        let registry = ComponentRegistry::new();
        let step = TestStep::Wait {
            duration: Duration::from_millis(100),
        };

        let result = generate_step_events(&step, &registry);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty()); // Wait doesn't generate events
    }
}
