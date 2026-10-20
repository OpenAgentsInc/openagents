//! Read the resident owner's retained records without opening a writable store.

use std::io::Read;

use base64::{Engine, engine::general_purpose::STANDARD};
use coder_host::{Code, access::task_read as wire};
use serde::Serialize;
use serde_json::Value;

use super::Inbox;
use crate::task::{self, Error, Task, artifact};

fn checked<T>(value: coder_host::access::Result<T>) -> Result<T, Code> {
    value.map_err(|error| error.code)
}

fn snapshot(inbox: &Inbox, workspace: &str, id: &str) -> Result<(Task, Vec<u8>), Code> {
    if !inbox.workspaces.contains_key(workspace) {
        return Err(Code::Forbidden);
    }
    let (task, bytes) = task::retained_snapshot(&inbox.store, id).map_err(super::refusal)?;
    if label(inbox, &task)?.as_deref() != Some(workspace) {
        return Err(Code::Forbidden);
    }
    Ok((task, bytes))
}

fn label(inbox: &Inbox, task: &Task) -> Result<Option<String>, Code> {
    if let Some(label) = task::commands::label_for(&inbox.workspaces, task) {
        return Ok(Some(label.into()));
    }
    // Studio works in a worktree. Only its retained, task-bound local record
    // may map that worktree back to an admitted checkout.
    let directory = inbox.store.join("local");
    match std::fs::symlink_metadata(&directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Code::Unavailable),
        Ok(_) => task::verify_directory(&directory).map_err(super::refusal)?,
    }
    let bytes = match read(
        &directory.join(format!("{}.json", task.task_id)),
        task::MAX_STORE_BYTES as u64,
    ) {
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        other => other.map_err(super::refusal)?,
    };
    let value =
        task::parse_strict_bounded(&bytes, task::MAX_STORE_BYTES).map_err(|_| Code::Unavailable)?;
    let record: task::local::Record =
        serde_json::from_value(value).map_err(|_| Code::Unavailable)?;
    if record.schema != task::local::RECORD_SCHEMA
        || record.task != task.task_id
        || record.worktree != task.intent.workspace.path
    {
        return Err(Code::Unavailable);
    }
    Ok(inbox
        .workspaces
        .iter()
        .find(|(_, root)| root.to_string_lossy() == record.checkout)
        .map(|(label, _)| label.clone()))
}

fn scope(workspace: &str, task: &Task) -> wire::Scope {
    wire::Scope {
        workspace: workspace.into(),
        task: task.task_id.clone(),
        revision: task.revision,
        attempt: task.run.as_ref().map(|_| task.turn() as u64),
        intent_digest: task.intent_digest.clone(),
    }
}

fn digest<T: Serialize>(value: &T) -> Result<String, Code> {
    serde_json::to_vec(value)
        .map(|bytes| task::digest_bytes(&bytes))
        .map_err(|_| Code::Unavailable)
}

pub(super) fn list(inbox: &Inbox, query: &wire::ListQuery) -> Result<wire::List, Code> {
    checked(query.validate())?;
    if !inbox.workspaces.contains_key(&query.workspace) {
        return Err(Code::Forbidden);
    }
    let ids = task::retained_ids(&inbox.store).map_err(super::refusal)?;
    let archived = task::archive::entries(&inbox.store).map_err(super::refusal)?;
    let mut rows = Vec::new();
    let mut pins = Vec::new();
    for id in ids {
        let (task, bytes) = task::retained_snapshot(&inbox.store, &id).map_err(super::refusal)?;
        if !archived.contains_key(&id)
            && label(inbox, &task)?.as_deref() == Some(query.workspace.as_str())
        {
            pins.push((id, task::digest_bytes(&bytes)));
            rows.push(wire::Row {
                task: task.task_id.clone(),
                revision: task.revision,
                title: task.intent.title.clone(),
                phase: super::current(&task).phase,
                attempt: task.run.as_ref().map(|_| task.turn() as u64),
            });
        }
    }
    let snapshot_digest = digest(&(&query.workspace, pins))?;
    let start = query
        .cursor
        .as_ref()
        .map_or(0, |cursor| cursor.next as usize);
    if query
        .cursor
        .as_ref()
        .is_some_and(|cursor| cursor.snapshot_digest != snapshot_digest)
        || start > rows.len()
    {
        return Err(Code::Stale);
    }
    let end = (start + usize::from(query.limit)).min(rows.len());
    let response = wire::List {
        workspace: query.workspace.clone(),
        snapshot_digest: snapshot_digest.clone(),
        rows: rows[start..end].to_vec(),
        next: Some(wire::ListCursor {
            workspace: query.workspace.clone(),
            snapshot_digest,
            next: end as u64,
        }),
        more_available: end < rows.len(),
    };
    checked(response.validate())?;
    Ok(response)
}

