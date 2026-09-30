//! Screen snapshots and the rules every screen keeps.
//!
//! Each screen state is built from fixed data and its view outlined as
//! text under `snapshots/<name>.txt`: what it says and what each control
//! does. `cargo test -p openagents-desktop` fails on any difference. To
//! record a change on purpose:
//!
//! ```sh
//! UPDATE_SNAPSHOTS=1 cargo test -p openagents-desktop
//! git diff crates/openagents-desktop/snapshots
//! ```

use crate::codes::{Action, Conditions};
use crate::control::{Autostart, Device, Project, Status};
use crate::model::{Agent, Agents, Intent, Model, OldSetup, Refreshed, Screen, Task};
use crate::screens::{Presenter, outline, root, words};
use crate::words::banned_in;
use rust_native::Node;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// "Now", in Unix seconds, for "last seen".
const NOW: u64 = 1_790_000_000;

fn status(online: bool) -> Status {
    Status {
        host: "a".repeat(64),
        endpoint: "b".repeat(64),
        label: "Studio Mac".into(),
        online,
        relay: None,
        devices: 2,
        outstanding_invitations: 0,
        version: "test".into(),
    }
}

fn phone(id: char, label: &str, terminal: bool, seen: u64) -> Device {
    let mut rights = vec!["observe".to_string(), "operate".to_string()];
    if terminal {
        rights.push("terminal".into());
    }
    Device {
        device: id.to_string().repeat(64),
        label: label.into(),
        rights,
        grant: "g".repeat(64),
        epoch: 0,
        enrolled_at: NOW - 3_600,
        last_seen: Some(NOW - seen),
        revoked: false,
    }
}

fn host(devices: Vec<Device>, project: bool, autostart: bool) -> Refreshed {
    let projects = if project {
        vec![Project {
            label: "website".into(),
            path: "/Users/kai/.openagents/host/projects/website-1a2b3c4d".into(),
            folder: Some("/Users/kai/code/website".into()),
        }]
    } else {
        vec![]
    };
    Refreshed {
        status: status(true),
        devices,
        autostart: Autostart {
            enabled: autostart,
            projects: projects.iter().map(|p| p.label.clone()).collect(),
            max_running: 1,
        },
        projects,
        nearby: None,
    }
}

/// A model on `screen`, in the Mac's words on every platform, so the
/// snapshots are the same wherever the tests run.
fn model(screen: Screen) -> (Model, Instant) {
    let now = Instant::now();
    let mut model = Model::new(now, screen, Agent::Enabled, None);
    model.computer = "Mac";
    (model, now)
}

/// A model on the code screen with a code showing.
fn with_code(terminal: bool) -> Model {
    let (mut model, now) = model(Screen::Connect);
    if terminal {
        let _ = model.activate(Intent::ToggleTerminal, now);
    }
    model.host = Some(host(vec![], false, false));
    model.reached = true;
    let conditions = Conditions {
        visible: true,
        unlocked: true,
        connect_screen: true,
    };
    let actions = model.codes.tick(now, conditions);
    let Some(Action::Create { ticket, .. }) = actions.first().cloned() else {
        panic!("no create: {actions:?}");
    };
    let _ = model.codes.created(
        ticket,
        "c".repeat(64),
        "openagents-connect:AQ".into(),
        terminal,
        now,
    );
    model
}

