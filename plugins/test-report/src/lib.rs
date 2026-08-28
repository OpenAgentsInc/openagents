//! Parse captured test-runner output into typed failures.
//!
//! Pure computation: the shell already ran the suite; this plugin turns
//! that dump into file, test name, assertion, and bounded traceback so
//! the next round does not re-read three hundred lines. No mounts, no
//! network, no writes.

use openagents_pdk::Refusal;
use serde::{Deserialize, Serialize};

const DEFAULT_MAX_FAILURES: usize = 50;
const MAX_FAILURES_CAP: usize = 200;
const DEFAULT_MAX_TRACEBACK_LINES: usize = 12;
const MAX_TRACEBACK_LINES_CAP: usize = 40;
const DEFAULT_MAX_TEXT_CHARS: usize = 200_000;
const MAX_TEXT_CHARS_CAP: usize = 1_000_000;
const ASSERTION_CAP: usize = 400;

#[derive(Deserialize)]
pub struct Input {
    /// Captured stdout/stderr from a test run.
    pub text: String,
    /// `auto` (default) or one of `pytest`, `cargo`, `jest`, `vitest`, `go`.
    #[serde(default)]
    pub runner: Option<String>,
    #[serde(default)]
    pub max_failures: Option<usize>,
    #[serde(default)]
    pub max_traceback_lines: Option<usize>,
    #[serde(default)]
    pub max_text_chars: Option<usize>,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct Failure {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assertion: Option<String>,
    pub traceback_lines: Vec<String>,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct Output {
    pub runner: String,
    pub detected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignored: Option<u32>,
    pub failures: Vec<Failure>,
    pub truncated: bool,
    pub notes: Vec<String>,
}

pub(crate) fn handle(input: Input) -> Result<Output, Refusal> {
    let max_text = input
        .max_text_chars
        .unwrap_or(DEFAULT_MAX_TEXT_CHARS)
        .clamp(1, MAX_TEXT_CHARS_CAP);
    let max_failures = input
        .max_failures
        .unwrap_or(DEFAULT_MAX_FAILURES)
        .clamp(1, MAX_FAILURES_CAP);
    let max_tb = input
        .max_traceback_lines
        .unwrap_or(DEFAULT_MAX_TRACEBACK_LINES)
        .clamp(1, MAX_TRACEBACK_LINES_CAP);

    let mut truncated = false;
    let text = if input.text.len() > max_text {
        truncated = true;
        &input.text[..max_text]
    } else {
        input.text.as_str()
    };

    let requested = input
        .runner
        .as_deref()
        .unwrap_or("auto")
        .trim()
        .to_ascii_lowercase();
    let runner = match requested.as_str() {
        "" | "auto" => detect_runner(text),
        "pytest" | "cargo" | "jest" | "vitest" | "go" => requested,
        other => {
            return Err(Refusal::unsupported(format!(
                "runner `{other}` is not one of auto, pytest, cargo, jest, vitest, go"
            )));
        }
    };

    if runner == "unknown" {
        return Err(Refusal::unsupported(
            "could not detect a pytest, cargo, jest/vitest, or go test report in the text"
                .to_string(),
        ));
    }

    let mut notes = Vec::new();
    if truncated {
        notes.push(format!(
            "input truncated to {max_text} bytes before parsing"
        ));
    }

    let mut parsed = match runner.as_str() {
        "pytest" => parse_pytest(text, max_failures, max_tb),
        "cargo" => parse_cargo(text, max_failures, max_tb),
        "jest" | "vitest" => parse_jest(text, max_failures, max_tb),
        "go" => parse_go(text, max_failures, max_tb),
        _ => unreachable!(),
    };
    parsed.runner = runner;
    parsed.detected = true;
    parsed.truncated = truncated || parsed.truncated;
    parsed.notes.append(&mut notes);
    if parsed.failures.len() == max_failures {
        parsed.truncated = true;
        parsed
            .notes
            .push(format!("failure list capped at {max_failures}"));
    }
    Ok(parsed)
}

fn detect_runner(text: &str) -> String {
    if text.contains("=== FAILURES ===")
        || text.contains("short test summary info")
        || (text.contains("FAILED ") && text.contains(".py"))
        || text.contains("pytest")
    {
        return "pytest".to_string();
    }
    if text.contains("test result:") || text.contains(" ... FAILED") {
        return "cargo".to_string();
    }
    if text.contains("FAIL src/")
        || text.contains("● ")
        || (text.contains("Tests:") && (text.contains("failed") || text.contains("passed")))
    {
        return "jest".to_string();
    }
    if text.contains("--- FAIL:") || text.contains("FAIL\t") {
        return "go".to_string();
    }
    "unknown".to_string()
}

fn parse_pytest(text: &str, max_failures: usize, max_tb: usize) -> Output {
    let mut failures = Vec::new();
    let mut passed = None;
    let mut failed = None;
    let mut errors = None;
    let mut ignored = None;
    let mut truncated = false;

    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("FAILED ") {
            if failures.len() >= max_failures {
                truncated = true;
                break;
            }
            let (node, assertion) = split_once_dash(rest);
            let (file, test) = split_pytest_node(node);
            failures.push(Failure {
                file,
                test,
                assertion: assertion.map(bound_assertion),
                traceback_lines: Vec::new(),
            });
        }
        if line.starts_with('=') && (line.contains("failed") || line.contains("passed")) {
            take_pytest_counts(line, &mut passed, &mut failed, &mut errors, &mut ignored);
        }
    }

    attach_pytest_tracebacks(text, &mut failures, max_tb);

    if failed.is_none() {
        failed = Some(failures.len() as u32);
    }
    Output {
        runner: "pytest".to_string(),
        detected: true,
        passed,
        failed,
        errors,
        ignored,
        failures,
        truncated,
        notes: Vec::new(),
    }
}

fn split_once_dash(rest: &str) -> (&str, Option<&str>) {
    match rest.split_once(" - ") {
        Some((node, assertion)) => (node.trim(), Some(assertion.trim())),
        None => (rest.trim(), None),
    }
}

fn split_pytest_node(node: &str) -> (Option<String>, Option<String>) {
    match node.split_once("::") {
        Some((file, test)) => (nonempty(file), nonempty(test)),
        None if node.contains(".py") => (nonempty(node), None),
        None => (None, nonempty(node)),
    }
}

fn take_pytest_counts(
    line: &str,
    passed: &mut Option<u32>,
    failed: &mut Option<u32>,
    errors: &mut Option<u32>,
    ignored: &mut Option<u32>,
) {
    for part in line.split(',') {
        let part = part.trim().trim_matches('=');
        let mut bits = part.split_whitespace();
        let Some(n) = bits.next().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let Some(kind) = bits.next() else { continue };
        match kind {
            "passed" => *passed = Some(n),
            "failed" => *failed = Some(n),
            "error" | "errors" => *errors = Some(n),
            "skipped" => *ignored = Some(n),
            _ => {}
        }
    }
}

fn attach_pytest_tracebacks(text: &str, failures: &mut [Failure], max_tb: usize) {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let heading = lines[i].trim().trim_matches('_').trim();
        if heading.is_empty() || !lines[i].contains("____") {
            i += 1;
            continue;
        }
        let Some(failure) = failures.iter_mut().find(|f| {
            f.test
                .as_deref()
                .is_some_and(|t| t == heading || t.ends_with(heading))
        }) else {
            i += 1;
            continue;
        };
        let mut tb = Vec::new();
        i += 1;
        while i < lines.len() {
            let line = lines[i];
            if line.contains("____") || line.contains("====") {
                break;
            }
            let trimmed = line.trim();
            if failure.assertion.is_none() && (trimmed.starts_with('E') || trimmed.starts_with('>'))
            {
                failure.assertion = Some(bound_assertion(
                    trimmed
                        .trim_start_matches('E')
                        .trim_start_matches('>')
                        .trim(),
                ));
            }
            if tb.len() < max_tb {
                tb.push(line.to_string());
            }
            i += 1;
        }
        failure.traceback_lines = tb;
    }
}

