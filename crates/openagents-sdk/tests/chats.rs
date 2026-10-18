use axum::{
    Json, Router,
    extract::Path,
    extract::{Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use openagents_sdk::{
    CancellationToken, Client, Deployment, Error, Session,
    chats::{self, Event, RefusalCode, Send},
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
const CHAT: &str = "d75ef8a1-16ca-49b6-83be-000000000093";
const OTHER: &str = "d75ef8a1-16ca-49b6-83be-000000000094";
#[derive(Default)]
struct Server {
    lost: AtomicBool,
    mode: AtomicUsize,
    attempts: AtomicUsize,
    ids: Mutex<HashSet<String>>,
    patches: Mutex<Vec<Value>>,
    filing: Mutex<Value>,
}
async fn send(State(s): State<Arc<Server>>, Json(body): Json<Value>) -> Response {
    s.attempts.fetch_add(1, Ordering::SeqCst);
    s.ids
        .lock()
        .unwrap()
        .insert(body["request_id"].as_str().unwrap().to_owned());
    if s.lost.swap(false, Ordering::SeqCst) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":{"code":"unavailable","message":"Synthetic lost response"}})),
        )
            .into_response();
    }
    (StatusCode::ACCEPTED, Json(json!({"answering":true}))).into_response()
}
async fn events(
    State(s): State<Arc<Server>>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let id = if s.mode.load(Ordering::SeqCst) == 2 {
        OTHER
    } else {
        &query["request_id"]
    };
    let values = if s.mode.load(Ordering::SeqCst) == 3 {
        vec![
            json!({"type":"error","request_id":id,"error":{"code":"quota_exhausted","message":"Try again later.","retry_after_ms":500}}),
        ]
    } else {
        vec![
            json!({"type":"text","request_id":id,"index":1,"role":"assistant","text":"Crab 🦀","replace":true}),
            json!({"type":"tool","request_id":id,"index":2,"role":"tool","text":"Done","replace":true}),
            json!({"type":"cards","request_id":id,"cards":{"cards":[{"card":"video_lab","title":"A result"}]}}),
            json!({"type":"done","request_id":id,"outcome":"answered","revision":2}),
        ]
    };
    let text = values
        .into_iter()
        .map(|v| {
            format!(
                "event: {}\r\ndata: {v}\r\n\r\n",
                v["type"].as_str().unwrap()
            )
        })
        .collect::<String>();
    let chunks = text
        .into_bytes()
        .chunks(3)
        .map(|b| Ok::<_, Infallible>(axum::body::Bytes::copy_from_slice(b)))
        .collect::<Vec<_>>();
    (
        [(header::CONTENT_TYPE, "text/event-stream")],
        axum::body::Body::from_stream(futures_util::stream::iter(chunks)),
    )
        .into_response()
}
async fn file(
    State(s): State<Arc<Server>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    s.patches.lock().unwrap().push(body.clone());
    let mut filing = s.filing.lock().unwrap();
    for name in ["pinned", "archived"] {
        if let Some(value) = body.get(name) {
            filing[name] = value.clone();
        }
    }
    let id = if s.mode.load(Ordering::SeqCst) == 2 {
        OTHER.to_owned()
    } else {
        id
    };
    Json(json!({"id": id, "object": "thread", "pinned": filing["pinned"], "archived": filing["archived"]}))
        .into_response()
}
struct Fixture {
    client: Client,
    session: Session,
    server: Arc<Server>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn fixture() -> Fixture {
    let server = Arc::new(Server {
        filing: Mutex::new(json!({"pinned": false, "archived": false})),
        ..Server::default()
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app=Router::new().route("/v1/account",get(|h:axum::http::HeaderMap|async move{Json(json!({"account":{"id":if h[header::AUTHORIZATION].to_str().unwrap().ends_with('2'){"two"}else{"one"},"label":"Synthetic account"},"workspaces":[]}))}))
        .route("/v1/threads/{id}/messages",post(send)).route("/v1/threads/{id}/events",get(events))
        .route("/v1/threads/{id}",axum::routing::patch(file))
        .route("/v1/threads/{id}/stop",post(|Json(body):Json<Value>|async move{Json(json!({"request_id":body["request_id"],"stopped":true,"finished":true}))})).with_state(server.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let session = client
        .authenticate(&format!("oa_sk_{}", "1".repeat(64)))
        .await
        .unwrap();
    Fixture {
        client,
        session,
        server,
        task,
    }
}
#[tokio::test]
async fn lost_response_reuses_identity_and_streams_fragmented_unicode_tools_and_cards() {
    let f = fixture().await;
    let cancel = CancellationToken::new();
    let send = Send::new(f.client.chat(&f.session, CHAT).unwrap(), "Hello".into()).unwrap();
    f.server.lost.store(true, Ordering::SeqCst);
    assert!(matches!(
        chats::send(&f.client, &f.session, &send, &cancel).await,
        Err(Error::Unavailable { .. })
    ));
    let retained = Send::with_id(
        send.chat().clone(),
        send.request_id().to_owned(),
        send.text().to_owned(),
    )
    .unwrap();
    chats::send(&f.client, &f.session, &retained, &cancel)
        .await
        .unwrap();
    assert_eq!(f.server.ids.lock().unwrap().len(), 1);
    assert_eq!(f.server.attempts.load(Ordering::SeqCst), 2);
    let mut stream = chats::stream(&f.client, &f.session, &send.turn(), &cancel)
        .await
        .unwrap();
    assert!(
        matches!(stream.next().await.unwrap(),Some(Event::Text{text,replace:true,..}) if text=="Crab 🦀")
    );
    assert!(matches!(
        stream.next().await.unwrap(),
        Some(Event::Tool { .. })
    ));
    assert!(matches!(
        stream.next().await.unwrap(),
        Some(Event::Cards { .. })
    ));
    assert!(matches!(
        stream.next().await.unwrap(),
        Some(Event::Done { .. })
    ));
    assert!(stream.next().await.unwrap().is_none());
    assert!(
        chats::stop(&f.client, &f.session, &send.turn(), &cancel)
            .await
            .unwrap()
            .stopped
    );
}
#[tokio::test]
async fn account_binding_cancellation_request_isolation_and_typed_refusals() {
    let f = fixture().await;
    let cancel = CancellationToken::new();
    let send = Send::new(f.client.chat(&f.session, CHAT).unwrap(), "Hello".into()).unwrap();
    let other = f
        .client
        .authenticate(&format!("oa_sk_{}", "2".repeat(64)))
        .await
        .unwrap();
    assert!(matches!(
        chats::send(&f.client, &other, &send, &cancel).await,
        Err(Error::WrongAccount)
    ));
    assert_eq!(f.server.attempts.load(Ordering::SeqCst), 0);
    f.server.mode.store(2, Ordering::SeqCst);
    let mut stream = chats::stream(&f.client, &f.session, &send.turn(), &cancel)
        .await
        .unwrap();
    assert!(matches!(stream.next().await, Err(Error::InvalidResponse)));
    f.server.mode.store(3, Ordering::SeqCst);
    let mut stream = chats::stream(&f.client, &f.session, &send.turn(), &cancel)
        .await
        .unwrap();
    assert!(
        matches!(stream.next().await.unwrap(),Some(Event::Error{error,..}) if error.code==RefusalCode::QuotaExhausted)
    );
    f.server.mode.store(0, Ordering::SeqCst);
    let mut stream = chats::stream(&f.client, &f.session, &send.turn(), &cancel)
        .await
        .unwrap();
    cancel.cancel();
    assert!(matches!(stream.next().await, Err(Error::Cancelled)));
    assert!(matches!(
        chats::send(&f.client, &f.session, &send, &cancel).await,
        Err(Error::Cancelled)
    ));
    assert_eq!(f.server.attempts.load(Ordering::SeqCst), 0);
    let unknown: RefusalCode = serde_json::from_value(json!("future_refusal")).unwrap();
    assert_eq!(unknown, RefusalCode::Other("future_refusal".into()));
}
#[tokio::test]
async fn pins_and_archives_the_chat_and_checks_the_answer() {
    let f = fixture().await;
    let cancel = CancellationToken::new();
    let chat = f.client.chat(&f.session, CHAT).unwrap();
    let filing = chats::set_pinned(&f.client, &f.session, &chat, true, &cancel)
        .await
        .unwrap();
    assert_eq!(
        filing,
        chats::Filing {
            pinned: true,
            archived: false
        }
    );
    let filing = chats::set_archived(&f.client, &f.session, &chat, true, &cancel)
        .await
        .unwrap();
    assert!(filing.pinned && filing.archived);
    let filing = chats::set_pinned(&f.client, &f.session, &chat, false, &cancel)
        .await
        .unwrap();
    assert!(!filing.pinned && filing.archived);
    assert_eq!(
        *f.server.patches.lock().unwrap(),
        [
            json!({"pinned": true}),
            json!({"archived": true}),
            json!({"pinned": false})
        ]
    );
    // Another account's session never sends; an answer for another chat is refused.
    let other = f
        .client
        .authenticate(&format!("oa_sk_{}", "2".repeat(64)))
        .await
        .unwrap();
    assert!(matches!(
        chats::set_archived(&f.client, &other, &chat, false, &cancel).await,
        Err(Error::WrongAccount)
    ));
    assert_eq!(f.server.patches.lock().unwrap().len(), 3);
    f.server.mode.store(2, Ordering::SeqCst);
    assert!(matches!(
        chats::set_pinned(&f.client, &f.session, &chat, true, &cancel).await,
        Err(Error::InvalidResponse)
    ));
}
