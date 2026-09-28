//! Xwayland: the X11 server the Android emulator, Wine, video clients, and
//! games draw through.
//!
//! The compositor starts Xwayland through Smithay's `xwayland` module the
//! first time it starts a program, and acts as that server's window
//! manager. An X11 window joins the layout the same way an `xdg-shell`
//! toplevel does: it takes a tile, floats, or fills the screen, and the
//! desk protocol lists it with the class the client announces, which is
//! the second string of its `WM_CLASS` pair. Hyprland 0.55 reports the same
//! string, so the `class:` selector in `os/bin/android-emulator`, and the
//! same selectors in the launchers a host flake adds, find the window on
//! either compositor.
//!
//! Every child the compositor starts after that reads the server's display
//! as `DISPLAY`, which is how `xdotool` and the `game` tool reach it.
//!
//! A window that sets override-redirect, such as a menu or a tooltip,
//! places itself. The compositor draws it where it asked and keeps it out
//! of the layout.
//!
//! The rules an X11 window maps under are the rows `crates/coder-binds`
//! holds, read by `crate::rules` when the window maps and again when its
//! class or title changes.

use std::process::Stdio;

use coder_wm::{Rect, WinId};
use smithay::desktop::Window;
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::wayland_server::Client;
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::xwayland_shell::{XWaylandShellHandler, XWaylandShellState};
use smithay::xwayland::xwm::{Reorder, ResizeEdge, WmWindowProperty, XwmId};
use smithay::xwayland::{
    X11Surface, X11Wm, XWayland, XWaylandClientData, XWaylandEvent, XwmHandler,
};

use crate::layout::{Placed, Screen};
use crate::state::Coder;

/// The X11 half of the compositor: the server, its window manager, and the
/// windows that place themselves.
pub struct Xwayland {
    /// The `xwayland_shell_v1` global's state, which pairs an X11 window
    /// with the Wayland surface Xwayland draws it on. Only the Xwayland
    /// client can bind it.
    pub shell: XWaylandShellState,
    /// The window manager, once the server says it is ready.
    pub wm: Option<X11Wm>,
    /// The display the server answers on, from the moment the compositor
    /// starts it.
    pub display: Option<u32>,
    /// Whether the compositor has asked for a server. A server that failed
    /// to start is asked for once, so a host without `Xwayland` logs one
    /// line rather than one for every program.
    pub asked: bool,
    /// The server's source in the loop, which stops the server when the
    /// compositor removes it.
    pub source: Option<RegistrationToken>,
    /// The override-redirect windows the server has mapped.
    pub unmanaged: Vec<Window>,
    /// The server's Wayland client, whose scale follows the screens'.
    pub client: Option<Client>,
}

impl Xwayland {
    /// The X11 half of a compositor that has not started a server yet.
    pub fn new(shell: XWaylandShellState) -> Xwayland {
        Xwayland {
            shell,
            wm: None,
            display: None,
            asked: false,
            source: None,
            unmanaged: Vec::new(),
            client: None,
        }
    }
}

/// Whether an X11 window takes the keyboard, and the X input focus with
/// it. A window the client destroyed or has not mapped takes neither: the
/// X server answers a focus or a raise on it with `BadWindow`, which is
/// what the log printed while a Wine window came and went.
pub fn takes_focus(alive: bool, mapped: bool) -> bool {
    alive && mapped
}

/// The class the desk protocol reports for an X11 window.
///
/// `WM_CLASS` holds two strings, the instance and then the class, and
/// Smithay reads them as `instance` and `class`. The class is the string
/// Hyprland 0.55 reports and the launchers match. A client that sets only
/// an instance is reported by its instance, so it is still listed with a
/// name.
pub fn desk_class(class: &str, instance: &str) -> String {
    if class.is_empty() {
        instance.to_string()
    } else {
        class.to_string()
    }
}

/// The scale the compositor maps the Xwayland client's coordinates
/// through, for the largest scale a screen draws at.
///
/// This is Hyprland's `xwayland:force_zero_scaling`. Xwayland has one
/// scale for every window it draws, and a client mapped at 1 on a screen
/// at 1.25 draws a window of 1,000 pixels that the screen stretches to
/// 1,250, which is what blurs the emulator and the games. Mapped through
/// the screen's own scale, the client's coordinates are the screen's
/// pixels: an X11 program draws at the screen's pixels, sharp, and looks
/// smaller by the scale.
pub fn client_scale(screen_scale: f64) -> f64 {
    if screen_scale.is_finite() && screen_scale > 0.0 {
        screen_scale
    } else {
        1.0
    }
}

