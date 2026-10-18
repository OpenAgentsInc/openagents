//! Run controls use the enrolled host channel and retain exact retry packets.
use crate::{
    App,
    chat_store::{Error, Store},
    coder_sync::{answer, refused, stored},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::post,
};
use coder_access::{
    app_rpc::{Method, Request},
    client::Pending,
    protocol::Outcome,
};
use openagents_sdk::runs::{
    CommandOutcome, CommandReceipt, CommandRefusal, RunCommand, RunKind, valid_id,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, time::Duration};

const KEY: &str = "runs/commands.json";
const MAX_RECORDS: usize = 4096;
const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Default, Serialize, Deserialize)]
struct Book {
    records: BTreeMap<String, Record>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Record {
    request: RunCommand,
    computer: String,
    binding: Option<String>,
    pending: Option<Pending>,
    receipt: Option<CommandReceipt>,
}
fn invalid() -> Response {
    refused(
        StatusCode::BAD_REQUEST,
        "invalid_run_command",
        "Check the run command and try again.",
    )
}
fn conflict() -> Response {
    refused(
        StatusCode::CONFLICT,
        "command_conflict",
        "This request ID was already used. Keep its command unchanged.",
    )
}
fn receipt(run: &str, request: &str, reason: CommandRefusal) -> CommandReceipt {
    CommandReceipt {
        run: run.into(),
        request_id: request.into(),
        outcome: CommandOutcome::Refused { reason },
    }
}
fn request_key(run: &str, request: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(run, request)).expect("string tuple"))
    )
}
async fn load(store: &Store, owner: &str) -> Result<(Book, Option<String>), Error> {
    match store.read_key(&Store::owner_key(owner, KEY)?).await? {
        Some((bytes, generation)) => Ok((
            serde_json::from_slice(&bytes)
                .map_err(|_| Error::Corrupt("The run commands could not be read."))?,
            Some(generation),
        )),
        None => Ok((Book::default(), None)),
    }
}
async fn reserve(
    store: &Store,
    owner: &str,
    key: &str,
    proposed: &Record,
) -> Result<Result<Record, ()>, Error> {
    for _ in 0..8 {
        let (mut book, generation) = load(store, owner).await?;
        if let Some(was) = book.records.get(key) {
            return Ok(if was.request == proposed.request {
                Ok(was.clone())
            } else {
                Err(())
            });
        }
        if book.records.len() >= MAX_RECORDS {
            return Err(Error::Invalid("Your run command list is full."));
        }
        book.records.insert(key.into(), proposed.clone());
        let bytes = serde_json::to_vec(&book)
            .map_err(|_| Error::Corrupt("The run command could not be saved."))?;
        let receipt_room = book
            .records
            .values()
            .filter(|r| r.receipt.is_none())
            .count()
            * 512;
        if bytes.len().saturating_add(receipt_room) > MAX_BYTES {
            return Err(Error::Invalid("Your run command list is full."));
        }
        match store
            .write_key(&Store::owner_key(owner, KEY)?, bytes, generation.as_deref())
            .await
        {
            Ok(_) => return Ok(Ok(proposed.clone())),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}
async fn finish(
    store: &Store,
    owner: &str,
    key: &str,
    result: &CommandReceipt,
) -> Result<(), Error> {
    for _ in 0..8 {
        let (mut book, generation) = load(store, owner).await?;
        let record = book.records.get_mut(key).ok_or(Error::Conflict)?;
        if record.receipt.is_some() {
            return Ok(());
        }
        record.receipt = Some(result.clone());
        let bytes = serde_json::to_vec(&book)
            .map_err(|_| Error::Corrupt("The run command could not be saved."))?;
        if bytes.len() > MAX_BYTES {
            return Err(Error::Invalid("Your run command list is full."));
        }
        match store
            .write_key(&Store::owner_key(owner, KEY)?, bytes, generation.as_deref())
            .await
        {
            Ok(_) => return Ok(()),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/runs/{id}/commands", post(command))
        .layer(DefaultBodyLimit::max(20 * 1024))
}
async fn command(
    State(app): State<App>,
    headers: HeaderMap,
    Path(run): Path<String>,
    body: Result<Bytes, axum::extract::rejection::BytesRejection>,
) -> Response {
    let account = match crate::coder_sync::account(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let owner = crate::chat_store::account_owner(&account);
    let body = match body {
        Ok(b) => b,
        Err(e) => return refused(e.status(), "too_large", "Send a shorter run command."),
    };
    let Ok(request) = serde_json::from_slice::<RunCommand>(&body) else {
        return invalid();
    };
    if !valid_id(&run) || !request.valid() {
        return invalid();
    }
    let store = &app.config.chat_store;
    let key = request_key(&run, &request.request_id);
    let existing = match load(store, &owner).await {
        Ok((book, _)) => book.records.get(&key).cloned(),
        Err(e) => return stored(&e),
    };
    let record = if let Some(record) = existing {
        if record.request != request {
            return conflict();
        }
        record
    } else {
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64;
        let presence = match crate::runs::lookup(store, &owner, &run, at).await {
            Ok(Some(v)) => v,
            Ok(None) => return refused(StatusCode::NOT_FOUND, "not_found", "No such run."),
            Err(e) => return stored(&e),
        };
        let reason = if presence.ended_at.is_some() {
            Some(CommandRefusal::RunEnded)
        } else if !presence.is_live(at) {
            Some(CommandRefusal::Offline)
        } else if presence.beat.kind != RunKind::AppChat {
            Some(CommandRefusal::NotSteerable)
        } else {
            None
        };
        let mut proposed = Record {
            request: request.clone(),
            computer: presence.beat.computer.clone(),
            binding: None,
            pending: None,
            receipt: reason.map(|r| receipt(&run, &request.request_id, r)),
        };
        if reason.is_none() {
            let binding = app
                .config
                .cloud_hosts
                .as_deref()
                .and_then(|hosts| hosts.for_run(&account, &presence.beat.computer).ok());
            if let Some(binding) = binding {
                let app_request = Request {
                    method: Method::RunCommand,
                    params: json!({"account":account,"run":run,"session":presence.beat.harness_session,"request_id":request.request_id,"command":request.command,"agent":binding.run_agent()}),
                };
                match binding.prepare_run(&account, app_request, request_key(&account, &key)) {
                    Ok(pending) => {
                        proposed.binding = Some(binding.identity().into());
                        proposed.pending = Some(pending);
                    }
                    Err(_) => {
                        proposed.receipt =
                            Some(receipt(&run, &request.request_id, CommandRefusal::NoGrant))
                    }
                }
            } else {
                proposed.receipt =
                    Some(receipt(&run, &request.request_id, CommandRefusal::NoGrant));
            }
        }
        match reserve(store, &owner, &key, &proposed).await {
            Ok(Ok(v)) => v,
            Ok(Err(())) => return conflict(),
            Err(e) => return stored(&e),
        }
    };
    if let Some(result) = record.receipt {
        return answer(StatusCode::OK, json!(result));
    }
    let Some(binding) = app
        .config
        .cloud_hosts
        .as_deref()
        .and_then(|hosts| hosts.for_run(&account, &record.computer).ok())
    else {
        return answer(
            StatusCode::OK,
            json!(receipt(&run, &request.request_id, CommandRefusal::NoGrant)),
        );
    };
    if record.binding.as_deref() != Some(binding.identity()) {
        return conflict();
    }
    let Some(pending) = record.pending.as_ref() else {
        return invalid();
    };
    let result =
        tokio::time::timeout(Duration::from_secs(8), binding.send_run(&account, pending)).await;
    let result = match result {
        Ok(Ok(Outcome::AppRpc { app: response })) if response.method == Method::RunCommand => {
            match serde_json::from_value::<CommandReceipt>(response.value) {
                Ok(r) if r.run == run && r.request_id == request.request_id => r,
                _ => receipt(&run, &request.request_id, CommandRefusal::Unavailable),
            }
        }
        Ok(Err(
            coder_access::Code::Forbidden
            | coder_access::Code::MissingRight
            | coder_access::Code::Revoked
            | coder_access::Code::Expired
            | coder_access::Code::Denied,
        )) => receipt(&run, &request.request_id, CommandRefusal::NoGrant),
        Ok(Err(coder_access::Code::Unsupported)) => {
            receipt(&run, &request.request_id, CommandRefusal::NotSteerable)
        }
        _ => receipt(&run, &request.request_id, CommandRefusal::Unavailable),
    };
    if result.outcome
        != (CommandOutcome::Refused {
            reason: CommandRefusal::Unavailable,
        })
    {
        if let Err(e) = finish(store, &owner, &key, &result).await {
            return stored(&e);
        }
    }
    answer(StatusCode::OK, json!(result))
}

#[cfg(test)]
mod tests;
