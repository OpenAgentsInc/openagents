use std::sync::atomic::{AtomicUsize, Ordering};

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request as HttpRequest};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use tower::ServiceExt;

use super::*;
use crate::chat_store::{RepositorySource, RuntimeSelection};

const OWNER: &str = "0123456789abcdef0123456789abcdef";
const OTHER_OWNER: &str = "abcdef0123456789abcdef0123456789";
const CHAT: &str = "12345678-1234-4234-8234-123456789abc";
const NEXT: &str = "22345678-1234-4234-8234-123456789abc";
const TEXT: &str = "Explain this repository";
const HOST: &str = "127.0.0.1:4300";

struct NoWorker(Arc<AtomicUsize>);

impl crate::ask::Chat for NoWorker {
    fn door(&self, _: secp256k1::SecretKey) -> Result<Box<dyn basic_coder::Door>, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err("This offline fixture cannot contact a worker.".into())
    }
}

struct Fixture {
    _directory: tempfile::TempDir,
    app: App,
    router: Router,
    worker_calls: Arc<AtomicUsize>,
}

impl Fixture {
    fn new() -> Self {
        // Build leases supply scratch; standalone tests still use isolated storage.
        let directory = match std::env::var_os("OPENAGENTS_SCRATCH") {
            Some(root) => tempfile::Builder::new()
                .prefix("web-composer-tests-")
                .tempdir_in(root)
                .unwrap(),
            None => tempfile::tempdir().unwrap(),
        };
        let worker_calls = Arc::new(AtomicUsize::new(0));
        let mut config = crate::Config::development(directory.path().join("tasks"));
        config.ask_salt = [11; 32];
        config.chat = Arc::new(NoWorker(worker_calls.clone()));
        let app = App(Arc::new(crate::Inner {
            config: config.clone(),
        }));
        Self {
            _directory: directory,
            app,
            router: crate::router(config),
            worker_calls,
        }
    }

    async fn record(&self, selection: Option<Selection>) -> Loaded {
        self.app
            .config
            .chat_store
            .create(&Conversation {
                id: CHAT.into(),
                owner: OWNER.into(),
                revision: 1,
                title: "Existing conversation".into(),
                messages: vec![
                    Message {
                        role: Role::User,
                        text: TEXT.into(),
                        request_id: Some(CHAT.into()),
                    },
                    Message {
                        role: Role::Assistant,
                        text: "Saved answer".into(),
                        request_id: Some(CHAT.into()),
                    },
                ],
                pending: None,
                requests: vec![Request {
                    id: CHAT.into(),
                    digest: request_digest(TEXT, selection.as_ref()),
                    outcome: Outcome::Answered,
                    selection: selection.clone(),
                    cloud: None,
                    reply: None,
                }],
                selection,
                updated_unix: 1,
                pinned_unix: None,
                archived_unix: None,
            })
            .await
            .unwrap()
    }

    async fn read(&self) -> Loaded {
        self.app
            .config
            .chat_store
            .load(OWNER, CHAT)
            .await
            .unwrap()
            .unwrap()
    }

    async fn request(
        &self,
        method: Method,
        uri: &str,
        owner: &str,
        fields: &[(&str, &str)],
    ) -> (StatusCode, String) {
        let mut builder = HttpRequest::builder()
            .method(method.clone())
            .uri(uri)
            .header(header::HOST, HOST)
            .header(header::COOKIE, format!("oa_visitor={owner}"))
            .header("HX-Request", "true");
        let body = if method == Method::POST {
            builder = builder
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, format!("http://{HOST}"))
                .header("Sec-Fetch-Site", "same-origin");
            Body::from(form(fields))
        } else {
            Body::empty()
        };
        let response = self
            .router
            .clone()
            .oneshot(builder.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let html = response
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|kind| kind.as_bytes().starts_with(b"text/html"));
        let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
            .await
            .unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        if html {
            // #11031: no chat page or fragment shows machine talk.
            crate::copy_guard::assert_plain(uri, &body);
        }
        (status, body)
    }

    fn no_worker(&self) {
        assert_eq!(self.worker_calls.load(Ordering::SeqCst), 0);
    }
}

