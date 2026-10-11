use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime},
};

use coder_new::{
    App, Mode,
    live::{Chat, Entry, Update},
    programmatic,
    sessions::{self, Store},
    trajectory, ui,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::{Value, json};

fn key(app: &mut App, key: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(key, KeyModifiers::NONE))));
}

fn paste(app: &mut App, text: &str) {
    assert!(app.handle(Event::Paste(text.into())));
}

fn new_app(store: &Store, cwd: &Path) -> App {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.cwd = Some(cwd.to_owned());
    app.attach_session_store(store.clone());
    app
}

fn seed(store: &Store, id: &str, title: &str, cwd: &Path, age: u64) -> Value {
    let mut chat = Chat::default();
    chat.entries = vec![
        Entry::User(title.into()),
        Entry::Assistant {
            elapsed_ms: None,
            text: "**Saved reply**".into(),
            model: Some("openai/example:low".into()),
        },
    ];
    let document = trajectory::document(&chat, id, "openai/example:low", cwd);
    store.save(id, &document).unwrap();
    fs::File::open(store.path(id).unwrap())
        .unwrap()
        .set_times(
            fs::FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(age)),
        )
        .unwrap();
    document
}

fn render(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn picker_restores_saved_chat_and_continues_the_same_session() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::under(temp.path().join("config"));
    seed(&store, "older", "Older conversation", temp.path(), 1);
    seed(&store, "recent", "Recent conversation", temp.path(), 2);
    let mut app = new_app(&store, temp.path());
    paste(&mut app, "/resume");
    key(&mut app, KeyCode::Enter);
    assert!(app.draft.text.is_empty());
    assert_eq!(app.resume_picker.as_ref().unwrap().sessions[0].id, "recent");
    let screen = render(&mut app, 110, 24);
    assert!(screen.contains("Resume · recent conversations"));
    assert!(screen.contains("1. Recent conversation"));
    assert!(screen.contains("2 entries"));
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.session_id(), Some("older"));
    assert!(app.request.is_none());
    assert!(!app.live.busy);
    assert!(
        matches!(&app.live.entries[1], Entry::Assistant { model: Some(model), .. } if model == "openai/example:low")
    );
    app.submit("Continue this conversation", temp.path());
    assert!(app.request.is_some());
    app.cancel_request();
    assert!(app.persist_session(true));
    let retained = sessions::read_document(&store.path("older").unwrap()).unwrap();
    assert_eq!(retained["session_id"], "older");
    assert_eq!(retained["trajectory_id"], "older");
    assert_eq!(retained["steps"].as_array().unwrap().len(), 3);
    assert_eq!(store.list().unwrap().len(), 2);
    assert!(
        store.lease("older").is_err(),
        "The terminal holds its writer lease"
    );
    let context = programmatic::Context {
        root: temp.path().join("config"),
        cwd: temp.path().into(),
        environment: Default::default(),
        input: None,
        canceled: None,
        approvals: None,
    };
    let read = programmatic::execute(
        &["sessions".into(), "read".into(), "older".into()],
        &context,
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(read["steps"].as_array().unwrap().len(), 3);
    let exported =
        programmatic::execute(&["export".into(), "older".into()], &context, &mut |_| {}).unwrap();
    assert_eq!(exported["session_id"], "older");
    assert!(
        programmatic::execute(
            &["sessions".into(), "delete".into(), "older".into()],
            &context,
            &mut |_| {}
        )
        .is_err()
    );
}

#[test]
fn numbers_use_the_last_picker_snapshot_and_ids_work_without_a_picker() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::under(temp.path().join("config"));
    seed(&store, "first", "First", temp.path(), 2);
    seed(&store, "second", "Second", temp.path(), 1);
    let mut app = new_app(&store, temp.path());
    assert!(!app.resume(Some("1")));
    assert!(app.resume(None));
    key(&mut app, KeyCode::Esc);
    seed(&store, "newer", "Added after picker closed", temp.path(), 3);
    paste(&mut app, "/resume 2");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.session_id(), Some("second"));
    assert!(app.resume(Some("first")));
    assert_eq!(app.session_id(), Some("first"));
    assert!(app.resume(Some("first")));
    assert!(!app.resume(Some("0")));
    assert!(!app.resume(Some("../unsafe")));
    assert_eq!(app.session_id(), Some("first"));
}

