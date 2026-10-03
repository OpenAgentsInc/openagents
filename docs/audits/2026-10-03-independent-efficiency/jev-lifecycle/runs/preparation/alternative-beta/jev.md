## Prepared source evidence

Source commit: `7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6`. Selections may omit requirements, dependencies, fixtures, and callers. Partial units are labeled; inspect their full spans before relying on completeness. Complete applicable instructions are supplied separately.

### c08 crates/atif/src/log.rs:225-306 — read

Role: implementation; complete_declaration. Full unit: 225-306. Blob: `07376e31683e0b2c99caa5853cdf1cfd0aa31ee0`. File SHA-256: `68f164446044a59b4b8b1199253acdcedc2fec0f65f2e1e540636b3ea94104f6`.

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

### c02 crates/coderbench/tests/negative.rs:193-207 — a_torn_trace_is_unverifiable

Role: test; complete_declaration. Full unit: 193-207. Blob: `0c9dd910b0753d02ed9ae3ae73ef94fdb9e5e0c2`. File SHA-256: `c4d2e8ffb6f319f4c84834217f4df7249f5278648105dac3e789df47da165976`.

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

### c06 crates/coderbench/src/main.rs:308-417 — report

Role: implementation; complete_declaration. Full unit: 308-417. Blob: `d71a6573f74c8a1357926462f0e37d1aab1f2358`. File SHA-256: `bb070df5ec15a7a01c03f5657e92f44d78fb3850bb3071fb585d92e823af389e`.

```text
/// Prints what the run did, then every way it left the path.
fn report(task: &Task, run: &Observed, trace: &Path) -> u8 {
    // What counted a delegation depends on who owns the answers. A task
    // that states them checks the recorded calls itself; a task that does
    // not can only count what the run recorded as checked.
    let (verified, proof) = match task.grade.expects.is_empty() {
        true => (
            run.delegations
                .iter()
                .filter(|delegation| delegation.verified())
                .count(),
            "verified correct",
        ),
        false => (
            run.delegations
                .iter()
                .zip(&task.grade.expects)
                .filter(|(delegation, want)| delegation.verified_against(want))
                .count(),
            "verified against the task's expected answers",
        ),
    };
    println!();
    println!("What the trace holds:");
    println!("  trace          {}", trace.display());
    println!(
        "  program        {}",
        run.program.as_deref().unwrap_or("none selected")
    );
    println!(
        "  decisions      {}",
        if run.decisions.is_empty() {
            "none".to_string()
        } else {
            run.decisions.keys().cloned().collect::<Vec<_>>().join(", ")
        }
    );
    println!(
        "  checks         {}",
        if run.checks.is_empty() {
            "none".to_string()
        } else {
            run.checks
                .iter()
                .map(|check| check.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    println!(
        "  delegations    {} started, {verified} {proof}",
        run.delegations.len()
    );
    println!(
        "  writes         {}",
        match &run.workspace {
            Some(workspace) if workspace.changed.is_empty() =>
                "the workspace is unchanged".to_string(),
            Some(workspace) => format!(
                "{} changed in the workspace: {}",
                workspace.changed.len(),
                workspace.changed.join(", ")
            ),
            None => format!(
                "{} self-reported, and nobody looked at the workspace",
                run.writes.len()
            ),
        }
    );
    println!(
        "  ended          {}{}",
        run.ending,
        if run.closed {
            String::new()
        } else {
            ", with no end record".to_string()
        }
    );
    if run.unreadable_lines > 0 {
        println!("  unreadable     {} lines", run.unreadable_lines);
    }

    let judgment = task.judge(run);
    println!();
    if judgment.faults.is_empty() {
        println!("No faults. The run took the path {} expects.", task.id);
        return EXIT_CLEAN;
    }
    println!(
        "{}: {} fault{}, in the order the path takes:",
        judgment.verdict,
        judgment.faults.len(),
        if judgment.faults.len() == 1 { "" } else { "s" }
    );
    for (index, fault) in judgment.faults.iter().enumerate() {
        println!("  {:>2}. [{}] {}", index + 1, fault.verdict(), fault);
    }
    if judgment.verdict == Verdict::Unverifiable {
        println!();
        println!(
            "Nothing here says the run left the path. It says the evidence to show it \
             took the path is missing, which is not a pass."
        );
    }
    match judgment.verdict {
        Verdict::Passed => EXIT_CLEAN,
        Verdict::Unverifiable => EXIT_UNVERIFIABLE,
        Verdict::Failed => EXIT_FAULTS,
    }
}
```

### c09 crates/coderbench/tests/devin_fan_out_six.rs:39-69 — a_trace_alone_is_unverifiable

Role: test; complete_declaration. Full unit: 39-69. Blob: `058e01de198b6f9f5b8a2284cde4a44c65d814da`. File SHA-256: `7c5b27538e9dc929bd579650460a17d6474d2963cc2293a8f4df6c17e9e9186c`.

