//! The lean loop's hooks for [`crate::stall`]: after a work session, read
//! the dispatch's session logs back, take the session-end checkpoint, ask
//! Jev when the answer can matter, and let code choose the action. Inside
//! a work session, [`InSession`] takes the every-8-turns checkpoints and
//! acts through Microluna's host interventions ([`microluna::Watch`]).

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

/// Jev's answers at a checkpoint, how they were got, and their cost.
struct Consulted {
    answers: Answers,
    how: &'static str,
    error: Option<String>,
    usd: f64,
}

/// The in-session stall check ([`Detect::in_session`]), as a
/// [`microluna::Watch`] on one work session. Every [`stall::EVERY`] turns
/// from [`stall::MIN_TURNS`], code reads the session's steps so far, with
/// the dispatch's earlier sessions, and takes an in-session checkpoint.
/// Jev is asked at a suspect one, and the detector's mode calls the stall.
/// A first stall tells the session the evidence; a stall at the next
/// checkpoint after that ends the session. Without the switch it never
/// intervenes.
pub(super) struct InSession<'a> {
    micro: &'a Micro,
    prepared: &'a Prepared,
    detect: Option<Detect>,
    earlier: Vec<stall::Session>,
    number: u32,
    told: bool,
    /// One record per checkpoint taken.
    pub records: Vec<Value>,
    /// Jev's cost across the checkpoints.
    pub usd: f64,
}

impl InSession<'_> {
    /// The checkpoints' record for the session's lean move, or `None` when
    /// none was taken.
    pub(super) fn record(&self) -> Option<Value> {
        (!self.records.is_empty()).then(|| {
            json!({
                "checkpoints": self.records,
                "jev_usd": self.usd,
            })
        })
    }
}

impl microluna::Watch for InSession<'_> {
    async fn after_turn(&mut self, turn: usize, steps: &[Step]) -> microluna::Intervention {
        let Some(detect) = self.detect.clone() else {
            return microluna::Intervention::Continue;
        };
        if turn < stall::MIN_TURNS || !(turn - stall::MIN_TURNS).is_multiple_of(stall::EVERY) {
            return microluna::Intervention::Continue;
        }
        let text = steps
            .iter()
            .map(|step| json!({ "record": "step", "step": step }).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let mut sessions = self.earlier.clone();
        sessions.push(stall::parse_session(&text));
        let checkpoint = stall::checkpoint(&sessions, sessions.len(), turn, At::InSession);
        let consulted = if checkpoint.features.suspect() {
            self.micro
                .consult(
                    self.prepared,
                    &checkpoint,
                    format!(
                        "jev-stall-{}-{}-t{turn}",
                        self.micro.dispatch(),
                        self.number
                    ),
                )
                .await
        } else {
            Consulted::none()
        };
        self.usd += consulted.usd;
        let verdict = stall::decide_in(
            detect.mode,
            &checkpoint.features,
            &consulted.answers,
            stall::Params::default(),
        );
        let action = stall::action(verdict.stalled, self.told);
        self.told = action == Action::Rebrief;
        let (intervention, note) = match action {
            Action::Continue => (microluna::Intervention::Continue, None),
            Action::Rebrief => {
                let note = stall::in_session_note(&checkpoint);
                (microluna::Intervention::Tell(note.clone()), Some(note))
            }
            Action::Stop => {
                let why = format!(
                    "The host's stall check at turn {turn} found no progress again after telling \
                     the session, so the host ended this session."
                );
                (microluna::Intervention::End(why.clone()), Some(why))
            }
        };
        if action != Action::Continue {
            crate::say::line(&format!(
                "  microluna ▸ stall check at turn {turn} of session {}: {}",
                self.number,
                match action {
                    Action::Rebrief => "telling the session the evidence",
                    _ => "ending the session",
                }
            ));
        }
        self.records.push(json!({
            "turn": turn,
            "features": checkpoint.features,
            "answers": consulted.answers,
            "verdict": verdict,
            "action": action,
            "note": note,
            "done_report_only": consulted.answers.done,
            "jev": { "how": consulted.how, "error": consulted.error },
            "jev_usd": consulted.usd,
        }));
        intervention
    }
}

impl Consulted {
    fn none() -> Self {
        Consulted {
            answers: Answers::default(),
            how: "not_asked",
            error: None,
            usd: 0.0,
        }
    }
}

impl Micro {
    /// The dispatch's sessions' logs, as the detectors read them.
    fn parsed_sessions(&self, sessions: &[Ran]) -> Vec<stall::Session> {
        sessions
            .iter()
            .filter_map(|ran| {
                let name = Path::new(&ran.trace).file_name()?;
                std::fs::read_to_string(self.artifacts.join(name)).ok()
            })
            .map(|text| stall::parse_session(&text))
            .collect()
    }