fn parse_cargo(text: &str, max_failures: usize, max_tb: usize) -> Output {
    let mut failures = Vec::new();
    let mut passed = None;
    let mut failed = None;
    let mut ignored = None;
    let mut truncated = false;

    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("test ") {
            if rest.contains(" ... FAILED") {
                if failures.len() >= max_failures {
                    truncated = true;
                    continue;
                }
                let name = rest
                    .split(" ... FAILED")
                    .next()
                    .unwrap_or(rest)
                    .trim()
                    .to_string();
                failures.push(Failure {
                    file: None,
                    test: nonempty(&name),
                    assertion: None,
                    traceback_lines: Vec::new(),
                });
            }
        }
        if let Some(rest) = line.strip_prefix("test result:") {
            for part in rest.split([';', '.']) {
                let part = part.trim();
                let mut bits = part.split_whitespace();
                let Some(n) = bits.next().and_then(|s| s.parse::<u32>().ok()) else {
                    continue;
                };
                let Some(kind) = bits.next() else { continue };
                match kind {
                    "passed" => passed = Some(n),
                    "failed" => failed = Some(n),
                    "ignored" => ignored = Some(n),
                    _ => {}
                }
            }
        }
    }

    attach_cargo_tracebacks(text, &mut failures, max_tb);

    if failed.is_none() {
        failed = Some(failures.len() as u32);
    }
    Output {
        runner: "cargo".to_string(),
        detected: true,
        passed,
        failed,
        errors: None,
        ignored,
        failures,
        truncated,
        notes: Vec::new(),
    }
}

fn attach_cargo_tracebacks(text: &str, failures: &mut [Failure], max_tb: usize) {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        if let Some(rest) = line.strip_prefix("---- ") {
            if let Some(name) = rest.strip_suffix(" stdout ----") {
                if let Some(failure) = failures
                    .iter_mut()
                    .find(|f| f.test.as_deref() == Some(name.trim()))
                {
                    let mut tb = Vec::new();
                    i += 1;
                    while i < lines.len() {
                        let next = lines[i];
                        if next.trim().starts_with("---- ") || next.trim().starts_with("failures:")
                        {
                            break;
                        }
                        if next.contains("panicked at") && failure.assertion.is_none() {
                            failure.assertion = Some(bound_assertion(next.trim()));
                        }
                        if tb.len() < max_tb {
                            tb.push(next.to_string());
                        }
                        i += 1;
                    }
                    failure.traceback_lines = tb;
                    continue;
                }
            }
        }
        i += 1;
    }
}

