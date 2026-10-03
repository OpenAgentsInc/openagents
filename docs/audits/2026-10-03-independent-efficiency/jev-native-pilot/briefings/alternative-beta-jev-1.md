## Prepared source spans

Source commit: `7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6`. Complete applicable instructions are supplied separately. Partial evidence and omissions are explicit; a selected span does not prove requirement or dependency coverage.

### s81 docs/coder/traces.md:1-69 — docs/coder/traces.md

document; partial_file. Full span: 1-200. Blob: `85914b6b156f297fde9f1ca85344edd1542a2a50`. File SHA-256: `b7570fc3dd4c74d377ad003e7a8c8d49e4901f620feb6cb2737db63c645390ca`.

````text
# Traces

Every Coder Terminal conversation writes itself to a local ATIF trace as
it runs. Nothing uploads, nothing phones home, and nothing has to be
asked for: a conversation on your machine leaves a file on your machine.

Status: implemented in `crates/atif` (the format and the log) and
`crates/coder` (`trace.rs`, wired through `agent.rs` and `main.rs`).

## Why this exists

`crates/coder` used to persist nothing. The transcript lived in `Agent`
for the life of the process and went when the terminal exited. So when
[issue #9379](https://github.com/OpenAgentsInc/openagents/issues/9379)
set out to score `classify.rs` on real `coder` turns, there were none to
score, and the suite had to be harvested from a different agent's session
record instead — 95 states measuring something adjacent to `coder` rather
than `coder`. Every measurement this repository has made rests on suites
written for the purpose. The one workload this repository owns was the
one it could not see.

A trace is the other half of the Gym's picture:

- **Rows** are per-decision, receipt-chained, and comparable across
  doors. They say what one door answered for one state.
- **Traces** are per-episode and ordered. They say what happened *next*,
  which is what an outcome label is derived from.

## Where the traces go

```text
~/.openagents/traces/<session>.atif.jsonl
```

One file per session, the directory and the files readable only by you.
The name leads with the session's UTC start time, so listing the
directory lists your history:

```text
20260919T142233Z-4f1a9c02.atif.jsonl
```

Three settings change that:

| Setting | Effect |
| --- | --- |
| `CODER_TRACE_DIR=<path>` | Write traces to `<path>` instead. |
| `CODER_TRACE=off` | Record nothing. `0`, `no`, and `false` also work. |
| `--trace <path>` | Write this session to that file. |

With `HOME` unset and no `CODER_TRACE_DIR`, there is nowhere to write and
recording is off.

`--trace` is the one a script wants: it names the file, so a caller reads
the trace back without globbing a directory. Naming a file is a request to
record, so it outranks `CODER_TRACE=off`, and a file that cannot be opened
ends the run rather than producing an unrecorded session. The flag works
in both modes — see [`headless.md`](headless.md).

The terminal says which of those it is on the first detail line of a
session, so you never have to guess. Press `⌥V` or type `/verbose` to see
it.

## What is recorded

| Step | When |
| --- | --- |
| `user` | You submit a draft. |
| `system` | The instructions a generation was given, recorded when they change rather than once a turn; and any note about what the host could not do, such as a missing `TYPESAFE_API_KEY`. |
````

### s06 crates/atif/src/log.rs:225-306 — read

implementation; complete_declaration. Full span: 231-306. Blob: `07376e31683e0b2c99caa5853cdf1cfd0aa31ee0`. File SHA-256: `68f164446044a59b4b8b1199253acdcedc2fec0f65f2e1e540636b3ea94104f6`.

```text
/// Reads a session log.
///
/// # Errors
///
/// Returns the underlying filesystem error, or an `InvalidData` error when
/// the file holds no session record and so is not a session log.
pub fn read(path: &Path) -> io::Result<Recording> {
    let file = File::open(path)?;
    let mut opened: Option<(u64, Session)> = None;
    let mut steps: Vec<Step> = Vec::new();
    let mut closed: Option<(u64, String)> = None;
    let mut unreadable_lines = 0usize;
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Map<String, Value>>(&line) else {
            unreadable_lines += 1;
            continue;
        };
        let at = record.get("at").and_then(Value::as_u64).unwrap_or_default();
        match record.get("record").and_then(Value::as_str) {
            Some("session") => {
                match record
                    .get("session")
                    .cloned()
                    .and_then(|value| serde_json::from_value::<Session>(value).ok())
                {
                    Some(session) => opened = Some((at, session)),
                    None => unreadable_lines += 1,
                }
            }
            Some("step") => match record
                .get("step")
                .cloned()
                .and_then(|value| serde_json::from_value::<Step>(value).ok())
            {
                Some(step) => steps.push(step),
                None => unreadable_lines += 1,
            },
            Some("end") => {
                let state = record
                    .get("state")
                    .and_then(Value::as_str)
                    .unwrap_or(ENDED)
                    .to_string();
                closed = Some((at, state));
            }
            _ => unreadable_lines += 1,
        }
    }
    let Some((started, mut session)) = opened else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} holds no session record", path.display()),
        ));
    };
    let last = closed
        .as_ref()
        .map(|(at, _)| *at)
        .or_else(|| steps.last().map(|step| step.at))
        .unwrap_or(started);
    session.seconds = last.saturating_sub(started) / 1_000;
    session.state = match &closed {
        Some((_, state)) => state.clone(),
        None => INTERRUPTED.to_string(),
    };
    if session.directive.is_empty() {
        session.directive = steps
            .iter()
            .find(|step| step.source == document::Source::User)
            .map(|step| step.message.clone())
            .unwrap_or_default();
    }
    Ok(Recording {
        session,
        steps,
        unreadable_lines,
        path: path.to_path_buf(),
    })
}
```

### s41 crates/coderbench/src/lib.rs:1267-1290 — Task::judge_record

implementation; complete_declaration. Full span: 1269-1290. Blob: `fc62b67bb082bad555b56b92dffbc2a196994f40`. File SHA-256: `1fcf6ce2bf972972471a6dfeaf4c0a1c469b1b6183b211b515a483c51fa98178`.

```text
    /// Whether the record is whole, and whether the episode ended a way the
    /// task allows.
    fn judge_record(&self, run: &Observed, faults: &mut Vec<Fault>) {
        if !run.closed {
            faults.push(Fault::Unfinished);
        }
        if run.unreadable_lines > 0 {
            faults.push(Fault::TornTrace {
                lines: run.unreadable_lines,
            });
        }
        let allowed = self.grade.allowed();
        if allowed.iter().any(|word| word == run.ending.word()) {
            return;
        }
        match &run.ending {
            Ending::Unobserved => faults.push(Fault::EndingUnobserved { allowed }),
            Ending::Closed => faults.push(Fault::EndingUnstated { allowed }),
            found => faults.push(Fault::Ended {
                found: found.word().to_string(),
                allowed,
            }),
        }
    }
```

### s65 crates/atif/src/document.rs:846-878 — tests::a_decision_call_records_the_door_and_the_state_it_read

test; complete_declaration. Full span: 849-878. Blob: `7e7b759c279e51ec96d2dab456cb82fb5490eaef`. File SHA-256: `133234ece9e73a75b3d5572eae57c1889b47eaa758db45e84b8eb3c206a6b4bb`.

```text
    /// A decision call records which door answered, what it was asked, what
    /// it said, and the digest of the state it read.
    #[test]
    fn a_decision_call_records_the_door_and_the_state_it_read() {
        let state = json!({"task": "list the crates", "transcript": []});
        let request = json!({
            "state": state,
            "model": "jev-latest",
            "questions": {"action": {"type": "choice"}, "risk": {"type": "score"}},
        });
        let call = Decision {
            id: "call-2".to_string(),
            name: "classify".to_string(),
            door: "https://api.typesafe.ai".to_string(),
            model: "jev-latest".to_string(),
            request,
            answers: json!({"action": {"choice": "respond", "confidence": 0.8}}),
            route: Some("respond".to_string()),
            error: None,
            milliseconds: 240,
        }
        .call();
        assert!(call.is_decision());
        assert_eq!(call.extra["door"], "https://api.typesafe.ai");
        assert_eq!(call.extra["state_digest"], digest(&state));
        assert_eq!(call.extra["question_ids"], json!(["action", "risk"]));
        assert_eq!(call.extra["route"], "respond");
        assert_eq!(call.extra["answers"]["action"]["choice"], "respond");
        assert_eq!(call.outcome, Outcome::Completed);
        // The arguments hold the state, so the question and the answer read
        // back together without a second file.
        assert_eq!(call.arguments["state"]["task"], "list the crates");
    }
```

### s66 crates/atif/src/log.rs:471-480 — tests::a_file_with_no_session_record_is_refused

test; complete_declaration. Full span: 474-480. Blob: `07376e31683e0b2c99caa5853cdf1cfd0aa31ee0`. File SHA-256: `68f164446044a59b4b8b1199253acdcedc2fec0f65f2e1e540636b3ea94104f6`.

```text
    /// A file that is not a session log says so rather than reading back as
    /// an empty session.
    #[test]
    fn a_file_with_no_session_record_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-trace.atif.jsonl");
        fs::write(&path, "{\"record\":\"step\",\"step\":{}}\n").unwrap();
        let error = read(&path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
```

### s67 crates/coderbench/tests/negative.rs:193-207 — a_torn_trace_is_unverifiable

test; complete_declaration. Full span: 196-207. Blob: `0c9dd910b0753d02ed9ae3ae73ef94fdb9e5e0c2`. File SHA-256: `c4d2e8ffb6f319f4c84834217f4df7249f5278648105dac3e789df47da165976`.

```text
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

### s76 crates/atif/src/document.rs:932-946 — tests::repeated_shell_commands_report_as_waste

test; complete_declaration. Full span: 935-946. Blob: `7e7b759c279e51ec96d2dab456cb82fb5490eaef`. File SHA-256: `133234ece9e73a75b3d5572eae57c1889b47eaa758db45e84b8eb3c206a6b4bb`.

```text
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

### s77 crates/atif/src/log.rs:512-518 — tests::a_session_id_leads_with_its_start_time

test; complete_declaration. Full span: 514-518. Blob: `07376e31683e0b2c99caa5853cdf1cfd0aa31ee0`. File SHA-256: `68f164446044a59b4b8b1199253acdcedc2fec0f65f2e1e540636b3ea94104f6`.

```text
    /// A session identifier leads with the time it started.
    #[test]
    fn a_session_id_leads_with_its_start_time() {
        let id = session_id(1_758_290_553_123);
        assert!(id.starts_with("20250919T140233Z-"), "{id}");
        assert_eq!(id.len(), "20250919T140233Z-".len() + 8);
    }
```

### s78 crates/coderbench/tests/negative.rs:209-221 — a_trace_without_an_end_record_is_a_fault

test; complete_declaration. Full span: 212-221. Blob: `0c9dd910b0753d02ed9ae3ae73ef94fdb9e5e0c2`. File SHA-256: `c4d2e8ffb6f319f4c84834217f4df7249f5278648105dac3e789df47da165976`.

```text
/// A trace with no end record is a session that never closed, whatever the
/// steps in it say.
#[test]
fn a_trace_without_an_end_record_is_a_fault() {
    let directory = tempfile::tempdir().unwrap();
    let lines: Vec<String> = authored_text().lines().map(str::to_string).collect();
    let stopped = lines[..lines.len() - 1].join("\n");
    let run = observed(directory.path(), "stopped", &stopped);
    assert!(!run.closed);
    let judgment = task().judge(&run);
    assert_eq!(judgment.verdict, Verdict::Failed);
    assert_eq!(judgment.faults, vec![Fault::Unfinished]);
}
```

### s01 crates/atif/src/document.rs:320-334 — Session::opening

implementation; complete_declaration. Full span: 323-334. Blob: `7e7b759c279e51ec96d2dab456cb82fb5490eaef`. File SHA-256: `133234ece9e73a75b3d5572eae57c1889b47eaa758db45e84b8eb3c206a6b4bb`.

```text
    /// A session with the fields that are known when it opens, and the
    /// rest left for the reader to compute.
    #[must_use]
    pub fn opening(id: &str, model: &str, door: &str, repository: &str, version: &str) -> Self {
        Session {
            id: id.to_string(),
            model: model.to_string(),
            door: door.to_string(),
            repository: repository.to_string(),
            directive: String::new(),
            state: String::new(),
            seconds: 0,
            version: version.to_string(),
        }
    }
```

### s02 crates/atif/src/log.rs:144-155 — Log::append

implementation; complete_declaration. Full span: 151-155. Blob: `07376e31683e0b2c99caa5853cdf1cfd0aa31ee0`. File SHA-256: `68f164446044a59b4b8b1199253acdcedc2fec0f65f2e1e540636b3ea94104f6`.

```text
    /// Appends one step and syncs it to disk.
    ///
    /// # Errors
    ///
    /// Returns the underlying filesystem error. A caller that cannot record
    /// should say so and carry on: a trace is evidence about a conversation,
    /// not a part of it.
    pub fn append(&mut self, step: &Step) -> io::Result<()> {
        write_line(&mut self.file, &json!({ "record": "step", "step": step }))?;
        self.steps += 1;
        Ok(())
    }
```

### s07 crates/atif/src/log.rs:340-354 — session_id

implementation; complete_declaration. Full span: 347-354. Blob: `07376e31683e0b2c99caa5853cdf1cfd0aa31ee0`. File SHA-256: `68f164446044a59b4b8b1199253acdcedc2fec0f65f2e1e540636b3ea94104f6`.

```text
/// A session identifier: the UTC start time, then eight hex digits that
/// separate two sessions started in the same second.
///
/// The time leads so that a directory listing is a history, and the file
/// name is the identifier so that a document and the file it came from
/// cannot drift apart.
#[must_use]
pub fn session_id(at: u64) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.subsec_nanos())
        .unwrap_or_default();
    let salt = u64::from(nanos) ^ (u64::from(std::process::id()) << 19);
    format!("{}-{:08x}", document::stamp(at), salt as u32)
}
```
