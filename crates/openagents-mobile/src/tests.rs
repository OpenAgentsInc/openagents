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
    let view = app
        .call(Request::Snapshot)
        .computers
        .expect("computers view");
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
