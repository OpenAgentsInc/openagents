//! The compositor's state: the Wayland globals, the layout, and the windows
//! the two of them agree on.
//!
//! The layout crate owns the tree, the nine desks, the floats, and the
//! fullscreen window, and returns rectangles in 0..1. This module holds the
//! window each of its identifiers stands for, turns the rectangles into
//! pixels, and configures the clients to them.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use coder_wm::{Dir, Manager, WinId};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::winit::WinitGraphicsBackend;
use smithay::desktop::space::SpaceElement;
use smithay::desktop::{PopupManager, Space, Window, WindowSurfaceType};
use smithay::input::keyboard::KeyboardHandle;
use smithay::input::pointer::{CursorImageStatus, PointerHandle};
use smithay::input::{Seat, SeatState};
use smithay::output::Output;
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, SERIAL_COUNTER, Serial};
use smithay::wayland::compositor::{CompositorClientState, CompositorState, with_states};
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufState};
use smithay::wayland::drm_syncobj::DrmSyncobjState;
use smithay::wayland::fractional_scale::FractionalScaleManagerState;
use smithay::wayland::idle_notify::IdleNotifierState;
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::pointer_constraints::PointerConstraintsState;
use smithay::wayland::relative_pointer::RelativePointerManagerState;
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::wayland::shell::wlr_layer::{Layer, WlrLayerShellState};
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shell::xdg::{XdgShellState, XdgToplevelSurfaceData};
use smithay::wayland::shm::ShmState;
use smithay::wayland::text_input::TextInputManagerState;
use smithay::wayland::viewporter::ViewporterState;
use smithay::wayland::virtual_keyboard::VirtualKeyboardManagerState;
use smithay::wayland::xwayland_shell::XWaylandShellState;

use crate::binds::{self, Action, Chord};
use crate::exec::{self, Session};
use crate::focus::Focus;
use crate::hands::{self, Hands};
use crate::idle::Activity;
use crate::layout::{self, Fill, Placed};
use crate::render::{Decor, Overlay};
use crate::screencopy::{self, Screencopy};
use crate::screens::{self, Head, Screens};
use crate::selection::Selection;
use crate::stacking::{self, Layer as Stack};
use crate::xwayland::{self, Xwayland};

/// One window the compositor holds: the layout crate's identifier for it,
/// the Wayland side of it, and what the desk protocol reports about it.
pub struct Tile {
    /// The layout crate's identifier, which is also the desk handle.
    pub id: WinId,
    /// The window the client draws.
    pub window: Window,
    /// The process that owns the window, when the client's credentials
    /// carried one.
    pub pid: Option<i64>,
    /// What the rule table said the last time the compositor read it for
    /// this window, so a read that says the same moves nothing.
    pub rule: coder_binds::Effects,
    /// The effects in force: every rule read folded together, with what
    /// the desk protocol's `shape` set on top.
    pub effects: coder_binds::Effects,
}

