//! Sharing a chat by link (`crate::pages::chat::share`): the owner shares
//! and stops; anyone signed out reads the share; another account can't
//! share someone else's chat; files and only-you projects never leak.

use super::*;
use crate::chat_files::{FileRef, Kind};
use crate::chat_store::{Message, Outcome, Role};
use serde_json::Value;

const SHARED_CHAT: &str = "42345678-1234-4234-8234-123456789abc";
const FILE_NAME: &str = "tax-return-2025.pdf";

/// A chat with a question (sent with a file), a Markdown answer, and a
/// step, kept for `owner`.
fn sample(id: &str, owner: &str, project: Option<&str>) -> Conversation {
    let mut chat = chat(id, owner, "Plan the launch email", project);
    chat.messages = vec![
        Message {
            role: Role::User,
            text: "Draft the launch email for Friday.".into(),
            request_id: Some(id.into()),
        },
        Message {
            role: Role::Tool,
            text: "Read: launch-notes.md".into(),
            request_id: Some(id.into()),
        },
        Message {
            role: Role::Assistant,
            text: "Here is a draft:\n\n## Subject\n\n**Coder is here.**\n\n```text\nopenagents login\n```".into(),
            request_id: Some(id.into()),
        },
    ];
    chat.requests = vec![crate::chat_store::Request {
        id: id.into(),
        digest: "d".repeat(64),
        outcome: Outcome::Answered,
        selection: None,
        cloud: None,
        reply: None,
        files: vec![FileRef {
            id: "0123456789abcdef0123456789abcdef".into(),
            name: FILE_NAME.into(),
            kind: Kind::Pdf,
            size: 2048,
        }],
    }];
    chat
}

/// The token of the share link on a share step's page.
fn token_in(html: &str) -> String {
    let at = html
        .find("/share/")
        .unwrap_or_else(|| panic!("no link: {html}"));
    html[at + 7..at + 39].to_owned()
}

