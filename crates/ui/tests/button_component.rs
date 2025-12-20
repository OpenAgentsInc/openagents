//! Integration tests for Button component

use ui::{Button, ButtonSize, ButtonVariant};

// =========================================================================
// Basic rendering tests
// =========================================================================

#[test]
fn test_button_default_rendering() {
    let button = Button::new("Click me").render();
    let html = button.into_string();

    assert!(html.contains("Click me"));
    assert!(html.contains("<button"));
    assert!(html.contains("</button>"));
}

#[test]
fn test_button_with_label() {
    let button = Button::new("Submit").render();
    let html = button.into_string();

    assert!(html.contains("Submit"));
}

#[test]
fn test_button_empty_label() {
    let button = Button::new("").render();
    let html = button.into_string();

    assert!(html.contains("<button"));
    assert!(html.contains("</button>"));
}

#[test]
fn test_button_unicode_label() {
    let button = Button::new("提交 🚀").render();
    let html = button.into_string();

    assert!(html.contains("提交 🚀"));
}

#[test]
fn test_button_special_characters() {
    let button = Button::new("Save & Continue").render();
    let html = button.into_string();

    // Maud should escape HTML entities
    assert!(html.contains("Save &amp; Continue"));
}

#[test]
fn test_button_xss_prevention() {
    let button = Button::new("<script>alert('xss')</script>").render();
    let html = button.into_string();

    // Maud should escape the script tag
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("<script>"));
}

// =========================================================================
// Size variant tests
// =========================================================================

#[test]
fn test_button_default_size() {
    let button = Button::new("Default").render();
    let html = button.into_string();

    assert!(html.contains("px-4 py-2 text-sm"));
}

#[test]
fn test_button_small_size() {
    let button = Button::new("Small")
        .size(ButtonSize::Small)
        .render();
    let html = button.into_string();

    assert!(html.contains("px-2 py-1 text-xs"));
}

#[test]
fn test_button_large_size() {
    let button = Button::new("Large")
        .size(ButtonSize::Large)
        .render();
    let html = button.into_string();

    assert!(html.contains("px-6 py-3 text-base"));
}

// =========================================================================
// Variant tests
// =========================================================================

#[test]
fn test_button_primary_variant() {
    let button = Button::new("Primary")
        .variant(ButtonVariant::Primary)
        .render();
    let html = button.into_string();

    assert!(html.contains("bg-primary"));
    assert!(html.contains("text-primary-foreground"));
}

#[test]
fn test_button_secondary_variant() {
    let button = Button::new("Secondary")
        .variant(ButtonVariant::Secondary)
        .render();
    let html = button.into_string();

    assert!(html.contains("bg-secondary"));
    assert!(html.contains("text-secondary-foreground"));
}

#[test]
fn test_button_ghost_variant() {
    let button = Button::new("Ghost")
        .variant(ButtonVariant::Ghost)
        .render();
    let html = button.into_string();

    assert!(html.contains("bg-transparent"));
    assert!(html.contains("text-muted-foreground"));
}

// =========================================================================
// Disabled state tests
// =========================================================================

#[test]
fn test_button_not_disabled_by_default() {
    let button = Button::new("Normal").render();
    let html = button.into_string();

    assert!(!html.contains("opacity-50"));
    assert!(!html.contains("cursor-not-allowed"));
}

#[test]
fn test_button_disabled_true() {
    let button = Button::new("Disabled")
        .disabled(true)
        .render();
    let html = button.into_string();

    assert!(html.contains("opacity-50"));
    assert!(html.contains("cursor-not-allowed"));
    assert!(html.contains("disabled"));
}

#[test]
fn test_button_disabled_false() {
    let button = Button::new("Enabled")
        .disabled(false)
        .render();
    let html = button.into_string();

    assert!(!html.contains("opacity-50"));
}

// =========================================================================
// Base classes tests
// =========================================================================

#[test]
fn test_button_has_base_classes() {
    let button = Button::new("Test").render();
    let html = button.into_string();

    assert!(html.contains("inline-flex"));
    assert!(html.contains("items-center"));
    assert!(html.contains("gap-2"));
    assert!(html.contains("font-mono"));
    assert!(html.contains("cursor-pointer"));
    assert!(html.contains("transition-colors"));
}

// =========================================================================
// Builder pattern tests
// =========================================================================

