//! What each verb becomes on a Hyprland session's socket, and what the
//! session's answers read as.
//!
//! The requests are the strings `hyprctl` sends for the same work, so a
//! test reads the exact bytes without a session. The answers are recorded
//! from `coderos-4080`, cut to the fields the backend reads.

use super::*;
use crate::{Border, Button, Shape};

/// The backend, with every program a drive verb runs on `PATH`.
fn hypr() -> Hyprland {
    Hyprland::with_tools(|_| true)
}

/// The requests one verb makes.
fn requests(verb: Verb) -> Vec<String> {
    hypr().requests(&verb).expect("the requests")
}

/// The window `0x55d1` names.
fn address() -> Selector {
    Selector::Address("0x55d1".to_string())
}

#[test]
fn the_three_reads_ask_the_session_to_read() {
    assert_eq!(requests(Verb::List), vec!["j/clients"]);
    assert_eq!(requests(Verb::Screens), vec!["j/monitors"]);
    assert_eq!(requests(Verb::Focused), vec!["j/activewindow"]);
    assert_eq!(
        hypr().reading_requests(),
        vec!["j/monitors", "j/workspaces"]
    );
}

#[test]
fn open_starts_a_program_and_names_the_desk_it_starts_it_on() {
    assert_eq!(
        requests(Verb::Open {
            command: Some("foot -e ls".to_string()),
            path: None,
            desk: None,
            silent: false,
        }),
        vec!["/dispatch exec foot -e ls"]
    );
    assert_eq!(
        requests(Verb::Open {
            command: Some("foot".to_string()),
            path: None,
            desk: Some(3),
            silent: true,
        }),
        vec!["/dispatch exec [workspace 3 silent] foot"]
    );
    assert_eq!(
        requests(Verb::Open {
            command: Some("foot".to_string()),
            path: None,
            desk: Some(3),
            silent: false,
        }),
        vec!["/dispatch exec [workspace 3] foot"]
    );
    // A session starts programs and shows no file of its own.
    let refused = hypr()
        .requests(&Verb::Open {
            command: None,
            path: Some("/tmp/note.rs".to_string()),
            desk: None,
            silent: false,
        })
        .expect_err("no file viewer");
    assert_eq!(
        refused.refusal().map(|refused| refused.code.as_str()),
        Some(refusal::UNSUPPORTED)
    );
    let refused = hypr()
        .requests(&Verb::Open {
            command: None,
            path: None,
            desk: None,
            silent: false,
        })
        .expect_err("neither a command nor a path");
    assert_eq!(
        refused.refusal().map(|refused| refused.code.as_str()),
        Some(refusal::MALFORMED)
    );
}

#[test]
fn focus_place_raise_and_close_each_name_one_window() {
    assert_eq!(
        requests(Verb::Focus { handle: address() }),
        vec!["/dispatch focuswindow address:0x55d1"]
    );
    assert_eq!(
        requests(Verb::Place {
            handle: Selector::Class("coder-child-t1".to_string()),
            desk: 3,
        }),
        vec!["/dispatch movetoworkspacesilent 3,class:^(coder-child-t1)$"]
    );
    assert_eq!(
        requests(Verb::Raise { handle: address() }),
        vec!["/dispatch alterzorder top,address:0x55d1"]
    );
    assert_eq!(
        requests(Verb::Close { handle: address() }),
        vec!["/dispatch closewindow address:0x55d1"]
    );
}

/// A `class:` selector is matched as a regular expression, so the backend
/// builds one that matches the app-id and nothing else. It is the only
/// place in the repository that builds one.
#[test]
fn a_class_selector_becomes_a_regular_expression_that_matches_one_app_id() {
    assert_eq!(
        requests(Verb::Focus {
            handle: Selector::Class("chromium-browser".to_string()),
        }),
        vec!["/dispatch focuswindow class:^(chromium-browser)$"]
    );
    assert_eq!(
        requests(Verb::Focus {
            handle: Selector::Class("battle.net.exe".to_string()),
        }),
        vec![r"/dispatch focuswindow class:^(battle\.net\.exe)$"]
    );
    assert_eq!(
        requests(Verb::Focus {
            handle: Selector::Title("selfie".to_string()),
        }),
        vec!["/dispatch focuswindow title:selfie"]
    );
}

