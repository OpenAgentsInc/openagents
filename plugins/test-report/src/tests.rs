//! Pure parser tests: no host, no WASM runtime.

use super::*;

fn run(text: &str) -> Output {
    handle(Input {
        text: text.to_string(),
        runner: None,
        max_failures: None,
        max_traceback_lines: None,
        max_text_chars: None,
    })
    .expect("fixture should parse")
}

#[test]
fn pytest_failed_line_and_summary() {
    let text = "\
FAILED tests/test_auth.py::test_login - assert 1 == 2
FAILED tests/test_auth.py::test_logout - assert None
=========================== 2 failed, 3 passed in 0.12s ===========================
";
    let out = run(text);
    assert_eq!(out.runner, "pytest");
    assert_eq!(out.failed, Some(2));
    assert_eq!(out.passed, Some(3));
    assert_eq!(out.failures.len(), 2);
    assert_eq!(out.failures[0].file.as_deref(), Some("tests/test_auth.py"));
    assert_eq!(out.failures[0].test.as_deref(), Some("test_login"));
    assert_eq!(out.failures[0].assertion.as_deref(), Some("assert 1 == 2"));
}

#[test]
fn pytest_failure_section_attaches_traceback() {
    let text = "\
FAILED tests/test_math.py::test_add
=================================== FAILURES ===================================
_________________________________ test_add _________________________________
    def test_add():
>       assert 1 + 1 == 3
E       assert 2 == 3

tests/test_math.py:4: AssertionError
=========================== 1 failed, 0 passed in 0.01s ===========================
";
    let out = run(text);
    assert_eq!(out.failures.len(), 1);
    assert!(
        out.failures[0]
            .traceback_lines
            .iter()
            .any(|line| line.contains("assert 2 == 3")),
        "{:?}",
        out.failures[0].traceback_lines
    );
}

#[test]
fn cargo_failed_tests_and_result_line() {
    let text = "\
running 3 tests
test parse::clean ... ok
test parse::drift ... FAILED
test parse::empty ... ok

failures:

---- parse::drift stdout ----
thread 'parse::drift' panicked at src/lib.rs:40:5:
assertion `left == right` failed

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
";
    let out = run(text);
    assert_eq!(out.runner, "cargo");
    assert_eq!(out.passed, Some(2));
    assert_eq!(out.failed, Some(1));
    assert_eq!(out.failures.len(), 1);
    assert_eq!(out.failures[0].test.as_deref(), Some("parse::drift"));
    assert!(
        out.failures[0]
            .assertion
            .as_deref()
            .unwrap_or("")
            .contains("panicked at"),
        "{:?}",
        out.failures[0].assertion
    );
}

#[test]
fn jest_bullet_failures() {
    let text = "\
FAIL src/auth.test.ts
  ● login › rejects a bad password

    expect(received).toBe(expected)
      Expected: true
      Received: false

Tests:       1 failed, 4 passed, 5 total
";
    let out = run(text);
    assert_eq!(out.runner, "jest");
    assert_eq!(out.failed, Some(1));
    assert_eq!(out.passed, Some(4));
    assert_eq!(out.failures.len(), 1);
    assert_eq!(out.failures[0].file.as_deref(), Some("src/auth.test.ts"));
    assert_eq!(
        out.failures[0].test.as_deref(),
        Some("login › rejects a bad password")
    );
}

#[test]
fn go_fail_blocks() {
    let text = "\
--- FAIL: TestParse (0.00s)
    parse_test.go:12: got 1 want 2
FAIL
FAIL	example.com/parse	0.012s
";
    let out = run(text);
    assert_eq!(out.runner, "go");
    assert_eq!(out.failures.len(), 1);
    assert_eq!(out.failures[0].test.as_deref(), Some("TestParse"));
    assert_eq!(out.failures[0].file.as_deref(), Some("parse_test.go"));
}

#[test]
fn unknown_text_is_unsupported() {
    let err = handle(Input {
        text: "hello world, nothing like a test report".to_string(),
        runner: None,
        max_failures: None,
        max_traceback_lines: None,
        max_text_chars: None,
    })
    .expect_err("not a report");
    assert_eq!(err.code, openagents_pdk::RefusalCode::Unsupported);
}

#[test]
fn failure_cap_sets_truncated() {
    let mut text = String::new();
    for i in 0..8 {
        text.push_str(&format!("FAILED tests/t.py::test_{i} - boom\n"));
    }
    text.push_str(
        "=========================== 8 failed, 0 passed in 0.01s ===========================\n",
    );
    let out = handle(Input {
        text,
        runner: Some("pytest".to_string()),
        max_failures: Some(3),
        max_traceback_lines: None,
        max_text_chars: None,
    })
    .expect("capped");
    assert_eq!(out.failures.len(), 3);
    assert!(out.truncated);
}
