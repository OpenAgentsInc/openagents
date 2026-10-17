//! Account-authorized JSON events and request-specific cancellation for cloud chats.
use crate::{
    App,
    chat_store::{Loaded, Outcome},
    coder_sync::{answer, refused, stored},
};
use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    convert::Infallible,
    time::Duration,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct TurnError {
    pub code: String,
    pub message: String,
    pub retry_after_ms: Option<u64>,
}
impl TurnError {
    pub(crate) fn from_failure(failure: &openagents_chat::basic_coder::Failure) -> Self {
        use openagents_chat::basic_coder::Failure;
        let (code, retry_after_ms) = match failure {
            Failure::Refused {
                code,
                retry_after_ms,
                ..
            } if !code.is_empty()
                && code.len() <= 128
                && code
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c)) =>
            {
                (code.as_str(), *retry_after_ms)
            }
            Failure::Silent => ("timeout", None),
            _ => ("unavailable", None),
        };
        Self {
            code: code.to_owned(),
            message: failure.describe(),
            retry_after_ms,
        }
    }
    fn unavailable() -> Self {
        Self {
            code: "unavailable".into(),
            message: "We couldn't get an answer. Try again.".into(),
            retry_after_ms: None,
        }
    }
}
/// Only person-visible card bodies cross the public event/history boundary.
pub(crate) fn display_cards(meta: &openagents_chat::router::Meta) -> Value {
    let cards: Vec<_> = meta
        .cards
        .iter()
        .take(16)
        .map(|card| {
            let mut card = card.clone();
            if let Some(object) = card.as_object_mut() {
                object.remove("_lab");
            }
            card
        })
        .collect();
    json!({"cards":cards})
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestId {
    request_id: String,
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/threads/{id}/events", get(events))
        .route("/v1/threads/{id}/stop", post(stop))
}
async fn load(app: &App, headers: &HeaderMap, id: &str) -> Result<(String, Loaded), Response> {
    // A signed-out app's own chats too (#11978).
    let (caller, visitor) = crate::visitor_api::owner(app, headers).await?;
    if !crate::chat_store::valid_id(id) {
        return Err(refused(StatusCode::NOT_FOUND, "not_found", "No such chat."));
    }
    let loaded = match app.config.chat_store.load(&caller, id).await {
        Ok(Some(v)) if !v.conversation.deleted() => v,
        Ok(_) if visitor => {
            return Err(refused(StatusCode::NOT_FOUND, "not_found", "No such chat."));
        }
        Ok(_) => match crate::projects::api::shared_thread(app, headers, &caller, id).await {
            Some((v, _)) => v,
            None => return Err(refused(StatusCode::NOT_FOUND, "not_found", "No such chat.")),
        },
        Err(e) => return Err(stored(&e)),
    };
    if loaded.conversation.terminal.is_some() {
        return Err(refused(
            StatusCode::CONFLICT,
            "computer_chat",
            "Continue this chat on its computer.",
        ));
    }
    Ok((caller, loaded))
}
async fn stop(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: axum::body::Bytes,
) -> Response {
    let request = match serde_json::from_slice::<RequestId>(&body) {
        Ok(v) if crate::chat_store::valid_id(&v.request_id) => v,
        _ => {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid",
                "Send the message's request ID.",
            );
        }
    };
    for _ in 0..6 {
        let (caller, loaded) = match load(&app, &headers, &id).await {
            Ok(v) => v,
            Err(r) => return r,
        };
        let chat = &loaded.conversation;
        let Some(sent) = chat.requests.iter().find(|r| r.id == request.request_id) else {
            return refused(StatusCode::NOT_FOUND, "not_found", "No such message.");
        };
        // A shared member may stop only their own currently pending turn.
        if chat.owner != caller
            && chat.pending.as_ref().and_then(|p| p.payer.as_deref()) != Some(caller.as_str())
        {
            return refused(
                StatusCode::FORBIDDEN,
                "project_role",
                "Only the sender can stop this answer.",
            );
        }
        if sent.outcome != Outcome::Pending {
            return answer(
                StatusCode::OK,
                json!({"request_id":request.request_id,"stopped":sent.outcome==Outcome::Stopped,"finished":true}),
            );
        }
        if chat
            .pending
            .as_ref()
            .is_none_or(|p| p.request_id != request.request_id)
        {
            return refused(
                StatusCode::CONFLICT,
                "unsupported_turn",
                "Stop this work on openagents.com.",
            );
        }
        let next = crate::pages::chat::stopped(chat);
        match app.config.chat_store.compare_and_swap(&loaded, &next).await {
            Ok(_) => {
                let _ = app
                    .config
                    .chat_store
                    .release(&chat.owner, &request.request_id)
                    .await;
                return answer(
                    StatusCode::OK,
                    json!({"request_id":request.request_id,"stopped":true,"finished":true}),
                );
            }
            Err(crate::chat_store::Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    refused(
        StatusCode::CONFLICT,
        "changed",
        "This chat just changed. Try again.",
    )
}
struct Stream {
    app: App,
    headers: HeaderMap,
    chat: String,
    request: String,
    previous: BTreeMap<usize, String>,
    cards: Option<Value>,
    queue: VecDeque<Value>,
    revision: u64,
    sequence: u64,
    ended: bool,
    ticks: u16,
}
impl Stream {
    fn update(&mut self, loaded: &Loaded) {
        let chat = &loaded.conversation;
        self.revision = chat.revision;
        for (index, message) in chat
            .messages
            .iter()
            .enumerate()
            .filter(|(_, m)| m.request_id.as_deref() == Some(&self.request))
        {
            let previous = self.previous.get(&index);
            if previous.is_some_and(|p| p == &message.text) {
                continue;
            }
            let (text, replace) = match previous.and_then(|p| message.text.strip_prefix(p)) {
                Some(delta) => (delta, false),
                None => (message.text.as_str(), true),
            };
            self.queue.push_back(json!({"type":if message.role==crate::chat_store::Role::Tool {"tool"} else {"text"},"request_id":self.request,"index":index,"role":message.role,"text":text,"replace":replace}));
            self.previous.insert(index, message.text.clone());
        }
        let Some(request) = chat.requests.iter().find(|r| r.id == self.request) else {
            self.queue.push_back(json!({"type":"error","request_id":self.request,"error":{"code":"not_found","message":"This message is no longer available.","retry_after_ms":null}}));
            self.ended = true;
            return;
        };
        if let Some(meta) = request
            .reply
            .as_ref()
            .filter(|m| !m.cards.is_empty())
            .map(display_cards)
        {
            if self.cards.as_ref() != Some(&meta) {
                self.queue
                    .push_back(json!({"type":"cards","request_id":self.request,"cards":meta}));
                self.cards = Some(meta);
            }
        }
        if request.outcome != Outcome::Pending {
            if matches!(request.outcome, Outcome::Failed | Outcome::Unknown) {
                self.queue.push_back(json!({"type":"error","request_id":self.request,"error":request.turn_error.clone().unwrap_or_else(TurnError::unavailable)}));
            } else {
                self.queue.push_back(json!({"type":"done","request_id":self.request,"outcome":request.outcome,"revision":chat.revision}));
            }
            self.ended = true;
        }
    }
}
async fn events(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    query: Result<Query<RequestId>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(_) => {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid",
                "Send the message's request ID.",
            );
        }
    };
    if !crate::chat_store::valid_id(&query.request_id) {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send the message's request ID.",
        );
    }
    let (_, loaded) = match load(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if !loaded
        .conversation
        .requests
        .iter()
        .any(|r| r.id == query.request_id)
    {
        return refused(StatusCode::NOT_FOUND, "not_found", "No such message.");
    }
    let mut state = Stream {
        app,
        headers,
        chat: id,
        request: query.request_id,
        previous: BTreeMap::new(),
        cards: None,
        queue: VecDeque::new(),
        revision: 0,
        sequence: 0,
        ended: false,
        ticks: 0,
    };
    state.update(&loaded);
    let shutdown = state.app.config.shutdown.clone();
    let stream = futures_util::stream::unfold(state, |mut s| async move {
        loop {
            if let Some(mut value) = s.queue.pop_front() {
                s.sequence += 1;
                value["revision"] = json!(s.revision);
                let event = Event::default()
                    .event(value["type"].as_str().unwrap())
                    .id(format!("{}:{}:{}", s.request, s.revision, s.sequence))
                    .data(value.to_string());
                return Some((Ok::<_, Infallible>(event), s));
            }
            if s.ended {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
            s.ticks += 1;
            if s.ticks >= 1200 {
                s.queue.push_back(json!({"type":"error","request_id":s.request,"error":{"code":"stream_timeout","message":"Reconnect to keep following this answer.","retry_after_ms":0}}));
                s.ended = true;
                continue;
            }
            match load(&s.app, &s.headers, &s.chat).await {
                Ok((_, v)) if v.conversation.revision != s.revision => s.update(&v),
                Ok(_) => {}
                Err(_) => {
                    s.queue.push_back(json!({"type":"error","request_id":s.request,"error":{"code":"access_lost","message":"Reopen this chat to check access.","retry_after_ms":null}}));
                    s.ended = true;
                }
            }
        }
    });
    let mut response = Sse::new(shutdown.until(stream))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}

#[cfg(test)]
#[path = "chat_api_tests.rs"]
mod tests;
