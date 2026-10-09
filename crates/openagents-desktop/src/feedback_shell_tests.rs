//! **Give feedback** on selected text (#10127): a selection in the
//! transcript, a right-click, **Give feedback**, a comment, and **Send**
//! file a playtest report with the quote, where it came from, and the
//! comment, which the triage key opens.

use super::tests::chat_fixture;
use super::*;
use openagents_desktop::chat::TRANSCRIPT;
use openagents_desktop::chat_action::Action as ChatAction;
use rust_native_desktop::input::{SurfaceInput, TextInput};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[test]
fn a_selection_becomes_feedback_the_triage_key_reads() {
    let (mut app, now) = chat_fixture(4);
    let _ = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
    let sent: Arc<Mutex<Vec<playtest::report::Report>>> = Arc::default();
    let record = sent.clone();
    let panel = app.chat.as_mut().unwrap();
    panel.set_feedback_sender(Arc::new(move |report| {
        record.lock().unwrap().push(report);
        Ok(playtest::feedback::SENT.into())
    }));
    // Without a selection the context menu offers no feedback.
    // Give feedback shows only in a preview build (#11120).
    app.chat.as_mut().unwrap().preview = true;
    assert!(app.context_menu_at(None, (400.0, 300.0), now));
    let view = serde_json::to_string(app.view().view()).unwrap();
    assert!(!view.contains("Give feedback"));
    app.activate(
        Intent::Chat {
            action: ChatAction::DismissOverlay,
        },
        now,
    );

    // Drag across the first message's words.
    let panel = app.chat.as_mut().unwrap();
    'drag: for y in (0..400).step_by(6) {
        for x in [20.0, 60.0, 120.0, 240.0] {
            let y = y as f32;
            let down = SurfaceInput::Down { x, y, shift: false };
            panel.surface(TRANSCRIPT, down, now);
            let end = (x + 160.0, y + 2.0);
            panel.surface(TRANSCRIPT, SurfaceInput::Move { x: end.0, y: end.1 }, now);
            panel.surface(TRANSCRIPT, SurfaceInput::Up { x: end.0, y: end.1 }, now);
            if !panel.transcript.selected_text().trim().is_empty() {
                break 'drag;
            }
        }
    }
    let selected = panel.transcript.selected_text();
    assert!(!selected.trim().is_empty(), "the drag selects text");

    assert!(app.context_menu_at(None, (400.0, 300.0), now));
    let view = serde_json::to_string(app.view().view()).unwrap();
    assert!(view.contains("Give feedback"), "{view}");
    app.activate(
        Intent::Chat {
            action: ChatAction::Command {
                key: "feedback".into(),
            },
        },
        now,
    );
    let view = serde_json::to_string(app.view().view()).unwrap();
    assert!(view.contains(playtest::feedback::PLACEHOLDER), "{view}");
    let _ = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
    app.text_input(TextInput::Commit("This should say it runs here."), now);
    app.activate(
        Intent::Chat {
            action: ChatAction::SaveName,
        },
        now,
    );
    for step in 0..200 {
        app.tick(now + Duration::from_millis(step));
        if app
            .chat
            .as_ref()
            .unwrap()
            .feedback_status()
            .is_some_and(|(sent, _)| sent)
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        app.chat.as_ref().unwrap().feedback_status(),
        Some((true, Some(playtest::feedback::SENT)))
    );
    let view = serde_json::to_string(app.view().view()).unwrap();
    assert!(view.contains("\"Sent\""), "{view}");

    let report = sent.lock().unwrap().remove(0);
    assert_eq!(report.kind, playtest::report::Kind::Comment);
    assert_eq!(report.happened, "This should say it runs here.");
    assert_eq!(report.context.platform, playtest::report::Platform::Macos);
    let selection = report.selection.clone().expect("the selection");
    assert_eq!(selection.text, selected.trim());
    assert!(selection.turn.is_some(), "{selection:?}");
    assert_eq!(
        selection.thread.as_deref(),
        app.chat.as_ref().unwrap().selected_chat()
    );
    assert!(report.chat.is_none() && report.session.is_none());

    // The triage key opens exactly that quote and comment.
    let tester = secp256k1::SecretKey::from_byte_array([1; 32]).unwrap();
    let triage = secp256k1::SecretKey::from_byte_array([2; 32]).unwrap();
    let public = triage.x_only_public_key(&secp256k1::Secp256k1::new()).0;
    let sealed = playtest::report::wrap(
        &report,
        &tester,
        &public,
        &playtest::report::Randomness {
            wrapper: secp256k1::SecretKey::from_byte_array([9; 32]).unwrap(),
            seal_nonce: [1; 32],
            wrap_nonce: [2; 32],
            seal_earlier: 0,
            wrap_earlier: 0,
        },
    )
    .unwrap();
    let opened = playtest::report::open(&sealed.wrap, &triage).unwrap();
    assert_eq!(opened.report.selection, Some(selection));
    assert_eq!(opened.report.happened, "This should say it runs here.");
}
