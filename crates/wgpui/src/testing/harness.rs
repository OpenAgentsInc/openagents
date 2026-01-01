//! Test harness that wraps a component for testing.
//!
//! Integrates the test runner, event injection, and input overlay into a
//! cohesive testing experience.

use crate::components::{AnyComponent, Component, ComponentId, EventResult};
use crate::components::{EventContext, PaintContext};
use crate::testing::assertion::AssertionResult;
use crate::testing::context::ComponentRegistry;
use crate::testing::injection::{EventPlayer, generate_step_events};
use crate::testing::overlay::InputOverlay;
use crate::testing::runner::{PlaybackSpeed, RunnerState, StepResult, TestRunner};
use crate::testing::step::{ElementSelector, TestStep};
use crate::{Bounds, InputEvent, Key, MouseButton, NamedKey, Point, Quad, theme};
use std::time::{Duration, Instant};

/// Height of the control bar.
const CONTROL_BAR_HEIGHT: f32 = 32.0;

/// Test harness that wraps a component for testing.
pub struct TestHarness {
    /// The component being tested.
    component: AnyComponent,
    /// Test runner.
    runner: Option<TestRunner>,
    /// Input overlay.
    overlay: InputOverlay,
    /// Whether to show the overlay.
    show_overlay: bool,
    /// Whether to show the control bar.
    show_controls: bool,
    /// Component registry for element lookup.
    registry: ComponentRegistry,
    /// Current event player for step execution.
    event_player: Option<EventPlayer>,
    /// Queued events from the player.
    event_queue: Vec<InputEvent>,
    /// When the current step started.
    step_started: Option<Instant>,
    /// Control bar hover states.
    play_hovered: bool,
    step_hovered: bool,
    speed_hovered: bool,
}