/// The state every Wayland handler and the desk socket act on.
pub struct Coder {
    /// The display every global lives on.
    pub display: DisplayHandle,
    /// The `wl_compositor` global's state.
    pub compositor_state: CompositorState,
    /// The `xdg_wm_base` global's state.
    pub xdg_shell_state: XdgShellState,
    /// The `wl_shm` global's state.
    pub shm_state: ShmState,
    /// The `zwlr_layer_shell_v1` global's state, which holds the anchored
    /// surfaces a notification daemon and a panel draw on.
    pub layer_shell_state: WlrLayerShellState,
    /// The `wl_data_device_manager` global's state, which carries the
    /// clipboard and a drag between clients.
    pub data_device_state: DataDeviceState,
    /// The `zwp_primary_selection_device_manager_v1` global's state, which
    /// carries the middle-click selection.
    pub primary_selection_state: PrimarySelectionState,
    /// The `zxdg_decoration_manager_v1` global's state. The compositor
    /// draws the border, so it answers every client server-side.
    #[allow(dead_code)]
    pub decoration_state: XdgDecorationState,
    /// The `zwp_text_input_manager_v3` global's state, which an input
    /// method and a client that takes text agree on.
    #[allow(dead_code)]
    pub text_input_state: TextInputManagerState,
    /// The `ext_idle_notifier_v1` global's state, which holds one timer for
    /// each timeout a client asked about.
    pub idle_notifier: IdleNotifierState<Coder>,
    /// How recently the compositor reported input to the idle notifier.
    pub activity: Activity,
    /// The `zwlr_screencopy_manager_v1` global and the copies waiting for a
    /// frame.
    pub screencopy: Screencopy,
    /// What the seat carries between clients: the clipboard, the primary
    /// selection, and a drag in progress.
    pub selection: Selection,
    /// The `zwp_linux_dmabuf_v1` global's state. A client that draws
    /// through Vulkan gets no surface from a compositor that advertises no
    /// dmabuf.
    pub dmabuf_state: DmabufState,
    /// The global that announces `zwp_linux_dmabuf_v1`, once the backend
    /// knows the device its renderer allocates on.
    pub dmabuf_global: Option<DmabufGlobal>,
    /// The `wp_linux_drm_syncobj_manager_v1` global's state, which carries
    /// explicit sync. The hardware backend announces it when the device
    /// supports it; the nested backend holds none.
    pub syncobj_state: Option<DrmSyncobjState>,
    /// The `wp_fractional_scale_manager_v1` global's state, which tells a
    /// client the scale of the screen it draws on.
    #[allow(dead_code)]
    pub fractional_scale_state: FractionalScaleManagerState,
    /// The `wp_viewporter` global's state, which a client drawing at a
    /// fractional scale sizes its buffer through.
    #[allow(dead_code)]
    pub viewporter_state: ViewporterState,
    /// The `zwp_virtual_keyboard_manager_v1` global's state. `wtype` makes a
    /// keyboard through it, which is how `os/bin/dictate-toggle` types a
    /// transcript into the focused tile.
    #[allow(dead_code)]
    pub virtual_keyboard_state: VirtualKeyboardManagerState,
    /// The backend's renderer, which the dmabuf handler imports a client's
    /// buffer into and the frame draws with.
    pub graphics: Graphics,
    /// The `wl_output` and `xdg_output` globals' state, which holds the
    /// globals open for as long as the compositor runs.
    #[allow(dead_code)]
    pub output_state: OutputManagerState,
    /// The seat's state, which holds the keyboard and the pointer.
    pub seat_state: SeatState<Coder>,
    /// The one seat, named for the backend it reads input from. The
    /// keyboard and the pointer below are its handles, and the seat itself
    /// holds the `wl_seat` global open.
    #[allow(dead_code)]
    pub seat: Seat<Coder>,
    /// The keyboard the seat holds.
    pub keyboard: KeyboardHandle<Coder>,
    /// The pointer the seat holds.
    pub pointer: PointerHandle<Coder>,
    /// Where the pointer sits in the space every screen shares.
    pub pointer_at: Point<f64, Logical>,
    /// The cursor the client under the pointer asked for.
    pub cursor_status: CursorImageStatus,
    /// The surface a drag in progress carries under the pointer.
    pub drag_icon: Option<WlSurface>,
    /// The Super drag in progress, which moves or sizes the window it
    /// holds until the button comes up.
    pub drag: Option<crate::drag::Drag>,
    /// The pointer's images and the solid bars the compositor draws.
    pub decor: Decor,
    /// The windows the renderer draws, in the order it draws them.
    pub space: Space<Window>,
    /// The popups each window holds.
    pub popups: PopupManager,
    /// The `wl_output` for each screen, in no particular order.
    pub outputs: Vec<Output>,
    /// The screens, left to right, and the desk each one shows.
    pub screens: Screens,
    /// The layout: the tree, the nine desks, the floats, and fullscreen.
    pub manager: Manager,
    /// The window each layout identifier stands for.
    pub tiles: Vec<Tile>,
    /// What an `open` asked for a window the compositor has not seen yet,
    /// matched to the window by the process the compositor started.
    pub pending: Vec<crate::desk_server::Pending>,
    /// The window that fills its screen on each desk, which the layout
    /// crate reports with the same full rectangle it gives a maximized
    /// window. The crate gains the distinction when the screens move into
    /// it.
    pub fullscreen: [Option<WinId>; 9],
    /// Every chord the compositor answers.
    pub binds: Vec<(Chord, Action)>,
    /// The order the windows the screens show drew in the last time the
    /// stack changed, back to front, so the log says it once a change.
    pub stack: Vec<WinId>,
    /// The window the keyboard went to the last time the focus was set, so
    /// a focus that stays where it is raises nothing: a float goes over the
    /// other floats of its layer when it takes the focus, and stays where a
    /// `raise` put it after that.
    pub focused: Option<WinId>,
    /// What the desk protocol's drive verbs keep: the keys this session's
    /// layout presses, and the screenshots waiting for a frame.
    pub drive: crate::drive::Driver,
    /// The virtual keyboards the clients of this session made, and the
    /// keymap each one uploaded, so a chord it sends runs the bind table.
    pub virtual_keyboards: crate::virtual_keyboard::Keyboards,
    /// What the two `exec` rows run, and what a child's environment holds.
    pub session: Session,
    /// What the close chord asked about last, so the press after a notice
    /// closes the window the notice was raised for.
    pub closing: crate::closing::Closing,
    /// The notice the compositor shows the operator, raised by the close
    /// chord and by the desk protocol's `notice`.
    pub notices: crate::closing::Notices,
    /// The loop runs until a chord, the nested window's close, or a signal
    /// ends it.
    pub running: bool,
    /// When the compositor started, which frame callbacks count from.
    pub started: Instant,
    /// The loop the idle timers and the Xwayland sources run on.
    pub events: LoopHandle<'static, Coder>,
    /// The X11 server, its window manager, and the windows that place
    /// themselves.
    pub xwayland: Xwayland,
    /// The hand that drives the desk: the reader on the camera daemon's
    /// socket, the gesture machine, and what the overlay draws.
    pub hands: Hands,
}