/// `shape` is the camera circle and the recording overlay: float it, size
/// it, put it where it goes, drop the border and the shadow, hold the
/// aspect, and pin it over every desk.
#[test]
fn shape_becomes_the_dispatches_that_draw_a_window() {
    assert_eq!(
        requests(shaping(
            Selector::Title("selfie".to_string()),
            Shape {
                float: Some(true),
                pin: Some(true),
                at: Some(Point { x: 1860, y: 140 }),
                size: Some(Size {
                    width: 560,
                    height: 560
                }),
                aspect: Some(true),
                border: Some(Border {
                    size: Some(0),
                    rounding: Some(0),
                }),
                shadow: Some(false),
            }
        )),
        vec![
            "/dispatch setfloating title:selfie",
            "/dispatch resizewindowpixel exact 560 560,title:selfie",
            "/dispatch movewindowpixel exact 1860 140,title:selfie",
            "/dispatch setprop title:selfie keep_aspect_ratio 1",
            "/dispatch setprop title:selfie border_size 0",
            "/dispatch setprop title:selfie rounding 0",
            "/dispatch setprop title:selfie no_shadow 1",
            "/dispatch pin title:selfie",
        ]
    );
    assert_eq!(
        requests(shaping(
            address(),
            Shape {
                float: Some(false),
                shadow: Some(true),
                ..Shape::default()
            }
        )),
        vec![
            "/dispatch settiled address:0x55d1",
            "/dispatch setprop address:0x55d1 no_shadow 0",
        ]
    );
    // A field left out keeps what the window has, so a `shape` that names
    // none asks the session for nothing.
    assert_eq!(
        requests(shaping(address(), Shape::default())),
        Vec::<String>::new()
    );
    // A session's `pin` toggles, so it cannot clear one by name.
    let refused = hypr()
        .requests(&shaping(
            address(),
            Shape {
                pin: Some(false),
                ..Shape::default()
            },
        ))
        .expect_err("no unpin");
    assert_eq!(
        refused.refusal().map(|refused| refused.code.as_str()),
        Some(refusal::UNSUPPORTED)
    );
}

/// The `shape` verb one window and one shape make.
fn shaping(handle: Selector, shape: Shape) -> Verb {
    Verb::Shape {
        handle,
        float: shape.float,
        pin: shape.pin,
        at: shape.at,
        size: shape.size,
        aspect: shape.aspect,
        border: shape.border,
        shadow: shape.shadow,
    }
}

#[test]
fn scale_notice_and_reload_reach_the_session_itself() {
    assert_eq!(
        requests(Verb::Scale {
            screen: "DP-2".to_string(),
            scale: 1.5,
        }),
        vec!["/keyword monitor DP-2,preferred,auto,1.5"]
    );
    assert_eq!(
        requests(Verb::Notice {
            text: "the gate passed".to_string(),
        }),
        vec!["/notify -1 6000 0 the gate passed"]
    );
    assert_eq!(requests(Verb::Reload), vec!["/reload"]);
    let refused = hypr()
        .requests(&Verb::Unknown)
        .expect_err("an unknown verb");
    assert_eq!(
        refused.refusal().map(|refused| refused.code.as_str()),
        Some(refusal::UNKNOWN_TYPE)
    );
}

/// The screen `coderos-4080` was showing, cut to what the backend reads.
const CLIENTS: &str = r#"[
  {"address":"0xaa","mapped":true,"at":[7,7],"size":[1269,709],"floating":false,
   "pinned":false,"fullscreen":0,"workspace":{"id":1,"name":"1"},"class":"foot",
   "title":"coder","pid":10},
  {"address":"0xcc","mapped":false,"at":[0,0],"size":[0,0],
   "workspace":{"id":2,"name":"2"},"class":"gone","title":"gone","pid":30}
]"#;
const MONITORS: &str = r#"[{"name":"DP-2","width":2560,"height":1440,"x":0,"y":0,"scale":1.0,
  "activeWorkspace":{"id":1,"name":"1"},"focused":true}]"#;
const ACTIVE: &str = r#"{"address":"0xaa","at":[7,7],"size":[1269,709],
  "workspace":{"id":1,"name":"1"},"class":"foot","title":"coder"}"#;
const WORKSPACES: &str = r#"[{"id":1,"name":"1","monitor":"DP-2","windows":2},
  {"id":2,"name":"2","monitor":"DP-2","windows":0}]"#;

