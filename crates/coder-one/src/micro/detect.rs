//! The lean loop's hook for [`crate::stall`]: after a work session, read
//! the dispatch's session logs back, take the session-end checkpoint, ask
//! Jev when the answer can matter, and let code choose the action.

use super::*;
use crate::stall::{self, Action, Answers, At, Detect};

/// What the detectors concluded after one session.
pub(super) struct Detected {
    /// The record the session's lean move carries.
    pub record: Value,
    /// Jev's cost.
    pub usd: f64,
    pub action: Action,
    /// The next session's re-brief, on a stall.
    pub rebrief: Option<String>,
    /// The next session's suggested first step.
    pub next: Option<String>,
}

impl Micro {
    /// Runs the detectors `detect` turns on after session `number`, with
    /// `rebriefed` saying whether the last one re-briefed.
    pub(super) async fn detect_after(
        &self,
        prepared: &Prepared,
        detect: &Detect,
        sessions: &[Ran],
        number: u32,
        rebriefed: bool,
    ) -> Detected {
        let parsed: Vec<stall::Session> = sessions
            .iter()
            .filter_map(|ran| {
                let name = Path::new(&ran.trace).file_name()?;
                std::fs::read_to_string(self.artifacts.join(name)).ok()
            })
            .map(|text| stall::parse_session(&text))
            .collect();
        let Some(last) = parsed.last() else {
            return Detected {
                record: json!({ "error": "no session log to read" }),
                usd: 0.0,
                action: Action::Continue,
                rebrief: None,
                next: None,
            };
        };
        let checkpoint = stall::checkpoint(&parsed, parsed.len(), last.turns, At::SessionEnd);
        let ask = (detect.stall && checkpoint.features.suspect()) || detect.next_step;
        let (answers, how, error, usd) = if ask {
            let (state, questions) = stall::request(&prepared.instruction, &checkpoint);
            let asked = jev_component::ask(
                &prepared.jev,
                &self.recorder,
                jev_component::Ask {
                    component: stall::STALL_COMPONENT,
                    name: stall::DECISION,
                    id: format!("jev-stall-{}-{number}", self.dispatch()),
                    state,
                    questions,
                    parent: None,
                    deadline: prepared.deadline.clone(),
                },
            )
            .await;
            let usd = asked.input_tokens.map_or(0.0, |t| {
                t as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0
            });
            (
                Answers::from_asked(&asked),
                asked.how,
                asked.error.clone(),
                usd,
            )
        } else {
            (Answers::default(), "not_asked", None, 0.0)
        };
        let verdict = stall::decide(&checkpoint.features, &answers, stall::Params::default());
        let action = if detect.stall {
            stall::action(verdict.stalled, rebriefed)
        } else {
            Action::Continue
        };
        let rebrief = (action == Action::Rebrief).then(|| stall::rebrief_note(&checkpoint));
        let next = detect
            .next_step
            .then(|| stall::next_note(&checkpoint, &answers))
            .flatten();
        if action != Action::Continue {
            crate::say::line(&format!(
                "  microluna ▸ stall check after session {number}: {}",
                match action {
                    Action::Rebrief => "re-briefing the next session",
                    _ => "stopping the work sessions",
                }
            ));
        }
        Detected {
            record: json!({
                "features": checkpoint.features,
                "candidates": checkpoint.candidates,
                "answers": answers,
                "verdict": verdict,
                "action": action,
                "rebrief": rebrief,
                "next": next,
                // Report-only: no code reads it to end the loop.
                "done_report_only": answers.done,
                "jev": { "how": how, "error": error },
                "jev_usd": usd,
            }),
            usd,
            action,
            rebrief,
            next,
        }
    }
}

#[cfg(test)]
mod tests {
    use microluna::fake::call;

    use super::*;
    use crate::component::jev::{JevMode, Recorded, RecordedAnswer};

    const TASK: &str = "Make `sh check.sh` exit 0.";

    fn usage() -> TokenUsage {
        TokenUsage {
            input: 1_000,
            cached: 0,
            output: 20,
            reasoning: 0,
        }
    }

    /// A session that edits once and then fails the same check nine
    /// times, and a second that does the same.
    fn stuck_replies(sessions: usize) -> Vec<microluna::Reply> {
        let mut replies = Vec::new();
        for s in 0..sessions {
            replies.push(call(
                &format!("s{s}w"),
                "write_file",
                &json!({ "path": "check.sh", "contents": "exit 3\n" }),
                usage(),
            ));
            for i in 0..9 {
                replies.push(call(
                    &format!("s{s}r{i}"),
                    "run_command",
                    &json!({ "command": "sh check.sh", "timeout_seconds": 10 }),
                    usage(),
                ));
            }
            replies.push(call(
                &format!("s{s}f"),
                "finish",
                &json!({ "status": "failed", "summary": "check.sh still exits 3.", "answer": "" }),
                usage(),
            ));
        }
        replies
    }

    fn lean(detect: Option<Detect>) -> lean::Lean {
        serde_json::from_value(json!({
            "sessions": 3,
            "source_chars": 1000,
            "detect": detect,
        }))
        .unwrap()
    }

