use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use openagents_sdk::{
    CancellationToken, ChatSource, Client, Deployment, Error, MessageRole, Session, ToolStatus,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

fn key(n: u8) -> String {
    format!("oa_sk_{}", n.to_string().repeat(64))
}
fn id(n: usize) -> String {
    format!("{n:08x}-1234-4234-8234-123456789abc")
}
const FILE: &str = "0123456789abcdef0123456789abcdef";
#[derive(Default)]
struct Control {
    mode: AtomicUsize,
    reads: AtomicUsize,
    stolen: AtomicUsize,
    started: tokio::sync::Notify,
}
impl Control {
    fn mode(&self, n: usize) {
        self.mode.store(n, Ordering::SeqCst)
    }
}
fn denied(mode: usize) -> Option<Response> {
    let (status, code) = match mode {
        4 => (404, "not_found"),
        5 => (503, "unavailable"),
        6 => (401, "signed_out"),
        14 => (403, "denied"),
        _ => return None,
    };
    Some(
        (
            StatusCode::from_u16(status).unwrap(),
            Json(json!({"error":{"code":code,"message":key(1)}})),
        )
            .into_response(),
    )
}
async fn before(c: &Control, headers: &HeaderMap) -> usize {
    assert!(
        headers
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("Bearer oa_sk_")
    );
    c.reads.fetch_add(1, Ordering::SeqCst);
    c.started.notify_one();
    let mode = c.mode.load(Ordering::SeqCst);
    if mode == 1 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
    mode
}
fn summary(n: usize, messages: usize) -> Value {
    json!({"id":id(n),"title":"Duplicate title","revision":1,"updated_unix":n+1,"pinned_unix":if n==204{Some(10)}else{None},"archived_unix":if n==5{Some(1)}else{None},"project":"prj_0000000000000001","source":match n%3{0=>"web",1=>"terminal",_=>"phone"},"messages":messages})
}
async fn list(
    State(c): State<Arc<Control>>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let mode = before(&c, &headers).await;
    if let Some(r) = denied(mode) {
        return r;
    }
    let limit = q.get("limit").unwrap().parse::<usize>().unwrap();
    assert!((1..=200).contains(&limit));
    let start = q
        .get("after")
        .map(|s| usize::from_str_radix(&s[..8], 16).unwrap() + 1)
        .unwrap_or(0);
    let end = (start + limit).min(205);
    let data: Vec<_> = (start..end)
        .map(|n| summary(if mode == 8 { 0 } else { n }, 405))
        .filter(|row| {
            mode == 16
                || q.get("query").is_none_or(|query| {
                    row["title"]
                        .as_str()
                        .unwrap()
                        .to_lowercase()
                        .contains(&query.to_lowercase())
                })
        })
        .collect();
    let next = if mode == 3 {
        Some(id(99))
    } else {
        (end < 205).then(|| id(end - 1))
    };
    Json(json!({"data":data,"next":next})).into_response()
}
async fn messages(
    State(c): State<Arc<Control>>,
    headers: HeaderMap,
    Path(chat): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    assert_eq!(chat, id(0));
    let mode = before(&c, &headers).await;
    if let Some(r) = denied(mode) {
        return r;
    }
    if mode == 2 && q.contains_key("after") {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error":{"code":"changed"}})),
        )
            .into_response();
    }
    let start = q
        .get("after")
        .map(|s| s.strip_prefix("msg_").unwrap().parse::<usize>().unwrap() + 1)
        .unwrap_or(0);
    if start > 0 {
        assert_eq!(q.get("revision").unwrap(), "1");
    }
    let end = (start + q["limit"].parse::<usize>().unwrap()).min(405);
    let data:Vec<_>=(start..end).map(|n| json!({"id":format!("msg_{n}"),"role":match n{0=>"user",1=>"tool",2=>"thinking",3=>"future-role",_=>"assistant"},
        "text":if n==0 && mode==10{"x".repeat(2*1024*1024)}else{format!("Saved {n}")},
        "files":if n==0{vec![json!({"id":if mode==9{"../unsafe"}else{FILE},"name":"fixture.bin","kind":"future-file","size":8,"url":"https://other.example/steal"})]}else{vec![]},
        "tool":if n==1{Some(json!({"name":"read","summary":"Read fixture","status":"future-status","input":"fixture.txt","output":"Full saved output"}))}else{None},
        "future_content":{"kind":"new-block","value":n},"generated_files":[],"artifacts":[],
        "cards":if n==4{vec![json!({"schema":"lab.card.v1","kind":"echo","document":"example","version":1,"title":"Echo","view":"future_view","space":"me"})]}else if n==5{vec![json!({"schema":"lab.card.v1","kind":"cad","document":"plate","version":2,"title":"Plate","view":"model3d","primary":"model","space":"project:prj_0000000000000001"})]}else{vec![]}})).collect();
    Json(json!({"thread":summary(0,405),"revision":1,"data":data,"next":if mode==12{None}else{(end<405).then(||format!("msg_{}",end-1))},
        "captures":[{"id":id(555),"name":"Screenshot","state":"saved","size":3,"media":"text/plain"}]})).into_response()
}
async fn bytes(State(c): State<Arc<Control>>, headers: HeaderMap) -> Response {
    let mode = before(&c, &headers).await;
    if let Some(r) = denied(mode) {
        return r;
    }
    if mode == 7 {
        return (StatusCode::TEMPORARY_REDIRECT, [("location", "/stolen")]).into_response();
    }
    if mode == 11 {
        return vec![b'x'; 9].into_response();
    }
    b"fixture!".to_vec().into_response()
}
async fn fixture() -> (Client, Session, Arc<Control>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let c = Arc::new(Control::default());
    let stolen = c.clone();
    let site=Router::new().route("/v1/account",get(|headers:HeaderMap|async move {
        let other=headers.get("authorization").unwrap().to_str().unwrap().contains(&key(2));
        Json(json!({"account":{"id":if other{"acct_two"}else{"acct_one"},"label":"Fixture"},"workspaces":[]}))
    })).route("/v1/history",get(list)).route("/v1/history/projects",get(||async{Json(json!({"data":[{"id":"prj_0000000000000001","name":"Shared","shared":true}],"next":null}))}))
        .route("/v1/history/{id}/messages",get(messages)).route("/v1/history/{id}/files/{file}",get(bytes))
        .route("/v1/history/{id}/captures/{capture}",get(||async{b"cap".to_vec()}))
        .route("/stolen",get(move || {stolen.stolen.fetch_add(1,Ordering::SeqCst);async{"bad"}})).with_state(c.clone());
    tokio::spawn(async move { axum::serve(listener, site).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let session = client.authenticate(&key(1)).await.unwrap();
    (client, session, c)
}

#[tokio::test]
async fn complete_owned_and_shared_pages_merge_by_id_and_sort_pins_archives() {
    let (client, session, _) = fixture().await;
    let cancel = CancellationToken::new();
    let mut index = client.history_index(&session).unwrap();
    let mut reader = client
        .history(&session)
        .unwrap()
        .with_page_size(200)
        .unwrap();
    let first = reader
        .next_page(&client, &session, &cancel)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.data.len(), 200);
    assert!(first.has_more);
    index.extend(first.data).unwrap();
    let second = reader
        .next_page(&client, &session, &cancel)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.data.len(), 5);
    assert!(!second.has_more);
    index.extend(second.data).unwrap();
    assert!(
        reader
            .next_page(&client, &session, &cancel)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(index.len(), 205);
    assert_eq!(index.sorted(false)[0].chat.id(), id(204));
    assert_eq!(index.sorted(true)[0].chat.id(), id(5));
    assert_eq!(index.get(&id(1)).unwrap().source, ChatSource::Terminal);
    assert_eq!(index.get(&id(2)).unwrap().source, ChatSource::Phone);
    let mut projects = client.history_projects(&session).unwrap();
    let project = projects
        .next_page(&client, &session, &cancel)
        .await
        .unwrap()
        .unwrap()
        .data
        .remove(0);
    let mut shared = client.shared_history(&session, &project).unwrap();
    while let Some(page) = shared.next_page(&client, &session, &cancel).await.unwrap() {
        index.extend(page.data).unwrap();
    }
    assert_eq!(index.len(), 205);
    index.clear();
    assert!(index.is_empty());
}

#[tokio::test]
async fn complete_transcripts_keep_unknown_roles_tools_content_and_safe_saved_bytes() {
    let (client, session, c) = fixture().await;
    let cancel = CancellationToken::new();
    let chat = client.chat(&session, &id(0)).unwrap();
    let mut reader = client
        .transcript(&session, &chat)
        .unwrap()
        .with_page_size(200)
        .unwrap();
    let first = reader
        .next_page(&client, &session, &cancel)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.data.len(), 200);
    assert_eq!(
        first.data[1].tool.as_ref().unwrap().status,
        ToolStatus::Other("future-status".into())
    );
    assert_eq!(first.data[2].role, MessageRole::Thinking);
    assert_eq!(first.data[3].role, MessageRole::Other("future-role".into()));
    assert_eq!(first.data[4].extra["future_content"]["value"], 4);
    assert!(first.data[0].cards.is_empty());
    assert_eq!(
        first.data[4].cards[0].card.view,
        openagents_sdk::labs::View::Report
    );
    assert_eq!(
        first.data[5].cards[0].card.view,
        openagents_sdk::labs::View::Model3d
    );
    assert_eq!(
        first.data[5].cards[0].document_ref().space(),
        "project:prj_0000000000000001"
    );
    assert_eq!(
        first.data[5].cards[0].document_ref().account(),
        session.account().id
    );
    let download = first.data[0].files[0].download_ref().unwrap();
    assert_eq!(
        client.download(&session, download, &cancel).await.unwrap(),
        b"fixture!"
    );
    assert_eq!(
        client
            .download(&session, first.captures[0].download_ref().unwrap(), &cancel)
            .await
            .unwrap(),
        b"cap"
    );
    assert_eq!(
        reader
            .next_page(&client, &session, &cancel)
            .await
            .unwrap()
            .unwrap()
            .data[0]
            .id,
        "msg_200"
    );
    let last = reader
        .next_page(&client, &session, &cancel)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(last.data.len(), 5);
    assert!(!last.has_more);
    assert!(
        reader
            .next_page(&client, &session, &cancel)
            .await
            .unwrap()
            .is_none()
    );
    c.mode(7);
    assert!(matches!(
        client.download(&session, download, &cancel).await,
        Err(Error::Redirect)
    ));
    assert_eq!(c.stolen.load(Ordering::SeqCst), 0);
    c.mode(11);
    assert!(matches!(
        client.download(&session, download, &cancel).await,
        Err(Error::TooLarge)
    ));
    c.mode(4);
    assert!(matches!(
        client.download(&session, download, &cancel).await,
        Err(Error::NotFound)
    ));
    c.mode(14);
    assert!(matches!(
        client.download(&session, download, &cancel).await,
        Err(Error::Forbidden)
    ));
    c.mode(9);
    let mut invalid = client.transcript(&session, &chat).unwrap();
    let page = invalid
        .next_page(&client, &session, &cancel)
        .await
        .unwrap()
        .unwrap();
    assert!(page.data[0].files[0].download_ref().is_none());
}

