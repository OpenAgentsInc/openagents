//! Publish screened terminal text through the apps' session schema and room client.
//! The saved ATIF file is the durable source. A failed room delivery leaves the
//! upload unsent, so the normal account-sync sweep retries it after a restart.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::BoxFuture;
use loro::{Container, ExportMode, LoroValue, ValueOrContainer, VersionVector};
use openagents_doc::{MessagePart, MessageRole, MessageStatus, SessionDoc, SessionMessageEntry};
use openagents_sdk::{CancellationToken, Client, Deployment, Error, terminal_chats};
use openagents_sync::chat_client::{ChatTransport, RowImportOutcome};
use openagents_sync::{ChatClient, ChatDocSink, CheckpointFetcher, SyncError, UrlProvider};
use sha2::{Digest, Sha256};

use super::{Answer, Saved, Upload};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub user: bool,
    pub text: String,
}

#[derive(Clone)]
struct Transport {
    room: openagents_sdk::rooms::Room,
    device: String,
}

fn sync_error(error: Error) -> SyncError {
    match error {
        Error::Unauthorized
        | Error::ExpiredSession
        | Error::WrongAccount
        | Error::Forbidden
        | Error::Cancelled => SyncError::Auth(error.to_string()),
        _ => SyncError::TemporarilyUnavailable(error.to_string()),
    }
}

impl UrlProvider for Transport {
    fn url(&self) -> BoxFuture<'static, Result<String, SyncError>> {
        let result = self.room.socket(&self.device).map_err(sync_error);
        Box::pin(async move { result })
    }
}
impl CheckpointFetcher for Transport {
    fn fetch(&self) -> BoxFuture<'static, Result<Vec<u8>, SyncError>> {
        let room = self.room.clone();
        Box::pin(async move { room.checkpoint().await.map_err(sync_error) })
    }
}
impl ChatTransport for Transport {
    fn fetch_rows(&self, after: u64) -> BoxFuture<'static, Result<Vec<u8>, SyncError>> {
        let room = self.room.clone();
        Box::pin(async move { room.rows(after).await.map_err(sync_error) })
    }
    fn push(
        &self,
        batch_id: String,
        bytes: Vec<u8>,
    ) -> BoxFuture<'static, Result<String, SyncError>> {
        let room = self.room.clone();
        let device = self.device.clone();
        Box::pin(async move {
            room.push(&device, &batch_id, bytes)
                .await
                .map_err(sync_error)
        })
    }
}

struct State {
    doc: SessionDoc,
    failed: bool,
}
struct Sink(Mutex<State>);
impl Sink {
    fn new() -> Self {
        Self(Mutex::new(State {
            doc: SessionDoc::from_doc(loro::LoroDoc::new()),
            failed: false,
        }))
    }
}
impl ChatDocSink for Sink {
    fn apply_row(&self, bytes: &[u8], _: u64) -> RowImportOutcome {
        let mut state = self.0.lock().unwrap();
        match state.doc.doc().import(bytes) {
            Ok(status) if status.pending.is_none() => RowImportOutcome::Applied,
            Ok(_) => RowImportOutcome::PendingDependencies,
            Err(_) => {
                state.failed = true;
                RowImportOutcome::PendingDependencies
            }
        }
    }
    fn apply_checkpoint(&self, bytes: &[u8], _: u64) -> Result<(), String> {
        let mut state = self.0.lock().unwrap();
        match state.doc.doc().import(bytes) {
            Ok(status) if status.pending.is_none() => Ok(()),
            _ => {
                state.failed = true;
                Err("The chat could not be loaded.".into())
            }
        }
    }
    fn contains_frontier(&self, frontier: &[u8]) -> bool {
        VersionVector::decode(frontier).is_ok_and(|vv| {
            !vv.is_empty() && self.0.lock().unwrap().doc.doc().oplog_vv().includes_vv(&vv)
        })
    }
    fn advance_cursor(&self, _: u64) {}
}