#[test]
fn cancel_errors_and_busy_work_preserve_the_current_chat_and_draft() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::under(temp.path().join("config"));
    seed(&store, "saved", "Saved chat", temp.path(), 1);
    let mut app = new_app(&store, temp.path());
    app.submit("Current chat", temp.path());
    app.cancel_request();
    assert!(app.persist_session(true));
    let id = app.session_id().unwrap().to_owned();
    paste(&mut app, "Unsent draft");
    let entries = app.live.entries.clone();
    assert!(app.resume(None));
    app.handle(Event::Paste("Ignored paste".into()));
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.draft.text, "Unsent draft");
    assert!(app.live.entries == entries);
    assert!(!app.resume(Some("missing")));
    assert_eq!(app.session_id(), Some(id.as_str()));
    assert_eq!(app.draft.text, "Unsent draft");
    assert!(app.live.entries == entries);
    app.submit("Still running", temp.path());
    let request_id = app.request_id;
    paste(&mut app, "/resume");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.draft.text, "/resume");
    assert_eq!(app.request_id, request_id);
    assert!(app.live.busy && app.request.is_some());
    assert!(app.resume_picker.is_none());
}

#[test]
fn auto_save_checkpoints_partial_text_and_never_writes_for_typing_or_scrolling() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::under(temp.path().join("config"));
    let mut app = new_app(&store, temp.path());
    assert!(app.persist_session(false));
    assert!(store.list().unwrap().is_empty());
    app.submit("First prompt", temp.path());
    assert!(app.persist_session(false));
    let path = store.path(app.session_id().unwrap()).unwrap();
    let initial = fs::read(&path).unwrap();
    app.apply_update(Update::Delta {
        id: app.request_id,
        text: "Partial reply".into(),
    });
    app.elapsed_seconds = 4;
    assert!(app.persist_session(false));
    assert_eq!(fs::read(&path).unwrap(), initial);
    app.elapsed_seconds = 5;
    assert!(app.persist_session(false));
    let retained = sessions::read_document(&path).unwrap();
    assert_eq!(retained["steps"][1]["message"], "Partial reply");
    app.cancel_request();
    assert!(app.persist_session(false), "Cancellation saves immediately");
    let idle = fs::read(&path).unwrap();
    key(&mut app, KeyCode::Char('x'));
    app.handle(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 1,
        row: 2,
        modifiers: KeyModifiers::NONE,
    }));
    app.elapsed_seconds = 20;
    app.tick();
    assert!(app.persist_session(false));
    assert_eq!(fs::read(&path).unwrap(), idle);
    app.set_mode(Mode::Demo);
    paste(&mut app, "Demo only");
    key(&mut app, KeyCode::Enter);
    assert!(app.persist_session(true));
    assert_eq!(fs::read(&path).unwrap(), idle);
    drop(app);
    let mut restored = new_app(&store, temp.path());
    assert!(restored.resume(Some(retained["session_id"].as_str().unwrap())));
    assert!(
        matches!(&restored.live.entries[1], Entry::Assistant {text,..} if text == "Partial reply")
    );
    assert!(!restored.live.busy);
}