#[tokio::test]
async fn changed_invalid_or_truncated_pages_never_finish_silently_or_loop() {
    let (client, session, c) = fixture().await;
    let cancel = CancellationToken::new();
    let chat = client.chat(&session, &id(0)).unwrap();
    let mut reader = client.transcript(&session, &chat).unwrap();
    reader.next_page(&client, &session, &cancel).await.unwrap();
    c.mode(2);
    assert!(matches!(
        reader.next_page(&client, &session, &cancel).await,
        Err(Error::Changed)
    ));
    let requests = c.reads.load(Ordering::SeqCst);
    assert!(matches!(
        reader.next_page(&client, &session, &cancel).await,
        Err(Error::Changed)
    ));
    assert_eq!(c.reads.load(Ordering::SeqCst), requests);
    c.mode(3);
    let mut list = client.history(&session).unwrap();
    list.next_page(&client, &session, &cancel).await.unwrap();
    assert!(matches!(
        list.next_page(&client, &session, &cancel).await,
        Err(Error::InvalidCursor)
    ));
    c.mode(8);
    let mut list = client.history(&session).unwrap();
    assert!(matches!(
        list.next_page(&client, &session, &cancel).await,
        Err(Error::InvalidResponse)
    ));
    c.mode(12);
    let mut reader = client.transcript(&session, &chat).unwrap();
    assert!(matches!(
        reader.next_page(&client, &session, &cancel).await,
        Err(Error::InvalidCursor)
    ));
}

