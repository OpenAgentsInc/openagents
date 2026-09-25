//! The lean loop's hooks for [`crate::localize`] (issue #9658): after a
//! work session, read its log back and the host's own check results,
//! and put what the three components find in the next brief; inside a
//! work session, [`Watching`] tells the session the source lines a failing
//! command's output names, after the turn that ran it.

use std::collections::BTreeSet;

use super::*;
use crate::checks::contract::executed;
use crate::localize::{self, Failure, Localize, Slow};

/// Failing commands of one session a note reads, the most recent kept.
const SESSION_FAILURES: usize = 6;

/// What the host's own checks showed after a session.
pub(super) struct Checked<'a> {
    /// The frozen score's output, when it ran and didn't pass everything.
    pub score_tail: Option<&'a str>,
    /// The commands `verify.executed` reran on the candidate.
    pub executed: &'a [executed::Record],
    /// Shared acceptance results the loop ran on the candidate, such as
    /// `checks.oracle`'s (issue #9656); their first failing case comes first.
    pub acceptance: &'a [crate::checks::acceptance::Acceptance],
    /// `verify.executed`'s per-command bound, in seconds.
    pub executed_sec: u64,
    /// The lean loop's cap on a session command's bound, in seconds; 0
    /// leaves the bound each command asked for.
    pub command_sec: u64,
}

fn record_output(record: &executed::Record) -> String {
    let mut text = record.stdout_head.clone();
    if !record.stderr_head.trim().is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str("[stderr]\n");
        text.push_str(&record.stderr_head);
    }
    text
}

impl Micro {
    /// The session's log, read back from the dispatch's artifacts.
    fn session_log(&self, ran: &Ran) -> localize::trace::Log {
        Path::new(&ran.trace)
            .file_name()
            .and_then(|name| std::fs::read_to_string(self.artifacts.join(name)).ok())
            .map(|text| localize::trace::parse(&text))
            .unwrap_or_default()
    }

    /// Runs the components `policy` turns on after work session `ran`,
    /// and returns the next brief's notes and the record.
    pub(super) async fn localize_after(
        &self,
        policy: &Localize,
        ran: &Ran,
        checked: &Checked<'_>,
        time_left: Duration,
    ) -> (Vec<String>, Value) {
        let log = self.session_log(ran);
        let session: Vec<Failure> = log
            .calls
            .iter()
            .filter_map(|(_, call)| call.failure())
            .collect();
        let session = &session[session.len().saturating_sub(SESSION_FAILURES)..];
        // Oldest first: the session's own failures, then the host's reruns,
        // then its score.
        let mut failures: Vec<Failure> = session.to_vec();
        for record in checked
            .executed
            .iter()
            .filter(|r| r.exit.is_some_and(|e| e != 0) || r.timed_out)
        {
            failures.push(Failure {
                command: record.command.clone(),
                output: record_output(record),
                exit: record.exit.map(i64::from),
                timed_out: record.timed_out,
                milliseconds: record.ms,
            });
        }
        if let Some(tail) = checked.score_tail {
            failures.push(Failure {
                command: "the host's frozen score script".to_string(),
                output: tail.to_string(),
                exit: None,
                timed_out: false,
                milliseconds: 0,
            });
        }
        let mut notes = Vec::new();
        let mut record = json!({ "failures": failures.len() });
        if policy.error_context {
            let (text, found) =
                localize::error_context(policy, &self.workdir, &failures, &BTreeSet::new());
            if let Some(text) = text {
                notes.push(localize::note(ran.number, localize::ERROR_CONTEXT, &text));
            }
            record["error_context"] = found;
        }
        if policy.mismatch_trace {
            let checks: Vec<(String, String)> = failures
                .iter()
                .rev()
                .map(|f| (f.command.clone(), f.output.clone()))
                .collect();
            let (text, found) = localize::mismatch_trace(checked.acceptance, &checks);
            if let Some(text) = text {
                notes.push(localize::note(ran.number, localize::MISMATCH_TRACE, &text));
            }
            record["mismatch_trace"] = found;
        }
        if policy.phase_timing {
            let mut slow: Vec<Slow> = log
                .calls
                .iter()
                .filter_map(|(_, call)| match call {
                    localize::trace::Call::Command {
                        command,
                        timed_out,
                        milliseconds,
                        bound_sec,
                        ..
                    } => Some(Slow {
                        command: command.clone(),
                        milliseconds: *milliseconds,
                        bound_ms: if checked.command_sec > 0 {
                            (*bound_sec).min(checked.command_sec)
                        } else {
                            *bound_sec
                        }
                        .saturating_mul(1000),
                        timed_out: *timed_out,
                    }),
                    _ => None,
                })
                .collect();
            slow.extend(checked.executed.iter().map(|r| Slow {
                command: r.command.clone(),
                milliseconds: r.ms,
                bound_ms: checked.executed_sec.saturating_mul(1000),
                timed_out: r.timed_out,
            }));
            match localize::timing_target(&slow) {
                Some(target) if time_left > Duration::from_secs(policy.timing_sec + 60) => {
                    let place = executed::Place {
                        workdir: self.workdir.clone(),
                        contained: self.isolation == Isolation::TaskContainer,
                        wall: Duration::from_secs(policy.timing_sec),
                        budget: Duration::from_secs(policy.timing_sec + 5),
                    };
                    crate::say::line(&format!(
                        "  microluna ▸ timing `{}` once more under a profiler, at most {} s",
                        crate::judge::clip(&target.command, 80),
                        policy.timing_sec
                    ));
                    let (text, found) = localize::phase_timing(policy, target, &place).await;
                    if let Some(text) = text {
                        notes.push(localize::note(ran.number, localize::PHASE_TIMING, &text));
                    }
                    record["phase_timing"] = found;
                }
                Some(target) => {
                    record["phase_timing"] = json!({
                        "component": localize::PHASE_TIMING,
                        "target": target,
                        "skipped": "too little time left in the dispatch",
                    });
                }
                None => {
                    record["phase_timing"] = json!({
                        "component": localize::PHASE_TIMING,
                        "target": null,
                    });
                }
            }
        }
        record["notes"] = json!(notes.len());
        (notes, record)
    }
}