async fn api(
    world: &World,
    browser: &Browser,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let token = browser
        .0
        .get("oa_cloud_session")
        .cloned()
        .unwrap_or_default();
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, world.host.as_str())
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    if body.is_null() {
        request = request.header(header::CONTENT_LENGTH, "0");
    }
    let response = world
        .site
        .clone()
        .oneshot(
            request
                .body(if body.is_null() {
                    Body::empty()
                } else {
                    Body::from(body.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn a_chat_shares_by_link_reads_signed_out_and_stops() {
    let world = world().await;
    assert!(crate::upstream::owned("/share/0123"));
    let mut octo = Browser::default();
    octo.sign_in(&world, "octo-local").await;
    let owner = account_owner_of(&world, "octo-local");
    world
        .store
        .create(&sample(SHARED_CHAT, &owner, None))
        .await
        .unwrap();

    // The chat's header offers Share, which opens the step over the chat.
    let page = octo.get(&world, &format!("/chat/{SHARED_CHAT}")).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    let step = format!("/chat/{SHARED_CHAT}/share");
    assert!(
        page.body.contains(&format!(r#"href="{step}""#)),
        "{}",
        page.body
    );
    assert!(
        page.body
            .contains(&format!(r#"hx-get="{step}?part=dialog""#))
    );
    assert!(page.body.contains(">Share<"));
    let csrf = chat_csrf(&page.body);

    let dialog = octo.get(&world, &format!("{step}?part=dialog")).await;
    assert_eq!(dialog.status, StatusCode::OK);
    assert!(dialog.body.contains("<dialog") && dialog.body.contains("data-oa-open"));
    assert!(
        dialog
            .body
            .contains("Anyone with the link can read it. You can stop sharing any time.")
    );
    assert!(dialog.body.contains(">Cancel<") && dialog.body.contains(">Share<"));
    crate::copy_guard::assert_plain(&step, &dialog.body);

    // Share: the step shows the link and Copy link.
    let shared = octo
        .post(&world, &step, &[("csrf", &csrf), ("shared", "true")])
        .await;
    assert_eq!(shared.status, StatusCode::SEE_OTHER, "{}", shared.body);
    let step_page = octo.get(&world, &step).await;
    assert_eq!(step_page.status, StatusCode::OK, "{}", step_page.body);
    assert!(step_page.body.contains("Copy link") && step_page.body.contains("Stop sharing"));
    crate::copy_guard::assert_plain(&step, &step_page.body);
    let token = token_in(&step_page.body);
    assert!(token.bytes().all(|b| b.is_ascii_hexdigit()), "{token}");
    assert!(!token.contains(SHARED_CHAT));
    let link = format!("/share/{token}");
    // Sharing again keeps the same link.
    let again = octo
        .post(&world, &step, &[("csrf", &csrf), ("shared", "true")])
        .await;
    assert_eq!(again.status, StatusCode::SEE_OTHER);
    assert_eq!(token_in(&octo.get(&world, &step).await.body), token);
    // The header now reads Shared.
    let page = octo.get(&world, &format!("/chat/{SHARED_CHAT}")).await;
    assert!(page.body.contains(">Shared<"), "{}", page.body);

    // Anyone signed out reads it: title and messages, nothing else.
    let mut visitor = Browser::default();
    let public = visitor.get(&world, &link).await;
    assert_eq!(public.status, StatusCode::OK, "{}", public.body);
    let body = &public.body;
    assert!(body.contains("Plan the launch email"));
    assert!(body.contains("Draft the launch email for Friday."));
    assert!(body.contains("<strong>Coder is here.</strong>"));
    assert!(body.contains("Read: launch-notes.md"));
    assert!(body.contains(r#"href="/""#) && body.contains("OpenAgents"));
    for absent in [
        FILE_NAME,
        "/files/",
        "chat-form",
        "<textarea",
        "oa-sidebar",
        "sse-connect",
        "csrf",
        "<form",
        SHARED_CHAT,
        "Run in Coder",
        "to-coder",
    ] {
        assert!(!body.contains(absent), "{absent}: {body}");
    }
    assert_eq!(public.headers["x-robots-tag"], "noindex, nofollow");
    assert_eq!(public.headers[header::REFERRER_POLICY], "no-referrer");
    assert!(body.contains(r#"<meta name="robots" content="noindex, nofollow">"#));
    crate::copy_guard::assert_plain("/share/x", body);

    // Social previews: Open Graph and Twitter tags, and the picture.
    let meta = |key: &str| -> String {
        let at = body
            .find(&format!(r#""{key}" content=""#))
            .unwrap_or_else(|| panic!("{key}: {body}"));
        let rest = &body[at + key.len() + 12..];
        rest[..rest.find('"').unwrap()].to_owned()
    };
    let origin = world.origin();
    assert_eq!(meta("og:title"), "Plan the launch email");
    assert_eq!(meta("og:type"), "article");
    assert_eq!(meta("og:site_name"), "OpenAgents");
    assert_eq!(meta("og:url"), format!("{origin}{link}"));
    assert_eq!(
        meta("og:description"),
        "Here is a draft: Subject. Coder is here."
    );
    assert_eq!(meta("og:image:width"), "1200");
    assert_eq!(meta("og:image:height"), "630");
    assert!(meta("og:image:alt").starts_with("Plan the launch email"));
    assert_eq!(meta("twitter:card"), "summary_large_image");
    assert_eq!(meta("twitter:title"), "Plan the launch email");
    assert_eq!(meta("twitter:description"), meta("og:description"));
    let image = meta("og:image");
    assert_eq!(meta("twitter:image"), image);
    assert!(!meta("twitter:image:alt").is_empty());
    let image_path = image.strip_prefix(&origin).unwrap().to_owned();
    assert!(image_path.starts_with(&format!("{link}/card/")) && image_path.ends_with(".png"));
    let (status, png) = visitor
        .send_bytes(&world, Request::get(&image_path), Body::empty())
        .await;
    assert_eq!(status, StatusCode::OK);
    let picture = image::load_from_memory(&png).unwrap();
    assert_eq!((picture.width(), picture.height()), (1200, 630));
    let card = world
        .site
        .clone()
        .oneshot(
            Request::get(&image_path)
                .header(header::HOST, world.host.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(card.headers()[header::CONTENT_TYPE], "image/png");
    assert!(
        card.headers()[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("immutable")
    );
    // Another digest is not this share's picture.
    let (status, _) = visitor
        .send_bytes(
            &world,
            Request::get(format!("{link}/card/0000000000000000.png")),
            Body::empty(),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    if let Ok(dir) = std::env::var("OA_CARD_OUT") {
        std::fs::write(format!("{dir}/card-from-share.png"), &png).unwrap();
    }

    // A file sent with the chat is never served through the share.
    let file = visitor
        .get(
            &world,
            &format!("/chat/{SHARED_CHAT}/files/0123456789abcdef0123456789abcdef"),
        )
        .await;
    assert_ne!(file.status, StatusCode::OK);

    // The apps read and change the same share.
    let (status, state) = api(
        &world,
        &octo,
        "GET",
        &format!("/v1/threads/{SHARED_CHAT}/share"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{state}");
    assert_eq!(state["shared"], true);
    assert!(state["url"].as_str().unwrap().ends_with(&link), "{state}");

    // Another account can't share, stop, or even see the step.
    let mut quiet = Browser::default();
    quiet.sign_in(&world, "quiet-local").await;
    let theirs = quiet.get(&world, "/").await;
    let quiet_csrf = chat_csrf(&theirs.body);
    let stopped = quiet
        .post(&world, &step, &[("csrf", &quiet_csrf), ("shared", "false")])
        .await;
    assert_eq!(stopped.status, StatusCode::NOT_FOUND, "{}", stopped.body);
    let started = quiet
        .post(&world, &step, &[("csrf", &quiet_csrf), ("shared", "true")])
        .await;
    assert_eq!(started.status, StatusCode::NOT_FOUND);
    assert_eq!(quiet.get(&world, &step).await.status, StatusCode::NOT_FOUND);
    for shared in [true, false] {
        let (status, body) = api(
            &world,
            &quiet,
            "POST",
            &format!("/v1/threads/{SHARED_CHAT}/share"),
            json!({"shared": shared}),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    }
    // The owner's form, sent with another's ticket, is refused too.
    let forged = octo
        .post(&world, &step, &[("csrf", &quiet_csrf), ("shared", "false")])
        .await;
    assert_eq!(forged.status, StatusCode::FORBIDDEN);
    assert_eq!(visitor.get(&world, &link).await.status, StatusCode::OK);

    // Stop sharing: the link stops working.
    let stop = octo
        .post(&world, &step, &[("csrf", &csrf), ("shared", "false")])
        .await;
    assert_eq!(stop.status, StatusCode::SEE_OTHER);
    let gone = visitor.get(&world, &link).await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND, "{}", gone.body);
    let (status, _) = visitor
        .send_bytes(&world, Request::get(&image_path), Body::empty())
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a stopped share's picture is gone"
    );
    assert!(!gone.body.contains("Draft the launch email"));
    let page = octo.get(&world, &format!("/chat/{SHARED_CHAT}")).await;
    assert!(page.body.contains(">Share<") && !page.body.contains(">Shared<"));

    // Sharing again makes a new link; the old one stays dead.
    let (status, state) = api(
        &world,
        &octo,
        "POST",
        &format!("/v1/threads/{SHARED_CHAT}/share"),
        json!({"shared": true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{state}");
    let url = state["url"].as_str().unwrap().to_owned();
    assert!(!url.ends_with(&link));
    let fresh = &url[url.find("/share/").unwrap()..];
    assert_eq!(visitor.get(&world, fresh).await.status, StatusCode::OK);
    assert_eq!(
        visitor.get(&world, &link).await.status,
        StatusCode::NOT_FOUND
    );

    // Deleting the chat ends its link.
    let deleted = octo
        .post(
            &world,
            &format!("/chat/{SHARED_CHAT}/delete"),
            &[("csrf", &csrf)],
        )
        .await;
    assert_eq!(deleted.status, StatusCode::SEE_OTHER, "{}", deleted.body);
    assert_eq!(
        visitor.get(&world, fresh).await.status,
        StatusCode::NOT_FOUND
    );
    // Nonsense links are plain 404s.
    for path in ["/share/nope", "/share/0123456789abcdef0123456789abcdef"] {
        assert_eq!(
            visitor.get(&world, path).await.status,
            StatusCode::NOT_FOUND
        );
    }
}

#[tokio::test]
async fn the_dialog_swaps_in_place_and_updates_the_header_button() {
    let world = world().await;
    let mut octo = Browser::default();
    octo.sign_in(&world, "octo-local").await;
    let owner = account_owner_of(&world, "octo-local");
    world
        .store
        .create(&sample(SHARED_CHAT, &owner, None))
        .await
        .unwrap();
    let page = octo.get(&world, &format!("/chat/{SHARED_CHAT}")).await;
    let csrf = chat_csrf(&page.body);
    let step = format!("/chat/{SHARED_CHAT}/share");
    let body: String = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([("csrf", csrf.as_str()), ("shared", "true")])
        .finish();
    let swapped = octo
        .send(
            &world,
            Request::post(&step)
                .header(header::ORIGIN, world.origin())
                .header("sec-fetch-site", "same-origin")
                .header("HX-Request", "true")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded"),
            Body::from(body),
        )
        .await;
    assert_eq!(swapped.status, StatusCode::OK, "{}", swapped.body);
    assert!(swapped.body.contains("Copy link") && swapped.body.contains("/share/"));
    assert!(
        swapped
            .body
            .contains(r#"id="chat-share" hx-swap-oob="outerHTML""#)
    );
    assert!(swapped.body.contains(">Shared<"));
    assert!(!swapped.body.contains("<dialog"));
}

#[tokio::test]
async fn signed_out_chats_and_only_you_projects_are_never_shared() {
    let world = world().await;
    // A chat kept by a signed-out browser has no Share button.
    let mut visitor = Browser::default();
    let home = visitor.get(&world, "/new").await;
    let browser_owner = visitor
        .0
        .get("oa_visitor")
        .cloned()
        .expect("visitor cookie");
    let _ = home;
    world
        .store
        .create(&sample(SHARED_CHAT, &browser_owner, None))
        .await
        .unwrap();
    let page = visitor.get(&world, &format!("/chat/{SHARED_CHAT}")).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(!page.body.contains("/share"), "{}", page.body);
    let csrf = chat_csrf(&page.body);
    let tried = visitor
        .post(
            &world,
            &format!("/chat/{SHARED_CHAT}/share"),
            &[("csrf", &csrf), ("shared", "true")],
        )
        .await;
    assert_eq!(tried.status, StatusCode::SEE_OTHER);
    let step = visitor
        .get(&world, &format!("/chat/{SHARED_CHAT}/share"))
        .await;
    assert!(
        step.body.contains("Log in to share a chat."),
        "{}",
        step.body
    );
    assert!(!step.body.contains("/share/"));

    // A chat in a project set to only you can't be shared, and a share
    // stops working when its project is set to only you later.
    let mut octo = Browser::default();
    octo.sign_in(&world, "octo-local").await;
    connect_public(&mut octo, &world, "octo-local").await;
    let owner = account_owner_of(&world, "octo-local");
    let (status, project) = api(
        &world,
        &octo,
        "POST",
        "/v1/projects",
        json!({"repository": "octo-local/hello-world"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{project}");
    let project = project["id"].as_str().unwrap().to_owned();
    const PROJECT_CHAT: &str = "52345678-1234-4234-8234-123456789abc";
    world
        .store
        .create(&sample(PROJECT_CHAT, &owner, Some(&project)))
        .await
        .unwrap();
    let share = format!("/v1/threads/{PROJECT_CHAT}/share");
    let (status, state) = api(&world, &octo, "POST", &share, json!({"shared": true})).await;
    assert_eq!(status, StatusCode::OK, "{state}");
    let url = state["url"].as_str().unwrap().to_owned();
    let link = url[url.find("/share/").unwrap()..].to_owned();
    assert_eq!(visitor.get(&world, &link).await.status, StatusCode::OK);
    let (status, body) = api(
        &world,
        &octo,
        "PATCH",
        &format!("/v1/projects/{project}"),
        json!({"protection": "only_me"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        visitor.get(&world, &link).await.status,
        StatusCode::NOT_FOUND
    );
    let (status, body) = api(&world, &octo, "POST", &share, json!({"shared": true})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "not_shareable");
    let step = octo
        .get(&world, &format!("/chat/{PROJECT_CHAT}/share?part=dialog"))
        .await;
    assert!(
        step.body
            .contains("This chat is in a project set to only you, so it can't be shared."),
        "{}",
        step.body
    );
    crate::copy_guard::assert_plain("/chat/x/share", &step.body);
}

/// Serves a signed-in world on `OA_SCREEN_PORT` for headless screenshots
/// of sharing (`cargo test ... serve_share_for_screenshots -- --ignored`):
/// one chat with a longer answer, not shared yet, and a second one shared.
/// The owner's cookies, the chat ids, and the share link go to
/// `OA_SCREEN_OUT`.
#[tokio::test]
#[ignore = "serves a local site for screenshots"]
async fn serve_share_for_screenshots() {
    let port: u16 = std::env::var("OA_SCREEN_PORT").unwrap().parse().unwrap();
    let out = std::path::PathBuf::from(std::env::var("OA_SCREEN_OUT").unwrap());
    HOST_OVERRIDE.set(format!("127.0.0.1:{port}")).unwrap();
    let world = world_on(port).await;
    let mut octo = Browser::default();
    octo.sign_in(&world, "octo-local").await;
    let owner = account_owner_of(&world, "octo-local");
    let mut chat = sample(SHARED_CHAT, &owner, None);
    chat.title = "Why does my Rust build take so long?".into();
    chat.messages = vec![
        Message {
            role: Role::User,
            text: "Why does my Rust build take so long after every small change?".into(),
            request_id: Some(SHARED_CHAT.into()),
        },
        Message {
            role: Role::Tool,
            text: "Read: Cargo.toml".into(),
            request_id: Some(SHARED_CHAT.into()),
        },
        Message {
            role: Role::Assistant,
            text: "Most of the time goes to **linking** and to crates that rebuild because a feature changed.\n\n## What to try\n\n1. Keep one target folder per checkout so nothing rebuilds from scratch.\n2. Split the big crate: a change then rebuilds only its part.\n3. Use a faster linker.\n\n```toml\n[profile.dev]\ndebug = \"line-tables-only\"\n```\n\nWith these, a one-line change usually builds in seconds.".into(),
            request_id: Some(SHARED_CHAT.into()),
        },
        Message {
            role: Role::User,
            text: "Which linker should I use on a Mac?".into(),
            request_id: Some(SHARED_CHAT.into()),
        },
        Message {
            role: Role::Assistant,
            text: "On a Mac the system linker is already fast. Leave it as it is and keep the target folder.".into(),
            request_id: Some(SHARED_CHAT.into()),
        },
    ];
    world.store.create(&chat).await.unwrap();
    const SECOND: &str = "62345678-1234-4234-8234-123456789abc";
    let mut second = sample(SECOND, &owner, None);
    second.updated_unix = 2;
    world.store.create(&second).await.unwrap();
    let (status, state) = api(
        &world,
        &octo,
        "POST",
        &format!("/v1/threads/{SECOND}/share"),
        json!({"shared": true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{state}");
    let url = state["url"].as_str().unwrap().to_owned();
    let cookies = octo
        .0
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("cookies-octo.txt"), cookies).unwrap();
    std::fs::write(out.join("chat.txt"), SHARED_CHAT).unwrap();
    std::fs::write(out.join("shared-chat.txt"), SECOND).unwrap();
    std::fs::write(out.join("share-url.txt"), url).unwrap();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    std::fs::write(out.join("ready"), "ok").unwrap();
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(1800),
        axum::serve(listener, world.site.clone()),
    )
    .await;
}