/// Every screen state the snapshots cover, by name.
fn states() -> Vec<(&'static str, Model)> {
    let mut states = Vec::new();

    states.push(("dsk-01-code", with_code(false)));

    let mut copied = with_code(true);
    let _ = copied.activate(Intent::CopyCode, Instant::now());
    states.push(("dsk-01-terminal-copied", copied));

    let (starting, _) = model(Screen::Connect);
    states.push(("dsk-01-starting", starting));

    let (mut approval, _) = model(Screen::Connect);
    approval.agent = Agent::NeedsApproval;
    states.push(("dsk-01-needs-approval", approval));

    let mut idle = with_code(false);
    let later = Instant::now() + Duration::from_secs(601);
    let _ = idle.codes.tick(
        later,
        Conditions {
            visible: true,
            unlocked: true,
            connect_screen: true,
        },
    );
    states.push(("dsk-01-idle", idle));

    let mut another = with_code(false);
    another.host = Some(host(
        vec![phone('d', "Kai's iPhone", false, 30)],
        true,
        true,
    ));
    states.push(("dsk-01-another-phone", another));

    let device = "d".repeat(64);
    let (mut connected, _) = model(Screen::Connected {
        device: device.clone(),
    });
    connected.host = Some(host(
        vec![phone('d', "Kai's iPhone", false, 5)],
        false,
        false,
    ));
    connected.agents = Agents {
        codex: true,
        claude: false,
    };
    states.push(("dsk-02-connected", connected));

    // A phone that paired by scanning gives the host no name.
    let (mut scanned, _) = model(Screen::Connected {
        device: device.clone(),
    });
    scanned.host = Some(host(vec![phone('d', "", true, 5)], true, true));
    scanned.agents = Agents {
        codex: true,
        claude: false,
    };
    states.push(("dsk-02-connected-unnamed", scanned));

    let (mut picked, _) = model(Screen::Connected { device });
    picked.host = Some(host(vec![phone('d', "Kai's iPhone", true, 5)], true, true));
    picked.agents = Agents::default();
    states.push(("dsk-02-project-no-agents", picked));

    let tasks = vec![
        Task {
            title: "Fix the login test".into(),
            status: "running".into(),
            reason: None,
        },
        Task {
            title: "Update the README".into(),
            status: "finished".into(),
            reason: None,
        },
    ];
    let (mut home, _) = model(Screen::Home);
    home.host = Some(host(
        vec![
            phone('d', "Kai's iPhone", true, 120),
            phone('e', "Kai's iPad", false, 7_200),
        ],
        true,
        true,
    ));
    home.tasks = tasks.clone();
    states.push(("dsk-03-home", home));

    let (mut confirm, _) = model(Screen::Home);
    confirm.host = Some(host(
        vec![phone('d', "Kai's iPhone", true, 120)],
        true,
        true,
    ));
    confirm.confirming = Some("d".repeat(64));
    confirm.tasks = tasks;
    states.push(("dsk-03-confirm-remove", confirm));

    let (mut unnamed_home, _) = model(Screen::Home);
    unnamed_home.host = Some(host(vec![phone('d', "", true, 120)], true, true));
    unnamed_home.confirming = Some("d".repeat(64));
    states.push(("dsk-03-confirm-remove-unnamed", unnamed_home));

    let (mut offline, _) = model(Screen::Home);
    let mut state = host(vec![], false, false);
    state.status.online = false;
    offline.host = Some(state);
    states.push(("dsk-03-offline-empty", offline));

    let (mut earlier, _) = model(Screen::Home);
    earlier.agent = Agent::NotRegistered;
    earlier.old = Some(OldSetup {
        phones: Some(6),
        ready: false,
    });
    states.push(("dsk-03-earlier-setup", earlier));

    let prompt = |label: &str| crate::control::NearbyPrompt {
        id: 7,
        label: label.into(),
        code: "482913".into(),
    };
    let mut nearby = with_code(false);
    if let Some(host) = &mut nearby.host {
        host.nearby = Some(prompt("Kai's iPhone"));
    }
    states.push(("dsk-04-nearby", nearby));

    let (mut unnamed, now) = model(Screen::Home);
    let mut state = host(vec![phone('d', "Kai's iPhone", true, 120)], true, true);
    state.nearby = Some(prompt(""));
    unnamed.host = Some(state);
    let _ = unnamed.activate(Intent::NearbyTerminal, now);
    states.push(("dsk-04-nearby-terminal-unnamed", unnamed));

    let old = |ready| OldSetup {
        phones: Some(6),
        ready,
    };
    let now = Instant::now();
    states.push((
        "adopt-ready",
        Model::new(now, Screen::Adopt, Agent::NotRegistered, Some(old(true))),
    ));
    states.push((
        "adopt-not-yet",
        Model::new(now, Screen::Adopt, Agent::NotRegistered, Some(old(false))),
    ));
    let uncounted = OldSetup {
        phones: None,
        ready: false,
    };
    states.push((
        "adopt-uncounted",
        Model::new(now, Screen::Adopt, Agent::NotRegistered, Some(uncounted)),
    ));
    for (_, model) in &mut states {
        model.computer = "Mac";
    }
    states
}

fn snapshot_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("snapshots")
        .join(format!("{name}.txt"))
}

fn check_snapshot(name: &str, actual: &str) {
    let path = snapshot_path(name);
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
        std::fs::write(&path, actual).expect("the snapshot");
        return;
    }
    match std::fs::read_to_string(&path) {
        Ok(expected) if expected == actual => {}
        Ok(expected) => panic!(
            "the {name} snapshot differs.\nexpected:\n{expected}\nactual:\n{actual}\n\
             Run the tests with UPDATE_SNAPSHOTS=1 to record the change."
        ),
        Err(_) => panic!(
            "no snapshot for {name}. Run the tests with UPDATE_SNAPSHOTS=1; the view is:\n{actual}"
        ),
    }
}

fn view_of(model: &Model) -> Node<Intent> {
    root(model, NOW)
}

#[test]
fn every_screen_matches_its_snapshot() {
    for (name, model) in states() {
        check_snapshot(name, &outline(&view_of(&model)));
    }
}

#[test]
fn no_screen_shows_a_banned_word() {
    for (name, model) in states() {
        for text in words(&view_of(&model)) {
            let banned = banned_in(&text);
            assert!(banned.is_empty(), "{name} shows {banned:?} in {text:?}");
        }
    }
}