/// What one client of this compositor carries.
#[derive(Default)]
pub struct ClientState {
    /// The per-client half of the compositor global's state.
    pub compositor_state: CompositorClientState,
}

impl smithay::reexports::wayland_server::backend::ClientData for ClientState {
    fn initialized(&self, _id: smithay::reexports::wayland_server::backend::ClientId) {}
    fn disconnected(
        &self,
        _id: smithay::reexports::wayland_server::backend::ClientId,
        _reason: smithay::reexports::wayland_server::backend::DisconnectReason,
    ) {
    }
}

/// The renderer each backend draws with.
pub enum Graphics {
    /// The nested window. Everything in this crate runs on one thread, so
    /// the loop and the dmabuf handler share it through a cell.
    Winit(Rc<RefCell<WinitGraphicsBackend<GlesRenderer>>>),
    /// The seat, the devices, and the connectors.
    Udev(Box<crate::udev::Hardware>),
}

/// What both backends build the state from.
pub struct Parts {
    /// The display every global lives on.
    pub display: DisplayHandle,
    /// The loop the timers and the Xwayland sources run on.
    pub events: LoopHandle<'static, Coder>,
    /// The backend's renderer.
    pub graphics: Graphics,
    /// The seat's name.
    pub seat: String,
    /// What the `exec` rows run and what a child's environment holds.
    pub session: Session,
}

impl Coder {
    /// The state both backends run on, with every global announced but the
    /// outputs and the dmabuf global, which the backend adds once it knows
    /// its screens and its device.
    pub fn new(parts: Parts) -> Result<Coder, String> {
        let Parts {
            display,
            events,
            graphics,
            seat,
            session,
        } = parts;
        // The pointer a client holds while it steers by motion, and the delta
        // it reads instead of a position. The display owns a global once it is
        // created, so there is no handle to keep: `crate::constraints` holds
        // the policy and reads the constraint off the surface.
        PointerConstraintsState::new::<Coder>(&display);
        RelativePointerManagerState::new::<Coder>(&display);

        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&display, seat);
        let keyboard_layout = crate::keys::Layout::from_environment();
        let keyboard = seat
            .add_keyboard(keyboard_layout.config(), 200, 25)
            .map_err(|err| format!("the seat has no keyboard: {err}"))?;
        let pointer = seat.add_pointer();
        log::info!("the keyboard layout is {}", keyboard_layout.named());
        // A host that granted hands starts with them on; a run by hand
        // starts with them off and Super+H turns them on.
        let mut hands_state = Hands::from_environment();
        let granted = session
            .launchers
            .as_ref()
            .is_some_and(|list| list.iter().any(|name| name == hands::OPTION));
        if granted {
            hands_state.turn_on();
        }
        Ok(Coder {
            display: display.clone(),
            compositor_state: CompositorState::new::<Coder>(&display),
            xdg_shell_state: XdgShellState::new::<Coder>(&display),
            shm_state: ShmState::new::<Coder>(&display, screencopy::SHM_FORMATS.to_vec()),
            layer_shell_state: WlrLayerShellState::new::<Coder>(&display),
            data_device_state: DataDeviceState::new::<Coder>(&display),
            primary_selection_state: PrimarySelectionState::new::<Coder>(&display),
            decoration_state: XdgDecorationState::new::<Coder>(&display),
            text_input_state: TextInputManagerState::new::<Coder>(&display),
            idle_notifier: IdleNotifierState::new(&display, events.clone()),
            activity: Activity::default(),
            screencopy: Screencopy::new(&display),
            selection: Selection::default(),
            dmabuf_state: DmabufState::new(),
            dmabuf_global: None,
            syncobj_state: None,
            fractional_scale_state: FractionalScaleManagerState::new::<Coder>(&display),
            viewporter_state: ViewporterState::new::<Coder>(&display),
            // Every client of this socket may make a virtual keyboard, the
            // way every client of a Hyprland session may: the socket is
            // the session's own, and a program that reaches it types with
            // the same standing as one that is typed into.
            virtual_keyboard_state: VirtualKeyboardManagerState::new::<Coder, _>(
                &display,
                |_client| true,
            ),
            graphics,
            output_state: OutputManagerState::new_with_xdg_output::<Coder>(&display),
            seat_state,
            seat,
            keyboard,
            pointer,
            pointer_at: (0.0, 0.0).into(),
            cursor_status: CursorImageStatus::default_named(),
            drag_icon: None,
            drag: None,
            decor: Decor::load(),
            space: Space::default(),
            popups: PopupManager::default(),
            outputs: Vec::new(),
            screens: Screens::default(),
            manager: Manager::new(),
            tiles: Vec::new(),
            pending: Vec::new(),
            fullscreen: [None; 9],
            binds: binds::table(session.launchers.as_deref()),
            stack: Vec::new(),
            focused: None,
            drive: Default::default(),
            virtual_keyboards: Default::default(),
            session,
            closing: Default::default(),
            notices: Default::default(),
            running: true,
            started: Instant::now(),
            events,
            xwayland: Xwayland::new(XWaylandShellState::new::<Coder>(&display)),
            hands: hands_state,
        })
    }
}

