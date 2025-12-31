//! Unit tests for WGPUI Framework (d-023)
//!
//! Tests covering:
//! - Component rendering basics (23.1.1, 23.1.2)
//! - Layout and bounds (23.2.1, 23.2.2, 23.2.3)
//! - Input handling (23.3.1, 23.3.2, 23.3.3, 23.3.4)
//! - Theme system
//! - Animation/easing (24.2.1)

use crate::animation::Easing;
use crate::components::atoms::{
    Mode, ModeBadge, Status, StatusDot, StreamingIndicator, ToolStatus, ToolStatusBadge,
};
use crate::components::{Button, Component, EventResult, Text};
use crate::{Bounds, InputEvent, Key, Modifiers, MouseButton, NamedKey, Point, theme};

// ============================================================================
// d-023.1.1: Quad Rendering (Colors and Borders)
// ============================================================================

#[test]
fn test_bounds_creation() {
    let bounds = Bounds::new(10.0, 20.0, 100.0, 50.0);
    assert_eq!(bounds.origin.x, 10.0);
    assert_eq!(bounds.origin.y, 20.0);
    assert_eq!(bounds.size.width, 100.0);
    assert_eq!(bounds.size.height, 50.0);
}

#[test]
fn test_bounds_contains_point() {
    let bounds = Bounds::new(0.0, 0.0, 100.0, 100.0);

    // Inside
    assert!(bounds.contains(Point::new(50.0, 50.0)));
    assert!(bounds.contains(Point::new(0.0, 0.0)));
    assert!(bounds.contains(Point::new(99.0, 99.0)));

    // Outside
    assert!(!bounds.contains(Point::new(-1.0, 50.0)));
    assert!(!bounds.contains(Point::new(50.0, -1.0)));
    assert!(!bounds.contains(Point::new(101.0, 50.0)));
    assert!(!bounds.contains(Point::new(50.0, 101.0)));
}

#[test]
fn test_bounds_zero() {
    let bounds = Bounds::ZERO;
    assert_eq!(bounds.origin.x, 0.0);
    assert_eq!(bounds.origin.y, 0.0);
    assert_eq!(bounds.size.width, 0.0);
    assert_eq!(bounds.size.height, 0.0);
}

// ============================================================================
// d-023.1.2: Text Rendering
// ============================================================================

#[test]
fn test_text_component_creation() {
    let text = Text::new("Hello World");
    assert_eq!(text.content(), "Hello World");
}

#[test]
fn test_text_component_builder() {
    let text = Text::new("Test")
        .font_size(16.0)
        .color(theme::text::PRIMARY);

    assert_eq!(text.content(), "Test");
}

#[test]
fn test_text_component_with_different_sizes() {
    let small = Text::new("Small").font_size(12.0);
    let medium = Text::new("Medium").font_size(14.0);
    let large = Text::new("Large").font_size(20.0);

    assert_eq!(small.content(), "Small");
    assert_eq!(medium.content(), "Medium");
    assert_eq!(large.content(), "Large");
}

// ============================================================================
// d-023.1.3: GPU Rendering (60fps Target via VSync)
// ============================================================================

#[test]
fn test_surface_config_targets_vsync() {
    let config =
        crate::platform::default_surface_config(800, 600, wgpu::TextureFormat::Bgra8UnormSrgb);

    assert_eq!(config.present_mode, wgpu::PresentMode::AutoVsync);
    assert_eq!(config.desired_maximum_frame_latency, 2);
}

// ============================================================================
// d-023.2: Layout (Bounds and Spacing)
// ============================================================================

#[test]
fn test_theme_spacing_scale() {
    // Verify spacing scale is consistent
    assert!(theme::spacing::XS < theme::spacing::SM);
    assert!(theme::spacing::SM < theme::spacing::MD);
    assert!(theme::spacing::MD < theme::spacing::LG);
    assert!(theme::spacing::LG < theme::spacing::XL);
}

#[test]
fn test_theme_font_size_scale() {
    // Verify font size scale is consistent
    assert!(theme::font_size::XS < theme::font_size::SM);
    assert!(theme::font_size::SM < theme::font_size::BASE);
    assert!(theme::font_size::BASE < theme::font_size::LG);
    assert!(theme::font_size::LG < theme::font_size::XL);
}

