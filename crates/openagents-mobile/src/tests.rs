use crate::app::{App, Config, Request};
use crate::tailnet::{Device, Tailnet, os_label};
use crate::tailnet_view::Screen;
use url::Url;

fn app() -> (App, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = App::new(Config {
        state_dir: dir.path().to_path_buf(),
        secret_hex: "11".repeat(32),
    })
    .expect("app");
    (app, dir)
}

fn device(name: &str, os: &str, online: Option<bool>) -> Device {
    Device {
        name: name.into(),
        os: os_label(os),
        address: "100.64.0.1".into(),
        online,
    }
}

fn values(view: &serde_json::Value) -> Vec<String> {
    let mut out = vec![];
    let mut pending = vec![&view["root"]];
    while let Some(node) = pending.pop() {
        let props = &node["element"]["props"];
        for field in ["value", "label"] {
            if let Some(value) = props[field].as_str() {
                out.push(value.to_owned());
            }
        }
        if let Some(children) = props["children"].as_array() {
            pending.extend(children.iter().rev());
        }
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
fn nodes_of<'a>(view: &'a serde_json::Value, kind: &str) -> Vec<&'a serde_json::Value> {
    let mut out = vec![];
    let mut pending = vec![&view["root"]];
    while let Some(node) = pending.pop() {
        if node["element"]["kind"] == kind {
            out.push(node);
        }
        if let Some(children) = node["element"]["props"]["children"].as_array() {
            pending.extend(children.iter().rev());
        }
    }
    out
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
fn chats_start_by_asking_for_a_computer() {
    let (mut app, _dir) = app();
    let packet = app.call(Request::Snapshot);
    let view = packet.chats.expect("chats view");
    let text = values(&view);
    assert!(text.contains(&"Add a computer".to_string()), "{text:?}");
    let node = key_for(&view, "Add a computer").expect("add control");
    let packet = app.call(Request::ChatsActivate {
        instance: view["instance"].as_str().expect("instance").into(),
        revision: view["revision"].as_u64().expect("revision"),
        node,
    });
    let input = packet.chats_input.expect("input request");
    assert!(input.scan);
    // Not a pairing code: the pairing fails with a notice, off the queue.
    let packet = app.call(Request::ChatsInput {
        token: input.token.clone(),
        value: "not an invitation".into(),
    });
    assert!(packet.chats_input.is_none());
    for _ in 0..100 {
        let packet = app.call(Request::Snapshot);
        let text = values(&packet.chats.expect("chats view"));
        if text.iter().any(|t| t.starts_with("Pairing failed")) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("no pairing notice");
}

/// Reads chats from a real observer. Set `OPENAGENTS_TEST_CHAT_SECRET` to the
/// device secret hex and `OPENAGENTS_TEST_CHAT_CODE` to a connection code
/// file from `coder-connect pair --client <device key>`, with
/// `coder-connect serve` running.
#[test]
#[ignore = "network: needs a running coder-connect observer"]
fn live_chats_from_an_observer() {
    let secret = std::env::var("OPENAGENTS_TEST_CHAT_SECRET").expect("secret");
    let code = std::fs::read_to_string(std::env::var("OPENAGENTS_TEST_CHAT_CODE").expect("code"))
        .expect("code file");
    let dir = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(Config {
        state_dir: dir.path().to_path_buf(),
        secret_hex: secret,
    })
    .expect("app");
    let view = app.call(Request::Snapshot).chats.expect("view");
    let node = key_for(&view, "Add a computer").expect("add");
    let input = app
        .call(Request::ChatsActivate {
            instance: view["instance"].as_str().expect("instance").into(),
            revision: view["revision"].as_u64().expect("revision"),
            node,
        })
        .chats_input
        .expect("input");
    app.call(Request::ChatsInput {
        token: input.token,
        value: code,
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let view = loop {
        let packet = app.call(Request::Snapshot);
        // Name the computer when asked.
        if let Some(input) = packet.chats_input {
            app.call(Request::ChatsInput {
                token: input.token,
                value: "This Mac".into(),
            });
            continue;
        }
        let view = packet.chats.expect("view");
        if !packet.chats_loading && key_for(&view, "Forget").is_some() {
            break view;
        }
        assert!(std::time::Instant::now() < deadline, "{:?}", values(&view));
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    let text = values(&view);
    eprintln!("catalog: {:?}", &text[..text.len().min(8)]);
    let chat = text
        .iter()
        .find(|t| t.contains("\nClaude · This Mac") || t.contains("\nCodex · This Mac"))
        .expect("a chat from this computer")
        .clone();
    let node = key_for(&view, &chat).expect("chat row");
    app.call(Request::ChatsActivate {
        instance: view["instance"].as_str().expect("instance").into(),
        revision: view["revision"].as_u64().expect("revision"),
        node,
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        let packet = app.call(Request::Snapshot);
        let view = packet.chats.expect("view");
        let text = values(&view);
        if !packet.chats_loading {
            let messages = nodes_of(&view, "message").len();
            eprintln!(
                "opened {chat:?}: {messages} messages; {:?}",
                &text[..text.len().min(4)]
            );
            assert!(messages > 0, "{text:?}");
            assert_eq!(nodes_of(&view, "transcript").len(), 1);
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "transcript did not load"
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
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
    loop {
        let packet = app.call(Request::Snapshot);
        let chats = values(&packet.chats.expect("chats"));
        if !packet.chats_loading
            && chats
                .iter()
                .any(|t| t.ends_with(" chats") && t != "0 chats")
        {
            eprintln!("chats: {:?}", &chats[..chats.len().min(6)]);
            return;
        }
        assert!(std::time::Instant::now() < deadline, "{chats:?}");
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

#[test]
fn coder_asks_for_a_computer_before_a_chat() {
    let (mut app, _dir) = app();
    let packet = app.call(Request::Snapshot);
    let text = values(&packet.coder.expect("coder view"));
    assert!(text.contains(&"Coder".to_string()));
    assert!(
        text.iter()
            .any(|t| t.starts_with("Add a computer under Account")),
        "{text:?}"
    );
}

/// Chats against a real host with tailnet admission, as in
/// `live_tailnet_admission_adds_the_computer_and_its_chats`: newest first
/// without subagents, and the newest chat opens at its end with earlier
/// messages on request. It only reads.
#[test]
#[ignore = "network: needs a host with tailnet admission"]
fn live_chat_list_and_tail() {
    let address = std::env::var("OPENAGENTS_TEST_ADMISSION").expect("address");
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let wait = |app: &mut App, what: &str, done: &dyn Fn(&crate::app::Packet) -> bool| loop {
        let packet = app.call(Request::Snapshot);
        if done(&packet) {
            return packet;
        }
        assert!(std::time::Instant::now() < deadline, "waiting for {what}");
        std::thread::sleep(std::time::Duration::from_millis(250));
    };
    // The chat list, newest first, with times.
    let packet = wait(&mut app, "chats", &|p| {
        !p.chats_loading && key_for(p.chats.as_ref().unwrap(), "Forget").is_some()
    });
    let chats = packet.chats.unwrap();
    let rows: Vec<String> = values(&chats)
        .into_iter()
        .filter(|t| t.contains('\n'))
        .collect();
    eprintln!("newest chats: {:?}", &rows[..rows.len().min(5)]);
    assert!(rows.len() >= 5);
    assert!(rows.iter().all(|row| !row.contains("subagent")));
    let times: Vec<&str> = rows
        .iter()
        .map(|row| row.rsplit(" · ").next().unwrap())
        .collect();
    assert!(times.windows(2).all(|pair| pair[0] >= pair[1]), "{times:?}");
    // Open the newest chat: it arrives whole, ending at the latest message.
    let node = key_for(&chats, &rows[0]).unwrap();
    app.call(Request::ChatsActivate {
        instance: chats["instance"].as_str().unwrap().into(),
        revision: chats["revision"].as_u64().unwrap(),
        node,
    });
    let packet = wait(&mut app, "the newest messages", &|p| !p.chats_loading);
    let reader = packet.chats.unwrap();
    let transcript = nodes_of(&reader, "transcript")[0].clone();
    let before = nodes_of(&reader, "message").len() + nodes_of(&reader, "tool").len();
    let earlier = !transcript["element"]["props"]["earlier"].is_null();
    eprintln!("opened: {before} rows, earlier: {earlier}");
    assert!(before > 0);
    if earlier {
        app.call(Request::ChatsActivate {
            instance: reader["instance"].as_str().unwrap().into(),
            revision: reader["revision"].as_u64().unwrap(),
            node: transcript["key"].as_str().unwrap().into(),
        });
        let packet = wait(&mut app, "earlier messages", &|p| !p.chats_loading);
        let reader = packet.chats.unwrap();
        let after = nodes_of(&reader, "message").len() + nodes_of(&reader, "tool").len();
        eprintln!("earlier: {before} -> {after} rows");
        assert!(after > before);
    }
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
    let coder = loop {
        // The app refreshes Computers every few seconds while a tab shows.
        let coder = app.call(Request::ComputersRefresh).coder.unwrap();
        if values(&coder).contains(&"On test-host · openagents".to_string()) {
            break coder;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{:?} {:?}",
            values(&coder),
            values(&app.call(Request::Snapshot).computers.unwrap())
        );
        std::thread::sleep(std::time::Duration::from_millis(500));
    };
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
    let chat = packet.coder.unwrap();
    let text = values(&chat);
    eprintln!("coder: {text:?}");
    assert!(
        text.contains(&"Say hello from the OpenAgents app test.".to_string()),
        "{text:?}"
    );
    assert_eq!(nodes_of(&chat, "transcript").len(), 1);
    let composer = &nodes_of(&chat, "composer")[0]["element"]["props"];
    assert_eq!(composer["busy"], true, "a running chat offers stop");
    // Back on the list, the host's activity summary lists it.
    let back = key_for(&chat, "Coder").expect("back");
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
        let coder = app.call(Request::ComputersRefresh).coder.unwrap();
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
