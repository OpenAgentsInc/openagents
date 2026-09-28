use super::*;
use tailnet::Device;

fn device(name: &str, os: &str, online: Option<bool>) -> Device {
    Device {
        name: name.into(),
        os: tailnet::os_label(os),
        address: "100.64.0.1".into(),
        online,
    }
}

fn app() -> App {
    let dir = std::env::temp_dir().join(format!("openagents-mobile-{}", std::process::id()));
    App::new(Config { state_dir: dir }).expect("app")
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
fn lists_devices_with_names_and_types() {
    let mut app = app();
    app.screen = Screen::Devices(Tailnet {
        name: Some("example.ts.net".into()),
        this_device: Some("openagents-ios".into()),
        devices: vec![
            device("laptop", "macOS", Some(true)),
            device("pi", "linux", Some(false)),
        ],
    });
    let reply = app.respond(Request::Show);
    let text = values(&reply.view);
    assert!(text.contains(&"2 devices on example.ts.net".to_string()));
    assert!(text.contains(&"laptop".to_string()));
    assert!(text.contains(&"macOS · 100.64.0.1".to_string()));
    assert!(text.contains(&"Linux · 100.64.0.1".to_string()));
    assert!(text.contains(&"Offline".to_string()));
}

#[test]
fn empty_tailnet_asks_to_connect() {
    let mut app = app();
    app.screen = Screen::Devices(Tailnet {
        name: None,
        this_device: None,
        devices: vec![],
    });
    let text = values(&app.respond(Request::Show).view);
    assert!(text.contains(&"Connect to a tailnet".to_string()));
}

#[test]
fn sign_in_opens_the_url_only_from_the_current_view() {
    let mut app = app();
    let url = Url::parse("https://login.tailscale.com/a/example").expect("url");
    app.screen = Screen::SignIn(url.clone());
    let reply = app.respond(Request::Show);
    let activation = Activation {
        instance: "openagents.home".into(),
        revision: reply.view["revision"].as_u64().expect("revision"),
        node: "sign-in".into(),
    };
    let stale = Activation {
        revision: 1_000,
        ..activation.clone()
    };
    let reply = app.respond(Request::Activate { activation: stale });
    assert!(reply.open_url.is_none());
    let reply = app.respond(Request::Activate {
        activation: Activation {
            revision: reply.view["revision"].as_u64().expect("revision"),
            ..activation
        },
    });
    assert_eq!(reply.open_url.as_deref(), Some(url.as_str()));
    assert!(reply.wait_for_sign_in);
}

/// Contacts Tailscale's control server. A new node gets a sign-in URL.
#[test]
#[ignore = "network: contacts controlplane.tailscale.com"]
fn live_control_server_answers() {
    let mut app = app();
    app.respond(Request::Refresh);
    match &app.screen {
        Screen::SignIn(url) => assert_eq!(url.host_str(), Some("login.tailscale.com")),
        Screen::Devices(_) => {}
        Screen::Failed(error) => panic!("{error}"),
        Screen::Loading => panic!("still loading"),
    }
}
