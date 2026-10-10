//! The account surface (#11107, #11165) against a fake website.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use crate::account_link::{
    Action, Calling, Http, Intent, Link, Reply, Screen, Session, SignIn, upload_messages,
    upload_title,
};

const SITE: &str = "https://staging.example";

/// One call the fake site saw.
#[derive(Clone, Debug)]
struct Call {
    method: &'static str,
    path: String,
    token: Option<String>,
    body: Option<Value>,
}

/// A fake website: canned answers by method and path (queued answers
/// first, then a standing one), and every call it saw.
#[derive(Default)]
struct Site {
    calls: Mutex<Vec<Call>>,
    queued: Mutex<Vec<(String, VecDeque<Reply>)>>,
    standing: Mutex<Vec<(String, Reply)>>,
}

impl Site {
    fn answer(&self, route: &str, status: u16, body: Value) {
        self.standing
            .lock()
            .unwrap()
            .push((route.into(), Reply { status, body }));
    }

    fn once(&self, route: &str, status: u16, body: Value) {
        let mut queued = self.queued.lock().unwrap();
        let reply = Reply { status, body };
        match queued.iter_mut().find(|(r, _)| r == route) {
            Some((_, replies)) => replies.push_back(reply),
            None => queued.push((route.into(), VecDeque::from([reply]))),
        }
    }

    fn calls(&self, route: &str) -> Vec<Call> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| format!("{} {}", c.method, c.path) == route)
            .cloned()
            .collect()
    }
}

impl Http for Site {
    fn call(
        &self,
        method: &'static str,
        url: String,
        token: Option<String>,
        body: Option<Value>,
    ) -> Calling {
        let path = url.strip_prefix(SITE).unwrap_or(&url).to_owned();
        let route = format!("{method} {path}");
        self.calls.lock().unwrap().push(Call {
            method,
            path,
            token,
            body,
        });
        let queued = self
            .queued
            .lock()
            .unwrap()
            .iter_mut()
            .find(|(r, _)| *r == route)
            .and_then(|(_, replies)| replies.pop_front());
        let reply = queued.or_else(|| {
            self.standing
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|(r, _)| *r == route)
                .map(|(_, reply)| reply.clone())
        });
        Box::pin(async move {
            Ok(reply.unwrap_or(Reply {
                status: 404,
                body: json!({"error": {"code": "not_found", "message": "Not found."}}),
            }))
        })
    }

    /// An upload is seen as a call whose body is the byte count.
    fn upload(&self, url: String, token: Option<String>, bytes: Vec<u8>) -> Calling {
        self.call("POST", url, token, Some(json!({"bytes": bytes.len()})))
    }
}

struct Fixture {
    runtime: tokio::runtime::Runtime,
    site: Arc<Site>,
    link: Link,
}

fn fixture() -> Fixture {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let site = Arc::new(Site::default());
    site.answer(
        "GET /v1/threads",
        200,
        json!({"threads": [], "computers": []}),
    );
    site.answer("GET /v1/agents", 200, json!({"computers": []}));
    site.answer(
        "GET /v1/computers/Pixel%20Test/sync",
        200,
        json!({"choice": null}),
    );
    let link = Link::new(
        SITE,
        site.clone(),
        Some(runtime.handle().clone()),
        Arc::new(|| {}),
        None,
    );
    Fixture {
        runtime,
        site,
        link,
    }
}

fn until(mut done: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for the account surface"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn signed(fixture: &mut Fixture) {
    let session = serde_json::to_string(&Session::for_test(SITE, "sess_test")).unwrap();
    fixture.link.act(Action::Hello {
        name: "Pixel Test".into(),
        session: Some(session),
    });
}

/// Every text the view shows, for copy checks.
fn texts(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if matches!(key.as_str(), "value" | "label" | "placeholder")
                    && let Some(text) = value.as_str()
                {
                    out.push(text.to_owned());
                }
                texts(value, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| texts(v, out)),
        _ => {}
    }
}

