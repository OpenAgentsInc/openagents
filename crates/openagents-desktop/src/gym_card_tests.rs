//! The Gym and its tests through the desktop chat (#10020): a reply that
//! offers to run a test set, and a draft test set's sheet, mount the
//! phone's own cards and sheets from the shared Gym state, and a click
//! goes through the phone's own `Gym::tap`. Set
//! `OPENAGENTS_GYM_CAPTURE_DIR` to write each step as a PNG.

use super::*;
use openagents_chat::{
    basic_coder::Turn,
    router::{Meta, Offer, Screen},
    service::{Command, Snapshot},
};
use openagents_desktop::chat::TRANSCRIPT;
use rust_native::{Element, Node};
use rust_native_desktop::input::SurfaceInput;
use std::sync::Arc;

fn fixture(name: &str) -> serde_json::Value {
    let text = match name {
        "tool" => include_str!("../../coder/fixtures/nip-cj/router-card-tool.json"),
        "draft" => include_str!("../../coder/fixtures/nip-cj/router-card-draft.json"),
        "start" => include_str!("../../coder/fixtures/nip-cj/router-offer-start-eval.json"),
        other => panic!("no fixture {other}"),
    };
    serde_json::from_str(text).unwrap()
}

/// A desktop chat whose reply is `reply` with `meta`.
fn chat_with(reply: &str, meta: Meta) -> DesktopApp {
    let mut app = super::tests::chat_fixture(0).0;
    let panel = app.chat.as_mut().unwrap();
    let Request::Chat {
        ticket,
        command: Command::Read { chat, .. },
    } = panel
        .tick(Instant::now() + std::time::Duration::from_secs(2))
        .unwrap()
    else {
        panic!("read")
    };
    panel.outcome(
        ticket,
        Ok(Snapshot {
            chat: Some(chat),
            total: 2,
            turns: vec![Turn::user("Ask"), Turn::assistant(reply, Some(meta))],
            ..Snapshot::default()
        }),
    );
    app.present();
    app
}

/// Every button in the transcript: its key and label.
fn buttons(app: &DesktopApp) -> Vec<(String, String)> {
    fn walk(node: &Node<()>, out: &mut Vec<(String, String)>) {
        match &node.element {
            Element::Button { label, .. } => out.push((node.key.clone(), label.clone())),
            Element::Stack { children, .. } => children.iter().for_each(|c| walk(c, out)),
            _ => {}
        }
    }
    let mut out = vec![];
    for row in app.chat.as_ref().unwrap().transcript_rows() {
        walk(Arc::as_ref(row), &mut out);
    }
    out
}

fn text(app: &DesktopApp) -> String {
    serde_json::to_string(
        &app.chat
            .as_ref()
            .unwrap()
            .transcript_rows()
            .iter()
            .map(|row| Arc::as_ref(row).clone())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn key_of(app: &DesktopApp, label: &str) -> String {
    buttons(app)
        .into_iter()
        .find(|(_, shown)| shown == label)
        .unwrap_or_else(|| panic!("no {label} in {:?}", buttons(app)))
        .0
}

/// Paint, then click the transcript button labelled `label` where it
/// painted, and carry out what it asks: the request it sends, if any.
fn click(app: &mut DesktopApp, label: &str) -> Option<Request> {
    rust_native_desktop::capture(app, 1200.0, 840.0, 1.0);
    let key = key_of(app, label);
    let now = Instant::now();
    let panel = app.chat.as_mut().unwrap();
    let bounds = panel
        .transcript
        .control_bounds(&key)
        .unwrap_or_else(|| panic!("{label} is painted"));
    let (x, y) = (bounds.x + bounds.w / 2.0, bounds.y + bounds.h / 2.0);
    assert!(panel.surface(TRANSCRIPT, SurfaceInput::Down { x, y, shift: false }, now));
    panel.surface(TRANSCRIPT, SurfaceInput::Up { x, y }, now);
    assert_eq!(panel.take_activated(), vec![key.clone()]);
    let view = app.view().clone();
    let request = app.chat.as_mut().unwrap().action(
        openagents_desktop::chat_action::Action::Card { key },
        &view,
        now,
    );
    app.present();
    request
}

fn capture(app: &mut DesktopApp, name: &str) {
    let (frame, scene) = rust_native_desktop::capture(app, 1200.0, 840.0, 2.0);
    assert!(scene.unsupported.is_empty(), "{:?}", scene.unsupported);
    if let Some(directory) = std::env::var_os("OPENAGENTS_GYM_CAPTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{name}.png")), frame.png().unwrap()).unwrap();
    }
}