    fn executor(
        dir: &Path,
        replies: Vec<microluna::Reply>,
        lean: lean::Lean,
        jev: JevMode,
    ) -> Micro {
        let work = dir.join("work");
        let artifacts = dir.join("artifacts");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(&artifacts).unwrap();
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
        micro.wire = Ok(Wire::Fake(FakeTransport::new(replies)));
        micro.prepared = Some(Prepared {
            instruction: TASK.to_string(),
            title: "check".to_string(),
            directions: String::new(),
            requirements: crate::requirements::mechanical(TASK),
            items: Vec::new(),
            informs: std::collections::BTreeMap::new(),
            jev,
            deadline: None,
        });
        micro
    }

    fn briefing() -> Briefing {
        Briefing {
            text: TASK.to_string(),
            cap: 12_000,
            included: Vec::new(),
            omitted: Vec::new(),
        }
    }

    fn detects(record: &Value) -> Vec<Value> {
        record["moves"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m.get("detect").cloned())
            .collect()
    }

    #[tokio::test]
    async fn without_the_switch_the_loop_runs_every_session() {
        let dir = tempfile::tempdir().unwrap();
        let mut micro = executor(dir.path(), stuck_replies(3), lean(None), JevMode::Off);
        micro.execute(&briefing()).await;
        let record = micro.last.clone().unwrap();
        assert_eq!(record["sessions"].as_array().unwrap().len(), 3);
        assert!(detects(&record).is_empty());
        assert!(record["policy"]["lean"].get("detect").is_none());
    }

    #[tokio::test]
    async fn a_stall_rebriefs_then_a_second_stall_stops_the_work_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let detect = Detect {
            stall: true,
            next_step: false,
        };
        // Jev off: the strong code signal decides.
        let mut micro = executor(
            dir.path(),
            stuck_replies(3),
            lean(Some(detect)),
            JevMode::Off,
        );
        micro.execute(&briefing()).await;
        let record = micro.last.clone().unwrap();
        let found = detects(&record);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0]["action"], "rebrief");
        assert_eq!(found[0]["verdict"]["by"], "code_fallback");
        assert_eq!(found[1]["action"], "stop");
        assert_eq!(record["sessions"].as_array().unwrap().len(), 2);
        assert!(
            record["stopped"]
                .as_str()
                .unwrap()
                .contains("stall check stopped the work sessions after session 2")
        );
        // The second session was briefed with the evidence.
        let fake = match &micro.wire {
            Ok(Wire::Fake(fake)) => fake,
            _ => unreachable!(),
        };
        let second = fake
            .requests()
            .into_iter()
            .map(|r| serde_json::to_string(&r.input).unwrap())
            .find(|input| input.contains("Session 2 of at most"))
            .unwrap();
        assert!(second.contains("stall check after session 1"), "{second}");
        assert!(second.contains("failed 9 times"), "{second}");
    }

    #[tokio::test]
    async fn jev_saying_the_session_progresses_keeps_the_loop_going() {
        // First run with Jev off, to read back the exact request the loop
        // asks after session 1.
        let first = tempfile::tempdir().unwrap();
        let detect = Detect {
            stall: true,
            next_step: true,
        };
        let mut micro = executor(
            first.path(),
            stuck_replies(1),
            lean(Some(detect.clone())),
            JevMode::Off,
        );
        micro.execute(&briefing()).await;
        let text = std::fs::read_to_string(first.path().join("artifacts/microluna-1-1.atif.jsonl"))
            .unwrap();
        let parsed = vec![stall::parse_session(&text)];
        let at = stall::checkpoint(&parsed, 1, parsed[0].turns, At::SessionEnd);
        let (state, questions) = stall::request(TASK, &at);
        let body = jev::SystemOneRequest::new(jev::Entry::from(state), questions)
            .body(crate::credentials::JEV_MODEL)
            .unwrap();
        let key = crate::component::jev::key(&body["state"], &body["questions"]);
        let mut recorded = Recorded::empty();
        recorded.entries.insert(
            key,
            RecordedAnswer {
                name: stall::DECISION.to_string(),
                model: "jev-test".to_string(),
                answers: json!({
                    "repeating": { "noul": 0.05 },
                    "progress": { "noul": 0.95 },
                    "done": { "noul": 0.9 },
                    "next": { "choice": "run_example", "confidence": 0.8,
                        "probabilities": { "run_example": 0.8, "continue": 0.2 } },
                }),
                input_tokens: Some(2_000),
                output_tokens: Some(10),
                milliseconds: Some(100),
                source: "test".to_string(),
            },
        );
        let second = tempfile::tempdir().unwrap();
        let mut micro = executor(
            second.path(),
            stuck_replies(2),
            lean(Some(detect)),
            JevMode::Recorded(recorded),
        );
        micro.execute(&briefing()).await;
        let record = micro.last.clone().unwrap();
        let found = detects(&record);
        assert_eq!(found[0]["jev"]["how"], "recorded", "{found:?}");
        assert_eq!(found[0]["verdict"]["stalled"], false);
        assert_eq!(found[0]["action"], "continue");
        // A done answer is reported, and the loop doesn't end on it.
        assert_eq!(found[0]["done_report_only"], 0.9);
        assert!(record["sessions"].as_array().unwrap().len() >= 2);
        let fake = match &micro.wire {
            Ok(Wire::Fake(fake)) => fake,
            _ => unreachable!(),
        };
        let brief = fake
            .requests()
            .into_iter()
            .map(|r| serde_json::to_string(&r.input).unwrap())
            .find(|input| input.contains("Session 2 of at most"))
            .unwrap();
        assert!(brief.contains("suggests starting this session"), "{brief}");
        assert!(brief.contains("sh check.sh"), "{brief}");
    }
}
