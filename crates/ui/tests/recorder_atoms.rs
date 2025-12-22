//! Integration tests for Recorder Atoms components
//!
//! These tests verify HTML structure, accessibility, and XSS prevention
//! for all recorder atom components following directive d-013.

use ui::recorder::atoms::*;

// =============================================================================
// attempt_badge tests
// =============================================================================

#[test]
fn test_attempt_badge_basic_rendering() {
    let badge = attempt_badge(1, 3);
    let html = badge.into_string();

    assert!(html.contains("1/3"));
    assert!(html.contains("<span"));
    assert!(html.contains("</span>"));
}

#[test]
fn test_attempt_badge_has_title_attribute() {
    let badge = attempt_badge(2, 5);
    let html = badge.into_string();

    assert!(html.contains("title=\"Attempt 2 of 5\""));
}

#[test]
fn test_attempt_badge_styling() {
    let badge = attempt_badge(1, 1);
    let html = badge.into_string();

    assert!(html.contains("text-xs"));
    assert!(html.contains("text-orange"));
    assert!(html.contains("tabular-nums"));
}

#[test]
fn test_attempt_badge_first_attempt() {
    let badge = attempt_badge(1, 10);
    let html = badge.into_string();

    assert!(html.contains("1/10"));
}

#[test]
fn test_attempt_badge_last_attempt() {
    let badge = attempt_badge(5, 5);
    let html = badge.into_string();

    assert!(html.contains("5/5"));
}

#[test]
fn test_attempt_badge_max_u8_values() {
    let badge = attempt_badge(255, 255);
    let html = badge.into_string();

    assert!(html.contains("255/255"));
}

// =============================================================================
// cost_badge tests
// =============================================================================

#[test]
fn test_cost_badge_basic_rendering() {
    let badge = cost_badge(0.0050);
    let html = badge.into_string();

    assert!(html.contains("$0.0050"));
    assert!(html.contains("<span"));
    assert!(html.contains("</span>"));
}

#[test]
fn test_cost_badge_green_color_low_cost() {
    // < $0.01 should be green
    let badge = cost_badge(0.0050);
    let html = badge.into_string();

    assert!(html.contains("text-green"));
}

#[test]
fn test_cost_badge_yellow_color_medium_cost() {
    // >= $0.01 and < $0.10 should be yellow
    let badge = cost_badge(0.0500);
    let html = badge.into_string();

    assert!(html.contains("text-yellow"));
}

#[test]
fn test_cost_badge_red_color_high_cost() {
    // >= $0.10 should be red
    let badge = cost_badge(0.5000);
    let html = badge.into_string();

    assert!(html.contains("text-red"));
}

#[test]
fn test_cost_badge_has_title_attribute() {
    let badge = cost_badge(1.2345);
    let html = badge.into_string();

    assert!(html.contains("title=\"Cost: $1.2345\""));
}

#[test]
fn test_cost_badge_styling() {
    let badge = cost_badge(0.1000);
    let html = badge.into_string();

    assert!(html.contains("text-xs"));
    assert!(html.contains("tabular-nums"));
}

#[test]
fn test_cost_badge_zero_cost() {
    let badge = cost_badge(0.0000);
    let html = badge.into_string();

    assert!(html.contains("$0.0000"));
    assert!(html.contains("text-green"));
}

#[test]
fn test_cost_badge_edge_case_boundary_low() {
    // Exactly $0.01 should be yellow (>= threshold)
    let badge = cost_badge(0.0100);
    let html = badge.into_string();

    assert!(html.contains("text-yellow"));
}

#[test]
fn test_cost_badge_edge_case_boundary_high() {
    // Exactly $0.10 should be red (>= threshold)
    let badge = cost_badge(0.1000);
    let html = badge.into_string();

    assert!(html.contains("text-red"));
}

#[test]
fn test_cost_badge_very_high_cost() {
    let badge = cost_badge(99.9999);
    let html = badge.into_string();

    assert!(html.contains("$99.9999"));
    assert!(html.contains("text-red"));
}

