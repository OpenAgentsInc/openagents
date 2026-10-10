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
                project: None,
                terminal: None,
                environment: None,
                tasks: Vec::new(),
                opened_unix: None,
                branch: None,
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
    assert_eq!(body.matches("id=\"composer-controls\"").count(), 0);
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
        "composer-row",
        "composer-state",
        "composer-panel",
    ] {
        assert_eq!(body.matches(&format!("id=\"{id}\"")).count(), 1, "{id}");
    }
    // The selector row is empty for a visitor who isn't signed in (it
    // offers projects; `crate::composer_row`), and the old source
    // selectors are gone; the context, model and voice buttons are
    // commented out until they work.
    assert!(body.contains(r#"<div class="oa-composer-selector-group" id="composer-row"></div>"#));
    for kind in [
        "repository",
        "branch",
        "environment",
        "context",
        "model",
        "voice",
    ] {
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
        project: None,
        terminal: None,
        environment: None,
        tasks: Vec::new(),
        opened_unix: None,
        branch: None,
    };
    assert_eq!(line_two(&chat, true), None);
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
    assert_eq!(line_two(&chat, true).as_deref(), Some("acme/app · main"));
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
        // The search box is hidden until people have more chats.
        body.contains(&row) && !body.contains("Search chats"),
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
        project: None,
        terminal: None,
        environment: None,
        tasks: Vec::new(),
        opened_unix: None,
        branch: None,
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

#[tokio::test]
async fn delete_asks_first_then_removes_only_the_owners_chat() {
    let fixture = Fixture::new();
    fixture.record(None).await;
    let delete = format!("/chat/{CHAT}/delete");
    let store = fixture.app.config.chat_store.clone();
    let stored = move || {
        let store = store.clone();
        async move { store.load(OWNER, CHAT).await.unwrap() }
    };

    // The chat's row menu opens the confirm step; the confirm step asks once.
    let (status, body) = fixture
        .request(Method::GET, &format!("/chat/{CHAT}"), OWNER, &[])
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains(&format!("action=\"{delete}\"")), "{body}");
    let (status, body) = fixture.request(Method::GET, &delete, OWNER, &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("Delete this chat? This can&#39;t be undone.")
            || body.contains("Delete this chat? This can't be undone."),
        "{body}"
    );
    assert!(body.contains(&format!("action=\"{delete}\"")), "{body}");
    assert!(stored().await.is_some());

    // Another browser can neither see the confirm step nor delete the chat.
    let (status, _) = fixture
        .request(Method::GET, &delete, OTHER_OWNER, &[])
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let theirs = csrf(&fixture.app, OTHER_OWNER);
    let (status, _) = fixture
        .request(Method::POST, &delete, OTHER_OWNER, &[("csrf", &theirs)])
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // A form without this browser's token is refused.
    let (status, _) = fixture
        .request(Method::POST, &delete, OWNER, &[("csrf", &theirs)])
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(stored().await.is_some());

    let token = csrf(&fixture.app, OWNER);
    let (status, _) = fixture
        .request(Method::POST, &delete, OWNER, &[("csrf", &token)])
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert!(stored().await.is_none());
    let (status, _) = fixture
        .request(Method::GET, &format!("/chat/{CHAT}"), OWNER, &[])
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = fixture
        .request(Method::POST, &delete, OWNER, &[("csrf", &token)])
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    fixture.no_worker();
}

#[tokio::test]
async fn delete_waits_for_a_running_answer() {
    let fixture = Fixture::new();
    let loaded = fixture.record(None).await;
    let mut next = loaded.conversation.clone();
    next.revision += 1;
    next.pending = Some(Pending {
        request_id: NEXT.into(),
        started_unix: now(),
        job_id: None,
    });
    fixture
        .app
        .config
        .chat_store
        .compare_and_swap(&loaded, &next)
        .await
        .unwrap();
    let token = csrf(&fixture.app, OWNER);
    let (status, body) = fixture
        .request(
            Method::POST,
            &format!("/chat/{CHAT}/delete"),
            OWNER,
            &[("csrf", &token)],
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(body.contains("Wait for the answer to finish"), "{body}");
    assert!(
        fixture
            .app
            .config
            .chat_store
            .load(OWNER, CHAT)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn delete_all_asks_first_then_removes_only_this_browsers_chats() {
    let fixture = Fixture::new();
    fixture.record(None).await;
    let store = fixture.app.config.chat_store.clone();
    let mut theirs = fixture.read().await.conversation;
    theirs.owner = OTHER_OWNER.into();
    store.create(&theirs).await.unwrap();
    let path = delete_all::PATH;

    // Signed out, the Archived chats page links the confirm step.
    let (status, body) = fixture
        .request(Method::GET, "/chat/archived", OWNER, &[])
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains(&format!("href=\"{path}\"")), "{body}");
    // So does a chat's own delete step.
    let (status, body) = fixture
        .request(Method::GET, &format!("/chat/{CHAT}/delete"), OWNER, &[])
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains(&format!("href=\"{path}\"")), "{body}");
    let (status, body) = fixture.request(Method::GET, path, OWNER, &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("Delete your 1 chat in this browser?"),
        "{body}"
    );
    assert!(body.contains(&format!("action=\"{path}\"")), "{body}");

    // Another browser's token is refused.
    let wrong = csrf(&fixture.app, OTHER_OWNER);
    let (status, _) = fixture
        .request(Method::POST, path, OWNER, &[("csrf", &wrong)])
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(store.load(OWNER, CHAT).await.unwrap().is_some());

    let token = csrf(&fixture.app, OWNER);
    let (status, _) = fixture
        .request(Method::POST, path, OWNER, &[("csrf", &token)])
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert!(store.list(OWNER).await.unwrap().is_empty());
    // The other browser keeps its chat.
    assert!(store.load(OTHER_OWNER, CHAT).await.unwrap().is_some());

    // With nothing left, the page says so and offers no Delete button.
    let (status, body) = fixture.request(Method::GET, path, OWNER, &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("You have no saved chats."), "{body}");
    assert!(!body.contains(&format!("action=\"{path}\"")), "{body}");
    let (_, body) = fixture
        .request(Method::GET, "/chat/archived", OWNER, &[])
        .await;
    assert!(!body.contains(&format!("href=\"{path}\"")), "{body}");
    fixture.no_worker();
}

#[tokio::test]
async fn delete_all_leaves_a_chat_still_being_answered() {
    let fixture = Fixture::new();
    let loaded = fixture.record(None).await;
    let mut next = loaded.conversation.clone();
    next.revision += 1;
    next.pending = Some(Pending {
        request_id: NEXT.into(),
        started_unix: now(),
        job_id: None,
    });
    let store = fixture.app.config.chat_store.clone();
    store.compare_and_swap(&loaded, &next).await.unwrap();
    let token = csrf(&fixture.app, OWNER);
    let (status, body) = fixture
        .request(Method::POST, delete_all::PATH, OWNER, &[("csrf", &token)])
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(body.contains("1 chat is still being answered"), "{body}");
    assert!(store.load(OWNER, CHAT).await.unwrap().is_some());
}

#[test]
fn delete_all_says_whose_chats_go() {
    let account = crate::chat_store::account_owner("acct_1");
    assert_eq!(
        delete_all::question(&account, 3),
        "Delete all 3 chats on your account? This can't be undone."
    );
    assert_eq!(
        delete_all::question(OWNER, 1),
        "Delete your 1 chat in this browser? This can't be undone."
    );
}

#[tokio::test]
async fn an_accounts_chats_never_open_with_a_browser_cookie() {
    let fixture = Fixture::new();
    fixture.record(None).await;
    let account = crate::chat_store::account_owner("acct_1");
    let store = fixture.app.config.chat_store.clone();
    // Move the browser's chat to an account, as signing in does.
    assert_eq!(
        crate::chat_owner::adopt_all(&store, OWNER, &account).await,
        1
    );
    // The browser that made it no longer opens it, follows it, or deletes it.
    for uri in [format!("/chat/{CHAT}"), format!("/chat/{CHAT}/events")] {
        let (status, _) = fixture.request(Method::GET, &uri, OWNER, &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
    let token = csrf(&fixture.app, OWNER);
    let (status, _) = fixture
        .request(
            Method::POST,
            &format!("/chat/{CHAT}/delete"),
            OWNER,
            &[("csrf", &token)],
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = fixture
        .request(Method::POST, delete_all::PATH, OWNER, &[("csrf", &token)])
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(store.list(&account).await.unwrap().len(), 1);
    // An account's owner value is not a cookie this site accepts.
    let (status, _) = fixture
        .request(Method::GET, &format!("/chat/{CHAT}"), &account, &[])
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// #11035: a page's sidebar holds one live connection for row statuses,
/// beside the list so the list's own replacements keep it.
#[tokio::test]
async fn the_sidebar_follows_row_statuses_live() {
    let fixture = Fixture::new();
    fixture.record(None).await;
    let get = |uri: &str, cookie: Option<&str>| {
        let mut builder = HttpRequest::builder()
            .method(Method::GET)
            .uri(uri)
            .header(header::HOST, HOST);
        if let Some(owner) = cookie {
            builder = builder.header(header::COOKIE, format!("oa_visitor={owner}"));
        }
        fixture
            .router
            .clone()
            .oneshot(builder.body(Body::empty()).unwrap())
    };
    let response = get(&format!("/chat/{CHAT}"), Some(OWNER)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 2 * 1024 * 1024)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(
        body.matches(r#"id="chat-sidebar-live""#).count(),
        1,
        "{body}"
    );
    assert!(
        body.contains(r#"sse-connect="/chats/events?after="#),
        "{body}"
    );

    // Someone without chats gets no stream, and is told not to reconnect.
    let response = get("/chats/events", None).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = get("/chats/events?after=1", Some(OWNER)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/event-stream"
    );
    assert!(
        response
            .headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .is_some()
    );
    fixture.no_worker();
}

/// A Coder chat synced to the account (#11047): marked in the sidebar,
/// read-only, Working from Coder's heartbeat, and deleted for Coder too.
#[tokio::test]
async fn a_coder_chat_opens_read_only_with_its_computer() {
    let fixture = Fixture::new();
    let store = fixture.app.config.chat_store.clone();
    let mut chat = Conversation {
        id: CHAT.into(),
        owner: OWNER.into(),
        revision: 1,
        title: "Fix the build".into(),
        messages: vec![
            Message {
                role: Role::User,
                text: "Fix the build".into(),
                request_id: None,
            },
            Message {
                role: Role::Assistant,
                text: "Fixed **it**.".into(),
                request_id: None,
            },
        ],
        pending: None,
        requests: Vec::new(),
        selection: None,
        updated_unix: 1,
        pinned_unix: None,
        archived_unix: None,
        project: None,
        terminal: Some(crate::chat_store::Terminal {
            computer: "Studio".into(),
            session: "coder-new-1".into(),
            title: "Fix the build".into(),
            digest: "d".repeat(64),
            working_unix: None,
            deleted_unix: None,
            replies: Vec::new(),
            reply_ids: Vec::new(),
            continued: Vec::new(),
            continued_taken: 0,
        }),
        environment: None,
        tasks: Vec::new(),
        opened_unix: None,
        branch: None,
    };
    assert_eq!(line_two(&chat, true).as_deref(), Some("Terminal · Studio"));
    assert_eq!(row_status(&chat), None);
    chat.terminal.as_mut().unwrap().working_unix = Some(now());
    assert_eq!(row_status(&chat), Some(ChatStatus::Working));
    chat.terminal.as_mut().unwrap().working_unix = Some(now() - 600);
    assert_eq!(row_status(&chat), None);
    chat.terminal.as_mut().unwrap().working_unix = None;
    store.create(&chat).await.unwrap();

    let page = format!("/chat/{CHAT}");
    let (status, body) = fixture.request(Method::GET, &page, OWNER, &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("This chat runs in Coder on Studio."),
        "{body}"
    );
    assert!(body.contains("Terminal · Studio"), "{body}");
    assert!(body.contains("<strong>it</strong>"), "{body}");
    assert!(!body.contains("id=\"chat-form\""), "{body}");

    // It can't be continued here, and a row click opens it as a page.
    let token = csrf(&fixture.app, OWNER);
    let (status, _) = fixture
        .request(
            Method::POST,
            &page,
            OWNER,
            &[("q", "More"), ("request_id", NEXT), ("csrf", &token)],
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        store
            .load(OWNER, CHAT)
            .await
            .unwrap()
            .unwrap()
            .conversation
            .messages
            .len(),
        2
    );

    // The confirm step says Coder loses it too; the delete hides it here
    // and keeps a marker until Coder hears of it.
    let delete = format!("/chat/{CHAT}/delete");
    let (_, body) = fixture.request(Method::GET, &delete, OWNER, &[]).await;
    assert!(body.contains("deleted in Coder on Studio too"), "{body}");
    let (status, _) = fixture
        .request(Method::POST, &delete, OWNER, &[("csrf", &token)])
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert!(store.list(OWNER).await.unwrap().is_empty());
    let marker = store.load(OWNER, CHAT).await.unwrap().unwrap().conversation;
    assert!(marker.deleted() && marker.messages.is_empty());
    let (status, _) = fixture.request(Method::GET, &page, OWNER, &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    fixture.no_worker();
}

/// A Coder chat takes a reply from the website while Coder on its computer
/// is online (#11048): the composer shows, the reply waits for Coder, and
/// another visitor can't send one.
#[tokio::test]
async fn a_coder_chat_takes_a_reply_while_its_computer_is_online() {
    use crate::coder_sync::{Upload, WireMessage};
    let fixture = Fixture::new();
    let store = fixture.app.config.chat_store.clone();
    let upload = Upload {
        computer: "Studio".into(),
        title: "Fix the build".into(),
        messages: vec![WireMessage {
            role: "user".into(),
            text: "Fix the build".into(),
        }],
    };
    crate::coder_sync::save(&store, OWNER, "coder-new-1", &upload)
        .await
        .unwrap();
    let chat = crate::coder_sync::chat_id(OWNER, "coder-new-1");
    let page = format!("/chat/{chat}");

    // Offline: one plain line, no composer.
    let (status, body) = fixture.request(Method::GET, &page, OWNER, &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("To reply here, open Coder there"), "{body}");
    assert!(!body.contains("id=\"chat-form\""), "{body}");

    // Coder on Studio checks in: the composer shows.
    crate::coder_sync::check_in(&store, OWNER, "Studio")
        .await
        .unwrap()
        .unwrap();
    let (status, body) = fixture.request(Method::GET, &page, OWNER, &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("id=\"chat-form\""), "{body}");
    assert!(body.contains("Reply to Coder on Studio"), "{body}");
    assert!(!body.contains("chat-terminal-note"), "{body}");

    // Another visitor can't send to it.
    let other = csrf(&fixture.app, OTHER_OWNER);
    let (status, _) = fixture
        .request(
            Method::POST,
            &page,
            OTHER_OWNER,
            &[("q", "Hi"), ("request_id", NEXT), ("csrf", &other)],
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The owner's reply waits for Coder and shows in the thread.
    let token = csrf(&fixture.app, OWNER);
    let (status, body) = fixture
        .request(
            Method::POST,
            &page,
            OWNER,
            &[
                ("q", "Now the tests"),
                ("request_id", NEXT),
                ("csrf", &token),
            ],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, body) = fixture
        .request(Method::GET, &format!("{page}/transcript"), OWNER, &[])
        .await;
    assert!(body.contains("Now the tests"), "{body}");
    assert!(body.contains("Waiting for Coder on Studio."), "{body}");
    assert_eq!(
        crate::coder_sync::check_in(&store, OWNER, "Studio")
            .await
            .unwrap(),
        Ok(vec!["coder-new-1".to_string()])
    );
    fixture.no_worker();
}

/// #11035: a task that ran over a minute and finished makes the row say
/// Done until the chat is opened; opening it records that, and the row
/// goes quiet.
#[tokio::test]
async fn a_long_finished_task_says_done_until_the_chat_is_opened() {
    use crate::chat_store::{ChatTask, TaskKind, TaskState};
    let fixture = Fixture::new();
    let loaded = fixture.record(None).await;
    let mut next = loaded.conversation.clone();
    next.revision += 1;
    next.tasks = vec![ChatTask {
        id: "claude-env-1-1".into(),
        kind: TaskKind::Claude,
        environment: "env-1".into(),
        title: "Fix the login redirect".into(),
        state: TaskState::Done,
        started_unix: 100,
        after_message: 2,
        version: Some(3),
        finished_unix: Some(400),
        agent: None,
    }];
    fixture
        .app
        .config
        .chat_store
        .compare_and_swap(&loaded, &next)
        .await
        .unwrap();
    assert_eq!(
        row_status(&fixture.read().await.conversation),
        Some(ChatStatus::Done)
    );
    let (status, body) = fixture
        .request(Method::GET, &format!("/chat/{CHAT}"), OWNER, &[])
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let opened = fixture.read().await.conversation;
    assert!(opened.opened_unix.is_some_and(|at| at >= 400));
    assert_eq!(row_status(&opened), None);
    // Opening it again writes nothing.
    let before = fixture.read().await.generation;
    fixture
        .request(Method::GET, &format!("/chat/{CHAT}"), OWNER, &[])
        .await;
    assert_eq!(fixture.read().await.generation, before);
}

/// An answered reply sits between hidden markers that name how it was
/// served, so the chat goldens read a reply from the page exactly as a
/// person gets it (docs/web/chat-goldens.md); a reply still streaming has
/// the markers without a tier. None of it is visible.
#[test]
fn an_answered_reply_carries_its_served_tier_route_and_answer() {
    let mut chat = Conversation {
        id: CHAT.into(),
        owner: OWNER.into(),
        revision: 2,
        title: "Repo".into(),
        messages: vec![
            Message {
                role: Role::User,
                text: "how do i connect github repo".into(),
                request_id: Some(CHAT.into()),
            },
            Message {
                role: Role::Assistant,
                text: "Open Projects and connect GitHub.".into(),
                request_id: Some(CHAT.into()),
            },
        ],
        pending: None,
        requests: vec![Request {
            id: CHAT.into(),
            digest: String::new(),
            outcome: Outcome::Answered,
            selection: None,
            cloud: None,
            reply: Some(openagents_chat::router::Meta {
                tier: Some("canned".into()),
                route: Some("meta".into()),
                answer: Some("meta.github.website@1".into()),
                ..openagents_chat::router::Meta::default()
            }),
        }],
        selection: None,
        updated_unix: 1,
        pinned_unix: None,
        archived_unix: None,
        project: None,
        terminal: None,
        environment: None,
        tasks: Vec::new(),
        opened_unix: None,
        branch: None,
    };
    let html = messages(&chat, None, false).into_string();
    assert!(html.contains(r#"data-oa-reply="1""#), "{html}");
    assert!(html.contains(r#"data-oa-tier="canned""#), "{html}");
    assert!(html.contains(r#"data-oa-route="meta""#), "{html}");
    assert!(
        html.contains(r#"data-oa-answer="meta.github.website@1""#),
        "{html}"
    );
    assert!(html.contains("data-oa-reply-end"), "{html}");
    let visible = oa_copy::visible_text(&html);
    assert!(!visible.contains("canned"), "{visible}");
    chat.requests[0].outcome = Outcome::Pending;
    let streaming = messages(&chat, None, false).into_string();
    assert!(streaming.contains(r#"data-oa-reply="1""#));
    assert!(!streaming.contains("data-oa-tier"), "{streaming}");
}

/// A reply still streaming shows only what renders cleanly so far: no
/// half-written fence, table row, link, list marker, emphasis, or heading
/// shows as raw syntax, and it is marked to grow smoothly. Once answered
/// it renders exactly as the whole text renders in one go (#11112).
#[test]
fn a_streaming_reply_never_shows_half_written_markdown() {
    let full = "## Steps\n\n1. Run **cargo build**.\n2. See [the docs](https://openagents.com/docs).\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```sh\ncargo test\n```\n";
    let mut chat = Conversation {
        id: CHAT.into(),
        owner: OWNER.into(),
        revision: 2,
        title: "Steps".into(),
        messages: vec![
            Message {
                role: Role::User,
                text: "how do i build".into(),
                request_id: Some(CHAT.into()),
            },
            Message {
                role: Role::Assistant,
                text: String::new(),
                request_id: Some(CHAT.into()),
            },
        ],
        pending: Some(Pending {
            request_id: CHAT.into(),
            started_unix: 1,
            job_id: None,
        }),
        requests: vec![Request {
            id: CHAT.into(),
            digest: String::new(),
            outcome: Outcome::Pending,
            selection: None,
            cloud: None,
            reply: None,
        }],
        selection: None,
        updated_unix: 1,
        pinned_unix: None,
        archived_unix: None,
        project: None,
        terminal: None,
        environment: None,
        tasks: Vec::new(),
        opened_unix: None,
        branch: None,
    };
    for (cut, hidden) in [
        ("## Steps\n\n1. Run **cargo bu", "**"),
        (
            "## Steps\n\n1. Run **cargo build**.\n2. See [the docs](https",
            "](",
        ),
        ("## Steps\n\n1. Run **cargo build**.\n2.", "2."),
        ("## Steps\n\n| a | b |\n|---|---|\n| 1 |", "| 1"),
        ("## Steps\n\n| a | b |\n", "| a"),
        ("## Steps\n\n```sh\ncargo te", "```"),
        ("## Steps\n\n##", "##"),
    ] {
        chat.messages[1].text = cut.into();
        let html = messages(&chat, None, false).into_string();
        assert!(html.contains("data-oa-streaming"), "{html}");
        let visible = oa_copy::visible_text(&html);
        assert!(
            !visible.contains(hidden),
            "{cut:?} shows {hidden:?}: {visible}"
        );
    }
    // The open fence shows the code so far, as code.
    chat.messages[1].text = "Run:\n\n```sh\ncargo te".into();
    let html = messages(&chat, None, false).into_string();
    assert!(html.contains("cargo te"), "{html}");
    assert!(html.contains("<pre"), "{html}");
    assert!(!oa_copy::visible_text(&html).contains("```"), "{html}");

    chat.messages[1].text = full.into();
    chat.pending = None;
    chat.requests[0].outcome = Outcome::Answered;
    let html = messages(&chat, None, false).into_string();
    assert!(!html.contains("data-oa-streaming"), "{html}");
    assert!(
        html.contains(&crate::markdown::render_reply(full)),
        "{html}"
    );
}

// Sending twice, and refusals in the composer (the owner's double-submit
// report, 2026-10-09): a duplicate send goes to the same chat, and a
// refusal never replaces the page with bare text.

const THIRD: &str = "32345678-1234-4234-8234-123456789abc";

impl Fixture {
    /// A send as a browser makes it: `hx` adds `HX-Request` (and, when
    /// true, `HX-Boosted`: a boosted form); none is a plain form post.
    async fn send(
        &self,
        uri: &str,
        fields: &[(&str, &str)],
        hx: Option<bool>,
    ) -> (StatusCode, HeaderMap, String) {
        let mut builder = HttpRequest::builder()
            .method(Method::POST)
            .uri(uri)
            .header(header::HOST, HOST)
            .header(header::COOKIE, format!("oa_visitor={OWNER}"))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::ORIGIN, format!("http://{HOST}"))
            .header("Sec-Fetch-Site", "same-origin");
        if let Some(boosted) = hx {
            builder = builder.header("HX-Request", "true");
            if boosted {
                builder = builder.header("HX-Boosted", "true");
            }
        }
        let response = self
            .router
            .clone()
            .oneshot(builder.body(Body::from(form(fields))).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
            .await
            .unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        if headers
            .get(header::CONTENT_TYPE)
            .is_some_and(|kind| kind.as_bytes().starts_with(b"text/html"))
        {
            crate::copy_guard::assert_plain(uri, &body);
        }
        (status, headers, body)
    }

    /// The chat with an answer still being written for `request`.
    async fn answering(&self, request: &str) {
        let loaded = self.record(None).await;
        let mut next = loaded.conversation.clone();
        next.revision += 1;
        next.messages.push(Message {
            role: Role::User,
            text: "Still going?".into(),
            request_id: Some(request.into()),
        });
        next.messages.push(Message {
            role: Role::Assistant,
            text: String::new(),
            request_id: Some(request.into()),
        });
        next.pending = Some(Pending {
            request_id: request.into(),
            started_unix: now(),
            job_id: None,
        });
        next.requests.push(Request {
            id: request.into(),
            digest: digest("Still going?"),
            outcome: Outcome::Pending,
            selection: None,
            cloud: None,
            reply: None,
        });
        self.app
            .config
            .chat_store
            .compare_and_swap(&loaded, &next)
            .await
            .unwrap();
    }
}

fn location(headers: &HeaderMap) -> &str {
    headers[header::LOCATION].to_str().unwrap()
}

/// A refusal for an HTMX send: the refusal's status, marked, and only the
/// composer's status region (out of band), nothing that replaces a page.
fn assert_inline(status: StatusCode, headers: &HeaderMap, body: &str, text: &str) {
    assert_eq!(headers[REFUSED_HEADER], "1", "{body}");
    assert_eq!(headers["HX-Reswap"], "none");
    assert_eq!(headers["HX-Push-Url"], "false");
    assert!(status.is_client_error(), "{status}");
    assert!(
        body.starts_with("<div class=\"oa-composer-status\" id=\"chat-form-status\""),
        "{body}"
    );
    assert!(body.contains("hx-swap-oob=\"true\""), "{body}");
    assert!(body.contains(text), "{body}");
    assert!(!body.contains("<html") && !body.contains("<body"), "{body}");
}

/// A refusal for a plain form post: the whole page, with the reason in the
/// composer's status region and the text back in the box.
fn assert_page_with_notice(body: &str, text: &str, draft: &str) {
    assert!(body.starts_with("<!DOCTYPE html>"), "{body}");
    assert!(body.contains("id=\"chat-form\""), "{body}");
    let status = body.find("id=\"chat-form-status\"").unwrap();
    assert!(body[status..].contains(text), "{body}");
    let textarea = body.find("id=\"chat-input\"").unwrap();
    let end = textarea + body[textarea..].find("</textarea>").unwrap();
    assert!(
        body[textarea..end].contains(draft),
        "{}",
        &body[textarea..end]
    );
}

#[tokio::test]
async fn two_rapid_sends_of_a_new_chat_both_open_it() {
    // Two clicks at once, as a plain form (no script yet) and boosted.
    for hx in [None, Some(true)] {
        let fixture = Fixture::new();
        let csrf = csrf(&fixture.app, OWNER);
        let fields = [("q", TEXT), ("request_id", CHAT), ("csrf", csrf.as_str())];
        let (first, second) = tokio::join!(
            fixture.send("/chat", &fields, hx),
            fixture.send("/chat", &fields, hx)
        );
        for (status, headers, body) in [first, second] {
            assert_eq!(status, StatusCode::SEE_OTHER, "{hx:?}: {body}");
            assert_eq!(location(&headers), format!("/chat/{CHAT}"));
        }
        let saved = fixture.read().await;
        assert_eq!(saved.conversation.requests.len(), 1);
        let users = saved
            .conversation
            .messages
            .iter()
            .filter(|m| m.role == Role::User);
        assert_eq!(users.count(), 1);
        // One after the other: the second goes to the saved chat.
        let (status, headers, body) = fixture.send("/chat", &fields, None).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
        assert_eq!(location(&headers), format!("/chat/{CHAT}"));
        // A plain HTMX request (not boosted) is sent on with HX-Redirect.
        let (status, headers, _) = fixture.send("/chat", &fields, Some(false)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["HX-Redirect"], format!("/chat/{CHAT}"));
    }
}

#[tokio::test]
async fn a_second_new_chat_send_waits_for_the_first_to_save_it() {
    let fixture = Fixture::new();
    let store = fixture.app.config.chat_store.clone();
    // The first send holds the answer and has not saved the chat yet.
    assert!(store.claim(OWNER, CHAT, now() + 60).await.unwrap());
    let app = fixture.app.clone();
    let saver = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        app.config
            .chat_store
            .create(&Conversation {
                id: CHAT.into(),
                owner: OWNER.into(),
                revision: 1,
                title: TEXT.into(),
                messages: vec![Message {
                    role: Role::User,
                    text: TEXT.into(),
                    request_id: Some(CHAT.into()),
                }],
                pending: None,
                requests: vec![Request {
                    id: CHAT.into(),
                    digest: digest(TEXT),
                    outcome: Outcome::Answered,
                    selection: None,
                    cloud: None,
                    reply: None,
                }],
                selection: None,
                updated_unix: 1,
                pinned_unix: None,
                archived_unix: None,
                project: None,
                terminal: None,
                environment: None,
                tasks: Vec::new(),
                opened_unix: None,
                branch: None,
            })
            .await
            .unwrap();
    });
    let csrf = csrf(&fixture.app, OWNER);
    let (status, headers, body) = fixture
        .send(
            "/chat",
            &[("q", TEXT), ("request_id", CHAT), ("csrf", &csrf)],
            Some(true),
        )
        .await;
    saver.await.unwrap();
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    assert_eq!(location(&headers), format!("/chat/{CHAT}"));
    fixture.no_worker();
}

#[tokio::test]
async fn a_new_chat_while_another_answers_says_so_in_the_composer() {
    let fixture = Fixture::new();
    let store = fixture.app.config.chat_store.clone();
    assert!(store.claim(OWNER, THIRD, now() + 60).await.unwrap());
    let csrf = csrf(&fixture.app, OWNER);
    let fields = [
        ("q", "A new question"),
        ("request_id", NEXT),
        ("csrf", csrf.as_str()),
    ];
    let text = "OpenAgents is still answering your previous message.";
    let (status, headers, body) = fixture.send("/chat", &fields, Some(true)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_inline(status, &headers, &body, text);
    let (status, headers, body) = fixture.send("/chat", &fields, None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(!headers.contains_key(REFUSED_HEADER));
    assert_page_with_notice(&body, text, "A new question");
    // The new chat's page, boosted like any other.
    assert!(body.contains("hx-boost=\"true\""), "{body}");
    assert!(store.load(OWNER, NEXT).await.unwrap().is_none());
    fixture.no_worker();
}

#[tokio::test]
async fn a_send_while_the_chat_answers_keeps_the_page_and_the_draft() {
    let fixture = Fixture::new();
    fixture.answering(THIRD).await;
    let before = fixture.read().await;
    let csrf = csrf(&fixture.app, OWNER);
    let page = format!("/chat/{CHAT}");
    let fields = [
        ("q", "One more thing"),
        ("request_id", NEXT),
        ("csrf", csrf.as_str()),
    ];
    let text = "OpenAgents is still answering your previous message.";
    let (status, headers, body) = fixture.send(&page, &fields, Some(false)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_inline(status, &headers, &body, text);
    let (status, _, body) = fixture.send(&page, &fields, None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_page_with_notice(&body, text, "One more thing");
    // It is the chat's own page, still answering: the send button stays off
    // until the answer is written.
    assert!(body.contains("Existing conversation"), "{body}");
    assert!(
        body.contains("data-oa-composer-busy=\"chat-form\""),
        "{body}"
    );
    assert_eq!(fixture.read().await.generation, before.generation);
    fixture.no_worker();
}

#[tokio::test]
async fn two_rapid_sends_on_a_chat_take_the_message_once() {
    let fixture = Fixture::new();
    fixture.record(None).await;
    let csrf = csrf(&fixture.app, OWNER);
    let page = format!("/chat/{CHAT}");
    let fields = [
        ("q", "And then?"),
        ("request_id", NEXT),
        ("csrf", csrf.as_str()),
    ];
    let (first, second) = tokio::join!(
        fixture.send(&page, &fields, Some(false)),
        fixture.send(&page, &fields, Some(false))
    );
    for (status, headers, body) in [first, second] {
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(!headers.contains_key(REFUSED_HEADER), "{body}");
        assert!(body.contains("id=\"chat-ticket\""), "{body}");
    }
    let saved = fixture.read().await;
    let sent = |r: &&Request| r.id == NEXT;
    assert_eq!(saved.conversation.requests.iter().filter(sent).count(), 1);
    assert_eq!(
        saved
            .conversation
            .messages
            .iter()
            .filter(|m| m.text == "And then?")
            .count(),
        1
    );
}

#[tokio::test]
async fn the_chat_pages_are_boosted_and_send_once() {
    let fixture = Fixture::new();
    fixture.record(None).await;
    for uri in ["/".to_owned(), format!("/chat/{CHAT}")] {
        let (status, body) = fixture.request(Method::GET, &uri, OWNER, &[]).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert!(
            body.contains("<body class=\"oa-body\" hx-boost=\"true\">"),
            "{uri}"
        );
        assert!(body.contains("hx-sync=\"this:drop\""), "{uri}");
        assert!(
            body.contains("hx-disabled-elt=\"#chat-form button[type=submit]\""),
            "{uri}"
        );
        assert!(body.contains("/static/chat-start.js"), "{uri}");
        assert!(body.contains("refreshOnHistoryMiss&quot;:false"), "{uri}");
    }
    // A page with its own head loads in full.
    let (_, body) = fixture.request(Method::GET, "/docs", OWNER, &[]).await;
    assert!(!body.contains("hx-boost"), "{body}");
    // The browser code keeps boosting to the chat pages only, and binds the
    // new composer after a boosted swap.
    let script = include_str!("../../static/chat-start.js");
    assert!(script.contains("htmx:confirm"));
    assert!(script.contains("htmx:afterSettle"));
}