#[test]
fn restores_all_children_as_idle_history_and_keeps_current_settings_and_directory() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::under(temp.path().join("config"));
    let saved_cwd = temp.path().join("old-workspace");
    let mut document = seed(&store, "parent", "Parent task", &saved_cwd, 1);
    let mut child = Chat::default();
    child.entries = vec![
        Entry::User("Child task".into()),
        Entry::Tool {
            name: "Run".into(),
            input: json!({"command":"never replay me"}),
            output: Value::Null,
            running: true,
        },
    ];
    child.partial = "In-progress child reply".into();
    child.busy = true;
    let mut child = trajectory::document(&child, "child-id", "model:low", &saved_cwd);
    child["extra"]["agent"] = json!("microcoder");
    child["extra"]["task"] = json!("Child task");
    document["subagent_trajectories"] = json!([child]);
    store.save("parent", &document).unwrap();
    let mut app = new_app(&store, temp.path());
    app.plugins.model = "chosen/model".into();
    app.branch = Some("current-branch".into());
    assert!(app.resume(Some("parent")));
    assert_eq!(app.cwd.as_deref(), Some(temp.path()));
    assert_eq!(app.plugins.model, "chosen/model");
    assert_eq!(app.branch.as_deref(), Some("current-branch"));
    assert!(app.notice.as_deref().unwrap().contains("continuing in"));
    assert_eq!(app.delegations.len(), 1);
    let child = &app.delegations[0];
    assert!(!child.running && !child.chat.busy);
    assert!(matches!(
        child.chat.entries[1],
        Entry::Tool { running: false, .. }
    ));
    assert!(app.request.is_none());
    app.submit("Continue parent", temp.path());
    app.cancel_request();
    key(&mut app, KeyCode::Down);
    assert_eq!(app.selected_agent, Some(0));
    assert!(app.persist_session(true));
    let retained = sessions::read_document(&store.path("parent").unwrap()).unwrap();
    assert_eq!(retained["session_id"], "parent");
    assert_eq!(
        retained["subagent_trajectories"][0]["session_id"],
        "child-id"
    );
    assert_eq!(retained["steps"][2]["message"], "Continue parent");
}

#[test]
fn invalid_child_does_not_replace_existing_state_and_storage_failure_preserves_input() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::under(temp.path().join("config"));
    let mut invalid = seed(&store, "invalid-child", "Invalid child", temp.path(), 1);
    invalid["subagent_trajectories"] = json!([{"schema_version":"bad"}]);
    fs::write(
        store.path("invalid-child").unwrap(),
        serde_json::to_vec(&invalid).unwrap(),
    )
    .unwrap();
    let mut app = new_app(&store, temp.path());
    app.live.entries.push(Entry::User("Keep me".into()));
    paste(&mut app, "Keep this draft");
    assert!(!app.resume(Some("invalid-child")));
    assert!(matches!(&app.live.entries[0], Entry::User(text) if text == "Keep me"));
    assert_eq!(app.draft.text, "Keep this draft");
    assert!(app.session_id().is_none());
    let blocked = temp.path().join("blocked");
    fs::write(&blocked, "not a directory").unwrap();
    app.attach_session_store(Store::under(blocked));
    app.submit_live();
    assert_eq!(app.draft.text, "Keep this draft");
    assert!(!app.live.busy);
    assert!(app.request.is_none());
}

#[test]
fn empty_picker_and_keyboard_paging_fit_small_terminals() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::under(temp.path().join("config"));
    let mut app = new_app(&store, temp.path());
    assert!(app.resume(None));
    assert!(render(&mut app, 60, 18).contains("No conversations to resume."));
    key(&mut app, KeyCode::Enter);
    assert!(app.resume_picker.is_some());
    key(&mut app, KeyCode::Esc);
    for n in 0..20 {
        seed(
            &store,
            &format!("session-{n}"),
            &format!("Task {n}"),
            temp.path(),
            n + 1,
        );
    }
    assert!(app.resume(None));
    render(&mut app, 30, 12);
    let page = app.resume_picker.as_ref().unwrap().page;
    key(&mut app, KeyCode::PageDown);
    assert_eq!(app.resume_picker.as_ref().unwrap().selected, page);
    key(&mut app, KeyCode::End);
    let rendered = render(&mut app, 60, 18);
    assert!(rendered.contains("20. Task 0"));
    key(&mut app, KeyCode::Char('k'));
    assert_eq!(app.resume_picker.as_ref().unwrap().selected, 18);
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.resume_picker.as_ref().unwrap().selected, 1);
    render(&mut app, 12, 5);
}

