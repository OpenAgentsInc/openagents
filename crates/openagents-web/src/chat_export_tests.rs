use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::Request;
use tower::ServiceExt;

use super::*;

const CHAT: &str = "83e18906-00e2-436c-978b-13a4932f58b0";
const AT: u64 = 1_791_982_937_357;

/// A Video Lab chat: a question, a Lab answer with its card, a progress
/// note, and a plain answer.
fn lab_chat() -> Conversation {
    serde_json::from_value(json!({
        "id": CHAT, "owner": "account:test", "revision": 3, "title": "Episode 290 cut",
        "messages": [
            {"role": "user", "text": "Show me version 3", "request_id": "r1"},
            {"role": "assistant", "text": "Here is version 3.", "request_id": "r1"},
            {"role": "tool", "text": "Run: render preview"},
            {"role": "user", "text": "Thanks", "request_id": "r2"},
            {"role": "assistant", "text": "You're welcome.", "request_id": "r2"}
        ],
        "pending": null,
        "requests": [
            {"id": "r1", "digest": "d1", "outcome": "answered", "reply": {
                "route": "video_lab",
                "cards": [{
                    "card": "video_lab", "lab": "http://127.0.0.1:8792", "doc": "episode-290",
                    "title": "Episode 290", "kind": "cut.v1", "version": 3, "quality": "preview",
                    "versions": [{"n": 1}, {"n": 2}, {"n": 3}],
                    "video": "/media/films/episode-290/renders/abc-preview.mp4",
                    "player": "http://127.0.0.1:8792/player?doc=episode-290&v=3"
                }]
            }},
            {"id": "r2", "digest": "d2", "outcome": "answered"}
        ],
        "updated_unix": 1_791_982_900
    }))
    .unwrap()
}

fn keys(value: &Value) -> BTreeSet<String> {
    value
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

/// The export has the shape Coder's terminal `/export` writes: the same
/// fields at the top, on the agent, in the metrics, and on each kind of
/// step, and it reads back as a valid ATIF document.
#[test]
fn the_export_has_coders_shape() {
    let coder: Value =
        serde_json::from_str(include_str!("../tests/fixtures/coder-export.atif.json")).unwrap();
    let web = document(&lab_chat(), AT);
    assert!(atif::validate(&coder).is_empty());
    assert_eq!(atif::validate(&web), Vec::<String>::new());
    assert_eq!(web["schema_version"], coder["schema_version"]);
    assert_eq!(web["extra"]["exporter"], coder["extra"]["exporter"]);
    assert_eq!(web["agent"]["name"], coder["agent"]["name"]);
    assert_eq!(keys(&web), keys(&coder));
    assert_eq!(keys(&web["agent"]), keys(&coder["agent"]));
    assert_eq!(keys(&web["final_metrics"]), keys(&coder["final_metrics"]));
    let coder_metrics = keys(&coder["final_metrics"]["extra"]);
    let web_metrics = keys(&web["final_metrics"]["extra"]);
    // Coder adds its own model accounting; the rest is the same.
    for key in &web_metrics {
        assert!(
            coder_metrics.contains(key),
            "{key} is not in Coder's export"
        );
    }
    assert!(keys(&coder["extra"]).is_subset(&keys(&web["extra"])));
    let steps = web["steps"].as_array().unwrap();
    let coder_steps = coder["steps"].as_array().unwrap();
    // The user's step carries exactly Coder's fields, and an answer
    // Coder's fields but the token counts and timing the web doesn't keep.
    assert_eq!(keys(&steps[3]), keys(&coder_steps[0]));
    let mut answer = keys(&coder_steps[1]);
    answer.remove("metrics");
    answer.remove("extra");
    assert_eq!(keys(&steps[4]), answer);
    let mut lab_answer = answer.clone();
    lab_answer.insert("extra".into());
    assert_eq!(keys(&steps[1]), lab_answer);
    assert_eq!(web["extra"]["timestamps"], "export-time");
    assert_eq!(web["extra"]["directive"], "Show me version 3");
    assert_eq!(web["agent"]["extra"]["door"], DOOR);
    assert_eq!(web["final_metrics"]["total_steps"], 5);
}

#[test]
fn a_video_lab_chat_keeps_its_document_and_versions() {
    let web = document(&lab_chat(), AT);
    let reference = json!({
        "doc": "episode-290", "title": "Episode 290", "kind": "cut.v1", "version": 3,
        "quality": "preview", "player": "http://127.0.0.1:8792/player?doc=episode-290&v=3",
        "versions": [1, 2, 3]
    });
    assert_eq!(web["steps"][1]["extra"]["video_lab"], reference);
    assert_eq!(web["steps"][1]["extra"]["route"], "video_lab");
    assert_eq!(web["extra"]["video_lab"], json!([reference]));
    assert_eq!(web["steps"][2]["source"], "system");
    assert!(web["steps"][4]["extra"].get("video_lab").is_none());
}

#[test]
fn the_file_name_is_made_here_and_never_replaces_a_file() {
    assert_eq!(
        file_name(CHAT, AT),
        "openagents-chat-20261014T130217Z-83e18906.atif.json"
    );
    // Only hex from the id reaches the name.
    assert_eq!(
        file_name("../../x/83e1", AT),
        "openagents-chat-20261014T130217Z-83e1.atif.json"
    );
    let dir = tempfile::tempdir().unwrap();
    let first = save(dir.path(), CHAT, AT, b"one").unwrap();
    let second = save(dir.path(), CHAT, AT, b"two").unwrap();
    assert_eq!(first.parent(), Some(dir.path()));
    assert_eq!(second.parent(), Some(dir.path()));
    assert_eq!(
        second.file_name().unwrap(),
        "openagents-chat-20261014T130217Z-83e18906-2.atif.json"
    );
    assert_eq!(std::fs::read(&first).unwrap(), b"one");
    assert_eq!(std::fs::read(&second).unwrap(), b"two");
}

struct OfflineChat;
impl crate::ask::Chat for OfflineChat {
    fn door(
        &self,
        _: secp256k1::SecretKey,
    ) -> Result<Box<dyn openagents_chat::basic_coder::Door>, String> {
        Err("The synthetic test chat has no network connection.".into())
    }
}

async fn call(site: &Router, request: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = site.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 1 << 24).await.unwrap();
    (status, headers, body.to_vec())
}

