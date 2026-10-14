//! `openagents projects members|invite|join|role|remove|leave` (#11590)
//! against a small stand-in for the website's sharing routes
//! (`docs/api/projects.md`, "Members and invitations").
#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

const OWNED: &str = "prj_aaaaaaaaaaaaaaaa";
const SHARED: &str = "prj_bbbbbbbbbbbbbbbb";
const ONLY_ME: &str = "prj_cccccccccccccccc";
const JOINED: &str = "prj_dddddddddddddddd";
const RECIPES: &str = "prj_eeeeeeeeeeeeeeee";

/// What the stand-in keeps: the caller's projects, the owned project's
/// members, and every write it was sent as (method, path, body).
#[derive(Default)]
struct State {
    projects: Vec<Value>,
    members: Vec<Value>,
    writes: Vec<(String, String, Value)>,
}

type Shared = Arc<Mutex<State>>;

fn token() -> String {
    format!("sess_{}", "a".repeat(64))
}

fn project(id: &str, name: &str, role: &str, members: u64) -> Value {
    json!({"id": id, "object": "project", "name": name, "repositories": [],
        "counts": {"threads": 0}, "updated_at": "2026-10-14T08:00:00Z",
        "protection": if id == ONLY_ME { "only_me" } else { "standard" },
        "role": role, "members": members, "shared": members > 1})
}

fn state() -> Shared {
    Arc::new(Mutex::new(State {
        projects: vec![
            project(OWNED, "finances", "owner", 3),
            project(SHARED, "books", "editor", 2),
            project(ONLY_ME, "diary", "owner", 1),
            project(RECIPES, "recipes", "viewer", 4),
        ],
        members: vec![
            json!({"account": "acct_1", "object": "project.member", "name": "Chris", "login": "chris", "role": "owner", "joined_at": "2026-10-01T00:00:00Z", "you": true}),
            json!({"account": "acct_2", "object": "project.member", "name": "Sam", "login": "sam-a", "role": "editor", "joined_at": "2026-10-02T00:00:00Z", "you": false}),
            json!({"account": "acct_3", "object": "project.member", "name": "Sam", "login": "sam-b", "role": "viewer", "joined_at": "2026-10-03T00:00:00Z", "you": false}),
        ],
        writes: Vec::new(),
    }))
}

fn site(state: Shared) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = Vec::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                head.push(line.trim_end().to_owned());
            }
            let header = |name: &str| {
                head.iter().find_map(|l| {
                    l.split_once(':')
                        .filter(|(n, _)| n.eq_ignore_ascii_case(name))
                        .map(|(_, v)| v.trim().to_owned())
                })
            };
            let length: usize = header("content-length")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let mut words = head[0].split(' ');
            let (method, target) = (words.next().unwrap(), words.next().unwrap());
            let path = target.split_once('?').map_or(target, |(path, _)| path);
            let signed_in =
                header("authorization").as_deref() == Some(format!("Bearer {}", token()).as_str());
            let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let (status, answer) = if signed_in {
                route(&state, method, path, body)
            } else {
                ("401 Unauthorized", json!({"error": {"code": "signed_out"}}))
            };
            let text = answer.to_string();
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
                text.len()
            );
        }
    });
    origin
}

fn refused(status: &'static str, code: &str, message: &str) -> (&'static str, Value) {
    (status, json!({"error": {"code": code, "message": message}}))
}