fn read(path: &std::path::Path, maximum: u64) -> Result<Vec<u8>, Error> {
    let file = task::private_open(path, false, false)?;
    if file.metadata()?.len() > maximum {
        return Err(Error::LimitExceeded);
    }
    let mut bytes = Vec::new();
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(Error::LimitExceeded);
    }
    Ok(bytes)
}

fn original_pin(source: String, bytes: &[u8], media_type: &str) -> wire::Original {
    wire::Original {
        source,
        digest: task::digest_bytes(bytes),
        bytes: bytes.len() as u64,
        media_type: media_type.into(),
    }
}

struct Trace {
    state: String,
    bytes: Vec<u8>,
    original: Option<wire::Original>,
    steps: Vec<Value>,
    faults: Vec<wire::Fault>,
    children: Vec<wire::Child>,
    more_children: bool,
}

fn trace(inbox: &Inbox, task: &Task) -> Trace {
    let mut trace = Trace {
        state: "not_started".into(),
        bytes: Vec::new(),
        original: None,
        steps: Vec::new(),
        faults: Vec::new(),
        children: Vec::new(),
        more_children: false,
    };
    let Some(run) = &task.run else {
        return trace;
    };
    // The owner admission must name this exact task's current turn.
    if run.admission.trace_file != task.trace_file(task.turn()) {
        trace.state = "source_mismatch".into();
        return trace;
    }
    let path = inbox.store.join(&run.admission.trace_file);
    trace.bytes = match read(&path, wire::MAX_SOURCE_BYTES) {
        Ok(bytes) => bytes,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            trace.state = "missing".into();
            return trace;
        }
        Err(Error::LimitExceeded) => {
            trace.state = "oversized".into();
            return trace;
        }
        Err(_) => {
            trace.state = "unavailable".into();
            return trace;
        }
    };
    let pin = original_pin(
        format!("trace:{}", task.turn()),
        &trace.bytes,
        "application/x-ndjson",
    );
    trace.original = Some(pin.clone());
    let recording = match atif::log::read_bytes(&path, &trace.bytes) {
        Ok(recording) => recording,
        Err(_) => {
            trace.state = "malformed".into();
            return trace;
        }
    };
    let expected_session = format!("{}-{}", task.task_id, task.turn());
    if recording.session.id != expected_session {
        trace.state = "source_mismatch".into();
        return trace;
    }
    trace.faults = recording
        .faults
        .iter()
        .map(|fault| wire::Fault {
            line: fault.line as u64,
            kind: fault.kind.word().into(),
        })
        .collect();
    for (index, step) in recording.steps.iter().enumerate() {
        let Some(call) = &step.call else {
            continue;
        };
        if call.name != "delegate"
            || call.extra.get("schema").and_then(Value::as_str)
                != Some(crate::trace::DELEGATE_CALL_SCHEMA)
        {
            continue;
        }
        let Some(agent) = call
            .extra
            .get("capability")
            .and_then(Value::as_str)
            .filter(|value| plain(value, 128))
        else {
            trace.more_children = true;
            continue;
        };
        if !plain(&call.id, 128) {
            trace.more_children = true;
            continue;
        }
        let reference = call
            .extra
            .get("session_id")
            .and_then(Value::as_str)
            .filter(|value| plain(value, 128))
            .map(str::to_owned);
        if trace.children.len() >= wire::MAX_ITEMS {
            trace.more_children = true;
            continue;
        }
        trace.children.push(wire::Child {
            source_step: index as u64,
            call_id: call.id.clone(),
            agent: agent.into(),
            state: if reference.is_some() {
                "reference_only"
            } else {
                "unavailable"
            }
            .into(),
            reference,
        });
        if wire::bounded(&trace.children, 8 * 1024).is_err() {
            trace.children.pop();
            trace.more_children = true;
        }
    }
    trace.state = if run
        .result
        .as_ref()
        .is_some_and(|result| result.trace_digest != pin.digest)
    {
        "digest_mismatch"
    } else if !recording.faults.is_empty() {
        "damaged"
    } else if !recording.ended() {
        "unsealed"
    } else {
        "sealed"
    }
    .into();
    trace.steps = recording
        .document()
        .get_mut("steps")
        .and_then(Value::as_array_mut)
        .map(std::mem::take)
        .unwrap_or_default();
    trace
}

