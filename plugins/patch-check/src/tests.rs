//! The checker as pure logic: no host, no WASM runtime — build inputs,
//! call `handle`, and assert on the placement report. These tests are the
//! product; the sandbox test in the CLI proves the same behavior through
//! the real boundary.

use super::*;

fn input(diff: &str, content: &str) -> Input {
    Input {
        diff: diff.to_string(),
        content: content.to_string(),
        fuzz: None,
        whitespace_lenient: false,
        include_preview: false,
        max_preview_chars: None,
    }
}

fn numbered(count: usize) -> String {
    (1..=count).map(|n| format!("line {n}\n")).collect()
}

#[test]
fn clean_apply_places_every_hunk_at_its_declared_line() {
    let content = "alpha\nbeta\ngamma\ndelta\nepsilon\n";
    let diff = "\
--- a/file.txt
+++ b/file.txt
@@ -2,3 +2,3 @@
 beta
-gamma
+GAMMA
 delta
";
    let out = handle(input(diff, content)).expect("a clean diff applies");
    assert!(out.applies);
    assert_eq!(out.applied_hunks, 1);
    assert_eq!(out.failed_hunks, 0);
    assert_eq!(out.hunks.len(), 1);
    let hunk = &out.hunks[0];
    assert_eq!(hunk.index, 0);
    assert!(hunk.applies);
    assert_eq!(hunk.at_line, Some(2));
    assert_eq!(hunk.drift_lines, None);
    assert_eq!(hunk.reason, None);
    assert_eq!(hunk.mismatch, None);
    assert_eq!(out.preview, None, "no preview unless asked");
}

#[test]
fn multi_hunk_apply_accounts_for_cumulative_line_shift() {
    // Hunk 1 grows the file by two lines; hunk 2's declared position is in
    // original coordinates, so exact placement only works if the checker
    // carries the shift into the working buffer.
    let content = numbered(40);
    let diff = "\
--- a/file.txt
+++ b/file.txt
@@ -2,2 +2,4 @@
 line 2
+inserted a
+inserted b
 line 3
@@ -30,3 +32,3 @@
 line 30
-line 31
+LINE 31
 line 32
";
    let mut request = input(&diff, &content);
    request.include_preview = true;
    // Fuzz zero: any drift would fail, so exact placement of hunk 2 proves
    // the shift bookkeeping rather than the search covering for it.
    request.fuzz = Some(0);
    let out = handle(request).expect("both hunks apply");
    assert!(out.applies);
    assert_eq!(out.applied_hunks, 2);
    assert_eq!(out.hunks[0].at_line, Some(2));
    assert_eq!(out.hunks[0].drift_lines, None);
    assert_eq!(out.hunks[1].at_line, Some(30), "original coordinates");
    assert_eq!(out.hunks[1].drift_lines, None, "no drift: the shift was accounted for");

    let preview = out.preview.expect("preview was requested and it applies");
    assert!(preview.contains("line 2\ninserted a\ninserted b\nline 3\n"));
    assert!(preview.contains("line 30\nLINE 31\nline 32\n"));
    assert_eq!(preview.lines().count(), 42);
}

#[test]
fn positive_drift_within_fuzz_is_found_and_reported() {
    // Five lines were prepended since the diff was cut: the hunk sits five
    // lines later than declared.
    let mut content = String::new();
    for n in 1..=5 {
        content.push_str(&format!("prepended {n}\n"));
    }
    content.push_str(&numbered(20));
    let diff = "\
@@ -9,3 +9,3 @@
 line 9
-line 10
+LINE 10
 line 11
";
    let out = handle(input(&diff, &content)).expect("drift within fuzz applies");
    assert!(out.applies);
    let hunk = &out.hunks[0];
    assert!(hunk.applies);
    assert_eq!(hunk.drift_lines, Some(5), "signed, positive: found later than declared");
    assert_eq!(hunk.at_line, Some(14), "declared 9 plus drift 5");
}

