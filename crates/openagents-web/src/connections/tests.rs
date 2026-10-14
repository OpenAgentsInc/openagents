//! Connections through the site with a fake Google: connecting Google from
//! Settings, attaching a Drive folder to a project, and the chat's Drive
//! door answering from it with a fake Gemini, citing what it read.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State as AxumState;
use axum::http::StatusCode;
use inference::upstream::google::TokenSource;
use inference::upstream::secret::Secret;
use inference::upstream::vertex;
use oa_connections::fake::{self, Content, FakeFile, FakeGoogle};
use oa_connections::google::drive::{Drive, Readers};
use oa_connections::google::oauth::Client;
use oa_connections::google::{DRIVE_READONLY, SHEETS_READONLY};
use openagents_chat::basic_coder::Turn;
use serde_json::{Value, json};

use super::chat::{self, Ending, FAST};
use super::{Google, Live};
use crate::chat_vision::{GEMINI, Gemini};
use crate::cloud::connections::AccessEntry;
use crate::cloud::connections::{Source, Store, Stored};
use crate::projects::tests::{
    Browser, Options, World, account_id_of, account_owner_of, hidden, world_with,
};

const FOLDER: &str = "folder-finances-0001";
const SHEET: &str = "sheet-spending-00001";

fn files() -> Vec<FakeFile> {
    vec![
        FakeFile::new(FOLDER, "Finances", Some("root"), Content::Folder),
        FakeFile::new(
            SHEET,
            "Spending October",
            Some(FOLDER),
            Content::Sheet(vec![(
                "October".into(),
                vec![
                    vec!["Item".into(), "Amount".into()],
                    vec!["Groceries".into(), "84.10".into()],
                    vec!["Rent".into(), "1200.00".into()],
                ],
            )]),
        ),
    ]
}

fn store(dir: &tempfile::TempDir) -> Arc<Store> {
    Arc::new(
        Store::open(
            &dir.path().canonicalize().unwrap().join("connections"),
            oa_seal::Keyring::scratch("test").unwrap().0,
        )
        .unwrap(),
    )
}

