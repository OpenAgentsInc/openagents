use crate::app::{App, Config, Request};
use crate::tailnet::{Device, Tailnet, os_label};
use crate::tailnet_view::Screen;
use url::Url;

/// An app as the phone hosts open it: its chats publish their rows for the
/// host's transcript layout, which [`children_of`] reads.
fn app() -> (App, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = App::open(
        Config {
            state_dir: dir.path().to_path_buf(),
            secret_hex: "11".repeat(32),
        },
        crate::app::Launch {
            pulled_transcripts: true,
            ..crate::app::Launch::default()
        },
    )
    .expect("app");
    (app, dir)
}

/// Run the rest of a live test that created a task on a real host, then
/// archive that task whatever the outcome, so the test leaves nothing in the
/// owner's task and chat lists. The host keeps the task's record.
fn archiving(app: &mut App, task: (String, String), rest: impl FnOnce(&mut App)) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rest(&mut *app)));
    let archived = app.archive_task_for_test(task, std::time::Duration::from_secs(300));
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    archived.expect("the test's task is archived");
}

fn device(name: &str, os: &str, online: Option<bool>) -> Device {
    Device {
        name: name.into(),
        os: os_label(os),
        address: "100.64.0.1".into(),
        online,
    }
}

/// A node's children, or, for a transcript whose rows the app published to a
/// transcript source (`rust_native::layout::source`), the rows it holds, as
/// the host's layout reads them.
fn children_of(node: &serde_json::Value) -> Vec<serde_json::Value> {
    let props = &node["element"]["props"];
    if let Some(name) = props["source"].as_str() {
        let snapshot = rust_native::layout::source::get(name)
            .unwrap_or_else(|| panic!("no transcript source {name}"));
        return snapshot
            .rows()
            .map(|row| serde_json::to_value(row).expect("row"))
            .collect();
    }
    props["children"].as_array().cloned().unwrap_or_default()
}

fn values(view: &serde_json::Value) -> Vec<String> {
    let mut out = vec![];
    let mut pending = vec![view["root"].clone()];
    while let Some(node) = pending.pop() {
        let props = &node["element"]["props"];
        for field in ["value", "label"] {
            if let Some(value) = props[field].as_str() {
                out.push(value.to_owned());
            }
        }
        // Markdown: the text of its paragraphs and headings.
        if let Some(blocks) = props["blocks"].as_array() {
            for block in blocks {
                if let Some(spans) = block["spans"].as_array() {
                    out.push(spans.iter().filter_map(|s| s["text"].as_str()).collect());
                }
            }
        }
        pending.extend(children_of(&node).into_iter().rev());
    }
    out
}

#[test]
fn computers_surface_opens_with_the_device_key() {
    let (mut app, _dir) = app();
    let packet = app.call(Request::Snapshot);
    assert_eq!(packet.device.len(), 64);
    let computers = packet.computers.expect("computers view");
    assert_eq!(computers["schema"], "rust-native.view.v2");
    assert!(packet.notices.is_empty(), "{:?}", packet.notices);
    assert!(!packet.terminal);
}

#[test]
fn push_wakes_are_off_unless_the_build_names_a_relay_and_gateway() {
    // A default build: no status until a token arrives, then "off".
    let (mut app, _dir) = app();
    assert_eq!(app.call(Request::Snapshot).push, None);
    let packet = app.call(Request::PushToken {
        token: "ab".repeat(32),
    });
    assert_eq!(packet.push.as_deref(), Some("Wakes are off in this build."));
    // A build configured for push starts with wakes off until it registers,
    // and refuses a relay that is not wss:// outside loopback tests.
    let dir = tempfile::tempdir().expect("temp dir");
    let open = |relay: &str| {
        App::open(
            Config {
                state_dir: dir.path().to_path_buf(),
                secret_hex: "11".repeat(32),
            },
            crate::app::Launch {
                push: Some(
                    serde_json::from_value(serde_json::json!({
                        "relay_url": relay,
                        "gateway_url": "https://push.example.com",
                        "app_profile": "openagents-ios",
                    }))
                    .unwrap(),
                ),
                ..crate::app::Launch::default()
            },
        )
        .expect("app")
    };
    let mut configured = open("wss://relay.example.com");
    assert_eq!(
        configured.call(Request::Snapshot).push.as_deref(),
        Some("Wakes off")
    );
    drop(configured);
    let mut insecure = open("ws://relay.example.com");
    let status = insecure.call(Request::Snapshot).push.unwrap();
    assert!(status.starts_with("Wakes unavailable"), "{status}");
}