/// The display number the session asks Xwayland to take, or `None` to
/// take the first free one.
///
/// `coder-compositor-session` writes the session's TTY number into
/// `CODER_COMPOSITOR_X11_DISPLAY`, spelled as X displays are (`:2`).
/// Two sessions on one host hold different numbers whichever order they
/// started in, so a grant such as `game`'s can name one session's display
/// rather than whichever session reached an X11 client first.
///
/// The variable is the only source. `XDG_VTNR` looks like the same fact
/// but is not: it survives into the terminals a session opens, so a
/// compositor nested in a window would read a TTY it does not own and
/// could take the number before the TTY's own session asks for it. A
/// compositor with no named display takes the first free one, as it
/// always has.
fn wanted_display() -> Option<u32> {
    display_number(&std::env::var("CODER_COMPOSITOR_X11_DISPLAY").ok()?)
}

/// The number a display name holds, spelled as X spells it (`:2`) or
/// bare (`2`).
fn display_number(raw: &str) -> Option<u32> {
    raw.trim().trim_start_matches(':').parse().ok()
}

/// A window's rectangle in pixels as a fraction of the screen, which is how
/// the layout holds a float. A window that has not sized itself yet has no
/// fraction.
pub fn fraction(geometry: Placed, screen: Screen) -> Option<Rect> {
    if geometry.width <= 0 || geometry.height <= 0 {
        return None;
    }
    let width = screen.width.max(1) as f32;
    let height = screen.height.max(1) as f32;
    Some(Rect {
        x: geometry.x as f32 / width,
        y: geometry.y as f32 / height,
        w: geometry.width as f32 / width,
        h: geometry.height as f32 / height,
    })
}

impl Coder {
    /// Starts Xwayland, unless the compositor has asked for it already.
    ///
    /// The display number is known before the server is ready, because
    /// Smithay binds the X11 sockets first, so the program the compositor
    /// starts next reads `DISPLAY` and its connection waits for the
    /// server.
    ///
    /// The server takes the number [`wanted_display`] asks for — the
    /// session's TTY — so a host's sessions each hold their own display
    /// whichever order they started in, and a grant such as `game`'s can
    /// name one. A session that cannot take its number, or names none,
    /// takes the first free one.
    pub fn start_xwayland(&mut self) {
        if self.xwayland.asked {
            return;
        }
        self.xwayland.asked = true;
        let spawn = |display: Option<u32>| {
            XWayland::spawn(
                &self.display,
                display,
                std::iter::empty::<(String, String)>(),
                true,
                Stdio::null(),
                Stdio::null(),
                |_| (),
            )
        };
        let spawned = match wanted_display() {
            Some(number) => spawn(Some(number)).or_else(|err| {
                log::warn!(
                    "Xwayland did not take :{number}, which this session's TTY names: {err}. \
                     Taking the first free display instead."
                );
                spawn(None)
            }),
            None => spawn(None),
        };
        let (server, client) = match spawned {
            Ok(spawned) => spawned,
            Err(err) => {
                log::warn!(
                    "Xwayland did not start, so X11 programs have no server here: {err}. \
                     The compositor looks for `Xwayland` on PATH."
                );
                return;
            }
        };
        let number = server.display_number();
        let events = self.events.clone();
        let inserted = events.insert_source(server, move |event, _, state| match event {
            XWaylandEvent::Ready {
                x11_socket,
                display_number,
            } => state.xwayland_ready(x11_socket, display_number, &client),
            XWaylandEvent::Error => {
                log::warn!("Xwayland exited before it was ready");
                state.xwayland_gone();
            }
        });
        match inserted {
            Ok(token) => {
                self.xwayland.source = Some(token);
                self.xwayland.display = Some(number);
                self.session.x11_display = Some(format!(":{number}"));
                log::info!("Xwayland is starting on :{number}");
            }
            Err(err) => log::warn!("Xwayland's source did not join the loop: {err}"),
        }
    }

