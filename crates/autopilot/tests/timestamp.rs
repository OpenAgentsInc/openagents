//! Integration tests for timestamp utility functions

use autopilot::timestamp::{date_dir, filename, generate_slug};

// =========================================================================
// Slug generation tests
// =========================================================================

#[test]
fn test_generate_slug_basic() {
    assert_eq!(generate_slug("Say hello and list files"), "say-hello-and-list");
}

#[test]
fn test_generate_slug_with_punctuation() {
    assert_eq!(generate_slug("Fix the bug!"), "fix-the-bug");
}

#[test]
fn test_generate_slug_single_chars_filtered() {
    assert_eq!(generate_slug("a b c d e f"), "");
}

#[test]
fn test_generate_slug_with_dot() {
    assert_eq!(generate_slug("Read README.md"), "read-readme-md");
}

#[test]
fn test_generate_slug_empty_string() {
    assert_eq!(generate_slug(""), "");
}

#[test]
fn test_generate_slug_whitespace_only() {
    assert_eq!(generate_slug("   "), "");
}

#[test]
fn test_generate_slug_unicode() {
    // Unicode characters are alphanumeric, so they're kept
    assert_eq!(generate_slug("Hello 世界 test"), "hello-世界-test");
}

#[test]
fn test_generate_slug_special_characters() {
    assert_eq!(generate_slug("test@#$%^&*()file"), "test-file");
}

#[test]
fn test_generate_slug_multiple_spaces() {
    assert_eq!(generate_slug("test    multiple    spaces    here"), "test-multiple-spaces-here");
}

#[test]
fn test_generate_slug_consecutive_dashes() {
    assert_eq!(generate_slug("test---with---dashes"), "test-with-dashes");
}

#[test]
fn test_generate_slug_leading_trailing_special() {
    assert_eq!(generate_slug("!!!test with leading!!!"), "test-with-leading");
}

#[test]
fn test_generate_slug_numbers() {
    assert_eq!(generate_slug("test 123 file 456"), "test-123-file-456");
}

#[test]
fn test_generate_slug_mixed_case() {
    assert_eq!(generate_slug("MiXeD CaSe TeXt"), "mixed-case-text");
}

#[test]
fn test_generate_slug_only_special_chars() {
    assert_eq!(generate_slug("!@#$%^&*()"), "");
}

#[test]
fn test_generate_slug_long_text() {
    let long = "this is a very long prompt with many words but only first four should be used";
    assert_eq!(generate_slug(long), "this-is-very-long");
}

#[test]
fn test_generate_slug_newlines() {
    assert_eq!(generate_slug("test\nwith\nnewlines\nhere"), "test-with-newlines-here");
}

#[test]
fn test_generate_slug_tabs() {
    assert_eq!(generate_slug("test\twith\ttabs\there"), "test-with-tabs-here");
}

#[test]
fn test_generate_slug_path_like() {
    assert_eq!(generate_slug("path/to/some/file.txt"), "path-to-some-file-txt");
}

#[test]
fn test_generate_slug_url_like() {
    assert_eq!(generate_slug("https://example.com/test"), "https-example-com-test");
}

#[test]
fn test_generate_slug_quotes() {
    assert_eq!(generate_slug("\"quoted\" 'text' here"), "quoted-text-here");
}

#[test]
fn test_generate_slug_ampersand() {
    assert_eq!(generate_slug("save & continue test"), "save-continue-test");
}

#[test]
fn test_generate_slug_underscores() {
    assert_eq!(generate_slug("test_with_underscores"), "test-with-underscores");
}

#[test]
fn test_generate_slug_parentheses() {
    assert_eq!(generate_slug("test (with parentheses)"), "test-with-parentheses");
}

// =========================================================================
// Filename generation tests
// =========================================================================

#[test]
fn test_filename_format() {
    let f = filename("test-slug", "rlog");
    assert!(f.ends_with("-test-slug.rlog"));
}

#[test]
fn test_filename_length() {
    let f = filename("test-slug", "rlog");
    assert_eq!(f.len(), "HHMMSS-test-slug.rlog".len());
}

