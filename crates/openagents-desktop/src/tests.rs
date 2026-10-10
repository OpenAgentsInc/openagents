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
use crate::model::{Agent, Agents, Intent, Model, Refreshed, Screen, Task};
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
        watchers: Vec::new(),
        background: None,
    }
}

/// A model on `screen`, in the Mac's words on every platform, so the
/// snapshots are the same wherever the tests run.
fn model(screen: Screen) -> (Model, Instant) {
    let now = Instant::now();
    let mut model = Model::new(now, screen, Agent::Enabled);
    model.computer = "Mac";
    (model, now)
}

/// A model on the code screen with a code showing.
fn with_code() -> Model {
    let (mut model, now) = model(Screen::Connect);
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
    let _ = model
        .codes
        .created(ticket, "c".repeat(64), "openagents-connect:AQ".into(), now);
    model
}

/// Every screen state the snapshots cover, by name.
fn states() -> Vec<(&'static str, Model)> {
    let mut states = Vec::new();

    states.push(("dsk-01-code", with_code()));

    let mut copied = with_code();
    let _ = copied.activate(Intent::CopyCode, Instant::now());
    states.push(("dsk-01-copied", copied));

    let (starting, _) = model(Screen::Connect);
    states.push(("dsk-01-starting", starting));

    let (mut approval, _) = model(Screen::Connect);
    approval.agent = Agent::NeedsApproval;
    states.push(("dsk-01-needs-approval", approval));

    let mut idle = with_code();
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

    let mut another = with_code();
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
        grok: None,
        claude_problem: None,
    };
    states.push(("dsk-02-connected", connected));

    // Grok Build installed here: allowed by default, so it is listed
    // (#10091), and it alone is enough for Coder to work.
    let (mut grok, _) = model(Screen::Connected {
        device: device.clone(),
    });
    grok.host = Some(host(
        vec![phone('d', "Kai's iPhone", false, 5)],
        false,
        false,
    ));
    grok.agents = Agents {
        codex: false,
        claude: false,
        grok: Some(true),
        claude_problem: None,
    };
    states.push(("dsk-02-connected-grok", grok));

    // A phone that paired by scanning gives the host no name.
    let (mut scanned, _) = model(Screen::Connected {
        device: device.clone(),
    });
    scanned.host = Some(host(vec![phone('d', "", true, 5)], true, true));
    scanned.agents = Agents {
        codex: true,
        claude: false,
        grok: None,
        claude_problem: None,
    };
    states.push(("dsk-02-connected-unnamed", scanned));

    // No folder chooser opened (a Linux desktop without one).
    let (mut no_chooser, _) = model(Screen::Connected {
        device: device.clone(),
    });
    no_chooser.host = Some(host(
        vec![phone('d', "Kai's iPhone", false, 5)],
        false,
        false,
    ));
    no_chooser.problem = Some(crate::model::NO_CHOOSER.into());
    states.push(("dsk-02-no-folder-chooser", no_chooser));

    // Another folder just chosen: it shows at once, saving.
    let (mut saving, _) = model(Screen::Connected {
        device: device.clone(),
    });
    saving.host = Some(host(vec![phone('d', "Kai's iPhone", true, 5)], true, true));
    saving.saving = Some(crate::model::Saving {
        path: PathBuf::from("/Users/kai/code/omarchy"),
        replace: Some("website".into()),
        autostart: true,
        running: true,
        tries: 1,
    });
    states.push(("dsk-02-saving-folder", saving));

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

    // An earlier setup that a safety check kept running: the normal code
    // screen, with one quiet line where the code goes.
    let (mut kept, _) = model(Screen::Connect);
    kept.agent = Agent::NotRegistered;
    kept.note = Some(crate::migrate::KEPT_RUNNING.into());
    states.push(("dsk-01-earlier-setup-kept", kept));

    let prompt = |label: &str| crate::control::NearbyPrompt {
        id: 7,
        label: label.into(),
        code: "482913".into(),
    };
    let mut nearby = with_code();
    if let Some(host) = &mut nearby.host {
        host.nearby = Some(prompt("Kai's iPhone"));
    }
    states.push(("dsk-04-nearby", nearby));

    let (mut unnamed, _) = model(Screen::Home);
    let mut state = host(vec![phone('d', "Kai's iPhone", true, 120)], true, true);
    state.nearby = Some(prompt(""));
    unnamed.host = Some(state);
    states.push(("dsk-04-nearby-unnamed", unnamed));

    // Home before Coder's first answer, and after it has not answered for
    // a while: a plain line and Try again, never a wait with no end.
    let (starting_home, _) = model(Screen::Home);
    states.push(("dsk-03-starting", starting_home));
    let (mut stalled_home, _) = model(Screen::Home);
    stalled_home.stalled = true;
    states.push(("dsk-03-not-answering", stalled_home));
    let (mut stalled_code, _) = model(Screen::Connect);
    stalled_code.stalled = true;
    stalled_code.reached = true;
    states.push(("dsk-01-not-answering", stalled_code));

    // Five phones that paired by scanning: no names, and every one with
    // the same full rights, so no row carries a rights note.
    let (mut five, _) = model(Screen::Home);
    five.host = Some(host(
        ['d', 'e', 'f', '1', '2']
            .into_iter()
            .map(|id| phone(id, "", true, 120))
            .collect(),
        true,
        true,
    ));
    five.tasks = vec![Task {
        title: "Fix the login test".into(),
        status: "running".into(),
        reason: None,
    }];
    states.push(("dsk-03-home-five-phones", five));

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

