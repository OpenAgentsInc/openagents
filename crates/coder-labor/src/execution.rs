//! Bridge a durably bound labor request to the existing local task owner.
use crate::book::Book;
use crate::*;
use coder::task::owner::Grant;
use coder::task::{self, Action, Command, TaskIntent};
use nostr::domain::Event;

pub(crate) fn prepare(
    book: &Book,
    event: &Event,
    grant_bytes: &[u8],
    now: u64,
) -> Result<(Command, Grant)> {
    let execute = book.check_execute(event, now)?;
    let input = book.blobs.resolve(&book.labor().execution.input)?;
    let requirements = book.blobs.resolve(&book.labor().execution.requirements)?;
    prepare_frozen(
        input,
        requirements,
        &execute,
        &execute.request,
        grant_bytes,
        now,
    )
}

/// Match a separately granted task-owner command to one exact admitted CJ
/// attempt. This validates bytes and bounds; it does not admit an order or
/// dispatch. Native repair hosts give each attempt its own local task ID while
/// retaining the order's original CJ request and run.
pub fn prepare_frozen(
    input: &Value,
    requirements: &Value,
    execute: &nostr::execution::Execute,
    local_task: &str,
    grant_bytes: &[u8],
    now: u64,
) -> Result<(Command, Grant)> {
    let grant = Grant::parse(grant_bytes).map_err(|e| e.to_string())?;
    let input_ref = execute
        .input_artifact
        .as_ref()
        .ok_or("the admitted command has no artifact input")?;
    nostr::contracts::check_artifact_bytes(input_ref, &jcs(input).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    nostr::contracts::check_artifact_bytes(
        &execute.requirements,
        &jcs(requirements).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    exact(
        input,
        "coder.free-labor.command.v1",
        &["intent", "source_snapshot", "expected_output_digest"],
    )?;
    let intent: TaskIntent =
        serde_json::from_value(input["intent"].clone()).map_err(|e| e.to_string())?;
    exact(
        requirements,
        "coder.free-labor.requirements.v1",
        &[
            "program_digest",
            "arguments_digest",
            "write_workspace",
            "wall_seconds",
            "stream_bytes",
            "memory_bytes",
        ],
    )?;
    if intent
        .workspace
        .source_revision
        .as_ref()
        .is_none_or(|revision| revision.len() != 40)
        || grant.task_id != local_task
        || grant.expected_revision != 1
        || grant.expected_source_snapshot.as_deref() != input["source_snapshot"].as_str()
        || grant.intent_digest
            != nostr::contracts::digest_bytes(
                &serde_json::to_vec(&intent).map_err(|e| e.to_string())?,
            )
        || requirements["program_digest"]
            != nostr::contracts::digest_bytes(
                &std::fs::read(&grant.program).map_err(|e| e.to_string())?,
            )
        || requirements["arguments_digest"]
            != nostr::contracts::digest_bytes(
                &jcs(&json!(grant.arguments)).map_err(|e| e.to_string())?,
            )
        || requirements["write_workspace"] != grant.write_workspace
        || requirements["wall_seconds"] != grant.wall_seconds
        || requirements["stream_bytes"] != grant.stream_bytes
        || requirements["memory_bytes"] != grant.memory_bytes
        || grant.requirements.is_some()
        || grant.adapter_configuration.is_some()
        || now >= execute.deadline
        || execute
            .bounds
            .wall_ms
            .is_none_or(|ceiling| grant.wall_seconds.saturating_mul(1000) > ceiling)
        || grant.wall_seconds > execute.deadline.saturating_sub(now)
        || execute.bounds.output_bytes.is_none_or(|ceiling| {
            (2 * 1024 * 1024u64).saturating_add((grant.stream_bytes as u64).saturating_mul(2))
                > ceiling
        })
    {
        return Err("operator execution grant differs from frozen labor requirements".into());
    }
    let command = Command {
        schema: task::COMMAND_SCHEMA.into(),
        command_id: format!("labor-{local_task}"),
        task_id: local_task.into(),
        expected_revision: None,
        action: Action::Submit { intent },
    };
    task::parse_command(&serde_json::to_vec(&command).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    Ok((command, grant))
}