#[test]
fn the_wallets_advanced_section_opens_from_the_host_and_is_remembered() {
    let (mut app, _dir) = app();
    let advanced = |packet: crate::app::Packet| match packet.wallet {
        crate::wallet::Screen::Ready(summary) => summary.advanced.open,
        crate::wallet::Screen::Failed { .. } => panic!("the wallet failed"),
    };
    assert!(!advanced(app.call(Request::Snapshot)));
    let request: Request =
        serde_json::from_str(r#"{"op":"wallet_advanced","open":true}"#).expect("request");
    assert!(advanced(app.call(request)));
    // Saving the words before a wallet runs changes nothing.
    let request: Request = serde_json::from_str(r#"{"op":"wallet_words_saved"}"#).expect("request");
    assert!(advanced(app.call(request)));
}

#[test]
fn the_amount_format_is_saved_and_reaches_the_wallet() {
    let (mut app, dir) = app();
    let invoice_error = |app: &mut App, amount: &str| {
        let packet = app.call(Request::WalletInvoice {
            amount: amount.into(),
        });
        match packet.wallet {
            crate::wallet::Screen::Ready(summary) => summary.receive.lightning_error,
            crate::wallet::Screen::Failed { message } => Some(message),
        }
    };
    let packet = app.call(Request::Snapshot);
    assert_eq!(packet.amounts.format, "bip177");
    // BIP 177 mode takes whole base units only.
    assert_eq!(
        invoice_error(&mut app, "1.5").as_deref(),
        Some("Enter the amount in whole bitcoin base units, such as ₿1,000.")
    );
    let request: Request =
        serde_json::from_str(r#"{"op":"amount_format","format":"btc"}"#).expect("request");
    let packet = app.call(request);
    assert_eq!((packet.amounts.format, packet.amounts.unit), ("btc", "BTC"));
    assert!(packet.amounts.decimal);
    // Legacy mode reads decimal BTC, so the amount passes and the wallet,
    // which has no key here, is what stops it.
    assert_eq!(
        invoice_error(&mut app, "1.5").as_deref(),
        Some("The wallet is still starting.")
    );
    drop(app);
    let mut reopened = App::open(
        Config {
            state_dir: dir.path().to_path_buf(),
            secret_hex: "11".repeat(32),
        },
        crate::app::Launch::default(),
    )
    .expect("app");
    assert_eq!(reopened.call(Request::Snapshot).amounts.format, "btc");
    assert_eq!(
        reopened
            .call(Request::AmountFormat {
                format: "bip177".into()
            })
            .amounts
            .format,
        "bip177"
    );
}

/// Account > Appearance: the choice is saved, and System follows the
/// appearance the host reports. It stays on dark schemes: the scheme is
/// process-wide and other tests read it.
#[test]
fn the_theme_is_saved_and_system_follows_the_phone() {
    let (mut app, dir) = app();
    let request: Request =
        serde_json::from_str(r#"{"op":"theme","theme":"system"}"#).expect("request");
    let packet = app.call(request);
    assert_eq!(packet.appearance.choice, "system");
    let request: Request =
        serde_json::from_str(r#"{"op":"system_appearance","dark":true}"#).expect("request");
    let packet = app.call(request);
    assert_eq!(packet.appearance.scheme, "dark");
    assert_eq!(
        packet.appearance.palette,
        crate::appearance::HostPalette::DARK
    );
    drop(app);
    let mut reopened = App::open(
        Config {
            state_dir: dir.path().to_path_buf(),
            secret_hex: "11".repeat(32),
        },
        crate::app::Launch::default(),
    )
    .expect("app");
    assert_eq!(reopened.call(Request::Snapshot).appearance.choice, "system");
}

#[test]
fn lists_tailnet_devices_with_names_and_types() {
    let (mut app, _dir) = app();
    app.set_tailnet(Screen::Devices(Tailnet {
        name: Some("example.ts.net".into()),
        this_device: Some("openagents-ios".into()),
        devices: vec![
            device("laptop", "macOS", Some(true)),
            device("pi", "linux", Some(false)),
        ],
    }));
    let text = values(&app.call(Request::Snapshot).tailnet.expect("tailnet view"));
    assert!(text.contains(&"2 devices on example.ts.net".to_string()));
    assert!(text.contains(&"laptop".to_string()));
    assert!(text.contains(&"macOS · 100.64.0.1".to_string()));
    assert!(text.contains(&"Linux · 100.64.0.1".to_string()));
    assert!(text.contains(&"Offline".to_string()));
}

#[test]
fn empty_tailnet_asks_to_connect() {
    let (mut app, _dir) = app();
    app.set_tailnet(Screen::Devices(Tailnet {
        name: None,
        this_device: None,
        devices: vec![],
    }));
    let text = values(&app.call(Request::Snapshot).tailnet.expect("tailnet view"));
    assert!(text.contains(&"Connect to a tailnet".to_string()));
}

#[test]
fn sign_in_opens_the_url_only_from_the_current_view() {
    let (mut app, _dir) = app();
    let url = Url::parse("https://login.tailscale.com/a/example").expect("url");
    app.set_tailnet(Screen::SignIn(url.clone()));
    let revision = app.call(Request::Snapshot).tailnet.expect("view")["revision"]
        .as_u64()
        .expect("revision");
    let stale = app.call(Request::TailnetActivate {
        instance: "openagents.tailnet".into(),
        revision: revision + 100,
        node: "sign-in".into(),
    });
    assert!(stale.open_url.is_none());
    let revision = stale.tailnet.expect("view")["revision"]
        .as_u64()
        .expect("revision");
    let packet = app.call(Request::TailnetActivate {
        instance: "openagents.tailnet".into(),
        revision,
        node: "sign-in".into(),
    });
    assert_eq!(packet.open_url.as_deref(), Some(url.as_str()));
}

#[test]
fn terminal_requests_without_a_terminal_answer_closed() {
    let (mut app, _dir) = app();
    let packet: serde_json::Value =
        serde_json::from_slice(&app.respond(Request::TerminalPoll { known: None })).expect("json");
    assert_eq!(packet["open"], false);
}

/// Contacts Tailscale's control server. A new node gets a sign-in URL.
#[test]
#[ignore = "network: contacts controlplane.tailscale.com"]
fn live_control_server_answers() {
    let (mut app, _dir) = app();
    app.call(Request::TailnetRefresh);
    for _ in 0..200 {
        let packet = app.call(Request::Snapshot);
        if !packet.tailnet_loading {
            let text = values(&packet.tailnet.expect("view"));
            assert!(
                text.contains(&"Sign in with Tailscale".to_string())
                    || text.contains(&"Tailnet".to_string()),
                "{text:?}"
            );
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("tailnet read did not finish");
}

/// Nodes of `kind` in `view`, in document order.
fn nodes_of(view: &serde_json::Value, kind: &str) -> Vec<serde_json::Value> {
    let mut out = vec![];
    let mut pending = vec![view["root"].clone()];
    while let Some(node) = pending.pop() {
        pending.extend(children_of(&node).into_iter().rev());
        if node["element"]["kind"] == kind {
            out.push(node);
        }
    }
    out
}

/// Wait on the Coder tab's new chat, where it opens, until its computer
/// can take a chat, as `ready` says of the screen's text. Returns the screen.
fn new_chat(
    app: &mut App,
    deadline: std::time::Instant,
    ready: impl Fn(&[String]) -> bool,
) -> serde_json::Value {
    loop {
        // A new chat targets a computer once one is ready; before, it
        // starts with the basic Coder.
        let coder = app.call(Request::ComputersRefresh).coder.unwrap();
        if !nodes_of(&coder, "composer").is_empty() && ready(&values(&coder)) {
            return coder;
        }
        assert!(std::time::Instant::now() < deadline, "{:?}", values(&coder));
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

fn key_for(view: &serde_json::Value, label: &str) -> Option<String> {
    let mut pending = vec![&view["root"]];
    while let Some(node) = pending.pop() {
        let props = &node["element"]["props"];
        if node["element"]["kind"] == "button" && props["label"] == label {
            return node["key"].as_str().map(str::to_owned);
        }
        if let Some(children) = props["children"].as_array() {
            pending.extend(children.iter());
        }
    }
    None
}

#[test]
fn paste_invitation_asks_for_the_invitation() {
    let (mut app, _dir) = app();
    // The phone opens on the Computers list; adding by invitation is one
    // step away.
    let view = app
        .call(Request::Snapshot)
        .computers
        .expect("computers view");
    let node = key_for(&view, "Add a computer").expect("add control");
    let view = app
        .call(Request::ComputersActivate {
            instance: view["instance"].as_str().expect("instance").into(),
            revision: view["revision"].as_u64().expect("revision"),
            node,
        })
        .computers
        .expect("add screen");
    let node = key_for(&view, "Paste invitation").expect("paste control");
    let packet = app.call(Request::ComputersActivate {
        instance: view["instance"].as_str().expect("instance").into(),
        revision: view["revision"].as_u64().expect("revision"),
        node,
    });
    let input = packet.computers_input.expect("input request");
    let token = serde_json::to_value(&input).expect("json")["token"]
        .as_str()
        .expect("token")
        .to_owned();
    // A value that is not an invitation is refused and shown on the screen.
    let packet = app.call(Request::ComputersInput {
        token,
        value: "not an invitation".into(),
    });
    assert!(packet.computers.is_some());
}

#[test]
fn computers_screens_draw_in_neutral_colors() {
    let (mut app, _dir) = app();
    let view = app
        .call(Request::Snapshot)
        .computers
        .expect("computers view");
    let mut pending = vec![&view];
    let mut colors = 0;
    while let Some(value) = pending.pop() {
        if let Some(object) = value.as_object() {
            for field in ["foreground", "background"] {
                if let Some(color) = value["style"][field].as_object() {
                    colors += 1;
                    assert_eq!(color["red"], color["green"]);
                    assert_eq!(color["red"], color["blue"]);
                }
            }
            pending.extend(object.values());
        } else if let Some(items) = value.as_array() {
            pending.extend(items.iter());
        }
    }
    assert!(colors > 0);
    assert!(
        !values(&view)
            .iter()
            .any(|t| t.starts_with("Your directory")),
        "the owner directory is left out"
    );
}

/// Tailnet admission against a real host. Set `OPENAGENTS_TEST_ADMISSION`
/// to a tailnet IPv4 address whose host runs
/// `coder host serve --tailnet-admission standard` for this machine's
/// Tailscale user, and `OPENAGENTS_TEST_ADMISSION_HOST` to its host key.
#[test]
#[ignore = "network: needs a host with tailnet admission"]
fn live_tailnet_admission_adds_the_computer_and_its_chats() {
    let address = std::env::var("OPENAGENTS_TEST_ADMISSION").expect("address");
    let host = std::env::var("OPENAGENTS_TEST_ADMISSION_HOST").expect("host key");
    let (mut app, _dir) = app();
    app.set_tailnet(Screen::Devices(Tailnet {
        name: None,
        this_device: None,
        devices: vec![Device {
            name: "test-computer".into(),
            os: "macOS".into(),
            address,
            online: Some(true),
        }],
    }));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    loop {
        let packet = app.call(Request::Snapshot);
        let tailnet = values(&packet.tailnet.expect("tailnet view"));
        if tailnet.contains(&"OpenAgents connected".to_string()) {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "{tailnet:?}");
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    assert!(app.hosts().contains(&host), "{:?}", app.hosts());
    let computers = values(&app.call(Request::Snapshot).computers.expect("computers"));
    eprintln!("computers: {computers:?}");
    // Its Coder chats read through the chat pairing the answer carried.
    loop {
        app.call(Request::Snapshot);
        if app.chats_linked(&host) {
            return;
        }
        assert!(std::time::Instant::now() < deadline, "no chat pairing");
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

/// With no computer, the Coder tab opens on a new chat with OpenAgents,
/// ready to type, and never asks for a computer first: no target pill, and
/// no Connect a computer above the field (that is an offer under a reply
/// that needs one).
#[test]
fn the_coder_tab_needs_no_computer() {
    let (mut app, _dir) = app();
    let packet = app.call(Request::Snapshot);
    let coder = packet.coder.expect("coder view");
    let text = values(&coder);
    assert!(text.contains(&"OpenAgents".to_string()), "{text:?}");
    assert!(!text.contains(&"Cloud".to_string()), "{text:?}");
    assert!(key_for(&coder, "Connect a computer").is_none(), "{text:?}");
    let composer = &nodes_of(&coder, "composer")[0]["element"]["props"];
    assert_eq!(composer["enabled"], true);
    assert_eq!(composer["focus"], true);
    assert!(key_for(&coder, "Previous chats").is_some(), "{text:?}");
    assert!(!packet.chat_streaming);
    assert!(packet.coder_go.is_none());
}

/// A new Coder chat becomes a task on a real host. Set
/// `OPENAGENTS_TEST_INVITATION` to a fresh `coder host invite` from a host
/// with an `openagents` workspace. Without auto-start the task only queues.
#[test]
#[ignore = "network: needs a host invitation"]
fn live_coder_chat_creates_a_task() {
    let invitation = std::env::var("OPENAGENTS_TEST_INVITATION").expect("invitation");
    let (mut app, _dir) = app();
    app.admit_for_test(&invitation, "test-host");
    // The app reports the foreground, as the host does at launch.
    app.call(Request::Lifecycle { active: true });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    // The app refreshes Computers every few seconds while a tab shows.
    let coder = new_chat(&mut app, deadline, |text| {
        text.contains(&"On test-host · openagents".to_string())
    });
    // Send from the composer: the chat opens on the new task.
    let composer = nodes_of(&coder, "composer")[0].clone();
    let token = composer["element"]["props"]["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let packet = app.call(Request::CoderInput {
        token,
        value: "Say hello from the OpenAgents app test.".into(),
    });
    let task = app.open_coder_task().expect("the chat opens on its task");
    archiving(&mut app, task, |app| {
        let chat = packet.coder.unwrap();
        let text = values(&chat);
        eprintln!("coder: {text:?}");
        assert!(
            text.contains(&"Say hello from the OpenAgents app test.".to_string()),
            "{text:?}"
        );
        assert_eq!(nodes_of(&chat, "transcript").len(), 1);
        // A running chat offers stop, and its composer queues, with the
        // other ways to send on a long press.
        assert!(key_for(&chat, "Stop").is_some(), "{text:?}");
        let composer = &nodes_of(&chat, "composer")[0]["element"]["props"];
        let choices: Vec<&str> = composer["choices"]
            .as_array()
            .expect("choices")
            .iter()
            .filter_map(|choice| choice["label"].as_str())
            .collect();
        assert_eq!(choices[0], "Queue for next turn", "{choices:?}");
        assert_eq!(composer["busy"], false);
        assert_eq!(composer["enabled"], true);
        // In the previous chats, the host's activity summary lists it.
        let back = key_for(&chat, "Previous chats").expect("menu");
        app.call(Request::CoderActivate {
            instance: chat["instance"].as_str().unwrap().into(),
            revision: chat["revision"].as_u64().unwrap(),
            node: back,
        });
        loop {
            let text = values(&app.call(Request::ComputersRefresh).coder.unwrap());
            if text
                .iter()
                .any(|t| t.starts_with("Say hello from the OpenAgents app test.\n"))
            {
                eprintln!("listed: {text:?}");
                return;
            }
            assert!(std::time::Instant::now() < deadline, "{text:?}");
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
}

/// Coder chats against a real host with tailnet admission that already ran
/// a task: opening one shows its transcript. It only reads.
#[test]
#[ignore = "network: needs a host with tailnet admission and a finished task"]
fn live_coder_chat_shows_a_task_transcript() {
    let address = std::env::var("OPENAGENTS_TEST_ADMISSION").expect("address");
    let (mut app, _dir) = app();
    app.call(Request::Lifecycle { active: true });
    app.set_tailnet(Screen::Devices(Tailnet {
        name: None,
        this_device: None,
        devices: vec![Device {
            name: "test-computer".into(),
            os: "macOS".into(),
            address,
            online: Some(true),
        }],
    }));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
    let coder = loop {
        let mut coder = app.call(Request::ComputersRefresh).coder.unwrap();
        // The previous chats are behind the menu button.
        if let Some(menu) = key_for(&coder, "Previous chats") {
            coder = tap(&mut app, &coder, &menu);
        }
        let rows: Vec<String> = values(&coder)
            .into_iter()
            .filter(|t| t.contains('\n'))
            .collect();
        if !rows.is_empty() {
            eprintln!("chats: {:?}", &rows[..rows.len().min(3)]);
            break coder;
        }
        assert!(std::time::Instant::now() < deadline, "{:?}", values(&coder));
        std::thread::sleep(std::time::Duration::from_millis(500));
    };
    let row = nodes_of(&coder, "button")
        .into_iter()
        .find(|b| {
            b["element"]["props"]["label"]
                .as_str()
                .unwrap_or("")
                .contains('\n')
        })
        .unwrap()
        .clone();
    app.call(Request::CoderActivate {
        instance: coder["instance"].as_str().unwrap().into(),
        revision: coder["revision"].as_u64().unwrap(),
        node: row["key"].as_str().unwrap().into(),
    });
    loop {
        let chat = app.call(Request::ComputersRefresh).coder.unwrap();
        let messages = nodes_of(&chat, "message").len();
        if messages > 0 && nodes_of(&chat, "working").is_empty() {
            eprintln!("transcript: {:?}", values(&chat));
            return;
        }
        assert!(std::time::Instant::now() < deadline, "{:?}", values(&chat));
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

/// A real Coder chat on a real host with tailnet admission and auto-start:
/// the task runs and its transcript shows the reply. It sends one harmless
/// task. Set `OPENAGENTS_TEST_ADMISSION` and `OPENAGENTS_TEST_PROMPT`.
#[test]
#[ignore = "network: runs a real task on a real host"]
fn live_coder_chat_runs_a_task() {
    let address = std::env::var("OPENAGENTS_TEST_ADMISSION").expect("address");
    let prompt = std::env::var("OPENAGENTS_TEST_PROMPT").expect("prompt");
    let (mut app, _dir) = app();
    app.call(Request::Lifecycle { active: true });
    app.set_tailnet(Screen::Devices(Tailnet {
        name: None,
        this_device: None,
        devices: vec![Device {
            name: "test-computer".into(),
            os: "macOS".into(),
            address,
            online: Some(true),
        }],
    }));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
    let coder = new_chat(&mut app, deadline, |text| {
        text.iter()
            .any(|t| t.starts_with("On ") && t.contains(" · "))
    });
    let composer = nodes_of(&coder, "composer")[0].clone();
    let token = composer["element"]["props"]["token"]
        .as_str()
        .unwrap()
        .to_owned();
    app.call(Request::CoderInput {
        token,
        value: prompt,
    });
    let task = app.open_coder_task().expect("the chat opens on its task");
    archiving(&mut app, task, |app| {
        let mut last = String::new();
        loop {
            let chat = app.call(Request::ComputersRefresh).coder.unwrap();
            let text = values(&chat);
            let place = text.get(1).cloned().unwrap_or_default();
            if place != last {
                eprintln!("status: {place}");
                last = place.clone();
            }
            let running = !nodes_of(&chat, "working").is_empty();
            if !running
                && (place.starts_with("Done")
                    || place.starts_with("Failed")
                    || place.starts_with("Stopped"))
            {
                eprintln!("final: {text:?}");
                return;
            }
            assert!(std::time::Instant::now() < deadline, "{text:?}");
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
    });
}

/// The Coder list leaves out a task whose saved chat is archived, and keeps
/// one with no saved chat yet or an unarchived one.
#[test]
fn the_coder_list_leaves_out_archived_tasks() {
    let chat = |archived| coder_history::Chat {
        id: "chat".into(),
        harness: coder_history::Harness::Coder,
        native_id: Some("ab".repeat(32)),
        title: "Hello".into(),
        title_truncated: false,
        updated_at: None,
        archived,
        subagent: false,
        source_id: Some("source".into()),
        status: coder_history::SourceStatus::Available,
    };
    assert!(crate::coder_tab::archived(Some(&chat(true))));
    assert!(!crate::coder_tab::archived(Some(&chat(false))));
    assert!(!crate::coder_tab::archived(None));
}

/// The composer's action comes from the task's phase and the attention its
/// summary asks for, never from the text; a long press offers the ways to
/// send that fit the turn.
#[test]
fn the_composer_mode_follows_the_task_and_its_attention() {
    use crate::coder_tab::{Choice, Mode};
    use nostr::activity_summary::{Attention, Phase};
    for phase in [
        None,
        Some(Phase::Queued),
        Some(Phase::Running),
        Some(Phase::Waiting),
    ] {
        assert_eq!(Mode::of(phase, None), Mode::Queue);
        assert_eq!(Mode::of(phase, Some(Attention::None)), Mode::Queue);
    }
    // A turn that ended with a question or an approval request is answered.
    for attention in [Attention::Input, Attention::Approval] {
        assert_eq!(
            Mode::of(Some(Phase::Waiting), Some(attention)),
            Mode::Answer
        );
    }
    for phase in [
        Phase::Completed,
        Phase::Failed,
        Phase::Cancelled,
        Phase::Unknown,
    ] {
        assert_eq!(Mode::of(Some(phase), None), Mode::Send);
        assert!(Choice::offered(Some(phase)).is_empty());
    }
    // A turn that has not started can be steered; a running one stops for
    // the message.
    assert_eq!(
        Choice::offered(Some(Phase::Queued)),
        [Choice::Queue, Choice::SteerNow]
    );
    assert_eq!(
        Choice::offered(Some(Phase::Running)),
        [Choice::Queue, Choice::StopAndSend]
    );
}

/// A finished Coder chat continues on the same task: the follow-up runs as
/// the task's next turn and the chat shows it. Needs a host with tailnet
/// admission and auto-start; set `OPENAGENTS_TEST_ADMISSION`,
/// `OPENAGENTS_TEST_PROMPT`, and `OPENAGENTS_TEST_FOLLOW_UP`.
#[test]
#[ignore = "network: runs two real turns on a real host"]
fn live_coder_chat_continues_with_a_follow_up() {
    let address = std::env::var("OPENAGENTS_TEST_ADMISSION").expect("address");
    let prompt = std::env::var("OPENAGENTS_TEST_PROMPT").expect("prompt");
    let follow_up = std::env::var("OPENAGENTS_TEST_FOLLOW_UP").expect("follow-up");
    let (mut app, _dir) = app();
    app.call(Request::Lifecycle { active: true });
    app.set_tailnet(Screen::Devices(Tailnet {
        name: None,
        this_device: None,
        devices: vec![Device {
            name: "test-computer".into(),
            os: "macOS".into(),
            address,
            online: Some(true),
        }],
    }));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1200);
    let send = |app: &mut App, view: &serde_json::Value, value: &str| {
        let composer = nodes_of(view, "composer")[0].clone();
        let token = composer["element"]["props"]["token"]
            .as_str()
            .unwrap()
            .to_owned();
        app.call(Request::CoderInput {
            token,
            value: value.into(),
        })
    };
    let ended = |app: &mut App| loop {
        let chat = app.call(Request::ComputersRefresh).coder.unwrap();
        let text = values(&chat);
        let place = text.get(1).cloned().unwrap_or_default();
        if nodes_of(&chat, "working").is_empty()
            && ["Done", "Failed", "Stopped"]
                .iter()
                .any(|word| place.starts_with(word))
        {
            return chat;
        }
        assert!(std::time::Instant::now() < deadline, "{text:?}");
        std::thread::sleep(std::time::Duration::from_secs(3));
    };
    let coder = new_chat(&mut app, deadline, |text| {
        text.iter()
            .any(|t| t.starts_with("On ") && t.contains(" · "))
    });
    send(&mut app, &coder, &prompt);
    let task = app.open_coder_task().expect("the chat opens on its task");
    archiving(&mut app, task.clone(), |app| {
        let first = ended(app);
        eprintln!("first turn: {:?}", values(&first));
        send(app, &first, &follow_up);
        assert_eq!(app.open_coder_task(), Some(task.clone()), "same task");
        // The host's summary moves off the first turn's ending once the
        // follow-up is queued; the next turn may wait for a free slot.
        loop {
            let chat = app.call(Request::ComputersRefresh).coder.unwrap();
            let place = values(&chat).get(1).cloned().unwrap_or_default();
            if !place.starts_with("Done") {
                eprintln!("follow-up: {place}");
                break;
            }
            assert!(std::time::Instant::now() < deadline, "{:?}", values(&chat));
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        // The next turn ends, and its transcript shows both turns.
        loop {
            let second = ended(app);
            let text = values(&second);
            if text.iter().any(|t| t.contains(&follow_up)) {
                eprintln!("second turn: {text:?}");
                assert!(text.iter().any(|t| t.contains(&prompt)), "{text:?}");
                return;
            }
            assert!(std::time::Instant::now() < deadline, "{text:?}");
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
    });
}

/// Open a Coder chat on the tailnet computer at `OPENAGENTS_TEST_ADMISSION`
/// and send `prompt`; return the app, its directory, and the chat's task.
fn live_chat(prompt: &str) -> (App, tempfile::TempDir, (String, String)) {
    let address = std::env::var("OPENAGENTS_TEST_ADMISSION").expect("address");
    let (mut app, dir) = app();
    app.call(Request::Lifecycle { active: true });
    app.set_tailnet(Screen::Devices(Tailnet {
        name: None,
        this_device: None,
        devices: vec![Device {
            name: "test-computer".into(),
            os: "macOS".into(),
            address,
            online: Some(true),
        }],
    }));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let coder = new_chat(&mut app, deadline, |text| {
        text.iter()
            .any(|t| t.starts_with("On ") && t.contains(" · "))
    });
    send_as(&mut app, &coder, None, prompt);
    let task = app.open_coder_task().expect("the chat opens on its task");
    (app, dir, task)
}

/// Send `value` from the chat's composer, with its send token or the token
/// of the choice labeled `choice`.
fn send_as(
    app: &mut App,
    view: &serde_json::Value,
    choice: Option<&str>,
    value: &str,
) -> crate::app::Packet {
    let props = nodes_of(view, "composer")[0]["element"]["props"].clone();
    let token = match choice {
        None => props["token"].as_str().unwrap().to_owned(),
        Some(label) => props["choices"]
            .as_array()
            .expect("choices")
            .iter()
            .find(|c| c["label"] == label)
            .unwrap_or_else(|| panic!("no choice {label}"))["token"]
            .as_str()
            .unwrap()
            .to_owned(),
    };
    app.call(Request::CoderInput {
        token,
        value: value.into(),
    })
}

/// Refresh until `done` holds for the open chat, then return it.
fn until(
    app: &mut App,
    deadline: std::time::Instant,
    done: impl Fn(&serde_json::Value, &[String]) -> bool,
) -> serde_json::Value {
    loop {
        let chat = app.call(Request::ComputersRefresh).coder.unwrap();
        let text = values(&chat);
        if done(&chat, &text) {
            return chat;
        }
        assert!(std::time::Instant::now() < deadline, "{text:?}");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

/// The key of the button labeled `label` in the queue row showing `text`.
fn queued_button(view: &serde_json::Value, text: &str, label: &str) -> String {
    let mut pending = vec![&view["root"]];
    while let Some(node) = pending.pop() {
        let children = node["element"]["props"]["children"].as_array();
        let is_row = node["key"]
            .as_str()
            .is_some_and(|key| key.starts_with("coder-queued-row-"));
        if is_row
            && values(&serde_json::json!({"root": node}))
                .iter()
                .any(|t| t == text)
        {
            return key_for(&serde_json::json!({"root": node}), label).expect("button");
        }
        if let Some(children) = children {
            pending.extend(children.iter());
        }
    }
    panic!("no queued row shows {text}")
}

fn tap(app: &mut App, view: &serde_json::Value, key: &str) -> serde_json::Value {
    app.call(Request::CoderActivate {
        instance: view["instance"].as_str().unwrap().into(),
        revision: view["revision"].as_u64().unwrap(),
        node: key.into(),
    })
    .coder
    .unwrap()
}

/// Coder ends its turn with a question; the phone answers it, and the
/// answer runs as the task's next turn. Needs a host with tailnet admission,
/// auto-start, and this commit's engine; set `OPENAGENTS_TEST_ADMISSION`.
#[test]
#[ignore = "network: runs real turns on a real host"]
fn live_coder_chat_answers_a_question() {
    let prompt = "Before you do anything else, ask me with a question (set ask to \
                  question) whether I want the word apple or the word pear. Don't \
                  run any command. After I answer, reply with only the word I chose.";
    let (mut app, _dir, task) = live_chat(prompt);
    archiving(&mut app, task, |app| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
        let asked = until(app, deadline, |_, text| {
            text.contains(&"Coder is waiting for your answer.".to_owned())
        });
        eprintln!("asked: {:?}", values(&asked));
        let composer = &nodes_of(&asked, "composer")[0]["element"]["props"];
        assert_eq!(composer["placeholder"], "Answer Coder");
        send_as(app, &asked, None, "Pear.");
        let answered = until(app, deadline, |chat, text| {
            let place = text.get(1).cloned().unwrap_or_default();
            nodes_of(chat, "working").is_empty()
                && place.starts_with("Done")
                && text.iter().any(|t| t.contains("Pear."))
        });
        let text = values(&answered);
        eprintln!("answered: {text:?}");
        assert!(
            text.iter()
                .any(|t| t.to_lowercase().contains("pear") && t != "Pear."),
            "{text:?}"
        );
    });
}

/// While Coder works, messages queue on the computer; the queue panel
/// lists them under the edit lease, edits, reorders, and removes them, and
/// the queue then runs in its new order. Needs a host with tailnet
/// admission, auto-start, and this commit's host; set
/// `OPENAGENTS_TEST_ADMISSION`.
#[test]
#[ignore = "network: runs real turns on a real host"]
fn live_coder_chat_edits_its_queue() {
    let prompt = "Run the command `sleep 45`, then reply with only the word ready.";
    let (mut app, _dir, task) = live_chat(prompt);
    archiving(&mut app, task, |app| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1200);
        let running = until(app, deadline, |_, text| {
            text.get(1)
                .is_some_and(|place| place.starts_with("Working"))
        });
        let running = send_as(
            app,
            &running,
            Some("Queue for next turn"),
            "Reply with only the word one.",
        )
        .coder
        .unwrap();
        send_as(app, &running, None, "Reply with only the word two.");
        let listed = until(app, deadline, |chat, _| {
            key_for(chat, "Edit queue").is_some()
        });
        let key = key_for(&listed, "Edit queue").unwrap();
        let panel = tap(app, &listed, &key);
        let text = values(&panel);
        eprintln!("queue: {text:?}");
        assert!(text.contains(&"Queued messages".to_owned()), "{text:?}");
        assert!(text.contains(&"Reply with only the word one.".to_owned()));
        // Move the second up, then remove it: only "one" is left to run.
        let up = key_for(&panel, "Move up").expect("move up");
        let panel = tap(app, &panel, &up);
        let first = values(&panel)
            .into_iter()
            .find(|t| t.starts_with("Reply with only the word"))
            .unwrap();
        assert_eq!(first, "Reply with only the word two.");
        let remove = queued_button(&panel, "Reply with only the word two.", "Remove");
        let panel = tap(app, &panel, &remove);
        assert!(!values(&panel).contains(&"Reply with only the word two.".to_owned()));
        let done = key_for(&panel, "Done").expect("done");
        tap(app, &panel, &done);
        // The first turn ends and the one queued message runs next.
        let ended = until(app, deadline, |chat, text| {
            let place = text.get(1).cloned().unwrap_or_default();
            nodes_of(chat, "working").is_empty()
                && place.starts_with("Done")
                && text.iter().any(|t| t == "Reply with only the word one.")
        });
        let text = values(&ended);
        eprintln!("ended: {text:?}");
        assert!(
            !text.contains(&"Reply with only the word two.".to_owned()),
            "{text:?}"
        );
    });
}

/// The owner's reports on build 13, end to end on a real host with tailnet
/// admission and auto-start: a new chat runs within seconds, a running
/// turn's steps show while it runs, one working row shows at a time, a
/// follow-up shows at once and its reply arrives, and a reopened chat shows
/// its messages at once. Set `OPENAGENTS_TEST_ADMISSION`.
#[test]
#[ignore = "network: runs real turns on a real host"]
fn live_coder_chat_starts_promptly_follows_its_turns_and_reopens_at_once() {
    let prompt = "Run the command `sleep 5; echo first-step`, then run the command \
                  `sleep 5; echo second-step`, then reply with only the word finished.";
    let follow_up = "Reply with only the word again.";
    let started = std::time::Instant::now();
    let at = |what: &str| eprintln!("{:>6.1}s {what}", started.elapsed().as_secs_f32());
    let (mut app, _dir, task) = live_chat(prompt);
    at("sent");
    archiving(&mut app, task.clone(), |app| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
        // The message shows at once, before the computer lists the chat.
        let chat = app.call(Request::Snapshot).coder.unwrap();
        // Markdown shows the code spans without their backticks.
        let shown = prompt.replace('`', "");
        assert!(values(&chat).contains(&shown), "{:?}", values(&chat));
        // Follow the turn at the app's own cadence while it changes.
        let follow = |app: &mut App, done: &dyn Fn(&[String]) -> bool| {
            let mut last = String::new();
            let mut working = None;
            let mut steps_while_running = 0;
            loop {
                let packet = app.call(Request::ComputersRefresh);
                let chat = packet.coder.unwrap();
                let text = values(&chat);
                let place = text.get(1).cloned().unwrap_or_default();
                assert!(nodes_of(&chat, "working").len() <= 1, "{text:?}");
                if place != last {
                    at(&format!("status: {place}"));
                    last.clone_from(&place);
                }
                if place.starts_with("Working") && working.is_none() {
                    working = Some(started.elapsed());
                }
                let steps = nodes_of(&chat, "tool").len();
                if place.starts_with("Working") && steps > steps_while_running {
                    at(&format!("{steps} tool rows while it runs"));
                    steps_while_running = steps;
                }
                if nodes_of(&chat, "working").is_empty() && done(&text) {
                    at(&format!("ended: {text:?}"));
                    return (chat, working, steps_while_running);
                }
                assert!(std::time::Instant::now() < deadline, "{text:?}");
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        };
        let (ended, working, steps) = follow(app, &|text| {
            text.get(1).is_some_and(|place| place.starts_with("Done"))
                && text
                    .iter()
                    .any(|t| t.trim().eq_ignore_ascii_case("finished"))
        });
        eprintln!("working after {working:?}; tool rows seen while running: {steps}");
        assert!(working.is_some(), "the chat never showed Working");
        assert!(steps >= 1, "no step showed before the turn ended");
        // A follow-up shows at once, and its reply arrives.
        let sent = send_as(app, &ended, None, follow_up).coder.unwrap();
        assert!(
            values(&sent).iter().any(|t| t == follow_up),
            "{:?}",
            values(&sent)
        );
        at("follow-up sent");
        follow(app, &|text| {
            text.get(1).is_some_and(|place| place.starts_with("Done"))
                && text.iter().any(|t| t.trim().eq_ignore_ascii_case("again"))
        });
        // Out to the previous chats and in again: the messages show at once.
        let chat = app.call(Request::Snapshot).coder.unwrap();
        let list = tap(app, &chat, &key_for(&chat, "Previous chats").expect("menu"));
        let row = format!("task-{}", &task.1[..16]);
        let reopened = tap(app, &list, &row);
        let text = values(&reopened);
        at(&format!("reopened: {text:?}"));
        assert!(text.iter().any(|t| t == follow_up), "{text:?}");
        assert!(
            text.iter().any(|t| t.trim().eq_ignore_ascii_case("again")),
            "{text:?}"
        );
    });
}

/// Timings of the Coder tab against a real host with tailnet admission, as
/// the owner sees them: until a new chat on the computer is ready to type,
/// until the previous chats list its Coder chats, and opening the newest
/// ones with nothing kept on the phone and again. Set
/// `OPENAGENTS_TEST_ADMISSION`. It only reads, and prints what it measured.
#[test]
#[ignore = "network: needs a host with tailnet admission"]
fn live_coder_timings() {
    let address = std::env::var("OPENAGENTS_TEST_ADMISSION").expect("address");
    let (mut app, _dir) = app();
    app.call(Request::Lifecycle { active: true });
    app.set_tailnet(Screen::Devices(Tailnet {
        name: None,
        this_device: None,
        devices: vec![Device {
            name: "test-computer".into(),
            os: "macOS".into(),
            address,
            online: Some(true),
        }],
    }));
    let wait = |app: &mut App, what: &str, done: &dyn Fn(&serde_json::Value) -> bool| {
        let started = std::time::Instant::now();
        loop {
            let coder = app.call(Request::ComputersRefresh).coder.unwrap();
            if done(&coder) {
                return (coder, started.elapsed());
            }
            assert!(
                started.elapsed() < std::time::Duration::from_secs(120),
                "waiting for {what}: {:?}",
                values(&coder)
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    };
    let (landing, took) = wait(&mut app, "a new chat on the computer", &|coder| {
        values(coder)
            .iter()
            .any(|t| t.starts_with("On ") && t.contains(" · "))
            && nodes_of(coder, "composer")
                .first()
                .is_some_and(|c| c["element"]["props"]["enabled"] == true)
    });
    eprintln!("admission until a new chat on the computer is ready to type: {took:?}");
    let menu = key_for(&landing, "Previous chats").unwrap();
    let started = std::time::Instant::now();
    tap(&mut app, &landing, &menu);
    let (list, took) = wait(&mut app, "the previous chats", &|coder| {
        nodes_of(coder, "button")
            .iter()
            .any(|b| b["key"].as_str().is_some_and(|k| k.starts_with("task-")))
    });
    eprintln!(
        "previous chats with Coder tasks: {took:?} ({:?} since the tap)",
        started.elapsed()
    );
    let rows: Vec<String> = nodes_of(&list, "button")
        .iter()
        .filter_map(|b| b["key"].as_str().filter(|k| k.starts_with("task-")))
        .map(str::to_owned)
        .collect();
    for (index, row) in rows.iter().take(3).enumerate() {
        for attempt in ["open", "reopen"] {
            let list = app.call(Request::Snapshot).coder.unwrap();
            let list = match key_for(&list, "Previous chats") {
                Some(menu) => tap(&mut app, &list, &menu),
                None => list,
            };
            let started = std::time::Instant::now();
            tap(&mut app, &list, row);
            let (chat, took) = wait(&mut app, "the chat", &|coder| {
                !nodes_of(coder, "message").is_empty() || !nodes_of(coder, "tool").is_empty()
            });
            let shown = nodes_of(&chat, "message").len() + nodes_of(&chat, "tool").len();
            eprintln!(
                "coder chat {index} {attempt}: {took:?} ({:?} since the tap), {shown} rows",
                started.elapsed()
            );
        }
    }
}

/// Account > Your keys (BYOK, #10176): the host's stored keys and switch
/// reach the packet as last four characters and the status line, never as
/// a key, and "Use my keys for everything" with only a TypeSafe key stays
/// off with the one line.
#[test]
fn your_keys_reach_the_packet_without_the_key() {
    let (mut app, _dir) = app();
    let request: Request = serde_json::from_str(
        r#"{"op":"provider_keys","keys":[{"provider":"openrouter","key":"sk-or-v1-secretsecretABCD"}],"mine":true}"#,
    )
    .expect("request");
    let packet = app.call(request);
    let text = serde_json::to_string(&packet).expect("packet");
    assert!(!text.contains("secretsecret"), "a key reached the packet");
    let keys = packet.provider_keys;
    assert!(keys.mine);
    assert_eq!(keys.status, "Running on your keys.");
    assert_eq!(keys.rows[0].last_four.as_deref(), Some("ABCD"));
    let request: Request =
        serde_json::from_str(r#"{"op":"provider_key_remove","provider":"openrouter"}"#)
            .expect("request");
    let keys = app.call(request).provider_keys;
    assert!(!keys.mine, "removing the last chat key returns to ours");
    let request: Request = serde_json::from_str(
        r#"{"op":"provider_keys","keys":[{"provider":"typesafe","key":"ts-key-1234"}],"mine":false}"#,
    )
    .expect("request");
    app.call(request);
    let request: Request =
        serde_json::from_str(r#"{"op":"provider_keys_mine","on":true}"#).expect("request");
    let keys = app.call(request).provider_keys;
    assert!(!keys.mine);
    assert_eq!(keys.notice.as_deref(), Some(model_access::TYPESAFE_ONLY));
}