```text
/// A trace on its own cannot pass. It holds the path, and it does not hold
/// the exit code or the workspace, so the two faults left are about the
/// evidence rather than about the run. The trace here is the authored
/// fixture — the golden rewritten to the calls a sentence-driven run is
/// expected to make — so the delegation answers are the manifest's check
/// rather than a claim the recording makes about itself.
#[test]
fn a_trace_alone_is_unverifiable() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authored.atif.jsonl");
    std::fs::write(&path, authored_text()).unwrap();
    let run = observe(&path).expect("the rewritten golden still reads");
    let task = task();
    for (delegation, want) in run.delegations.iter().zip(&task.grade.expects) {
        assert_eq!(delegation.prompt, want.prompt);
        assert_eq!(delegation.correct, None);
    }
    let judgment = task.judge(&run);
    assert_eq!(judgment.verdict, Verdict::Unverifiable);
    let said: Vec<String> = judgment.faults.iter().map(ToString::to_string).collect();
    assert_eq!(
        said,
        vec![
            "nothing compared the workspace, so writing nothing is unobserved rather than shown"
                .to_string(),
            "the trace closed without saying how the episode ended; the task allows answered"
                .to_string(),
        ],
        "a trace is missing exactly the two things only a driver sees"
    );
}
```

### c07 crates/atif/src/log.rs:471-480 — tests::a_file_with_no_session_record_is_refused

Role: test; complete_declaration. Full unit: 471-480. Blob: `07376e31683e0b2c99caa5853cdf1cfd0aa31ee0`. File SHA-256: `68f164446044a59b4b8b1199253acdcedc2fec0f65f2e1e540636b3ea94104f6`.

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

### c03 docs/coder/traces.md:30-82 — docs/coder/traces.md

Role: document; partial_file. Full unit: 1-200. Blob: `85914b6b156f297fde9f1ca85344edd1542a2a50`. File SHA-256: `b7570fc3dd4c74d377ad003e7a8c8d49e4901f620feb6cb2737db63c645390ca`.

````text

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
| `agent` | Every reply the model produced, with the turn's token counts, wall time, and the model that produced it. A plan is a reply, and it is recorded verbatim. |
| `agent` with a `shell` call | Every command, its working directory, its whole output, its exit status, and how long it took. |
| `agent` with a decision call | Every `classify` and every `shell_judge`: which door answered, the state and questions that went out, the typed answers that came back, the digest of that state, and the route the table made of it. |

A decision call carries `schema: openagents.decision-call.v1` in its
`extra`, so a reader that wants questions put to a door and not commands
run on a machine separates them on one field. That is the point of using
ATIF rather than a format written here: a Jev, Kev, or Lev call is not a
foreign object in it.

### The step names the model, when the session header cannot

A session header carries the model the door serves, and it is written when
````

### c04 crates/atif/src/document.rs:288-317 — Session

Role: implementation; complete_declaration. Full unit: 288-317. Blob: `7e7b759c279e51ec96d2dab456cb82fb5490eaef`. File SHA-256: `133234ece9e73a75b3d5572eae57c1889b47eaa758db45e84b8eb3c206a6b4bb`.

```text
/// What a document says about the session as a whole.
///
/// The fields a session knows when it opens — its identity, its door, where
/// it is running — are set once. The fields that are only true at the end —
/// how it ended and how long it took — are computed by [`crate::log::read`]
/// from the steps, so a session that is killed still reports them.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    /// Unique per session, and the stem of the file the session writes to.
    pub id: String,
    /// The model the Generate door serves, as the session knows it at the
    /// start. A door that forwards to a worker does not know it then, and
    /// says so; the answer steps carry what answered.
    pub model: String,
    /// Which Generate door the session's turns went through.
    pub door: String,
    /// Where the session was running, as the agent knows it.
    pub repository: String,
    /// What the session was first asked to do.
    #[serde(default)]
    pub directive: String,
    /// How the session ended: `ended` or `interrupted`.
    #[serde(default)]
    pub state: String,
    /// Wall time from the first record to the last.
    #[serde(default)]
    pub seconds: u64,
    /// The version of the agent that wrote it.
    pub version: String,
}
```

### c11 crates/coderbench/src/lib.rs:265-274 — Grade::allowed

Role: implementation; complete_declaration. Full unit: 265-274. Blob: `fc62b67bb082bad555b56b92dffbc2a196994f40`. File SHA-256: `1fcf6ce2bf972972471a6dfeaf4c0a1c469b1b6183b211b515a483c51fa98178`.

```text
    /// The endings this task allows, which is `answered` when it says
    /// nothing.
    #[must_use]
    pub fn allowed(&self) -> Vec<String> {
        if self.endings.is_empty() {
            vec![Ending::Answered.word().to_string()]
        } else {
            self.endings.clone()
        }
    }
```