impl Coder {
    /// The tile one layout identifier stands for.
    pub fn tile(&self, id: WinId) -> Option<&Tile> {
        self.tiles.iter().find(|tile| tile.id == id)
    }

    /// The topmost window under `point` and where its surface is drawn,
    /// as `Space::element_under` finds it, except that a window takes the
    /// pointer only inside its tile, where it draws. A client that commits
    /// a buffer larger than its tile, such as a terminal whose text grew,
    /// leaves the pointer to the neighbour it would otherwise cover. A
    /// popup can open past the tile's edge and still takes the pointer.
    pub fn window_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(Window, Point<i32, Logical>)> {
        let every = self.manager.all_tiles();
        self.space.elements().rev().find_map(|window| {
            let bbox = self.space.element_bbox(window)?;
            if !bbox.to_f64().contains(point) {
                return None;
            }
            let at = self.space.element_location(window)? - window.geometry().loc;
            let local = point - at.to_f64();
            if !window.is_in_input_region(&local) {
                return None;
            }
            let tile = self
                .tile_of(window)
                .and_then(|tile| every.iter().find(|(_, held)| held.id == tile.id))
                .map(|(desk, tile)| self.placed(*desk, *tile));
            if let Some(tile) = tile {
                let inside = layout::contains(tile, point.x, point.y);
                let on_popup = window
                    .surface_under(local, WindowSurfaceType::POPUP)
                    .is_some();
                if !inside && !on_popup {
                    return None;
                }
            }
            Some((window.clone(), at))
        })
    }

    /// The tile one window stands for.
    pub fn tile_of(&self, window: &Window) -> Option<&Tile> {
        self.tiles.iter().find(|tile| &tile.window == window)
    }

    /// The tile one layout identifier stands for, to change.
    pub fn tile_mut(&mut self, id: WinId) -> Option<&mut Tile> {
        self.tiles.iter_mut().find(|tile| tile.id == id)
    }

    /// The app-id the client set, or an empty string until it sets one.
    /// An X11 window reports its class, the second string of its
    /// `WM_CLASS` pair.
    pub fn app_id(&self, id: WinId) -> String {
        if let Some(surface) = self.tile(id).and_then(|tile| tile.window.x11_surface()) {
            return xwayland::desk_class(&surface.class(), &surface.instance());
        }
        self.attribute(id, |data| data.app_id.clone())
    }

    /// The title the client set, or an empty string until it sets one.
    pub fn title(&self, id: WinId) -> String {
        if let Some(surface) = self.tile(id).and_then(|tile| tile.window.x11_surface()) {
            return surface.title();
        }
        self.attribute(id, |data| data.title.clone())
    }

    fn attribute(
        &self,
        id: WinId,
        read: impl Fn(&smithay::wayland::shell::xdg::XdgToplevelSurfaceRoleAttributes) -> Option<String>,
    ) -> String {
        let Some(tile) = self.tile(id) else {
            return String::new();
        };
        let Some(toplevel) = tile.window.toplevel() else {
            return String::new();
        };
        with_states(toplevel.wl_surface(), |states| {
            let Some(data) = states.data_map.get::<XdgToplevelSurfaceData>() else {
                return String::new();
            };
            match data.lock() {
                Ok(attributes) => read(&attributes).unwrap_or_default(),
                Err(_) => String::new(),
            }
        })
    }

    /// Whether a window fills its screen rather than the layout's area.
    pub fn fills_screen(&self, desk: usize, id: WinId) -> bool {
        desk.checked_sub(1)
            .and_then(|index| self.fullscreen.get(index))
            .is_some_and(|held| *held == Some(id))
    }

    /// The screen a desk, numbered 1 through 9, is measured against: the
    /// screen that shows it, or the focused screen when none does.
    pub fn home(&self, desk: usize) -> Option<&Head> {
        self.screens.home_of(desk.saturating_sub(1))
    }

    /// The size in logical pixels of the screen a desk is measured
    /// against, or a screen of one pixel when no screen is connected.
    pub fn home_size(&self, desk: usize) -> layout::Screen {
        self.home(desk)
            .map(Head::logical)
            .unwrap_or(layout::Screen {
                width: 1,
                height: 1,
            })
    }

    /// Where one tile of one desk sits in logical pixels in the space every
    /// screen shares.
    pub fn placed(&self, desk: usize, tile: coder_wm::Tile) -> Placed {
        let fill = if self.fills_screen(desk, tile.id) {
            Fill::Screen
        } else {
            Fill::Tiled
        };
        match self.home(desk) {
            Some(head) => head.place(tile.rect, fill),
            None => layout::place(
                tile.rect,
                self.home_size(desk),
                layout::whole(self.home_size(desk)),
                fill,
            ),
        }
    }