#[test]
fn test_bounds_with_padding() {
    let outer = Bounds::new(0.0, 0.0, 200.0, 100.0);
    let padding = theme::spacing::MD;

    let inner = Bounds::new(
        outer.origin.x + padding,
        outer.origin.y + padding,
        outer.size.width - padding * 2.0,
        outer.size.height - padding * 2.0,
    );

    assert!(inner.size.width < outer.size.width);
    assert!(inner.size.height < outer.size.height);
    assert_eq!(inner.size.width, outer.size.width - padding * 2.0);
}

// ============================================================================
// d-023.3.1: Mouse Click Handling
// ============================================================================

#[test]
fn test_mouse_down_event_creation() {
    let event = InputEvent::MouseDown {
        button: MouseButton::Left,
        x: 50.0,
        y: 75.0,
    };

    if let InputEvent::MouseDown { button, x, y } = event {
        assert_eq!(button, MouseButton::Left);
        assert_eq!(x, 50.0);
        assert_eq!(y, 75.0);
    } else {
        panic!("Expected MouseDown event");
    }
}

#[test]
fn test_mouse_up_event_creation() {
    let event = InputEvent::MouseUp {
        button: MouseButton::Left,
        x: 50.0,
        y: 75.0,
    };

    if let InputEvent::MouseUp { button, x, y } = event {
        assert_eq!(button, MouseButton::Left);
        assert_eq!(x, 50.0);
        assert_eq!(y, 75.0);
    } else {
        panic!("Expected MouseUp event");
    }
}

#[test]
fn test_right_click_event() {
    let event = InputEvent::MouseDown {
        button: MouseButton::Right,
        x: 100.0,
        y: 100.0,
    };

    if let InputEvent::MouseDown { button, .. } = event {
        assert_eq!(button, MouseButton::Right);
    } else {
        panic!("Expected MouseDown event");
    }
}

// ============================================================================
// d-023.3.2: Keyboard Input
// ============================================================================

#[test]
fn test_key_down_event() {
    let event = InputEvent::KeyDown {
        key: Key::Character("a".to_string()),
        modifiers: Modifiers::default(),
    };

    if let InputEvent::KeyDown { key, modifiers } = event {
        assert!(!modifiers.shift);
        assert!(!modifiers.ctrl);
        if let Key::Character(c) = key {
            assert_eq!(c, "a");
        }
    } else {
        panic!("Expected KeyDown event");
    }
}

#[test]
fn test_named_key_event() {
    let event = InputEvent::KeyDown {
        key: Key::Named(NamedKey::Enter),
        modifiers: Modifiers::default(),
    };

    if let InputEvent::KeyDown { key, .. } = event {
        assert!(matches!(key, Key::Named(NamedKey::Enter)));
    } else {
        panic!("Expected KeyDown event");
    }
}

#[test]
fn test_keyboard_modifiers() {
    let shift = Modifiers {
        shift: true,
        ctrl: false,
        alt: false,
        meta: false,
    };
    let ctrl = Modifiers {
        shift: false,
        ctrl: true,
        alt: false,
        meta: false,
    };
    let alt = Modifiers {
        shift: false,
        ctrl: false,
        alt: true,
        meta: false,
    };

    assert!(shift.shift);
    assert!(!shift.ctrl);

    // Combined modifiers
    let combined = Modifiers {
        shift: true,
        ctrl: true,
        alt: false,
        meta: false,
    };
    assert!(combined.shift);
    assert!(combined.ctrl);
    assert!(!combined.alt);
}

// ============================================================================
// d-023.3.3: Mouse Hover
// ============================================================================

#[test]
fn test_mouse_move_event() {
    let event = InputEvent::MouseMove { x: 150.0, y: 200.0 };

    if let InputEvent::MouseMove { x, y } = event {
        assert_eq!(x, 150.0);
        assert_eq!(y, 200.0);
    } else {
        panic!("Expected MouseMove event");
    }
}

#[test]
fn test_hover_detection_with_bounds() {
    let button_bounds = Bounds::new(100.0, 100.0, 80.0, 30.0);

    // Inside - should be hovering
    let inside_point = Point::new(140.0, 115.0);
    assert!(button_bounds.contains(inside_point));

    // Outside - not hovering
    let outside_point = Point::new(50.0, 50.0);
    assert!(!button_bounds.contains(outside_point));
}

