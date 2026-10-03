## Prepared source evidence

Source commit: `7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6`. Selections may omit requirements, dependencies, fixtures, and callers. Partial units are labeled; inspect their full spans before relying on completeness. Complete applicable instructions are supplied separately.

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

### c01 crates/coderbench/src/lib.rs:169-262 — Grade

Role: implementation; complete_declaration. Full unit: 169-262. Blob: `fc62b67bb082bad555b56b92dffbc2a196994f40`. File SHA-256: `1fcf6ce2bf972972471a6dfeaf4c0a1c469b1b6183b211b515a483c51fa98178`.

```text
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

### c05 crates/atif/src/document.rs:932-946 — tests::repeated_shell_commands_report_as_waste

Role: test; complete_declaration. Full unit: 932-946. Blob: `7e7b759c279e51ec96d2dab456cb82fb5490eaef`. File SHA-256: `133234ece9e73a75b3d5572eae57c1889b47eaa758db45e84b8eb3c206a6b4bb`.

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
