use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::Query,
    http::header,
    response::Response,
    routing::get,
};
use openagents_sdk::{
    CancellationToken, Client, Deployment,
    attention::{Attention, AttentionEvent},
};
use serde_json::json;
use std::{collections::BTreeMap, time::Duration};

fn frame(seq: u64) -> String {
    let event = AttentionEvent {
        seq,
        at: 1000 + seq as i64,
        agent: Some("b".repeat(64)),
        attention: Attention::RunStalled {
            run: format!("run-{seq}"),
            since: 1000,
        },
    };
    format!(
        "event: attention\nid: {seq}\ndata: {}\n\n",
        serde_json::to_string(&event).unwrap()
    )
}

#[tokio::test]
async fn reads_the_stream_and_resumes_after_a_dropped_connection() {
    let router = Router::new()
        .route(
            "/v1/account",
            get(|| async {
                Json(json!({"account":{"id":"acct_attention","label":"Attention"},"workspaces":[]}))
            }),
        )
        .route(
            "/v1/attention",
            get(|Query(q): Query<BTreeMap<String, String>>| async move {
                let after: u64 = q["after"].parse().unwrap();
                let body = if after == 0 {
                    // Event 1, a comment, a kind from a newer server, then the
                    // connection drops mid-stream.
                    let chunks: Vec<Result<Bytes, std::io::Error>> = vec![
                        Ok(Bytes::from(frame(1))),
                        Ok(Bytes::from_static(b": current\n\n")),
                        Ok(Bytes::from(
                            "event: attention\nid: 2\ndata: {\"seq\":2,\"at\":5,\"kind\":\"future_kind\",\"x\":1}\n\n",
                        )),
                    ];
                    let drop = futures_util::stream::once(async {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        Err(std::io::Error::other("dropped"))
                    });
                    Body::from_stream(futures_util::StreamExt::chain(
                        futures_util::stream::iter(chunks),
                        drop,
                    ))
                } else {
                    assert_eq!(after, 2);
                    Body::from(frame(3))
                };
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/event-stream")
                    .body(body)
                    .unwrap()
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::with_timeout(
        Deployment::new(&format!("{base}/v1"), &base).unwrap(),
        Duration::from_secs(5),
    )
    .unwrap();
    let session = client
        .authenticate(&format!("oa_sk_{}", "a".repeat(64)))
        .await
        .unwrap();
    let cancel = CancellationToken::new();
    let mut stream = client.attention(&session, 0, &cancel).await.unwrap();
    let first = stream.next().await.unwrap().unwrap();
    assert_eq!(first.seq, 1);
    assert_eq!(first.agent.as_deref(), Some("b".repeat(64).as_str()));
    assert_eq!(
        first.attention,
        Attention::RunStalled {
            run: "run-1".into(),
            since: 1000
        }
    );
    let unknown = stream.next().await.unwrap().unwrap();
    assert_eq!(unknown.attention, Attention::Unknown);
    assert!(
        stream.next().await.is_err(),
        "the drop surfaces as an error"
    );
    assert_eq!(stream.cursor(), 2);
    let mut resumed = client
        .attention(&session, stream.cursor(), &cancel)
        .await
        .unwrap();
    assert_eq!(resumed.next().await.unwrap().unwrap().seq, 3);
    assert_eq!(resumed.next().await.unwrap(), None);
    cancel.cancel();
    assert!(resumed.next().await.is_err());
    task.abort();
}