fn form(fields: &[(&str, &str)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.extend_pairs(fields.iter().copied());
    serializer.finish()
}

fn source() -> Selection {
    Selection {
        revision: 1,
        repository: Some(RepositorySource {
            repository: "OpenAgentsInc/openagents".into(),
            branch: "main".into(),
            revision: "a".repeat(40),
        }),
        runtime: None,
    }
}

fn native() -> Selection {
    let mut selection = source();
    selection.runtime = Some(RuntimeSelection {
        binding: "fixture-host".into(),
        account: "fixture-account".into(),
        workspace: "fixture-workspace".into(),
        members_epoch: 7,
        project: "openagents".into(),
        profile: "boat-codex".into(),
        profile_revision: format!("sha256:{}", "b".repeat(64)),
        source_revision: "a".repeat(40),
        source_digest: format!("sha256:{}", "c".repeat(64)),
        placement: "boat".into(),
        executor: "codex".into(),
        model: Some("synthetic".into()),
        max_timeout_seconds: 300,
    });
    selection
}

#[tokio::test]
async fn selector_csrf_refusal_preserves_the_conversation() {
    let fixture = Fixture::new();
    let initial = fixture.record(Some(source())).await;
    let token = crate::composer::seal(&fixture.app, OWNER, &source());
    let (status, _) = fixture
        .request(
            Method::POST,
            "/composer/repository",
            OWNER,
            &[
                ("selection", &token),
                ("csrf", "changed"),
                ("chat", CHAT),
                ("value", "none"),
            ],
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let retained = fixture.read().await;
    assert_eq!(retained.generation, initial.generation);
    assert_eq!(retained.conversation.selection, Some(source()));
    fixture.no_worker();
}

#[tokio::test]
async fn selection_changes_freeze_accepted_sources_and_reject_stale_followups() {
    let fixture = Fixture::new();
    let initial = fixture.record(Some(source())).await;
    let old_token = crate::composer::seal(&fixture.app, OWNER, &source());
    let csrf = csrf(&fixture.app, OWNER);
    let (status, body) = fixture
        .request(
            Method::POST,
            "/composer/repository",
            OWNER,
            &[
                ("selection", &old_token),
                ("csrf", &csrf),
                ("chat", CHAT),
                ("value", "none"),
            ],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body.matches("id=\"composer-state\"").count(), 1);
    assert_eq!(body.matches("id=\"composer-controls\"").count(), 1);
    assert!(body.contains("hx-swap-oob=\"outerHTML\""));
    assert!(!body.contains("<textarea") && !body.contains("id=\"chat-form\""));
    let retained = fixture.read().await;
    assert_eq!(retained.conversation.revision, 2);
    assert_eq!(
        retained.conversation.selection,
        Some(Selection {
            revision: 2,
            repository: None,
            runtime: None,
        })
    );
    assert_eq!(retained.conversation.requests.len(), 1);
    assert_eq!(retained.conversation.requests[0].selection, Some(source()));
    assert_eq!(
        retained.conversation.requests[0].digest,
        initial.conversation.requests[0].digest
    );
    assert_eq!(retained.conversation.messages.len(), 2);
    let (status, body) = fixture
        .request(
            Method::POST,
            &format!("/chat/{CHAT}"),
            OWNER,
            &[
                ("q", "Another question"),
                ("request_id", NEXT),
                ("csrf", &csrf),
                ("selection", &old_token),
            ],
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(fixture.read().await.generation, retained.generation);
    fixture.no_worker();
}

#[tokio::test]
async fn changed_and_other_browser_source_tickets_cannot_create_chats() {
    let fixture = Fixture::new();
    let token = crate::composer::seal(&fixture.app, OWNER, &source());
    let (payload, tag) = token.split_once('.').unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).unwrap()).unwrap();
    value["repository"]["branch"] = serde_json::json!("other");
    let changed = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&value).unwrap()),
        tag
    );
    for (owner, ticket) in [(OWNER, &changed), (OTHER_OWNER, &token)] {
        let csrf = csrf(&fixture.app, owner);
        let (status, body) = fixture
            .request(
                Method::POST,
                "/chat",
                owner,
                &[
                    ("q", TEXT),
                    ("request_id", CHAT),
                    ("csrf", &csrf),
                    ("selection", ticket),
                ],
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert!(
            fixture
                .app
                .config
                .chat_store
                .load(owner, CHAT)
                .await
                .unwrap()
                .is_none()
        );
    }
    fixture.no_worker();
}

