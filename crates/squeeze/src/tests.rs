use std::cell::Cell;

use super::*;

/// A judge that rates by a closure over the chunk index, or fails.
struct Fake<F: Fn(usize) -> f64> {
    rate: Option<F>,
    calls: Cell<usize>,
}

impl<F: Fn(usize) -> f64> Judge for Fake<F> {
    fn rate(
        &self,
        _input: &str,
        questions: &[(String, String)],
    ) -> Result<(String, Option<String>, u64, Vec<(String, f64)>), String> {
        self.calls.set(self.calls.get() + 1);
        let rate = self.rate.as_ref().ok_or("unreachable")?;
        Ok((
            "clef-flash".into(),
            Some("clef".into()),
            30,
            questions
                .iter()
                .map(|(name, _)| (name.clone(), rate(name[1..].parse().unwrap())))
                .collect(),
        ))
    }
}

fn fake(rate: impl Fn(usize) -> f64) -> Fake<impl Fn(usize) -> f64> {
    Fake {
        rate: Some(rate),
        calls: Cell::new(0),
    }
}

fn failing() -> Fake<fn(usize) -> f64> {
    Fake {
        rate: None,
        calls: Cell::new(0),
    }
}

/// A cargo-test-shaped log: progress, then `errors` panics spread through
/// it, then a summary.
fn log(progress: usize, errors: &[usize]) -> String {
    let mut out = Vec::new();
    for i in 0..progress {
        if errors.contains(&i) {
            out.push(format!(
                "thread 'tests::case_{i}' panicked at src/lib.rs:{i}:5:"
            ));
            out.push("assertion `left == right` failed".to_owned());
        } else {
            out.push(format!("test tests::case_{i} ... ok"));
        }
    }
    out.push(String::new());
    out.push("test result: FAILED. 10 passed; 2 failed; 0 ignored".to_owned());
    out.join("\n")
}

#[test]
fn short_output_passes_unchanged() {
    let text = log(20, &[3]);
    let judge = failing();
    assert!(squeeze(&text, "cargo test", 101, None, &judge).is_none());
    assert_eq!(judge.calls.get(), 0);
}

#[test]
fn flagged_chunks_that_fit_are_kept_without_a_decision() {
    let text = log(600, &[100, 400]);
    let judge = failing();
    let (reply, line) = squeeze(&text, "cargo test", 101, None, &judge).unwrap();
    assert_eq!(judge.calls.get(), 0, "the rule decided");
    assert!(reply.contains("case_100' panicked"), "{reply}");
    assert!(reply.contains("case_400' panicked"));
    assert!(reply.contains("test result: FAILED"));
    assert!(reply.contains("omitted"));
    assert!(reply.lines().count() < 200, "{}", reply.lines().count());
    assert_eq!(line["how"], "rule");
}

#[test]
fn too_many_flagged_chunks_ask_the_decision_model() {
    let errors: Vec<usize> = (10..590).step_by(20).collect();
    let text = log(600, &errors);
    // Only chunk 10 matters.
    let judge = fake(|i| if i == 10 { 0.95 } else { 0.1 });
    let (reply, line) = squeeze(&text, "cargo test", 101, None, &judge).unwrap();
    assert_eq!(judge.calls.get(), 1);
    assert_eq!(line["how"], "decision");
    assert_eq!(line["door"], "clef");
    assert_eq!(line["detail"]["kept_chunks"], json!([10]));
    assert!(reply.contains("middle chunks kept by decision"));
}

#[test]
fn a_failed_decision_falls_back_to_the_first_flagged_chunks() {
    let errors: Vec<usize> = (10..590).step_by(20).collect();
    let text = log(600, &errors);
    let judge = failing();
    let (reply, line) = squeeze(&text, "cargo test", 101, None, &judge).unwrap();
    assert_eq!(line["how"], "fallback");
    assert!(reply.contains("case_10' panicked"), "the first error stays");
    let kept = line["detail"]["kept_chunks"].as_array().unwrap().len();
    assert!(kept > 0 && kept * CHUNK_LINES <= MIDDLE_BUDGET + CHUNK_LINES);
}

#[test]
fn unflagged_long_output_asks_and_can_keep_nothing() {
    let text = (0..500)
        .map(|i| format!("   Compiling crate_{i} v0.1.0"))
        .collect::<Vec<_>>()
        .join("\n");
    let judge = fake(|_| 0.05);
    let (reply, line) = squeeze(&text, "cargo build", 0, None, &judge).unwrap();
    assert_eq!(line["detail"]["kept_chunks"], json!([]));
    assert_eq!(reply.lines().count(), HEAD_LINES + 1 + TAIL_LINES + 1);
}

#[test]
fn passing_summaries_are_not_failures() {
    assert!(!flagged("test result: ok. 40 passed; 0 failed; 0 ignored"));
    assert!(!flagged("Finished with 0 errors"));
    assert!(flagged("error[E0308]: mismatched types"));
    assert!(flagged("test result: FAILED. 1 passed; 2 failed"));
    assert!(flagged("not ok 3 - parses dates"));
}

#[test]
fn chunks_cover_the_middle_exactly() {
    let text = log(300, &[]);
    let lines: Vec<&str> = text.lines().collect();
    let middle = chunks(&lines, 8, lines.len() - 40);
    assert_eq!(middle.first().unwrap().start, 8);
    assert_eq!(middle.last().unwrap().end, lines.len() - 40);
    for pair in middle.windows(2) {
        assert_eq!(pair[0].end, pair[1].start);
    }
    assert!(middle.iter().all(|c| c.len() <= CHUNK_LINES));
}

#[test]
fn the_hook_rewrites_only_noisy_foreground_bash() {
    let call = |command: &str, background: bool| {
        json!({"tool_name": "Bash", "tool_input": {
            "command": command, "description": "d", "run_in_background": background}})
    };
    let out = hook_output(&call("cargo test -p x", false), "sq").unwrap();
    assert_eq!(out["hookEventName"], "PreToolUse");
    assert_eq!(out["updatedInput"]["command"], "sq 'cargo test -p x'");
    assert_eq!(out["updatedInput"]["description"], "d");
    assert!(out.get("permissionDecision").is_none());
    assert!(hook_output(&call("cargo test", true), "sq").is_none());
    assert!(hook_output(&call("ls", false), "sq").is_none());
    let read = json!({"tool_name": "Read", "tool_input": {"file_path": "x"}});
    assert!(hook_output(&read, "sq").is_none());
}

#[cfg(unix)]
#[test]
fn the_squeezer_keeps_the_exit_status_and_saves_the_full_output() {
    let dir = tempfile::tempdir().unwrap();
    // SAFETY: tests in this crate that read these variables run here only.
    unsafe {
        std::env::set_var(DIR_VAR, dir.path());
        std::env::set_var(LOG_VAR, dir.path().join("log.jsonl"));
    }
    let (text, full, status) =
        run("for i in $(seq 1 300); do echo line $i; done; echo oops >&2; exit 7").unwrap();
    assert_eq!(status, 7);
    assert!(text.contains("line 300\noops"));
    let full = full.unwrap();
    assert_eq!(std::fs::read_to_string(&full).unwrap(), text);
    let reply = render(
        &text.lines().collect::<Vec<_>>(),
        &[],
        &[],
        &Picked::Rule,
        Some(&full),
        status,
    );
    assert!(reply.contains(&full.display().to_string()));
    assert!(reply.ends_with("exit 7]\n"));
}
