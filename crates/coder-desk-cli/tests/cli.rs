//! The `coder-desk` command, run against a fake desk.
//!
//! Each test starts `coder_desk::fake::Session`, which holds windows and
//! screens in memory, answers the protocol on a socket of its own, and
//! records every request. The command is run as a child with
//! `CODER_DESK_SOCKET` naming that socket, so the test reads the JSON the
//! scripts under `os/bin/` read and the exit status they branch on.

use std::process::{Command, Output};

use coder_desk::{Point, Screen, Size, Window, fake};
use serde_json::Value;

/// The command under test.
const BINARY: &str = env!("CARGO_BIN_EXE_coder-desk");

/// A window on the fake desk.
fn window(handle: &str, app_id: &str, title: &str) -> Window {
    Window {
        handle: handle.to_string(),
        app_id: app_id.to_string(),
        title: title.to_string(),
        pid: Some(4242),
        screen: "DP-2".to_string(),
        desk: 1,
        at: Point { x: 10, y: 20 },
        size: Size {
            width: 800,
            height: 600,
        },
        floating: false,
        pinned: false,
        fullscreen: false,
    }
}

/// The one screen the fake desk holds.
fn screen() -> Screen {
    Screen {
        name: "DP-2".to_string(),
        at: Point { x: 0, y: 0 },
        size: Size {
            width: 2560,
            height: 1440,
        },
        scale: 1.0,
        desk: 1,
    }
}

/// A desk holding two windows and one screen.
fn held() -> fake::Session {
    fake::Session::holding(
        vec![
            window("0x01", "coder-child-t1", "child one"),
            window("0x02", "foot", "selfie"),
        ],
        vec![screen()],
    )
}

/// One run of the command against a desk, or against no desk when `socket`
/// is empty. Every variable a session announces is cleared, so the run
/// reads the one this test names and nothing the machine happens to carry.
fn ran(socket: &str, arguments: &[&str]) -> Output {
    let mut command = Command::new(BINARY);
    command
        .args(arguments)
        .env_remove("HYPRLAND_INSTANCE_SIGNATURE")
        .env_remove("CODER_QUEST_SOCKET")
        .env_remove("XDG_RUNTIME_DIR");
    match socket.is_empty() {
        true => command.env_remove("CODER_DESK_SOCKET"),
        false => command.env("CODER_DESK_SOCKET", socket),
    };
    command.output().expect("the command runs")
}

/// What one run printed on stdout, with its trailing newline removed.
fn said(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string()
}

/// What one run printed on stderr.
fn complained(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// The status one run exited with.
fn status(output: &Output) -> i32 {
    output.status.code().unwrap_or(-1)
}

/// What one run printed, read as JSON.
fn json(output: &Output) -> Value {
    serde_json::from_str(&said(output)).expect("the command printed JSON")
}

/// The verbs the desk was asked, as the JSON each one carries.
fn verbs(session: &fake::Session) -> Vec<Value> {
    session
        .requests()
        .iter()
        .map(|request| serde_json::to_value(&request.verb).unwrap_or_default())
        .collect()
}

#[test]
fn list_prints_every_window_with_the_contract_fields() {
    let desk = held();
    let out = ran(&desk.socket().display().to_string(), &["list"]);
    assert_eq!(status(&out), 0, "{}", complained(&out));
    let rows = json(&out);
    let rows = rows.as_array().expect("an array of windows");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["handle"], "0x01");
    assert_eq!(rows[0]["app_id"], "coder-child-t1");
    assert_eq!(rows[0]["title"], "child one");
    assert_eq!(rows[0]["pid"], 4242);
    assert_eq!(rows[0]["desk"], 1);
    assert_eq!(rows[0]["at"]["x"], 10);
    assert_eq!(rows[0]["size"]["width"], 800);
    assert_eq!(rows[0]["pinned"], false);
}

#[test]
fn screens_prints_every_screen() {
    let desk = held();
    let out = ran(&desk.socket().display().to_string(), &["screens"]);
    assert_eq!(status(&out), 0, "{}", complained(&out));
    let rows = json(&out);
    let rows = rows.as_array().expect("an array of screens");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], "DP-2");
    assert_eq!(rows[0]["size"]["height"], 1440);
    assert_eq!(rows[0]["scale"], 1.0);
}