#[test]
fn every_screen_is_a_valid_view() {
    for (name, model) in states() {
        let mut presenter = Presenter::new("openagents-desktop");
        assert!(
            presenter.present(view_of(&model)),
            "{name} did not validate"
        );
        // The same tree again keeps its revision.
        assert!(!presenter.present(view_of(&model)));
    }
}

/// The code is on screen only as the drawing surface, never as text.
#[test]
fn the_code_text_is_never_a_word_on_screen() {
    for (name, model) in states() {
        for text in words(&view_of(&model)) {
            assert!(
                !text.contains("openagents-connect:"),
                "{name} shows the code"
            );
        }
    }
}

/// `DSK-02` and Home show the folder the person picked, never the host's
/// worktree of it; a project the host lists without one shows its path.
#[test]
fn the_picked_folder_shows_not_the_hosts_worktree() {
    let connected = Screen::Connected {
        device: "d".repeat(64),
    };
    for screen in [connected, Screen::Home] {
        let (mut model, _) = model(screen);
        model.host = Some(host(
            vec![phone('d', "Kai's iPhone", false, 60)],
            true,
            true,
        ));
        model.reached = true;
        let shown = outline(&view_of(&model));
        assert!(
            shown.contains("body \"/Users/kai/code/website\""),
            "{shown}"
        );
        assert!(!shown.contains(".openagents"), "{shown}");

        let refreshed = model.host.as_mut().expect("a host");
        refreshed.projects[0].folder = None;
        refreshed.projects[0].path = "/Users/kai/code/site-worktree".into();
        let shown = outline(&view_of(&model));
        assert!(
            shown.contains("body \"/Users/kai/code/site-worktree\""),
            "{shown}"
        );
    }
}

/// The terminal checkbox starts off.
#[test]
fn the_terminal_checkbox_is_off_by_default() {
    let model = with_code(false);
    let outline = outline(&view_of(&model));
    assert!(outline.contains("checkbox [ ] \"Let this phone open a terminal on this Mac\""));
}

/// On Linux and Windows no screen says "Mac": each says "this computer"
/// where a Mac's says "this Mac".
#[test]
fn off_a_mac_no_screen_says_mac() {
    for (name, mut model) in states() {
        model.computer = "computer";
        let words = words(&view_of(&model));
        for text in &words {
            assert!(
                !text
                    .split(|c: char| !c.is_alphanumeric())
                    .any(|w| w == "Mac"),
                "{name} shows {text:?}"
            );
            assert!(banned_in(text).is_empty(), "{name} shows {text:?}");
        }
    }
    let mut model = with_code(false);
    model.computer = "computer";
    let outline = outline(&view_of(&model));
    assert!(outline.contains("checkbox [ ] \"Let this phone open a terminal on this computer\""));
    assert_eq!(
        crate::words::COMPUTER,
        if cfg!(target_os = "macos") {
            "Mac"
        } else {
            "computer"
        }
    );
}

/// The window process holds no secret: its sources never name a keychain
/// store or the in-process adoption call. The host reads the keychain, and
/// adoption runs in `coder host adopt`, a child process.
#[test]
fn the_window_sources_never_reach_the_keychain() {
    let window = [
        ("main.rs", include_str!("main.rs")),
        ("shell.rs", include_str!("shell.rs")),
        ("worker.rs", include_str!("worker.rs")),
        ("mac.rs", include_str!("mac.rs")),
        ("migrate.rs", include_str!("migrate.rs")),
    ];
    for (file, source) in window {
        for needle in [
            "keychain::",
            "KeychainKeySource",
            "OsKeychain",
            "adopt_here",
            "keyring",
            "coder_service",
            "SecretKey",
        ] {
            assert!(!source.contains(needle), "{file} names {needle}");
        }
    }
}

/// Every screen lays out in the desktop adapter with nothing it can't draw.
#[cfg(feature = "app")]
#[test]
fn every_screen_lays_out_on_the_desktop() {
    use rust_native_desktop::layout::{Interaction, lay_out_window};
    use rust_native_desktop::text::Fonts;
    let mut fonts = Fonts::new();
    for (name, model) in states() {
        let view = rust_native::View::new("test", 1, view_of(&model));
        for (width, height) in [(560.0, 720.0), (420.0, 520.0)] {
            let scene = lay_out_window(
                &view,
                &rust_native_desktop::Theme::default(),
                &mut fonts,
                &|resource, available| {
                    (resource == crate::screens::CODE_SURFACE)
                        .then_some((available.min(300.0), available.min(300.0)))
                },
                &Interaction::default(),
                width,
                height,
            );
            assert!(
                scene.unsupported.is_empty(),
                "{name} at {width}: {:?}",
                scene.unsupported
            );
            assert!(!scene.ops.is_empty(), "{name} drew nothing");
            for hit in &scene.hits {
                assert!(
                    hit.rect.x >= 0.0 && hit.rect.x + hit.rect.w <= width + 0.5,
                    "{name}: {} leaves the window at {width}",
                    hit.key
                );
            }
        }
    }
}