fn parse_jest(text: &str, max_failures: usize, max_tb: usize) -> Output {
    let mut failures = Vec::new();
    let mut passed = None;
    let mut failed = None;
    let mut truncated = false;
    let mut current_file: Option<String> = None;
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("FAIL ") {
            current_file = nonempty(rest.split_whitespace().next().unwrap_or(rest));
        }
        if let Some(rest) = trimmed.strip_prefix('●') {
            if failures.len() >= max_failures {
                truncated = true;
                i += 1;
                continue;
            }
            let test = rest.trim().to_string();
            let mut tb = Vec::new();
            let mut assertion = None;
            i += 1;
            while i < lines.len() {
                let next = lines[i];
                let next_trim = next.trim();
                if next_trim.starts_with('●') || next_trim.starts_with("FAIL ") {
                    i -= 1;
                    break;
                }
                if next_trim.starts_with("Tests:") {
                    take_jest_counts(next_trim, &mut passed, &mut failed);
                    break;
                }
                if assertion.is_none()
                    && (next_trim.starts_with("expect(")
                        || next_trim.starts_with("AssertionError")
                        || next_trim.contains("Received:")
                        || next_trim.contains("Expected:"))
                {
                    assertion = Some(bound_assertion(next_trim));
                }
                if tb.len() < max_tb && !next_trim.is_empty() {
                    tb.push(next.to_string());
                }
                i += 1;
            }
            failures.push(Failure {
                file: current_file.clone(),
                test: nonempty(&test),
                assertion,
                traceback_lines: tb,
            });
        }
        take_jest_counts(trimmed, &mut passed, &mut failed);
        i += 1;
    }
    if failed.is_none() {
        failed = Some(failures.len() as u32);
    }
    Output {
        runner: "jest".to_string(),
        detected: true,
        passed,
        failed,
        errors: None,
        ignored: None,
        failures,
        truncated,
        notes: Vec::new(),
    }
}

fn take_jest_counts(line: &str, passed: &mut Option<u32>, failed: &mut Option<u32>) {
    let Some(rest) = line.trim().strip_prefix("Tests:") else {
        return;
    };
    for part in rest.split(',') {
        let part = part.trim();
        let mut bits = part.split_whitespace();
        let Some(n) = bits.next().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let Some(kind) = bits.next() else { continue };
        match kind {
            "failed" => *failed = Some(n),
            "passed" => *passed = Some(n),
            _ => {}
        }
    }
}

fn parse_go(text: &str, max_failures: usize, max_tb: usize) -> Output {
    let mut failures = Vec::new();
    let passed = None;
    let mut failed = None;
    let mut truncated = false;
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if let Some(rest) = trimmed.strip_prefix("--- FAIL:") {
            if failures.len() >= max_failures {
                truncated = true;
                i += 1;
                continue;
            }
            let name = rest.split_whitespace().next().unwrap_or("").to_string();
            let mut tb = Vec::new();
            let mut assertion = None;
            let mut file = None;
            i += 1;
            while i < lines.len() {
                let next = lines[i];
                let next_trim = next.trim();
                if next_trim.starts_with("--- FAIL:")
                    || next_trim.starts_with("--- PASS:")
                    || next_trim == "FAIL"
                    || next_trim.starts_with("FAIL\t")
                {
                    i -= 1;
                    break;
                }
                if file.is_none() {
                    if let Some((path, _)) = next_trim.split_once(':') {
                        if path.ends_with("_test.go") || path.ends_with(".go") {
                            file = nonempty(path);
                        }
                    }
                }
                if assertion.is_none() && next_trim.contains(':') {
                    assertion = Some(bound_assertion(next_trim));
                }
                if tb.len() < max_tb {
                    tb.push(next.to_string());
                }
                i += 1;
            }
            failures.push(Failure {
                file,
                test: nonempty(&name),
                assertion,
                traceback_lines: tb,
            });
        }
        if trimmed.starts_with("FAIL\t") {
            failed = Some(failed.unwrap_or(0).max(failures.len() as u32));
        }
        i += 1;
    }
    if failed.is_none() {
        failed = Some(failures.len() as u32);
    }
    Output {
        runner: "go".to_string(),
        detected: true,
        passed,
        failed,
        errors: None,
        ignored: None,
        failures,
        truncated,
        notes: Vec::new(),
    }
}

fn nonempty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn bound_assertion(value: &str) -> String {
    let mut out = value.trim().to_string();
    if out.chars().count() > ASSERTION_CAP {
        out = out.chars().take(ASSERTION_CAP).collect();
        out.push('…');
    }
    out
}

openagents_pdk::plugin_entry!(handle);

#[cfg(test)]
mod tests;
