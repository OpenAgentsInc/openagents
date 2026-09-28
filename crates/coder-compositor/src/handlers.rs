//! The Wayland protocols the compositor answers.
//!
//! `xdg-shell` toplevels and popups, `wl_shm` buffers, one seat with a
//! keyboard and a pointer, the outputs, the clipboard and the primary
//! selection, drag and drop, server-side decorations, text input, idle
//! notification, fractional scale with the viewporter, and explicit sync
//! on the hardware backend. `wlr-layer-shell` is `layers.rs`,
//! `wlr-screencopy` is `screencopy.rs`, `zwp_virtual_keyboard_v1` is
//! `virtual_keyboard.rs`, and Xwayland with `xwayland-shell` is
//! `xwayland.rs`.

use std::sync::OnceLock;

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::renderer::ImportDma;
use smithay::desktop::{PopupKind, Window};
use smithay::reexports::calloop::Interest;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_data_source::WlDataSource;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::utils::Serial;
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    add_blocker, add_pre_commit_hook, get_parent, is_sync_subsurface, with_states,
    BufferAssignment, CompositorClientState, CompositorHandler, CompositorState,
    SurfaceAttributes,
};
use smithay::wayland::dmabuf::{
    get_dmabuf, DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier,
};
use smithay::wayland::drm_syncobj::{DrmSyncobjCachedState, DrmSyncobjHandler, DrmSyncobjState};
use smithay::wayland::fractional_scale::FractionalScaleHandler;
use smithay::wayland::idle_notify::{IdleNotifierHandler, IdleNotifierState};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::data_device::{
    set_data_device_focus, with_source_metadata, ClientDndGrabHandler, DataDeviceHandler,
    DataDeviceState, ServerDndGrabHandler,
};
use smithay::wayland::selection::primary_selection::{
    set_primary_focus, PrimarySelectionHandler, PrimarySelectionState,
};
use smithay::wayland::selection::{SelectionHandler, SelectionSource, SelectionTarget};
use smithay::wayland::shell::xdg::decoration::XdgDecorationHandler;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::{
    delegate_compositor, delegate_data_device, delegate_dmabuf, delegate_drm_syncobj,
    delegate_fractional_scale, delegate_idle_notify, delegate_output, delegate_primary_selection,
    delegate_seat, delegate_shm, delegate_text_input_manager, delegate_viewporter,
    delegate_xdg_decoration, delegate_xdg_shell, delegate_xwayland_shell,
};
use smithay::wayland::seat::WaylandFocus;
use smithay::xwayland::XWaylandClientData;

use crate::focus::Focus;
use crate::state::{ClientState, Coder, Graphics};

/// The per-client state a client that carries none falls back to. A client
/// without it reached the display through a path this compositor does not
/// open, and it gets an empty state rather than ending the session.
static ORPHAN: OnceLock<CompositorClientState> = OnceLock::new();

impl CompositorHandler for Coder {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        if let Some(state) = client.get_data::<ClientState>() {
            return &state.compositor_state;
        }
        // Xwayland reaches the display through the pair of sockets Smithay
        // opens for it, with a client state of its own.
        if let Some(state) = client.get_data::<XWaylandClientData>() {
            return &state.compositor_state;
        }
        ORPHAN.get_or_init(CompositorClientState::default)
    }

    fn new_surface(&mut self, surface: &WlSurface) {
        // A buffer a client commits before the graphics card has finished
        // drawing it would show half drawn, so the commit waits for it: for
        // the acquire point a client names through explicit sync when it
        // names one, and for the dmabuf to be readable otherwise.
        add_pre_commit_hook::<Self, _>(surface, |state, _display, surface| {
            let (dmabuf, acquire) = with_states(surface, |states| {
                let acquire = states
                    .cached_state
                    .get::<DrmSyncobjCachedState>()
                    .pending()
                    .acquire_point
                    .clone();
                let dmabuf = states
                    .cached_state
                    .get::<SurfaceAttributes>()
                    .pending()
                    .buffer
                    .as_ref()
                    .and_then(|assignment| match assignment {
                        BufferAssignment::NewBuffer(buffer) => get_dmabuf(buffer).cloned().ok(),
                        _ => None,
                    });
                (dmabuf, acquire)
            });
            let Some(dmabuf) = dmabuf else {
                return;
            };
            let Some(client) = surface.client() else {
                return;
            };
            if let Some(acquire) = acquire
                && let Ok((blocker, source)) = acquire.generate_blocker()
            {
                let waiting = client.clone();
                let inserted = state.events.insert_source(source, move |_, _, state| {
                    let display = state.display.clone();
                    state
                        .client_compositor_state(&waiting)
                        .blocker_cleared(state, &display);
                    Ok(())
                });
                if inserted.is_ok() {
                    add_blocker(surface, blocker);
                    return;
                }
            }
            if let Ok((blocker, source)) = dmabuf.generate_blocker(Interest::READ) {
                let inserted = state.events.insert_source(source, move |_, _, state| {
                    let display = state.display.clone();
                    state
                        .client_compositor_state(&client)
                        .blocker_cleared(state, &display);
                    Ok(())
                });
                if inserted.is_ok() {
                    add_blocker(surface, blocker);
                }
            }
        });
    }

    fn commit(&mut self, surface: &WlSurface) {
        smithay::backend::renderer::utils::on_commit_buffer_handler::<Self>(surface);
        if let Graphics::Udev(hardware) = &mut self.graphics {
            hardware.early_import(surface);
        }
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self.window_of_surface(&root) {
                window.on_commit();
            }
        }
        self.popups.commit(surface);
        if self.is_layer_surface(surface) {
            self.configure_new_layer(surface);
            self.refresh_layers();
        }
        self.space.refresh();
    }
}

