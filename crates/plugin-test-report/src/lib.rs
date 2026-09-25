//! Test-report guest.
//!
//! The `parse` operation reads test output files from a granted snapshot
//! and returns each failing test with its location: JUnit XML, `cargo
//! test` output, and pytest output. The format comes from the content, not
//! the file name. A file whose content is none of the three is named under
//! `unrecognized` and not parsed.
//!
//! Without `paths` in the input, the guest reads the files whose names
//! look like test output: XML files, and text, log, and output files whose
//! names mention tests, results, reports, JUnit, pytest, or cargo. With
//! `paths`, it reads those files only.
//!
//! The parser is line based and deliberately small. It extracts what the
//! runners print in their default formats; a runner configured to print
//! something else is `unrecognized` or yields fewer failures, never a
//! guessed one. `truncated` is true when a file was cut, or when the
//! failure bound left a failure out.
//!
//! This reimplements the pre-reset `test-report` plugin's purpose on the
//! `openagents.plugin-packet.v1` ABI, reading report files from a snapshot
//! rather than taking captured text as input, and adds JUnit XML.

use plugin_pdk::Request;
use plugin_pdk::guest::{self, Host, Refusal};
use serde_json::{Map, Value, json};

plugin_pdk::export_guest!(handle);

const DEFAULT_MAX_FAILURES: usize = 50;
const MAX_FAILURES_CAP: usize = 400;
const DEFAULT_FILE_BYTES: usize = 1024 * 1024;
const FILE_BYTES_CAP: usize = 4 * 1024 * 1024;
const MAX_REPORTS: usize = 20;
const MAX_LISTED: usize = 20_000;
/// The most characters of a failure message the result keeps.
const MESSAGE_CHARS: usize = 300;

fn handle(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    match request.operation.as_str() {
        "parse" => parse(request, host),
        _ => Err(Refusal::unsupported("operation")),
    }
}

/// One failing test.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Failure {
    test: String,
    file: Option<String>,
    line: Option<u64>,
    message: Option<String>,
    /// `failure` or `error`.
    kind: &'static str,
}

impl Failure {
    fn json(&self) -> Value {
        let mut object = Map::new();
        object.insert("test".into(), json!(self.test));
        object.insert("kind".into(), json!(self.kind));
        if let Some(file) = &self.file {
            object.insert("file".into(), json!(file));
        }
        if let Some(line) = self.line {
            object.insert("line".into(), json!(line));
        }
        if let Some(message) = &self.message {
            object.insert("message".into(), json!(message));
        }
        Value::Object(object)
    }
}

/// What one report file said.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Report {
    format: &'static str,
    counts: Vec<(&'static str, u64)>,
    failures: Vec<Failure>,
}

fn parse(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    let input = &request.input;
    let max_failures = bounded(
        input,
        "max_failures",
        DEFAULT_MAX_FAILURES,
        MAX_FAILURES_CAP,
    );
    let file_bytes = bounded(input, "max_file_bytes", DEFAULT_FILE_BYTES, FILE_BYTES_CAP);
    let wanted: Option<Vec<String>> = match &input["paths"] {
        Value::Null => None,
        Value::Array(items) => Some(
            items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| Refusal::unsupported("paths is a list of strings"))
                })
                .collect::<Result<_, _>>()?,
        ),
        _ => return Err(Refusal::unsupported("paths is a list of strings")),
    };
    let root = guest::root(request).ok_or_else(|| Refusal::refused("no granted snapshot"))?;
    let listing = guest::list(host, &root, MAX_LISTED)
        .map_err(|code| Refusal::refused(format!("list {code}")))?;
    let candidates: Vec<&guest::Listed> = listing
        .entries
        .iter()
        .filter(|entry| entry.kind == "file")
        .filter(|entry| {
            let path = guest::relative(&entry.name);
            match &wanted {
                Some(paths) => paths.iter().any(|wanted| wanted == path),
                None => looks_like_report(path),
            }
        })
        .collect();
    let mut truncated = candidates.len() > MAX_REPORTS;
    let mut reports = Vec::new();
    let mut unrecognized = Vec::new();
    let mut unread = Vec::new();
    let mut left = max_failures;
    let mut failures_total = 0_usize;
    for entry in candidates.iter().take(MAX_REPORTS) {
        let path = guest::relative(&entry.name);
        let Ok(read) = guest::read(host, &entry.handle, file_bytes) else {
            unread.push(path.to_string());
            truncated = true;
            continue;
        };
        if !read.complete {
            truncated = true;
        }
        let text = String::from_utf8_lossy(&read.bytes);
        let Some(report) = report(&text) else {
            unrecognized.push(path.to_string());
            continue;
        };
        failures_total += report.failures.len();
        if report.failures.len() > left {
            truncated = true;
        }
        let kept: Vec<Value> = report
            .failures
            .iter()
            .take(left)
            .map(Failure::json)
            .collect();
        left -= kept.len();
        let mut object = Map::new();
        object.insert("path".into(), json!(path));
        object.insert("format".into(), json!(report.format));
        for (name, count) in &report.counts {
            object.insert((*name).into(), json!(count));
        }
        object.insert("failures".into(), Value::Array(kept));
        reports.push(Value::Object(object));
    }
    Ok(json!({
        "kind": "test-report",
        "files_considered": candidates.len(),
        "reports": reports,
        "unrecognized": unrecognized,
        "unread": unread,
        "failures_total": failures_total,
        "truncated": truncated,
    }))
}