#[test]
fn focused_prints_the_window_and_null_when_there_is_none() {
    let desk = held();
    let out = ran(&desk.socket().display().to_string(), &["focused"]);
    assert_eq!(status(&out), 0, "{}", complained(&out));
    assert_eq!(json(&out)["handle"], "0x01");

    let empty = fake::Session::holding(Vec::new(), vec![screen()]);
    let out = ran(&empty.socket().display().to_string(), &["focused"]);
    assert_eq!(status(&out), 0, "{}", complained(&out));
    assert_eq!(said(&out), "null");
}

#[test]
fn reading_prints_the_screens_the_focus_and_the_desks() {
    let desk = held();
    let out = ran(&desk.socket().display().to_string(), &["reading"]);
    assert_eq!(status(&out), 0, "{}", complained(&out));
    let reading = json(&out);
    assert_eq!(reading["focused"], "DP-2");
    assert_eq!(reading["screens"][0]["name"], "DP-2");
    assert_eq!(reading["desks"][0]["id"], 1);
    assert_eq!(reading["desks"][0]["windows"], 2);
}

#[test]
fn open_names_the_command_the_desk_and_that_it_is_silent() {
    let desk = held();
    let out = ran(
        &desk.socket().display().to_string(),
        &["open", "--desk", "3", "--silent", "--", "foot -e coder"],
    );
    assert_eq!(status(&out), 0, "{}", complained(&out));
    assert_eq!(said(&out), "");
    assert_eq!(
        verbs(&desk),
        vec![serde_json::json!({
            "type": "open",
            "command": "foot -e coder",
            "desk": 3,
            "silent": true,
        })]
    );
}

#[test]
fn open_refuses_a_command_and_a_path_together() {
    let desk = held();
    let out = ran(
        &desk.socket().display().to_string(),
        &["open", "--path", "/tmp/a", "cat /tmp/a"],
    );
    assert_eq!(status(&out), 2, "{}", complained(&out));
    assert!(
        complained(&out).contains("exactly one of a command"),
        "{}",
        complained(&out)
    );
}

#[test]
fn focus_place_raise_and_close_name_one_window() {
    let desk = held();
    let socket = desk.socket().display().to_string();
    for arguments in [
        vec!["focus", "class:coder-child-t1"],
        vec!["place", "0x01", "3"],
        vec!["raise", "title:selfie"],
        vec!["close", "address:0x02"],
    ] {
        let out = ran(&socket, &arguments);
        assert_eq!(status(&out), 0, "{}", complained(&out));
    }
    assert_eq!(
        verbs(&desk),
        vec![
            serde_json::json!({"type": "focus", "handle": "class:coder-child-t1"}),
            serde_json::json!({"type": "place", "handle": "0x01", "desk": 3}),
            serde_json::json!({"type": "raise", "handle": "title:selfie"}),
            serde_json::json!({"type": "close", "handle": "0x02"}),
        ]
    );
}

#[test]
fn shape_sends_only_the_fields_its_flags_name() {
    let desk = held();
    let out = ran(
        &desk.socket().display().to_string(),
        &[
            "shape",
            "title:selfie",
            "--float",
            "--size",
            "360x360",
            "--at",
            "2140,60",
            "--aspect",
            "--border",
            "0",
            "--rounding",
            "0",
            "--no-shadow",
        ],
    );
    assert_eq!(status(&out), 0, "{}", complained(&out));
    assert_eq!(
        verbs(&desk),
        vec![serde_json::json!({
            "type": "shape",
            "handle": "title:selfie",
            "float": true,
            "at": {"x": 2140, "y": 60},
            "size": {"width": 360, "height": 360},
            "aspect": true,
            "border": {"size": 0, "rounding": 0},
            "shadow": false,
        })]
    );
}

#[test]
fn shape_with_tile_and_pin_sends_what_the_pane_watcher_sends() {
    let desk = held();
    let socket = desk.socket().display().to_string();
    assert_eq!(status(&ran(&socket, &["shape", "0x01", "--pin"])), 0);
    assert_eq!(status(&ran(&socket, &["shape", "0x01", "--tile"])), 0);
    assert_eq!(
        verbs(&desk),
        vec![
            serde_json::json!({"type": "shape", "handle": "0x01", "pin": true}),
            serde_json::json!({"type": "shape", "handle": "0x01", "float": false}),
        ]
    );
}