// =============================================================================
// status_dot tests
// =============================================================================

#[test]
fn test_status_dot_success() {
    let dot = status_dot(StatusState::Success);
    let html = dot.into_string();

    assert!(html.contains("text-green"));
    assert!(html.contains("title=\"success\""));
    assert!(html.contains("\u{25CF}")); // ●
}

#[test]
fn test_status_dot_running() {
    let dot = status_dot(StatusState::Running);
    let html = dot.into_string();

    assert!(html.contains("text-blue"));
    assert!(html.contains("title=\"running\""));
    assert!(html.contains("\u{25CF}")); // ●
}

#[test]
fn test_status_dot_pending() {
    let dot = status_dot(StatusState::Pending);
    let html = dot.into_string();

    assert!(html.contains("text-yellow"));
    assert!(html.contains("title=\"pending\""));
    assert!(html.contains("\u{25CF}")); // ●
}

#[test]
fn test_status_dot_error() {
    let dot = status_dot(StatusState::Error);
    let html = dot.into_string();

    assert!(html.contains("text-red"));
    assert!(html.contains("title=\"error\""));
    assert!(html.contains("\u{25CF}")); // ●
}

#[test]
fn test_status_dot_skipped() {
    let dot = status_dot(StatusState::Skipped);
    let html = dot.into_string();

    assert!(html.contains("text-muted-foreground"));
    assert!(html.contains("opacity-60"));
    assert!(html.contains("title=\"skipped\""));
    assert!(html.contains("\u{25CB}")); // ○ (hollow circle)
}

#[test]
fn test_status_dot_has_span_tag() {
    let dot = status_dot(StatusState::Success);
    let html = dot.into_string();

    assert!(html.contains("<span"));
    assert!(html.contains("</span>"));
}

#[test]
fn test_status_dot_base_styling() {
    let dot = status_dot(StatusState::Success);
    let html = dot.into_string();

    assert!(html.contains("text-xs"));
    assert!(html.contains("leading-none"));
}

#[test]
fn test_status_state_all_variants_accessible() {
    // Verify all variants have accessible titles
    let states = [
        StatusState::Success,
        StatusState::Running,
        StatusState::Pending,
        StatusState::Error,
        StatusState::Skipped,
    ];

    for state in &states {
        let dot = status_dot(*state);
        let html = dot.into_string();

        // Every status must have a title attribute for accessibility
        assert!(html.contains("title="));
    }
}

// =============================================================================
// StatusState enum tests
// =============================================================================

#[test]
fn test_status_state_clone() {
    let state = StatusState::Success;
    let cloned = state.clone();
    assert_eq!(state, cloned);
}

#[test]
fn test_status_state_copy() {
    let state = StatusState::Error;
    let copied = state;
    assert_eq!(state, copied);
}

#[test]
fn test_status_state_equality() {
    assert_eq!(StatusState::Success, StatusState::Success);
    assert_ne!(StatusState::Success, StatusState::Error);
}

// =============================================================================
// Accessibility tests
// =============================================================================

#[test]
fn test_attempt_badge_accessibility_title() {
    // All badges must have title attributes for screen readers
    let badge = attempt_badge(1, 3);
    let html = badge.into_string();

    assert!(html.contains("title="));
}

#[test]
fn test_cost_badge_accessibility_title() {
    let badge = cost_badge(0.50);
    let html = badge.into_string();

    assert!(html.contains("title="));
}

#[test]
fn test_status_dot_accessibility_title() {
    let dot = status_dot(StatusState::Running);
    let html = dot.into_string();

    assert!(html.contains("title="));
}

// =============================================================================
// Edge cases and robustness
// =============================================================================

#[test]
fn test_attempt_badge_zero_attempts() {
    let badge = attempt_badge(0, 0);
    let html = badge.into_string();

    assert!(html.contains("0/0"));
}

#[test]
fn test_cost_badge_negative_cost() {
    // Testing robustness - negative costs shouldn't crash
    let badge = cost_badge(-1.0);
    let html = badge.into_string();

    assert!(html.contains("$-1.0000"));
    // Negative should still use green class (< 0.01)
    assert!(html.contains("text-green"));
}

