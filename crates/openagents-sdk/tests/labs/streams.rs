use super::*;
use sha2::Digest;
use std::sync::atomic::AtomicUsize;

async fn serve(router: Router) -> (Client, tokio::task::JoinHandle<()>) {
    let router=router.route("/v1/account",get(|headers:HeaderMap| async move { axum::Json(json!({"account":{"id":account(&headers).unwrap(),"label":"Synthetic"},"workspaces":[]})) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (
        Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap(),
        task,
    )
}
fn event(cursor: u64) -> labs::Event {
    labs::Event {
        cursor,
        document: "design".into(),
        at_unix_ms: cursor,
        change: json!({"text":"Synthetic café"}),
    }
}
fn frame(cursor: u64) -> String {
    format!(
        "id: {cursor}\r\nevent: changes\r\ndata: {{\"data\":\r\ndata: [{}]}}\r\n\r\n",
        serde_json::to_string(&event(cursor)).unwrap()
    )
}

#[tokio::test]
async fn events_reconnect_after_eof_with_delivered_cursor_and_split_utf8_frames() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (counter, record) = (calls.clone(), seen.clone());
    let router = Router::new().route(
        "/v1/labs/documents/design/events",
        get(move |request: Request| {
            let (counter, record) = (counter.clone(), record.clone());
            async move {
                let call = counter.fetch_add(1, Ordering::SeqCst);
                record.lock().unwrap().push((
                    request.uri().query().unwrap_or_default().to_owned(),
                    request.headers()["Last-Event-ID"]
                        .to_str()
                        .unwrap()
                        .to_owned(),
                ));
                let value = format!(": keep-alive\r\n\r\n{}", frame(call as u64 + 1));
                let bytes = value.into_bytes();
                let split = bytes.iter().position(|byte| *byte == 0xc3).unwrap() + 1;
                let chunks = vec![
                    Ok::<_, std::io::Error>(bytes[..split].to_vec()),
                    Ok(bytes[split..].to_vec()),
                ];
                Response::builder()
                    .header("Content-Type", "text/event-stream; charset=utf-8")
                    .body(Body::from_stream(futures_util::stream::iter(chunks)))
                    .unwrap()
            }
        }),
    );
    let (transport, task) = serve(router).await;
    let session = transport.authenticate(&credential("alice")).await.unwrap();
    let client = transport.labs(&session).unwrap();
    let cancel = CancellationToken::new();
    let mut stream = client
        .events(&client.document_ref("me", "design").unwrap(), 0)
        .unwrap();
    assert_eq!(stream.next(&cancel).await.unwrap(), event(1));
    assert_eq!(stream.cursor(), 1);
    assert_eq!(stream.next(&cancel).await.unwrap(), event(2));
    assert_eq!(stream.cursor(), 2);
    let seen = seen.lock().unwrap().clone();
    assert!(seen[0].0.contains("since=0"));
    assert_eq!(seen[0].1, "0");
    assert!(seen[1].0.contains("since=1"));
    assert_eq!(seen[1].1, "1");
    cancel.cancel();
    assert!(matches!(
        stream.next(&cancel).await,
        Err(labs::Error::Cancelled)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    task.abort();
}

#[tokio::test]
async fn incomplete_frames_retry_without_advancing_cursor_and_account_switch_drops_queued_events() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let router = Router::new().route(
        "/v1/labs/documents/design/events",
        get(move || {
            let counter = counter.clone();
            async move {
                let call = counter.fetch_add(1, Ordering::SeqCst);
                let data = if call == 0 {
                    "data: {\"data\":[".to_owned()
                } else {
                    format!("id: 2\ndata: {}\n\n", json!({"data":[event(1),event(2)]}))
                };
                Response::builder()
                    .header("Content-Type", "text/event-stream")
                    .body(Body::from(data))
                    .unwrap()
            }
        }),
    );
    let (transport, task) = serve(router).await;
    let alice = transport.authenticate(&credential("alice")).await.unwrap();
    let bob = transport.authenticate(&credential("bob")).await.unwrap();
    let client = transport.labs(&alice).unwrap();
    let mut stream = client
        .events(&client.document_ref("me", "design").unwrap(), 0)
        .unwrap();
    assert_eq!(
        stream.next(&CancellationToken::new()).await.unwrap().cursor,
        1
    );
    client.set_session(&bob).unwrap();
    assert!(matches!(
        stream.next(&CancellationToken::new()).await,
        Err(labs::Error::AccountChanged)
    ));
    assert_eq!(stream.cursor(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    task.abort();
}

#[tokio::test]
async fn an_invalid_event_cursor_is_not_saved_or_returned_as_a_change() {
    let router = Router::new().route(
        "/v1/labs/documents/design/events",
        get(|| async {
            Response::builder()
                .header("Content-Type", "text/event-stream")
                .body(Body::from(format!(
                    "id: 3\ndata: {}\n\n",
                    json!({"data":[event(2)]})
                )))
                .unwrap()
        }),
    );
    let (transport, task) = serve(router).await;
    let session = transport.authenticate(&credential("alice")).await.unwrap();
    let client = transport.labs(&session).unwrap();
    let mut stream = client
        .events(&client.document_ref("me", "design").unwrap(), 0)
        .unwrap();
    assert!(matches!(
        stream.next(&CancellationToken::new()).await,
        Err(labs::Error::InvalidResponse)
    ));
    assert_eq!(stream.cursor(), 0);
    task.abort();
}

#[tokio::test]
async fn signed_range_requests_never_forward_account_credentials_or_follow_redirects() {
    let signed_calls = Arc::new(AtomicUsize::new(0));
    let redirect = Arc::new(AtomicBool::new(false));
    let (calls, mode) = (signed_calls.clone(), redirect.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let signed_url = format!(
        "http://{}/signed?private-link-fixture",
        listener.local_addr().unwrap()
    );
    let signed_router = Router::new().route(
        "/signed",
        get(move |headers: HeaderMap| {
            let (calls, mode) = (calls.clone(), mode.clone());
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                for name in ["authorization", "proxy-authorization", "cookie"] {
                    assert!(headers.get(name).is_none(), "{name}");
                }
                if mode.load(Ordering::SeqCst) {
                    return Response::builder()
                        .status(302)
                        .header("Location", "/steal")
                        .body(Body::empty())
                        .unwrap();
                }
                assert_eq!(headers["Range"], "bytes=0-3");
                Response::builder()
                    .status(206)
                    .header("Content-Range", "bytes 0-3/4")
                    .header("Content-Length", "4")
                    .body(Body::from("glTF"))
                    .unwrap()
            }
        }),
    );
    let signed_task = tokio::spawn(async move {
        axum::serve(listener, signed_router).await.unwrap();
    });
    let metadata = Arc::new(Mutex::new(
        json!({"artifact":{"id":"file","version":{"document":"design","version":1},"role":"export","media_type":"model/gltf-binary","sha256":format!("{:x}",sha2::Sha256::digest(b"glTF")),"size":4,"class":"public","no_training":false},"url":signed_url,"expires_seconds":60}),
    ));
    let response = metadata.clone();
    let router = Router::new().route(
        "/v1/labs/artifacts/file",
        get(move |headers: HeaderMap| {
            let response = response.clone();
            async move {
                account(&headers).unwrap();
                let value = response.lock().unwrap().clone();
                axum::Json(value)
            }
        }),
    );
    let (transport, task) = serve(router).await;
    let session = transport.authenticate(&credential("alice")).await.unwrap();
    let client = transport.labs(&session).unwrap();
    let cancel = CancellationToken::new();
    let artifact = client.artifact("me", "file", &cancel).await.unwrap();
    assert!(!format!("{artifact:?}").contains("private-link-fixture"));
    assert_eq!(
        collect(
            client
                .download(&artifact, Some(0..4), &cancel)
                .await
                .unwrap()
        )
        .await,
        b"glTF"
    );
    redirect.store(true, Ordering::SeqCst);
    assert!(matches!(
        client.download(&artifact, Some(0..4), &cancel).await,
        Err(labs::Error::Session(openagents_sdk::Error::Redirect))
    ));
    assert_eq!(signed_calls.load(Ordering::SeqCst), 2);
    metadata.lock().unwrap()["expires_seconds"] = Value::Null;
    let invalid = client.artifact("me", "file", &cancel).await.unwrap();
    assert!(matches!(
        client.download(&invalid, Some(0..4), &cancel).await,
        Err(labs::Error::InvalidResponse)
    ));
    assert_eq!(signed_calls.load(Ordering::SeqCst), 2);
    task.abort();
    signed_task.abort();
}

#[test]
fn diagnostics_do_not_expose_private_refusals_and_copy_stays_plain() {
    let error = labs::Error::Refused {
        status: 422,
        refusal: labs::Refusal::new("future_code", "private document fixture"),
        retry_after: None,
    };
    assert!(!format!("{error:?}").contains("private document fixture"));
    assert!(!error.to_string().contains("private document fixture"));
    for error in [
        labs::Error::InvalidInput,
        labs::Error::InvalidResponse,
        labs::Error::TooLarge,
        labs::Error::Cancelled,
        labs::Error::AccountChanged,
        labs::Error::WaitTimedOut,
        labs::Error::ExpiredLink,
    ] {
        assert!(
            oa_copy::violations(&error.to_string(), &[]).is_empty(),
            "{error}"
        );
    }
}

#[tokio::test]
async fn event_reconnects_keep_the_cursor_after_a_temporary_service_failure() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let router = Router::new().route(
        "/v1/labs/documents/design/events",
        get(move |request: Request| {
            let counter = counter.clone();
            async move {
                assert_eq!(request.headers()["Last-Event-ID"], "0");
                if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                    return Response::builder()
                        .status(503)
                        .header("Retry-After", "0")
                        .header("Content-Type", "application/json")
                        .body(Body::from(
                            json!({"error":{"code":"unavailable","message":"Try again."}})
                                .to_string(),
                        ))
                        .unwrap();
                }
                Response::builder()
                    .header("Content-Type", "text/event-stream")
                    .body(Body::from(frame(1)))
                    .unwrap()
            }
        }),
    );
    let (transport, task) = serve(router).await;
    let session = transport.authenticate(&credential("alice")).await.unwrap();
    let client = transport.labs(&session).unwrap();
    let mut events = client
        .events(&client.document_ref("me", "design").unwrap(), 0)
        .unwrap();
    assert_eq!(
        events.next(&CancellationToken::new()).await.unwrap(),
        event(1)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    task.abort();
}