impl Coder {
    /// The window that draws on this surface: a toplevel, an X11 window in
    /// the layout, or an X11 window that places itself.
    pub fn window_of_surface(&self, surface: &WlSurface) -> Option<Window> {
        self.tiles
            .iter()
            .map(|tile| &tile.window)
            .chain(self.xwayland.unmanaged.iter())
            .find(|window| window.wl_surface().is_some_and(|drawn| *drawn == *surface))
            .cloned()
    }
}

impl OutputHandler for Coder {}

impl DmabufHandler for Coder {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        let imported = match &mut self.graphics {
            Graphics::Winit(backend) => backend
                .borrow_mut()
                .renderer()
                .import_dmabuf(&dmabuf, None)
                .is_ok(),
            Graphics::Udev(hardware) => hardware.import_dmabuf(&dmabuf),
        };
        if imported {
            let _ = notifier.successful::<Coder>();
        } else {
            notifier.failed();
        }
    }
}

impl SelectionHandler for Coder {
    type SelectionUserData = ();

    fn new_selection(
        &mut self,
        target: SelectionTarget,
        source: Option<SelectionSource>,
        _seat: Seat<Self>,
    ) {
        let mimes = source
            .map(|source| source.mime_types().to_vec())
            .unwrap_or_default();
        self.selection.set(target, mimes);
        log::debug!(
            "a client took the {} and offers {}",
            match target {
                SelectionTarget::Clipboard => "clipboard",
                SelectionTarget::Primary => "primary selection",
            },
            self.selection.mimes(target).join(", ")
        );
    }
}

impl DataDeviceHandler for Coder {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl PrimarySelectionHandler for Coder {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}

impl ClientDndGrabHandler for Coder {
    fn started(
        &mut self,
        source: Option<WlDataSource>,
        icon: Option<WlSurface>,
        _seat: Seat<Self>,
    ) {
        let mimes = source
            .as_ref()
            .and_then(|source| with_source_metadata(source, |data| data.mime_types.clone()).ok())
            .unwrap_or_default();
        // The icon draws under the pointer until the drop.
        self.selection.start_drag(mimes, icon.is_some());
        self.drag_icon = icon;
    }

    fn dropped(&mut self, _target: Option<WlSurface>, validated: bool, _seat: Seat<Self>) {
        log::debug!(
            "a drag of {:?} ended, and the target {} it",
            self.selection.drag(),
            if validated { "took" } else { "refused" }
        );
        self.selection.end_drag();
        self.drag_icon = None;
    }
}

impl ServerDndGrabHandler for Coder {
    fn send(&mut self, _mime_type: String, _fd: std::os::unix::io::OwnedFd, _seat: Seat<Self>) {}
}

impl XdgDecorationHandler for Coder {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        self.decorate(&toplevel);
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: DecorationMode) {
        // The compositor draws the border every tile carries, so a client
        // that asks to draw its own is told the server draws it.
        self.decorate(&toplevel);
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        self.decorate(&toplevel);
    }
}

impl IdleNotifierHandler for Coder {
    fn idle_notifier_state(&mut self) -> &mut IdleNotifierState<Self> {
        &mut self.idle_notifier
    }
}

impl Coder {
    /// Tells one client the compositor draws its decorations.
    fn decorate(&mut self, toplevel: &ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(DecorationMode::ServerSide);
        });
        toplevel.send_pending_configure();
    }
}