// ============================================================================
// d-023.3.4: Scroll Events
// ============================================================================

#[test]
fn test_scroll_event() {
    let event = InputEvent::Scroll { dx: 0.0, dy: -10.0 };

    if let InputEvent::Scroll { dx, dy } = event {
        assert_eq!(dx, 0.0);
        assert_eq!(dy, -10.0);
    } else {
        panic!("Expected Scroll event");
    }
}

#[test]
fn test_horizontal_scroll() {
    let event = InputEvent::Scroll { dx: 15.0, dy: 0.0 };

    if let InputEvent::Scroll { dx, dy } = event {
        assert_eq!(dx, 15.0);
        assert_eq!(dy, 0.0);
    } else {
        panic!("Expected Scroll event");
    }
}

// ============================================================================
// d-024.2.1: Easing Functions
// ============================================================================

#[test]
fn test_linear_easing() {
    let easing = Easing::Linear;
    assert_eq!(easing.apply(0.0), 0.0);
    assert_eq!(easing.apply(0.5), 0.5);
    assert_eq!(easing.apply(1.0), 1.0);
}

#[test]
fn test_ease_in_out_quad() {
    let easing = Easing::EaseInOutQuad;

    // Start and end should be 0 and 1
    assert_eq!(easing.apply(0.0), 0.0);
    assert_eq!(easing.apply(1.0), 1.0);

    // Midpoint should be 0.5
    let mid = easing.apply(0.5);
    assert!((mid - 0.5).abs() < 0.01);
}

#[test]
fn test_ease_out_quad() {
    let easing = Easing::EaseOutQuad;

    assert_eq!(easing.apply(0.0), 0.0);
    assert_eq!(easing.apply(1.0), 1.0);

    // EaseOut should be faster at the start
    let quarter = easing.apply(0.25);
    assert!(quarter > 0.25); // Should be ahead of linear
}

#[test]
fn test_ease_in_quad() {
    let easing = Easing::EaseInQuad;

    assert_eq!(easing.apply(0.0), 0.0);
    assert_eq!(easing.apply(1.0), 1.0);

    // EaseIn should be slower at the start
    let quarter = easing.apply(0.25);
    assert!(quarter < 0.25); // Should be behind linear
}

#[test]
fn test_all_easing_functions_valid_range() {
    let easings = [
        Easing::Linear,
        Easing::EaseIn,
        Easing::EaseOut,
        Easing::EaseInOut,
        Easing::EaseInQuad,
        Easing::EaseOutQuad,
        Easing::EaseInOutQuad,
        Easing::EaseInCubic,
        Easing::EaseOutCubic,
        Easing::EaseInOutCubic,
        Easing::EaseInQuart,
        Easing::EaseOutQuart,
        Easing::EaseInOutQuart,
        Easing::EaseInSine,
        Easing::EaseOutSine,
        Easing::EaseInOutSine,
    ];

    for easing in easings {
        // All should start at 0 and end at 1
        assert!(
            (easing.apply(0.0) - 0.0).abs() < 0.001,
            "{:?} failed at 0.0",
            easing
        );
        assert!(
            (easing.apply(1.0) - 1.0).abs() < 0.001,
            "{:?} failed at 1.0",
            easing
        );

        // Output should be in reasonable range
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let result = easing.apply(t);
            assert!(
                result >= -0.1 && result <= 1.1,
                "{:?} out of range at {}: {}",
                easing,
                t,
                result
            );
        }
    }
}