#[test]
fn sign_in_shows_a_code_and_keeps_the_session_once() {
    let mut f = fixture();
    f.site.answer(
        "POST /device/code",
        200,
        json!({
            "device_code": "dev_1",
            "user_code": "ABCD-EFGH",
            "verification_uri": format!("{SITE}/device"),
            "verification_uri_complete": format!("{SITE}/device?code=ABCD-EFGH"),
            "expires_in": 600,
            "interval": 1,
        }),
    );
    f.site.once(
        "POST /device/token",
        400,
        json!({"error": "authorization_pending"}),
    );
    f.site.answer(
        "POST /device/token",
        200,
        json!({
            "access_token": "sess_phone",
            "token_type": "Bearer",
            "expires_in": 2_592_000,
            "account": {"id": "acct_1", "label": "Chris"},
        }),
    );
    f.link.act(Action::Hello {
        name: "Pixel Test".into(),
        session: None,
    });
    f.link.act(Action::Show {
        screen: Screen::Account,
        id: None,
    });
    let packet = f.link.packet();
    assert!(!packet.signed_in);
    f.link.tap(Intent::SignIn);
    until(|| matches!(f.link.lock().sign_in, SignIn::Waiting { .. }));
    let packet = f.link.packet();
    assert!(packet.qr.is_some(), "the QR shows while waiting");
    let mut words = vec![];
    texts(packet.view.as_ref().unwrap(), &mut words);
    assert!(words.iter().any(|w| w == "ABCD-EFGH"), "{words:?}");
    assert_eq!(
        f.link.tap(Intent::OpenPage).as_deref(),
        Some("https://staging.example/device?code=ABCD-EFGH")
    );
    let start = &f.site.calls("POST /device/code")[0];
    assert_eq!(start.body.as_ref().unwrap()["app"], "OpenAgents");
    assert_eq!(start.body.as_ref().unwrap()["computer"], "Pixel Test");
    until(|| f.link.signed_in());
    let packet = f.link.packet();
    assert!(packet.signed_in);
    assert_eq!(packet.label.as_deref(), Some("Chris"));
    let stored = packet
        .store
        .expect("the session goes to the protected store");
    assert_eq!(stored.origin, SITE);
    assert!(format!("{stored:?}").contains("[redacted]"));
    assert!(!format!("{stored:?}").contains("sess_phone"));
    assert!(f.link.packet().store.is_none(), "handed over once");
    // Signed in, it reads the account with the new session.
    until(|| !f.site.calls("GET /v1/threads").is_empty());
    assert_eq!(
        f.site.calls("GET /v1/threads")[0].token.as_deref(),
        Some("sess_phone")
    );
    drop(f.runtime);
}

#[test]
fn a_denied_code_says_so() {
    let mut f = fixture();
    f.site.answer(
        "POST /device/code",
        200,
        json!({"device_code": "d", "user_code": "C", "verification_uri": format!("{SITE}/device"),
               "expires_in": 600, "interval": 1}),
    );
    f.site
        .answer("POST /device/token", 400, json!({"error": "access_denied"}));
    f.link.tap(Intent::SignIn);
    until(|| matches!(f.link.lock().sign_in, SignIn::Failed(_)));
    assert_eq!(
        f.link.lock().sign_in,
        SignIn::Failed("Sign-in was denied on the website.".into())
    );
}

#[test]
fn a_session_for_another_site_is_forgotten() {
    let mut f = fixture();
    let other =
        serde_json::to_string(&Session::for_test("https://openagents.com", "sess_x")).unwrap();
    f.link.act(Action::Hello {
        name: "Pixel Test".into(),
        session: Some(other),
    });
    let packet = f.link.packet();
    assert!(!packet.signed_in);
    assert!(packet.forget);
}

