//! Reconstruct task state and page its original ATIF evidence without executing.
use super::*;
use serde_json::{Value, json};

pub const VIEW_SCHEMA: &str = "openagents.coder.task-view.v1";
pub const MAX_TRACE_BYTES: usize = 64 * 1024 * 1024;

/// A reconnect token binds the delivered prefix, not the writer's memory.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    pub schema: String,
    pub task_id: String,
    pub intent_digest: String,
    pub trace_file: String,
    pub next_step: usize,
    pub prefix_digest: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Evidence {
    pub state: String,
    pub trace_file: Option<String>,
    pub digest: Option<String>,
    pub matches_result: Option<bool>,
    pub faults: Vec<Value>,
    pub total_steps: usize,
    pub next: Option<Cursor>,
    pub more_available: bool,
    pub steps: Vec<Value>,
}

#[derive(Clone, Debug, Serialize)]
pub struct View {
    pub schema: String,
    pub task: Task,
    pub evidence: Evidence,
    pub artifacts: Option<artifact::Manifest>,
    pub artifact_error: Option<String>,
    pub artifact_faults: Vec<String>,
    pub verification: String,
    pub integration: String,
    pub cost_usd: Option<f64>,
    pub cost_status: String,
}

fn unavailable(state: &str, trace_file: Option<String>) -> Evidence {
    Evidence {
        state: state.into(),
        trace_file,
        digest: None,
        matches_result: None,
        faults: Vec::new(),
        total_steps: 0,
        next: None,
        more_available: false,
        steps: Vec::new(),
    }
}

/// Read local evidence. Unknown, missing, damaged, or unsealed evidence never
/// changes the authoritative task state or supplies a verification verdict.
pub fn read(
    directory: &Path,
    task_id: &str,
    cursor: Option<&Cursor>,
    limit: usize,
) -> Result<View, Error> {
    if !(1..=200).contains(&limit) {
        return Err(Error::InvalidCommand(
            "view limit must be between 1 and 200",
        ));
    }
    let store = Store::open(directory)?;
    let task = store.show(task_id)?;
    let (artifacts, artifact_error) = match artifact::manifest(&store.dir, &task) {
        Ok(manifest) => (manifest, None),
        Err(error) => (None, Some(error.to_string())),
    };
    let artifact_faults = artifacts
        .as_ref()
        .map_or_else(Vec::new, |manifest| artifact::faults(&store.dir, manifest));
    let path = task
        .run
        .as_ref()
        .map(|run| store.dir.join(&run.admission.trace_file));
    // Capture the identity under the journal lock, then release it before reading
    // the potentially long trace. Any newer result appears on the next read.
    drop(store);
    let evidence = match path {
        None if cursor.is_some() => return Err(Error::Conflict),
        None => unavailable("not_started", None),
        Some(path) => read_trace(&task, &path, cursor, limit)?,
    };
    let verification = match task.checks {
        Checks::NotRun => "not_run",
        Checks::Running => "running",
        Checks::Passed => "passed",
        Checks::Failed => "failed",
        Checks::Unavailable => "unavailable",
        Checks::Disputed => "disputed",
    }
    .into();
    Ok(View {
        schema: VIEW_SCHEMA.into(),
        task,
        evidence,
        artifacts,
        artifact_error,
        artifact_faults,
        verification,
        integration: "not_attempted".into(),
        cost_usd: None,
        cost_status: "unknown".into(),
    })
}

fn read_trace(
    task: &Task,
    path: &Path,
    cursor: Option<&Cursor>,
    limit: usize,
) -> Result<Evidence, Error> {
    let run = task.run.as_ref().ok_or(Error::InvalidTransition)?;
    let filename = run.admission.trace_file.clone();
    let file = match private_open(path, false, false) {
        Ok(file) => file,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(unavailable("missing", Some(filename)));
        }
        Err(_) => return Ok(unavailable("unavailable_or_unsafe", Some(filename))),
    };
    let mut bytes = Vec::new();
    if file
        .take(MAX_TRACE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Ok(unavailable("unreadable", Some(filename)));
    }
    if bytes.len() > MAX_TRACE_BYTES {
        return Ok(unavailable("oversized", Some(filename)));
    }
    let recording = match atif::log::read_bytes(path, &bytes) {
        Ok(recording) => recording,
        Err(_) => return Ok(unavailable("malformed", Some(filename))),
    };
    let digest = digest_bytes(&bytes);
    let matches_result = run
        .result
        .as_ref()
        .map(|result| result.trace_digest == digest);
    let document = recording.document();
    let steps = document["steps"]
        .as_array()
        .ok_or(Error::Corrupt("ATIF has no steps"))?;
    let start = cursor.map_or(0, |cursor| cursor.next_step);
    if let Some(cursor) = cursor
        && (cursor.schema != VIEW_SCHEMA
            || cursor.task_id != task.task_id
            || cursor.intent_digest != task.intent_digest
            || cursor.trace_file != filename
            || start > steps.len()
            || cursor.prefix_digest != prefix(task, &filename, &steps[..start])?)
    {
        return Err(Error::Conflict);
    }
    let end = steps.len().min(start + limit);
    let next = Cursor {
        schema: VIEW_SCHEMA.into(),
        task_id: task.task_id.clone(),
        intent_digest: task.intent_digest.clone(),
        trace_file: filename.clone(),
        next_step: end,
        prefix_digest: prefix(task, &filename, &steps[..end])?,
    };
    let faults = recording
        .faults
        .iter()
        .map(|fault| json!({"line":fault.line,"kind":fault.kind.word()}))
        .collect();
    let state = if matches_result == Some(false) {
        "digest_mismatch"
    } else if !recording.faults.is_empty() {
        "damaged"
    } else if !recording.whole() {
        "incomplete"
    } else if matches_result == Some(true) {
        "sealed"
    } else {
        "unsealed"
    };
    Ok(Evidence {
        state: state.into(),
        trace_file: Some(filename),
        digest: Some(digest),
        matches_result,
        faults,
        total_steps: steps.len(),
        more_available: end < steps.len(),
        next: Some(next),
        steps: steps[start..end].to_vec(),
    })
}