    /// Takes the window manager's role on a server that is ready.
    fn xwayland_ready(
        &mut self,
        socket: std::os::unix::net::UnixStream,
        number: u32,
        client: &Client,
    ) {
        self.xwayland.client = Some(client.clone());
        self.set_xwayland_scale();
        if client.get_data::<XWaylandClientData>().is_none() {
            log::warn!("the Xwayland client carries no client state, so its scale stays at 1");
        }
        match X11Wm::start_wm(self.events.clone(), socket, client.clone()) {
            Ok(wm) => {
                self.xwayland.wm = Some(wm);
                log::info!("Xwayland is ready on :{number}");
            }
            Err(err) => {
                log::warn!("the compositor did not become Xwayland's window manager: {err}");
                self.xwayland_gone();
            }
        }
    }

    /// Forgets a server that exited, and every window it drew, so the next
    /// program the compositor starts asks for a new one.
    fn xwayland_gone(&mut self) {
        let windows: Vec<Window> = self
            .tiles
            .iter()
            .filter(|tile| tile.window.x11_surface().is_some())
            .map(|tile| tile.window.clone())
            .collect();
        for window in windows {
            self.remove_window(&window);
        }
        for window in std::mem::take(&mut self.xwayland.unmanaged) {
            self.space.unmap_elem(&window);
        }
        if let Some(token) = self.xwayland.source.take() {
            self.events.remove(token);
        }
        self.xwayland.wm = None;
        self.xwayland.client = None;
        self.xwayland.display = None;
        self.xwayland.asked = false;
        self.session.x11_display = None;
    }

    /// The layout identifier one X11 window carries.
    pub fn id_of_x11(&self, surface: &X11Surface) -> Option<WinId> {
        self.tiles
            .iter()
            .find(|tile| tile.window.x11_surface() == Some(surface))
            .map(|tile| tile.id)
    }

    /// The rectangle the layout gives one window, on the desk the focused
    /// screen shows, in the shared space.
    pub fn rect_of_window(&self, id: WinId) -> Option<Rectangle<i32, Logical>> {
        let desk = self.manager.workspace() + 1;
        let tile = self
            .manager
            .tiles()
            .into_iter()
            .find(|tile| tile.id == id)?;
        let placed = self.placed(desk, tile);
        Some(Rectangle::new(
            (placed.x, placed.y).into(),
            (placed.width, placed.height).into(),
        ))
    }

    /// Adds one X11 window to the layout under the rules its class and
    /// title select.
    fn manage_x11(&mut self, surface: X11Surface) {
        if let Err(err) = surface.set_mapped(true) {
            log::warn!("an X11 window did not map: {err}");
            return;
        }
        let geometry = surface.geometry();
        let (origin, screen) = self
            .screens
            .focused()
            .map(|head| (head.at, head.logical()))
            .unwrap_or((
                (0, 0),
                Screen {
                    width: 1,
                    height: 1,
                },
            ));
        let asked = fraction(
            Placed {
                x: geometry.loc.x - origin.0,
                y: geometry.loc.y - origin.1,
                width: geometry.size.w,
                height: geometry.size.h,
            },
            screen,
        );
        let pid = surface.pid().map(i64::from);
        let window = Window::new_x11_window(surface);
        let id = self.add_window(window, pid);
        self.read_rules(id, asked);
        self.after_layout();
    }

    /// Drops one X11 window from the layout or from the windows that place
    /// themselves.
    fn forget_x11(&mut self, surface: &X11Surface) {
        let tile = self
            .tiles
            .iter()
            .find(|tile| tile.window.x11_surface() == Some(surface))
            .map(|tile| tile.window.clone());
        if let Some(window) = tile {
            self.remove_window(&window);
        }
        let unmanaged = self
            .xwayland
            .unmanaged
            .iter()
            .position(|window| window.x11_surface() == Some(surface));
        if let Some(index) = unmanaged {
            let window = self.xwayland.unmanaged.remove(index);
            self.space.unmap_elem(&window);
        }
    }

    /// Sends one X11 window the rectangle the layout gives it, which is the
    /// answer to a client that asked for another.
    fn configure_to_layout(&self, surface: &X11Surface) -> bool {
        let Some(id) = self.id_of_x11(surface) else {
            return false;
        };
        if let Some(rect) = self.rect_of_window(id)
            && let Err(err) = surface.configure(rect)
        {
            log::debug!("an X11 window did not take its tile: {err}");
        }
        true
    }
}

