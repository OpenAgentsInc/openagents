//! What the client asks a desk, and what it answers back.
//!
//! Every verb runs against the fake desk in [`crate::fake`], which answers
//! the protocol over a real socket, so each test reads the request the desk
//! received and the reply the caller got.

use super::*;
use crate::fake::Session;

/// The tests that read or set this process's environment take this lock,
/// because the tests in one binary run beside each other and a socket one
/// of them names would reach the others.
static ENVIRONMENT: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// One window, as a desk reports it.
fn window(handle: &str, app_id: &str, desk: u32) -> Window {
    Window {
        handle: handle.to_string(),
        app_id: app_id.to_string(),
        title: format!("{app_id} window"),
        pid: Some(10),
        screen: "DP-2".to_string(),
        desk,
        at: Point { x: 7, y: 7 },
        size: Size {
            width: 1269,
            height: 709,
        },
        floating: false,
        pinned: false,
        fullscreen: false,
    }
}

/// One screen, as a desk reports it.
fn screen(name: &str, desk: u32) -> Screen {
    Screen {
        name: name.to_string(),
        at: Point { x: 0, y: 0 },
        size: Size {
            width: 2560,
            height: 1440,
        },
        scale: 1.0,
        desk,
    }
}

/// A desk holding two windows on one screen.
fn desk() -> Session {
    Session::holding(
        vec![window("0xaa", "foot", 1), window("0xbb", "chromium", 2)],
        vec![screen("DP-2", 1)],
    )
}

/// The verb each request carried, in order.
fn asked(held: &Session) -> Vec<Verb> {
    held.requests()
        .into_iter()
        .map(|request| request.verb)
        .collect()
}

#[tokio::test]
async fn list_answers_every_window_the_desk_holds() {
    let held = desk();
    let windows = held.desk().list().await.expect("the windows");
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0].handle, "0xaa");
    assert_eq!(windows[1].app_id, "chromium");
    assert_eq!(asked(&held), vec![Verb::List]);
}

#[tokio::test]
async fn screens_answers_every_screen_the_desk_holds() {
    let held = desk();
    let screens = held.desk().screens().await.expect("the screens");
    assert_eq!(screens.len(), 1);
    assert_eq!(screens[0].name, "DP-2");
    assert_eq!(screens[0].desk, 1);
    assert_eq!(asked(&held), vec![Verb::Screens]);
}

#[tokio::test]
async fn focused_answers_the_one_window_that_has_the_focus() {
    let held = desk();
    let window = held.desk().focused().await.expect("the focused window");
    assert_eq!(window.map(|window| window.handle), Some("0xaa".to_string()));
    assert_eq!(asked(&held), vec![Verb::Focused]);
}

#[tokio::test]
async fn open_carries_a_command_a_desk_a_silence_or_a_path() {
    let held = desk();
    held.desk()
        .open(Open::command("foot -e ls").on_desk(3).silently())
        .await
        .expect("the program started");
    held.desk()
        .open(Open::path("/tmp/note.rs"))
        .await
        .expect("the file opened");
    assert_eq!(
        asked(&held),
        vec![
            Verb::Open {
                command: Some("foot -e ls".to_string()),
                path: None,
                desk: Some(3),
                silent: true,
            },
            Verb::Open {
                command: None,
                path: Some("/tmp/note.rs".to_string()),
                desk: None,
                silent: false,
            },
        ]
    );
}

#[tokio::test]
async fn focus_moves_the_focus_and_a_selector_naming_nothing_is_refused() {
    let held = desk();
    let on = held.desk();
    on.focus(&Selector::Class("chromium".to_string()))
        .await
        .expect("the focus moved");
    let window = on.focused().await.expect("the focused window");
    assert_eq!(window.map(|window| window.handle), Some("0xbb".to_string()));

    let refused = on
        .focus(&Selector::Address("0xff".to_string()))
        .await
        .expect_err("no such window");
    assert_eq!(
        refused.refusal().map(|refused| refused.code.as_str()),
        Some(refusal::NO_SUCH_WINDOW)
    );
}

#[tokio::test]
async fn place_moves_a_window_to_a_desk() {
    let held = desk();
    let on = held.desk();
    on.place(&Selector::Address("0xaa".to_string()), 4)
        .await
        .expect("the window moved");
    let windows = on.list().await.expect("the windows");
    assert_eq!(windows[0].desk, 4);
}

#[tokio::test]
async fn raise_names_the_window_it_raises() {
    let held = desk();
    held.desk()
        .raise(&Selector::Address("0xbb".to_string()))
        .await
        .expect("the window rose");
    assert_eq!(
        asked(&held),
        vec![Verb::Raise {
            handle: Selector::Address("0xbb".to_string())
        }]
    );
}