impl BufferHandler for Coder {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl ShmHandler for Coder {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl SeatHandler for Coder {
    type KeyboardFocus = Focus;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Coder> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&Focus>) {
        // The clipboard and the primary selection follow the keyboard: the
        // client whose surface holds the focus is the one that may read
        // what another client copied. An X11 window's client is Xwayland.
        let client = focused
            .and_then(|focus| focus.wl_surface())
            .and_then(|surface| self.display.get_client(surface.id()).ok());
        set_data_device_focus(&self.display, seat, client.clone());
        set_primary_focus(&self.display, seat, client);
    }

    fn led_state_changed(&mut self, _seat: &Seat<Self>, leds: smithay::input::keyboard::LedState) {
        if let Graphics::Udev(hardware) = &mut self.graphics {
            hardware.set_leds(leds);
        }
    }

    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        image: smithay::input::pointer::CursorImageStatus,
    ) {
        self.cursor_status = image;
    }
}

impl XdgShellHandler for Coder {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let pid = surface
            .wl_surface()
            .client()
            .and_then(|client| client.get_credentials(&self.display).ok())
            .map(|credentials| credentials.pid as i64);
        self.send_first_scale(surface.wl_surface());
        let window = Window::new_wayland_window(surface);
        self.add_window(window, pid);
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let window = self
            .tiles
            .iter()
            .find(|tile| tile.window.toplevel() == Some(&surface))
            .map(|tile| tile.window.clone());
        if let Some(window) = window {
            self.remove_window(&window);
        }
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        if let Err(err) = self.popups.track_popup(PopupKind::Xdg(surface.clone())) {
            log::warn!("a popup was not tracked: {err}");
            return;
        }
        if let Err(err) = surface.send_configure() {
            log::warn!("a popup was not configured: {err}");
        }
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {
        // A popup grab takes the keyboard and closes the popup on a click
        // outside it. The compositor closes a popup when its client closes
        // it, and answers no grab.
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            state.positioner = positioner;
            state.geometry = positioner.get_geometry();
        });
        surface.send_repositioned(token);
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, _output: Option<WlOutput>) {
        let Some(id) = self.id_of_toplevel(&surface) else {
            return;
        };
        // A window whose rule suppresses fullscreen is answered with the
        // configure the layout sent, which keeps it in its tile.
        if self.suppresses_fullscreen(id) {
            surface.send_pending_configure();
            return;
        }
        self.set_fullscreen(id, true);
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        let window = self
            .tiles
            .iter()
            .find(|tile| tile.window.toplevel() == Some(&surface))
            .map(|tile| tile.window.clone());
        if let Some(window) = window {
            self.rules_changed(&window);
        }
    }

    fn app_id_changed(&mut self, surface: ToplevelSurface) {
        self.title_changed(surface);
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        let Some(id) = self.id_of_toplevel(&surface) else {
            return;
        };
        self.set_fullscreen(id, false);
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        // Every tile already fills the rectangle the layout gives it, so a
        // maximize is answered with the configure the layout sent.
        surface.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Maximized);
        });
        surface.send_pending_configure();
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        surface.with_pending_state(|state| {
            state.states.unset(xdg_toplevel::State::Maximized);
        });
        surface.send_pending_configure();
    }
}

impl FractionalScaleHandler for Coder {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        // A surface asks for its scale before the layout places it, so it
        // reads the focused screen's; arranging sends every placed window
        // the scale of the screen it landed on.
        self.send_first_scale(&surface);
    }
}

impl DrmSyncobjHandler for Coder {
    fn drm_syncobj_state(&mut self) -> Option<&mut DrmSyncobjState> {
        self.syncobj_state.as_mut()
    }
}

impl Coder {
    /// The layout identifier one toplevel carries.
    pub fn id_of_toplevel(&self, surface: &ToplevelSurface) -> Option<coder_wm::WinId> {
        self.tiles
            .iter()
            .find(|tile| tile.window.toplevel() == Some(surface))
            .map(|tile| tile.id)
    }
}

delegate_compositor!(Coder);
delegate_data_device!(Coder);
delegate_dmabuf!(Coder);
delegate_drm_syncobj!(Coder);
delegate_fractional_scale!(Coder);
delegate_idle_notify!(Coder);
delegate_primary_selection!(Coder);
delegate_shm!(Coder);
delegate_seat!(Coder);
delegate_output!(Coder);
delegate_text_input_manager!(Coder);
delegate_viewporter!(Coder);
// A key a virtual keyboard sends reaches the focused window under the keymap
// its client uploaded, and Smithay sends the seat's own keymap again before
// the next key from the real keyboard, so a physical press after `wtype`
// reads under the session's layout. A chord it sends runs its bind row, the
// way a chord on the keyboard does: `virtual_keyboard.rs` answers the
// keyboard's own requests and hands the rest to Smithay.
delegate_xdg_decoration!(Coder);
delegate_xdg_shell!(Coder);
delegate_xwayland_shell!(Coder);