#[tokio::test]
async fn unconfigured_native_selection_cannot_stage_or_dispatch_work() {
    let fixture = Fixture::new();
    let token = crate::composer::seal(&fixture.app, OWNER, &native());
    let csrf = csrf(&fixture.app, OWNER);
    let (status, body) = fixture
        .request(
            Method::POST,
            "/chat",
            OWNER,
            &[
                ("q", "Build this repository"),
                ("request_id", CHAT),
                ("csrf", &csrf),
                ("selection", &token),
            ],
        )
        .await;
    assert_eq!(status, StatusCode::GONE, "{body}");
    assert!(
        fixture
            .app
            .config
            .chat_store
            .load(OWNER, CHAT)
            .await
            .unwrap()
            .is_none()
    );
    let default_token = crate::composer::seal(&fixture.app, OWNER, &Selection::default());
    let (status, body) = fixture
        .request(
            Method::GET,
            &format!(
                "/composer/environment?{}",
                form(&[("selection", &default_token)])
            ),
            OWNER,
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("No environments yet."));
    assert!(!body.contains("/cloud/app"));
    fixture.no_worker();
}

#[tokio::test]
async fn retained_native_source_requires_fresh_authority_without_a_cloud_request() {
    let fixture = Fixture::new();
    let mut selected = native();
    selected.repository.as_mut().unwrap().repository = "fixture-owner/private-fixture".into();
    let retained = fixture.record(Some(selected.clone())).await;
    let token = crate::composer::seal(&fixture.app, OWNER, &selected);
    let csrf = csrf(&fixture.app, OWNER);
    for uri in [
        format!("/chat/{CHAT}"),
        format!("/chat/{CHAT}/transcript"),
        format!("/composer/context?{}", form(&[("selection", &token)])),
    ] {
        let (status, body) = fixture.request(Method::GET, &uri, OWNER, &[]).await;
        assert_eq!(status, StatusCode::GONE, "{body}");
        assert!(!body.contains("fixture-owner/private-fixture"), "{body}");
    }
    let (status, body) = fixture
        .request(
            Method::POST,
            "/composer/environment",
            OWNER,
            &[
                ("selection", &token),
                ("csrf", &csrf),
                ("chat", CHAT),
                ("value", "none"),
            ],
        )
        .await;
    assert_eq!(status, StatusCode::GONE, "{body}");
    assert!(!body.contains("fixture-owner/private-fixture"), "{body}");
    assert_eq!(fixture.read().await.generation, retained.generation);
    fixture.no_worker();
}

#[tokio::test]
async fn legacy_request_replay_does_not_dispatch_another_answer() {
    let fixture = Fixture::new();
    let retained = fixture.record(None).await;
    assert_eq!(retained.conversation.requests[0].digest, digest(TEXT));
    let csrf = csrf(&fixture.app, OWNER);
    let (status, body) = fixture
        .request(
            Method::POST,
            &format!("/chat/{CHAT}"),
            OWNER,
            &[("q", TEXT), ("request_id", CHAT), ("csrf", &csrf)],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("id=\"chat-ticket\""));
    assert_eq!(fixture.read().await.generation, retained.generation);
    fixture.no_worker();
}

#[tokio::test]
async fn chat_and_information_panels_render_semantic_controls() {
    let fixture = Fixture::new();
    fixture.record(Some(source())).await;
    let token = crate::composer::seal(&fixture.app, OWNER, &source());
    let (status, body) = fixture
        .request(Method::GET, &format!("/chat/{CHAT}"), OWNER, &[])
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for id in [
        "chat-form",
        "chat-input",
        "composer-controls",
        "composer-state",
        "composer-panel",
    ] {
        assert_eq!(body.matches(&format!("id=\"{id}\"")).count(), 1, "{id}");
    }
    // A chat that pins a source shows the source selectors; the context,
    // model and voice buttons are commented out until they work.
    for kind in ["repository", "branch", "environment"] {
        assert!(
            body.contains(&format!("hx-get=\"/composer/{kind}\"")),
            "{kind}"
        );
    }
    for kind in ["context", "model", "voice"] {
        assert!(
            !body.contains(&format!("hx-get=\"/composer/{kind}\"")),
            "{kind}"
        );
    }
    assert!(body.contains("/static/chat-start.js"));
    assert!(body.contains("hx-disabled-elt=\"#chat-form button[type=submit]\""));
    assert!(!body.contains("hx-disabled-elt=\"find button[type=submit]\""));
    for retired in ["OriginalText", "/components/assets/", "coder_web.js"] {
        assert!(!body.contains(retired), "{retired}");
    }
    for (kind, expected) in [
        ("context", "OpenAgentsInc/openagents"),
        ("model", "picks a model for you"),
        ("voice", "Type your message instead."),
    ] {
        let (status, body) = fixture
            .request(
                Method::GET,
                &format!(
                    "/composer/{kind}?{}",
                    form(&[("selection", &token), ("chat", CHAT)])
                ),
                OWNER,
                &[],
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body.contains(expected), "{body}");
    }
    fixture.no_worker();
}