/// One flow (#9965): no screen asks about, or apologizes for, an earlier
/// setup. The upgrade is silent; at most one quiet line says it had to
/// wait.
#[test]
fn no_screen_asks_about_an_earlier_setup() {
    let sources = [include_str!("screens.rs"), include_str!("model.rs")];
    for phrase in [
        "already runs Coder",
        "take it over",
        "existing Coder setup",
        "Use it",
        "Not now",
        "Moving your setup",
        "from an earlier setup",
    ] {
        for source in sources {
            assert!(!source.contains(phrase), "a screen still says {phrase:?}");
        }
        for (name, model) in states() {
            for text in words(&view_of(&model)) {
                assert!(!text.contains(phrase), "{name} says {text:?}");
            }
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

/// No screen asks which rights a phone gets: there is no checkbox on the
/// code screen or on the nearby prompt, and no terminal question anywhere.
#[test]
fn no_screen_asks_about_a_terminal() {
    for (name, model) in states() {
        let outline = outline(&view_of(&model));
        assert!(
            !outline.contains("open a terminal"),
            "{name} asks about a terminal:\n{outline}"
        );
        if name.starts_with("dsk-01") || name.starts_with("dsk-04") {
            assert!(!outline.contains("checkbox"), "{name}:\n{outline}");
        }
    }
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
    assert_eq!(
        crate::words::COMPUTER,
        if cfg!(target_os = "macos") {
            "Mac"
        } else {
            "computer"
        }
    );
}

/// The pairing and task paths hold no host secret: their sources never name a keychain
/// store or the in-process adoption call. The host reads the keychain, and
/// adoption runs in `coder host adopt`, a child process.
#[test]
fn pairing_sources_never_reach_the_host_keychain() {
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

/// Home always shows the way to a QR code near its top: **Connect another
/// phone** is in the Phones header, whatever the phones or Coder's state.
#[test]
fn home_shows_connect_another_phone_in_the_phones_header() {
    for (name, model) in states() {
        if model.screen != Screen::Home || model.nearby().is_some() {
            continue;
        }
        let shown = outline(&view_of(&model));
        let header = shown
            .lines()
            .skip_while(|line| !line.trim_start().starts_with("horizontal stack"))
            .take(3)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            header.contains("bold \"Phones\"") || header.contains("\"Phones\" [bold]"),
            "{name}:\n{shown}"
        );
        assert!(
            header.contains("button \"Connect another phone\" -> connect_another"),
            "{name}:\n{shown}"
        );
    }
}

/// A rights note that every row would share says nothing: with one set
/// of rights on every phone, no row says "terminal".
#[test]
fn a_right_every_phone_shares_is_not_repeated_on_each_row() {
    let (mut model, _) = model(Screen::Home);
    model.host = Some(host(
        vec![phone('d', "", true, 60), phone('e', "", true, 60)],
        false,
        false,
    ));
    assert!(!outline(&view_of(&model)).contains("terminal"));
    model.host = Some(host(
        vec![phone('d', "", true, 60), phone('e', "", false, 60)],
        false,
        false,
    ));
    assert_eq!(outline(&view_of(&model)).matches("· terminal").count(), 1);
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

/// The intent of the button labelled `label` in `node`, if there is one.
fn button_intent(node: &Node<Intent>, label: &str) -> Option<Intent> {
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        match &node.element {
            rust_native::Element::Button {
                label: shown,
                intent,
                ..
            } if shown == label => return Some(intent.clone()),
            rust_native::Element::Stack { children, .. }
            | rust_native::Element::List { children, .. } => pending.extend(children),
            _ => {}
        }
    }
    None
}

/// #11234: Claude Code that can't run on this computer shows one card
/// with what to do, the exact command, and Retry, which checks again. A
/// missing Claude Code stays quiet while another agent can work.
#[test]
fn claude_code_problems_show_a_card_with_the_command_and_retry() {
    use crate::claude_setup::Problem;
    use crate::model::Request;
    for (problem, codex, shown) in [
        (Problem::NotFound, false, true),
        (Problem::NotFound, true, false),
        (Problem::NotSignedIn, true, true),
        (Problem::NotSignedIn, false, true),
        (Problem::Root, false, true),
    ] {
        let (mut model, start) = model(Screen::Connected {
            device: "d".repeat(64),
        });
        model.host = Some(host(
            vec![phone('d', "Kai's iPhone", false, 5)],
            false,
            false,
        ));
        model.agents = Agents {
            codex,
            claude: false,
            grok: None,
            claude_problem: Some(problem),
        };
        let view = view_of(&model);
        let text = words(&view);
        assert_eq!(
            text.iter().any(|line| line == problem.title()),
            shown,
            "{problem:?}: {text:?}"
        );
        if shown {
            assert!(text.iter().any(|line| line == problem.detail()));
            if let Some(command) = problem.command() {
                assert!(text.iter().any(|line| line == command), "{text:?}");
            }
            assert_eq!(button_intent(&view, "Retry"), Some(Intent::CheckClaude));
        } else {
            assert_eq!(button_intent(&view, "Retry"), None);
        }
        for line in &text {
            let banned = banned_in(line);
            assert!(
                banned.is_empty(),
                "{problem:?} shows {banned:?} in {line:?}"
            );
        }
        let mut presenter = Presenter::new("openagents-desktop");
        assert!(presenter.present(view), "{problem:?} did not validate");
        // Retry asks Coder again at once.
        let requests = model.activate(Intent::CheckClaude, start + Duration::from_secs(1));
        assert!(requests.contains(&Request::Coder), "{problem:?}");
    }
    // Once Claude Code can run, the card is gone.
    let (mut model, _) = model(Screen::Connected {
        device: "d".repeat(64),
    });
    model.host = Some(host(
        vec![phone('d', "Kai's iPhone", false, 5)],
        false,
        false,
    ));
    model.agents = Agents {
        codex: false,
        claude: true,
        grok: None,
        claude_problem: None,
    };
    assert_eq!(button_intent(&view_of(&model), "Retry"), None);
}