#[test]
fn test_cost_badge_very_small_cost() {
    let badge = cost_badge(0.0001);
    let html = badge.into_string();

    assert!(html.contains("$0.0001"));
}

#[test]
fn test_multiple_badges_independence() {
    let badge1 = attempt_badge(1, 5);
    let badge2 = attempt_badge(3, 5);

    let html1 = badge1.into_string();
    let html2 = badge2.into_string();

    assert!(html1.contains("1/5"));
    assert!(html2.contains("3/5"));
}

// =============================================================================
// blob_ref tests
// =============================================================================

#[test]
fn test_blob_ref_basic_rendering() {
    let blob = blob_ref("abcdef1234567890abcdef1234567890", 1024, None);
    let html = blob.into_string();

    assert!(html.contains("@blob sha256="));
    assert!(html.contains("<span"));
    assert!(html.contains("</span>"));
}

#[test]
fn test_blob_ref_short_sha_display() {
    let blob = blob_ref("abcdef1234567890abcdef1234567890", 1024, None);
    let html = blob.into_string();

    // Should show first 8 characters
    assert!(html.contains("abcdef12"));
}

#[test]
fn test_blob_ref_full_sha_in_title() {
    let full_sha = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
    let blob = blob_ref(full_sha, 1024, None);
    let html = blob.into_string();

    assert!(html.contains(&format!("title=\"Blob: {}\"", full_sha)));
}

#[test]
fn test_blob_ref_size_bytes() {
    let blob = blob_ref("abc123", 500, None);
    let html = blob.into_string();

    assert!(html.contains("500B"));
}

#[test]
fn test_blob_ref_size_kilobytes() {
    let blob = blob_ref("abc123", 2048, None);
    let html = blob.into_string();

    assert!(html.contains("2.0KB"));
}

#[test]
fn test_blob_ref_size_megabytes() {
    let blob = blob_ref("abc123", 2 * 1024 * 1024, None);
    let html = blob.into_string();

    assert!(html.contains("2.0MB"));
}

#[test]
fn test_blob_ref_with_mime_type() {
    let blob = blob_ref("abc123", 1024, Some("image/png"));
    let html = blob.into_string();

    assert!(html.contains("image/png"));
}

#[test]
fn test_blob_ref_without_mime_type() {
    let blob = blob_ref("abc123", 1024, None);
    let html = blob.into_string();

    // Should still render but without mime type section
    assert!(html.contains("1.0KB"));
}

#[test]
fn test_blob_ref_styling() {
    let blob = blob_ref("abc123", 1024, None);
    let html = blob.into_string();

    assert!(html.contains("text-cyan"));
    assert!(html.contains("text-xs"));
    assert!(html.contains("inline-flex"));
}

#[test]
fn test_blob_ref_short_sha() {
    // Test with SHA shorter than 8 chars
    let blob = blob_ref("abc", 100, None);
    let html = blob.into_string();

    assert!(html.contains("abc"));
}

#[test]
fn test_blob_ref_xss_prevention_in_mime() {
    let blob = blob_ref("abc123", 1024, Some("<script>alert('xss')</script>"));
    let html = blob.into_string();

    // Maud should escape the script tag
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("<script>"));
}

// =============================================================================
// latency_badge tests
// =============================================================================

#[test]
fn test_latency_badge_milliseconds() {
    let badge = latency_badge(500);
    let html = badge.into_string();

    assert!(html.contains("500ms"));
}

#[test]
fn test_latency_badge_seconds() {
    let badge = latency_badge(2500);
    let html = badge.into_string();

    assert!(html.contains("2.5s"));
}

#[test]
fn test_latency_badge_green_fast() {
    // < 1000ms should be green
    let badge = latency_badge(500);
    let html = badge.into_string();

    assert!(html.contains("text-green"));
}