fn bounded(input: &Value, key: &str, default: usize, cap: usize) -> usize {
    input[key]
        .as_u64()
        .map_or(default, |n| usize::try_from(n).unwrap_or(cap))
        .clamp(1, cap)
}

/// Whether a path's name looks like a test report.
fn looks_like_report(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    let mentions = ["test", "result", "report", "junit", "pytest", "cargo"]
        .iter()
        .any(|word| stem.contains(word));
    match extension {
        "xml" => mentions || stem.starts_with("test"),
        "txt" | "log" | "out" => mentions,
        _ => false,
    }
}

/// Parse `text` in the format its content shows, or `None` when it shows
/// none of the three.
fn report(text: &str) -> Option<Report> {
    if text.contains("<testsuite") || text.contains("<testcase") {
        return Some(junit(text));
    }
    if text.contains("test result:") || (text.contains("---- ") && text.contains(" stdout ----")) {
        return Some(cargo(text));
    }
    if text.contains("short test summary info")
        || text.contains("= FAILURES =")
        || text.contains("test session starts")
    {
        return Some(pytest(text));
    }
    None
}

fn clip(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= MESSAGE_CHARS {
        return text.to_string();
    }
    let mut clipped: String = text.chars().take(MESSAGE_CHARS).collect();
    clipped.push('…');
    clipped
}

// JUnit XML.

fn junit(text: &str) -> Report {
    let mut report = Report {
        format: "junit",
        ..Report::default()
    };
    let (mut tests, mut failed, mut errors, mut skipped) = (0, 0, 0, 0);
    let mut rest = text;
    while let Some(start) = rest.find("<testcase") {
        let after = &rest[start..];
        let Some(tag_end) = after.find('>') else {
            break;
        };
        let tag = &after[..tag_end];
        tests += 1;
        let (body, next) = if tag.ends_with('/') {
            ("", &after[tag_end + 1..])
        } else {
            match after.find("</testcase>") {
                Some(close) => (&after[tag_end + 1..close], &after[close..]),
                None => (&after[tag_end + 1..], ""),
            }
        };
        rest = next;
        let kind = if body.contains("<failure") {
            "failure"
        } else if body.contains("<error") {
            "error"
        } else {
            if body.contains("<skipped") {
                skipped += 1;
            }
            continue;
        };
        if kind == "failure" {
            failed += 1;
        } else {
            errors += 1;
        }
        let name = attribute(tag, "name").unwrap_or_default();
        let test = match attribute(tag, "classname") {
            Some(class) if !class.is_empty() => format!("{class}::{name}"),
            _ => name,
        };
        let element = &body[body.find(&format!("<{kind}")).unwrap_or(0)..];
        let element_tag = &element[..element.find('>').unwrap_or(element.len())];
        let message = attribute(element_tag, "message")
            .filter(|message| !message.is_empty())
            .or_else(|| {
                let content = element.get(element_tag.len() + 1..).unwrap_or_default();
                let content = content.split("</").next().unwrap_or_default();
                let content = unescape(content.trim().trim_start_matches("<![CDATA["));
                content
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .map(str::to_string)
            })
            .map(|message| clip(&message));
        let (mut file, mut line) = (
            attribute(tag, "file"),
            attribute(tag, "line").and_then(|line| line.parse().ok()),
        );
        if file.is_none()
            && let Some((found_file, found_line)) = location(body)
        {
            file = Some(found_file);
            line = line.or(Some(found_line));
        }
        report.failures.push(Failure {
            test,
            file,
            line,
            message,
            kind,
        });
    }
    report.counts = vec![
        ("tests", tests),
        ("failed", failed),
        ("errors", errors),
        ("skipped", skipped),
    ];
    report
}

