## Prepared source context

Source commit: `7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6`. These are complete syntax declarations selected from a bounded candidate pool. They are starting points, not a complete dependency graph or a proof that the requirements are covered. Imports, callers, other modules, macros, cfg behavior, and unselected tests may need inspection. Read additional source whenever this material is insufficient.

### crates/coderbench/tests/negative.rs:193-207 — a_torn_trace_is_unverifiable

File SHA-256: `c4d2e8ffb6f319f4c84834217f4df7249f5278648105dac3e789df47da165976`

```rust
/// A trace with a line that did not read back is a trace with a hole in it,
/// and a hole is unknown rather than fine.
#[test]
fn a_torn_trace_is_unverifiable() {
    let directory = tempfile::tempdir().unwrap();
    let lines: Vec<String> = authored_text().lines().map(str::to_string).collect();
    // A line the writer did not finish, which is what a session killed
    // mid-write leaves behind.
    let torn = r#"{"record": "step", "step": {"at": 1789869709018, "source": "Agent", "mess"#;
    let mut with_hole = lines.clone();
    with_hole.insert(lines.len() - 1, torn.to_string());
    let judgment = task().judge(&observed(directory.path(), "torn", &with_hole.join("\n")));
    assert_eq!(judgment.verdict, Verdict::Unverifiable);
    assert_eq!(judgment.faults, vec![Fault::TornTrace { lines: 1 }]);
}
```

### docs/audits/2026-09-19-codebase-audit/calibration-wire.rs:8-54 — main

File SHA-256: `c58d6ead3dff8fce9dfd8ef5489267ec7f1ca918da6196c728b3c63b3ee86736`

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let map = Map::fit(&[Observation::new(0.8, false)], 1);
    let cases = [
        (
            "choice",
            Kind::Choice,
            vec![("yes", 0.8), ("no", 0.2)],
            "no",
        ),
        ("noul", Kind::Noul, vec![("no", 0.2), ("yes", 0.8)], "no"),
        (
            "score",
            Kind::Score,
            vec![("0", 0.8), ("1", 0.15), ("2", 0.05)],
            "1",
        ),
    ];
    for (name, kind, pairs, truth) in cases {
        let raw = pairs
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect();
        let selected = lev::estimator::argmax(&raw)?;
        let mapped = map.apply_distribution(&raw);
        let typed = lev::estimator::answer(kind, &mapped, &Default::default(), &selected)?;
        let bytes = serde_json::to_vec(&serde_json::json!({
            "model": "audit-local", "answers": {"q": typed}, "usage": {}
        }))?;
        let response = jev::SystemOneResponse::decode(jev::RawResponse {
            status: 200,
            headers: Default::default(),
            bytes,
        })?;
        let Disposition::Answered { chosen, .. } = read_answer(&response.answers["q"]) else {
            return Err("the served answer was not decoded as an answer".into());
        };
        let row = Row::new("audit", "audit", name, "audit").scored(raw, selected == truth);
        let observations = mapped_observations(&[row], &map);
        let observation = observations.first().ok_or("no mapped observation")?;
        println!(
            "kind={name} raw_selected={selected} mapped_metric_correct={} wire_selected={chosen} wire_correct={}",
            observation.correct,
            chosen == truth,
        );
    }
    Ok(())
}
```

### docs/audits/2026-09-19-codebase-audit/reproduce.rs:7-28 — streamed

File SHA-256: `03d69e2c3be756f64f157b838105c552325e59deaa3b07c21e82780293e89723`

```rust
async fn streamed(chunks: Vec<Vec<u8>>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0; 16384];
        let _ = stream.read(&mut buf).unwrap();
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").unwrap();
        for chunk in chunks {
            write!(stream, "{:x}\r\n", chunk.len()).unwrap();
            stream.write_all(&chunk).unwrap();
            stream.write_all(b"\r\n").unwrap();
            stream.flush().unwrap();
            std::thread::sleep(Duration::from_millis(150));
        }
        stream.write_all(b"0\r\n\r\n").unwrap();
    });
    let door = ResponsesDoor::new(format!("http://{addr}"), "audit", "unused-local-test");
    let result = door.generate("", &[], &mut |_| {}, &mut |_| {}).await;
    server.join().unwrap();
    format!("{result:?}")
}
```

### crates/coder/tests/program_run.rs:585-615 — a_bound_this_host_cannot_enforce_refuses_before_anything_runs

File SHA-256: `a2b709fec83667ad422d9cb4bf05f24ab34272f4b1ade78130ae0a76e6a534f8`

```rust
/// A step whose bounds this host cannot enforce does not run, and neither
/// does the program that carried it.
#[tokio::test]
async fn a_bound_this_host_cannot_enforce_refuses_before_anything_runs() {
    let machine = machine();
    let root = machine.path();
    let runtime = runtime(root).await;

    for (bounds, expected) in [
        // A shape nobody here can make. Running it in the shared
        // directory instead is the substitution the rule forbids.
        (json!({"isolation": "vm", "minutes": 60}), "vm checkout"),
        // A bound key this host keeps nothing for.
        (json!({"budget_cents": 500}), "cannot enforce budget_cents"),
        // A key it knows, carrying a value it cannot hold to.
        (json!({"concurrent_max": 0}), "count above zero"),
    ] {
        let program = program_with(root, bounds.clone());
        let refused = runtime
            .admit(&program)
            .expect_err(&format!("{bounds} is not enforceable here"));
        assert_eq!(refused.step, "fan_out");
        assert_eq!(refused.code, "bound_unenforceable");
        assert!(refused.reason.contains(expected), "{refused}");

        let run = runtime.run(&program, &inputs(), None).await;
        assert!(run.steps.is_empty(), "nothing ran: {:?}", run.step_names());
        assert!(run.delegations.is_empty());
        assert_eq!(run.stopped, Some(refused));
    }
}
```

### crates/gym/src/suite.rs:1567-1595 — tests::a_record_that_lost_its_newline_still_counts_and_the_next_append_repairs_it

File SHA-256: `b0e915d75384349e85ef4db685a9a2c26b5efb7c62e60269ada8b38073b5c422`

```rust
    #[test]
    fn a_record_that_lost_its_newline_still_counts_and_the_next_append_repairs_it() {
        // The other shape an interrupted write leaves: the record landed
        // whole and the newline did not. The record parses, so the spend
        // counts — reading past it would spend the partition twice.
        let suite = suite();
        let (_directory, ledger) = ledger();
        ledger.read_locked(&suite, &SPEND).expect("the first read");
        let text = fs::read_to_string(ledger.path()).expect("the ledger reads");
        fs::write(ledger.path(), text.trim_end_matches('\n')).expect("the newline is removed");

        assert!(
            matches!(
                ledger.read_locked(&suite, &SPEND),
                Err(SuiteError::AlreadyRead { .. })
            ),
            "a committed record counts whether or not its newline landed"
        );

        ledger
            .read_locked_again(&suite, &SPEND, "chris", "the newline never landed")
            .expect("an override still works");
        let repaired = fs::read_to_string(ledger.path()).expect("the ledger reads");
        assert!(
            repaired.ends_with('\n'),
            "the append repaired the line it would have joined"
        );
        assert_eq!(ledger.reads().expect("the ledger reads back").len(), 2);
    }