    /// The layout crate's rectangle that puts one window of one desk at
    /// `target`, in logical pixels in the space every screen shares: the
    /// inverse of [`Coder::placed`] for a float the desk protocol shapes.
    pub fn normalized(&self, desk: usize, target: Placed) -> coder_wm::Rect {
        match self.home(desk) {
            Some(head) => head.normalize(target),
            None => layout::normalize(target, layout::whole(self.home_size(desk))),
        }
    }

    /// The `wl_output` of the screen one name names.
    pub fn output_named(&self, name: &str) -> Option<&Output> {
        self.outputs.iter().find(|output| output.name() == name)
    }

    /// The `wl_output` of the focused screen.
    pub fn focused_output(&self) -> Option<&Output> {
        self.screens
            .focused()
            .and_then(|head| self.output_named(&head.name))
    }

    /// Adds a toplevel to the layout and gives it the focus.
    pub fn add_window(&mut self, window: Window, pid: Option<i64>) -> WinId {
        let id = self.manager.spawn();
        self.tiles.push(Tile {
            id,
            window,
            pid,
            rule: coder_binds::Effects::default(),
            effects: coder_binds::Effects::default(),
        });
        let asked = pid.and_then(|pid| self.take_pending(pid));
        if let Some(asked) = asked {
            if let Some(desk) = asked.desk {
                let home = self.manager.workspace();
                self.manager.movetoworkspace(desk as usize - 1);
                self.manager.switch_workspace(home);
            }
            if asked.silent {
                self.arrange();
                return id;
            }
        }
        self.arrange();
        self.focus_layout_window();
        id
    }

    /// What an `open` asked for the window one process owns.
    fn take_pending(&mut self, pid: i64) -> Option<crate::desk_server::Pending> {
        let index = self
            .pending
            .iter()
            .position(|asked| i64::from(asked.pid) == pid)?;
        Some(self.pending.remove(index))
    }

    /// Drops a window from the layout and from the renderer.
    pub fn remove_window(&mut self, window: &Window) {
        let Some(index) = self.tiles.iter().position(|tile| &tile.window == window) else {
            return;
        };
        let tile = self.tiles.remove(index);
        self.manager.close(tile.id);
        for held in &mut self.fullscreen {
            if *held == Some(tile.id) {
                *held = None;
            }
        }
        self.space.unmap_elem(&tile.window);
        self.arrange();
        self.focus_layout_window();
    }

    /// Configures every window on a desk a screen shows to the pixels the
    /// layout gives it, and takes the windows on every other desk off the
    /// screens.
    pub fn arrange(&mut self) {
        let heads: Vec<Head> = self.screens.heads().to_vec();
        let every = self.manager.all_tiles();
        let mut showing: Vec<WinId> = Vec::new();
        for head in &heads {
            let desk = head.desk + 1;
            let tiles = every
                .iter()
                .filter(|(held, _)| *held == desk)
                .map(|(_, tile)| *tile);
            for tile in tiles {
                showing.push(tile.id);
                let placed = self.placed(desk, tile);
                let Some(window) = self.window_for(tile.id) else {
                    continue;
                };
                if let Some(toplevel) = window.toplevel() {
                    toplevel.with_pending_state(|state| {
                        state.size = Some((placed.width, placed.height).into());
                    });
                    toplevel.send_pending_configure();
                }
                // An X11 window is configured in the root window's
                // coordinates, which are the shared space's, so it gets its
                // place as well as its size.
                if let Some(surface) = window.x11_surface() {
                    let rect = smithay::utils::Rectangle::new(
                        (placed.x, placed.y).into(),
                        (placed.width, placed.height).into(),
                    );
                    if let Err(err) = surface.configure(rect) {
                        log::debug!("an X11 window did not take its tile: {err}");
                    }
                }
                self.space
                    .map_element(window.clone(), (placed.x, placed.y), false);
            }
        }
        let hidden: Vec<Window> = self
            .tiles
            .iter()
            .filter(|tile| !showing.contains(&tile.id))
            .map(|tile| tile.window.clone())
            .collect();
        for window in hidden {
            self.space.unmap_elem(&window);
        }
        self.space.refresh();
        self.send_scales();
        self.restack();
    }