#[tokio::test]
async fn account_switch_revocation_and_cancellation_cannot_adopt_old_pages() {
    let (client, session, c) = fixture().await;
    let cancel = CancellationToken::new();
    let mut reader = client.history(&session).unwrap();
    let other = client.authenticate(&key(2)).await.unwrap();
    let before = c.reads.load(Ordering::SeqCst);
    assert!(matches!(
        reader.next_page(&client, &other, &cancel).await,
        Err(Error::WrongAccount)
    ));
    assert_eq!(c.reads.load(Ordering::SeqCst), before);
    let foreign =
        Client::new(Deployment::new("http://127.0.0.1:1/v1", "http://127.0.0.1:1").unwrap())
            .unwrap();
    assert!(matches!(
        reader.next_page(&foreign, &session, &cancel).await,
        Err(Error::WrongDeployment)
    ));
    cancel.cancel();
    assert!(matches!(
        reader.next_page(&client, &session, &cancel).await,
        Err(Error::Cancelled)
    ));
    assert_eq!(c.reads.load(Ordering::SeqCst), before);
    let cancel = CancellationToken::new();
    c.mode(1);
    let task_cancel = cancel.clone();
    let task_client = client.clone();
    let task_session = session.clone();
    let work = tokio::spawn(async move {
        let result = reader
            .next_page(&task_client, &task_session, &task_cancel)
            .await;
        (result, reader)
    });
    c.started.notified().await;
    cancel.cancel();
    let (result, mut reader) = work.await.unwrap();
    assert!(matches!(result, Err(Error::Cancelled)));
    c.mode(0);
    let page = reader
        .next_page(&client, &session, &CancellationToken::new())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page.data[0].chat.id(), id(0));
    let mut index = client.history_index(&other).unwrap();
    assert!(matches!(index.extend(page.data), Err(Error::WrongAccount)));
    assert!(index.is_empty());
    for (mode, expected) in [
        (4, Error::NotFound),
        (5, Error::Unavailable { retry_after: None }),
        (6, Error::Unauthorized),
    ] {
        c.mode(mode);
        let mut reader = client.history(&session).unwrap();
        let error = reader
            .next_page(&client, &session, &CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?}").contains(&key(1)));
    }
}