/// The value of `name="…"` in a tag.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let mut from = 0;
    while let Some(at) = tag[from..].find(name).map(|at| at + from) {
        let before = tag[..at].chars().last();
        let rest = &tag[at + name.len()..];
        if before.is_some_and(char::is_whitespace)
            && let Some(rest) = rest.trim_start().strip_prefix('=')
        {
            let rest = rest.trim_start();
            let quote = rest.chars().next()?;
            if quote == '"' || quote == '\'' {
                let value = &rest[1..];
                let end = value.find(quote)?;
                return Some(unescape(&value[..end]));
            }
        }
        from = at + name.len();
    }
    None
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#10;", "\n")
        .replace("&amp;", "&")
}

/// The first `path:line` in `text` whose path has a source extension.
fn location(text: &str) -> Option<(String, u64)> {
    for line in text.lines() {
        for word in line.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == ',') {
            let word = word.trim_matches(|c: char| c == '"' || c == '\'');
            let mut parts = word.split(':');
            let (Some(path), Some(number)) = (parts.next(), parts.next()) else {
                continue;
            };
            let source = [
                ".py", ".rs", ".go", ".js", ".ts", ".java", ".rb", ".kt", ".c", ".cpp",
            ]
            .iter()
            .any(|extension| path.ends_with(extension));
            if source && let Ok(number) = number.parse() {
                return Some((path.to_string(), number));
            }
        }
    }
    None
}

// cargo test.

fn cargo(text: &str) -> Report {
    let mut report = Report {
        format: "cargo",
        ..Report::default()
    };
    let (mut passed, mut failed, mut ignored) = (0, 0, 0);
    let mut names: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(summary) = line.strip_prefix("test result:") {
            for part in summary.split([';', '.']) {
                let mut words = part.split_whitespace();
                if let (Some(count), Some(word)) = (words.next(), words.next())
                    && let Ok(count) = count.parse::<u64>()
                {
                    match word {
                        "passed" => passed += count,
                        "failed" => failed += count,
                        "ignored" => ignored += count,
                        _ => {}
                    }
                }
            }
        } else if let Some(rest) = line.strip_prefix("test ")
            && let Some(name) = rest.strip_suffix(" ... FAILED")
            && !names.iter().any(|seen| seen == name)
        {
            names.push(name.to_string());
        }
    }
    let blocks = cargo_blocks(text);
    for (name, _) in &blocks {
        if !names.iter().any(|seen| seen == name) {
            names.push(name.clone());
        }
    }
    for name in names {
        let block = blocks
            .iter()
            .find(|(block_name, _)| *block_name == name)
            .map(|(_, block)| block.as_str())
            .unwrap_or_default();
        let (file, line, message) = panic_site(block);
        report.failures.push(Failure {
            test: name,
            file,
            line,
            message,
            kind: "failure",
        });
    }
    report.counts = vec![("passed", passed), ("failed", failed), ("ignored", ignored)];
    report
}

/// Each `---- name stdout ----` block, by test name.
fn cargo_blocks(text: &str) -> Vec<(String, String)> {
    let mut blocks: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed
            .strip_prefix("---- ")
            .and_then(|rest| rest.strip_suffix(" stdout ----"))
        {
            blocks.extend(current.take());
            current = Some((name.to_string(), String::new()));
            continue;
        }
        if trimmed == "failures:" || trimmed.starts_with("test result:") {
            blocks.extend(current.take());
            continue;
        }
        if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    blocks.extend(current);
    blocks
}

/// Where a cargo test panicked, and the panic message: `panicked at
/// src/lib.rs:10:5:` followed by the message, or the older `panicked at
/// 'message', src/lib.rs:10:5`.
fn panic_site(block: &str) -> (Option<String>, Option<u64>, Option<String>) {
    let lines: Vec<&str> = block.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let Some(at) = line.find("panicked at ") else {
            continue;
        };
        let rest = line[at + "panicked at ".len()..].trim();
        if let Some(quoted) = rest.strip_prefix('\'')
            && let Some(end) = quoted.rfind("', ")
        {
            let message = &quoted[..end];
            let site = &quoted[end + 3..];
            let (file, line) = site_parts(site);
            return (file, line, Some(clip(message)));
        }
        let (file, line) = site_parts(rest.trim_end_matches(':'));
        let message: Vec<&str> = lines[index + 1..]
            .iter()
            .take_while(|line| !line.trim().is_empty() && !line.starts_with("note:"))
            .take(3)
            .map(|line| line.trim())
            .collect();
        let message = (!message.is_empty()).then(|| clip(&message.join(" ")));
        return (file, line, message);
    }
    (None, None, None)
}