    /// Puts the space in the order `crate::stacking` gives the windows the
    /// screens show: the tiles, then the floats, then the pinned floats on
    /// top. Every path that maps or raises an element ends here, so a
    /// tile that takes the focus goes back under the camera circle. The
    /// log says the order once each time it changes.
    pub fn restack(&mut self) {
        let windows = self.stack_windows();
        let order = stacking::order(&windows);
        let layer_of = |id: WinId| {
            windows
                .iter()
                .find(|(_, held)| *held == id)
                .map(|(layer, _)| *layer)
        };
        // A tile overlaps nothing, so only the floats are raised, in the
        // order that puts the pinned ones on top.
        for id in order
            .iter()
            .filter(|id| layer_of(**id) != Some(Stack::Tiled))
        {
            if let Some(window) = self.window_for(*id) {
                self.space.raise_element(&window, false);
            }
        }
        if order != self.stack {
            let named: Vec<String> = order
                .iter()
                .map(|id| {
                    // The handle, the app-id or the title when the client
                    // has set one, and the layer for a float.
                    let mut named = crate::desk_server::handle(*id);
                    let name = match self.app_id(*id) {
                        app_id if !app_id.is_empty() => app_id,
                        _ => self.title(*id),
                    };
                    if !name.is_empty() {
                        named.push(' ');
                        named.push_str(&name);
                    }
                    if let Some(layer) = layer_of(*id).and_then(Stack::name) {
                        named.push_str(&format!(" ({layer})"));
                    }
                    named
                })
                .collect();
            log::info!(
                "the windows draw back to front: {}",
                if named.is_empty() {
                    "none".to_string()
                } else {
                    named.join(", ")
                }
            );
            self.stack = order;
        }
        // The pointer reads the same order the renderer draws, so a window
        // that came over the one under it takes the pointer now, and not
        // when the pointer next moves.
        self.refresh_pointer();
    }

    /// The windows the screens show, each with its layer, in the order the
    /// layout holds them: what `crate::stacking` sorts.
    pub fn stack_windows(&self) -> Vec<(Stack, WinId)> {
        let shown: Vec<usize> = self
            .screens
            .heads()
            .iter()
            .map(|head| head.desk + 1)
            .collect();
        self.manager
            .all_tiles()
            .into_iter()
            .filter(|(desk, _)| shown.contains(desk))
            .map(|(_, tile)| {
                (
                    Stack::of(tile.floating, self.manager.is_pinned(tile.id)),
                    tile.id,
                )
            })
            .collect()
    }

    /// Moves the pointer's focus onto the surface under it. The focus is
    /// found when the pointer moves, so a window that mapped, closed, was
    /// raised, or was restacked under a pointer that held still left the
    /// focus on the window that used to be there, and a press went to
    /// that window: the record button on the strip under the camera did
    /// nothing.
    /// A click or a drag in progress holds its surface, and is left alone.
    pub fn refresh_pointer(&mut self) {
        if self.pointer.is_grabbed() {
            return;
        }
        let under = self.surface_under();
        let current = self.pointer.current_focus();
        if !crate::input::pointer_moves(
            current.as_ref(),
            under.as_ref().map(|(surface, _)| surface),
        ) {
            return;
        }
        let serial = self.serial();
        let time = self.started.elapsed().as_millis() as u32;
        let pointer = self.pointer.clone();
        pointer.motion(
            self,
            under,
            &smithay::input::pointer::MotionEvent {
                location: self.pointer_at,
                serial,
                time,
            },
        );
        pointer.frame(self);
    }

    fn window_for(&self, id: WinId) -> Option<Window> {
        self.tile(id).map(|tile| tile.window.clone())
    }

    /// Gives the keyboard to the window the layout focuses, and marks every
    /// other window idle so its client draws itself unfocused. An X11
    /// window takes the X input focus with the keyboard, through
    /// `crate::focus`, and gives it back when the keyboard leaves.
    pub fn focus_layout_window(&mut self) {
        let id = self.manager.focus();
        let focus = id.and_then(|id| self.window_for(id));
        for tile in &self.tiles {
            let active = focus.as_ref() == Some(&tile.window);
            tile.window.set_activated(active);
            if let Some(toplevel) = tile.window.toplevel() {
                toplevel.send_pending_configure();
            }
        }
        let serial = SERIAL_COUNTER.next_serial();
        let target = focus.as_ref().and_then(Focus::of);
        // A float that takes the focus goes over the other floats of its
        // layer, once, when it takes it: a focus that stays put is not
        // raised again, so a `raise` of another float holds until the
        // focus moves. The raise puts the focused window over every float,
        // and the stack is put back: a tile under the floats, and the
        // pinned floats on top of all of them.
        if id != self.focused {
            if let Some(window) = focus.as_ref() {
                self.space.raise_element(window, true);
                let x11 = window
                    .x11_surface()
                    .filter(|x11| xwayland::takes_focus(x11.alive(), x11.is_mapped()));
                if let (Some(wm), Some(x11)) = (self.xwayland.wm.as_mut(), x11)
                    && let Err(err) = wm.raise_window(x11)
                {
                    log::debug!("an X11 window was not raised: {err}");
                }
            }
            if let Some(id) = id {
                self.manager.raise(id);
            }
            self.focused = id;
        }
        self.restack();
        let keyboard = self.keyboard.clone();
        keyboard.set_focus(self, target, serial);
    }