#[test]
fn sidebar_rows_show_the_repository_and_a_plain_status() {
    let mut chat = Conversation {
        id: CHAT.into(),
        owner: OWNER.into(),
        revision: 1,
        title: "Row".into(),
        messages: Vec::new(),
        pending: None,
        requests: Vec::new(),
        selection: None,
        updated_unix: 1,
        pinned_unix: None,
        archived_unix: None,
    };
    assert_eq!(row_detail(&chat), None);
    assert_eq!(row_status(&chat), None);
    chat.selection = Some(Selection {
        revision: 1,
        repository: Some(RepositorySource {
            repository: "acme/app".into(),
            branch: "main".into(),
            revision: "0".repeat(40),
        }),
        runtime: None,
    });
    assert_eq!(row_detail(&chat).as_deref(), Some("acme/app · main"));
    chat.requests.push(Request {
        id: CHAT.into(),
        digest: String::new(),
        outcome: Outcome::Failed,
        selection: None,
        cloud: None,
        reply: None,
    });
    assert_eq!(row_status(&chat), Some(ChatStatus::Failed));
    chat.pending = Some(Pending {
        request_id: NEXT.into(),
        started_unix: 2,
        job_id: None,
    });
    assert_eq!(row_status(&chat), Some(ChatStatus::Working));
}