/// "Test Project map on Coder": the tool card with START THE TEST; with no
/// test computers in this build and no computer ready, the click leaves the
/// phone's run card saying why, whose Connect a computer opens Computers.
#[test]
fn running_a_test_set_from_chat() {
    let mut meta = Meta::default();
    meta.carded(&fixture("tool"));
    meta.offers.extend(Offer::parse(&fixture("start")));
    let mut app = chat_with("We'd try Project map.", meta);
    capture(&mut app, "dsk-gym-01-tool-card");
    let shown = text(&app);
    assert!(shown.contains("PROJECT MAP"));
    assert!(key_of(&app, "START THE TEST").ends_with(".start"));

    assert!(click(&mut app, "START THE TEST").is_none());
    capture(&mut app, "dsk-gym-02-run-card");
    let shown = text(&app);
    assert!(shown.contains(
        "Our computers can't take these tests right now. Connect a computer and we'll run them there with Coder."
    ));
    assert!(
        !shown.contains("START THE TEST"),
        "the run replaces the offer"
    );
    assert!(click(&mut app, "Connect a computer").is_none());
    assert_eq!(
        app.chat.as_mut().unwrap().take_navigation(),
        Some(Screen::Computers)
    );
}

/// A draft test set: See every test opens the phone's `SCR-21` sheet under
/// the cards, Close closes it, and Looks good sends the approval.
#[test]
fn a_draft_test_sets_sheet() {
    let mut meta = Meta::default();
    meta.carded(&fixture("draft"));
    let mut app = chat_with(
        "Here are the tests.\n\nAre these the right tests? Tap Looks good, or tell us what to change.",
        meta,
    );
    capture(&mut app, "dsk-gym-03-draft-card");
    click(&mut app, "See every test");
    capture(&mut app, "dsk-gym-04-test-set-sheet");
    let shown = text(&app);
    assert!(shown.contains("TIDY IMPORTS · 1 TESTS"), "{shown}");
    assert!(shown.contains("Checked: Sorted."));
    assert!(buttons(&app).iter().any(|(key, _)| key == "sheet.close"));
    click(&mut app, "Close");
    assert!(!text(&app).contains("TIDY IMPORTS · 1 TESTS"));
    click(&mut app, "See every test");
    let sent = click(&mut app, "LOOKS GOOD");
    assert!(
        matches!(&sent, Some(Request::Chat { command: Command::Send { text, .. }, .. }) if text == "Looks good"),
        "{sent:?}"
    );
    assert!(
        !text(&app).contains("TIDY IMPORTS · 1 TESTS"),
        "the sheet closes"
    );
}

/// A stand-in for the hosted runner (never the live one): it answers a
/// request as the runner would, queued and then, when `finish`, the
/// recorded report, and records what it was asked to start and follow.
#[cfg(not(windows))]
#[derive(Default)]
struct StandIn {
    finish: bool,
    started: std::sync::Mutex<Vec<openagents_chat_app::gym::HostedRun>>,
    resumed: std::sync::Mutex<Vec<serde_json::Value>>,
}

#[cfg(not(windows))]
impl openagents_chat_app::gym::Hosted for StandIn {
    fn start(
        &self,
        _world: secp256k1::SecretKey,
        run: openagents_chat_app::gym::HostedRun,
        live: Arc<std::sync::Mutex<openagents_chat_app::gym::Live>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        self.started.lock().unwrap().push(run);
        let finish = self.finish;
        Box::pin(async move {
            let mut live = live.lock().unwrap();
            live.request = Some("ab".repeat(32));
            live.event = Some(serde_json::json!({"id": "ab".repeat(32)}));
            live.queued = true;
            if finish {
                live.outcome = Some(Ok(openagents_chat_app::gym::Outcome::from_report(
                    include_bytes!("../../openagents-mobile/fixtures/gym-report.json"),
                )
                .unwrap()));
            }
        })
    }
    fn resume(
        &self,
        _world: secp256k1::SecretKey,
        event: serde_json::Value,
        _live: Arc<std::sync::Mutex<openagents_chat_app::gym::Live>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        self.resumed.lock().unwrap().push(event);
        Box::pin(async {})
    }
    fn stop(
        &self,
        _world: secp256k1::SecretKey,
        _event: serde_json::Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        Box::pin(async {})
    }
    fn publish(
        &self,
        _world: secp256k1::SecretKey,
        _request: String,
        _report: serde_json::Value,
        _live: Arc<std::sync::Mutex<openagents_chat_app::gym::Live>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        Box::pin(async {})
    }
}