#[test]
fn chats_list_web_and_terminal_chats_by_computer() {
    let mut f = fixture();
    f.site.answer(
        "GET /v1/threads",
        200,
        json!({
            "threads": [
                {"id": "w1", "title": "Plan the launch", "surface": "web", "updated_unix": 10,
                 "can_reply": true},
                {"id": "t1", "title": "Fix the build", "surface": "terminal", "computer": "Studio",
                 "session": "s1", "updated_unix": 20, "online": true, "can_reply": true},
                {"id": "p1", "title": "Mine", "surface": "phone", "computer": "Pixel Test",
                 "updated_unix": 30},
            ],
            "computers": [{"name": "Studio", "online": true, "sync": "all"}],
        }),
    );
    signed(&mut f);
    until(|| f.link.lock().threads_read);
    let packet = f.link.packet();
    let drawer: Vec<(String, String)> = packet
        .drawer
        .iter()
        .map(|r| (r.title.clone(), r.detail.clone()))
        .collect();
    // This phone's own chats are already in the drawer.
    assert_eq!(
        drawer,
        vec![
            ("Fix the build".into(), "Terminal · Studio".into()),
            ("Plan the launch".into(), "openagents.com".into()),
        ]
    );
    f.link.act(Action::Show {
        screen: Screen::Chats,
        id: None,
    });
    let mut words = vec![];
    texts(f.link.packet().view.as_ref().unwrap(), &mut words);
    assert!(
        words.iter().any(|w| w == "Terminal · Studio · Online"),
        "{words:?}"
    );
    assert!(words.iter().any(|w| w == "openagents.com"), "{words:?}");
}