#[test]
fn test_button_builder_chaining() {
    let button = Button::new("Chained")
        .variant(ButtonVariant::Secondary)
        .size(ButtonSize::Large)
        .disabled(false)
        .render();
    let html = button.into_string();

    assert!(html.contains("Chained"));
    assert!(html.contains("bg-secondary"));
    assert!(html.contains("px-6 py-3 text-base"));
}

#[test]
fn test_button_builder_all_options() {
    let button = Button::new("Full Options")
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Small)
        .disabled(true)
        .render();
    let html = button.into_string();

    assert!(html.contains("Full Options"));
    assert!(html.contains("bg-transparent"));
    assert!(html.contains("px-2 py-1 text-xs"));
    assert!(html.contains("opacity-50"));
}

// =========================================================================
// Combination tests
// =========================================================================

#[test]
fn test_button_primary_large() {
    let button = Button::new("Primary Large")
        .variant(ButtonVariant::Primary)
        .size(ButtonSize::Large)
        .render();
    let html = button.into_string();

    assert!(html.contains("bg-primary"));
    assert!(html.contains("px-6 py-3 text-base"));
}

#[test]
fn test_button_secondary_small_disabled() {
    let button = Button::new("Small Disabled")
        .variant(ButtonVariant::Secondary)
        .size(ButtonSize::Small)
        .disabled(true)
        .render();
    let html = button.into_string();

    assert!(html.contains("bg-secondary"));
    assert!(html.contains("px-2 py-1 text-xs"));
    assert!(html.contains("opacity-50"));
}

#[test]
fn test_button_ghost_default_enabled() {
    let button = Button::new("Ghost Normal")
        .variant(ButtonVariant::Ghost)
        .render();
    let html = button.into_string();

    assert!(html.contains("bg-transparent"));
    assert!(html.contains("px-4 py-2 text-sm")); // default size
    assert!(!html.contains("opacity-50"));
}

// =========================================================================
// Edge cases
// =========================================================================

#[test]
fn test_button_very_long_label() {
    let long_label = "A".repeat(1000);
    let button = Button::new(&long_label).render();
    let html = button.into_string();

    assert!(html.contains(&long_label));
}

#[test]
fn test_button_newlines_in_label() {
    let button = Button::new("Line 1\nLine 2").render();
    let html = button.into_string();

    assert!(html.contains("Line 1"));
    assert!(html.contains("Line 2"));
}

#[test]
fn test_button_html_entities_in_label() {
    let button = Button::new("Price: $10 & up").render();
    let html = button.into_string();

    assert!(html.contains("$10 &amp; up"));
}

#[test]
fn test_button_quotes_in_label() {
    let button = Button::new("Say \"Hello\"").render();
    let html = button.into_string();

    // Maud should escape quotes
    assert!(html.contains("Say &quot;Hello&quot;"));
}

// =========================================================================
// Variant enum tests
// =========================================================================

#[test]
fn test_button_variant_default() {
    let variant = ButtonVariant::default();
    assert!(matches!(variant, ButtonVariant::Primary));
}

// =========================================================================
// Size enum tests
// =========================================================================

#[test]
fn test_button_size_default() {
    let size = ButtonSize::default();
    assert!(matches!(size, ButtonSize::Default));
}

// =========================================================================
// Multiple buttons independence
// =========================================================================

#[test]
fn test_multiple_buttons_independent() {
    let button1 = Button::new("First")
        .variant(ButtonVariant::Primary)
        .render();
    let button2 = Button::new("Second")
        .variant(ButtonVariant::Ghost)
        .render();

    let html1 = button1.into_string();
    let html2 = button2.into_string();

    assert!(html1.contains("bg-primary"));
    assert!(html2.contains("bg-transparent"));
    assert!(html1.contains("First"));
    assert!(html2.contains("Second"));
}

// =========================================================================
// Accessibility tests
// =========================================================================

#[test]
fn test_button_disabled_attribute_set() {
    let button = Button::new("Test")
        .disabled(true)
        .render();
    let html = button.into_string();

    assert!(html.contains("disabled"));
}

#[test]
fn test_button_disabled_attribute_not_set() {
    let button = Button::new("Test")
        .disabled(false)
        .render();
    let html = button.into_string();

    // Should not have disabled attribute
    assert!(!html.contains("disabled"));
}
