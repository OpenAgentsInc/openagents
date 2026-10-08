//! Private statement reads preserve bounded selection and redact failed payloads.
use jev::{Client, Config, Error, JoinedStatementQuery, RetryPolicy};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn view() -> Value {
    json!({"native_workspace":"workspace", "statement":{"schema":"openagents.joined-statement.v1","origin":"canonical","customer":"customer","workspace":"canonical-workspace","unit":{"kind":"millisatoshis"},"unit_scale":100_000_000_000u64,"balance":null,"snapshot":"a".repeat(64),"rows":[],"next":null,"scanned":0,"disclosure":[]},"payee":null,"payee_disclosure":"Separate earnings rights.","source_attribution":[],"attribution_disclosure":"Original source review.","native_projection":[],"native_projection_disclosure":"Original native prices only."})
}
async fn server(status: u16, body: String) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        loop {
            let mut bytes = [0; 4096];
            let n = socket.read(&mut bytes).await.unwrap();
            assert!(n > 0);
            request.extend_from_slice(&bytes[..n]);
            if request.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        socket.write_all(format!("HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nX-Request-Id: private_statement_canary\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        drop(socket);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
        String::from_utf8(request).unwrap()
    });
    (origin, handle)
}
fn client(origin: String) -> Client {
    Client::new(
        Config::new()
            .api_key("sess_joined_fixture")
            .base_url(origin)
            .retry(RetryPolicy {
                max_retries: 5,
                ..RetryPolicy::default()
            }),
    )
    .unwrap()
}
#[tokio::test]
async fn joined_read_and_private_export_use_bounded_original_routes() {
    for export in [false, true] {
        let (origin, request) = server(200, format!("{}\n", view())).await;
        let query = JoinedStatementQuery {
            cursor: Some("abcd".into()),
            limit: Some(7),
            after_earning: Some(3),
            after_payout: Some(4),
        };
        let got = client(origin)
            .account()
            .joined_statement("workspace", &query, export)
            .await
            .unwrap();
        assert_eq!(got.statement.workspace, "canonical-workspace");
        let path = format!(
            "GET /v1/workspaces/workspace/usage{}?joined=true&cursor=abcd&limit=7&after_earning=3&after_payout=4 ",
            if export { "/export" } else { "" }
        );
        assert!(request.await.unwrap().starts_with(&path));
    }
}
#[tokio::test]
async fn joined_rejects_foreign_or_private_response_without_retry_or_payload() {
    let mut foreign = view();
    foreign["native_workspace"] = json!("foreign");
    let mut schema = view();
    schema["statement"]["schema"] = json!("unqualified");
    let mut oversized = view();
    oversized["statement"]["rows"] = json!(vec![json!({}); 51]);
    for (status, body) in [
        (200, foreign.to_string()),
        (200, schema.to_string()),
        (200, oversized.to_string()),
        (503, "private_statement_canary".into()),
    ] {
        let (origin, request) = server(status, body).await;
        let error = client(origin)
            .account()
            .joined_statement("workspace", &JoinedStatementQuery::default(), false)
            .await
            .unwrap_err();
        assert!(!format!("{error:?}").contains("private_statement_canary"));
        if let Error::Api(e) = error {
            assert!(e.body.is_none());
            assert!(e.headers.is_empty());
        }
        request.await.unwrap();
    }
}
#[tokio::test]
async fn joined_rejects_route_escape_and_unbounded_page_before_transport() {
    let client = client("http://127.0.0.1:1".into());
    for workspace in ["..", "foreign/workspace", "foreign%2fworkspace", ""] {
        assert!(matches!(
            client
                .account()
                .joined_statement(workspace, &JoinedStatementQuery::default(), false)
                .await,
            Err(Error::Config(_))
        ));
    }
    for query in [
        JoinedStatementQuery {
            limit: Some(101),
            ..Default::default()
        },
        JoinedStatementQuery {
            after_payout: Some(-1),
            ..Default::default()
        },
        JoinedStatementQuery {
            cursor: Some("a".repeat(4097)),
            ..Default::default()
        },
    ] {
        assert!(matches!(
            client
                .account()
                .joined_statement("workspace", &query, false)
                .await,
            Err(Error::Config(_))
        ));
    }
}
