//! Account-owned authoring grants. The preview never receives account credentials.
use crate::{
    App,
    chat_store::{Error, Store, account_owner},
    cloud::session::{SessionError, Viewer},
    coder_sync::{answer, refused, stored},
};
use aethyrforge_host::{Action, Project, ScopedGrant, action_digest, mutation};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
mod house_request;
mod projects;

const ROOT: &str = "/v1/verse/studio";
const SCOPE: &str = "verse-studio";
const TTL: u64 = 300;
pub(crate) fn owns(path: &str) -> bool {
    path == ROOT
        || path.starts_with("/v1/verse/studio/")
        || path == house_request::PATH
        || path.starts_with("/v1/verse/play/")
        || path.starts_with("/new-lander/play/")
        || path.starts_with("/new-lander/planet/")
        || path.starts_with("/new-lander/visit/")
        || path.starts_with("/new-lander/invite/")
}
pub(crate) fn routes() -> Router<App> {
    Router::new()
        .merge(house_request::routes())
        .merge(projects::routes())
        .route("/v1/verse/studio/session", get(session))
        .route("/v1/verse/studio/grants", post(issue))
        .route("/v1/verse/studio/authorize", post(authorize))
        .route("/v1/verse/studio/revoke", post(revoke))
        .layer(DefaultBodyLimit::max(
            aethyrforge_host::MAX_AUTHORIZATION_BYTES,
        ))
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}
fn now() -> u64 {
    crate::cloud::session::now()
}
fn denied() -> Response {
    refused(
        StatusCode::FORBIDDEN,
        "permission_ended",
        "Sign in and confirm your change again.",
    )
}
struct Actor {
    account_id: String,
    session_id: String,
    expires_at: u64,
    cookie: Option<Viewer>,
}
async fn viewer(app: &App, headers: &HeaderMap) -> Result<Actor, Response> {
    let Some(service) = app.config.cloud.as_deref() else {
        return Err(refused(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Sign in to make a change.",
        ));
    };
    if let Some(token) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        if token.len() > 512 {
            return Err(denied());
        }
        // Native clients use the existing account read, freshly checked for every write.
        return service
            .app_account(token)
            .await
            .map(|account_id| Actor {
                account_id,
                session_id: hash(token.as_bytes()),
                expires_at: now() + TTL,
                cookie: None,
            })
            .map_err(|_| denied());
    }
    service
        .authenticate(headers)
        .await
        .map(|v| Actor {
            account_id: v.account_id.clone(),
            session_id: v.session_id.clone(),
            expires_at: v.expires_at,
            cookie: Some(v),
        })
        .map_err(|e| match e {
            SessionError::Unauthenticated | SessionError::InvalidRequest => refused(
                StatusCode::UNAUTHORIZED,
                "signed_out",
                "Sign in to make a change.",
            ),
            _ => refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "Sign-in could not be checked. Try again.",
            ),
        })
}
async fn session(State(app): State<App>, headers: HeaderMap) -> Response {
    match viewer(&app, &headers).await {
        Ok(v) => {
            let Some(service) = app.config.cloud.as_deref() else {
                return denied();
            };
            match v.cookie.as_ref().map_or(Ok(String::new()), |cookie| {
                service.csrf(&headers, cookie, SCOPE, "")
            }) {
                Ok(csrf) => answer(
                    StatusCode::OK,
                    json!({"signed_in":true,"account":v.account_id,"csrf":csrf}),
                ),
                Err(_) => denied(),
            }
        }
        Err(r) if r.status() == StatusCode::UNAUTHORIZED => {
            answer(StatusCode::OK, json!({"signed_in":false}))
        }
        Err(r) => r,
    }
}
#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Book {
    grants: Vec<Held>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Held {
    digest: String,
    tab: String,
    account_session: String,
    session: String,
    release: String,
    project: Project,
    draft: Project,
    action: Action,
    expires_at: u64,
    consumed: bool,
}
fn key(account: &str) -> Result<String, Error> {
    Store::owner_key(&account_owner(account), "verse-studio-access.json")
}
async fn load(store: &Store, account: &str) -> Result<(Book, Option<String>), Error> {
    match store.read_key(&key(account)?).await? {
        Some((bytes, generation)) => Ok((
            serde_json::from_slice(&bytes)
                .map_err(|_| Error::Corrupt("Saved permissions could not be read."))?,
            Some(generation),
        )),
        None => Ok((Book::default(), None)),
    }
}
async fn save(
    store: &Store,
    account: &str,
    book: &Book,
    generation: Option<&str>,
) -> Result<(), Error> {
    let key = key(account)?;
    let bytes =
        serde_json::to_vec(book).map_err(|_| Error::Invalid("Permissions could not be saved."))?;
    write_grants(|| async {
        store
            .write_key(&key, bytes.clone(), generation)
            .await
            .map(|_| ())
    })
    .await
}

