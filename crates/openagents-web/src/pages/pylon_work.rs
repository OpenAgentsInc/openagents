//! Private staff reads and signed, bounded Pylon execution summaries (#12524).

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use pylons_view::work::{MAX_BODY_BYTES, PATH, WorkSnapshot, WorkView, provider};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::App;
use crate::chat_store::{Error, Store};

const PUBLISHERS_ENV: &str = "OPENAGENTS_PYLON_WORK_PUBLISHERS";
const MAX_PUBLISHERS: usize = 16;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PATH, get(read).post(publish))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    pylon: String,
}

#[derive(Serialize)]
struct Problem {
    error: &'static str,
}

fn response(status: StatusCode, value: impl Serialize) -> Response {
    let mut result = (status, Json(value)).into_response();
    result.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    result.headers_mut().insert(
        header::VARY,
        HeaderValue::from_static("Cookie, Authorization"),
    );
    result.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    result
}

fn problem(status: StatusCode, error: &'static str) -> Response {
    response(status, Problem { error })
}

async fn read(
    State(app): State<App>,
    headers: HeaderMap,
    Query(selection): Query<Selection>,
) -> Response {
    let access = if let Some(service) = app.config.cloud.as_deref() {
        match service.authenticate(&headers).await {
            Ok(viewer) if viewer.admin => StatusCode::OK,
            Ok(_) => StatusCode::FORBIDDEN,
            Err(_) => StatusCode::UNAUTHORIZED,
        }
    } else {
        StatusCode::UNAUTHORIZED
    };
    read_authorized(
        &app.config.chat_store,
        access,
        &selection.pylon,
        crate::chat_store::now_unix(),
    )
    .await
}

async fn read_authorized(store: &Store, access: StatusCode, pylon: &str, now: u64) -> Response {
    if access != StatusCode::OK {
        return problem(access, "Work details are available to signed-in staff.");
    }
    if provider(pylon).is_none() {
        return problem(StatusCode::BAD_REQUEST, "Choose a valid Pylon.");
    }
    match latest(store, pylon).await {
        Ok((snapshot, _)) => response(StatusCode::OK, WorkView::at(snapshot, now)),
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "Work details are unavailable. Try again later.",
        ),
    }
}

fn publisher_keys(value: Option<&str>, default: &str) -> Result<Vec<String>, ()> {
    let keys: Vec<String> = value
        .unwrap_or(default)
        .split(',')
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .collect();
    if keys.is_empty()
        || keys.len() > MAX_PUBLISHERS
        || keys
            .iter()
            .any(|k| k.len() != 64 || !k.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(());
    }
    Ok(keys)
}

async fn publish(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let setting = std::env::var(PUBLISHERS_ENV).ok();
    let Ok(keys) = publisher_keys(
        setting.as_deref(),
        &crate::grid_foreman_commands::foreman_key(),
    ) else {
        return problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "Work updates are unavailable.",
        );
    };
    publish_signed(
        &app.config.chat_store,
        &headers,
        &body,
        crate::chat_store::now_unix(),
        &keys,
        &crate::grid_foreman_commands::origins(),
    )
    .await
}

async fn publish_signed(
    store: &Store,
    headers: &HeaderMap,
    body: &[u8],
    now: u64,
    keys: &[String],
    origins: &[String],
) -> Response {
    if body.len() > MAX_BODY_BYTES {
        return problem(
            StatusCode::PAYLOAD_TOO_LARGE,
            "That work update is too large.",
        );
    }
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    if !keys.iter().any(|key| {
        crate::grid_foreman_commands::verify_at(authorization, body, now, key, origins, PATH)
            .is_ok()
    }) {
        return problem(
            StatusCode::UNAUTHORIZED,
            "Only an approved publisher can send work updates.",
        );
    }
    let snapshot: WorkSnapshot = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => return problem(StatusCode::BAD_REQUEST, "That work update is invalid."),
    };
    if snapshot.validate(now).is_err() {
        return problem(StatusCode::BAD_REQUEST, "That work update is invalid.");
    }
    match save(store, &snapshot).await {
        Ok(()) => response(StatusCode::OK, WorkView::at(Some(snapshot), now)),
        Err(Error::Conflict) => problem(
            StatusCode::CONFLICT,
            "A newer work update is already available.",
        ),
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "The work update could not be saved. Try again later.",
        ),
    }
}