#[tokio::test]
async fn saved_text_above_auth_response_bound_is_not_truncated() {
    let (client, session, c) = fixture().await;
    c.mode(10);
    let chat = client.chat(&session, &id(0)).unwrap();
    let mut reader = client.transcript(&session, &chat).unwrap();
    let page = reader
        .next_page(&client, &session, &CancellationToken::new())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page.data[0].text.len(), 2 * 1024 * 1024);
    assert!(client.chat(&session, "../unsafe").is_err());
    assert!(
        client
            .history(&session)
            .unwrap()
            .with_page_size(201)
            .is_err()
    );
}

#[tokio::test]
async fn saved_title_search_keeps_scan_cursor_through_empty_pages() {
    let (client, session, control) = fixture().await;
    let cancel = CancellationToken::new();
    let mut reader = client
        .history(&session)
        .unwrap()
        .with_page_size(200)
        .unwrap()
        .with_query("Absent")
        .unwrap();
    let first = reader
        .next_page(&client, &session, &cancel)
        .await
        .unwrap()
        .unwrap();
    assert!(first.data.is_empty() && first.has_more);
    let last = reader
        .next_page(&client, &session, &cancel)
        .await
        .unwrap()
        .unwrap();
    assert!(last.data.is_empty() && !last.has_more);
    assert_eq!(control.reads.load(Ordering::SeqCst), 2);
    let mut reader = reader.with_query(" DUPLICATE ").unwrap();
    assert_eq!(
        reader
            .next_page(&client, &session, &cancel)
            .await
            .unwrap()
            .unwrap()
            .data
            .len(),
        200
    );
    assert!(
        client
            .history(&session)
            .unwrap()
            .with_query(&"q".repeat(201))
            .is_err()
    );
}

#[tokio::test]
async fn saved_search_refuses_a_server_that_ignored_the_query() {
    let (client, session, control) = fixture().await;
    control.mode(16);
    let mut reader = client
        .history(&session)
        .unwrap()
        .with_query("Absent")
        .unwrap();
    assert!(matches!(
        reader
            .next_page(&client, &session, &CancellationToken::new())
            .await,
        Err(Error::InvalidResponse)
    ));
}