#[tokio::test]
async fn connecting_google_and_attaching_a_drive_folder_to_a_project() {
    let google = FakeGoogle::start(files(), &[DRIVE_READONLY, SHEETS_READONLY]).await;
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let world = world_with(Options {
        connections: Some((
            store.clone(),
            Arc::new(Google::new(
                Client::new("cid.apps".into(), "client-secret".into()),
                google.endpoints(),
            )),
        )),
        ..Options::default()
    })
    .await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    store
        .update_access(|a| {
            a.add(AccessEntry {
                account: Some(account_id_of(&world, "octo-local")),
                ..AccessEntry::default()
            })
        })
        .unwrap()
        .unwrap();

    // Settings offers Connect, in plain words.
    let page = browser.get(&world, super::PAGE).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("Not connected"), "{}", page.body);
    assert!(page.body.contains(super::CONNECT));
    crate::copy_guard::assert_plain(super::PAGE, &page.body);
    let settings = browser.get(&world, crate::settings::PAGE).await;
    assert!(settings.body.contains(super::PAGE), "{}", settings.body);

    // Connect goes to Google with Drive and Sheets read-only, offline,
    // incremental, PKCE.
    let start = browser
        .get(
            &world,
            &format!("{}?return_to=/settings/connections", super::CONNECT),
        )
        .await;
    assert_eq!(start.status, StatusCode::SEE_OTHER, "{}", start.body);
    let authorize = url::Url::parse(start.location()).unwrap();
    assert!(start.location().starts_with(&google.root));
    let param = |k: &str| {
        authorize
            .query_pairs()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.into_owned())
            .unwrap()
    };
    assert!(param("scope").contains(DRIVE_READONLY));
    assert!(param("scope").contains(SHEETS_READONLY));
    assert_eq!(param("include_granted_scopes"), "true");
    assert_eq!(param("code_challenge_method"), "S256");
    assert!(param("redirect_uri").ends_with("/auth/google/callback"));
    assert!(!start.location().contains("client-secret"));
    let state = param("state");

    // A wrong state is refused.
    let callback = browser
        .get(
            &world,
            &format!("/auth/google/callback?code={}&state=nope", fake::CODE),
        )
        .await;
    let next = callback.body[callback.body.find(super::FINISH).unwrap()..]
        .split('"')
        .next()
        .unwrap()
        .replace("&amp;", "&");
    let refused = browser.get(&world, &next).await;
    assert_eq!(refused.location(), "/settings/connections?problem=expired");

    // Google comes back; a same-site step finishes.
    let start = browser.get(&world, super::CONNECT).await;
    let state_again = url::Url::parse(start.location())
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    assert_ne!(state_again, state);
    let callback = browser
        .get(
            &world,
            &format!(
                "/auth/google/callback?code={}&state={state_again}",
                fake::CODE
            ),
        )
        .await;
    assert_eq!(callback.status, StatusCode::OK);
    let next = callback.body[callback.body.find(super::FINISH).unwrap()..]
        .split('"')
        .next()
        .unwrap()
        .replace("&amp;", "&");
    let finished = browser.get(&world, &next).await;
    assert_eq!(finished.status, StatusCode::SEE_OTHER, "{}", finished.body);
    assert_eq!(finished.location(), super::PAGE);
    let owner = account_owner_of(&world, "octo-local");
    let account = store.load(&owner).unwrap();
    let stored = account.connection("google", "default").unwrap();
    assert_eq!(stored.refresh_token, fake::REFRESH);
    assert_eq!(stored.identity.as_deref(), Some(fake::EMAIL));
    let page = browser.get(&world, super::PAGE).await;
    assert!(
        page.body.contains("Connected as owner@example.com"),
        "{}",
        page.body
    );
    assert!(!page.body.contains(fake::REFRESH));
    crate::copy_guard::assert_plain(super::PAGE, &page.body);

    // A project, then its sources page lists My Drive to pick from.
    let callback = browser
        .through_github(&world, "/auth/github/repos?access=private", "octo-local")
        .await;
    let next = callback.body[callback.body.find(crate::projects::FINISH).unwrap()..]
        .split('"')
        .next()
        .unwrap()
        .replace("&amp;", "&");
    browser.get(&world, &next).await;
    let listing = browser.get(&world, "/projects/repositories?page=1").await;
    let csrf = hidden(
        &listing.body,
        r#"<form method="post" action="/projects">"#,
        "csrf",
    );
    browser
        .post(
            &world,
            crate::projects::PAGE,
            &[("csrf", &csrf), ("repository", "acme/storefront")],
        )
        .await;
    let projects = browser.get(&world, crate::projects::PAGE).await;
    let id = projects.body[projects.body.find("/?project=prj_").unwrap() + 10..][..20].to_string();
    let here = super::sources::href(&id);
    assert!(projects.body.contains(&here), "{}", projects.body);
    let sources = browser.get(&world, &here).await;
    assert_eq!(sources.status, StatusCode::OK, "{}", sources.body);
    assert!(sources.body.contains("Finances"), "{}", sources.body);
    assert!(sources.body.contains(FAST));
    crate::copy_guard::assert_plain(&here, &sources.body);
    let csrf = hidden(&sources.body, &format!(r#"action="{here}""#), "csrf");

    // A link that isn't Drive is refused; the folder is attached.
    let bad = browser
        .post(
            &world,
            &here,
            &[("csrf", &csrf), ("link", "https://example.com/x")],
        )
        .await;
    assert!(bad.location().contains("problem="), "{}", bad.location());
    let added = browser
        .post(
            &world,
            &here,
            &[
                ("csrf", &csrf),
                (
                    "link",
                    &format!("https://drive.google.com/drive/folders/{FOLDER}"),
                ),
            ],
        )
        .await;
    assert_eq!(added.location(), here);
    let account = store.load(&owner).unwrap();
    assert_eq!(account.sources_of(&id).len(), 1);
    assert_eq!(account.sources_of(&id)[0].name, "Finances");
    assert_eq!(account.sources_of(&id)[0].kind, "folder");

    // Another person can't see or change this project's sources.
    let mut other = Browser::default();
    other.sign_in(&world, "quiet-local").await;
    assert_eq!(other.get(&world, &here).await.status, StatusCode::NOT_FOUND);

    // Remove forgets the connection and asks Google to revoke it; the
    // sources stay for when Google is connected again.
    let page = browser.get(&world, super::PAGE).await;
    let csrf = hidden(
        &page.body,
        &format!(r#"action="{}""#, super::REMOVE),
        "csrf",
    );
    let removed = browser
        .post(&world, super::REMOVE, &[("csrf", &csrf)])
        .await;
    assert_eq!(removed.location(), super::PAGE);
    let account = store.load(&owner).unwrap();
    assert!(account.connection("google", "default").is_none());
    assert_eq!(account.sources.len(), 1);
    assert!(
        google
            .inner
            .lock()
            .unwrap()
            .revoked
            .contains(&fake::REFRESH.to_owned())
    );
}

/// A fake Gemini on Vertex: lists the folder, reads the sheet (and tries a
/// made-up id), then answers; or hands off when `handoff`.
#[derive(Default)]
struct FakeGemini {
    bodies: Vec<Value>,
    handoff: bool,
}

async fn gemini_handle(
    AxumState(state): AxumState<Arc<Mutex<FakeGemini>>>,
    axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
    let mut state = state.lock().unwrap();
    state.bodies.push(body.clone());
    let last = body["contents"].as_array().unwrap().last().unwrap().clone();
    let answered = last["parts"][0]["functionResponse"]["name"].as_str();
    let part = match (state.handoff, answered) {
        (true, _) => json!({"functionCall": {"name": "general_chat", "args": {}}}),
        (false, None) => {
            json!({"functionCall": {"name": "drive_list_folder", "args": {"folder": FOLDER}}, "thoughtSignature": "sig-1"})
        }
        (false, Some("drive_list_folder")) => {
            return axum::Json(
                json!({"candidates": [{"content": {"role": "model", "parts": [
                    {"functionCall": {"name": "drive_read", "args": {"file": SHEET}}},
                    {"functionCall": {"name": "drive_read", "args": {"file": "made-up-file-00001"}}}
                ]}}]}),
            );
        }
        (false, Some(_)) => json!({"text": "You spent 1,284.10 in October."}),
    };
    axum::Json(json!({"candidates": [{"content": {"role": "model", "parts": [part]}}]}))
}

async fn fake_gemini(handoff: bool) -> (Gemini, Arc<Mutex<FakeGemini>>) {
    let state = Arc::new(Mutex::new(FakeGemini {
        handoff,
        ..FakeGemini::default()
    }));
    let app = Router::new()
        .fallback(axum::routing::post(gemini_handle))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let gemini = Gemini {
        url: format!("{root}/v1/models/gemini:generateContent"),
        token: TokenSource::fixed(Secret::new("vertex-token").unwrap()),
        row: vertex::default_models()
            .into_iter()
            .find(|(row, _)| row.id == GEMINI)
            .unwrap()
            .0,
        thinking: vertex::Thinking::Level,
    };
    (gemini, state)
}

fn live(google: &FakeGoogle) -> Live {
    let stored = Stored {
        integration: "google".into(),
        name: "default".into(),
        identity: None,
        granted_scopes: vec![DRIVE_READONLY.into(), SHEETS_READONLY.into()],
        refresh_token: fake::REFRESH.into(),
        connected_at: 1,
        reconnect: false,
    };
    Live {
        drive: Drive::new(
            reqwest::Client::new(),
            google.endpoints(),
            fake::ACCESS.into(),
            true,
        ),
        connection: stored.connection("owner"),
        owner: "owner".into(),
    }
}

fn sources() -> Vec<Source> {
    vec![Source {
        project: "prj_0123456789abcdef".into(),
        integration: "google".into(),
        id: FOLDER.into(),
        name: "Finances".into(),
        kind: "folder".into(),
        added_at: 1,
    }]
}

#[tokio::test]
async fn the_drive_door_reads_the_sources_and_cites_the_files_it_read() {
    let google = FakeGoogle::start(files(), &[DRIVE_READONLY, SHEETS_READONLY]).await;
    let (gemini, state) = fake_gemini(false).await;
    let instructions = format!("Be brief.{}", chat::guidance(&sources()));
    let ended = chat::converse(
        &gemini,
        &Readers::default(),
        &live(&google),
        &sources(),
        &instructions,
        &[Turn::user("summarize my spending this month")],
    )
    .await
    .unwrap();
    let Ending::Answer(answered) = ended else {
        panic!("handed off");
    };
    let chat::Answered { text, read, .. } = &answered;
    let (text, read) = (text.clone(), read.clone());
    assert_eq!(text, "You spent 1,284.10 in October.");
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].name, "Spending October");
    let cited = chat::citations(&read);
    assert!(
        cited
            .contains("Files read: [Spending October](https://drive.example/sheet-spending-00001)")
    );
    assert!(cited.ends_with(FAST));

    let bodies = state.lock().unwrap().bodies.clone();
    assert_eq!(bodies.len(), 3);
    // The first request declares the Drive tools and the handoff, and names
    // the sources; nothing in any request is a token.
    let names: Vec<&str> = bodies[0]["tools"][0]["functionDeclarations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "drive_search",
            "drive_list_folder",
            "drive_read",
            "general_chat"
        ]
    );
    assert!(
        bodies[0]["systemInstruction"]["parts"][0]["text"]
            .as_str()
            .unwrap()
            .contains(FOLDER)
    );
    for body in &bodies {
        let text = body.to_string();
        assert!(!text.contains(fake::REFRESH) && !text.contains(fake::ACCESS));
    }
    // The model's turn goes back with its thought signature, then the
    // results; the sheet came as CSV and the made-up id was refused.
    let contents = bodies[2]["contents"].as_array().unwrap();
    assert_eq!(contents[1]["parts"][0]["thoughtSignature"], "sig-1");
    let results = &contents[4]["parts"];
    assert!(
        results[0]["functionResponse"]["response"]["result"]["text"]
            .as_str()
            .unwrap()
            .contains("Rent,1200.00")
    );
    assert!(
        results[1]["functionResponse"]["response"]["error"]
            .as_str()
            .unwrap()
            .contains("Only this project's sources")
    );
}