#[test]
fn a_reply_to_a_terminal_chat_waits_for_coder() {
    let mut f = fixture();
    f.site.answer(
        "GET /v1/threads/t1",
        200,
        json!({
            "thread": {"id": "t1", "title": "Fix the build", "surface": "terminal",
                       "computer": "Studio", "online": true, "can_reply": true},
            "messages": [{"role": "user", "text": "fix it"}, {"role": "assistant", "text": "Done."}],
            "earlier": 0, "waiting": 0,
        }),
    );
    f.site
        .answer("POST /v1/threads/t1/messages", 202, json!({"queued": true}));
    signed(&mut f);
    f.link.act(Action::Show {
        screen: Screen::Chat,
        id: Some("t1".into()),
    });
    until(|| f.link.lock().open.as_ref().is_some_and(|o| o.loaded));
    let packet = f.link.packet();
    let mut words = vec![];
    texts(packet.view.as_ref().unwrap(), &mut words);
    assert!(words.iter().any(|w| w == "Reply"), "{words:?}");
    let token = format!("link-reply-t1-{}", f.link.composer);
    f.link.input(&token, "  run the tests  ");
    until(|| !f.site.calls("POST /v1/threads/t1/messages").is_empty());
    let sent = &f.site.calls("POST /v1/threads/t1/messages")[0];
    assert_eq!(sent.body.as_ref().unwrap()["text"], "run the tests");
    assert_eq!(sent.token.as_deref(), Some("sess_test"));
    let request = sent.body.as_ref().unwrap()["request_id"].as_str().unwrap();
    assert_eq!(request.len(), 36);
    until(|| {
        f.link
            .lock()
            .open
            .as_ref()
            .is_some_and(|o| o.sent == vec!["run the tests".to_owned()])
    });
    // A credential never leaves the phone.
    f.link.input(
        &token,
        "my key is sk-ant-api03-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    );
    assert_eq!(f.site.calls("POST /v1/threads/t1/messages").len(), 1);
    assert!(
        f.link
            .lock()
            .notice
            .as_deref()
            .is_some_and(|n| n.contains("password or key"))
    );
}

#[test]
fn running_notices_a_question_and_answers_it() {
    let mut f = fixture();
    let working = json!({"computers": [{"name": "Studio", "online": true, "updated_unix": 1,
        "items": [{"id": "s1", "kind": "chat", "title": "Fix the build", "status": "working",
                   "started_unix": 1, "cost_usd": 0.42}]}]});
    let asking = json!({"computers": [{"name": "Studio", "online": true, "updated_unix": 2,
        "items": [{"id": "s1", "kind": "chat", "title": "Fix the build", "status": "asking",
                   "started_unix": 1, "question": {"id": "7", "text": "Run rm -rf build?"}}]}]});
    f.site.once("GET /v1/agents", 200, working);
    f.site.answer("GET /v1/agents", 200, asking);
    f.site
        .answer("POST /v1/agents/actions", 202, json!({"queued": true}));
    signed(&mut f);
    until(|| f.link.lock().agents_read);
    // The first read only learns what runs.
    assert!(f.link.packet().notify.is_empty());
    f.link.act(Action::Show {
        screen: Screen::Running,
        id: None,
    });
    until(|| {
        f.link
            .lock()
            .agents
            .first()
            .is_some_and(|c| c.items[0].status == "asking")
    });
    let packet = f.link.packet();
    assert_eq!(packet.asking, 1);
    let notice = packet.notify.first().expect("a notice for the question");
    assert_eq!(notice.title, "Fix the build asks");
    assert_eq!(notice.body, "Run rm -rf build?");
    assert_eq!(notice.question.as_deref(), Some("7"));
    let mut words = vec![];
    texts(packet.view.as_ref().unwrap(), &mut words);
    assert!(words.iter().any(|w| w == "Approve"), "{words:?}");
    assert!(words.iter().any(|w| w == "Stop"), "{words:?}");
    // The notification's Approve.
    f.link.act(Action::Answer {
        computer: "Studio".into(),
        item: "s1".into(),
        question: "7".into(),
        approve: true,
    });
    until(|| !f.site.calls("POST /v1/agents/actions").is_empty());
    let body = f.site.calls("POST /v1/agents/actions")[0]
        .body
        .clone()
        .unwrap();
    assert_eq!(body["action"], "approve");
    assert_eq!(body["question"], "7");
    assert_eq!(body["computer"], "Studio");
    // A second tap sends nothing more.
    f.link.act(Action::Answer {
        computer: "Studio".into(),
        item: "s1".into(),
        question: "7".into(),
        approve: true,
    });
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(f.site.calls("POST /v1/agents/actions").len(), 1);
    // Stop from the screen.
    f.link.tap(Intent::Act {
        computer: "Studio".into(),
        item: "s1".into(),
        action: "stop".into(),
        question: None,
    });
    until(|| f.site.calls("POST /v1/agents/actions").len() == 2);
    assert_eq!(
        f.site.calls("POST /v1/agents/actions")[1]
            .body
            .as_ref()
            .unwrap()["action"],
        "stop"
    );
}

#[test]
fn a_message_to_an_agent_goes_through_its_composer() {
    let mut f = fixture();
    f.site.answer(
        "GET /v1/agents",
        200,
        json!({"computers": [{"name": "Studio", "online": true, "updated_unix": 1,
            "items": [{"id": "a1", "kind": "agent", "title": "Docs agent", "status": "working",
                       "started_unix": 1}]}]}),
    );
    f.site
        .answer("POST /v1/agents/actions", 202, json!({"queued": true}));
    signed(&mut f);
    until(|| f.link.lock().agents_read);
    f.link.act(Action::Show {
        screen: Screen::Running,
        id: None,
    });
    f.link.tap(Intent::Message {
        computer: "Studio".into(),
        item: "a1".into(),
    });
    assert_eq!(f.link.lock().screen, Screen::Message);
    let token = format!("link-message-{}", f.link.composer);
    f.link.input(&token, "also update the README");
    until(|| !f.site.calls("POST /v1/agents/actions").is_empty());
    let body = f.site.calls("POST /v1/agents/actions")[0]
        .body
        .clone()
        .unwrap();
    assert_eq!(body["action"], "message");
    assert_eq!(body["text"], "also update the README");
    until(|| f.link.lock().screen == Screen::Running);
}

#[test]
fn an_ended_session_signs_out() {
    let mut f = fixture();
    f.site.answer(
        "GET /v1/threads",
        401,
        json!({"error": {"code": "signed_out", "message": "Sign in again."}}),
    );
    signed(&mut f);
    until(|| !f.link.signed_in());
    let packet = f.link.packet();
    assert!(packet.forget);
    assert!(!packet.signed_in);
}

#[test]
fn sign_out_ends_the_session_on_the_site() {
    let mut f = fixture();
    f.site.answer("POST /device/sign-out", 200, json!({}));
    signed(&mut f);
    f.link.tap(Intent::SignOut);
    assert!(!f.link.signed_in());
    assert!(f.link.packet().forget);
    until(|| !f.site.calls("POST /device/sign-out").is_empty());
    assert_eq!(
        f.site.calls("POST /device/sign-out")[0].token.as_deref(),
        Some("sess_test")
    );
}

#[test]
fn the_choice_is_asked_once_and_phone_chats_upload_screened() {
    let mut f = fixture();
    f.site.answer(
        "PUT /v1/computers/Pixel%20Test/sync",
        200,
        json!({"choice": "all"}),
    );
    f.site.answer(
        "PUT /coder/sessions/phone-c1",
        200,
        json!({"chat": "x", "changed": true}),
    );
    signed(&mut f);
    until(|| f.link.lock().choice_read);
    f.link.act(Action::Show {
        screen: Screen::Account,
        id: None,
    });
    let mut words = vec![];
    texts(f.link.packet().view.as_ref().unwrap(), &mut words);
    assert!(words.iter().any(|w| w == "Sync all my chats"), "{words:?}");
    assert!(
        words.iter().any(|w| w == "Keep chats on this phone"),
        "{words:?}"
    );
    // Nothing uploads before the person chose.
    assert!(f.link.wanted_uploads(&[("c1".into(), 5)]).is_empty());
    f.link.tap(Intent::Choose { all: true });
    until(|| {
        !f.site
            .calls("PUT /v1/computers/Pixel%20Test/sync")
            .is_empty()
    });
    assert_eq!(
        f.site.calls("PUT /v1/computers/Pixel%20Test/sync")[0]
            .body
            .as_ref()
            .unwrap()["choice"],
        "all"
    );
    assert_eq!(
        f.link.wanted_uploads(&[("c1".into(), 5)]),
        vec!["c1".to_owned()]
    );
    let turns = vec![
        openagents_chat::basic_coder::Turn::user("hello"),
        openagents_chat::basic_coder::Turn::assistant(
            "use sk-ant-api03-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            None,
        ),
    ];
    f.link.queue_uploads(vec![(
        "c1".into(),
        upload_title("Greeting"),
        5,
        upload_messages(&turns),
    )]);
    until(|| !f.site.calls("PUT /coder/sessions/phone-c1").is_empty());
    let body = f.site.calls("PUT /coder/sessions/phone-c1")[0]
        .body
        .clone()
        .unwrap();
    assert_eq!(body["computer"], "Pixel Test");
    assert_eq!(body["title"], "Greeting");
    assert_eq!(body["messages"][0]["text"], "hello");
    assert_eq!(body["messages"][1]["text"], crate::account_link::LEFT_OUT);
    until(|| f.link.wanted_uploads(&[("c1".into(), 5)]).is_empty());
    // A later change uploads again.
    assert_eq!(
        f.link.wanted_uploads(&[("c1".into(), 6)]),
        vec!["c1".to_owned()]
    );
}

#[test]
fn every_screen_draws_plain_words() {
    let mut f = fixture();
    f.site.answer(
        "GET /v1/agents",
        200,
        json!({"computers": [{"name": "Studio", "online": false, "updated_unix": 1,
            "items": [{"id": "s1", "kind": "chat", "title": "Fix", "status": "done",
                       "started_unix": 1, "finished_unix": 100, "cost_usd": 1.5}]}]}),
    );
    signed(&mut f);
    until(|| f.link.lock().agents_read && f.link.lock().threads_read);
    for screen in [
        Screen::Account,
        Screen::Chats,
        Screen::Running,
        Screen::Message,
    ] {
        f.link.act(Action::Show {
            screen: screen.clone(),
            id: None,
        });
        let packet = f.link.packet();
        let view = packet.view.unwrap_or_else(|| panic!("{screen:?} draws"));
        let mut words = vec![];
        texts(&view, &mut words);
        for word in words {
            assert!(
                oa_copy::violations(&word, &[]).is_empty(),
                "{screen:?}: {word}: {:?}",
                oa_copy::violations(&word, &[])
            );
        }
    }
}

#[test]
fn every_screen_validates() {
    let mut state = crate::account_link::State::default();
    state.session = Some(Session::for_test(SITE, "sess_x"));
    state.choice_read = true;
    state.threads_read = true;
    state.agents_read = true;
    for screen in [
        Screen::Account,
        Screen::Chats,
        Screen::Running,
        Screen::Message,
    ] {
        state.screen = screen.clone();
        let root = crate::link_view::root(&state, 0);
        let result = rust_native::View::new("openagents.link", 1, root).validate();
        assert!(result.is_ok(), "{screen:?}: {:?}", result.err());
    }
}

#[test]
fn memory_notes_list_edit_delete_and_go_with_chat_turns() {
    let mut f = fixture();
    f.site.answer(
        "GET /coder/memory",
        200,
        json!({"notes": [
            {"id": "mem-1", "scope": "user", "kind": "feedback", "name": "Tabs",
             "description": "Indent with tabs.", "body": "Use tabs.", "updated": 20},
            {"id": "mem-2", "scope": "project", "project": "repo-1", "project_name": "openagents",
             "kind": "project", "name": "Builds", "body": "cargo build", "updated": 10},
            {"id": "mem-3", "scope": "user", "updated": 30, "deleted": true},
        ]}),
    );
    f.site.answer(
        "PUT /coder/memory/mem-1",
        200,
        json!({"note": {"id": "mem-1", "scope": "user", "kind": "feedback", "name": "Tabs",
                        "description": "Indent with tabs.", "body": "Use two spaces.", "updated": 40}}),
    );
    f.site.answer(
        "PUT /coder/memory/new",
        200,
        json!({"note": {"id": "mem-9", "scope": "user", "kind": "user", "name": "Central time",
                        "description": "", "body": "Central time\nI'm in Chicago.", "updated": 50}}),
    );
    f.site
        .answer("DELETE /coder/memory/mem-2", 200, json!({"deleted": true}));
    assert!(f.link.memory_notes().is_empty(), "signed out");
    signed(&mut f);
    until(|| f.link.lock().memory_read);
    // Chat turns carry only the notes that apply everywhere.
    let notes = f.link.memory_notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].name, "Tabs");
    assert_eq!(notes[0].kind, "feedback");
    f.link.act(Action::Show {
        screen: Screen::Memory,
        id: None,
    });
    let packet = f.link.packet();
    let mut words = vec![];
    texts(packet.view.as_ref().unwrap(), &mut words);
    for word in ["Everywhere", "openagents", "Tabs", "Builds", "Add a note"] {
        assert!(words.iter().any(|w| w == word), "{word}: {words:?}");
    }
    // Change what a note says.
    // The website's list as it stands after each change, so a read in
    // between agrees with what the phone shows.
    let tabs = |body: &str, updated: u64| {
        json!({"id": "mem-1", "scope": "user", "kind": "feedback", "name": "Tabs",
               "description": "Indent with tabs.", "body": body, "updated": updated})
    };
    let builds = json!({"id": "mem-2", "scope": "project", "project": "repo-1",
        "project_name": "openagents", "kind": "project", "name": "Builds",
        "body": "cargo build", "updated": 10});
    let central = json!({"id": "mem-9", "scope": "user", "kind": "user", "name": "Central time",
        "description": "", "body": "Central time\nI'm in Chicago.", "updated": 50});
    f.site.answer(
        "GET /coder/memory",
        200,
        json!({"notes": [tabs("Use two spaces.", 40), builds.clone()]}),
    );
    f.link.tap(Intent::EditNote { id: "mem-1".into() });
    assert_eq!(f.link.lock().screen, Screen::Note);
    let token = format!("link-note-{}", f.link.composer);
    f.link.input(&token, "Use two spaces.");
    until(|| !f.site.calls("PUT /coder/memory/mem-1").is_empty());
    let sent = f.site.calls("PUT /coder/memory/mem-1")[0]
        .body
        .clone()
        .unwrap();
    assert_eq!(sent["body"], "Use two spaces.");
    assert_eq!(sent["name"], "Tabs");
    until(|| f.link.lock().screen == Screen::Memory);
    assert_eq!(f.link.memory_notes()[0].body, "Use two spaces.");
    // A new note is named by its first line and applies everywhere.
    f.site.answer(
        "GET /coder/memory",
        200,
        json!({"notes": [central.clone(), tabs("Use two spaces.", 40), builds]}),
    );
    f.link.tap(Intent::NewNote);
    let token = format!("link-note-{}", f.link.composer);
    f.link.input(&token, "Central time\nI'm in Chicago.");
    until(|| !f.site.calls("PUT /coder/memory/new").is_empty());
    let sent = f.site.calls("PUT /coder/memory/new")[0]
        .body
        .clone()
        .unwrap();
    assert_eq!(sent["name"], "Central time");
    assert_eq!(sent["kind"], "user");
    until(|| f.link.memory_notes().len() == 2);
    // A credential never leaves the phone.
    f.link.tap(Intent::NewNote);
    let token = format!("link-note-{}", f.link.composer);
    let key = format!("sk-ant-{}", "a1".repeat(20));
    f.link.input(&token, &format!("my key is {key}"));
    assert_eq!(f.site.calls("PUT /coder/memory/new").len(), 1);
    // Delete removes it here too.
    f.site.answer(
        "GET /coder/memory",
        200,
        json!({"notes": [central, tabs("Use two spaces.", 40)]}),
    );
    f.link.tap(Intent::DeleteNote { id: "mem-2".into() });
    until(|| !f.site.calls("DELETE /coder/memory/mem-2").is_empty());
    until(|| f.link.lock().memory.iter().all(|note| note.id != "mem-2"));
}