#[test]
fn test_extended_easing_functions_valid_range() {
    let easings = [
        Easing::EaseInQuint,
        Easing::EaseOutQuint,
        Easing::EaseInOutQuint,
        Easing::EaseInExpo,
        Easing::EaseOutExpo,
        Easing::EaseInOutExpo,
        Easing::EaseInCirc,
        Easing::EaseOutCirc,
        Easing::EaseInOutCirc,
        Easing::EaseInBounce,
        Easing::EaseOutBounce,
        Easing::EaseInOutBounce,
        Easing::EaseInElastic,
        Easing::EaseOutElastic,
        Easing::EaseInOutElastic,
        Easing::EaseInBack,
        Easing::EaseOutBack,
        Easing::EaseInOutBack,
        Easing::CubicBezier(0.42, 0.0, 0.58, 1.0),
    ];

    for easing in easings {
        assert!(
            (easing.apply(0.0) - 0.0).abs() < 0.001,
            "{:?} failed at 0.0",
            easing
        );
        assert!(
            (easing.apply(1.0) - 1.0).abs() < 0.001,
            "{:?} failed at 1.0",
            easing
        );

        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let result = easing.apply(t);
            assert!(
                result.is_finite(),
                "{:?} returned non-finite at {}",
                easing,
                t
            );
            assert!(
                result >= -2.0 && result <= 2.0,
                "{:?} out of range at {}: {}",
                easing,
                t,
                result
            );
        }
    }
}

#[test]
fn test_bounce_easing() {
    let easing = Easing::EaseOutBounce;

    assert_eq!(easing.apply(0.0), 0.0);
    assert_eq!(easing.apply(1.0), 1.0);

    // Bounce can overshoot slightly
    let mid = easing.apply(0.5);
    assert!(mid >= 0.0 && mid <= 1.5);
}

// ============================================================================
// Component State Tests
// ============================================================================

#[test]
fn test_status_dot_states() {
    let online = StatusDot::new(Status::Online);
    let offline = StatusDot::new(Status::Offline);
    let error = StatusDot::new(Status::Error);
    let busy = StatusDot::new(Status::Busy);

    // Just verify they can be created without panic
    let _ = online;
    let _ = offline;
    let _ = error;
    let _ = busy;
}

#[test]
fn test_tool_status_badge_states() {
    let pending = ToolStatusBadge::new(ToolStatus::Pending);
    let running = ToolStatusBadge::new(ToolStatus::Running);
    let success = ToolStatusBadge::new(ToolStatus::Success);
    let error = ToolStatusBadge::new(ToolStatus::Error);

    assert_eq!(pending.status(), ToolStatus::Pending);
    assert_eq!(running.status(), ToolStatus::Running);
    assert_eq!(success.status(), ToolStatus::Success);
    assert_eq!(error.status(), ToolStatus::Error);
}

#[test]
fn test_mode_badge_variants() {
    let code = ModeBadge::new(Mode::Code);
    let plan = ModeBadge::new(Mode::Plan);
    let act = ModeBadge::new(Mode::Act);
    let normal = ModeBadge::new(Mode::Normal);
    let chat = ModeBadge::new(Mode::Chat);

    // Verify modes can be created without panic
    let _ = code;
    let _ = plan;
    let _ = act;
    let _ = normal;
    let _ = chat;
}

// ============================================================================
// Theme Color Tests
// ============================================================================

#[test]
fn test_theme_colors_exist() {
    // Background colors
    let _ = theme::bg::APP;
    let _ = theme::bg::SURFACE;
    let _ = theme::bg::MUTED;

    // Text colors
    let _ = theme::text::PRIMARY;
    let _ = theme::text::SECONDARY;
    let _ = theme::text::MUTED;

    // Status colors
    let _ = theme::status::SUCCESS;
    let _ = theme::status::ERROR;
    let _ = theme::status::WARNING;

    // Accent colors
    let _ = theme::accent::PRIMARY;
    let _ = theme::accent::GREEN;
    let _ = theme::accent::RED;
}

#[test]
fn test_hsla_alpha_modification() {
    let color = theme::accent::PRIMARY;
    let semi_transparent = color.with_alpha(0.5);

    assert_eq!(semi_transparent.a, 0.5);
    assert_eq!(semi_transparent.h, color.h);
    assert_eq!(semi_transparent.s, color.s);
    assert_eq!(semi_transparent.l, color.l);
}

// ============================================================================
// Event Result Tests
// ============================================================================

#[test]
fn test_event_result_variants() {
    let handled = EventResult::Handled;
    let ignored = EventResult::Ignored;

    assert!(matches!(handled, EventResult::Handled));
    assert!(matches!(ignored, EventResult::Ignored));
}

// ============================================================================
// Button Component Tests
// ============================================================================

#[test]
fn test_button_creation() {
    let button = Button::new("Click Me");
    assert_eq!(button.label(), "Click Me");
}

