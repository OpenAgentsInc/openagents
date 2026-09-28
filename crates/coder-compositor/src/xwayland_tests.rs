//! Tests for the X11 half of the compositor.
//!
//! The tests need no display and no Xwayland. The class the desk protocol
//! reports and the fraction of the screen a window asked for are functions
//! of strings and of integers, so each test drives them directly; where
//! the layout puts a window under a rule is `rules_tests.rs`. The desk
//! protocol test answers `list` from a fixture whose rows carry the
//! classes this module reports, and reads the answer back the way
//! `coder-desk` does.

use super::*;
use coder_desk::protocol::{
    Answer, Point, Refusal, Reply, Request, Screen as DeskScreen, Selector, Size, Verb,
    Window as DeskWindow,
};

use coder_desk::serve::{self, Desk, Open, Shape};

const SCREEN: Screen = Screen {
    width: 1280,
    height: 800,
};

#[test]
fn a_window_that_has_not_sized_itself_has_no_fraction() {
    assert_eq!(
        fraction(
            Placed {
                x: 0,
                y: 0,
                width: 0,
                height: 10,
            },
            SCREEN,
        ),
        None
    );
    let asked = fraction(
        Placed {
            x: 128,
            y: 80,
            width: 320,
            height: 640,
        },
        SCREEN,
    )
    .expect("a sized window has a fraction");
    assert!((asked.x - 0.1).abs() < 1e-6);
    assert!((asked.y - 0.1).abs() < 1e-6);
    assert!((asked.w - 0.25).abs() < 1e-6);
    assert!((asked.h - 0.8).abs() < 1e-6);
}

#[test]
fn a_mapped_x11_window_takes_the_focus_and_one_that_is_gone_does_not() {
    // The deck on map and on a click: the layout focuses it, and the
    // keyboard goes to it through `crate::focus`, which sets the X input
    // focus. A window the client destroyed, or one that has not mapped,
    // is skipped, because the X server answers a focus on it with
    // `BadWindow`.
    assert!(takes_focus(true, true));
    assert!(!takes_focus(false, true), "destroyed");
    assert!(!takes_focus(true, false), "not mapped");
    assert!(!takes_focus(false, false));
}

#[test]
fn the_desk_reports_the_second_string_of_the_wm_class_pair() {
    // Each launcher's client, the `WM_CLASS` pair it announces as instance
    // and class, and the class its launcher looks for.
    let launchers = [
        ("os/bin/android-emulator", "qemu-system-x86_64", "Emulator"),
        ("coder-zoom", "zoom", "zoom"),
        ("coder-deck-open", "coder-deck", "coder-deck"),
        ("coder-battlenet", "battle.net.exe", "battle.net.exe"),
    ];
    for (launcher, instance, class) in launchers {
        assert_eq!(desk_class(class, instance), class, "{launcher}");
    }
    assert_eq!(desk_class("", "xeyes"), "xeyes");
}

#[test]
fn an_x11_client_is_mapped_through_the_screens_scale_so_it_draws_at_its_pixels() {
    assert_eq!(client_scale(1.0), 1.0);
    assert_eq!(client_scale(1.25), 1.25);
    assert_eq!(client_scale(2.0), 2.0);
    assert_eq!(client_scale(0.0), 1.0);
    assert_eq!(client_scale(f64::NAN), 1.0);
}

/// A desk holding a Wayland tile and the emulator, which answers the way
/// the compositor does: each row's app-id is what `Coder::app_id` reports.
struct Fixture;

fn row(handle: &str, app_id: String) -> DeskWindow {
    DeskWindow {
        handle: handle.to_string(),
        app_id,
        title: String::new(),
        pid: None,
        screen: "nested-1".to_string(),
        desk: 1,
        at: Point { x: 0, y: 0 },
        size: Size {
            width: 320,
            height: 640,
        },
        floating: true,
        pinned: false,
        fullscreen: false,
    }
}

impl Desk for Fixture {
    fn windows(&mut self) -> Vec<DeskWindow> {
        vec![
            row("0x1", "foot".to_string()),
            row("0x2", desk_class("Emulator", "qemu-system-x86_64")),
        ]
    }
    fn screens(&mut self) -> Vec<DeskScreen> {
        Vec::new()
    }
    fn focused(&mut self) -> Option<DeskWindow> {
        None
    }
    fn open(&mut self, _open: Open) -> Result<(), Refusal> {
        Ok(())
    }
    fn focus(&mut self, _handle: &Selector) -> Result<(), Refusal> {
        Ok(())
    }
    fn place(&mut self, _handle: &Selector, _desk: u32) -> Result<(), Refusal> {
        Ok(())
    }
    fn raise(&mut self, _handle: &Selector) -> Result<(), Refusal> {
        Ok(())
    }
    fn close(&mut self, _handle: &Selector) -> Result<(), Refusal> {
        Ok(())
    }
    fn shape(&mut self, _handle: &Selector, _shape: Shape) -> Result<(), Refusal> {
        Ok(())
    }
    fn scale(&mut self, _screen: &str, _scale: f64) -> Result<(), Refusal> {
        Ok(())
    }
    fn notice(&mut self, _text: &str) -> Result<(), Refusal> {
        Ok(())
    }
    fn reload(&mut self) -> Result<(), Refusal> {
        Ok(())
    }
}

#[test]
fn list_carries_the_class_a_class_selector_names() {
    let answer: Answer = serve::answer(Request::new(Verb::List), &mut Fixture);
    let wire = serde_json::to_string(&answer).expect("the answer encodes");
    let read: Answer = serde_json::from_str(&wire).expect("the answer decodes");
    let Reply::Windows { windows } = read.reply else {
        panic!("the desk answered {:?}", read.reply);
    };
    let Selector::Class(class) = Selector::parse("class:Emulator") else {
        panic!("the selector is not a class");
    };
    let found: Vec<&str> = windows
        .iter()
        .filter(|window| window.app_id == class)
        .map(|window| window.handle.as_str())
        .collect();
    assert_eq!(found, vec!["0x2"]);
}

#[test]
fn each_tty_names_its_own_x11_display() {
    // A session asks for its TTY's number, so two sessions hold different
    // displays whichever order they started in, and a grant such as
    // `game`'s can name one.
    assert_eq!(display_number(":1"), Some(1));
    assert_eq!(display_number(":2"), Some(2));
    assert_eq!(display_number("2"), Some(2));
    assert_ne!(display_number(":1"), display_number(":2"));
}

#[test]
fn a_display_name_the_session_cannot_say_names_none() {
    assert_eq!(display_number(""), None);
    assert_eq!(display_number(":"), None);
    assert_eq!(display_number("tty2"), None);
    assert_eq!(display_number(":-1"), None);
    assert_eq!(display_number("wayland-0"), None);
}

#[test]
fn the_environment_names_the_display() {
    // The only test that touches this variable, so the process-wide
    // environment is not contested.
    let saved = std::env::var("CODER_COMPOSITOR_X11_DISPLAY").ok();
    // SAFETY: no other test reads or writes this variable, and the
    // standard library's own environment lock covers the call itself.
    unsafe { std::env::set_var("CODER_COMPOSITOR_X11_DISPLAY", ":7") };
    let named = wanted_display();
    // SAFETY: as above.
    unsafe {
        match saved {
            Some(value) => std::env::set_var("CODER_COMPOSITOR_X11_DISPLAY", value),
            None => std::env::remove_var("CODER_COMPOSITOR_X11_DISPLAY"),
        }
    }
    assert_eq!(named, Some(7));
}