#[tokio::test]
async fn close_takes_the_window_off_the_desk() {
    let held = desk();
    let on = held.desk();
    on.close(&Selector::Address("0xaa".to_string()))
        .await
        .expect("the window closed");
    let windows = on.list().await.expect("the windows");
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].handle, "0xbb");
}

#[tokio::test]
async fn shape_changes_how_the_desk_draws_a_window() {
    let held = desk();
    let on = held.desk();
    on.shape(
        &Selector::Address("0xaa".to_string()),
        Shape {
            float: Some(true),
            pin: Some(true),
            at: Some(Point { x: 100, y: 200 }),
            size: Some(Size {
                width: 560,
                height: 560,
            }),
            aspect: Some(true),
            border: Some(Border {
                size: Some(0),
                rounding: Some(0),
            }),
            shadow: Some(false),
        },
    )
    .await
    .expect("the window was shaped");
    let windows = on.list().await.expect("the windows");
    assert!(windows[0].floating);
    assert!(windows[0].pinned);
    assert_eq!(windows[0].at, Point { x: 100, y: 200 });
    assert_eq!(
        windows[0].size,
        Size {
            width: 560,
            height: 560
        }
    );
}

#[tokio::test]
async fn scale_sets_a_screen_and_an_unknown_screen_is_refused() {
    let held = desk();
    let on = held.desk();
    on.scale("DP-2", 2.0).await.expect("the screen was scaled");
    let screens = on.screens().await.expect("the screens");
    assert_eq!(screens[0].scale, 2.0);

    let refused = on.scale("DP-9", 2.0).await.expect_err("no such screen");
    assert_eq!(
        refused.refusal().map(|refused| refused.code.as_str()),
        Some(refusal::NO_SUCH_SCREEN)
    );
}

#[tokio::test]
async fn notice_and_reload_carry_what_they_say() {
    let held = desk();
    let on = held.desk();
    on.notice("the gate passed").await.expect("the notice rose");
    on.reload().await.expect("the session reloaded");
    assert_eq!(
        asked(&held),
        vec![
            Verb::Notice {
                text: "the gate passed".to_string()
            },
            Verb::Reload,
        ]
    );
}

/// A reading holds what generation 1's rows do not: which screen has the
/// focus, and the desks with their window counts. A native desk answers it
/// from its screens, its focused window, and its windows.
#[tokio::test]
async fn a_reading_names_the_focused_screen_and_counts_the_desks() {
    let held = desk();
    let reading = held.desk().reading().await.expect("the reading");
    assert_eq!(reading.screens.len(), 1);
    assert_eq!(reading.focused.as_deref(), Some("DP-2"));
    assert_eq!(reading.focused_desk(), Some(1));
    assert_eq!(
        reading.desks,
        vec![
            DeskRow {
                id: 1,
                screen: "DP-2".to_string(),
                windows: 1,
            },
            DeskRow {
                id: 2,
                screen: "DP-2".to_string(),
                windows: 1,
            },
        ]
    );
}

/// The blocking half sends the same bytes from a thread, which is what a
/// caller running a frame loop rather than a reactor uses.
#[test]
fn the_blocking_half_runs_a_verb_without_a_reactor() {
    let held = desk();
    let windows = held.desk().blocking().list().expect("the windows");
    assert_eq!(windows.len(), 2);
    assert_eq!(asked(&held), vec![Verb::List]);
}

/// Nothing announced a desk, so this run is not inside one.
#[test]
fn a_run_with_no_announcement_has_no_desk() {
    let _environment = ENVIRONMENT.lock().unwrap_or_else(|held| held.into_inner());
    assert_eq!(
        Desk::found(&Announcement::default()).err(),
        Some(Absent::NoSession)
    );
    if Announcement::from_env() == Announcement::default() {
        assert_eq!(Desk::here().err(), Some(Absent::NoSession));
    }
}

/// A socket nothing answers on is a session that has ended, which is a
/// different answer from no session at all.
#[test]
fn a_socket_that_does_not_answer_says_the_session_has_ended() {
    let gone = tempfile::Builder::new()
        .prefix("coder-desk-")
        .tempdir()
        .expect("a temporary directory");
    let announced = Announcement {
        desk_socket: None,
        signature: Some("s".into()),
        runtime_dir: Some(gone.path().to_path_buf()),
        quest_socket: None,
        panes: false,
    };
    let absent = Desk::found(&announced).expect_err("no session");
    assert!(matches!(absent, Absent::Unreachable(_)), "{absent:?}");
    assert!(absent.say().contains("no longer there"), "{}", absent.say());

    let missing = Desk::native(gone.path().join("desk.sock"));
    let error = missing.blocking().list().expect_err("no desk");
    assert!(matches!(error, DeskError::Unreachable(_)), "{error:?}");
    assert!(error.say().contains("no longer there"), "{}", error.say());
}