#[test]
fn auto_saved_and_exported_transcripts_redact_configured_keys_in_every_chat() {
    use coder_new::{bundled_runtime::RuntimeEvent, credentials::Imported};
    let temp = tempfile::tempdir().unwrap();
    let store = Store::under(temp.path().join("config"));
    let mut app = new_app(&store, temp.path());
    app.plugins.bootstrap_credentials(Imported {
        openrouter_key: Some(model_access::ApiKey::new("resume-redaction-router")),
        jev_key: Some(model_access::ApiKey::new("resume-redaction-jev")),
        ..Default::default()
    });
    app.submit("Do not retain resume-redaction-router", temp.path());
    app.apply_update(Update::Delegation {
        id: app.request_id,
        delegation: "child".into(),
        name: "microcoder".into(),
        task: "Do not retain resume-redaction-jev".into(),
        event: RuntimeEvent::Tool {
            name: "Run".into(),
            input: json!({"input":"resume-redaction-router"}),
            output: json!({"output":"resume-redaction-jev"}),
            running: false,
        },
    });
    app.apply_update(Update::Delta {
        id: app.request_id,
        text: "Do not retain resume-redaction-jev".into(),
    });
    app.cancel_request();
    assert!(app.persist_session(true));
    let path = store.path(app.session_id().unwrap()).unwrap();
    let saved = fs::read_to_string(path).unwrap();
    assert!(!saved.contains("resume-redaction-router"));
    assert!(!saved.contains("resume-redaction-jev"));
    assert!(saved.contains("[redacted]"));
    key(&mut app, KeyCode::Down);
    assert_eq!(app.selected_agent, Some(0));
    let exported = trajectory::app_document(&app, temp.path()).to_string();
    assert!(!exported.contains("resume-redaction-router"));
    assert!(!exported.contains("resume-redaction-jev"));
    assert!(atif::validate(&serde_json::from_str::<Value>(&saved).unwrap()).is_empty());
}

#[test]
fn failed_auto_save_prevents_switching_and_retries_after_storage_recovers() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("config");
    let store = Store::under(&root);
    seed(&store, "other", "Other conversation", temp.path(), 1);
    let mut app = new_app(&store, temp.path());
    app.submit("Current conversation", temp.path());
    assert!(app.persist_session(true));
    let id = app.session_id().unwrap().to_owned();
    app.apply_update(Update::Delta {
        id: app.request_id,
        text: "New partial text".into(),
    });
    app.cancel_request();
    paste(&mut app, "Unsent text");
    let displaced = temp.path().join("displaced-sessions");
    fs::rename(root.join("sessions"), &displaced).unwrap();
    fs::write(root.join("sessions"), "not a directory").unwrap();
    assert!(!app.persist_session(true));
    assert!(!app.resume(Some("other")));
    assert_eq!(app.session_id(), Some(id.as_str()));
    assert_eq!(app.draft.text, "Unsent text");
    assert!(
        matches!(&app.live.entries[1], Entry::Assistant {text,..} if text == "New partial text")
    );
    fs::remove_file(root.join("sessions")).unwrap();
    fs::rename(&displaced, root.join("sessions")).unwrap();
    assert!(app.persist_session(true));
    let saved = sessions::read_document(&store.path(&id).unwrap()).unwrap();
    assert_eq!(saved["steps"][1]["message"], "New partial text");
    assert!(app.resume(Some("other")));
}

/// An agent's pane follows the session its agent holds, reloads it as the
/// agent saves, and takes it over once a key asked and the agent let go.
#[test]
fn following_reloads_the_held_session_and_a_key_takes_it_over() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::under(dir.path().join("state"));
    let held = store.lease("agent-alice").unwrap();
    let mut chat = Chat::default();
    chat.entries = vec![Entry::User("run the atif tests".into())];
    let mut document = trajectory::document(&chat, "agent-alice", "openai/example:low", dir.path());
    held.save(&document).unwrap();

    let mut app = new_app(&store, dir.path());
    assert!(app.follow("agent-alice"));
    assert!(app.following());
    assert_eq!(app.live.entries.len(), 1);
    assert!(app.session_id().is_none(), "the agent holds it");

    // The agent saves more; the pane shows it without a key.
    chat.entries.push(Entry::Assistant {
        elapsed_ms: None,
        text: "atif: 31 passed.".into(),
        model: Some("openai/example:low".into()),
    });
    document = trajectory::document(&chat, "agent-alice", "openai/example:low", dir.path());
    std::thread::sleep(Duration::from_millis(20));
    held.save(&document).unwrap();
    fs::File::open(store.path("agent-alice").unwrap())
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(5)))
        .unwrap();
    app.follow_tick();
    assert_eq!(app.live.entries.len(), 2);

    // A key asks; the session stays the agent's until it lets go.
    key(&mut app, KeyCode::Char('x'));
    assert!(app.following());
    assert!(app.notice.as_deref().unwrap().starts_with("Taking over"));
    drop(held);
    app.follow_tick();
    assert!(!app.following());
    assert_eq!(app.session_id(), Some("agent-alice"));
    assert_eq!(app.live.entries.len(), 2);
    // Typing now goes to the composer.
    key(&mut app, KeyCode::Char('o'));
    assert_eq!(app.draft.text, "o");
}