/// A work session's watch with the in-session error context
/// ([`Localize::in_session`]) after `inner`'s: when `inner` lets the
/// session go on and the turn just run had a failing command whose output
/// names workspace source lines not told yet, the host tells the session
/// those lines, at most [`localize::IN_SESSION_TELLS`] times.
pub(super) struct Watching<'a, W> {
    pub inner: W,
    policy: Option<&'a Localize>,
    workdir: PathBuf,
    told: BTreeSet<(String, u64)>,
    /// One record per tell.
    pub records: Vec<Value>,
}

impl<'a, W> Watching<'a, W> {
    pub(super) fn new(inner: W, policy: Option<&'a Localize>, workdir: PathBuf) -> Self {
        Watching {
            inner,
            policy: policy.filter(|p| p.error_context && p.in_session),
            workdir,
            told: BTreeSet::new(),
            records: Vec::new(),
        }
    }

    /// The tells' record, or `None` when there was none.
    pub(super) fn record(&self) -> Option<Value> {
        (!self.records.is_empty()).then(|| json!(self.records))
    }
}

impl<W: microluna::Watch> microluna::Watch for Watching<'_, W> {
    async fn after_turn(&mut self, turn: usize, steps: &[Step]) -> microluna::Intervention {
        let first = self.inner.after_turn(turn, steps).await;
        let Some(policy) = self.policy else {
            return first;
        };
        if first != microluna::Intervention::Continue
            || self.records.len() >= localize::IN_SESSION_TELLS
        {
            return first;
        }
        let log = localize::trace::from_steps(steps);
        let last = log.turns.max(1);
        let failures: Vec<Failure> = log
            .calls
            .iter()
            .filter(|(t, _)| *t == last)
            .filter_map(|(_, call)| call.failure())
            .collect();
        if failures.is_empty() {
            return first;
        }
        let (text, record) = localize::error_context(policy, &self.workdir, &failures, &self.told);
        let Some(text) = text else {
            return first;
        };
        for region in record["regions"].as_array().into_iter().flatten() {
            let file = region["file"].as_str().unwrap_or("").to_string();
            for line in region["named"].as_array().into_iter().flatten() {
                if let Some(line) = line.as_u64() {
                    self.told.insert((file.clone(), line));
                }
            }
        }
        self.records
            .push(json!({ "turn": turn, "error_context": record }));
        crate::say::line(&format!(
            "  microluna ▸ after turn {turn}, telling the session the source lines its failing \
             command names"
        ));
        microluna::Intervention::Tell(format!(
            "The host's {}, for the command that just failed:\n{text}",
            localize::ERROR_CONTEXT
        ))
    }
}

#[cfg(test)]
mod tests {
    use microluna::fake::call;

    use super::*;
    use crate::component::jev::JevMode;

    const TASK: &str = "Make `sh check.sh` exit 0.";

    fn usage() -> TokenUsage {
        TokenUsage {
            input: 1_000,
            cached: 0,
            output: 20,
            reasoning: 0,
        }
    }