fn field<'a>(html: &'a str, name: &str) -> &'a str {
    html.split_once(&format!("name=\"{name}\""))
        .unwrap()
        .1
        .split('>')
        .next()
        .unwrap()
        .split_once("value=\"")
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap()
}

/// A site on `host` with a chat in it: the chat's path, the visitor's
/// cookie, and the form's token.
async fn with_chat(site: &Router, host: &str) -> (String, String, String) {
    let get = |uri: &str, cookie: Option<&str>| {
        let mut request = Request::builder().uri(uri).header(header::HOST, host);
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        request.body(Body::empty()).unwrap()
    };
    let (_, headers, home) = call(site, get("/", None)).await;
    let home = String::from_utf8(home).unwrap();
    let cookie = headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let input = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("q", "Save me"),
            ("request_id", field(&home, "request_id")),
            ("csrf", field(&home, "csrf")),
            ("selection", field(&home, "selection")),
        ])
        .finish();
    let (status, headers, _) = call(
        site,
        Request::builder()
            .method("POST")
            .uri("/chat")
            .header(header::HOST, host)
            .header(header::COOKIE, &cookie)
            .header(header::ORIGIN, format!("http://{host}"))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(input))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let chat = headers[header::LOCATION].to_str().unwrap().to_owned();
    let (_, _, page) = call(site, get(&chat, Some(&cookie))).await;
    let page = String::from_utf8(page).unwrap();
    assert!(
        page.contains("data-oa-export-hint"),
        "the composer offers /export"
    );
    assert!(page.contains(SCRIPT_PATH));
    let csrf = field(&page, "csrf").to_owned();
    (chat, cookie, csrf)
}