/// A session that announces a Hyprland signature is reached through the
/// Hyprland backend, which translates each verb to the wire format that
/// session speaks.
#[test]
fn a_hyprland_signature_is_reached_through_the_hyprland_backend() {
    let held = fake::hyprland_session(vec![("j/clients", "[]")]);
    let desk = held.desk().expect("the session");
    assert_eq!(desk.backend(), "hyprland");
    assert_eq!(desk.blocking().list().expect("the windows"), Vec::new());
}

/// `CODER_QUEST_SOCKET` names a socket CoderQuest answers the desk protocol
/// on, so the client reaches it through the native backend and asks it the
/// verbs it asks any other desk.
#[test]
fn a_quest_socket_is_reached_through_the_native_backend() {
    let quest = Session::holding(
        vec![window("0x1", "coder-terminal", 1)],
        vec![screen("CoderQuest", 1)],
    );
    let announced = Announcement {
        desk_socket: None,
        signature: None,
        runtime_dir: None,
        quest_socket: Some(quest.socket().to_path_buf()),
        panes: false,
    };
    let desk = Desk::found(&announced).expect("the session");
    assert_eq!(desk.backend(), "native");
    let held = desk.blocking().list().expect("the tiles");
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].handle, "0x1");
    assert_eq!(
        desk.blocking().screens().expect("the screen")[0].name,
        "CoderQuest"
    );
    desk.blocking()
        .focus(&Selector::Class("coder-terminal".into()))
        .expect("the focus");
    let asked: Vec<Verb> = quest.requests().into_iter().map(|ask| ask.verb).collect();
    assert_eq!(
        asked,
        vec![
            Verb::List,
            Verb::Screens,
            Verb::Focus {
                handle: Selector::Class("coder-terminal".into())
            }
        ]
    );
}

/// `CODER_DESK_SOCKET` is the desk protocol's own socket, which the Coder
/// compositor announces and nothing else, so an announcement that carries
/// it alone reaches the desk through the native backend. It is read before
/// a CoderQuest socket and before a Hyprland signature, so a session that
/// announces more than one is reached on its own socket.
#[test]
fn a_desk_socket_is_reached_first_through_the_native_backend() {
    let held = Session::holding(
        vec![window("0x1", "coder-terminal", 1)],
        vec![screen("DP-2", 1)],
    );
    let alone = Announcement {
        desk_socket: Some(held.socket().to_path_buf()),
        ..Announcement::default()
    };
    assert!(alone.names_a_desk());
    assert!(!alone.draws_its_own_panes());
    let desk = Desk::found(&alone).expect("the session");
    assert_eq!(desk.backend(), "native");
    assert_eq!(desk.socket(), held.socket());
    let tiles = desk.blocking().list().expect("the tiles");
    assert_eq!(tiles.len(), 1);
    assert_eq!(tiles[0].handle, "0x1");

    let gone = tempfile::Builder::new()
        .prefix("coder-desk-")
        .tempdir()
        .expect("a temporary directory");
    let several = Announcement {
        desk_socket: Some(held.socket().to_path_buf()),
        signature: Some("s".into()),
        runtime_dir: Some(gone.path().to_path_buf()),
        quest_socket: Some(gone.path().join("quest.sock")),
        panes: false,
    };
    let desk = Desk::found(&several).expect("the session");
    assert_eq!(desk.backend(), "native");
    assert_eq!(desk.socket(), held.socket());
    assert_eq!(asked(&held), vec![Verb::List]);
}

/// `Desk::here` is `Desk::found` over what this process was told, so the
/// socket `CODER_DESK_SOCKET` names arrives on the announcement rather
/// than being read behind it. This is the reading every caller of
/// `Announcement::from_env` shares.
#[test]
fn the_announcement_carries_the_socket_the_environment_names() {
    let _environment = ENVIRONMENT.lock().unwrap_or_else(|held| held.into_inner());
    let socket = std::path::PathBuf::from("/run/user/1000/coder-desk/1.sock");
    let was = std::env::var_os(crate::protocol::SOCKET_VAR);
    // SAFETY: `ENVIRONMENT` is held, so no other test in this binary reads
    // or writes the environment while this one changes it.
    unsafe { std::env::set_var(crate::protocol::SOCKET_VAR, &socket) };
    let announced = Announcement::from_env();
    // SAFETY: as above, `ENVIRONMENT` is still held.
    match was {
        Some(value) => unsafe { std::env::set_var(crate::protocol::SOCKET_VAR, value) },
        None => unsafe { std::env::remove_var(crate::protocol::SOCKET_VAR) },
    }
    assert_eq!(announced.desk_socket, Some(socket.clone()));
    assert!(announced.names_a_desk());
    assert_eq!(
        Desk::found(&announced).err(),
        Some(Absent::Unreachable(socket))
    );
}