    /// Session 1 runs a check that prints a traceback into `app.py` and a
    /// failing case, then gives up; session 2 gives up at once.
    fn replies() -> Vec<microluna::Reply> {
        vec![
            call(
                "s1r",
                "run_command",
                &json!({ "command": "sh check.sh", "timeout_seconds": 10 }),
                usage(),
            ),
            call(
                "s1f",
                "finish",
                &json!({ "status": "failed", "summary": "check.sh fails.", "answer": "" }),
                usage(),
            ),
            call(
                "s2f",
                "finish",
                &json!({ "status": "failed", "summary": "still fails.", "answer": "" }),
                usage(),
            ),
        ]
    }

    fn executor(dir: &Path, localize: Option<Value>) -> Micro {
        let work = dir.join("work");
        let artifacts = dir.join("artifacts");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(&artifacts).unwrap();
        std::fs::write(
            work.join("app.py"),
            "def f(x):\n    y = x + 1\n    return y / 0\n\nprint(f(1))\n",
        )
        .unwrap();
        std::fs::write(
            work.join("check.sh"),
            "echo 'Traceback (most recent call last):' >&2\n\
             echo '  File \"app.py\", line 3, in f' >&2\n\
             echo 'expected: 2'\necho 'got: 3'\nexit 1\n",
        )
        .unwrap();
        let mut lean = json!({ "sessions": 2, "source_chars": 1000 });
        if let Some(localize) = localize {
            lean["localize"] = localize;
        }
        let lean: lean::Lean = serde_json::from_value(lean).unwrap();
        let mut micro = Micro::new(
            "gpt-6-luna",
            None,
            Duration::from_secs(120),
            &work,
            &artifacts,
            Recorder::default(),
            0,
            Policy {
                lean: Some(lean),
                spend_usd: 1.0,
                ..Policy::default()
            },
            Isolation::TaskContainer,
        );
        micro.wire = Ok(Wire::Fake(FakeTransport::new(replies())));
        micro.prepared = Some(Prepared {
            instruction: TASK.to_string(),
            title: "check".to_string(),
            directions: String::new(),
            requirements: crate::requirements::mechanical(TASK),
            items: Vec::new(),
            informs: std::collections::BTreeMap::new(),
            jev: JevMode::Off,
            deadline: None,
        });
        micro
    }

    fn inputs(micro: &Micro) -> Vec<String> {
        match &micro.wire {
            Ok(Wire::Fake(fake)) => fake
                .requests()
                .into_iter()
                .map(|r| serde_json::to_string(&r.input).unwrap())
                .collect(),
            _ => unreachable!(),
        }
    }

    fn briefing() -> Briefing {
        Briefing {
            text: TASK.to_string(),
            cap: 12_000,
            included: Vec::new(),
            omitted: Vec::new(),
        }
    }

    #[tokio::test]
    async fn without_the_switch_no_note_is_added() {
        let dir = tempfile::tempdir().unwrap();
        let mut micro = executor(dir.path(), None);
        micro.execute(&briefing()).await;
        let sent = inputs(&micro);
        assert_eq!(sent.len(), 3);
        assert!(!sent.iter().any(|s| s.contains("evidence.error_context")));
        let record = micro.last.clone().unwrap();
        assert!(record["policy"]["lean"].get("localize").is_none());
        assert!(
            record["moves"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m.get("localize").is_none())
        );
    }

    #[tokio::test]
    async fn the_next_session_and_the_next_turn_get_the_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let mut micro = executor(
            dir.path(),
            Some(json!({"error_context": true, "in_session": true, "mismatch_trace": true})),
        );
        micro.execute(&briefing()).await;
        let sent = inputs(&micro);
        assert_eq!(sent.len(), 3, "{sent:?}");
        // The turn after the failing command heard the lines it names.
        assert!(!sent[0].contains("for the command that just failed"));
        assert!(
            sent[1].contains("for the command that just failed"),
            "{}",
            sent[1]
        );
        assert!(sent[1].contains(">     3      return y / 0"), "{}", sent[1]);
        // The next session's brief carries both notes.
        assert!(
            sent[2].contains("After session 1, the host's evidence.error_context"),
            "{}",
            sent[2]
        );
        assert!(sent[2].contains("After session 1, the host's evidence.mismatch_trace"));
        assert!(sent[2].contains("Expected: 2"));
        assert!(sent[2].contains("Observed: 3"));
        let record = micro.last.clone().unwrap();
        let first = record["moves"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["kind"] == "lean" && m["after_session"] == 1)
            .unwrap()
            .clone();
        assert_eq!(first["localize"]["error_context"]["resolved"], 1);
        assert_eq!(
            first["localize"]["mismatch_trace"]["case"]["format"],
            "labeled"
        );
        assert_eq!(first["localize_in_session"][0]["turn"], 1);
    }
}