impl XWaylandShellHandler for Coder {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland.shell
    }
}

impl XwmHandler for Coder {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        // Smithay calls this from the sources the window manager itself
        // registered when it started, and the compositor stores the manager
        // in the same pass, before the loop dispatches any of them.
        match self.xwayland.wm.as_mut() {
            Some(wm) => wm,
            None => unreachable!("an X11 event arrived before the window manager started"),
        }
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if self.id_of_x11(&window).is_some() {
            return;
        }
        self.manage_x11(window);
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let at = window.geometry().loc;
        let element = Window::new_x11_window(window);
        self.space.map_element(element.clone(), at, true);
        self.xwayland.unmanaged.push(element);
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.forget_x11(&window);
        if !window.is_override_redirect()
            && let Err(err) = window.set_mapped(false)
        {
            log::debug!("an X11 window did not unmap: {err}");
        }
    }

    fn destroyed_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.forget_x11(&window);
    }

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        // A window the layout holds is answered with the rectangle the
        // layout gives it. The layout crate moves a float too, through
        // `shape` and the chords, so a float is answered the same way.
        if self.configure_to_layout(&window) {
            return;
        }
        // A window that has not mapped yet sizes itself, and the layout
        // reads that size when it floats the window.
        let mut geometry = window.geometry();
        if let Some(x) = x {
            geometry.loc.x = x;
        }
        if let Some(y) = y {
            geometry.loc.y = y;
        }
        if let Some(w) = w {
            geometry.size.w = w as i32;
        }
        if let Some(h) = h {
            geometry.size.h = h as i32;
        }
        if let Err(err) = window.configure(geometry) {
            log::debug!("an X11 window did not take the size it asked for: {err}");
        }
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        geometry: Rectangle<i32, Logical>,
        _above: Option<u32>,
    ) {
        let unmanaged = self
            .xwayland
            .unmanaged
            .iter()
            .find(|held| held.x11_surface() == Some(&window))
            .cloned();
        if let Some(element) = unmanaged {
            self.space.map_element(element, geometry.loc, false);
        }
    }

    fn fullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let Some(id) = self.id_of_x11(&window) else {
            return;
        };
        if self.suppresses_fullscreen(id) {
            self.configure_to_layout(&window);
            return;
        }
        if let Err(err) = window.set_fullscreen(true) {
            log::debug!("an X11 window was not told it fills the screen: {err}");
        }
        self.set_fullscreen(id, true);
    }

    fn unfullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let Some(id) = self.id_of_x11(&window) else {
            return;
        };
        if let Err(err) = window.set_fullscreen(false) {
            log::debug!("an X11 window was not told it left fullscreen: {err}");
        }
        self.set_fullscreen(id, false);
    }

    fn property_notify(&mut self, _xwm: XwmId, window: X11Surface, property: WmWindowProperty) {
        // A Wine window announces the game's title after it maps, so the
        // rules are read again when the title or the class changes.
        if !matches!(property, WmWindowProperty::Title | WmWindowProperty::Class) {
            return;
        }
        let held = self
            .tiles
            .iter()
            .find(|tile| tile.window.x11_surface() == Some(&window))
            .map(|tile| tile.window.clone());
        if let Some(held) = held {
            self.rules_changed(&held);
        }
    }

    fn maximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        // Every tile already fills the rectangle the layout gives it, which
        // is the answer to a maximize, and a game's rule asks for no more.
        self.configure_to_layout(&window);
    }

    fn unmaximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.configure_to_layout(&window);
    }

    fn resize_request(
        &mut self,
        _xwm: XwmId,
        _window: X11Surface,
        _button: u32,
        _edge: ResizeEdge,
    ) {
        // The layout sizes every window, through the chords and `shape`.
    }

    fn move_request(&mut self, _xwm: XwmId, _window: X11Surface, _button: u32) {
        // The layout places every window, through the chords and `shape`.
    }

    fn disconnected(&mut self, _xwm: XwmId) {
        log::warn!("Xwayland closed its connection to the window manager");
        self.xwayland_gone();
    }
}

#[cfg(test)]
#[path = "xwayland_tests.rs"]
mod tests;