impl TestHarness {
    /// Create a new test harness wrapping a component.
    pub fn new<C: Component + 'static>(component: C) -> Self {
        Self {
            component: AnyComponent::new(component),
            runner: None,
            overlay: InputOverlay::new(),
            show_overlay: true,
            show_controls: true,
            registry: ComponentRegistry::new(),
            event_player: None,
            event_queue: Vec::new(),
            step_started: None,
            play_hovered: false,
            step_hovered: false,
            speed_hovered: false,
        }
    }

    /// Set the test runner.
    pub fn with_runner(mut self, runner: TestRunner) -> Self {
        self.runner = Some(runner);
        self
    }

    /// Set whether to show the overlay.
    pub fn show_overlay(mut self, show: bool) -> Self {
        self.show_overlay = show;
        self.overlay.set_visible(show);
        self
    }

    /// Set whether to show the control bar.
    pub fn show_controls(mut self, show: bool) -> Self {
        self.show_controls = show;
        self
    }

    /// Get a reference to the runner.
    pub fn runner(&self) -> Option<&TestRunner> {
        self.runner.as_ref()
    }

    /// Get a mutable reference to the runner.
    pub fn runner_mut(&mut self) -> Option<&mut TestRunner> {
        self.runner.as_mut()
    }

    /// Get the component registry.
    pub fn registry(&self) -> &ComponentRegistry {
        &self.registry
    }

    /// Get the component bounds (excluding control bar).
    fn component_bounds(&self, bounds: Bounds) -> Bounds {
        if self.show_controls {
            Bounds::new(
                bounds.origin.x,
                bounds.origin.y + CONTROL_BAR_HEIGHT,
                bounds.size.width,
                bounds.size.height - CONTROL_BAR_HEIGHT,
            )
        } else {
            bounds
        }
    }

    /// Get the control bar bounds.
    fn control_bar_bounds(&self, bounds: Bounds) -> Bounds {
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            CONTROL_BAR_HEIGHT,
        )
    }

    /// Paint the control bar.
    fn paint_control_bar(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let bar = self.control_bar_bounds(bounds);

        // Background
        cx.scene.draw_quad(
            Quad::new(bar)
                .with_background(theme::bg::MUTED)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = 8.0;
        let mut x = bar.origin.x + padding;
        let y = bar.origin.y;

        if let Some(runner) = &self.runner {
            // State indicator
            let state = runner.state();
            let state_color = match state {
                RunnerState::Idle => theme::text::MUTED,
                RunnerState::Running => theme::status::SUCCESS,
                RunnerState::Paused => theme::status::WARNING,
                RunnerState::Stepping => theme::status::INFO,
                RunnerState::Passed => theme::status::SUCCESS,
                RunnerState::Failed => theme::status::ERROR,
                RunnerState::Aborted => theme::text::DISABLED,
            };

            // State badge
            let state_text = state.label();
            let state_width = state_text.len() as f32 * 8.0 + 12.0;
            let state_bounds = Bounds::new(x, y + 4.0, state_width, 24.0);
            cx.scene.draw_quad(
                Quad::new(state_bounds)
                    .with_background(state_color.with_alpha(0.2))
                    .with_border(state_color, 1.0),
            );
            let state_run = cx.text.layout(
                state_text,
                Point::new(x + 6.0, y + 9.0),
                theme::font_size::XS,
                state_color,
            );
            cx.scene.draw_text(state_run);
            x += state_width + padding;

            // Step counter
            let progress = runner.progress_string();
            let progress_run = cx.text.layout(
                &progress,
                Point::new(x, y + 9.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(progress_run);
            x += progress.len() as f32 * 7.0 + padding * 2.0;

            // Play/Pause button
            let play_text = if state == RunnerState::Running {
                "||"
            } else {
                ">"
            };
            let play_bounds = Bounds::new(x, y + 4.0, 28.0, 24.0);
            let play_bg = if self.play_hovered {
                theme::bg::HOVER
            } else {
                theme::bg::ELEVATED
            };
            cx.scene.draw_quad(
                Quad::new(play_bounds)
                    .with_background(play_bg)
                    .with_border(theme::border::DEFAULT, 1.0),
            );
            let play_run = cx.text.layout(
                play_text,
                Point::new(x + 8.0, y + 9.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(play_run);
            x += 36.0;

            // Step button
            let step_bounds = Bounds::new(x, y + 4.0, 28.0, 24.0);
            let step_bg = if self.step_hovered {
                theme::bg::HOVER
            } else {
                theme::bg::ELEVATED
            };
            cx.scene.draw_quad(
                Quad::new(step_bounds)
                    .with_background(step_bg)
                    .with_border(theme::border::DEFAULT, 1.0),
            );
            let step_run = cx.text.layout(
                ">|",
                Point::new(x + 6.0, y + 9.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(step_run);
            x += 36.0;

            // Speed indicator
            let speed = runner.speed().multiplier();
            let speed_text = format!("{:.1}x", speed);
            let speed_bounds = Bounds::new(x, y + 4.0, 40.0, 24.0);
            let speed_bg = if self.speed_hovered {
                theme::bg::HOVER
            } else {
                theme::bg::ELEVATED
            };
            cx.scene.draw_quad(
                Quad::new(speed_bounds)
                    .with_background(speed_bg)
                    .with_border(theme::border::DEFAULT, 1.0),
            );
            let speed_run = cx.text.layout(
                &speed_text,
                Point::new(x + 6.0, y + 9.0),
                theme::font_size::XS,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(speed_run);

            // Shortcuts hint (right-aligned)
            let hints = "[P]lay [S]tep [Space] [1-4]Speed";
            let hints_width = hints.len() as f32 * 6.0;
            let hints_run = cx.text.layout(
                hints,
                Point::new(
                    bar.origin.x + bar.size.width - hints_width - padding,
                    y + 9.0,
                ),
                theme::font_size::XS,
                theme::text::DISABLED,
            );
            cx.scene.draw_text(hints_run);
        } else {
            // No runner loaded
            let msg = "No test loaded";
            let msg_run = cx.text.layout(
                msg,
                Point::new(x, y + 9.0),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(msg_run);
        }
    }

    /// Handle control bar interaction.
    fn handle_control_bar_event(&mut self, event: &InputEvent, bounds: Bounds) -> EventResult {
        let bar = self.control_bar_bounds(bounds);
        let padding = 8.0;

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                if !bar.contains(point) {
                    self.play_hovered = false;
                    self.step_hovered = false;
                    self.speed_hovered = false;
                    return EventResult::Ignored;
                }

                // Calculate button positions
                let state_width = if let Some(runner) = &self.runner {
                    runner.state().label().len() as f32 * 8.0 + 12.0
                } else {
                    0.0
                };
                let progress_width = if let Some(runner) = &self.runner {
                    runner.progress_string().len() as f32 * 7.0
                } else {
                    0.0
                };

                let play_x =
                    bar.origin.x + padding + state_width + padding + progress_width + padding * 2.0;
                let step_x = play_x + 36.0;
                let speed_x = step_x + 36.0;

                let play_bounds = Bounds::new(play_x, bar.origin.y + 4.0, 28.0, 24.0);
                let step_bounds = Bounds::new(step_x, bar.origin.y + 4.0, 28.0, 24.0);
                let speed_bounds = Bounds::new(speed_x, bar.origin.y + 4.0, 40.0, 24.0);

                let was_play = self.play_hovered;
                let was_step = self.step_hovered;
                let was_speed = self.speed_hovered;

                self.play_hovered = play_bounds.contains(point);
                self.step_hovered = step_bounds.contains(point);
                self.speed_hovered = speed_bounds.contains(point);

                if was_play != self.play_hovered
                    || was_step != self.step_hovered
                    || was_speed != self.speed_hovered
                {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown {
                button: MouseButton::Left,
                x,
                y,
            } => {
                let point = Point::new(*x, *y);
                if !bar.contains(point) {
                    return EventResult::Ignored;
                }

                if self.play_hovered {
                    if let Some(runner) = &mut self.runner {
                        runner.toggle_pause();
                        return EventResult::Handled;
                    }
                }

                if self.step_hovered {
                    if let Some(runner) = &mut self.runner {
                        runner.step_mode();
                        runner.step_forward();
                        return EventResult::Handled;
                    }
                }

                if self.speed_hovered {
                    if let Some(runner) = &mut self.runner {
                        // Cycle through speeds
                        let current = runner.speed().multiplier();
                        let next = if current < 0.75 {
                            PlaybackSpeed::NORMAL
                        } else if current < 1.5 {
                            PlaybackSpeed::FAST
                        } else if current < 5.0 {
                            PlaybackSpeed::INSTANT
                        } else {
                            PlaybackSpeed::SLOW
                        };
                        runner.set_speed(next);
                        return EventResult::Handled;
                    }
                }
            }
            InputEvent::KeyDown { key, modifiers }
                if !modifiers.ctrl && !modifiers.alt && !modifiers.meta =>
            {
                if let Some(runner) = &mut self.runner {
                    match key {
                        Key::Character(c) if c == "p" || c == "P" => {
                            runner.toggle_pause();
                            return EventResult::Handled;
                        }
                        Key::Character(c) if c == "s" || c == "S" => {
                            runner.step_mode();
                            runner.step_forward();
                            return EventResult::Handled;
                        }
                        Key::Character(c) if c == " " => {
                            runner.toggle_pause();
                            return EventResult::Handled;
                        }
                        Key::Character(c) if c == "1" => {
                            runner.set_speed(PlaybackSpeed::SLOW);
                            return EventResult::Handled;
                        }
                        Key::Character(c) if c == "2" => {
                            runner.set_speed(PlaybackSpeed::NORMAL);
                            return EventResult::Handled;
                        }
                        Key::Character(c) if c == "3" => {
                            runner.set_speed(PlaybackSpeed::FAST);
                            return EventResult::Handled;
                        }
                        Key::Character(c) if c == "4" => {
                            runner.set_speed(PlaybackSpeed::INSTANT);
                            return EventResult::Handled;
                        }
                        Key::Named(NamedKey::Escape) => {
                            runner.abort();
                            return EventResult::Handled;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        EventResult::Ignored
    }

    /// Tick the test runner and generate events.
    fn tick_runner(&mut self) {
        let Some(runner) = &mut self.runner else {
            return;
        };

        // Handle step execution
        if runner.state().is_active() {
            // Check if we have an active event player
            if let Some(player) = &mut self.event_player {
                // Poll for events that should be dispatched
                while let Some(event) = player.poll() {
                    self.event_queue.push(event);
                }

                // Check if player is complete
                if player.is_complete() {
                    self.event_player = None;

                    // Complete the current step
                    let step_started = self.step_started.take();
                    let duration = step_started.map(|t| t.elapsed()).unwrap_or(Duration::ZERO);

                    let result = StepResult {
                        step_index: runner.current_step(),
                        duration,
                        assertion: None,
                        error: None,
                    };
                    runner.complete_step(result);
                }
            } else {
                // Start the next step
                if let Some(step) = runner.current_step_ref().cloned() {
                    self.step_started = Some(Instant::now());

                    // Check if this is an action step that needs event injection
                    match &step {
                        TestStep::Click { .. }
                        | TestStep::DoubleClick { .. }
                        | TestStep::Type { .. }
                        | TestStep::KeyPress { .. }
                        | TestStep::Scroll { .. }
                        | TestStep::MoveTo { .. } => {
                            // Generate events for this step
                            match generate_step_events(&step, &self.registry) {
                                Ok(sequence) => {
                                    let mut player = EventPlayer::new(sequence);
                                    player.start();
                                    self.event_player = Some(player);
                                }
                                Err(error) => {
                                    // Step failed
                                    let result = StepResult {
                                        step_index: runner.current_step(),
                                        duration: Duration::ZERO,
                                        assertion: None,
                                        error: Some(error),
                                    };
                                    runner.complete_step(result);
                                }
                            }
                        }
                        TestStep::Wait { duration } => {
                            // For wait steps, create an empty player with the delay
                            // Actually we should track elapsed time
                            if let Some(started) = self.step_started {
                                if started.elapsed() >= runner.speed().scale(*duration) {
                                    let result = StepResult {
                                        step_index: runner.current_step(),
                                        duration: *duration,
                                        assertion: None,
                                        error: None,
                                    };
                                    runner.complete_step(result);
                                    self.step_started = None;
                                }
                            }
                        }
                        TestStep::WaitFor { selector, timeout } => {
                            // Check if element exists
                            if Self::check_selector_exists(selector, &self.registry) {
                                let duration = self
                                    .step_started
                                    .map(|t| t.elapsed())
                                    .unwrap_or(Duration::ZERO);
                                let result = StepResult {
                                    step_index: runner.current_step(),
                                    duration,
                                    assertion: Some(AssertionResult::Passed),
                                    error: None,
                                };
                                runner.complete_step(result);
                                self.step_started = None;
                            } else if let Some(started) = self.step_started {
                                if started.elapsed() >= runner.speed().scale(*timeout) {
                                    let result = StepResult {
                                        step_index: runner.current_step(),
                                        duration: *timeout,
                                        assertion: Some(AssertionResult::failed(
                                            "Element not found within timeout",
                                        )),
                                        error: None,
                                    };
                                    runner.complete_step(result);
                                    self.step_started = None;
                                }
                            }
                        }
                        TestStep::Expect { selector } => {
                            let exists = Self::check_selector_exists(selector, &self.registry);
                            let assertion = if exists {
                                AssertionResult::Passed
                            } else {
                                AssertionResult::failed(format!("Element {:?} not found", selector))
                            };
                            let result = StepResult {
                                step_index: runner.current_step(),
                                duration: Duration::ZERO,
                                assertion: Some(assertion),
                                error: None,
                            };
                            runner.complete_step(result);
                            self.step_started = None;
                        }
                        TestStep::ExpectText { selector: _, text } => {
                            // For now, just check if the text exists anywhere
                            let found = self.registry.find_by_text(text).is_some();
                            let assertion = if found {
                                AssertionResult::Passed
                            } else {
                                AssertionResult::failed(format!("Text \"{}\" not found", text))
                            };
                            let result = StepResult {
                                step_index: runner.current_step(),
                                duration: Duration::ZERO,
                                assertion: Some(assertion),
                                error: None,
                            };
                            runner.complete_step(result);
                            self.step_started = None;
                        }
                        TestStep::ExpectVisible { selector } => {
                            // For visible check, same as exists for now
                            let exists = Self::check_selector_exists(selector, &self.registry);
                            let assertion = if exists {
                                AssertionResult::Passed
                            } else {
                                AssertionResult::failed(format!(
                                    "Element {:?} not visible",
                                    selector
                                ))
                            };
                            let result = StepResult {
                                step_index: runner.current_step(),
                                duration: Duration::ZERO,
                                assertion: Some(assertion),
                                error: None,
                            };
                            runner.complete_step(result);
                            self.step_started = None;
                        }
                    }
                }
            }
        }
    }

    /// Check if a selector matches any element.
    fn check_selector_exists(selector: &ElementSelector, registry: &ComponentRegistry) -> bool {
        match selector {
            ElementSelector::Id(id) => registry.find_by_id(*id).is_some(),
            ElementSelector::Text(text) => registry.find_by_text(text).is_some(),
            ElementSelector::Bounds(_b) => true, // Bounds always "exist"
            ElementSelector::Query(query) => {
                let parsed = ElementSelector::parse(query);
                match parsed {
                    ElementSelector::Query(_) => registry.find_by_text(query).is_some(),
                    other => Self::check_selector_exists(&other, registry),
                }
            }
        }
    }
}

impl Component for TestHarness {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Tick the runner
        self.tick_runner();

        // Clear registry for this frame
        self.registry.clear();

        // Paint control bar
        if self.show_controls {
            self.paint_control_bar(bounds, cx);
        }

        // Paint the component
        let component_bounds = self.component_bounds(bounds);
        self.component.paint(component_bounds, cx);

        // Paint overlay on top
        if self.show_overlay {
            self.overlay.paint(component_bounds, cx);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        // Handle control bar events
        if self.show_controls {
            let result = self.handle_control_bar_event(event, bounds);
            if result.is_handled() {
                return result;
            }
        }

        // Inject queued events from test runner
        let component_bounds = self.component_bounds(bounds);
        for queued in self.event_queue.drain(..) {
            self.overlay.observe_event(&queued);
            self.component.event(&queued, component_bounds, cx);
        }

        // Let overlay observe the event
        self.overlay.observe_event(event);

        // Forward to component
        self.component.event(event, component_bounds, cx)
    }

    fn id(&self) -> Option<ComponentId> {
        self.component.id()
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let (w, h) = self.component.size_hint();
        if self.show_controls {
            (w, h.map(|h| h + CONTROL_BAR_HEIGHT))
        } else {
            (w, h)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::dsl::test;
    use crate::{Bounds, EventContext, InputEvent, Key, Modifiers};

    // A minimal test component
    struct TestComponent;

    impl Component for TestComponent {
        fn paint(&mut self, _bounds: Bounds, _cx: &mut PaintContext) {}
    }

    #[test]
    fn test_harness_creation() {
        let harness = TestHarness::new(TestComponent);
        assert!(harness.runner().is_none());
    }

    #[test]
    fn test_harness_with_runner() {
        let runner = test("Test").click("#button").build();
        let harness = TestHarness::new(TestComponent).with_runner(runner);
        assert!(harness.runner().is_some());
        assert_eq!(harness.runner().unwrap().name(), "Test");
    }

    #[test]
    fn test_harness_show_options() {
        let harness = TestHarness::new(TestComponent)
            .show_overlay(false)
            .show_controls(false);
        assert!(!harness.show_overlay);
        assert!(!harness.show_controls);
    }

    #[test]
    fn test_component_bounds_with_controls() {
        let harness = TestHarness::new(TestComponent).show_controls(true);
        let full = Bounds::new(0.0, 0.0, 800.0, 600.0);
        let component = harness.component_bounds(full);

        assert_eq!(component.origin.y, CONTROL_BAR_HEIGHT);
        assert_eq!(component.size.height, 600.0 - CONTROL_BAR_HEIGHT);
    }

    #[test]
    fn test_component_bounds_without_controls() {
        let harness = TestHarness::new(TestComponent).show_controls(false);
        let full = Bounds::new(0.0, 0.0, 800.0, 600.0);
        let component = harness.component_bounds(full);

        assert_eq!(component.origin.y, 0.0);
        assert_eq!(component.size.height, 600.0);
    }

    #[test]
    fn test_control_bar_keyboard_shortcuts() {
        let runner = test("Controls").click("#button").build();
        let mut harness = TestHarness::new(TestComponent).with_runner(runner);
        let bounds = Bounds::new(0.0, 0.0, 800.0, 600.0);
        let mut cx = EventContext::new();

        let play = InputEvent::KeyDown {
            key: Key::Character("p".to_string()),
            modifiers: Modifiers::default(),
        };
        harness.event(&play, bounds, &mut cx);
        assert_eq!(harness.runner().unwrap().state(), RunnerState::Running);

        let pause = InputEvent::KeyDown {
            key: Key::Character(" ".to_string()),
            modifiers: Modifiers::default(),
        };
        harness.event(&pause, bounds, &mut cx);
        assert_eq!(harness.runner().unwrap().state(), RunnerState::Paused);

        let step = InputEvent::KeyDown {
            key: Key::Character("s".to_string()),
            modifiers: Modifiers::default(),
        };
        harness.event(&step, bounds, &mut cx);
        assert_eq!(harness.runner().unwrap().state(), RunnerState::Stepping);
    }
}