fn key(pylon: &str) -> String {
    let hash: String = Sha256::digest(pylon.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("pylons/work/{hash}.json")
}

async fn latest(
    store: &Store,
    pylon: &str,
) -> Result<(Option<WorkSnapshot>, Option<String>), Error> {
    let Some((bytes, generation)) = store.read_key(&key(pylon)).await? else {
        return Ok((None, None));
    };
    if bytes.len() > MAX_BODY_BYTES {
        return Err(Error::Corrupt("The work report exceeds its size limit."));
    }
    let snapshot: WorkSnapshot = serde_json::from_slice(&bytes)
        .map_err(|_| Error::Corrupt("The work report is invalid."))?;
    if snapshot.pylon != pylon || snapshot.validate(snapshot.observed_at).is_err() {
        return Err(Error::Corrupt("The work report is invalid."));
    }
    Ok((Some(snapshot), Some(generation)))
}

async fn save(store: &Store, snapshot: &WorkSnapshot) -> Result<(), Error> {
    let bytes =
        serde_json::to_vec(snapshot).map_err(|_| Error::Invalid("The work report is invalid."))?;
    if bytes.len() > MAX_BODY_BYTES {
        return Err(Error::Invalid("The work report exceeds its size limit."));
    }
    for _ in 0..5 {
        let (previous, generation) = latest(store, &snapshot.pylon).await?;
        if previous.as_ref().is_some_and(|old| !snapshot.is_after(old)) {
            return Err(Error::Conflict);
        }
        match store
            .write_key(&key(&snapshot.pylon), bytes.clone(), generation.as_deref())
            .await
        {
            Ok(_) => return Ok(()),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use base64::Engine as _;
    use nostr::domain::{RelaySigner, Tag};
    use pylons_view::work::{SCHEMA, WorkAnswer, WorkPhase, WorkStep, WorkStepKind};
    use serde_json::{Value, json};
    use std::sync::Arc;
    use tower::ServiceExt as _;

    const NOW: u64 = 1000;
    const ORIGIN: &str = "https://openagents.com";

    fn temporary() -> tempfile::TempDir {
        let root = std::env::var_os("OPENAGENTS_SCRATCH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        tempfile::Builder::new()
            .prefix("pylon-work-")
            .tempdir_in(root)
            .unwrap()
    }

    fn snapshot() -> WorkSnapshot {
        WorkSnapshot {
            schema: SCHEMA.into(),
            pylon: format!("30200:{}:maya-1", "22".repeat(32)),
            session_id: "workspace-1".into(),
            session_started_at: 900,
            sequence: 1,
            observed_at: NOW,
            issue: Some(12524),
            title: "Show current Pylon work".into(),
            phase: WorkPhase::Working,
            current: None,
            turns: 1,
            calls: 1,
            changed: 1,
            files: vec!["src/lib.rs".into()],
            checks: vec![],
            steps: vec![WorkStep {
                sequence: 1,
                at: 990,
                kind: WorkStepKind::Read,
                label: "Read file".into(),
                path: Some("src/lib.rs".into()),
                summary: "Checked the current implementation.".into(),
            }],
            answer: Some(WorkAnswer {
                request: "33".repeat(32),
                provider: "22".repeat(32),
                model: "deepseek-v4.1-flash".into(),
                profile: "consumer-q".into(),
                lossy: true,
            }),
        }
    }

    fn signed(signer: &RelaySigner, body: &[u8], at: u64, path: &str, method: &str) -> HeaderMap {
        let hash: String = Sha256::digest(body)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let event = signer.sign(
            at,
            27235,
            vec![
                Tag::new(vec!["u".into(), format!("{ORIGIN}{path}")]),
                Tag::new(vec!["method".into(), method.into()]),
                Tag::new(vec!["payload".into(), hash]),
            ],
            String::new(),
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            format!(
                "Nostr {}",
                base64::engine::general_purpose::STANDARD
                    .encode(serde_json::to_vec(&event).unwrap())
            )
            .parse()
            .unwrap(),
        );
        headers
    }

    async fn send(store: &Store, value: &WorkSnapshot) -> Response {
        let signer = RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
        let body = serde_json::to_vec(value).unwrap();
        publish_signed(
            store,
            &signed(&signer, &body, NOW, PATH, "POST"),
            &body,
            NOW,
            &[signer.pubkey().to_owned()],
            &[ORIGIN.into()],
        )
        .await
    }

    async fn body(response: Response) -> Value {
        serde_json::from_slice(
            &to_bytes(response.into_body(), MAX_BODY_BYTES as usize * 2)
                .await
                .unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn signed_update_has_private_direct_workview_and_exact_pylon_isolation() {
        let root = temporary();
        let store = Store::local(root.path().into());
        let value = snapshot();
        let published = send(&store, &value).await;
        assert_eq!(published.status(), StatusCode::OK);
        assert_eq!(
            published.headers()[header::CACHE_CONTROL],
            "private, no-store"
        );
        assert_eq!(body(published).await["snapshot"]["title"], value.title);
        let view = read_authorized(&store, StatusCode::OK, &value.pylon, NOW).await;
        assert_eq!(body(view).await, json!({"snapshot": value, "stale": false}));
        let other = read_authorized(
            &store,
            StatusCode::OK,
            &format!("30200:{}:other", "44".repeat(32)),
            NOW,
        )
        .await;
        assert_eq!(body(other).await, json!({"snapshot": null, "stale": false}));
    }

    #[tokio::test]
    async fn anonymous_and_nonstaff_readers_receive_no_private_work() {
        let root = temporary();
        let store = Store::local(root.path().into());
        let value = snapshot();
        assert_eq!(send(&store, &value).await.status(), StatusCode::OK);
        for status in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN] {
            let reply = read_authorized(&store, status, &value.pylon, NOW).await;
            assert_eq!(reply.status(), status);
            assert_eq!(reply.headers()[header::CACHE_CONTROL], "private, no-store");
            let text = body(reply).await.to_string();
            assert!(!text.contains(&value.title));
            assert!(!text.contains("workspace-1"));
        }
        let app = App(Arc::new(crate::Inner {
            config: crate::Config::development(root.path().join("app")),
        }));
        let reply = routes()
            .with_state(app)
            .oneshot(
                Request::builder()
                    .uri(format!("{PATH}?pylon={}", value.pylon))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(reply.status(), StatusCode::UNAUTHORIZED);
        assert!(!body(reply).await.to_string().contains(&value.title));
    }

    #[tokio::test]
    async fn nip98_binds_publisher_url_method_time_signature_and_payload() {
        let root = temporary();
        let store = Store::local(root.path().into());
        let value = snapshot();
        let signer = RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
        let keys = vec![signer.pubkey().to_owned()];
        let origins = vec![ORIGIN.into()];
        let bytes = serde_json::to_vec(&value).unwrap();
        for (at, path, method) in [
            (NOW, grid_foreman::COMMANDS_PATH, "POST"),
            (NOW, PATH, "GET"),
            (NOW - 100, PATH, "POST"),
        ] {
            let reply = publish_signed(
                &store,
                &signed(&signer, &bytes, at, path, method),
                &bytes,
                NOW,
                &keys,
                &origins,
            )
            .await;
            assert_eq!(reply.status(), StatusCode::UNAUTHORIZED);
        }
        let valid = signed(&signer, &bytes, NOW, PATH, "POST");
        assert_eq!(
            publish_signed(&store, &valid, b"{}", NOW, &keys, &origins)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            publish_signed(&store, &valid, &bytes, NOW, &["55".repeat(32)], &origins)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            publish_signed(&store, &HeaderMap::new(), &bytes, NOW, &keys, &origins)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            crate::grid_foreman_commands::verify(
                valid
                    .get(header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok()),
                &bytes,
                NOW,
                signer.pubkey(),
                &origins
            ),
            Err("foreman_bad_signature")
        );
        assert_eq!(latest(&store, &value.pylon).await.unwrap().0, None);
    }

    #[tokio::test]
    async fn replay_and_old_session_cannot_replace_newer_snapshot() {
        let root = temporary();
        let store = Store::local(root.path().into());
        let original = snapshot();
        assert_eq!(send(&store, &original).await.status(), StatusCode::OK);
        assert_eq!(send(&store, &original).await.status(), StatusCode::CONFLICT);
        let mut next = original.clone();
        next.sequence = 2;
        assert_eq!(send(&store, &next).await.status(), StatusCode::OK);
        let mut session = original.clone();
        session.session_id = "workspace-2".into();
        session.session_started_at = 950;
        assert_eq!(send(&store, &session).await.status(), StatusCode::OK);
        next.sequence = 500;
        assert_eq!(send(&store, &next).await.status(), StatusCode::CONFLICT);
        assert_eq!(
            latest(&store, &original.pylon).await.unwrap().0,
            Some(session)
        );
    }

    #[tokio::test]
    async fn concurrent_replicas_keep_highest_sequence_through_generation_cas() {
        let root = temporary();
        let store = Store::local(root.path().into());
        let original = snapshot();
        save(&store, &original).await.unwrap();
        let mut second = original.clone();
        second.sequence = 2;
        let mut third = original.clone();
        third.sequence = 3;
        let (two, three) = tokio::join!(save(&store, &second), save(&store, &third));
        assert!(two.is_ok() || matches!(two, Err(Error::Conflict)));
        assert!(three.is_ok());
        assert_eq!(
            latest(&store, &original.pylon)
                .await
                .unwrap()
                .0
                .unwrap()
                .sequence,
            3
        );
    }

    #[tokio::test]
    async fn malformed_unknown_oversized_or_stale_updates_are_not_saved() {
        let root = temporary();
        let store = Store::local(root.path().into());
        let signer = RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
        let keys = vec![signer.pubkey().to_owned()];
        let origins = vec![ORIGIN.into()];
        let mut unknown = serde_json::to_value(snapshot()).unwrap();
        unknown["raw_output"] = json!("private code");
        let mut bad = snapshot();
        bad.observed_at = 800;
        bad.session_started_at = 700;
        for bytes in [
            serde_json::to_vec(&unknown).unwrap(),
            serde_json::to_vec(&bad).unwrap(),
            b"{}".to_vec(),
        ] {
            let reply = publish_signed(
                &store,
                &signed(&signer, &bytes, NOW, PATH, "POST"),
                &bytes,
                NOW,
                &keys,
                &origins,
            )
            .await;
            assert_eq!(reply.status(), StatusCode::BAD_REQUEST);
        }
        let bytes = vec![b' '; MAX_BODY_BYTES + 1];
        assert_eq!(
            publish_signed(&store, &HeaderMap::new(), &bytes, NOW, &keys, &origins)
                .await
                .status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert!(latest(&store, &snapshot().pylon).await.unwrap().0.is_none());
    }

    #[tokio::test]
    async fn stale_private_report_is_marked_and_corruption_is_not_hidden_as_empty() {
        let root = temporary();
        let store = Store::local(root.path().into());
        let value = snapshot();
        save(&store, &value).await.unwrap();
        let reply = read_authorized(&store, StatusCode::OK, &value.pylon, NOW + 121).await;
        assert_eq!(body(reply).await["stale"], true);
        let (_, generation) = latest(&store, &value.pylon).await.unwrap();
        store
            .write_key(
                &key(&value.pylon),
                b"invalid-json".to_vec(),
                generation.as_deref(),
            )
            .await
            .unwrap();
        assert_eq!(
            read_authorized(&store, StatusCode::OK, &value.pylon, NOW)
                .await
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn publisher_allowlist_is_scoped_explicit_and_fail_closed() {
        let default = "11".repeat(32);
        let other = "22".repeat(32);
        assert_eq!(publisher_keys(None, &default), Ok(vec![default.clone()]));
        assert_eq!(publisher_keys(Some(&other), &default), Ok(vec![other]));
        assert!(publisher_keys(Some(""), &default).is_err());
        assert!(publisher_keys(Some("11,invalid"), &default).is_err());
        assert!(crate::upstream::owned(PATH));
    }
}