```

### crates/coderbench/src/lib.rs:169-262 — Grade

File SHA-256: `1fcf6ce2bf972972471a6dfeaf4c0a1c469b1b6183b211b515a483c51fa98178`

```rust
/// The path a correct run takes.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Grade {
    pub kind: String,
    /// The program the run is expected to select.
    #[serde(default)]
    pub program: String,
    /// How many delegations the run is expected to start.
    #[serde(default)]
    pub delegations: usize,
    /// How many of them are expected to answer correctly.
    ///
    /// A delegation counts here only when the trace says it completed, says
    /// it was correct, and holds the answer. One that recorded none of that
    /// is unverified, and the shortfall it leaves is
    /// [`Verdict::Unverifiable`] rather than a pass.
    ///
    /// When [`Grade::expects`] states the answers itself, a delegation
    /// counts when the recorded call is checked against the manifest's own
    /// expectation instead, and what the run asserted about itself adds
    /// nothing either way.
    #[serde(default)]
    pub delegations_correct: usize,
    /// The answers the task expects, one per delegation, in the order the
    /// request asks the questions.
    ///
    /// This is the manifest's own copy of the answers, so a run is checked
    /// against something other than what it said about itself: the runtime
    /// is never told them, and a `correct` flag the trace recorded is a
    /// claim rather than the check. Each entry pins the prompt that
    /// identifies the delegation and the output it owes, and the list is
    /// positional — a reordered, duplicated, or substituted delegation is
    /// not the delegation the task expects in its place.
    ///
    /// A non-empty `expects` pins every delegation the run owes, so its
    /// length is `delegations`, and every pinned answer must verify, so
    /// `delegations_correct` is the same count. Anything else is a
    /// malformed manifest rather than a passing grade: a prompt or answer
    /// that is blank, a prompt two entries share, or a count that
    /// disagrees fails [`Task::load`] and faults a [`Task::judge`] call on
    /// a task built by hand.
    ///
    /// A task that states no expectations keeps the trace-reported evidence rule:
    /// only a delegation the trace itself records as checked counts, and
    /// one that recorded nothing either way is [`Verdict::Unverifiable`].
    #[serde(default)]
    pub expects: Vec<ExpectedAnswer>,
    /// How many distinct workspace paths the run is expected to change.
    /// This includes directory and metadata changes; a rename names both
    /// its source and destination. Zero describes a read-only task.
    ///
    /// Judged against the workspace rather than against what the run said
    /// about itself. Absent `wrote` metadata is unknown, not proof.
    #[serde(default)]
    pub writes_expected: usize,
    /// The decisions the run is expected to ask a decision model, by name.
    #[serde(default)]
    pub decisions: Vec<String>,
    /// What those decisions have to answer.
    ///
    /// A decision that was asked and answered nothing did not go the right
    /// way, and a name in `decisions` cannot say which way it went. State
    /// the predicate the run gates on, and only that one: a predicate the
    /// run does not act on grades the door rather than the path.
    #[serde(default)]
    pub answers: Vec<Expected>,
    /// The deterministic checks the run is expected to run, by call name.
    ///
    /// Kept apart from `decisions` because the difference matters: a check
    /// is code and answers the same way every time, and a decision is a
    /// model and does not. A task that listed them together would accept a
    /// run that asked a model what a check should have settled.
    #[serde(default)]
    pub checks: Vec<String>,
    /// The steps a correct run takes, in the order it takes them.
    ///
    /// `decisions` and `checks` say which steps a run owes; this says when,
    /// and [`Task::judge`] holds the run to it. A run that admitted a
    /// delegation before it probed for the executor did the steps in an
    /// order that cannot establish what they establish.
    ///
    /// Faults are reported in this order too, so the first fault is the
    /// earliest thing that went wrong rather than the first thing the
    /// checker happened to test. A step this list does not name sorts last.
    #[serde(default)]
    pub path: Vec<String>,
    /// The endings this task allows, by the word [`Ending::word`] spells.
    ///
    /// Empty means `answered`. Stating it per task is what keeps a
    /// timed-out or declined run from grading clean because the partial
    /// trace it left holds the expected names.
    #[serde(default)]
    pub endings: Vec<String>,
}
```

### crates/coderbench/tests/harness.rs:287-323 — a_wrong_commit_refuses_before_the_run

File SHA-256: `f3dcaf1475e5f67b557600c43dfe9715b670d4d21ee858c29d7941dd708505be`

```rust
/// A requirement that does not hold refuses before Coder is started, names
/// the requirement, and leaves no trace behind — there was no run to
/// record.
#[test]
fn a_wrong_commit_refuses_before_the_run() {
    let directory = tempfile::tempdir().unwrap();
    let repository = repository(directory.path());
    let base = "0".repeat(40);
    let task = task_with(
        directory.path(),
        &format!(r#"{{"base": "{base}", "capabilities": ["nothing-describes-this"]}}"#),
        "elsewhere",
    );
    let coder = fake_coder(directory.path(), &golden(), 0);
    let trace = directory.path().join("run.atif.jsonl");

    let output = coderbench(&[
        "run",
        &task.display().to_string(),
        "--repository",
        &repository.display().to_string(),
        "--coder",
        &coder.display().to_string(),
        "--trace",
        &trace.display().to_string(),
    ]);

    let report = said(&output);
    assert_eq!(output.status.code(), Some(2), "{report}");
    assert!(report.contains("repository at 000000000000"), "{report}");
    assert!(
        report.contains("capability nothing-describes-this"),
        "{report}"
    );
    assert!(report.contains("Refused before starting Coder"), "{report}");
    assert!(!trace.exists(), "nothing ran, so nothing recorded");
}
```

### crates/atif/src/log.rs:448-469 — tests::a_half_written_line_does_not_cost_the_lines_before_it

File SHA-256: `68f164446044a59b4b8b1199253acdcedc2fec0f65f2e1e540636b3ea94104f6`

```rust
    /// A process killed in the middle of writing a line leaves a partial
    /// line behind. The lines before it are the record, and they read.
    #[test]
    fn a_half_written_line_does_not_cost_the_lines_before_it() {
        let dir = tempfile::tempdir().unwrap();
        let session = a_session();
        let mut log = Log::create(dir.path(), &session).unwrap();
        log.append(&Step::said(Source::User, "count the crates"))
            .unwrap();
        let path = log.path().to_path_buf();
        drop(log);
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(br#"{"record":"step","step":{"at":1,"sour"#)
            .unwrap();
        drop(file);

        let recording = read(&path).unwrap();
        assert_eq!(recording.steps.len(), 1);
        assert_eq!(recording.unreadable_lines, 1);
        assert_eq!(recording.session.state, INTERRUPTED);
        assert_eq!(recording.document()["extra"]["unreadable_lines"], 1);
    }
```

### crates/atif/src/document.rs:932-946 — tests::repeated_shell_commands_report_as_waste

File SHA-256: `133234ece9e73a75b3d5572eae57c1889b47eaa758db45e84b8eb3c206a6b4bb`

```rust
    /// A pipeline is what its first program does, and a repeated command is
    /// counted once as work and once as the cost of doing it again.
    #[test]
    fn repeated_shell_commands_report_as_waste() {
        let mut second = a_shell_call("cargo test -p atif", "ok");
        second.id = "call-2".to_string();
        let steps = vec![
            Step::called(a_shell_call("cargo test -p atif", "ok")),
            Step::called(second),
        ];
        let waste = &document(&a_session(), &steps)["final_metrics"]["extra"]["waste"];
        assert_eq!(waste["repeated_calls"], 1);
        assert_eq!(waste["repeated"][0]["executions"], 2);
        assert_eq!(waste["repeated"][0]["wasted_ms"], 12);
    }
```