#[test]
fn test_filename_empty_slug() {
    let f = filename("", "txt");
    assert!(f.ends_with("-.txt"));
}

#[test]
fn test_filename_empty_extension() {
    let f = filename("test", "");
    assert!(f.ends_with("-test."));
}

#[test]
fn test_filename_special_chars_in_slug() {
    let f = filename("test@#$slug", "log");
    assert!(f.contains("test@#$slug"));
}

#[test]
fn test_filename_unicode_slug() {
    let f = filename("test-世界", "md");
    assert!(f.ends_with("-test-世界.md"));
}

#[test]
fn test_filename_long_slug() {
    let long_slug = "a".repeat(100);
    let f = filename(&long_slug, "txt");
    assert!(f.contains(&long_slug));
}

#[test]
fn test_filename_multiple_dots() {
    let f = filename("file.name", "tar.gz");
    assert!(f.ends_with("-file.name.tar.gz"));
}

// =========================================================================
// Date directory tests
// =========================================================================

#[test]
fn test_date_dir_format() {
    let d = date_dir();
    assert_eq!(d.len(), 8); // YYYYMMDD = 8 chars
}

#[test]
fn test_date_dir_numeric() {
    let d = date_dir();
    assert!(d.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn test_date_dir_year_prefix() {
    let d = date_dir();
    assert!(d.starts_with("20")); // Assuming we're in the 2000s
}

#[test]
fn test_date_dir_valid_month() {
    let d = date_dir();
    let month: u32 = d[4..6].parse().unwrap();
    assert!(month >= 1 && month <= 12);
}

#[test]
fn test_date_dir_valid_day() {
    let d = date_dir();
    let day: u32 = d[6..8].parse().unwrap();
    assert!(day >= 1 && day <= 31);
}

// =========================================================================
// Integration tests combining functions
// =========================================================================

#[test]
fn test_full_path_simulation() {
    let dir = date_dir();
    let slug = generate_slug("Test prompt for file");
    let file = filename(&slug, "rlog");

    let full_path = format!("{}/{}", dir, file);

    assert!(full_path.contains(&dir));
    assert!(full_path.contains(&slug));
    assert!(full_path.ends_with(".rlog"));
}

#[test]
fn test_slug_then_filename() {
    let slug = generate_slug("Create a test file");
    let file = filename(&slug, "txt");

    assert_eq!(slug, "create-test-file");
    assert!(file.ends_with("-create-test-file.txt"));
}

// =========================================================================
// Edge cases and boundary tests
// =========================================================================

#[test]
fn test_generate_slug_exactly_four_words() {
    assert_eq!(generate_slug("one two three four"), "one-two-three-four");
}

#[test]
fn test_generate_slug_more_than_four_words() {
    assert_eq!(generate_slug("one two three four five six"), "one-two-three-four");
}

#[test]
fn test_generate_slug_less_than_four_words() {
    assert_eq!(generate_slug("one two"), "one-two");
}

#[test]
fn test_generate_slug_one_word() {
    assert_eq!(generate_slug("hello"), "hello");
}

#[test]
fn test_generate_slug_emoji() {
    assert_eq!(generate_slug("test 🚀 emoji 🎉"), "test-emoji");
}

#[test]
fn test_generate_slug_numeric_only() {
    assert_eq!(generate_slug("123 456 789 101112"), "123-456-789-101112");
}

#[test]
fn test_generate_slug_alphanumeric_mix() {
    assert_eq!(generate_slug("test123 abc456 xyz789 def321"), "test123-abc456-xyz789-def321");
}

#[test]
fn test_filename_with_path_separator() {
    let f = filename("test/slug", "txt");
    assert!(f.contains("test/slug")); // Not sanitized in filename function
}

#[test]
fn test_generate_slug_trailing_dash_removed() {
    assert_eq!(generate_slug("test word three!"), "test-word-three");
}

#[test]
fn test_generate_slug_leading_dash_prevented() {
    assert_eq!(generate_slug("!!! test word"), "test-word");
}