#[test]
fn negative_drift_within_fuzz_is_found_and_reported() {
    // Three lines were deleted above the hunk: it sits three lines earlier.
    let content: String = numbered(20)
        .lines()
        .filter(|line| !matches!(*line, "line 1" | "line 2" | "line 3"))
        .map(|line| format!("{line}\n"))
        .collect();
    let diff = "\
@@ -9,3 +9,3 @@
 line 9
-line 10
+LINE 10
 line 11
";
    let out = handle(input(&diff, &content)).expect("negative drift applies");
    let hunk = &out.hunks[0];
    assert!(hunk.applies);
    assert_eq!(hunk.drift_lines, Some(-3));
    assert_eq!(hunk.at_line, Some(6), "declared 9 minus drift 3");
}

#[test]
fn drift_beyond_fuzz_is_context_not_found() {
    let mut content = String::new();
    for n in 1..=10 {
        content.push_str(&format!("prepended {n}\n"));
    }
    content.push_str(&numbered(20));
    let diff = "\
@@ -9,3 +9,3 @@
 line 9
-line 10
+LINE 10
 line 11
";
    let mut request = input(&diff, &content);
    request.fuzz = Some(4); // real drift is 10
    let out = handle(request).expect("a placement failure is a report, not a refusal");
    assert!(!out.applies);
    assert_eq!(out.hunks[0].reason.as_deref(), Some("context_not_found"));
}

#[test]
fn context_not_found_quotes_the_first_mismatching_line() {
    let content = "alpha\nbeta\ngamma\n";
    let diff = "\
@@ -1,3 +1,3 @@
 alpha
-nowhere to be seen
+replacement
 gamma
";
    let out = handle(input(diff, content)).expect("reported, not refused");
    assert!(!out.applies);
    assert_eq!(out.applied_hunks, 0);
    assert_eq!(out.failed_hunks, 1);
    let hunk = &out.hunks[0];
    assert!(!hunk.applies);
    assert_eq!(hunk.reason.as_deref(), Some("context_not_found"));
    assert_eq!(
        hunk.mismatch.as_deref(),
        Some("nowhere to be seen"),
        "the first old-side line that no longer matches, quoted"
    );
    assert_eq!(hunk.at_line, None);
}

#[test]
fn mismatch_quote_is_bounded_to_200_chars() {
    let long = "x".repeat(500);
    let content = "alpha\nbeta\n";
    let diff = format!("@@ -1,2 +1,2 @@\n alpha\n-{long}\n+short\n");
    let out = handle(input(&diff, content)).expect("reported");
    let mismatch = out.hunks[0].mismatch.as_deref().expect("a quote");
    assert_eq!(mismatch.chars().count(), 200);
    assert!(mismatch.chars().all(|c| c == 'x'));
}

#[test]
fn ambiguous_context_fails_with_both_candidates_unchosen() {
    // The old side matches at two positions inside the fuzz window and not
    // at the declared position, so no unique placement exists.
    let block = "marker\nsame\nsame\n";
    let content = format!("{block}filler one\nfiller two\n{block}tail\n");
    let diff = "\
@@ -4,3 +4,3 @@
 marker
-same
+changed
 same
";
    let out = handle(input(&diff, &content)).expect("reported, not refused");
    assert!(!out.applies);
    let hunk = &out.hunks[0];
    assert_eq!(hunk.reason.as_deref(), Some("ambiguous"));
    assert!(hunk.mismatch.is_some(), "still says what mismatched at the declared spot");
    assert_eq!(hunk.at_line, None);
}

#[test]
fn exact_position_wins_even_when_the_context_also_matches_elsewhere() {
    // The same block appears twice, but one occurrence is exactly at the
    // declared position: that one wins and there is no ambiguity.
    let block = "marker\nsame\nsame\n";
    let content = format!("{block}filler\n{block}");
    let diff = "\
@@ -1,3 +1,3 @@
 marker
-same
+changed
 same
";
    let out = handle(input(&diff, &content)).expect("exact match wins");
    assert!(out.applies);
    assert_eq!(out.hunks[0].at_line, Some(1));
    assert_eq!(out.hunks[0].drift_lines, None);
}

