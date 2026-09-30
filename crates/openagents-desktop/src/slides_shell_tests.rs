//! The slide viewer in the window (#10057): `open_presentation` lays the
//! viewer over the page, animates it open on the frame clock, takes the
//! keys, and animates closed before the overlay goes.

use super::*;
use openagents_desktop::fake::FakeHost;
use openagents_desktop::model::{Agent, Screen};
use openagents_desktop::slides::{Control, NODE, OPEN, Phase, RESOURCE};
use rust_native_desktop::input::{SurfaceInput, TextInput};
use rust_native_desktop::layout::Op;
use std::sync::atomic::Ordering;
use std::time::Duration;

const DECK: &str = "three-devdays-later";
const WIDTH: f32 = 1200.0;
const HEIGHT: f32 = 840.0;

fn shell() -> (DesktopApp, Instant) {
    let fake = FakeHost::new("Test computer", unix_now());
    let context = Context::new(
        Box::new(fake.clone()),
        Some(fake),
        None,
        None,
        std::env::temp_dir(),
    );
    let now = Instant::now();
    let mut app = DesktopApp::inline_shell(Model::new(now, Screen::Home, Agent::Enabled), context);
    app.tick(now);
    (app, now)
}

fn key(app: &mut DesktopApp, name: &str, command: bool, now: Instant) -> bool {
    App::text_input(
        app,
        TextInput::Key {
            key: name,
            text: None,
            command,
            alt: false,
            shift: false,
        },
        now,
    )
}

/// The viewer's surface in a capture of the window, if it shows.
fn overlay(app: &mut DesktopApp) -> Option<rust_native_desktop::layout::Rect> {
    let (_, scene) = rust_native_desktop::capture(app, WIDTH, HEIGHT, 1.0);
    scene.ops.iter().find_map(|op| match op {
        Op::Surface { resource, rect, .. } if resource == RESOURCE => Some(*rect),
        _ => None,
    })
}

fn opacity(app: &DesktopApp) -> f32 {
    app.presentation().expect("the viewer shows").opacity()
}

#[test]
fn open_presentation_covers_the_window_and_animates_open_over_frames() {
    let (mut app, start) = shell();
    assert!(app.open_presentation("no-such-deck", start).is_err());
    assert!(app.presentation().is_none());
    app.open_presentation(DECK, start).expect("the deck opens");
    let rect = overlay(&mut app).expect("the viewer is the overlay");
    assert_eq!((rect.x, rect.y, rect.w, rect.h), (0.0, 0.0, WIDTH, HEIGHT));
    assert_eq!(app.modal_root(), Some(NODE));
    assert!(app.key_bindings().is_empty(), "the page's shortcuts wait");
    assert_eq!(opacity(&app), 0.0);
    let mut last = 0.0;
    for frame in 1..=14u64 {
        let now = start + Duration::from_millis(frame * 16);
        let wake = app.tick(now).expect("a wake");
        assert!(
            wake <= now + Duration::from_millis(16),
            "frame {frame} asks for the next"
        );
        let presentation = app.presentation().expect("the viewer shows");
        assert!(presentation.opacity() > last && presentation.opacity() < 1.0);
        assert!(presentation.scale() > 0.96 && presentation.scale() < 1.0);
        last = presentation.opacity();
    }
    app.tick(start + OPEN);
    let presentation = app.presentation().expect("the viewer shows");
    assert_eq!(presentation.phase(), Phase::Open);
    assert_eq!((presentation.opacity(), presentation.scale()), (1.0, 1.0));
}

#[test]
fn keys_navigate_and_the_counter_follows() {
    let (mut app, start) = shell();
    app.open_presentation(DECK, start).expect("the deck opens");
    let now = start + OPEN;
    app.tick(now);
    let total = app.presentation().unwrap().viewer().deck().len();
    assert!(key(&mut app, "ArrowRight", false, now));
    assert!(key(&mut app, "Space", false, now));
    assert_eq!(
        app.presentation().unwrap().counter(),
        format!("3 / {total}")
    );
    assert!(key(&mut app, "ArrowLeft", false, now));
    assert_eq!(
        app.presentation().unwrap().counter(),
        format!("2 / {total}")
    );
    assert!(
        key(&mut app, "x", false, now),
        "no plain key reaches the page"
    );
    assert!(!key(&mut app, "q", true, now), "quit stays the window's");
}