#[tokio::test]
async fn a_message_not_about_the_files_goes_to_the_general_chat() {
    let google = FakeGoogle::start(files(), &[DRIVE_READONLY]).await;
    let (gemini, state) = fake_gemini(true).await;
    let ended = chat::converse(
        &gemini,
        &Readers::default(),
        &live(&google),
        &sources(),
        "Be brief.",
        &[Turn::user("hello there")],
    )
    .await
    .unwrap();
    assert!(matches!(ended, Ending::Handoff));
    assert_eq!(state.lock().unwrap().bodies.len(), 1);
    // Nothing was read from Drive.
    assert!(google.requests().is_empty());
}

/// One app API call with `token`: the status and the JSON answer.
async fn api(
    world: &World,
    token: &str,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, header};
    use tower::ServiceExt;
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, world.host.as_str())
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(if body.is_null() {
            Body::empty()
        } else {
            Body::from(body.to_string())
        })
        .unwrap();
    let response = world.site.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// Connect Google for a signed-in browser through the fake Google.
async fn connect_google(browser: &mut Browser, world: &World) {
    let start = browser.get(world, super::CONNECT).await;
    assert_eq!(start.status, StatusCode::SEE_OTHER, "{}", start.body);
    let state = url::Url::parse(start.location())
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    let callback = browser
        .get(
            world,
            &format!("/auth/google/callback?code={}&state={state}", fake::CODE),
        )
        .await;
    let next = callback.body[callback.body.find(super::FINISH).unwrap()..]
        .split('"')
        .next()
        .unwrap()
        .replace("&amp;", "&");
    let finished = browser.get(world, &next).await;
    assert_eq!(finished.status, StatusCode::SEE_OTHER, "{}", finished.body);
}