#[test]
fn test_latency_badge_yellow_medium() {
    // >= 1000ms and < 5000ms should be yellow
    let badge = latency_badge(2000);
    let html = badge.into_string();

    assert!(html.contains("text-yellow"));
}

#[test]
fn test_latency_badge_red_slow() {
    // >= 5000ms should be red
    let badge = latency_badge(6000);
    let html = badge.into_string();

    assert!(html.contains("text-red"));
}

#[test]
fn test_latency_badge_has_title() {
    let badge = latency_badge(1234);
    let html = badge.into_string();

    assert!(html.contains("title=\"Latency: 1234ms\""));
}

#[test]
fn test_latency_badge_styling() {
    let badge = latency_badge(100);
    let html = badge.into_string();

    assert!(html.contains("text-xs"));
    assert!(html.contains("tabular-nums"));
}

#[test]
fn test_latency_badge_boundary_1000ms() {
    // Exactly 1000ms should be yellow (>= threshold)
    let badge = latency_badge(1000);
    let html = badge.into_string();

    assert!(html.contains("text-yellow"));
    assert!(html.contains("1.0s"));
}

#[test]
fn test_latency_badge_boundary_5000ms() {
    // Exactly 5000ms should be red (>= threshold)
    let badge = latency_badge(5000);
    let html = badge.into_string();

    assert!(html.contains("text-red"));
    assert!(html.contains("5.0s"));
}

#[test]
fn test_latency_badge_zero() {
    let badge = latency_badge(0);
    let html = badge.into_string();

    assert!(html.contains("0ms"));
    assert!(html.contains("text-green"));
}

// =============================================================================
// token_badge tests
// =============================================================================

#[test]
fn test_token_badge_basic_rendering() {
    let badge = token_badge(100, 50, None);
    let html = badge.into_string();

    assert!(html.contains("100"));
    assert!(html.contains("50"));
    assert!(html.contains("in"));
    assert!(html.contains("out"));
}

#[test]
fn test_token_badge_formatted_thousands() {
    let badge = token_badge(5000, 3500, None);
    let html = badge.into_string();

    assert!(html.contains("5.0k"));
    assert!(html.contains("3.5k"));
}

#[test]
fn test_token_badge_without_cache() {
    let badge = token_badge(100, 50, None);
    let html = badge.into_string();

    assert!(!html.contains("cached"));
}

#[test]
fn test_token_badge_with_cache() {
    let badge = token_badge(100, 50, Some(75));
    let html = badge.into_string();

    assert!(html.contains("cached"));
    assert!(html.contains("75"));
}

#[test]
fn test_token_badge_with_cache_formatted() {
    let badge = token_badge(5000, 3000, Some(2000));
    let html = badge.into_string();

    assert!(html.contains("2.0k"));
    assert!(html.contains("cached"));
}

#[test]
fn test_token_badge_title_attribute() {
    let badge = token_badge(150, 75, None);
    let html = badge.into_string();

    assert!(html.contains("title=\"Prompt: 150, Completion: 75\""));
}

#[test]
fn test_token_badge_title_with_cache() {
    let badge = token_badge(150, 75, Some(50));
    let html = badge.into_string();

    assert!(html.contains("Cached: 50"));
}

#[test]
fn test_token_badge_styling() {
    let badge = token_badge(100, 50, None);
    let html = badge.into_string();

    assert!(html.contains("text-xs"));
    assert!(html.contains("text-muted-foreground"));
}

#[test]
fn test_token_badge_opacity_classes() {
    let badge = token_badge(100, 50, Some(25));
    let html = badge.into_string();

    assert!(html.contains("opacity-60"));
    assert!(html.contains("opacity-40"));
}

#[test]
fn test_token_badge_zero_tokens() {
    let badge = token_badge(0, 0, None);
    let html = badge.into_string();

    assert!(html.contains("0"));
}

#[test]
fn test_token_badge_large_values() {
    let badge = token_badge(50000, 25000, Some(10000));
    let html = badge.into_string();

    assert!(html.contains("50.0k"));
    assert!(html.contains("25.0k"));
    assert!(html.contains("10.0k"));
}