fn route(state: &Shared, method: &str, path: &str, body: Value) -> (&'static str, Value) {
    let mut state = state.lock().unwrap();
    if method != "GET" {
        state
            .writes
            .push((method.to_owned(), path.to_owned(), body.clone()));
    }
    let owned = state.projects[0].clone();
    match (method, path) {
        ("GET", "/v1/projects") => ("200 OK", json!({"data": state.projects, "next": null})),
        ("GET", "/v1/threads") => ("200 OK", json!({"data": [], "next": null})),
        ("GET", p) if p == format!("/v1/projects/{OWNED}/members") => (
            "200 OK",
            json!({"data": state.members, "next": null, "role": "owner",
                "invitations": [{"id": "pinv_1", "role": "viewer", "expires_at": "2026-10-21T00:00:00Z"}]}),
        ),
        ("POST", p) if p == format!("/v1/projects/{ONLY_ME}/invitations") => refused(
            "409 Conflict",
            "only_me_project",
            "Files in this project open only on your devices. Change it to standard to share it.",
        ),
        ("POST", p) if p == format!("/v1/projects/{OWNED}/invitations") => (
            "201 Created",
            json!({"object": "project.invitation", "id": "pinv_2", "project": OWNED,
                "role": body["role"].as_str().unwrap_or("editor"), "code": "goodcode",
                "url": "https://openagents.com/projects/join#goodcode",
                "expires_at": "2026-10-15T00:00:00Z"}),
        ),
        ("POST", "/v1/projects/invitations/preview" | "/v1/projects/invitations/accept")
            if !body["code"]
                .as_str()
                .unwrap_or_default()
                .contains("goodcode") =>
        {
            refused(
                "404 Not Found",
                "invitation_not_found",
                "That invitation was used or has expired. Ask for a new link.",
            )
        }
        ("POST", "/v1/projects/invitations/preview") => (
            "200 OK",
            json!({"object": "project.invitation", "project": {"id": JOINED, "name": "garden"},
                "role": "viewer", "invited_by": "Pat", "expires_at": "2026-10-15T00:00:00Z", "member": false}),
        ),
        ("POST", "/v1/projects/invitations/accept") => {
            let joined = project(JOINED, "garden", "viewer", 2);
            state.projects.push(joined.clone());
            ("200 OK", joined)
        }
        ("PATCH", p) if p.starts_with(&format!("/v1/projects/{OWNED}/members/")) => {
            let account = p.rsplit('/').next().unwrap_or_default().to_owned();
            let mut changed = owned;
            if body["role"] == "owner" {
                changed["role"] = json!("editor");
            }
            for member in &mut state.members {
                if member["account"] == account.as_str() {
                    member["role"] = body["role"].clone();
                }
            }
            ("200 OK", changed)
        }
        ("DELETE", p) if p.starts_with(&format!("/v1/projects/{OWNED}/members/")) => {
            let account = p.rsplit('/').next().unwrap_or_default().to_owned();
            state.members.retain(|m| m["account"] != account.as_str());
            (
                "200 OK",
                json!({"account": account, "object": "project.member.removed", "project": OWNED}),
            )
        }
        ("POST", p)
            if p == format!("/v1/projects/{SHARED}/leave")
                || p == format!("/v1/projects/{RECIPES}/leave") =>
        {
            let id = p
                .trim_start_matches("/v1/projects/")
                .trim_end_matches("/leave")
                .to_owned();
            state.projects.retain(|p| p["id"] != id.as_str());
            ("200 OK", json!({"id": id, "object": "project.left"}))
        }
        ("POST", p) if p == format!("/v1/projects/{OWNED}/leave") => refused(
            "409 Conflict",
            "owner_stays",
            "The owner can't leave the project.",
        ),
        _ => refused("404 Not Found", "not_found", "Not found."),
    }
}