#[test]
fn scale_notice_and_reload_reach_the_desk() {
    let desk = held();
    let socket = desk.socket().display().to_string();
    assert_eq!(
        status(&ran(&socket, &["scale", "DP-2", "1.5"])),
        0,
        "the desk refused a scale"
    );
    assert_eq!(status(&ran(&socket, &["notice", "the run needs you"])), 0);
    assert_eq!(status(&ran(&socket, &["reload"])), 0);
    assert_eq!(
        verbs(&desk),
        vec![
            serde_json::json!({"type": "scale", "screen": "DP-2", "scale": 1.5}),
            serde_json::json!({"type": "notice", "text": "the run needs you"}),
            serde_json::json!({"type": "reload"}),
        ]
    );
}

#[test]
fn the_drive_verbs_reach_the_desk_with_their_fields() {
    let desk = held();
    let socket = desk.socket().display().to_string();
    for arguments in [
        vec!["key", "Super+Shift+d"],
        vec!["type", "echo hi"],
        vec!["click", "100", "200"],
        vec!["click", "100", "200", "right"],
        vec!["move", "5", "6"],
        vec!["shot", "/tmp/a.png"],
        vec!["shot", "/tmp/a.png", "--screen", "DP-2"],
    ] {
        let out = ran(&socket, &arguments);
        assert_eq!(status(&out), 0, "{arguments:?}: {}", complained(&out));
        assert_eq!(said(&out), "", "{arguments:?} prints nothing");
    }
    assert_eq!(
        verbs(&desk),
        vec![
            serde_json::json!({"type": "key", "chord": "super+shift+d"}),
            serde_json::json!({"type": "type", "text": "echo hi"}),
            serde_json::json!({"type": "click", "x": 100, "y": 200, "button": "left"}),
            serde_json::json!({"type": "click", "x": 100, "y": 200, "button": "right"}),
            serde_json::json!({"type": "move", "x": 5, "y": 6}),
            serde_json::json!({"type": "shot", "path": "/tmp/a.png"}),
            serde_json::json!({"type": "shot", "path": "/tmp/a.png", "screen": "DP-2"}),
        ]
    );
}

#[test]
fn a_key_that_is_not_a_chord_is_a_usage_error_before_the_desk_is_asked() {
    let desk = held();
    let socket = desk.socket().display().to_string();
    let out = ran(&socket, &["key", "hyper+t"]);
    assert_eq!(status(&out), 2, "{}", complained(&out));
    assert!(complained(&out).contains("hyper"), "{}", complained(&out));
    let out = ran(&socket, &["click", "1", "2", "back"]);
    assert_eq!(status(&out), 2, "{}", complained(&out));
    assert!(verbs(&desk).is_empty(), "nothing reached the desk");
}

#[test]
fn status_names_the_backend_the_socket_and_the_hands() {
    let desk = held();
    let socket = desk.socket().display().to_string();
    let out = ran(&socket, &["status"]);
    assert_eq!(status(&out), 0, "{}", complained(&out));
    // The fake desk tracks no hands, so it answers the `status` verb with
    // hands off; a desk that does not know the verb prints the first two
    // lines alone.
    assert_eq!(
        said(&out),
        format!("backend: native\nsocket: {socket}\nhands: off")
    );
}

#[test]
fn a_run_with_no_session_says_so_and_exits_three() {
    let out = ran("", &["list"]);
    assert_eq!(status(&out), 3, "{}", said(&out));
    let complaint = complained(&out);
    assert!(complaint.contains("no desktop session here"), "{complaint}");
    assert!(complaint.contains("reached over SSH"), "{complaint}");
    assert_eq!(said(&out), "");
}

#[test]
fn a_session_that_ended_says_so_and_exits_four() {
    let dir = std::env::temp_dir().join("coder-desk-cli-gone");
    let out = ran(&dir.display().to_string(), &["list"]);
    assert_eq!(status(&out), 4, "{}", said(&out));
    assert!(
        complained(&out).contains("no longer there"),
        "{}",
        complained(&out)
    );
}

#[test]
fn a_refusal_exits_five_and_says_why() {
    let desk = held();
    let out = ran(
        &desk.socket().display().to_string(),
        &["focus", "class:nothing-owns-this"],
    );
    assert_eq!(status(&out), 5, "{}", said(&out));
    assert!(
        complained(&out).contains("the desk refused"),
        "{}",
        complained(&out)
    );
}