#[tokio::test]
async fn pin_rename_search_and_archive_work_end_to_end() {
    let fixture = Fixture::new();
    let initial = fixture.record(None).await;
    let csrf = csrf(&fixture.app, OWNER);
    let row = format!("id=\"chat-row-{CHAT}\"");

    // The list carries the row menu and the search box.
    let (status, body) = fixture
        .request(
            Method::GET,
            &format!("/chat/list?current={CHAT}"),
            OWNER,
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains(&row) && body.contains("Search chats"),
        "{body}"
    );
    assert!(body.contains(">Pin<") && body.contains(">Rename<") && body.contains(">Archive<"));
    assert!(!body.contains(">Pinned<") && !body.contains("Archived chats"));

    // A wrong token or another visitor changes nothing.
    let (status, _) = fixture
        .request(
            Method::POST,
            &format!("/chat/{CHAT}/pin"),
            OWNER,
            &[("csrf", "changed"), ("pinned", "1")],
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let other = super::csrf(&fixture.app, OTHER_OWNER);
    let (status, _) = fixture
        .request(
            Method::POST,
            &format!("/chat/{CHAT}/pin"),
            OTHER_OWNER,
            &[("csrf", &other), ("pinned", "1")],
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(fixture.read().await.generation, initial.generation);

    // Pin: the chat moves to the Pinned group and the menu offers Unpin.
    let (status, body) = fixture
        .request(
            Method::POST,
            &format!("/chat/{CHAT}/pin"),
            OWNER,
            &[("csrf", &csrf), ("current", CHAT), ("pinned", "1")],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains(">Pinned<") && body.contains(">Unpin<"),
        "{body}"
    );
    let pinned = fixture.read().await.conversation;
    assert!(pinned.pinned_unix.is_some());
    assert_eq!(pinned.revision, 2);
    assert_eq!(
        pinned.updated_unix, 1,
        "pinning keeps the chat's place in time"
    );

    // Rename: the field replaces the row, then the new name is saved and the
    // open chat's header follows.
    let (status, body) = fixture
        .request(
            Method::GET,
            &format!("/chat/{CHAT}/rename?current={CHAT}"),
            OWNER,
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains(&row) && body.contains("data-oa-rename"));
    assert!(body.contains(r#"value="Existing conversation""#), "{body}");
    let long = "x".repeat(121);
    for bad in ["   ", long.as_str(), "two\nlines"] {
        let (status, _) = fixture
            .request(
                Method::POST,
                &format!("/chat/{CHAT}/rename"),
                OWNER,
                &[("csrf", &csrf), ("current", CHAT), ("title", bad)],
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, body) = fixture
        .request(
            Method::POST,
            &format!("/chat/{CHAT}/rename"),
            OWNER,
            &[("csrf", &csrf), ("current", CHAT), ("title", "  New name ")],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains(">New name<") && body.contains("hx-swap-oob"),
        "{body}"
    );
    assert_eq!(fixture.read().await.conversation.title, "New name");

    // Search: titles as you type, message text from two characters.
    for (q, found) in [("NEW", true), ("s", false), ("sa", true), ("zzz", false)] {
        let (status, body) = fixture
            .request(
                Method::GET,
                &format!("/chat/list?current={CHAT}&q={q}"),
                OWNER,
                &[],
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body.contains(&row), found, "{q}: {body}");
        assert_eq!(body.contains("No chats found"), !found, "{q}: {body}");
    }

    // Archive: the row leaves the list with an Undo notice, and the
    // Archived page lists it with Restore.
    let (status, body) = fixture
        .request(
            Method::POST,
            &format!("/chat/{CHAT}/archive"),
            OWNER,
            &[("csrf", &csrf), ("current", CHAT), ("archived", "1")],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains(&row), "{body}");
    assert!(
        body.contains("Chat archived.") && body.contains(">Undo<"),
        "{body}"
    );
    assert!(body.contains("href=\"/chat/archived\""));
    assert!(fixture.read().await.conversation.archived_unix.is_some());
    let (status, body) = fixture
        .request(Method::GET, "/chat/archived", OWNER, &[])
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("New name") && body.contains("Restore"),
        "{body}"
    );
    let (status, body) = fixture
        .request(
            Method::POST,
            &format!("/chat/{CHAT}/archive"),
            OWNER,
            &[("csrf", &csrf), ("archived", "0"), ("back", "archived")],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains(&row) && !body.contains("Chat archived."),
        "{body}"
    );
    let restored = fixture.read().await.conversation;
    assert!(restored.archived_unix.is_none() && restored.pinned_unix.is_some());
    assert_eq!(restored.requests.len(), initial.conversation.requests.len());
    fixture.no_worker();
}

#[test]
fn pinned_chats_keep_pin_order_and_archived_chats_leave_the_list() {
    let chat = |id: &str, title: &str| Conversation {
        id: id.into(),
        owner: OWNER.into(),
        revision: 1,
        title: title.into(),
        messages: Vec::new(),
        pending: None,
        requests: Vec::new(),
        selection: None,
        updated_unix: 1,
        pinned_unix: None,
        archived_unix: None,
    };
    let mut first = chat(CHAT, "First pinned");
    first.pinned_unix = Some(5);
    let mut second = chat(NEXT, "Second pinned");
    second.pinned_unix = Some(9);
    let mut working = chat("32345678-1234-4234-8234-123456789abc", "Working one");
    working.pending = Some(Pending {
        request_id: NEXT.into(),
        started_unix: 2,
        job_id: None,
    });
    let mut gone = chat("42345678-1234-4234-8234-123456789abc", "Archived one");
    gone.archived_unix = Some(3);
    // The store lists newest first; pins still show in pin order.
    let rows = vec![second, working, gone, first];
    let html = sidebar::build(
        ChatList::new().id("chat-sidebar"),
        &rows,
        "token",
        sidebar::View::default(),
    )
    .render()
    .into_string();
    let at = |text: &str| html.find(text).unwrap_or_else(|| panic!("{text}: {html}"));
    assert!(at(">Pinned<") < at("First pinned"));
    assert!(at("First pinned") < at("Second pinned"));
    assert!(at("Second pinned") < at(">Chats<"));
    assert!(at(">Chats<") < at("Working one"));
    assert!(!html.contains("Archived one"));
    assert!(html.contains("hx-confirm=\"This chat is still working. Archive it anyway?\""));
    assert_eq!(html.matches("hx-confirm").count(), 1);
    assert!(html.contains("href=\"/chat/archived\""));
    // Rows on a page without an open chat are plain links.
    assert!(!html.contains("/workspace"));
    assert_eq!(sidebar::clean_title("  Fine  ").as_deref(), Some("Fine"));
    assert_eq!(sidebar::clean_title("tab\there"), None);
}