#[test]
fn test_button_with_id() {
    let button = Button::new("Test").with_id(42);
    assert_eq!(button.id(), Some(42));
}

#[test]
fn test_button_disabled_builder() {
    // Verify the disabled() builder method exists
    let _enabled = Button::new("Enabled").disabled(false);
    let _disabled = Button::new("Disabled").disabled(true);
}

// ============================================================================
// Component Size Hints
// ============================================================================

#[test]
fn test_streaming_indicator_size_hint() {
    let indicator = StreamingIndicator::new().dot_count(3).dot_size(4.0);
    let (width, height) = indicator.size_hint();

    // Should provide size hints
    assert!(width.is_some());
    assert!(height.is_some());

    // Width should be dots + gaps
    let expected_width = 3.0 * 4.0 + 2.0 * 4.0; // 3 dots * 4px + 2 gaps * 4px
    assert_eq!(width.unwrap(), expected_width);
}

// ============================================================================
// Point and Size Tests
// ============================================================================

#[test]
fn test_point_creation() {
    let point = Point::new(10.0, 20.0);
    assert_eq!(point.x, 10.0);
    assert_eq!(point.y, 20.0);
}

#[test]
fn test_point_zero() {
    let point = Point::ZERO;
    assert_eq!(point.x, 0.0);
    assert_eq!(point.y, 0.0);
}

#[test]
fn test_size_creation() {
    let size = crate::Size::new(100.0, 50.0);
    assert_eq!(size.width, 100.0);
    assert_eq!(size.height, 50.0);
}

// ============================================================================
// Animation Lerp Tests
// ============================================================================

#[test]
fn test_f32_lerp() {
    use crate::animation::Animatable;

    assert_eq!(f32::lerp(0.0, 100.0, 0.0), 0.0);
    assert_eq!(f32::lerp(0.0, 100.0, 0.5), 50.0);
    assert_eq!(f32::lerp(0.0, 100.0, 1.0), 100.0);
}

#[test]
fn test_point_lerp() {
    use crate::animation::Animatable;

    let from = Point::new(0.0, 0.0);
    let to = Point::new(100.0, 200.0);

    let mid = Point::lerp(from, to, 0.5);
    assert_eq!(mid.x, 50.0);
    assert_eq!(mid.y, 100.0);
}

// ============================================================================
// Text Sequence Tests (d-024.3.1)
// ============================================================================

#[test]
fn test_text_sequence_character_reveal_logic() {
    // Test that text reveals character by character
    let text = "Hello";
    let chars: Vec<char> = text.chars().collect();

    // At 0% progress, no characters visible
    let visible_at_0 = (0.0 * chars.len() as f32) as usize;
    assert_eq!(visible_at_0, 0);

    // At 50% progress, half visible
    let visible_at_50 = (0.5 * chars.len() as f32) as usize;
    assert_eq!(visible_at_50, 2);

    // At 100% progress, all visible
    let visible_at_100 = (1.0 * chars.len() as f32) as usize;
    assert_eq!(visible_at_100, 5);
}

#[test]
fn test_character_by_character_iteration() {
    let text = "Test";
    let mut visible = String::new();

    for (i, ch) in text.chars().enumerate() {
        let progress = (i + 1) as f32 / text.len() as f32;
        visible.push(ch);

        // At each step, we should have i+1 characters visible
        assert_eq!(visible.len(), i + 1);
        assert!(progress <= 1.0);
    }

    assert_eq!(visible, "Test");
}

// ============================================================================
// Named Key Tests
// ============================================================================

#[test]
fn test_all_named_keys() {
    let keys = [
        NamedKey::Enter,
        NamedKey::Escape,
        NamedKey::Backspace,
        NamedKey::Delete,
        NamedKey::Tab,
        NamedKey::Home,
        NamedKey::End,
        NamedKey::ArrowUp,
        NamedKey::ArrowDown,
        NamedKey::ArrowLeft,
        NamedKey::ArrowRight,
    ];

    for key in keys {
        let event = InputEvent::KeyDown {
            key: Key::Named(key.clone()),
            modifiers: Modifiers::default(),
        };

        if let InputEvent::KeyDown {
            key: Key::Named(k), ..
        } = event
        {
            assert_eq!(k, key);
        } else {
            panic!("Expected KeyDown with Named key");
        }
    }
}