    /// The in-session watch for work session `number`: inert unless
    /// `detect` turns on both `stall` and `in_session`.
    pub(super) fn in_session<'a>(
        &'a self,
        prepared: &'a Prepared,
        detect: Option<&Detect>,
        sessions: &[Ran],
        number: u32,
    ) -> InSession<'a> {
        let detect = detect.filter(|d| d.stall && d.in_session).cloned();
        InSession {
            micro: self,
            prepared,
            earlier: if detect.is_some() {
                self.parsed_sessions(sessions)
            } else {
                Vec::new()
            },
            detect,
            number,
            told: false,
            records: Vec::new(),
            usd: 0.0,
        }
    }

    /// One Jev request at `checkpoint`.
    async fn consult(
        &self,
        prepared: &Prepared,
        checkpoint: &stall::Checkpoint,
        id: String,
    ) -> Consulted {
        let (state, questions) = stall::request(&prepared.instruction, checkpoint);
        let asked = jev_component::ask(
            &prepared.jev,
            &self.recorder,
            jev_component::Ask {
                component: stall::STALL_COMPONENT,
                name: stall::DECISION,
                id,
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
        Consulted {
            answers: Answers::from_asked(&asked),
            how: asked.how,
            error: asked.error.clone(),
            usd,
        }
    }

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
        let parsed = self.parsed_sessions(sessions);
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
        let Consulted {
            answers,
            how,
            error,
            usd,
        } = if ask {
            self.consult(
                prepared,
                &checkpoint,
                format!("jev-stall-{}-{number}", self.dispatch()),
            )
            .await
        } else {
            Consulted::none()
        };
        let verdict = stall::decide_in(
            detect.mode,
            &checkpoint.features,
            &answers,
            stall::Params::default(),
        );
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
            ..Detect::default()
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
            ..Detect::default()
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

    /// Sessions that each edit once and then fail the same check on every
    /// turn, with no finish: each runs until the host acts.
    fn looping_replies(sessions: usize, runs: usize) -> Vec<microluna::Reply> {
        let mut replies = Vec::new();
        for s in 0..sessions {
            replies.push(call(
                &format!("s{s}w"),
                "write_file",
                &json!({ "path": "check.sh", "contents": "exit 3\n" }),
                usage(),
            ));
            for i in 0..runs {
                replies.push(call(
                    &format!("s{s}r{i}"),
                    "run_command",
                    &json!({ "command": "sh check.sh", "timeout_seconds": 10 }),
                    usage(),
                ));
            }
        }
        replies
    }

    fn in_session(mode: stall::Mode) -> Detect {
        Detect {
            stall: true,
            mode,
            in_session: true,
            ..Detect::default()
        }
    }

    fn fake(micro: &Micro) -> &FakeTransport {
        match &micro.wire {
            Ok(Wire::Fake(fake)) => fake,
            _ => unreachable!(),
        }
    }

    fn inputs(micro: &Micro) -> Vec<String> {
        fake(micro)
            .requests()
            .into_iter()
            .map(|r| serde_json::to_string(&r.input).unwrap())
            .collect()
    }

    #[tokio::test]
    async fn in_session_a_stall_tells_the_session_then_a_second_one_ends_it() {
        let dir = tempfile::tempdir().unwrap();
        // Each session: an edit, then 15 failing runs. The checkpoint at
        // turn 8 tells, the one at turn 16 ends the session.
        let mut micro = executor(
            dir.path(),
            looping_replies(2, 15),
            lean(Some(in_session(stall::Mode::Code))),
            JevMode::Off,
        );
        micro.execute(&briefing()).await;
        let record = micro.last.clone().unwrap();
        let sessions = record["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 2, "{record}");
        let lean_moves: Vec<&Value> = record["moves"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["kind"] == "lean")
            .collect();
        for found in &lean_moves {
            let checkpoints = found["in_session"]["checkpoints"].as_array().unwrap();
            assert_eq!(checkpoints.len(), 2, "{found}");
            assert_eq!(checkpoints[0]["turn"], 8);
            assert_eq!(checkpoints[0]["action"], "rebrief");
            assert_eq!(checkpoints[0]["verdict"]["by"], "code");
            assert!(checkpoints[0]["verdict"]["jev"].is_null());
            assert_eq!(checkpoints[1]["turn"], 16);
            assert_eq!(checkpoints[1]["action"], "stop");
            assert_eq!(found["status"], "host_ended");
        }
        // Between the sessions the loop's own check re-briefed, and after
        // the second it stopped the work sessions.
        let found = detects(&record);
        assert_eq!(found[0]["action"], "rebrief");
        assert_eq!(found[1]["action"], "stop");
        assert!(
            record["stopped"]
                .as_str()
                .unwrap()
                .contains("stall check stopped the work sessions after session 2")
        );
        // The session heard the evidence from turn 9 on, and never before.
        let sent = inputs(&micro);
        let told = "stall check at turn 8 of this session";
        assert!(!sent[7].contains(told), "{}", sent[7]);
        assert!(sent[8].contains(told), "{}", sent[8]);
        assert!(sent[8].contains("failed commands repeated an earlier failure"));
        // Session 1 made 16 requests; the host ended it before a 17th.
        assert!(sent[16].contains("Session 2 of at most"), "{}", sent[16]);
        let log =
            std::fs::read_to_string(dir.path().join("artifacts/microluna-1-1.atif.jsonl")).unwrap();
        assert!(log.contains("the host ended this session"));
    }

    #[tokio::test]
    async fn without_in_session_the_session_runs_to_its_own_end() {
        let dir = tempfile::tempdir().unwrap();
        let detect = Detect {
            in_session: false,
            ..in_session(stall::Mode::Code)
        };
        let mut micro = executor(
            dir.path(),
            looping_replies(1, 15),
            lean(Some(detect)),
            JevMode::Off,
        );
        micro.execute(&briefing()).await;
        let record = micro.last.clone().unwrap();
        assert!(
            record["moves"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m.get("in_session").is_none())
        );
        assert!(
            inputs(&micro)
                .iter()
                .all(|input| !input.contains("of this session found no progress"))
        );
        assert_eq!(record["policy"]["lean"]["detect"]["mode"], "code");
        assert!(
            record["policy"]["lean"]["detect"]
                .get("in_session")
                .is_none()
        );
    }

    #[tokio::test]
    async fn in_session_jev_is_recorded_beside_the_code_call_and_decides_only_in_jev_mode() {
        // First run with Jev off, to read back the exact request the
        // in-session check asks at turn 8.
        let first = tempfile::tempdir().unwrap();
        let mut micro = executor(
            first.path(),
            looping_replies(1, 15),
            lean(Some(in_session(stall::Mode::Code))),
            JevMode::Off,
        );
        micro.execute(&briefing()).await;
        let text = std::fs::read_to_string(first.path().join("artifacts/microluna-1-1.atif.jsonl"))
            .unwrap();
        let parsed = vec![stall::parse_session(&text)];
        let at = stall::checkpoint(&parsed, 1, 8, At::InSession);
        assert!(at.features.suspect());
        let (state, questions) = stall::request(TASK, &at);
        let body = jev::SystemOneRequest::new(jev::Entry::from(state), questions)
            .body(crate::credentials::JEV_MODEL)
            .unwrap();
        let key = crate::component::jev::key(&body["state"], &body["questions"]);
        let recorded = || {
            let mut recorded = Recorded::empty();
            recorded.entries.insert(
                key.clone(),
                RecordedAnswer {
                    name: stall::DECISION.to_string(),
                    model: "jev-test".to_string(),
                    answers: json!({
                        "repeating": { "noul": 0.05 },
                        "progress": { "noul": 0.95 },
                        "done": { "noul": 0.1 },
                    }),
                    input_tokens: Some(2_000),
                    output_tokens: Some(10),
                    milliseconds: Some(100),
                    source: "test".to_string(),
                },
            );
            JevMode::Recorded(recorded)
        };
        let first_checkpoint = |mode| async move {
            let dir = tempfile::tempdir().unwrap();
            // A finish after turn 16, so a session the host lets run ends
            // on its own.
            let mut replies = looping_replies(1, 15);
            replies.push(call(
                "f",
                "finish",
                &json!({ "status": "failed", "summary": "check.sh still exits 3.", "answer": "" }),
                usage(),
            ));
            let mut micro = executor(
                dir.path(),
                replies,
                lean(Some(in_session(mode))),
                recorded(),
            );
            micro.execute(&briefing()).await;
            let record = micro.last.clone().unwrap();
            record["moves"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["kind"] == "lean")
                .unwrap()["in_session"]["checkpoints"][0]
                .clone()
        };
        // Code mode: Jev says the session progresses, and the code's call
        // stands; Jev's answer is kept beside it.
        let code = first_checkpoint(stall::Mode::Code).await;
        assert_eq!(code["jev"]["how"], "recorded", "{code}");
        assert_eq!(code["answers"]["progress"], 0.95);
        assert_eq!(code["verdict"]["jev"], false);
        assert_eq!(code["verdict"]["stalled"], true);
        assert_eq!(code["action"], "rebrief");
        // Jev mode: the same answer keeps the session going.
        let jev_mode = first_checkpoint(stall::Mode::Jev).await;
        assert_eq!(jev_mode["jev"]["how"], "recorded", "{jev_mode}");
        assert_eq!(jev_mode["verdict"]["stalled"], false);
        assert_eq!(jev_mode["action"], "continue");
    }
}