#[tokio::test]
async fn the_api_connects_google_and_attaches_sources_for_allowed_accounts_only() {
    let google = FakeGoogle::start(files(), &[DRIVE_READONLY, SHEETS_READONLY]).await;
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    // octo-local is a site admin; quiet-local is invited, not an admin.
    let world = world_with(Options {
        public: true,
        invite: Some(json!({"github": [{"id": 583231, "admin": true}, {"id": 9000001}]})),
        connections: Some((
            store.clone(),
            Arc::new(Google::new(
                Client::new("cid.apps".into(), "client-secret".into()),
                google.endpoints(),
            )),
        )),
        ..Options::default()
    })
    .await;
    assert!(crate::upstream::owned("/v1/connections/google/drive"));
    assert!(crate::upstream::owned("/v1/admin/access/drive-connections"));
    assert!(crate::upstream::owned("/admin/access/drive-connections"));
    assert!(crate::upstream::owned("/v1/projects/prj_0/sources"));
    let mut admin = Browser::default();
    admin.sign_in(&world, "octo-local").await;
    let me = admin.0.get("oa_cloud_session").cloned().unwrap();
    let mut quiet = Browser::default();
    quiet.sign_in(&world, "quiet-local").await;
    let them = quiet.0.get("oa_cloud_session").cloned().unwrap();
    let quiet_id = account_id_of(&world, "quiet-local");

    // Someone not allowed sees nothing: no page, no link, no API.
    assert_eq!(
        quiet.get(&world, super::PAGE).await.status,
        StatusCode::NOT_FOUND
    );
    assert!(
        !quiet
            .get(&world, crate::settings::PAGE)
            .await
            .body
            .contains(super::PAGE)
    );
    for (method, path) in [
        ("GET", "/v1/connections"),
        ("POST", "/v1/connections/google/connect"),
        ("GET", "/v1/connections/google/drive"),
        ("DELETE", "/v1/connections/google"),
        ("POST", "/v1/connections/google/tools/drive_search"),
        ("GET", "/v1/projects/prj_0/sources"),
        ("GET", "/v1/admin/access/drive-connections"),
    ] {
        let (status, body) = api(&world, &them, method, path, json!({})).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}: {body}");
    }
    assert_eq!(
        quiet.get(&world, super::access::PAGE).await.status,
        StatusCode::NOT_FOUND
    );

    // An admin may: not connected yet, and the link that connects.
    let (status, listed) = api(&world, &me, "GET", "/v1/connections", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed["google"]["status"], "not_connected");
    let (status, consent) = api(
        &world,
        &me,
        "POST",
        "/v1/connections/google/connect",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{consent}");
    assert_eq!(
        consent["url"],
        "https://openagents.test/auth/google/connect?scope=drive"
    );
    let (_, consent) = api(
        &world,
        &me,
        "POST",
        "/v1/connections/google/connect",
        json!({"sheets": true}),
    )
    .await;
    assert_eq!(
        consent["url"],
        "https://openagents.test/auth/google/connect"
    );
    // That link asks Google for Drive alone.
    let start = admin.get(&world, "/auth/google/connect?scope=drive").await;
    assert!(start.location().contains("drive.readonly"));
    assert!(!start.location().contains("spreadsheets"));

    // A project; sources are refused cleanly until Google is connected.
    let (status, project) = api(
        &world,
        &me,
        "POST",
        "/v1/projects",
        json!({"name": "finances"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{project}");
    let id = project["id"].as_str().unwrap().to_owned();
    let sources = format!("/v1/projects/{id}/sources");
    let folder_link = format!("https://drive.google.com/drive/folders/{FOLDER}");
    let (status, refused) = api(&world, &me, "POST", &sources, json!({"link": folder_link})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(refused["error"]["code"], "google_not_connected");
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap()
            .contains("openagents connections connect google")
    );
    let (status, empty) = api(&world, &me, "GET", &sources, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["data"], json!([]));

    // Connected (Google's page is a person's click; the fake clicks).
    connect_google(&mut admin, &world).await;
    let (_, listed) = api(&world, &me, "GET", "/v1/connections", Value::Null).await;
    assert_eq!(listed["google"]["status"], "connected");
    assert_eq!(listed["google"]["email"], fake::EMAIL);
    assert_eq!(listed["google"]["sheets"], true);
    assert!(!listed.to_string().contains(fake::REFRESH));

    // Browse and search Drive to pick.
    let (status, root) = api(
        &world,
        &me,
        "GET",
        "/v1/connections/google/drive",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{root}");
    assert_eq!(root["data"][0]["name"], "Finances");
    assert_eq!(root["data"][0]["folder"], true);
    let (_, inside) = api(
        &world,
        &me,
        "GET",
        &format!("/v1/connections/google/drive?folder={FOLDER}"),
        Value::Null,
    )
    .await;
    assert_eq!(inside["data"][0]["name"], "Spending October");

    // Attach (checked with Drive), again (no change), a bad link, list.
    let (status, added) = api(&world, &me, "POST", &sources, json!({"link": folder_link})).await;
    assert_eq!(status, StatusCode::CREATED, "{added}");
    assert_eq!(added["name"], "Finances");
    assert_eq!(added["kind"], "folder");
    let (status, _) = api(&world, &me, "POST", &sources, json!({"id": FOLDER})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = api(
        &world,
        &me,
        "POST",
        &sources,
        json!({"link": "https://example.com/x"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = api(
        &world,
        &me,
        "POST",
        &sources,
        json!({"id": "missing-file-00001"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, listed) = api(&world, &me, "GET", &sources, Value::Null).await;
    assert_eq!(listed["data"].as_array().unwrap().len(), 1);
    assert_eq!(listed["google"], "connected");
    // The project's chat routes to Drive for this account.
    let owner = account_owner_of(&world, "octo-local");
    assert!(
        store
            .access()
            .unwrap()
            .seen
            .iter()
            .any(|s| crate::chat_store::account_owner(&s.account) == owner && s.via == "admin")
    );

    // The admin lets quiet-local in, with no redeploy; then out again.
    let (status, list) = api(
        &world,
        &me,
        "POST",
        "/v1/admin/access/drive-connections",
        json!({"account": quiet_id}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{list}");
    let (status, theirs) = api(&world, &them, "GET", "/v1/connections", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{theirs}");
    assert_eq!(theirs["google"]["status"], "not_connected");
    assert_eq!(quiet.get(&world, super::PAGE).await.status, StatusCode::OK);
    let (status, _) = api(
        &world,
        &me,
        "DELETE",
        &format!("/v1/admin/access/drive-connections/{quiet_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = api(&world, &them, "GET", "/v1/connections", Value::Null).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // By email too.
    let (status, list) = api(
        &world,
        &me,
        "POST",
        "/v1/admin/access/drive-connections",
        json!({"email": "Someone@Example.com"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{list}");
    assert!(list.to_string().contains("someone@example.com"));
    // The admin page lists them.
    let page = admin.get(&world, super::access::PAGE).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("someone@example.com"));
    crate::copy_guard::assert_plain(super::access::PAGE, &page.body);

    // Detach, then remove the connection: the sources stay.
    let (status, _) = api(
        &world,
        &me,
        "DELETE",
        &format!("{sources}/{FOLDER}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = api(
        &world,
        &me,
        "DELETE",
        &format!("{sources}/{FOLDER}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, removed) = api(&world, &me, "DELETE", "/v1/connections/google", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{removed}");
    assert_eq!(removed["deleted"], true);
    let (_, listed) = api(&world, &me, "GET", "/v1/connections", Value::Null).await;
    assert_eq!(listed["google"]["status"], "not_connected");
}

#[test]
fn the_access_list_starts_with_the_owners_accounts_and_matches_email_without_case() {
    use crate::cloud::connections::Access;
    let mut access = Access::seeded();
    assert!(
        access
            .entry_for("acct_x", Some("Chris@OpenAgents.com"))
            .is_some()
    );
    assert!(
        access
            .entry_for("acct_x", Some("other@example.com"))
            .is_none()
    );
    assert!(access.remove("chris@openagents.com"));
    assert!(
        access
            .entry_for("acct_x", Some("chris@openagents.com"))
            .is_none()
    );
    assert_eq!(
        access.add(AccessEntry {
            account: Some("acct_y".into()),
            ..AccessEntry::default()
        }),
        Ok(true)
    );
    assert!(access.knows("acct_y"));
}