/// The tool card with START THE TEST, on a desktop whose chat Gym has the
/// hosted runner (`runner`), a trainer key, and its store in `home`.
#[cfg(not(windows))]
fn hosted_chat(
    home: &std::path::Path,
    runner: Arc<StandIn>,
    runtime: &tokio::runtime::Runtime,
) -> DesktopApp {
    let mut meta = Meta::default();
    meta.carded(&fixture("tool"));
    meta.offers.extend(Offer::parse(&fixture("start")));
    let mut app = chat_with("We'd try Project map.", meta);
    let world = secp256k1::SecretKey::from_byte_array([0x42; 32]).unwrap();
    app.chat
        .as_mut()
        .unwrap()
        .use_gym(openagents_desktop::chat_gym::gym(
            home,
            world,
            runner,
            runtime.handle().clone(),
        ));
    app.present();
    app
}

/// Tick the window until `done` holds, as its wakes would.
#[cfg(not(windows))]
fn until(app: &mut DesktopApp, done: impl Fn(&openagents_chat_app::gym::Gym) -> bool) {
    let deadline = Instant::now() + std::time::Duration::from_secs(10);
    while !done(app.chat.as_ref().unwrap().gym()) {
        assert!(Instant::now() < deadline, "the Gym never settled");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _ = app.chat.as_mut().unwrap().tick(Instant::now());
    }
    app.present();
}

/// With no computer ready, START THE TEST runs on the hosted runner, as
/// it does on the phone, and its result is in the chat; the result is
/// kept, so a reopened desktop still has it and follows nothing again.
#[cfg(not(windows))]
#[test]
fn with_no_computer_a_test_runs_hosted_and_its_result_survives_a_reopen() {
    let home = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let runner = Arc::new(StandIn {
        finish: true,
        ..StandIn::default()
    });
    let mut app = hosted_chat(home.path(), runner.clone(), &runtime);
    assert!(click(&mut app, "START THE TEST").is_none());
    until(&mut app, |gym| gym.latest_result().is_some());
    capture(&mut app, "dsk-gym-05-hosted-result");
    let shown = text(&app);
    assert!(shown.contains("CODER GOT BETTER"), "{shown}");
    assert!(shown.contains("with Project map: 7 of 8"), "{shown}");
    assert!(!shown.contains("Connect a computer"), "{shown}");
    assert!(!shown.contains("can't take these tests"), "{shown}");
    let started = runner.started.lock().unwrap().clone();
    assert_eq!(started.len(), 1, "one request to the hosted runner");
    let offer = fixture("start");
    assert_eq!(started[0].offer["suite"], offer["suite"]);
    assert_eq!(started[0].offer["subject"], offer["subject"]);
    assert_eq!(started[0].runs, 3);
    let result = app
        .chat
        .as_ref()
        .unwrap()
        .gym()
        .latest_result()
        .unwrap()
        .clone();
    assert!(matches!(
        result.place,
        Some(openagents_chat_app::gym::Place::Hosted {
            request: Some(_),
            ..
        })
    ));
    assert!(result.outcome.is_some());
    drop(app);

    let again = Arc::new(StandIn::default());
    let app = hosted_chat(home.path(), again.clone(), &runtime);
    let kept = app.chat.as_ref().unwrap().gym().latest_result().cloned();
    assert_eq!(kept, Some(result), "the run and its result are kept");
    assert!(again.resumed.lock().unwrap().is_empty());
    assert!(again.started.lock().unwrap().is_empty());
}

/// A hosted run still going when the desktop closes is followed again
/// when it reopens, by the same signed request.
#[cfg(not(windows))]
#[test]
fn a_hosted_run_still_going_is_followed_again_after_a_reopen() {
    let home = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut app = hosted_chat(home.path(), Arc::new(StandIn::default()), &runtime);
    assert!(click(&mut app, "START THE TEST").is_none());
    until(&mut app, |gym| {
        gym.active().is_some_and(|run| {
            matches!(
                run.place,
                Some(openagents_chat_app::gym::Place::Hosted {
                    request: Some(_),
                    ..
                })
            )
        })
    });
    drop(app);

    let again = Arc::new(StandIn::default());
    let app = hosted_chat(home.path(), again.clone(), &runtime);
    assert!(app.chat.as_ref().unwrap().gym().active().is_some());
    assert_eq!(
        *again.resumed.lock().unwrap(),
        [serde_json::json!({"id": "ab".repeat(32)})]
    );
}