fn openagents(home: &Path, origin: &str, args: &[&str]) -> Output {
    let token = token();
    Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", home)
        .env("OPENAGENTS_ORIGIN", origin)
        .env("OPENAGENTS_TOKEN", &token)
        .env("BROWSER", "/usr/bin/true")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

fn text(output: &Output) -> String {
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn json_out(output: &Output) -> Value {
    serde_json::from_str(&text(output)).unwrap()
}

fn failed(output: &Output) -> String {
    assert!(!output.status.success());
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn last_write(state: &Shared) -> (String, String, Value) {
    state.lock().unwrap().writes.last().cloned().unwrap()
}

#[test]
fn list_and_show_print_your_role_and_mark_shared_projects() {
    let state = state();
    let origin = site(state.clone());
    let home = tempfile::tempdir().unwrap();
    let listed = text(&openagents(home.path(), &origin, &["projects", "list"]));
    assert!(
        listed.contains("(prj_aaaaaaaaaaaaaaaa)  shared with 3 people, you're the owner"),
        "{listed}"
    );
    assert!(
        listed.contains("(prj_bbbbbbbbbbbbbbbb)  shared with 2 people, you're an editor"),
        "{listed}"
    );
    assert!(
        listed.contains("(prj_cccccccccccccccc)\n") || listed.ends_with("(prj_cccccccccccccccc)\n"),
        "{listed}"
    );
    let shown = text(&openagents(
        home.path(),
        &origin,
        &["projects", "show", "books"],
    ));
    assert!(
        shown.contains("You're an editor of this project. See who else is in it with: openagents projects members books"),
        "{shown}"
    );
    let listed = json_out(&openagents(
        home.path(),
        &origin,
        &["--json", "projects", "list"],
    ));
    assert_eq!(listed["data"][1]["role"], "editor");
    assert_eq!(listed["data"][1]["shared"], true);
}

#[test]
fn members_lists_people_roles_and_open_links() {
    let state = state();
    let origin = site(state.clone());
    let home = tempfile::tempdir().unwrap();
    let listed = text(&openagents(
        home.path(),
        &origin,
        &["projects", "members", "finances"],
    ));
    assert!(
        listed.starts_with("finances: 3 people. You're the owner.\n"),
        "{listed}"
    );
    assert!(
        listed.contains("  Chris  @chris  owner  joined 2026-10-01  (acct_1)  you"),
        "{listed}"
    );
    assert!(
        listed.contains("  Sam  @sam-b  viewer  joined 2026-10-03  (acct_3)"),
        "{listed}"
    );
    assert!(
        listed.contains("  viewer  works until 2026-10-21  (pinv_1)"),
        "{listed}"
    );
    let listed = json_out(&openagents(
        home.path(),
        &origin,
        &["--json", "projects", "members", "finances"],
    ));
    assert_eq!(listed["role"], "owner");
    assert_eq!(listed["data"].as_array().unwrap().len(), 3);
    assert_eq!(listed["data"][1]["object"], "project.member");
    assert_eq!(listed["invitations"][0]["id"], "pinv_1");
}

#[test]
fn invite_prints_the_link_once_with_role_and_ttl() {
    let state = state();
    let origin = site(state.clone());
    let home = tempfile::tempdir().unwrap();
    let made = text(&openagents(
        home.path(),
        &origin,
        &[
            "projects", "invite", "finances", "--role", "viewer", "--ttl", "12h",
        ],
    ));
    assert!(
        made.contains("inviting to finances as a viewer. It works once, until 2026-10-15, and won't be shown again:\nhttps://openagents.com/projects/join#goodcode\n"),
        "{made}"
    );
    let (method, path, body) = last_write(&state);
    assert_eq!(
        (method.as_str(), path.as_str()),
        ("POST", "/v1/projects/prj_aaaaaaaaaaaaaaaa/invitations")
    );
    assert_eq!(body, json!({"role": "viewer", "expires_in": 43_200}));

    // Defaults: nothing sent, the website picks editor for 7 days.
    let made = json_out(&openagents(
        home.path(),
        &origin,
        &["--json", "projects", "invite", "finances"],
    ));
    assert_eq!(made["object"], "project.invitation");
    assert_eq!(made["role"], "editor");
    assert_eq!(made["code"], "goodcode");
    assert_eq!(last_write(&state).2, json!({}));

    // A wrong role or length never reaches the website.
    let writes = state.lock().unwrap().writes.len();
    let wrong = failed(&openagents(
        home.path(),
        &origin,
        &["projects", "invite", "finances", "--role", "owner"],
    ));
    assert!(wrong.contains("--role is editor or viewer"), "{wrong}");
    let wrong = failed(&openagents(
        home.path(),
        &origin,
        &["projects", "invite", "finances", "--ttl", "soon"],
    ));
    assert!(wrong.contains("30m, 12h, or 7d"), "{wrong}");
    assert_eq!(state.lock().unwrap().writes.len(), writes);

    // An only-me project can't be shared; the website says why.
    let refused = failed(&openagents(
        home.path(),
        &origin,
        &["projects", "invite", "diary"],
    ));
    assert!(
        refused.contains("Change it to standard to share it."),
        "{refused}"
    );
}

#[test]
fn join_previews_then_accepts_and_says_who_invited() {
    let state = state();
    let origin = site(state.clone());
    let home = tempfile::tempdir().unwrap();
    let joined = text(&openagents(
        home.path(),
        &origin,
        &[
            "projects",
            "join",
            "https://openagents.com/projects/join#goodcode",
        ],
    ));
    assert_eq!(
        joined.trim_end(),
        "You joined garden as a viewer. Pat invited you. See it with: openagents projects show garden"
    );
    let writes = state.lock().unwrap().writes.clone();
    let paths: Vec<&str> = writes.iter().map(|(_, p, _)| p.as_str()).collect();
    assert_eq!(
        paths,
        [
            "/v1/projects/invitations/preview",
            "/v1/projects/invitations/accept"
        ]
    );
    assert_eq!(
        writes[1].2["code"],
        "https://openagents.com/projects/join#goodcode"
    );

    let joined = json_out(&openagents(
        home.path(),
        &origin,
        &["--json", "projects", "join", "goodcode"],
    ));
    assert_eq!(joined["invitation"]["invited_by"], "Pat");
    assert_eq!(joined["project"]["id"], JOINED);
    assert_eq!(joined["project"]["role"], "viewer");

    let gone = failed(&openagents(
        home.path(),
        &origin,
        &["projects", "join", "usedcode"],
    ));
    assert!(gone.contains("used or has expired"), "{gone}");
}

#[test]
fn role_changes_a_member_found_by_login_name_or_account() {
    let state = state();
    let origin = site(state.clone());
    let home = tempfile::tempdir().unwrap();
    let changed = text(&openagents(
        home.path(),
        &origin,
        &["projects", "role", "finances", "@sam-b", "editor"],
    ));
    assert_eq!(changed.trim_end(), "Sam is an editor of finances now.");
    let (method, path, body) = last_write(&state);
    assert_eq!(
        (method.as_str(), path.as_str()),
        ("PATCH", "/v1/projects/prj_aaaaaaaaaaaaaaaa/members/acct_3")
    );
    assert_eq!(body, json!({"role": "editor"}));

    // Two members are named Sam: named both, nothing sent.
    let writes = state.lock().unwrap().writes.len();
    let ambiguous = failed(&openagents(
        home.path(),
        &origin,
        &["projects", "role", "finances", "Sam", "viewer"],
    ));
    assert!(
        ambiguous.contains("Two members are called Sam: acct_2 (Sam @sam-a), acct_3 (Sam @sam-b). Use the account id."),
        "{ambiguous}"
    );
    assert_eq!(state.lock().unwrap().writes.len(), writes);
    let wrong = failed(&openagents(
        home.path(),
        &origin,
        &["projects", "role", "finances", "acct_2", "admin"],
    ));
    assert!(wrong.contains("editor, viewer, or owner"), "{wrong}");

    // owner gives the project away.
    let given = json_out(&openagents(
        home.path(),
        &origin,
        &["--json", "projects", "role", "finances", "acct_2", "owner"],
    ));
    assert_eq!(given["object"], "project");
    assert_eq!(given["role"], "editor");
    let text_given = text(&openagents(
        home.path(),
        &origin,
        &["projects", "role", "finances", "chris", "owner"],
    ));
    assert_eq!(
        text_given.trim_end(),
        "Chris owns finances now. You're an editor."
    );
}

#[test]
fn remove_takes_a_member_out_and_leave_leaves() {
    let state = state();
    let origin = site(state.clone());
    let home = tempfile::tempdir().unwrap();
    let removed = text(&openagents(
        home.path(),
        &origin,
        &["projects", "remove", "finances", "sam-a"],
    ));
    assert_eq!(removed.trim_end(), "Took Sam out of finances.");
    let (method, path, _) = last_write(&state);
    assert_eq!(
        (method.as_str(), path.as_str()),
        ("DELETE", "/v1/projects/prj_aaaaaaaaaaaaaaaa/members/acct_2")
    );
    let removed = json_out(&openagents(
        home.path(),
        &origin,
        &["--json", "projects", "remove", "finances", "acct_3"],
    ));
    assert_eq!(
        removed,
        json!({"account": "acct_3", "object": "project.member.removed", "project": OWNED})
    );
    // The project itself is still there.
    assert_eq!(state.lock().unwrap().projects.len(), 4);

    let left = text(&openagents(
        home.path(),
        &origin,
        &["projects", "leave", "books"],
    ));
    assert_eq!(left.trim_end(), "You left books.");
    let left = json_out(&openagents(
        home.path(),
        &origin,
        &["--json", "projects", "leave", "recipes"],
    ));
    assert_eq!(left, json!({"id": RECIPES, "object": "project.left"}));
    assert_eq!(state.lock().unwrap().projects.len(), 2);

    // The owner can't leave; the command says how to give it away first.
    let writes = state.lock().unwrap().writes.len();
    let stays = failed(&openagents(
        home.path(),
        &origin,
        &["projects", "leave", "finances"],
    ));
    assert!(
        stays.contains("You own finances, so you can't leave it. Give it to another member first with: openagents projects role finances ACCOUNT owner"),
        "{stays}"
    );
    assert_eq!(state.lock().unwrap().writes.len(), writes);
}