/// A reply to a web chat carries photos (#11174): each uploads first, and
/// the reply names them by the ids the website gave them.
#[test]
fn a_reply_to_a_web_chat_carries_its_photos() {
    let mut f = fixture();
    f.site.answer(
        "GET /v1/threads/w1",
        200,
        json!({
            "thread": {"id": "w1", "title": "Plans", "surface": "web",
                       "online": true, "can_reply": true},
            "messages": [{"role": "user", "text": "hi"}, {"role": "assistant", "text": "Hello."}],
            "earlier": 0, "waiting": 0,
        }),
    );
    f.site.answer(
        "POST /v1/threads/w1/files?name=Photo.png",
        201,
        json!({"id": "a".repeat(32), "name": "Photo.png", "kind": "png", "size": "1 KB"}),
    );
    f.site.answer(
        "POST /v1/threads/w1/messages",
        202,
        json!({"answering": true}),
    );
    signed(&mut f);
    f.link.act(Action::Show {
        screen: Screen::Chat,
        id: Some("w1".into()),
    });
    until(|| f.link.lock().open.as_ref().is_some_and(|o| o.loaded));
    // Add photo asks the host for its picker once.
    f.link.tap(Intent::AddPhoto);
    assert!(f.link.take_pick());
    assert!(!f.link.take_pick());
    let png = openagents_chat_app::attachments::Image::pixels(3, 2, vec![200; 24]).unwrap();
    f.link.attach("Photo.png", png.bytes.as_ref().clone());
    f.link.attach("bad.png", b"not an image".to_vec());
    assert_eq!(f.link.lock().open.as_ref().unwrap().photos.len(), 1);
    assert!(f.link.lock().notice.is_some());
    let token = format!("link-reply-w1-{}", f.link.composer);
    f.link.input(&token, "what is in this?");
    until(|| !f.site.calls("POST /v1/threads/w1/messages").is_empty());
    let uploads = f.site.calls("POST /v1/threads/w1/files?name=Photo.png");
    assert_eq!(uploads.len(), 1);
    assert_eq!(uploads[0].token.as_deref(), Some("sess_test"));
    let sent = &f.site.calls("POST /v1/threads/w1/messages")[0];
    assert_eq!(
        sent.body.as_ref().unwrap()["files"],
        json!(["a".repeat(32)])
    );
    until(|| {
        f.link
            .lock()
            .open
            .as_ref()
            .is_some_and(|o| o.photos.is_empty() && !o.sending)
    });
}
