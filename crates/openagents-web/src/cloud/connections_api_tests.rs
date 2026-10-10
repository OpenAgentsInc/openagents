//! Coder's Google tools (#11238): `/v1/connections` and `POST
//! /v1/connections/google/tools/{tool}` under the app's own token, with a
//! fake Google. Each account reaches only its own connection, and no token
//! reaches an answer.

use super::*;
use crate::chat_store::account_owner;
use oa_connections::fake::{self, Content, FakeFile, FakeGoogle};
use oa_connections::google::oauth::Client;
use oa_connections::google::{DRIVE_READONLY, SHEETS_READONLY};
use serde_json::Value;

const SHEET: &str = "sheet-spending-00001";

async fn call(
    site: &Router,
    bearer: Option<&str>,
    method: Method,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, HOST)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(bearer) = bearer {
        request = request.header(header::AUTHORIZATION, format!("Bearer {bearer}"));
    }
    let response = site
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn the_app_token_runs_its_own_accounts_drive_tools() {
    let google = FakeGoogle::start(
        vec![FakeFile::new(
            SHEET,
            "Spending",
            None,
            Content::Sheet(vec![(
                "Oct".into(),
                vec![vec!["Rent".into(), "1200".into()]],
            )]),
        )],
        &[DRIVE_READONLY, SHEETS_READONLY],
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        crate::cloud::connections::Store::open(
            &dir.path().canonicalize().unwrap().join("connections"),
            oa_seal::Keyring::scratch("test").unwrap().0,
        )
        .unwrap(),
    );
    store
        .update(&account_owner("alice"), |account| {
            account.connections.push(crate::cloud::connections::Stored {
                integration: "google".into(),
                name: "default".into(),
                identity: Some(fake::EMAIL.into()),
                granted_scopes: vec![DRIVE_READONLY.into(), SHEETS_READONLY.into()],
                refresh_token: fake::REFRESH.into(),
                connected_at: 1,
                reconnect: false,
            })
        })
        .unwrap();
    let endpoints = google.endpoints();
    let kept = store.clone();
    let fixture = fixture_with(move |config| {
        config.connections = Some(kept);
        config.google = Some(Arc::new(crate::connections::Google::new(
            Client::new("cid".into(), "secret".into()),
            endpoints,
        )));
    })
    .await;
    let alice = token("alice");

    let (status, list) = call(
        &fixture.site,
        Some(&alice),
        Method::GET,
        "/v1/connections",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list["available"], true);
    let tools = list["connections"][0]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 3);
    assert_eq!(tools[0]["address"], "google.account.default.drive.search");
    assert_eq!(tools[0]["policy"], "allow");
    assert!(!list.to_string().contains(fake::REFRESH));

    let (status, read) = call(
        &fixture.site,
        Some(&alice),
        Method::POST,
        "/v1/connections/google/tools/drive_read",
        json!({"file": SHEET}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["result"]["text"], "# Oct\nRent,1200\n\n");
    assert_eq!(read["read"][0]["name"], "Spending");
    assert!(!read.to_string().contains(fake::ACCESS));

    // Unknown fields, unknown tools, and a PDF without a reader are refused.
    let (status, _) = call(
        &fixture.site,
        Some(&alice),
        Method::POST,
        "/v1/connections/google/tools/drive_read",
        json!({"file": SHEET, "x": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = call(
        &fixture.site,
        Some(&alice),
        Method::POST,
        "/v1/connections/google/tools/drive_delete",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Bob has no connection; no token is signed out.
    let (status, bob) = call(
        &fixture.site,
        Some(&token("bob")),
        Method::POST,
        "/v1/connections/google/tools/drive_read",
        json!({"file": SHEET}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{bob}");
    let (status, _) = call(
        &fixture.site,
        None,
        Method::POST,
        "/v1/connections/google/tools/drive_read",
        json!({"file": SHEET}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Google refusing the refresh token marks the connection for reconnecting.
    google
        .inner
        .lock()
        .unwrap()
        .revoked
        .push(fake::REFRESH.into());
    let (status, _) = call(
        &fixture.site,
        Some(&alice),
        Method::POST,
        "/v1/connections/google/tools/drive_read",
        json!({"file": SHEET}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(store.load(&account_owner("alice")).unwrap().connections[0].reconnect);
}
