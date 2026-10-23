//! The visit adapter writes authenticated messages into an existing coder-sync Loro room.
use super::*;
use loro::{ExportMode, LoroDoc};
use openagents_doc::chat_frames::{decode, frame_type};

fn unavailable() -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "chat_unavailable",
        "Chat could not connect. Try again.",
    )
}

fn credential(app: &App, id: &str) -> Result<String, Response> {
    let key = app
        .config
        .room_token_key
        .as_deref()
        .ok_or_else(unavailable)?;
    coder_access::room_token::mint(
        key,
        "verse_visits",
        coder_access::room_token::Room::Visit,
        id,
        now(),
        30,
    )
    .map_err(|_| unavailable())
}

/// Catch up from bounded room rows. The browser never gets the sync credential or Loro writes.
pub(super) async fn load(app: &App, id: &str) -> Result<LoroDoc, Response> {
    let doc = LoroDoc::new();
    let client = reqwest::Client::new();
    let mut after = 0;
    for _ in 0..16 {
        let response = client
            .get(format!(
                "{}/visit/{id}/rows?after={after}",
                app.config.sync_origin.trim_end_matches('/')
            ))
            .bearer_auth(credential(app, id)?)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|_| unavailable())?;
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|n| n > 4 * 1024 * 1024)
        {
            return Err(unavailable());
        }
        let bytes = response.bytes().await.map_err(|_| unavailable())?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(unavailable());
        }
        let mut rest = bytes.as_ref();
        let mut done = false;
        let before = after;
        while !rest.is_empty() {
            if rest.len() < 4 {
                return Err(unavailable());
            }
            let length = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
            if length > rest.len() - 4 {
                return Err(unavailable());
            }
            let frame = decode(&rest[4..4 + length]).ok_or_else(unavailable)?;
            rest = &rest[4 + length..];
            match frame.kind {
                frame_type::STATE if frame.header["seqFloor"].as_u64().unwrap_or(0) != 0 => {
                    return Err(unavailable());
                }
                frame_type::ROW => {
                    let seq = frame.header["seq"].as_u64().ok_or_else(unavailable)?;
                    if seq <= after {
                        return Err(unavailable());
                    }
                    doc.import(&frame.payload).map_err(|_| unavailable())?;
                    after = seq;
                }
                frame_type::ROWS_DONE => done = true,
                _ => {}
            }
        }
        if done {
            return Ok(doc);
        }
        if after == before {
            return Err(unavailable());
        }
    }
    Err(unavailable())
}

pub(super) fn messages(doc: &LoroDoc) -> Result<Vec<Value>, Response> {
    let mut messages = Vec::new();
    let value = serde_json::to_value(doc.get_map("messages").get_deep_value())
        .map_err(|_| unavailable())?;
    for text in value.as_object().into_iter().flat_map(|m| m.values()) {
        let message: Value = serde_json::from_str(text.as_str().ok_or_else(unavailable)?)
            .map_err(|_| unavailable())?;
        messages.push(message);
    }
    messages.sort_by(|a, b| {
        a["at"]
            .as_u64()
            .cmp(&b["at"].as_u64())
            .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
    });
    if messages.len() > 512 {
        return Err(unavailable());
    }
    Ok(messages)
}

/// One immutable entry per room/account/command. A lost acknowledgement retries the same batch.
pub(super) async fn send(app: &App, room: &str, id: &str, message: &Value) -> Result<(), Response> {
    let doc = LoroDoc::new();
    // Deterministic peer per message makes retries byte-identical, including after a process restart.
    let peer = u64::from_str_radix(&id[..16], 16).map_err(|_| unavailable())?;
    doc.set_peer_id(peer).map_err(|_| unavailable())?;
    doc.get_map("messages")
        .insert(id, message.to_string())
        .map_err(|_| unavailable())?;
    doc.set_next_commit_timestamp(message["at"].as_i64().unwrap_or(0));
    doc.commit();
    let bytes = doc
        .export(ExportMode::all_updates())
        .map_err(|_| unavailable())?;
    let response = reqwest::Client::new()
        .post(format!(
            "{}/visit/{room}/rows?device=website&batchId={id}",
            app.config.sync_origin.trim_end_matches('/')
        ))
        .bearer_auth(credential(app, room)?)
        .body(bytes)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| unavailable())?;
    if !response.status().is_success() {
        return Err(unavailable());
    }
    Ok(())
}