/// `src/lib.rs:10:5` as a file and a line.
fn site_parts(site: &str) -> (Option<String>, Option<u64>) {
    let mut parts = site.rsplitn(3, ':');
    let _column = parts.next();
    let line = parts.next().and_then(|line| line.parse().ok());
    let file = parts.next().map(str::to_string);
    match (file, line) {
        (Some(file), Some(line)) => (Some(file), Some(line)),
        _ => (None, None),
    }
}

// pytest.

fn pytest(text: &str) -> Report {
    let mut report = Report {
        format: "pytest",
        ..Report::default()
    };
    // Locations from the FAILURES section: a `___ name ___` header, then
    // `path:line: Exception` lines, of which the last is where it failed.
    let mut sites: Vec<(String, String, u64, String)> = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("___") && trimmed.ends_with("___") {
            let name = trimmed.trim_matches('_').trim();
            current = (!name.is_empty()).then(|| name.to_string());
            continue;
        }
        if trimmed.starts_with("===") {
            current = None;
            continue;
        }
        let Some(name) = &current else {
            continue;
        };
        if let Some((site, exception)) = trimmed.split_once(": ")
            && let Some((file, number)) = site.rsplit_once(':')
            && file.ends_with(".py")
            && let Ok(number) = number.parse::<u64>()
        {
            sites.retain(|(seen, ..)| seen != name);
            sites.push((
                name.clone(),
                file.to_string(),
                number,
                exception.to_string(),
            ));
        }
    }
    let mut counts: Vec<(&'static str, u64)> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let failed = trimmed
            .strip_prefix("FAILED ")
            .map(|rest| (rest, "failure"));
        let errored = trimmed.strip_prefix("ERROR ").map(|rest| (rest, "error"));
        if let Some((rest, kind)) = failed.or(errored) {
            let (id, message) = match rest.split_once(" - ") {
                Some((id, message)) => (id.trim(), Some(clip(message))),
                None => (rest.trim(), None),
            };
            let file = id.split("::").next().map(str::to_string);
            let short = id.rsplit("::").next().unwrap_or(id);
            // An error's header reads `ERROR at setup of name`.
            let site = sites.iter().find(|(name, ..)| {
                name == short || name == id || name.ends_with(&format!(" {short}"))
            });
            report.failures.push(Failure {
                test: id.to_string(),
                file: site.map(|(_, file, ..)| file.clone()).or(file),
                line: site.map(|(_, _, line, _)| *line),
                message: message.or_else(|| site.map(|(.., exception)| clip(exception))),
                kind,
            });
        }
        if trimmed.starts_with('=') && trimmed.ends_with('=') && trimmed.contains(" in ") {
            counts = summary_counts(trimmed);
        }
    }
    // Without a short summary, the FAILURES section's headers name the
    // failures.
    if report.failures.is_empty() {
        for (name, file, line, exception) in &sites {
            report.failures.push(Failure {
                test: name.clone(),
                file: Some(file.clone()),
                line: Some(*line),
                message: Some(clip(exception)),
                kind: "failure",
            });
        }
    }
    report.counts = counts;
    report
}

