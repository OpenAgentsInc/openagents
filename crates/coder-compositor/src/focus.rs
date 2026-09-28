//! What the keyboard focuses: a Wayland window's surface, or an X11 window
//! with the X server's input focus beside it.
//!
//! Smithay sends the keyboard's enter, leave, keys, and modifiers to the
//! target the compositor names. A `WlSurface` target reaches the surface
//! Xwayland draws an X11 window on, and nothing else: the X server hands a
//! key to the window that holds its own input focus, which a window manager
//! sets with `SetInputFocus` and `WM_TAKE_FOCUS`. The compositor set none,
//! so the deck drew the focus border and read no key until it went
//! fullscreen and covered the whole root window.
//! An
//! `X11Surface` target does both: Smithay's `enter` for it sets the X input
//! focus the way the window's `WM_HINTS` and `WM_PROTOCOLS` ask, and its
//! `leave` takes the focus away.

use std::borrow::Cow;

use smithay::backend::input::KeyState;
use smithay::desktop::{Window, WindowSurface};
use smithay::input::Seat;
use smithay::input::keyboard::{KeyboardTarget, KeysymHandle, ModifiersState};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{IsAlive, Serial};
use smithay::wayland::seat::WaylandFocus;
use smithay::xwayland::X11Surface;

use crate::state::Coder;

/// The target the keyboard's events go to.
#[derive(Clone, Debug, PartialEq)]
pub enum Focus {
    /// A Wayland window's toplevel surface.
    Wayland(WlSurface),
    /// An X11 window, which takes the X input focus with the keyboard.
    X11(X11Surface),
}

impl Focus {
    /// The target one window is. An X11 window that is not mapped or that
    /// the client destroyed has none: the X server answers a focus on it
    /// with `BadWindow`, so the keyboard goes nowhere until the layout
    /// focuses a window that is there.
    pub fn of(window: &Window) -> Option<Focus> {
        match window.underlying_surface() {
            WindowSurface::Wayland(toplevel) => Some(Focus::Wayland(toplevel.wl_surface().clone())),
            WindowSurface::X11(surface) => {
                crate::xwayland::takes_focus(surface.alive(), surface.is_mapped())
                    .then(|| Focus::X11(surface.clone()))
            }
        }
    }
}

impl IsAlive for Focus {
    fn alive(&self) -> bool {
        match self {
            Focus::Wayland(surface) => surface.alive(),
            Focus::X11(surface) => surface.alive(),
        }
    }
}

impl WaylandFocus for Focus {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            Focus::Wayland(surface) => Some(Cow::Borrowed(surface)),
            Focus::X11(surface) => surface.wl_surface().map(Cow::Owned),
        }
    }
}

impl KeyboardTarget<Coder> for Focus {
    fn enter(
        &self,
        seat: &Seat<Coder>,
        data: &mut Coder,
        keys: Vec<KeysymHandle<'_>>,
        serial: Serial,
    ) {
        match self {
            Focus::Wayland(surface) => KeyboardTarget::enter(surface, seat, data, keys, serial),
            Focus::X11(surface) => KeyboardTarget::enter(surface, seat, data, keys, serial),
        }
    }

    fn leave(&self, seat: &Seat<Coder>, data: &mut Coder, serial: Serial) {
        match self {
            Focus::Wayland(surface) => KeyboardTarget::leave(surface, seat, data, serial),
            Focus::X11(surface) => KeyboardTarget::leave(surface, seat, data, serial),
        }
    }

    fn key(
        &self,
        seat: &Seat<Coder>,
        data: &mut Coder,
        key: KeysymHandle<'_>,
        state: KeyState,
        serial: Serial,
        time: u32,
    ) {
        match self {
            Focus::Wayland(surface) => {
                KeyboardTarget::key(surface, seat, data, key, state, serial, time)
            }
            Focus::X11(surface) => {
                KeyboardTarget::key(surface, seat, data, key, state, serial, time)
            }
        }
    }

    fn modifiers(
        &self,
        seat: &Seat<Coder>,
        data: &mut Coder,
        modifiers: ModifiersState,
        serial: Serial,
    ) {
        match self {
            Focus::Wayland(surface) => {
                KeyboardTarget::modifiers(surface, seat, data, modifiers, serial)
            }
            Focus::X11(surface) => {
                KeyboardTarget::modifiers(surface, seat, data, modifiers, serial)
            }
        }
    }
}