    /// The surface under the pointer, and where that surface sits on the
    /// screen.
    ///
    /// The overlay and top layers draw over the windows and the bottom and
    /// background layers under them, so the pointer reaches them in the
    /// same order. A region selector such as `slurp` draws on the overlay
    /// layer and reads the drag from there.
    pub fn surface_under(&self) -> Option<(WlSurface, Point<f64, Logical>)> {
        if let Some(found) = self.layer_under(&[Layer::Overlay, Layer::Top]) {
            return Some(found);
        }
        if let Some((window, location)) = self.window_under(self.pointer_at) {
            let at = location.to_f64();
            // Ask the window which of its surfaces the pointer is over. A
            // window is not one surface: a menu, a tooltip, and a bubble such
            // as Chromium's offer to restore pages are popups of their own,
            // and a video or a decoration can be a subsurface. Answering with
            // the toplevel sent every press to the page under the bubble, so
            // the bubble could be seen and not clicked.
            if let Some((surface, offset)) =
                window.surface_under(self.pointer_at - at, WindowSurfaceType::ALL)
            {
                return Some((surface, at + offset.to_f64()));
            }
            if let Some(surface) = window.wl_surface() {
                return Some((surface.into_owned(), at));
            }
        }
        self.layer_under(&[Layer::Bottom, Layer::Background])
    }

    /// Moves the focus to whatever the pointer sits over, which is
    /// Hyprland's `follow_mouse = 1`. A pointer that crosses onto another
    /// screen gives that screen the focus first.
    pub fn focus_follows_pointer(&mut self) {
        // A drag holds the pointer until it drops, and moving the keyboard
        // out from under it would send the drop to another client.
        if self.selection.dragging() {
            return;
        }
        // A Super drag holds the window the press landed on however far
        // the pointer travels, and the focus stays on it.
        if self.drag.is_some() {
            return;
        }
        if let Some(index) = self.screens.head_at(self.pointer_at.x, self.pointer_at.y)
            && screens::focus_screen(&mut self.screens, &mut self.manager, index)
        {
            self.after_layout();
        }
        let Some((window, _)) = self.window_under(self.pointer_at) else {
            return;
        };
        let Some(id) = self.tile_of(&window.clone()).map(|tile| tile.id) else {
            return;
        };
        if self.manager.focus() == Some(id) {
            return;
        }
        self.manager.focus_id(id);
        self.focus_layout_window();
    }

    /// Puts the pointer in the middle of one screen, which is where
    /// Hyprland puts it when a chord moves the focus to another monitor.
    pub fn warp_to_focused_screen(&mut self) {
        let Some((x, y)) = self.screens.focused().map(Head::center) else {
            return;
        };
        self.pointer_at = (x, y).into();
        let under = self.surface_under();
        let serial = self.serial();
        let time = self.started.elapsed().as_millis() as u32;
        let pointer = self.pointer.clone();
        pointer.motion(
            self,
            under,
            &smithay::input::pointer::MotionEvent {
                location: self.pointer_at,
                serial,
                time,
            },
        );
        pointer.frame(self);
    }

    /// What a frame draws over the space: the borders of every tile a
    /// screen shows, the notice, the tracked hand, and the pointer.
    pub fn overlay(&self) -> Overlay {
        let focus = self.manager.focus();
        let mut bars = Vec::new();
        let mut clips = Vec::new();
        let every = self.manager.all_tiles();
        for head in self.screens.heads() {
            let desk = head.desk + 1;
            for (_, tile) in every.iter().filter(|(held, _)| *held == desk) {
                if let Some(window) = self.window_for(tile.id) {
                    clips.push((window, self.placed(desk, *tile)));
                }
            }
            for (_, tile) in every
                .iter()
                .filter(|(held, tile)| *held == desk && !self.borderless(tile.id))
            {
                let placed = self.placed(desk, *tile);
                let color = if Some(tile.id) == focus {
                    layout::BORDER_ACTIVE
                } else {
                    layout::BORDER_IDLE
                };
                bars.extend(
                    layout::border_bars(placed)
                        .into_iter()
                        .map(|bar| (bar, color)),
                );
            }
        }
        Overlay {
            bars,
            notice: self.notices.showing(Instant::now()).is_some(),
            pointer: Some(self.pointer_at),
            cursor: self.cursor_status.clone(),
            drag_icon: self.drag_icon.clone(),
            millis: self.started.elapsed().as_millis() as u32,
            hands: Arc::clone(self.hands.pictures()),
            clips,
        }
    }