fn trace_prefix(scope: &wire::Scope, source: &str, steps: &[Value]) -> Result<String, Code> {
    digest(&(scope, source, steps))
}

fn plain(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

fn artifact_alias(entry: &artifact::Entry) -> Result<String, Code> {
    // Hash the serialized manifest path, rather than a lossy display label.
    Ok(format!(
        "artifact:{}",
        digest(&entry.path)?.trim_start_matches("sha256:")
    ))
}

fn artifacts(inbox: &Inbox, task: &Task, journal: &[u8]) -> Vec<wire::Artifact> {
    let task_pin = original_pin("task".into(), journal, "application/json");
    let mut rows = vec![wire::Artifact {
        label: "Canonical task journal".into(),
        state: "retained".into(),
        original: Some(task_pin),
    }];
    let manifest = match artifact::manifest(&inbox.store, task) {
        Ok(Some(manifest)) => manifest,
        Ok(None) => {
            rows.push(wire::Artifact {
                label: "Candidate artifacts".into(),
                state: "unavailable".into(),
                original: None,
            });
            return rows;
        }
        Err(_) => {
            rows.push(wire::Artifact {
                label: "Candidate artifacts".into(),
                state: "damaged_or_unavailable".into(),
                original: None,
            });
            return rows;
        }
    };
    let result = task
        .run
        .as_ref()
        .and_then(|run| run.result.as_ref())
        .expect("manifest has a result");
    let manifest_bytes = match read(
        &inbox
            .store
            .join(result.artifact_file.as_ref().expect("manifest file")),
        task::MAX_STORE_BYTES as u64,
    ) {
        Ok(bytes)
            if task::digest_bytes(&bytes)
                == *result.artifact_digest.as_ref().expect("manifest digest") =>
        {
            bytes
        }
        _ => {
            rows.push(wire::Artifact {
                label: "Candidate artifacts".into(),
                state: "damaged_or_unavailable".into(),
                original: None,
            });
            return rows;
        }
    };
    let pin = original_pin(
        format!("manifest:{}", task.turn()),
        &manifest_bytes,
        "application/json",
    );
    rows.push(wire::Artifact {
        label: "Candidate artifact manifest".into(),
        state: if manifest.complete {
            "complete"
        } else {
            "incomplete"
        }
        .into(),
        original: Some(pin.clone()),
    });
    let mut represented = 0;
    for entry in &manifest.entries {
        // Keep space for transcript pages and an explicit omission row. The
        // manifest remains available in original byte chunks.
        if rows.len() >= 18 {
            break;
        }
        let label = entry
            .path
            .to_str()
            .filter(|path| {
                !path.is_empty() && path.len() <= 1024 && !path.chars().any(char::is_control)
            })
            .unwrap_or("Path is available in the original manifest")
            .to_owned();
        let (state, original) = if entry.state == "retained" {
            match artifact::read_entry(&inbox.store, entry).and_then(|bytes| {
                let alias = artifact_alias(entry)
                    .map_err(|_| Error::Corrupt("artifact path cannot be encoded"))?;
                Ok(original_pin(alias, &bytes, "application/octet-stream"))
            }) {
                Ok(original) => ("retained".into(), Some(original)),
                Err(_) => ("damaged_or_unavailable".into(), None),
            }
        } else {
            let state = match entry.state.as_str() {
                "removed"
                | "directory"
                | "symlink"
                | "unavailable"
                | "unavailable_or_over_limit" => entry.state.clone(),
                _ => "unknown".into(),
            };
            (state, None)
        };
        rows.push(wire::Artifact {
            label,
            state,
            original,
        });
        represented += 1;
        if wire::bounded(&rows, 10 * 1024).is_err() {
            rows.pop();
            represented -= 1;
            break;
        }
    }
    if represented < manifest.entries.len() {
        rows.push(wire::Artifact {
            label: format!("{} remaining entries", manifest.entries.len() - represented),
            state: "remaining_entries".into(),
            original: Some(pin.clone()),
        });
    }
    if manifest.omitted_changes > 0 {
        rows.push(wire::Artifact {
            label: format!("{} omitted changes", manifest.omitted_changes),
            state: "omitted_changes".into(),
            original: Some(pin),
        });
    }
    rows
}

pub(super) fn page(inbox: &Inbox, query: &wire::PageQuery) -> Result<wire::Page, Code> {
    checked(query.validate())?;
    let (task, journal) = snapshot(inbox, &query.workspace, &query.task)?;
    if query
        .revision
        .is_some_and(|revision| revision != task.revision)
    {
        return Err(Code::Stale);
    }
    let scope = scope(&query.workspace, &task);
    let trace = trace(inbox, &task);
    let start = query
        .cursor
        .as_ref()
        .map_or(0, |cursor| cursor.next_step as usize);
    if let Some(cursor) = &query.cursor {
        if cursor.scope != scope
            || trace
                .original
                .as_ref()
                .is_none_or(|original| original.source != cursor.source)
            || cursor.source_bytes > trace.bytes.len() as u64
            || task::digest_bytes(&trace.bytes[..cursor.source_bytes as usize])
                != cursor.source_digest
            || start > trace.steps.len()
            || trace_prefix(&scope, &cursor.source, &trace.steps[..start])? != cursor.prefix_digest
        {
            return Err(Code::Stale);
        }
    }
    let result = task.run.as_ref().and_then(|run| run.result.as_ref());
    let mut response = wire::Page {
        scope: scope.clone(),
        title: task.intent.title.clone(),
        prompt: task.intent.prompt.clone(),
        phase: super::current(&task).phase,
        execution: match task.execution {
            task::Execution::NotStarted => "not_started",
            task::Execution::Running => "running",
            task::Execution::Finished => "finished",
            task::Execution::Failed => "failed",
            task::Execution::Stopped => "stopped",
            task::Execution::Unknown => "unknown",
        }
        .into(),
        verification: match task.checks {
            task::Checks::NotRun => "not_run",
            task::Checks::Running => "running",
            task::Checks::Passed => "passed",
            task::Checks::Failed => "failed",
            task::Checks::Unavailable => "unavailable",
            task::Checks::Disputed => "disputed",
        }
        .into(),
        integration: "unknown".into(),
        termination: result
            .map_or("unknown", |result| {
                if plain(&result.ending, 128) {
                    result.ending.as_str()
                } else {
                    "unknown"
                }
            })
            .into(),
        delivery: result
            .map_or("unknown", |result| {
                if result.output_incomplete {
                    "incomplete"
                } else {
                    "complete"
                }
            })
            .into(),
        cleanup: result
            .map_or("unknown", |result| {
                if result.group_clear {
                    "clear"
                } else {
                    "unknown"
                }
            })
            .into(),
        cost_microusd: result.and_then(|result| result.cost_microusd),
        cost_status: result
            .map_or("unknown", |result| match result.cost_status.as_str() {
                "priced" | "partial" => result.cost_status.as_str(),
                _ => "unknown",
            })
            .into(),
        evidence: wire::Evidence {
            state: trace.state,
            original: trace.original.clone(),
            total_steps: trace.steps.len() as u64,
            steps: Vec::new(),
            faults: trace.faults.iter().take(wire::MAX_ITEMS).cloned().collect(),
            more_faults: trace.faults.len() > wire::MAX_ITEMS,
        },
        artifacts: artifacts(inbox, &task, &journal),
        next: None,
        more_available: false,
        children: trace.children,
        more_children: trace.more_children,
    };
    // Reserve enough room for one intact maximum-sized step and its cursor.
    // A long prompt remains available in the pinned canonical journal.
    if wire::bounded(
        &response,
        wire::MAX_REPLY_BYTES - wire::MAX_INLINE_STEP_BYTES - 4096,
    )
    .is_err()
    {
        response.prompt.clear();
        response.artifacts[0].state = "prompt_oversized".into();
    }
    checked(wire::bounded(
        &response,
        wire::MAX_REPLY_BYTES - wire::MAX_INLINE_STEP_BYTES - 4096,
    ))?;
    for (index, step) in trace
        .steps
        .iter()
        .enumerate()
        .skip(start)
        .take(query.limit.into())
    {
        let step = if wire::bounded(step, wire::MAX_INLINE_STEP_BYTES).is_ok() {
            wire::Step::Original {
                index: index as u64,
                step: step.clone(),
            }
        } else {
            wire::Step::Gap {
                index: index as u64,
                reason: wire::GapReason::Oversized,
                original: trace.original.clone().expect("steps have a source"),
            }
        };
        response.evidence.steps.push(step);
        if wire::bounded(&response, wire::MAX_REPLY_BYTES - 2048).is_err() {
            response.evidence.steps.pop();
            break;
        }
    }
    let end = start + response.evidence.steps.len();
    if let Some(original) = &trace.original {
        response.next = Some(wire::Cursor {
            scope,
            source: original.source.clone(),
            source_digest: original.digest.clone(),
            source_bytes: original.bytes,
            next_step: end as u64,
            prefix_digest: trace_prefix(&response.scope, &original.source, &trace.steps[..end])?,
        });
    }
    response.more_available = end < trace.steps.len();
    checked(response.validate())?;
    // A concurrent task transition invalidates the projection, even when the
    // old files were independently readable.
    let (current, _) = snapshot(inbox, &query.workspace, &query.task)?;
    if scope_of(&current, &response.scope) != response.scope {
        return Err(Code::Stale);
    }
    Ok(response)
}

fn scope_of(task: &Task, expected: &wire::Scope) -> wire::Scope {
    scope(&expected.workspace, task)
}

fn original_bytes(
    inbox: &Inbox,
    task: &Task,
    journal: Vec<u8>,
    source: &str,
) -> Result<(wire::Original, Vec<u8>), Code> {
    if source == "task" {
        return Ok((
            original_pin(source.into(), &journal, "application/json"),
            journal,
        ));
    }
    if source == format!("trace:{}", task.turn()) {
        let trace = trace(inbox, task);
        return trace
            .original
            .map(|pin| (pin, trace.bytes))
            .ok_or(Code::Unavailable);
    }
    let manifest = artifact::manifest(&inbox.store, task)
        .map_err(super::refusal)?
        .ok_or(Code::Forbidden)?;
    if source == format!("manifest:{}", task.turn()) {
        let result = task
            .run
            .as_ref()
            .and_then(|run| run.result.as_ref())
            .ok_or(Code::Forbidden)?;
        let bytes = read(
            &inbox
                .store
                .join(result.artifact_file.as_ref().ok_or(Code::Forbidden)?),
            task::MAX_STORE_BYTES as u64,
        )
        .map_err(super::refusal)?;
        if Some(&task::digest_bytes(&bytes)) != result.artifact_digest.as_ref() {
            return Err(Code::Stale);
        }
        return Ok((
            original_pin(source.into(), &bytes, "application/json"),
            bytes,
        ));
    }
    let mut found = None;
    for entry in &manifest.entries {
        if artifact_alias(entry)? == source {
            if found.is_some() {
                return Err(Code::Unavailable);
            }
            found = Some(entry);
        }
    }
    let entry = found.ok_or(Code::Forbidden)?;
    if entry.state != "retained" {
        return Err(Code::Forbidden);
    }
    let bytes = artifact::read_entry(&inbox.store, entry).map_err(super::refusal)?;
    Ok((
        original_pin(source.into(), &bytes, "application/octet-stream"),
        bytes,
    ))
}

fn byte_prefix(
    scope: &wire::Scope,
    original: &wire::Original,
    prefix: &[u8],
) -> Result<String, Code> {
    digest(&(scope, original, task::digest_bytes(prefix)))
}

pub(super) fn original(
    inbox: &Inbox,
    query: &wire::OriginalQuery,
) -> Result<wire::OriginalChunk, Code> {
    checked(query.validate())?;
    let (task, journal) = snapshot(inbox, &query.scope.workspace, &query.scope.task)?;
    if scope_of(&task, &query.scope) != query.scope {
        return Err(Code::Stale);
    }
    let (pin, bytes) = original_bytes(inbox, &task, journal, &query.original.source)?;
    if pin != query.original {
        return Err(Code::Stale);
    }
    let start = query
        .cursor
        .as_ref()
        .map_or(0, |cursor| cursor.next_byte as usize);
    if start > bytes.len() {
        return Err(Code::Stale);
    }
    if let Some(cursor) = &query.cursor
        && byte_prefix(&query.scope, &pin, &bytes[..start])? != cursor.prefix_digest
    {
        return Err(Code::Stale);
    }
    let end = (start + query.limit as usize).min(bytes.len());
    let response = wire::OriginalChunk {
        scope: query.scope.clone(),
        original: pin.clone(),
        start: start as u64,
        data: STANDARD.encode(&bytes[start..end]),
        next: Some(wire::OriginalCursor {
            scope: query.scope.clone(),
            source: pin.source.clone(),
            digest: pin.digest.clone(),
            next_byte: end as u64,
            prefix_digest: byte_prefix(&query.scope, &pin, &bytes[..end])?,
        }),
        more_available: end < bytes.len(),
    };
    checked(response.validate())?;
    let (current, _) = snapshot(inbox, &query.scope.workspace, &query.scope.task)?;
    if scope_of(&current, &query.scope) != query.scope {
        return Err(Code::Stale);
    }
    Ok(response)
}

#[cfg(test)]
#[path = "remote_observe_tests.rs"]
mod tests;