#[test]
fn whitespace_lenient_forgives_trailing_whitespace_on_context_lines() {
    let content = "alpha   \nbeta\ngamma\t\n";
    let diff = "\
@@ -1,3 +1,3 @@
 alpha
-beta
+BETA
 gamma
";
    let strict = handle(input(diff, content)).expect("reported");
    assert!(!strict.applies, "strict mode: trailing whitespace breaks the context");
    assert_eq!(strict.hunks[0].reason.as_deref(), Some("context_not_found"));
    assert_eq!(strict.hunks[0].mismatch.as_deref(), Some("alpha"));

    let mut lenient = input(diff, content);
    lenient.whitespace_lenient = true;
    let out = handle(lenient).expect("lenient mode applies");
    assert!(out.applies);
    assert_eq!(out.hunks[0].at_line, Some(1));
}

#[test]
fn whitespace_lenient_does_not_forgive_removal_lines() {
    // Leniency is scoped to context lines; a removal line must match the
    // file exactly or the claim "this line will be removed" is wrong.
    let content = "alpha\nbeta   \ngamma\n";
    let diff = "\
@@ -1,3 +1,3 @@
 alpha
-beta
+BETA
 gamma
";
    let mut request = input(diff, content);
    request.whitespace_lenient = true;
    let out = handle(request).expect("reported");
    assert!(!out.applies);
    assert_eq!(out.hunks[0].mismatch.as_deref(), Some("beta"));
}

#[test]
fn missing_trailing_newline_markers_are_tolerated_and_tracked() {
    // The file ends without a newline; the diff rewrites its last line and
    // keeps it newline-less. Both sides carry the marker.
    let content = "alpha\nbeta";
    let diff = "\
--- a/file.txt
+++ b/file.txt
@@ -1,2 +1,2 @@
 alpha
-beta
\\ No newline at end of file
+BETA
\\ No newline at end of file
";
    let mut request = input(diff, content);
    request.include_preview = true;
    let out = handle(request).expect("applies");
    assert!(out.applies);
    assert_eq!(out.preview.as_deref(), Some("alpha\nBETA"), "still no trailing newline");
}

#[test]
fn a_patch_that_adds_the_trailing_newline_previews_with_one() {
    let content = "alpha\nbeta";
    let diff = "\
@@ -1,2 +1,2 @@
 alpha
-beta
\\ No newline at end of file
+beta
";
    let mut request = input(diff, content);
    request.include_preview = true;
    let out = handle(request).expect("applies");
    assert!(out.applies);
    assert_eq!(out.preview.as_deref(), Some("alpha\nbeta\n"));
}

#[test]
fn preview_is_elided_in_the_middle_past_the_ceiling() {
    let content = numbered(200);
    let diff = "\
@@ -1,2 +1,2 @@
-line 1
+FIRST
 line 2
";
    let mut request = input(&diff, &content);
    request.include_preview = true;
    request.max_preview_chars = Some(100);
    let out = handle(request).expect("applies");
    assert!(out.applies);
    assert_eq!(out.preview_truncated, Some(true));
    let preview = out.preview.expect("preview requested");
    assert!(preview.starts_with("FIRST\n"), "the head survives: {preview:?}");
    assert!(preview.ends_with("line 200\n"), "the tail survives: {preview:?}");
    let full_len = content.replace("line 1\n", "FIRST\n").chars().count();
    let marker = format!("[{} chars elided]", full_len - 100);
    assert!(preview.contains(&marker), "expected {marker:?} in {preview:?}");
}

#[test]
fn small_previews_are_returned_whole_without_the_truncated_flag() {
    let content = "alpha\nbeta\n";
    let diff = "\
@@ -1,2 +1,2 @@
-alpha
+ALPHA
 beta
";
    let mut request = input(diff, content);
    request.include_preview = true;
    let out = handle(request).expect("applies");
    assert_eq!(out.preview.as_deref(), Some("ALPHA\nbeta\n"));
    assert_eq!(out.preview_truncated, None);
}