async fn post_export(
    site: &Router,
    host: &str,
    chat: &str,
    cookie: &str,
    csrf: &str,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    call(
        site,
        Request::builder()
            .method("POST")
            .uri(format!("{chat}/export"))
            .header(header::HOST, host)
            .header(header::COOKIE, cookie)
            .header(header::ORIGIN, format!("http://{host}"))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!("csrf={csrf}")))
            .unwrap(),
    )
    .await
}

fn site(root: &Path, downloads: Option<PathBuf>) -> Router {
    let mut config = crate::Config::development(root.join("tasks"));
    config.chat = Arc::new(OfflineChat);
    config.public_hosts.push("openagents.com".into());
    config.downloads = downloads;
    crate::router(config)
}

/// On the local stack the server writes the file into Downloads and
/// answers with its path.
#[tokio::test]
async fn the_local_site_saves_into_downloads() {
    let root = tempfile::tempdir().unwrap();
    let downloads = root.path().join("Downloads");
    std::fs::create_dir(&downloads).unwrap();
    let site = site(root.path(), Some(downloads.clone()));
    let host = "127.0.0.1:4300";
    let (chat, cookie, csrf) = with_chat(&site, host).await;
    let (status, headers, body) = post_export(&site, host, &chat, &cookie, &csrf).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[SAVED_HEADER], "saved");
    let answer: Value = serde_json::from_slice(&body).unwrap();
    let path = PathBuf::from(answer["path"].as_str().unwrap());
    assert_eq!(path.parent(), Some(downloads.as_path()));
    let name = answer["name"].as_str().unwrap();
    assert!(name.starts_with("openagents-chat-") && name.ends_with(".atif.json"));
    let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(atif::validate(&saved).is_empty());
    assert_eq!(saved["steps"][0]["message"], "Save me");
    assert_eq!(std::fs::read_dir(&downloads).unwrap().count(), 1);
    // A wrong token saves nothing.
    let (status, _, _) = post_export(&site, host, &chat, &cookie, "wrong").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(std::fs::read_dir(&downloads).unwrap().count(), 1);
}

/// The public host never writes on the server, even when this server has
/// a Downloads folder: the browser gets the document to save itself.
#[tokio::test]
async fn the_public_host_never_writes_a_file() {
    let root = tempfile::tempdir().unwrap();
    let downloads = root.path().join("Downloads");
    std::fs::create_dir(&downloads).unwrap();
    let site = site(root.path(), Some(downloads.clone()));
    let host = "openagents.com";
    let (chat, cookie, csrf) = with_chat(&site, host).await;
    let (status, headers, body) = post_export(&site, host, &chat, &cookie, &csrf).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers.get(SAVED_HEADER).is_none());
    let disposition = headers[header::CONTENT_DISPOSITION].to_str().unwrap();
    assert!(
        disposition.starts_with("attachment; filename=\"openagents-chat-"),
        "{disposition}"
    );
    let document: Value = serde_json::from_slice(&body).unwrap();
    assert!(atif::validate(&document).is_empty());
    assert_eq!(std::fs::read_dir(&downloads).unwrap().count(), 0);
    // The plain link downloads the same document.
    let (status, headers, _) = call(
        &site,
        Request::builder()
            .uri(format!("{chat}/export"))
            .header(header::HOST, host)
            .header(header::COOKIE, &cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers.contains_key(header::CONTENT_DISPOSITION));
}

/// Without `--local-downloads` even the local address gets the download.
#[tokio::test]
async fn without_a_downloads_folder_nothing_is_written() {
    let root = tempfile::tempdir().unwrap();
    let site = site(root.path(), None);
    let host = "127.0.0.1:4300";
    let (chat, cookie, csrf) = with_chat(&site, host).await;
    let (status, headers, _) = post_export(&site, host, &chat, &cookie, &csrf).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers.get(SAVED_HEADER).is_none());
    assert!(headers.contains_key(header::CONTENT_DISPOSITION));
}

#[test]
fn the_script_runs_the_command_before_the_send() {
    let script = include_str!("../static/chat-export.js");
    assert!(script.contains("stopImmediatePropagation"));
    assert!(script.contains("ClipboardItem"));
    assert!(script.contains("assumes your Downloads folder"));
}