    /// Runs what one chord does.
    pub fn run(&mut self, action: Action) {
        match action {
            Action::OpenCoder => {
                let line = self.session.coder_line();
                self.start(&line);
            }
            Action::OpenShell => {
                let line = self.session.shell_line();
                self.start(&line);
            }
            Action::Exec(command) => {
                log::info!("the chord runs `{command}`");
                self.start(command);
            }
            Action::Close => self.close_focused(),
            Action::MoveFocus(dir) => {
                self.manager.movefocus(dir);
                self.after_layout();
            }
            Action::MoveWindow(dir) => {
                self.manager.movewindow(dir);
                self.after_layout();
            }
            Action::Resize(dir) => {
                let horizontal = matches!(dir, Dir::Left | Dir::Right);
                let screen = self.home_size(self.manager.workspace() + 1);
                let step = layout::resize_fraction(binds::RESIZE_STEP, screen, horizontal);
                self.manager.resize(dir, step);
                self.after_layout();
            }
            Action::Desk(desk) => {
                screens::show_desk(&mut self.screens, &mut self.manager, desk as usize - 1);
                self.after_layout();
            }
            Action::MoveToDesk(desk) => {
                screens::send_to_desk(&mut self.screens, &mut self.manager, desk as usize - 1);
                self.after_layout();
            }
            Action::FocusScreenNext | Action::FocusScreenPrevious => {
                let step = if action == Action::FocusScreenNext {
                    1
                } else {
                    -1
                };
                if let Some(index) = self.screens.cycle(step) {
                    self.focus_screen(index);
                }
            }
            Action::FocusScreen(dir) => {
                let from = self.screens.focused_index();
                if let Some(index) = self.screens.neighbor(from, dir) {
                    self.focus_screen(index);
                }
            }
            Action::MoveWindowToScreen(dir) => {
                if screens::move_window_to_screen(&mut self.screens, &mut self.manager, dir) {
                    self.after_layout();
                    self.warp_to_focused_screen();
                }
            }
            Action::MoveDeskToScreen(dir) => {
                if screens::move_desk_to_screen(&mut self.screens, &mut self.manager, dir) {
                    self.after_layout();
                    self.warp_to_focused_screen();
                }
            }
            Action::ToggleSplit => {
                self.manager.togglesplit();
                self.after_layout();
            }
            Action::ToggleFloat => {
                self.manager.togglefloating();
                self.after_layout();
            }
            Action::Fullscreen => {
                let desk = self.manager.workspace();
                let focus = self.manager.focus();
                self.manager.fullscreen();
                self.fullscreen[desk] = match (self.fullscreen[desk], focus) {
                    (Some(held), Some(id)) if held == id => None,
                    (_, id) => id,
                };
                self.after_layout();
            }
            Action::Maximize => {
                let desk = self.manager.workspace();
                self.manager.maximize();
                self.fullscreen[desk] = None;
                self.after_layout();
            }
            Action::ToggleHands => self.toggle_hands(),
            Action::Exit => self.running = false,
        }
    }

    /// Gives one screen the focus, puts the pointer on it, and moves the
    /// keyboard to the window the layout focuses there.
    pub fn focus_screen(&mut self, index: usize) {
        if screens::focus_screen(&mut self.screens, &mut self.manager, index) {
            self.after_layout();
            self.warp_to_focused_screen();
        }
    }

    /// Redraws the layout and moves the focus with it.
    pub fn after_layout(&mut self) {
        self.arrange();
        self.focus_layout_window();
    }

    /// Starts a command line with the session's environment, and Xwayland
    /// first when the compositor has not started it.
    pub fn start(&mut self, line: &str) {
        self.start_xwayland();
        if let Err(err) = exec::spawn(line, &self.session) {
            log::warn!("{err}");
        }
    }

    /// Asks the focused window to close, after asking the session in it
    /// whether it has work in flight. `crate::closing` is the rule, and
    /// the client answers it.
    pub fn close_focused(&mut self) {
        let Some(id) = self.manager.focus() else {
            return;
        };
        let client = crate::closing::Client::read();
        self.close_chord(id, &client);
    }

    /// Asks one window to close.
    pub fn close_window(&mut self, id: WinId) {
        let Some(tile) = self.tile(id) else {
            return;
        };
        if let Some(toplevel) = tile.window.toplevel() {
            toplevel.send_close();
        }
        if let Some(surface) = tile.window.x11_surface()
            && let Err(err) = surface.close()
        {
            log::warn!("an X11 window was not asked to close: {err}");
        }
    }

    /// Makes one window fill the screen, or puts it back in the layout,
    /// which is what a client's own fullscreen request asks for.
    pub fn set_fullscreen(&mut self, id: WinId, fill: bool) {
        let Some(desk) = self.desk_of(id) else {
            return;
        };
        let home = self.manager.workspace();
        self.manager.focus_id(id);
        let held = self.fullscreen[desk - 1];
        let already = held == Some(id);
        if fill != already {
            self.manager.fullscreen();
            self.fullscreen[desk - 1] = if fill { Some(id) } else { None };
        }
        self.manager.switch_workspace(home);
        self.after_layout();
    }

    /// The desk one window sits on, numbered 1 through 9.
    pub fn desk_of(&self, id: WinId) -> Option<usize> {
        self.manager
            .all_tiles()
            .into_iter()
            .find(|(_, tile)| tile.id == id)
            .map(|(desk, _)| desk)
    }

    /// The serial the next input event carries.
    pub fn serial(&self) -> Serial {
        SERIAL_COUNTER.next_serial()
    }
}