async fn write_grants<F>(mut write: impl FnMut() -> F) -> Result<(), Error>
where
    F: std::future::Future<Output = Result<(), Error>>,
{
    // Retry only a refused write, with the same bytes and generation condition.
    // A conflict must return to the caller, which reads the new grant book.
    for seconds in [1, 2, 4] {
        match write().await {
            Err(Error::Http(429)) => {
                tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
            }
            result => return result,
        }
    }
    write().await
}
fn private_draft(account: &str) -> Project {
    Project {
        id: format!("verse-{}", &hash(account.as_bytes())[..32]),
        branch: "haunted-house".into(),
        revision: 0,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Issue {
    csrf: String,
    tab: String,
    session: String,
    release: String,
    project: Project,
    action: Action,
    confirmed: bool,
}
fn valid_issue(sent: &Issue, release: &str) -> bool {
    sent.confirmed
        && valid_id(&sent.tab)
        && valid_id(&sent.session)
        && sent.release == release
        && matches!(
            sent.project.id.as_str(),
            "haunted-house" | "personal-planet"
        )
        && sent.project.branch == "main"
        && sent.project.validate().is_ok()
        && sent.action.validate().is_ok()
        && sent.action.capability().is_some_and(mutation)
}
fn csrf(app: &App, headers: &HeaderMap, v: &Actor, token: &str) -> bool {
    if let Some(cookie) = &v.cookie {
        return app.config.cloud.as_deref().is_some_and(|s| {
            s.verify_csrf(headers, Some(cookie), SCOPE, "", token)
                .is_ok()
        });
    }
    // A bearer is never ambient browser authority. Reject a foreign Origin too.
    headers
        .get(axum::http::header::ORIGIN)
        .is_none_or(|origin| {
            app.config
                .cloud
                .as_deref()
                .is_some_and(|s| origin.to_str().ok() == Some(s.origin()))
        })
}
async fn issue(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<Issue>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let v = match viewer(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let sent = match body {
        Ok(Json(sent)) => sent,
        Err(error) if error.status() == StatusCode::PAYLOAD_TOO_LARGE => return too_large(),
        Err(_) => {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid",
                "Choose a change to confirm.",
            );
        }
    };
    let personal = sent.project.id == "personal-planet";
    let release = if personal {
        app.config
            .verse_personal_runtime
            .as_ref()
            .map(|r| r.engine_release.clone())
    } else {
        crate::pages::new_lander_house::engine_release()
    };
    let Some(release) = release else {
        return denied();
    };
    let draft = if personal {
        let planet = match projects::tutorial::owned_planet(&app, &headers, &v.account_id).await {
            Ok(p) => p,
            Err(r) => return r,
        };
        Project {
            id: planet.project,
            branch: "personal-planet".into(),
            revision: 0,
        }
    } else {
        private_draft(&v.account_id)
    };
    if !csrf(&app, &headers, &v, &sent.csrf) || !valid_issue(&sent, &release) {
        return denied();
    }
    let token = hash(&secp256k1::rand::random::<[u8; 32]>());
    let grant = ScopedGrant {
        token: token.clone(),
        action_sha256: action_digest(&sent.action).expect("validated action"),
        session: sent.session.clone(),
        release: sent.release.clone(),
        project: sent.project.clone(),
        draft,
        capability: sent.action.capability().expect("validated mutation"),
        expires_at: (now() + TTL).min(v.expires_at),
    };
    for _ in 0..4 {
        let (mut book, generation) = match load(&app.config.chat_store, &v.account_id).await {
            Ok(v) => v,
            Err(e) => return stored(&e),
        };
        book.grants
            .retain(|g| g.expires_at > now() && !(g.tab == sent.tab && g.session != sent.session));
        if book.grants.len() >= 32 {
            return refused(
                StatusCode::TOO_MANY_REQUESTS,
                "busy",
                "Finish an open change before starting another.",
            );
        }
        book.grants.push(Held {
            digest: hash(token.as_bytes()),
            tab: sent.tab.clone(),
            account_session: v.session_id.clone(),
            session: sent.session.clone(),
            release: sent.release.clone(),
            project: sent.project.clone(),
            draft: grant.draft.clone(),
            action: sent.action.clone(),
            expires_at: grant.expires_at,
            consumed: false,
        });
        match save(
            &app.config.chat_store,
            &v.account_id,
            &book,
            generation.as_deref(),
        )
        .await
        {
            Ok(()) => return answer(StatusCode::OK, json!({"grant":grant})),
            Err(Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    refused(
        StatusCode::CONFLICT,
        "changed",
        "Another tab changed this work. Try again.",
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Use {
    csrf: String,
    tab: String,
    grant: ScopedGrant,
    action: Action,
}
fn permits(
    held: &Held,
    account_session: &str,
    tab: &str,
    grant: &ScopedGrant,
    action: &Action,
    at: u64,
) -> bool {
    grant.validate().is_ok()
        && grant.expires_at > at
        && held.expires_at > at
        && grant.action_sha256 == action_digest(action).expect("validated action")
        && held.digest == hash(grant.token.as_bytes())
        && held.tab == tab
        && held.account_session == account_session
        && held.session == grant.session
        && held.release == grant.release
        && held.project == grant.project
        && held.draft == grant.draft
        && held.expires_at == grant.expires_at
        && held.action == *action
        && action.capability() == Some(grant.capability)
}
fn too_large() -> Response {
    refused(
        StatusCode::PAYLOAD_TOO_LARGE,
        "too_large",
        "This change is too large. Export your layers, then use fewer or smaller images.",
    )
}
async fn authorize(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<Use>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let v = match viewer(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let sent = match body {
        Ok(Json(sent)) => sent,
        Err(error) if error.status() == StatusCode::PAYLOAD_TOO_LARGE => return too_large(),
        Err(_) => return denied(),
    };
    if !csrf(&app, &headers, &v, &sent.csrf) {
        return denied();
    }
    if sent.grant.project.id == "personal-planet" {
        let planet = match projects::tutorial::owned_planet(&app, &headers, &v.account_id).await {
            Ok(p) => p,
            Err(r) => return r,
        };
        if sent.grant.draft.id != planet.project || sent.grant.draft.branch != "personal-planet" {
            return denied();
        }
    }
    for _ in 0..4 {
        let (mut book, generation) = match load(&app.config.chat_store, &v.account_id).await {
            Ok(v) => v,
            Err(e) => return stored(&e),
        };
        let Some(held) = book.grants.iter_mut().find(|g| {
            permits(
                g,
                &v.session_id,
                &sent.tab,
                &sent.grant,
                &sent.action,
                now(),
            )
        }) else {
            return denied();
        };
        let replay = held.consumed;
        held.consumed = true;
        match save(
            &app.config.chat_store,
            &v.account_id,
            &book,
            generation.as_deref(),
        )
        .await
        {
            Ok(()) => {
                return answer(
                    StatusCode::OK,
                    json!({"authorized":true,"replay":replay,"draft":sent.grant.draft}),
                );
            }
            Err(Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    refused(
        StatusCode::CONFLICT,
        "changed",
        "Another tab changed this work. Try again.",
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revoke {
    csrf: String,
    tab: String,
}
async fn revoke(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<Revoke>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let v = match viewer(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Ok(Json(sent)) = body else {
        return denied();
    };
    if !valid_id(&sent.tab) || !csrf(&app, &headers, &v, &sent.csrf) {
        return denied();
    }
    for _ in 0..4 {
        let (mut book, generation) = match load(&app.config.chat_store, &v.account_id).await {
            Ok(v) => v,
            Err(e) => return stored(&e),
        };
        book.grants
            .retain(|g| g.tab != sent.tab || g.account_session != v.session_id);
        match save(
            &app.config.chat_store,
            &v.account_id,
            &book,
            generation.as_deref(),
        )
        .await
        {
            Ok(()) => return answer(StatusCode::OK, json!({"revoked":true})),
            Err(Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    denied()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn throttled_grants_retry_until_storage_accepts_the_write() {
        let mut results = std::collections::VecDeque::from([
            Err(Error::Http(429)),
            Err(Error::Http(429)),
            Ok(()),
        ]);
        write_grants(|| std::future::ready(results.pop_front().expect("bounded writes")))
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn throttled_grants_stop_after_four_writes() {
        let mut attempts = 0;
        let result = write_grants(|| {
            attempts += 1;
            std::future::ready(Err(Error::Http(429)))
        })
        .await;
        assert!(matches!(result, Err(Error::Http(429))));
        assert_eq!(attempts, 4);
    }

    #[tokio::test]
    async fn throttled_grants_return_conflicts_and_other_failures_to_the_caller() {
        for error in [Error::Conflict, Error::Http(403), Error::Http(503)] {
            let mut results =
                std::collections::VecDeque::from([Err(Error::Http(429)), Err(error), Ok(())]);
            assert!(
                write_grants(|| std::future::ready(results.pop_front().unwrap()))
                    .await
                    .is_err()
            );
            assert_eq!(
                results.len(),
                1,
                "a failed precondition must not be retried"
            );
        }
    }

    fn fixture() -> (Held, ScopedGrant, Action) {
        let project = Project {
            id: "haunted-house".into(),
            branch: "main".into(),
            revision: 0,
        };
        let action = Action::Propose {
            operations: vec![aethyrforge_host::Edit::Move {
                target: "werewolf-1".into(),
                position: [-2., 0., -14.],
            }],
        };
        let grant = ScopedGrant {
            token: "a".repeat(64),
            action_sha256: action_digest(&action).unwrap(),
            session: "engine-session".into(),
            release: "house-v1".into(),
            project: project.clone(),
            draft: private_draft("alice"),
            capability: aethyrforge_host::Capability::Propose,
            expires_at: 200,
        };
        let held = Held {
            digest: hash(grant.token.as_bytes()),
            tab: "tab-1".into(),
            account_session: "account-session".into(),
            session: grant.session.clone(),
            release: grant.release.clone(),
            project,
            draft: grant.draft.clone(),
            action: action.clone(),
            expires_at: 200,
            consumed: false,
        };
        (held, grant, action)
    }
    #[test]
    fn verse_studio_refuses_forged_expired_and_cross_account_project_tab_action_grants() {
        let (held, grant, action) = fixture();
        assert!(permits(
            &held,
            "account-session",
            "tab-1",
            &grant,
            &action,
            100
        ));
        assert!(!permits(
            &held,
            "other-session",
            "tab-1",
            &grant,
            &action,
            100
        ));
        assert!(!permits(
            &held,
            "account-session",
            "tab-2",
            &grant,
            &action,
            100
        ));
        assert!(!permits(
            &held,
            "account-session",
            "tab-1",
            &grant,
            &action,
            200
        ));
        for field in [
            "token",
            "project",
            "draft",
            "session",
            "release",
            "capability",
        ] {
            let mut bad = grant.clone();
            match field {
                "token" => bad.token = "b".repeat(64),
                "project" => bad.project.branch = "other".into(),
                "draft" => bad.draft = private_draft("bob"),
                "session" => bad.session = "other".into(),
                "release" => bad.release = "other".into(),
                _ => bad.capability = aethyrforge_host::Capability::Publish,
            };
            assert!(
                !permits(&held, "account-session", "tab-1", &bad, &action, 100),
                "{field}"
            );
        }
        assert!(!permits(
            &held,
            "account-session",
            "tab-1",
            &grant,
            &Action::Save,
            100
        ));
        assert_ne!(private_draft("alice"), private_draft("bob"));
    }
    #[tokio::test]
    async fn verse_studio_revocation_is_durable_and_tabs_have_independent_grants() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::local(root.path().into());
        let (held, _, _) = fixture();
        let mut second = fixture().0;
        second.tab = "tab-2".into();
        save(
            &store,
            "alice",
            &Book {
                grants: vec![held, second],
            },
            None,
        )
        .await
        .unwrap();
        let (mut book, generation) = load(&store, "alice").await.unwrap();
        book.grants.retain(|g| g.tab != "tab-1");
        save(&store, "alice", &book, generation.as_deref())
            .await
            .unwrap();
        let (book, _) = load(&Store::local(root.path().into()), "alice")
            .await
            .unwrap();
        assert_eq!(book.grants.len(), 1);
        assert_eq!(book.grants[0].tab, "tab-2");
        assert!(load(&store, "bob").await.unwrap().0.grants.is_empty());
        assert!(matches!(
            save(&store, "alice", &book, generation.as_deref()).await,
            Err(Error::Conflict)
        ));
    }
    #[tokio::test]
    async fn verse_studio_guest_reads_work_but_mutations_require_a_current_account() {
        use axum::{
            body::{Body, to_bytes},
            http::Request,
        };
        use tower::ServiceExt;
        let root = tempfile::tempdir().unwrap();
        let app = crate::router(crate::Config::development(root.path().join("tasks")));
        let r = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/verse/studio/session")
                    .header("host", "127.0.0.1:4300")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                &to_bytes(r.into_body(), 4096).await.unwrap()
            )
            .unwrap()["signed_in"],
            false
        );
        for path in ["grants", "authorize", "revoke"] {
            let r = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .header("host", "127.0.0.1:4300")
                        .uri(format!("{ROOT}/{path}"))
                        .header("content-type", "application/json")
                        .body(Body::from("{} "))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        }
    }
}