/// One answer read as the reply it carries.
fn reply(verb: Verb, answer: &str) -> Reply {
    hypr()
        .reply(&verb, &[answer.to_string()])
        .expect("the reply")
}

/// A session answers `j/clients` with every window it holds, and a window
/// it has not mapped is not on a screen, so it is not one of them.
#[test]
fn the_windows_a_session_answers_read_as_rows() {
    let Reply::Windows { windows } = reply(Verb::List, CLIENTS) else {
        panic!("not the windows");
    };
    assert_eq!(windows.len(), 1);
    let window = &windows[0];
    assert_eq!(window.handle, "0xaa");
    assert_eq!(window.app_id, "foot");
    assert_eq!(window.title, "coder");
    assert_eq!(window.pid, Some(10));
    assert_eq!(window.desk, 1);
    assert_eq!(window.at, Point { x: 7, y: 7 });
    assert_eq!(
        window.size,
        Size {
            width: 1269,
            height: 709
        }
    );
    assert!(!window.floating);
    assert!(!window.fullscreen);
}

#[test]
fn the_screens_a_session_answers_read_as_rows() {
    let Reply::Screens { screens } = reply(Verb::Screens, MONITORS) else {
        panic!("not the screens");
    };
    assert_eq!(screens.len(), 1);
    assert_eq!(screens[0].name, "DP-2");
    assert_eq!(screens[0].desk, 1);
    assert_eq!(screens[0].scale, 1.0);
    assert_eq!(
        screens[0].size,
        Size {
            width: 2560,
            height: 1440
        }
    );
}

#[test]
fn the_focused_window_reads_as_one_row_or_as_none() {
    let Reply::Focused { window } = reply(Verb::Focused, ACTIVE) else {
        panic!("not the focused window");
    };
    assert_eq!(window.map(|window| window.handle), Some("0xaa".to_string()));
    let Reply::Focused { window } = reply(Verb::Focused, "{}") else {
        panic!("not the focused window");
    };
    assert_eq!(window, None);
}

/// A reading names the screen the focus is on and the desks the session
/// holds, which is what `j/monitors` and `j/workspaces` carry between them.
#[test]
fn a_reading_reads_the_monitors_and_the_workspaces() {
    let reading = hypr()
        .reading(&[MONITORS.to_string(), WORKSPACES.to_string()])
        .expect("the reading");
    assert_eq!(reading.focused.as_deref(), Some("DP-2"));
    assert_eq!(reading.focused_desk(), Some(1));
    assert_eq!(
        reading.desks,
        vec![
            DeskRow {
                id: 1,
                screen: "DP-2".to_string(),
                windows: 2,
            },
            DeskRow {
                id: 2,
                screen: "DP-2".to_string(),
                windows: 0,
            },
        ]
    );
}

/// A change verb answers `ok` or says why not, and the why not reaches the
/// caller as the refusal it is.
#[test]
fn a_change_the_session_refuses_carries_what_it_said() {
    assert_eq!(
        hypr()
            .reply(&Verb::Focus { handle: address() }, &["ok\n".to_string()])
            .expect("the reply"),
        Reply::Done
    );
    let refused = hypr()
        .reply(
            &Verb::Focus { handle: address() },
            &["Invalid dispatcher".to_string()],
        )
        .expect("the reply");
    assert_eq!(
        refused,
        Reply::Refused(Refusal::new(refusal::UNSUPPORTED, "Invalid dispatcher"))
    );
}

#[test]
fn a_key_that_is_a_launcher_chord_starts_the_launcher_and_any_other_goes_to_the_window() {
    assert_eq!(
        requests(Verb::Key {
            chord: "super+b".to_string()
        }),
        vec!["/dispatch exec coder-browser"]
    );
    assert_eq!(
        requests(Verb::Key {
            chord: "super+c".to_string()
        }),
        vec!["/dispatch exec camera-toggle"]
    );
    // A layout chord runs in the compositor, which a session has no request
    // for, so it reaches the window.
    assert_eq!(
        requests(Verb::Key {
            chord: "super+t".to_string()
        }),
        vec!["/dispatch sendshortcut SUPER, t"]
    );
    assert_eq!(
        requests(Verb::Key {
            chord: "ctrl+c".to_string()
        }),
        vec!["/dispatch sendshortcut CTRL, c"]
    );
    assert_eq!(
        requests(Verb::Key {
            chord: "return".to_string()
        }),
        vec!["/dispatch sendshortcut , return"]
    );
    let refused = hypr()
        .requests(&Verb::Key {
            chord: "hyper+t".to_string(),
        })
        .expect_err("no such modifier");
    assert_eq!(
        refused.refusal().map(|refused| refused.code.as_str()),
        Some(refusal::MALFORMED)
    );
}