#[test]
fn no_preview_when_a_hunk_fails_even_if_requested() {
    let content = "alpha\n";
    let diff = "\
@@ -1,1 +1,1 @@
-not here
+replacement
";
    let mut request = input(diff, content);
    request.include_preview = true;
    let out = handle(request).expect("reported");
    assert!(!out.applies);
    assert_eq!(out.preview, None);
}

#[test]
fn a_multi_file_diff_is_refused_with_one_file_per_call() {
    let diff = "\
--- a/one.txt
+++ b/one.txt
@@ -1,1 +1,1 @@
-a
+A
--- a/two.txt
+++ b/two.txt
@@ -1,1 +1,1 @@
-b
+B
";
    let refusal = handle(input(diff, "a\n")).expect_err("two files refuse");
    assert!(
        refusal.reason.contains("one file per call"),
        "names the rule: {}",
        refusal.reason
    );
    assert!(refusal.reason.contains("2 files"), "counts them: {}", refusal.reason);
}

#[test]
fn a_git_style_multi_file_diff_is_also_refused() {
    let diff = "\
diff --git a/one.txt b/one.txt
--- a/one.txt
+++ b/one.txt
@@ -1,1 +1,1 @@
-a
+A
diff --git a/two.txt b/two.txt
--- a/two.txt
+++ b/two.txt
@@ -1,1 +1,1 @@
-b
+B
";
    let refusal = handle(input(diff, "a\n")).expect_err("two files refuse");
    assert!(refusal.reason.contains("one file per call"));
}

#[test]
fn a_malformed_diff_with_no_hunks_is_refused_naming_the_shape() {
    let refusal =
        handle(input("this is prose, not a patch", "alpha\n")).expect_err("no hunks refuse");
    assert!(
        refusal.reason.contains("@@ -start,count +start,count @@"),
        "describes what a unified diff looks like: {}",
        refusal.reason
    );
}

#[test]
fn a_truncated_hunk_body_is_refused() {
    let diff = "\
@@ -1,3 +1,3 @@
 alpha
-beta
";
    let refusal = handle(input(diff, "alpha\nbeta\ngamma\n")).expect_err("truncated refuses");
    assert!(refusal.reason.contains("truncated"), "says so: {}", refusal.reason);
}

#[test]
fn count_free_hunk_headers_default_to_one_line() {
    // `@@ -2 +2 @@` means one old line and one new line.
    let content = "alpha\nbeta\ngamma\n";
    let diff = "\
@@ -2 +2 @@
-beta
+BETA
";
    let out = handle(input(diff, content)).expect("applies");
    assert!(out.applies);
    assert_eq!(out.hunks[0].at_line, Some(2));
}

#[test]
fn fuzz_is_capped_at_1000() {
    // Drift of 1500 with a requested fuzz of 5000: the cap keeps the
    // search window at 1000, so the hunk is not found.
    let mut content = String::new();
    for n in 1..=1500 {
        content.push_str(&format!("pad {n}\n"));
    }
    content.push_str("needle one\nneedle two\n");
    let diff = "\
@@ -1,2 +1,2 @@
 needle one
-needle two
+NEEDLE TWO
";
    let mut request = input(&diff, &content);
    request.fuzz = Some(5_000);
    let out = handle(request).expect("reported");
    assert!(!out.applies);
    assert_eq!(out.hunks[0].reason.as_deref(), Some("context_not_found"));
}

#[test]
fn later_hunks_still_report_after_an_earlier_failure() {
    let content = numbered(20);
    let diff = "\
@@ -2,3 +2,3 @@
 gone context
-line 3
+LINE 3
 line 4
@@ -10,3 +10,3 @@
 line 10
-line 11
+LINE 11
 line 12
";
    let out = handle(input(&diff, &content)).expect("reported");
    assert!(!out.applies);
    assert_eq!(out.applied_hunks, 1);
    assert_eq!(out.failed_hunks, 1);
    assert!(!out.hunks[0].applies);
    assert!(out.hunks[1].applies);
    assert_eq!(out.hunks[1].at_line, Some(10));
}