#[test]
fn f_toggles_full_screen_and_escape_steps_back_then_closes_the_overlay() {
    let (mut app, start) = shell();
    app.open_presentation(DECK, start).expect("the deck opens");
    let now = start + OPEN;
    app.tick(now);
    key(&mut app, "f", false, now);
    assert!(app.presentation().unwrap().fullscreen());
    key(&mut app, "F", false, now);
    assert!(!app.presentation().unwrap().fullscreen());
    key(&mut app, "f", false, now);
    key(&mut app, "Escape", false, now);
    let presentation = app.presentation().unwrap();
    assert!(!presentation.fullscreen() && presentation.phase() == Phase::Open);
    key(&mut app, "Escape", false, now);
    assert_eq!(app.presentation().unwrap().phase(), Phase::Closing);
    // Closing runs the animation backward, frame by frame.
    let mut last = 1.0;
    for frame in 1..=14u64 {
        app.tick(now + Duration::from_millis(frame * 16));
        let opacity = opacity(&app);
        assert!(opacity < last && opacity > 0.0, "frame {frame} fades");
        last = opacity;
    }
    assert!(overlay(&mut app).is_some(), "still showing while it closes");
    app.tick(now + OPEN);
    assert!(app.presentation().is_none(), "closed, the overlay goes");
    assert!(overlay(&mut app).is_none());
    assert_eq!(app.modal_root(), None);
    assert!(app.overlay_layout().is_none());
}

#[test]
fn the_close_button_animates_closed() {
    let (mut app, start) = shell();
    app.open_presentation(DECK, start).expect("the deck opens");
    let now = start + OPEN;
    app.tick(now);
    overlay(&mut app);
    let controls = app.presentation().unwrap().controls().to_vec();
    let (_, next) = controls.iter().find(|(c, _)| *c == Control::Next).unwrap();
    let down = |rect: PxRect| SurfaceInput::Down {
        x: rect.x + 4.0,
        y: rect.y + 4.0,
        shift: false,
    };
    assert!(app.surface_input(RESOURCE, down(*next), now));
    assert!(app.presentation().unwrap().counter().starts_with("2 / "));
    let (_, close) = controls.iter().find(|(c, _)| *c == Control::Close).unwrap();
    assert!(app.surface_input(RESOURCE, down(*close), now));
    assert_eq!(app.presentation().unwrap().phase(), Phase::Closing);
    app.tick(now + OPEN / 2);
    assert!(app.presentation().is_some());
    app.tick(now + OPEN);
    assert!(app.presentation().is_none());
}

#[test]
fn reduce_motion_opens_and_closes_at_once() {
    let (mut app, now) = shell();
    app.reduce_motion().store(true, Ordering::Relaxed);
    app.open_presentation(DECK, now).expect("the deck opens");
    assert_eq!(app.presentation().unwrap().phase(), Phase::Open);
    assert_eq!(opacity(&app), 1.0);
    key(&mut app, "Escape", false, now);
    app.tick(now);
    assert!(app.presentation().is_none(), "closed at once");
}

/// With `OPENAGENTS_SLIDES_CAPTURE=DIR`, writes the verification captures:
/// mid-open, the viewer, and full screen.
#[test]
fn captures_mid_open_viewer_and_full_screen() {
    let (mut app, start) = shell();
    app.open_presentation(DECK, start).expect("the deck opens");
    let directory = std::env::var_os("OPENAGENTS_SLIDES_CAPTURE").map(std::path::PathBuf::from);
    let write = |app: &mut DesktopApp, name: &str| {
        let (frame, scene) = rust_native_desktop::capture(app, WIDTH, HEIGHT, 2.0);
        assert!(scene.unsupported.is_empty(), "{:?}", scene.unsupported);
        if let Some(directory) = &directory {
            std::fs::create_dir_all(directory).unwrap();
            std::fs::write(directory.join(format!("{name}.png")), frame.png().unwrap()).unwrap();
        }
        frame
    };
    let before = write(&mut app, "slides-closed");
    app.tick(start + Duration::from_millis(60));
    let mid = write(&mut app, "slides-mid-open");
    app.tick(start + OPEN);
    let open = write(&mut app, "slides-viewer");
    assert_ne!(before.pixels, mid.pixels);
    assert_ne!(mid.pixels, open.pixels);
    key(&mut app, "f", false, start + OPEN);
    let full = write(&mut app, "slides-full-screen");
    assert_ne!(open.pixels, full.pixels);
}