/// Add or update only entries written by this terminal. Commands, continuation
/// metadata, and other devices' entries remain in the imported document.
fn merge(
    doc: &SessionDoc,
    chat: &str,
    device: &str,
    messages: &[Message],
    at: i64,
) -> Result<Vec<Vec<u8>>, String> {
    let fail = || "The chat could not be saved.".to_owned();
    let mut before = doc.doc().oplog_vv();
    match doc.chat_id() {
        Some(id) if id != chat => return Err(fail()),
        Some(_) => {}
        None => {
            doc.doc()
                .get_map("meta")
                .insert("chatId", chat)
                .map_err(|_| fail())?;
            doc.doc()
                .get_map("meta")
                .insert("schemaVersion", openagents_doc::SESSION_SCHEMA_VERSION)
                .map_err(|_| fail())?;
            doc.doc().commit();
        }
    }
    // The first delta includes metadata when this room has no founding row yet.
    let mut updates = Vec::new();
    let entries = doc.doc().get_list("messages");
    let mut by_id = std::collections::HashMap::new();
    let mut source_ids = std::collections::HashSet::new();
    for index in 0..entries.len() {
        let Some(ValueOrContainer::Container(Container::Map(map))) = entries.get(index) else {
            continue;
        };
        let Some(ValueOrContainer::Value(LoroValue::String(id))) = map.get("id") else {
            continue;
        };
        if by_id.insert(id.to_string(), map).is_some() {
            return Err(fail());
        }
    }
    for message in messages {
        if !source_ids.insert(&message.id) {
            return Err(fail());
        }
        if let Some(map) = by_id.get(&message.id) {
            if !matches!(map.get("deviceId"), Some(ValueOrContainer::Value(LoroValue::String(value))) if value.as_str() == device)
            {
                return Err(fail());
            }
            let expected_role = if message.user { "user" } else { "assistant" };
            if !matches!(map.get("role"), Some(ValueOrContainer::Value(LoroValue::String(value))) if value.as_str() == expected_role)
            {
                return Err(fail());
            }
            let Some(ValueOrContainer::Container(Container::List(parts))) = map.get("parts") else {
                return Err(fail());
            };
            let Some(ValueOrContainer::Container(Container::Map(part))) = parts.get(0) else {
                return Err(fail());
            };
            if parts.len() != 1 {
                return Err(fail());
            }
            let Some(ValueOrContainer::Container(Container::Text(text))) = part.get("text") else {
                return Err(fail());
            };
            if text.to_string() != message.text {
                text.update(&message.text, Default::default())
                    .map_err(|_| fail())?;
                doc.doc().commit();
            }
        } else {
            doc.push_message(&SessionMessageEntry {
                id: message.id.clone(),
                role: if message.user {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                parts: vec![MessagePart::Text {
                    id: format!("{}-text", message.id),
                    text: message.text.clone(),
                }],
                created_at: at,
                device_id: device.into(),
                status: Some(MessageStatus::Complete),
                continuation_of: None,
                duration_ms: None,
            })
            .map_err(|_| fail())?;
        }
        let now = doc.doc().oplog_vv();
        if now != before {
            let bytes = doc
                .doc()
                .export(ExportMode::updates(&before))
                .map_err(|_| fail())?;
            if bytes.len() > openagents_sync::chat_client::MAX_PUSH_BYTES {
                return Err(fail());
            }
            updates.push(bytes);
            before = now;
        }
    }
    Ok(updates)
}

fn answer(error: Error) -> Answer {
    match error {
        Error::Unauthorized
        | Error::ExpiredSession
        | Error::InvalidCredential
        | Error::WrongAccount => Answer::SignedOut,
        Error::Refused { status: 410, .. } => Answer::Deleted,
        Error::Refused { status: 413, .. } | Error::TooLarge => Answer::Full,
        Error::Refused {
            status: 400..=499, ..
        }
        | Error::Forbidden
        | Error::InvalidReference => Answer::Refused(error.to_string()),
        _ => Answer::Retry,
    }
}

/// `None` means the old website has no account-discovery route. Only that
/// specific compatibility case may use the earlier saved-chat endpoint.
pub(super) async fn put(saved: &Saved, local: &str, upload: &Upload) -> Option<Answer> {
    put_with_service(
        saved,
        local,
        upload,
        std::env::var("OPENAGENTS_SYNC_URL").ok(),
    )
    .await
}

async fn put_with_service(
    saved: &Saved,
    local: &str,
    upload: &Upload,
    service: Option<String>,
) -> Option<Answer> {
    let deployment = if saved.origin.trim_end_matches('/') == "https://openagents.com" {
        Deployment::production()
    } else {
        match Deployment::new(
            &format!("{}/v1", saved.origin.trim_end_matches('/')),
            &saved.origin,
        ) {
            Ok(value) => value,
            Err(error) => return Some(answer(error)),
        }
    };
    let client = match Client::with_test_run(
        deployment,
        Duration::from_secs(120),
        std::env::var("OPENAGENTS_TEST_RUN").as_deref() == Ok("1"),
    ) {
        Ok(client) => client,
        Err(error) => return Some(answer(error)),
    };
    let session = match client.authenticate(saved.token()).await {
        Ok(session) if session.account().id == saved.account => session,
        Ok(_) => return Some(Answer::SignedOut),
        Err(Error::NotFound | Error::Refused { status: 404, .. }) => return None,
        Err(error) => return Some(answer(error)),
    };
    let snapshot: terminal_chats::Snapshot = match serde_json::from_value(upload.body.clone()) {
        Ok(value) => value,
        Err(_) => return Some(Answer::Refused("The chat could not be saved.".into())),
    };
    let cancel = CancellationToken::new();
    let receipt = match terminal_chats::save(&client, &session, local, &snapshot, &cancel).await {
        Ok(value) => value,
        Err(error) => return Some(answer(error)),
    };
    // A custom deployment must explicitly configure its sync service. It must
    // never send that deployment's credentials to the production service.
    let base = match service {
        Some(base) => base,
        None if *client.deployment() == Deployment::production() => {
            "https://sync.openagents.com".into()
        }
        None => return Some(Answer::Retry),
    };
    let service = match openagents_sdk::rooms::Service::new(client, session, &base, cancel.clone())
    {
        Ok(value) => value,
        Err(error) => return Some(answer(error)),
    };
    let room = match service.room(receipt.chat.id()) {
        Ok(room) => room,
        Err(error) => return Some(answer(error)),
    };
    let device = format!(
        "terminal-{:x}",
        Sha256::digest(format!("{}\0{}", saved.account, snapshot.computer))
    );
    let transport = Arc::new(Transport {
        room,
        device: device.clone(),
    });
    let sink = Arc::new(Sink::new());
    let result = tokio::time::timeout(Duration::from_secs(300), async {
        let client = ChatClient::connect_via_transport(
            transport.clone(),
            sink.clone(),
            transport.clone(),
            &device,
            0,
            transport.clone(),
        )
        .await
        .map_err(|_| Answer::Retry)?;
        let published = async {
            while !client.caught_up() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            let updates = {
                let state = sink.0.lock().unwrap();
                if state.failed {
                    return Err(Answer::Retry);
                }
                merge(
                    &state.doc,
                    receipt.chat.id(),
                    &device,
                    &upload.room_messages,
                    snapshot
                        .active_unix
                        .unwrap_or(0)
                        .min(i64::MAX as u64 / 1000) as i64
                        * 1000,
                )
                .map_err(|message| Answer::Refused(message))?
            };
            for update in updates {
                let batch = format!("{:x}", Sha256::digest(&update));
                client.enqueue_batch(batch, update);
            }
            while client.stats().pending_pushes > 0 {
                if client.stats().rejected > 0 || sink.0.lock().unwrap().failed {
                    return Err(Answer::Retry);
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            if client.stats().rejected > 0 {
                return Err(Answer::Retry);
            }
            Ok(())
        }
        .await;
        client.shutdown().await;
        published
    })
    .await;
    cancel.cancel();
    Some(match result {
        Ok(Ok(())) => Answer::Done,
        Ok(Err(error)) => error,
        Err(_) => Answer::Retry,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_text_keeps_step_identity_and_excludes_tool_bodies_and_reasoning() {
        let mut doc = serde_json::json!({"steps":[
            {"step_id":1,"source":"user","message":"Question"},
            {"step_id":2,"source":"agent","message":"Answer","tool_calls":[
                {"tool_call_id":"file","function_name":"read_file","arguments":{"path":"private.txt"}}
            ],"observation":{"results":[{"source_call_id":"file","content":"file-only-marker"}]}}
        ]});
        let screen = secret_screen::Screen::shapes();
        let first = super::super::upload(&doc, "Fixture", &screen);
        doc["steps"][1]["reasoning_content"] = serde_json::json!("reasoning-only-marker");
        doc["steps"][1]["message"] = serde_json::json!("Answer continues");
        let next = super::super::upload(&doc, "Fixture", &screen);
        assert_eq!(first.room_messages.len(), 2);
        assert_eq!(next.room_messages.len(), 2);
        assert_eq!(first.room_messages[1].id, next.room_messages[1].id);
        assert_eq!(next.room_messages[1].text, "Answer continues");
        assert!(
            next.room_messages
                .iter()
                .all(|m| !m.text.contains("only-marker"))
        );
    }

    #[test]
    fn republishing_updates_text_without_duplicating_messages_or_removing_remote_commands() {
        let doc = SessionDoc::from_doc(loro::LoroDoc::new());
        let mut messages = vec![Message {
            id: "terminal-step-1".into(),
            user: true,
            text: "Remember violet roses".into(),
        }];
        let first = merge(&doc, "chat", "terminal", &messages, 1).unwrap();
        let reader = loro::LoroDoc::new();
        for bytes in first {
            reader.import(&bytes).unwrap();
        }
        assert_eq!(
            SessionDoc::from_doc(reader).read_entries().unwrap().len(),
            1
        );
        let command = openagents_doc::SessionCommandEntry {
            id: "phone-interrupt".into(),
            payload: openagents_doc::SessionCommandPayload::Interrupt {},
            issued_by: "phone".into(),
            issued_at: 1,
            based_on: None,
            expires_at: None,
            status: openagents_doc::SessionCommandStatus::Pending,
            resolution: None,
        };
        doc.queue_command(&command).unwrap();
        messages[0].text.push_str(" today");
        merge(&doc, "chat", "terminal", &messages, 1).unwrap();
        assert_eq!(doc.read_entries().unwrap().len(), 1);
        assert!(
            matches!(&doc.read_entries().unwrap()[0].parts[0], MessagePart::Text { text, .. } if text.ends_with("today"))
        );
        assert_eq!(doc.read_commands().unwrap(), [command]);
        assert!(
            merge(&doc, "chat", "terminal", &messages, 1)
                .unwrap()
                .is_empty()
        );
        assert!(merge(&doc, "other-chat", "terminal", &messages, 1).is_err());
        assert!(merge(&doc, "chat", "other-device", &messages, 1).is_err());
    }

    #[tokio::test]
    async fn sdk_receipt_checkpoint_and_lost_ack_keep_one_conversation_after_reconnect() {
        use axum::{
            Json, Router,
            body::Bytes,
            extract::{Query, State},
            http::HeaderMap,
            response::{IntoResponse, Response},
            routing::{get, put},
        };
        use openagents_sync::chat_frames::{encode, frame_type as frame};
        use serde_json::{Value, json};
        use std::collections::HashMap;
        const CHAT: &str = "b522f350-c806-4388-a5b5-c7fa6890a5db";
        #[derive(Default)]
        struct Server {
            rows: Mutex<Vec<(String, Vec<u8>)>>,
            denied: std::sync::atomic::AtomicBool,
            test_header: std::sync::atomic::AtomicBool,
            checkpoint: Vec<u8>,
            frontier: Vec<u8>,
            lose_ack: std::sync::atomic::AtomicBool,
        }
        async fn account(headers: HeaderMap) -> Json<Value> {
            assert!(headers.get("authorization").is_some());
            Json(json!({"account":{"id":"acct_fixture","label":"Fixture"},"workspaces":[]}))
        }
        async fn save() -> Json<Value> {
            Json(json!({"chat":CHAT,"changed":true}))
        }
        async fn rows(
            State(server): State<Arc<Server>>,
            Query(q): Query<HashMap<String, String>>,
            headers: HeaderMap,
        ) -> (axum::http::StatusCode, Vec<u8>) {
            assert!(headers.get("authorization").is_some());
            server.test_header.store(
                headers
                    .get("X-OpenAgents-Test")
                    .is_some_and(|value| value == "1"),
                std::sync::atomic::Ordering::Relaxed,
            );
            if server.denied.load(std::sync::atomic::Ordering::Relaxed) {
                return (axum::http::StatusCode::FORBIDDEN, Vec::new());
            }
            let rows = server.rows.lock().unwrap();
            let head = rows.len();
            let mut frames = vec![encode(
                frame::STATE,
                &json!({"headSeq":head,"seqFloor":0,"checkpointSeq":0,"checkpointSize":server.checkpoint.len()}),
                &server.frontier,
            )];
            let after = q.get("after").and_then(|v| v.parse().ok()).unwrap_or(0);
            for (index, (id, bytes)) in rows.iter().enumerate().skip(after) {
                frames.push(encode(
                    frame::ROW,
                    &json!({"seq":index+1,"device":"terminal-fixture","batchId":id}),
                    bytes,
                ));
            }
            frames.push(encode(frame::ROWS_DONE, &json!({"headSeq":head}), &[]));
            let mut body = Vec::new();
            for bytes in frames {
                body.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                body.extend(bytes);
            }
            (axum::http::StatusCode::OK, body)
        }
        async fn push(
            State(server): State<Arc<Server>>,
            Query(q): Query<HashMap<String, String>>,
            bytes: Bytes,
        ) -> Response {
            let batch = q.get("batchId").unwrap();
            assert!(
                q.get("device")
                    .is_some_and(|device| device.starts_with("terminal-"))
            );
            let mut rows = server.rows.lock().unwrap();
            let (seq, dup) = if let Some(index) = rows.iter().position(|(id, _)| id == batch) {
                (index + 1, true)
            } else {
                rows.push((batch.clone(), bytes.to_vec()));
                (rows.len(), false)
            };
            if server
                .lose_ack
                .swap(false, std::sync::atomic::Ordering::Relaxed)
            {
                return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
            Json(json!({"batchId":batch,"seq":seq,"dup":dup})).into_response()
        }
        let seed = SessionDoc::init(CHAT).unwrap();
        let command = openagents_doc::SessionCommandEntry {
            id: "phone-command".into(),
            payload: openagents_doc::SessionCommandPayload::Interrupt {},
            issued_by: "phone".into(),
            issued_at: 1,
            based_on: None,
            expires_at: None,
            status: openagents_doc::SessionCommandStatus::Pending,
            resolution: None,
        };
        seed.queue_command(&command).unwrap();
        let server = Arc::new(Server {
            checkpoint: seed.export_snapshot().unwrap(),
            frontier: seed.doc().oplog_vv().encode(),
            lose_ack: std::sync::atomic::AtomicBool::new(true),
            ..Server::default()
        });
        let router = Router::new()
            .route("/v1/account", get(account))
            .route("/v1/threads/synced/source-session", put(save))
            .route(&format!("/chat2/{CHAT}/rows"), get(rows).post(push))
            .route(
                &format!("/chat2/{CHAT}/checkpoint"),
                get(|State(server): State<Arc<Server>>| async move { server.checkpoint.clone() }),
            )
            .with_state(server.clone());
        let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", socket.local_addr().unwrap());
        let host = tokio::spawn(async move {
            axum::serve(socket, router).await.unwrap();
        });
        let saved: Saved = serde_json::from_value(json!({"origin":base,"account":"acct_fixture","label":"Fixture","expires_at":u64::MAX,"token":format!("sess_{}", "a".repeat(64))})).unwrap();
        let doc = json!({"session_id":"source-session","steps":[
            {"step_id":1,"source":"user","message":"Remember violet roses"},
            {"step_id":2,"source":"agent","message":"I will remember."}
        ]});
        let upload =
            super::super::upload(&doc, "Fixture terminal", &secret_screen::Screen::shapes());
        assert_eq!(
            put_with_service(&saved, "source-session", &upload, Some(base.clone())).await,
            Some(Answer::Done)
        );
        let count = server.rows.lock().unwrap().len();
        assert!(count > 0);
        assert_eq!(
            put_with_service(&saved, "source-session", &upload, Some(base.clone())).await,
            Some(Answer::Done)
        );
        assert_eq!(server.rows.lock().unwrap().len(), count);
        let shared = loro::LoroDoc::new();
        shared.import(&server.checkpoint).unwrap();
        for (_, bytes) in server.rows.lock().unwrap().iter() {
            shared.import(bytes).unwrap();
        }
        let shared = SessionDoc::from_doc(shared);
        assert_eq!(shared.chat_id().as_deref(), Some(CHAT));
        assert_eq!(shared.read_entries().unwrap().len(), 2);
        assert_eq!(shared.read_commands().unwrap(), [command]);
        let sdk = Client::with_test_run(
            Deployment::new(&format!("{base}/v1"), &base).unwrap(),
            Duration::from_secs(20),
            true,
        )
        .unwrap();
        let session = sdk.authenticate(saved.token()).await.unwrap();
        for untrusted in [
            "http://example.invalid",
            "https://example.invalid/?token=hidden",
            "https://user@example.invalid",
        ] {
            assert!(
                openagents_sdk::rooms::Service::new(
                    sdk.clone(),
                    session.clone(),
                    untrusted,
                    CancellationToken::new()
                )
                .is_err()
            );
        }
        let stop = CancellationToken::new();
        let service =
            openagents_sdk::rooms::Service::new(sdk, session, &base, stop.clone()).unwrap();
        let room = service.room(CHAT).unwrap();
        assert!(service.room("../other-account").is_err());
        assert!(matches!(
            room.push("terminal", "../other-batch", vec![1]).await,
            Err(Error::InvalidReference)
        ));
        server
            .denied
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(matches!(room.rows(0).await, Err(Error::Forbidden)));
        assert!(
            server
                .test_header
                .load(std::sync::atomic::Ordering::Relaxed)
        );
        stop.cancel();
        assert!(matches!(room.rows(0).await, Err(Error::Cancelled)));
        assert!(matches!(room.socket("terminal"), Err(Error::Cancelled)));
        host.abort();
        let _ = host.await;
    }
}