#[test]
fn type_shot_and_click_start_the_program_that_does_each_in_the_session() {
    assert_eq!(
        requests(Verb::Type {
            text: "echo 'hi'".to_string()
        }),
        vec![r"/dispatch exec wtype -- 'echo '\''hi'\'''"]
    );
    assert_eq!(
        requests(Verb::Shot {
            path: "/tmp/a.png".to_string(),
            screen: None
        }),
        vec!["/dispatch exec grim '/tmp/a.png'"]
    );
    assert_eq!(
        requests(Verb::Shot {
            path: "/tmp/a.png".to_string(),
            screen: Some("DP-2".to_string())
        }),
        vec!["/dispatch exec grim -o 'DP-2' '/tmp/a.png'"]
    );
    assert_eq!(
        requests(Verb::Click {
            x: 100,
            y: 200,
            button: Button::Right
        }),
        vec![
            "/dispatch movecursor 100 200",
            "/dispatch exec wlrctl pointer click right"
        ]
    );
    assert_eq!(
        requests(Verb::Move {
            x: 5,
            y: 6,
            steps: None,
            ms: None
        }),
        vec!["/dispatch movecursor 5 6"]
    );
    assert_eq!(
        requests(Verb::Scroll {
            dx: 0.0,
            dy: -3.0,
            discrete: false,
            steps: None,
            ms: None
        }),
        // `wlrctl pointer scroll` takes the vertical amount first.
        vec!["/dispatch exec wlrctl pointer scroll -3 0"]
    );
}

#[test]
fn a_verb_that_holds_a_button_or_a_key_is_refused_with_the_program_that_cannot() {
    let session = Hyprland::with_tools(|_| true);
    for (verb, says) in [
        (
            Verb::ButtonPress {
                button: Button::Left,
            },
            "wlrctl",
        ),
        (
            Verb::ButtonRelease {
                button: Button::Left,
            },
            "wlrctl",
        ),
        (
            Verb::Drag {
                from: Point { x: 1, y: 2 },
                to: Point { x: 3, y: 4 },
                button: Button::Left,
                modifiers: "super".to_string(),
                steps: None,
                ms: None,
            },
            "wlrctl",
        ),
        (
            Verb::KeyPress {
                key: "super".to_string(),
            },
            "wtype",
        ),
        (
            Verb::KeyRelease {
                key: "super".to_string(),
            },
            "wtype",
        ),
        (
            Verb::Scroll {
                dx: 0.0,
                dy: 1.0,
                discrete: true,
                steps: None,
                ms: None,
            },
            "notch",
        ),
        (
            Verb::Move {
                x: 1,
                y: 2,
                steps: Some(8),
                ms: None,
            },
            "step",
        ),
    ] {
        let refused = session
            .requests(&verb)
            .expect_err("this session holds nothing");
        let refusal = refused.refusal().expect("a refusal");
        assert_eq!(refusal.code, refusal::UNSUPPORTED);
        assert!(refusal.message.contains(says), "{}", refusal.message);
    }
}

#[test]
fn a_drive_verb_whose_program_is_missing_is_refused_by_name() {
    let bare = Hyprland::with_tools(|_| false);
    for verb in [
        Verb::Type {
            text: "hi".to_string(),
        },
        Verb::Shot {
            path: "/tmp/a.png".to_string(),
            screen: None,
        },
        Verb::Click {
            x: 1,
            y: 2,
            button: Button::Left,
        },
    ] {
        let refused = bare.requests(&verb).expect_err("the program is missing");
        let refusal = refused.refusal().expect("a refusal");
        assert_eq!(refusal.code, refusal::UNSUPPORTED);
        assert!(refusal.message.contains("PATH"), "{}", refusal.message);
    }
    // A move is the session's own dispatcher and needs no program.
    assert!(
        bare.requests(&Verb::Move {
            x: 1,
            y: 2,
            steps: None,
            ms: None
        })
        .is_ok()
    );
}
