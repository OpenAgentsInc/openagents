//! The Terminal beside the chat in the window (#11180).

use super::tests::chat_fixture;
use super::*;
use openagents_desktop::terminal_action::Action as TerminalAction;
use openagents_desktop::terminal_pane::{RESOURCE, SCREEN, TerminalPane};
use rust_native_desktop::input::{SurfaceInput, TextInput};

fn keys(app: &DesktopApp) -> Vec<String> {
    fn walk(node: &rust_native::Node<Intent>, out: &mut Vec<String>) {
        out.push(node.key.clone());
        if let rust_native::Element::Stack { children, .. } = &node.element {
            for child in children {
                walk(child, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(&app.view().view().root, &mut out);
    out
}

/// A chat window with the Terminal, as the app opens one, at a size with
/// room for both.
fn window() -> (DesktopApp, Instant) {
    let (mut app, now) = chat_fixture(4);
    app.terminal = Some(TerminalPane::new(None));
    app.viewport(1400.0, 900.0, 1.0);
    app.present();
    (app, now)
}

fn key(key: &'static str, control: bool) -> TextInput<'static> {
    TextInput::Key {
        key,
        text: Some(key),
        control,
        command: control,
        alt: false,
        shift: false,
    }
}

#[test]
fn the_chat_window_opens_with_the_terminal_beside_the_chat() {
    let (mut app, now) = window();
    let shown = keys(&app);
    for key in [
        "terminal-split",
        "terminal-pane",
        SCREEN,
        "terminal-agents-title",
    ] {
        assert!(shown.iter().any(|k| k == key), "{key} in {shown:?}");
    }
    assert!(
        app.surface_size(RESOURCE, 2000.0)
            .is_some_and(|(w, h)| w > 300.0 && h > 100.0)
    );

    // Hidden, the chat keeps a button that brings it back.
    app.activate(
        Intent::Terminal {
            action: TerminalAction::Hide,
        },
        now,
    );
    let shown = keys(&app);
    assert!(shown.iter().any(|k| k == "terminal-show"));
    assert!(!shown.iter().any(|k| k == SCREEN));
    app.activate(
        Intent::Terminal {
            action: TerminalAction::Show,
        },
        now,
    );
    assert!(keys(&app).iter().any(|k| k == SCREEN));
}

#[test]
fn a_click_on_the_screen_takes_the_keyboard_from_the_composer() {
    let (mut app, now) = window();
    assert!(app.surface_input(
        RESOURCE,
        SurfaceInput::Down {
            x: 5.0,
            y: 5.0,
            shift: false,
        },
        now,
    ));
    assert!(app.terminal.as_ref().unwrap().focused);
    let draft = app.chat.as_ref().unwrap().draft().to_owned();
    assert!(app.text_input(key("h", false), now));
    assert_eq!(
        app.chat.as_ref().unwrap().draft(),
        draft,
        "what is typed goes to the shell, not the composer"
    );

    // A click elsewhere gives the keyboard back to the window.
    let _ = app.pointer_down(Some("chat-body"), (1.0, 1.0), now);
    assert!(!app.terminal.as_ref().unwrap().focused);

    // Ctrl+` hides it and shows it again.
    assert!(app.text_input(key("`", true), now));
    assert!(!app.terminal.as_ref().unwrap().open);
    assert!(app.text_input(key("`", true), now));
    assert!(app.terminal.as_ref().unwrap().open);
}

/// Stop in the Agents panel leaves the request for the `coder` that runs
/// the agent.
#[test]
fn stop_in_the_agents_panel_reaches_the_coder_that_runs_the_agent() {
    let home = tempfile::tempdir().unwrap();
    let dir = agent_fleet::board::dir(home.path());
    let (mut app, now) = window();
    app.terminal = Some(TerminalPane::new(Some(dir.clone())));
    app.activate(
        Intent::Terminal {
            action: TerminalAction::Stop {
                pid: 4242,
                agent: "agent-2".into(),
            },
        },
        now,
    );
    assert!(dir.join("4242.agent-2.stop").exists());
}