/// The owner asked the agent for something while their pane held her
/// session: the pane hands it back, keeps the unsent draft, and follows
/// her new turn. Mid-reply, it says it is busy and keeps the session.
#[test]
fn a_held_session_goes_back_to_its_agent_when_she_asks() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::under(dir.path().join("state"));
    let mut chat = Chat::default();
    chat.entries = vec![Entry::User("what are you doing".into())];
    let document = trajectory::document(&chat, "agent-alice", "openai/example:low", dir.path());
    store.lease("agent-alice").unwrap().save(&document).unwrap();

    let mut app = new_app(&store, dir.path());
    assert!(app.follow("agent-alice"));
    key(&mut app, KeyCode::Char('x'));
    app.follow_tick();
    assert_eq!(app.session_id(), Some("agent-alice"));
    paste(&mut app, "half a thought");
    assert!(store.lease("agent-alice").is_err(), "the pane holds it");

    let marker = dir.path().join("state/sessions/agent-alice.atif.reclaim");
    fs::write(&marker, "reclaim\n").unwrap();
    app.live.busy = true;
    app.follow_tick();
    assert_eq!(fs::read_to_string(&marker).unwrap().trim(), "busy");
    assert_eq!(app.session_id(), Some("agent-alice"));

    app.live.busy = false;
    app.follow_tick();
    assert!(app.following());
    assert!(app.session_id().is_none());
    assert!(!marker.exists());
    assert_eq!(app.draft.text, "half a thought");
    assert!(store.lease("agent-alice").is_ok(), "the agent can take it");
}

#[test]
fn resume_opens_a_chat_held_by_another_terminal_and_takes_it_over() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::under(temp.path().join("config"));
    seed(&store, "other", "Other chat", temp.path(), 1);
    let mut owner = new_app(&store, temp.path());
    assert!(owner.resume(Some("other")));
    let mut app = new_app(&store, temp.path());
    app.submit("Current chat", temp.path());
    app.cancel_request();
    let previous = app.session_id().unwrap().to_owned();
    assert!(app.resume(None));
    let index = app
        .resume_picker
        .as_ref()
        .unwrap()
        .sessions
        .iter()
        .position(|session| session.id == "other")
        .unwrap();
    app.resume_picker.as_mut().unwrap().selected = index;
    key(&mut app, KeyCode::Enter);
    assert!(app.resume_picker.is_none());
    assert!(app.following());
    assert!(matches!(&app.live.entries[0], Entry::User(text) if text == "Other chat"));
    assert!(!app.notice.as_deref().unwrap().contains("Another process"));
    assert!(store.lease(&previous).is_ok());
    assert_eq!(
        store.read(&previous).unwrap()["steps"][0]["message"],
        "Current chat"
    );
    owner.follow_tick();
    app.follow_tick();
    assert_eq!(app.session_id(), Some("other"));
    assert!(!app.following());
    paste(&mut app, "Continue here");
    assert_eq!(app.draft.text, "Continue here");
    assert!(owner.following());
    assert!(store.lease("other").is_err());
}

#[test]
fn resume_shows_locked_chat_updates_until_its_writer_finishes() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::under(temp.path().join("config"));
    seed(&store, "held", "Held chat", temp.path(), 1);
    let lease = store.lease("held").unwrap();
    let mut app = new_app(&store, temp.path());
    assert!(app.resume(Some("held")));
    assert!(app.following());
    app.follow_tick();
    let mut document = lease.read().unwrap();
    document["steps"][1]["message"] = json!("Newest reply");
    lease.save(&document).unwrap();
    app.follow_tick();
    assert!(matches!(&app.live.entries[1], Entry::Assistant {text, ..} if text == "Newest reply"));
    drop(lease);
    app.follow_tick();
    assert_eq!(app.session_id(), Some("held"));
    assert!(!app.following());
}
