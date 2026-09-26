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
    let grant = Grant::parse(grant_bytes).map_err(|e| e.to_string())?;
    let input = book.blobs.resolve(&book.labor().execution.input)?;
    exact(
        input,
        "coder.free-labor.command.v1",
        &["intent", "source_snapshot", "expected_output_digest"],
    )?;
    let intent: TaskIntent =
        serde_json::from_value(input["intent"].clone()).map_err(|e| e.to_string())?;
    let requirements = book.blobs.resolve(&book.labor().execution.requirements)?;
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
        || grant.task_id != execute.request
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
        command_id: format!("labor-{}", execute.request),
        task_id: execute.request,
        expected_revision: None,
        action: Action::Submit { intent },
    };
    task::parse_command(&serde_json::to_vec(&command).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    Ok((command, grant))
}