fn prefix(task: &Task, filename: &str, steps: &[Value]) -> Result<String, Error> {
    serde_json::to_vec(
        &json!({"task":task.task_id,"intent":task.intent_digest,"trace":filename,"steps":steps}),
    )
    .map(|bytes| digest_bytes(&bytes))
    .map_err(|_| Error::Corrupt("the transcript prefix cannot be encoded"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use atif::{Log, Session, Source, Step};

    fn recorded() -> (tempfile::TempDir, Task, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task.1.atif.jsonl");
        let mut log = Log::create_at(
            &path,
            &Session::opening("task-1", "none", "fixture", "/workspace", "1"),
        )
        .unwrap();
        for n in 0..2000 {
            log.append(&Step::said(
                Source::Agent,
                &format!("Full step {n}: **markdown**\n```rust\nlet x = {n};\n```"),
            ))
            .unwrap();
        }
        log.finish(atif::log::ENDED).unwrap();
        let grant = owner::Grant {
            adapter_configuration: None,
            schema: owner::GRANT_SCHEMA.into(),
            task_id: "task".into(),
            intent_digest: "0".repeat(64),
            expected_revision: 1,
            program: "/bin/sh".into(),
            arguments: vec![],
            write_workspace: false,
            wall_seconds: 1,
            stream_bytes: 1024,
            memory_bytes: 67108864,
            requirements: None,
            expected_source_snapshot: None,
        };
        let task = Task {
            task_id: "task".into(),
            revision: 4,
            intent: TaskIntent {
                title: "Fixture".into(),
                prompt: "Fixture".into(),
                workspace: Workspace {
                    path: "/workspace".into(),
                    source_revision: None,
                },
                configuration: RequestedConfiguration {
                    adapter: "bounded-command".into(),
                    model: None,
                },
            },
            intent_digest: "0".repeat(64),
            status: Status::Running,
            execution: Execution::Running,
            checks: Checks::NotRun,
            cancellation_reason: None,
            corrections: Vec::new(),
            run: Some(owner::Run {
                epoch: 1,
                admission: owner::Admission {
                    grant,
                    grant_digest: String::new(),
                    grant_request: String::new(),
                    workspace: "/workspace".into(),
                    source_revision: String::new(),
                    source_snapshot: String::new(),
                    program_digest: String::new(),
                    adapter: "bounded-command".into(),
                    network: String::new(),
                    read_scope: String::new(),
                    authority: String::new(),
                    trace_file: "task.1.atif.jsonl".into(),
                    context: checks::Context {
                        schema: String::new(),
                        task_revision: 1,
                        prompt: String::new(),
                        instructions: Vec::new(),
                        suites: Vec::new(),
                        lineage: checks::Lineage::default(),
                        knowledge: Vec::new(),
                        digest: String::new(),
                    },
                },
                effect_id: None,
                result: None,
                process_id: None,
                recovery_reason: None,
                check_report: None,
            }),
        };
        (dir, task, path)
    }

    #[test]
    fn pages_two_thousand_full_steps_and_validates_a_reconnect_prefix() {
        let (_dir, task, path) = recorded();
        let mut cursor = None;
        let mut messages = Vec::new();
        loop {
            let page = read_trace(&task, &path, cursor.as_ref(), 73).unwrap();
            assert_eq!(page.state, "unsealed");
            messages.extend(
                page.steps
                    .iter()
                    .map(|step| step["message"].as_str().unwrap().to_string()),
            );
            cursor = page.next;
            if !page.more_available {
                break;
            }
        }
        assert_eq!(messages.len(), 2000);
        assert!(messages[1999].contains("let x = 1999;"));
        assert!(
            read_trace(&task, &path, cursor.as_ref(), 73)
                .unwrap()
                .steps
                .is_empty()
        );
        cursor.as_mut().unwrap().intent_digest = "changed".into();
        assert!(matches!(
            read_trace(&task, &path, cursor.as_ref(), 73),
            Err(Error::Conflict)
        ));
    }

    #[test]
    fn missing_corrupt_changed_and_torn_evidence_remain_visible() {
        let (_dir, task, path) = recorded();
        let first = read_trace(&task, &path, None, 1).unwrap();
        let original = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, original.replace("Full step 0", "Changed step 0")).unwrap();
        assert!(read_trace(&task, &path, first.next.as_ref(), 1).is_err());
        std::fs::write(&path, format!("{original}{{\"torn\":")).unwrap();
        let damaged = read_trace(&task, &path, None, 2).unwrap();
        assert_eq!(damaged.state, "damaged");
        assert_eq!(damaged.total_steps, 2000);
        assert!(!damaged.faults.is_empty());
        std::fs::write(&path, "not a log\n").unwrap();
        assert_eq!(
            read_trace(&task, &path, None, 2).unwrap().state,
            "malformed"
        );
        std::fs::remove_file(&path).unwrap();
        assert_eq!(read_trace(&task, &path, None, 2).unwrap().state, "missing");
    }
}