/// `=== 1 failed, 2 passed in 0.12s ===` as counts.
fn summary_counts(line: &str) -> Vec<(&'static str, u64)> {
    let body = line.trim_matches('=').trim();
    let body = body.split(" in ").next().unwrap_or(body);
    let mut counts = Vec::new();
    for part in body.split(',') {
        let mut words = part.split_whitespace();
        if let (Some(count), Some(word)) = (words.next(), words.next())
            && let Ok(count) = count.parse::<u64>()
        {
            let name = match word {
                "passed" => "passed",
                "failed" => "failed",
                "error" | "errors" => "errors",
                "skipped" => "skipped",
                "xfailed" => "xfailed",
                "xpassed" => "xpassed",
                _ => continue,
            };
            counts.push((name, count));
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_pdk::guest::MemoryHost;
    use std::path::Path;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/tree")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn junit_failures_carry_their_class_file_and_message() {
        let report = report(&fixture("reports/junit.xml")).unwrap();
        assert_eq!(report.format, "junit");
        assert_eq!(
            report.counts,
            [("tests", 4), ("failed", 1), ("errors", 1), ("skipped", 1)]
        );
        assert_eq!(report.failures.len(), 2);
        let first = &report.failures[0];
        assert_eq!(first.test, "tests.test_core::test_scale");
        assert_eq!(first.file.as_deref(), Some("tests/test_core.py"));
        assert_eq!(first.line, Some(12));
        assert_eq!(first.message.as_deref(), Some("assert [2, 4] == [2, 4, 6]"));
        assert_eq!(report.failures[1].kind, "error");
        assert_eq!(report.failures[1].file.as_deref(), Some("tests/test_io.py"));
        assert_eq!(report.failures[1].line, Some(7));
    }

    #[test]
    fn cargo_failures_carry_the_panic_site() {
        let report = report(&fixture("cargo-test-output.txt")).unwrap();
        assert_eq!(report.format, "cargo");
        assert_eq!(
            report.counts,
            [("passed", 3), ("failed", 2), ("ignored", 1)]
        );
        let names: Vec<&str> = report.failures.iter().map(|f| f.test.as_str()).collect();
        assert_eq!(
            names,
            ["ledger::tests::posts_twice", "ledger::tests::old_style"]
        );
        let first = &report.failures[0];
        assert_eq!(first.file.as_deref(), Some("src/ledger.rs"));
        assert_eq!(first.line, Some(42));
        assert!(first.message.as_deref().unwrap().contains("left == right"));
        let old = &report.failures[1];
        assert_eq!(old.file.as_deref(), Some("src/ledger.rs"));
        assert_eq!(old.line, Some(51));
        assert_eq!(old.message.as_deref(), Some("balance went negative"));
    }

    #[test]
    fn pytest_failures_carry_the_failing_line() {
        let report = report(&fixture("pytest-results.log")).unwrap();
        assert_eq!(report.format, "pytest");
        assert_eq!(report.counts, [("failed", 1), ("passed", 2), ("errors", 1)]);
        assert_eq!(report.failures.len(), 2);
        let first = &report.failures[0];
        assert_eq!(first.test, "tests/test_core.py::test_scale");
        assert_eq!(first.file.as_deref(), Some("tests/test_core.py"));
        assert_eq!(first.line, Some(12));
        assert_eq!(first.message.as_deref(), Some("assert [2, 4] == [2, 4, 6]"));
        let error = &report.failures[1];
        assert_eq!(error.kind, "error");
        assert_eq!(error.file.as_deref(), Some("tests/conftest.py"));
        assert_eq!(error.line, Some(5));
    }

    #[test]
    fn the_operation_finds_reports_by_name_and_reads_them_by_content() {
        let mut host =
            MemoryHost::from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/tree"));
        let value = handle(&MemoryHost::request("parse", json!({})), &mut host).unwrap();
        assert_eq!(value["kind"], "test-report");
        assert_eq!(value["files_considered"], 4);
        let formats: Vec<&str> = value["reports"]
            .as_array()
            .unwrap()
            .iter()
            .map(|report| report["format"].as_str().unwrap())
            .collect();
        assert_eq!(formats, ["cargo", "pytest", "junit"]);
        assert_eq!(value["unrecognized"], json!(["test-notes.txt"]));
        assert_eq!(value["failures_total"], 6);
        assert_eq!(value["truncated"], false);

        let value = handle(
            &MemoryHost::request(
                "parse",
                json!({"paths": ["reports/junit.xml"], "max_failures": 1}),
            ),
            &mut host,
        )
        .unwrap();
        assert_eq!(value["files_considered"], 1);
        assert_eq!(value["reports"][0]["failures"].as_array().unwrap().len(), 1);
        assert_eq!(value["truncated"], true);
    }

    #[test]
    fn names_that_look_like_reports() {
        assert!(looks_like_report("target/junit.xml"));
        assert!(looks_like_report("out/pytest-results.log"));
        assert!(looks_like_report("TEST-app.xml"));
        assert!(!looks_like_report("src/lib.rs"));
        assert!(!looks_like_report("pom.xml"));
        assert!(!looks_like_report("notes.txt"));
    }
}
