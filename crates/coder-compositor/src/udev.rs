//! The hardware backend: a seat, the monitors, and the input devices of a
//! TTY.
//!
//! `coder-compositor --backend udev`, or a start with no Wayland or X11
//! display in its environment, runs this. It takes the seat through
//! `libseat`, which asks logind or seatd for the devices, so the process
//! needs no group of its own. It opens the graphics card through the seat,
//! allocates its buffers through GBM, and drives each connected monitor
//! through a DRM compositor on a CRTC of its own. It reads the keyboard and
//! the pointer through `libinput`, and `udev` tells it when a monitor or an
//! input device arrives or leaves.
//!
//! The backend drives the monitors of one graphics card: the card
//! `CODER_COMPOSITOR_DRM_DEVICE` names, or the card the firmware booted
//! with. A second card's monitors stay dark, and the log says so.
//!
//! The seat decides when the card is driven. logind and seatd give DRM
//! master to the session on the active terminal alone, so a compositor
//! started while another terminal is active opens its card, reads what
//! needs no master, and waits; the log says so, and the seat's first
//! `ActivateSession` resets the connectors and starts the screens. A
//! start on the active terminal, which a TTY login is, takes the screen
//! at once. `crate::connectors::Readiness` is the decision.
//!
//! Every screen redraws on its own vertical blank. A frame with nothing new
//! is not sent to the monitor, and the screen looks again one refresh
//! later. A client's frame callbacks go out with every look, so a client
//! drawing an animation draws at the monitor's rate.
//!
//! The NVIDIA driver needs three things the backend does for it: the
//! overlay planes are left unused, because scanning a client's buffer out
//! on one breaks on that driver; a frame waits for its fence when the
//! driver cannot fence the commit; and a client's dmabuf holds its commit
//! until its acquire point or its buffer is ready, through explicit sync
//! when the device supports `syncobj_eventfd`. Each of these was found on
//! an NVIDIA card on 2026-09-16, and the crate's README says how to start
//! the compositor from a TTY.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use smithay::backend::allocator::Fourcc;
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::drm::compositor::{FrameFlags, PrimaryPlaneElement};
use smithay::backend::drm::exporter::gbm::GbmFramebufferExporter;
use smithay::backend::drm::output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements};
use smithay::backend::drm::{DrmDevice, DrmDeviceFd, DrmEvent, DrmNode, NodeType};
use smithay::backend::egl::context::ContextPriority;
use smithay::backend::input::InputEvent;
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::renderer::ImportDma;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::multigpu::gbm::GbmGlesBackend;
use smithay::backend::renderer::multigpu::{GpuManager, MultiRenderer};
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{Event as SessionEvent, Session};
use smithay::backend::udev::{UdevBackend, UdevEvent, all_gpus, primary_gpu};
use smithay::output::{Mode, Output, PhysicalProperties};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::ping::make_ping;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::{
    EventLoop, Interest, Mode as TriggerMode, PostAction, RegistrationToken,
};
use smithay::reexports::drm::Device as _;
use smithay::reexports::drm::control::{Device as ControlDevice, ModeTypeFlags, connector, crtc};
use smithay::reexports::input::{self, DeviceCapability, Libinput};
use smithay::reexports::rustix::fs::OFlags;
use smithay::reexports::wayland_server::Display;
use smithay::reexports::wayland_server::backend::GlobalId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::DeviceFd;
use smithay::wayland::dmabuf::DmabufFeedbackBuilder;
use smithay::wayland::drm_syncobj::{DrmSyncobjState, supports_syncobj_eventfd};
use smithay::wayland::socket::ListeningSocketSource;

use crate::connectors::{Change, Readiness, Seen, Tracker};
use crate::exec::Session as ExecSession;
use crate::input::on_input;
use crate::layout;
use crate::render::{self, Element};
use crate::screencopy;
use crate::state::{ClientState, Coder, Graphics, Parts};
use coder_desk::serve as desk;

/// The variable that names the graphics card the backend drives, as a path
/// such as `/dev/dri/card2`.
pub const DRM_DEVICE_VAR: &str = "CODER_COMPOSITOR_DRM_DEVICE";

/// The variable that keeps every client buffer off the primary plane, so
/// every frame is composited. Set it to `1` when a client's buffer on the
/// monitor shows torn or stale.
pub const NO_SCANOUT_VAR: &str = "CODER_COMPOSITOR_NO_SCANOUT";

/// The formats a monitor's buffers are allocated in, in the order the
/// backend tries them. Eight bits a channel, which is what every driver
/// scans out, and without alpha first, which the primary plane needs.
const COLOR_FORMATS: [Fourcc; 4] = [
    Fourcc::Xrgb8888,
    Fourcc::Xbgr8888,
    Fourcc::Argb8888,
    Fourcc::Abgr8888,
];

/// How long the loop waits for an event before it looks at `running`.
const WAKE_EVERY: Duration = Duration::from_millis(250);

/// The renderer every screen draws with: the one graphics card, rendering
/// and scanning out on itself.
type Gpu = GbmGlesBackend<GlesRenderer, DrmDeviceFd>;
type UdevRenderer<'a> = MultiRenderer<'a, 'a, Gpu, Gpu>;
type Manager = DrmOutputManager<
    GbmAllocator<DrmDeviceFd>,
    GbmFramebufferExporter<DrmDeviceFd>,
    (),
    DrmDeviceFd,
>;
type Scanout =
    DrmOutput<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;

/// The seat, the card, and the screens the backend drives.
pub struct Hardware {
    session: LibSeatSession,
    libinput: Libinput,
    /// The render node of the card every screen draws with.
    primary: DrmNode,
    gpus: GpuManager<Gpu>,
    devices: HashMap<DrmNode, Card>,
    keyboards: Vec<input::Device>,
    frame_flags: FrameFlags,
}

/// One graphics card the backend drives.
struct Card {
    token: RegistrationToken,
    render: DrmNode,
    manager: Manager,
    tracker: Tracker,
    screens: HashMap<crtc::Handle, Screen>,
    nvidia: bool,
    /// Whether the seat drives the card yet, or the card waits for it.
    readiness: Readiness,
}

/// One monitor on one CRTC.
struct Screen {
    name: String,
    global: GlobalId,
    output: Scanout,
    /// A frame waits for its vertical blank.
    queued: bool,
    /// A redraw is scheduled on the loop.
    scheduled: bool,
    /// The time between two refreshes.
    refresh: Duration,
}

impl Hardware {
    /// Imports a client's buffer into the renderer, which is how the dmabuf
    /// global checks a buffer before the client uses it.
    pub fn import_dmabuf(&mut self, dmabuf: &Dmabuf) -> bool {
        match self.gpus.single_renderer(&self.primary) {
            Ok(mut renderer) => renderer.import_dmabuf(dmabuf, None).is_ok(),
            Err(_) => false,
        }
    }

    /// Uploads a committed buffer before the frame that draws it, so the
    /// frame does not wait for it.
    pub fn early_import(&mut self, surface: &WlSurface) {
        if let Err(err) = self.gpus.early_import(self.primary, surface) {
            log::debug!("a buffer was not imported early: {err}");
        }
    }

    /// Switches to a virtual terminal, which is what Ctrl+Alt with a
    /// function key asks for.
    pub fn switch_vt(&mut self, vt: i32) {
        log::info!("switching to virtual terminal {vt}");
        if let Err(err) = self.session.change_vt(vt) {
            log::warn!("the seat did not switch to virtual terminal {vt}: {err}");
        }
    }

    /// Lights the keyboards' LEDs for Caps Lock and Num Lock.
    pub fn set_leds(&mut self, leds: smithay::input::keyboard::LedState) {
        for keyboard in &mut self.keyboards {
            keyboard.led_update(leds.into());
        }
    }
}

impl Coder {
    /// Switches to a virtual terminal on the hardware backend. The nested
    /// backend leaves the keys to the session it runs in.
    pub fn switch_vt(&mut self, vt: i32) {
        match &mut self.graphics {
            Graphics::Udev(hardware) => hardware.switch_vt(vt),
            Graphics::Winit(_) => log::debug!("the nested backend switches no terminal"),
        }
    }

    fn hardware(&mut self) -> Option<&mut Hardware> {
        match &mut self.graphics {
            Graphics::Udev(hardware) => Some(hardware),
            Graphics::Winit(_) => None,
        }
    }
}

/// The render node of the card to drive: the one `CODER_COMPOSITOR_DRM_DEVICE`
/// names, the one the firmware booted with, or the first card on the seat.
fn pick_card(seat: &str) -> Result<DrmNode, String> {
    let named = std::env::var(DRM_DEVICE_VAR)
        .ok()
        .filter(|path| !path.is_empty());
    let path = match named {
        Some(path) => Some(path.into()),
        None => primary_gpu(seat)
            .map_err(|err| format!("udev did not list the graphics cards: {err}"))?
            .or_else(|| {
                all_gpus(seat)
                    .ok()
                    .and_then(|cards| cards.into_iter().next())
            }),
    };
    let path: std::path::PathBuf = path.ok_or_else(|| {
        format!("the seat {seat} has no graphics card, so there is no monitor to draw on")
    })?;
    let node = DrmNode::from_path(&path)
        .map_err(|err| format!("{} is not a graphics card: {err}", path.display()))?;
    Ok(render_node(node))
}

/// The render node of a card, or the node itself when the card has none.
fn render_node(node: DrmNode) -> DrmNode {
    node.node_with_type(NodeType::Render)
        .and_then(Result::ok)
        .unwrap_or(node)
}

/// Runs the compositor on the seat of the TTY this process started on.
pub fn run() -> Result<(), String> {
    let mut events: EventLoop<'static, Coder> =
        EventLoop::try_new().map_err(|err| format!("the event loop: {err}"))?;
    let mut display: Display<Coder> = Display::new().map_err(|err| format!("display: {err}"))?;
    let handle = display.handle();

    let (session, notifier) = LibSeatSession::new().map_err(|err| {
        format!(
            "libseat did not open a seat: {err}. The hardware backend runs from a TTY login, \
             where logind or seatd hands it the seat; inside a session, run it with \
             --backend winit."
        )
    })?;
    let seat = session.seat();
    let seat_name = seat.clone();
    let primary = pick_card(&seat)?;
    log::info!("the seat is {seat}, and the screens draw with {primary}");
    let gpus = GpuManager::new(GbmGlesBackend::with_context_priority(ContextPriority::High))
        .map_err(|err| format!("the renderer did not start: {err}"))?;

    let mut libinput =
        Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
    libinput
        .udev_assign_seat(&seat)
        .map_err(|()| format!("libinput did not take the seat {seat}"))?;
    let input_backend = LibinputInputBackend::new(libinput.clone());

    let socket = ListeningSocketSource::new_auto()
        .map_err(|err| format!("the compositor's socket: {err}"))?;
    let socket_name = socket.socket_name().to_string_lossy().to_string();
    let (ping, desk_wake) = make_ping().map_err(|err| format!("the desk socket's wake: {err}"))?;
    let desk_server = desk::bind_waking(Some(Box::new(move || ping.ping())))?;
    let exec_session = ExecSession::read(socket_name.clone(), desk_server.path());

    let frame_flags = if std::env::var(NO_SCANOUT_VAR).is_ok_and(|value| value == "1") {
        log::info!("{NO_SCANOUT_VAR} is set, so every frame is composited");
        FrameFlags::empty()
    } else {
        FrameFlags::DEFAULT
    };
    let hardware = Hardware {
        session,
        libinput: libinput.clone(),
        primary,
        gpus,
        devices: HashMap::new(),
        keyboards: Vec::new(),
        frame_flags,
    };
    let mut state = Coder::new(Parts {
        display: handle.clone(),
        events: events.handle(),
        graphics: Graphics::Udev(Box::new(hardware)),
        seat,
        session: exec_session,
    })?;
    state.screencopy.offer_dmabuf();

    let loop_handle = events.handle();
    loop_handle
        .insert_source(socket, |stream, _, state| {
            if let Err(err) = state
                .display
                .insert_client(stream, Arc::new(ClientState::default()))
            {
                log::warn!("a client was refused: {err}");
            }
        })
        .map_err(|err| format!("the compositor's socket did not join the loop: {err}"))?;
    let display_fd = display
        .backend()
        .poll_fd()
        .try_clone_to_owned()
        .map_err(|err| format!("the display's descriptor: {err}"))?;
    loop_handle
        .insert_source(
            Generic::new(display_fd, Interest::READ, TriggerMode::Level),
            |_, _, _| Ok(PostAction::Continue),
        )
        .map_err(|err| format!("the display did not join the loop: {err}"))?;
    loop_handle
        .insert_source(input_backend, |event, _, state| {
            match &event {
                InputEvent::DeviceAdded { device } => state.input_device_added(device.clone()),
                InputEvent::DeviceRemoved { device } => state.input_device_removed(device),
                _ => {}
            }
            on_input(state, event);
        })
        .map_err(|err| format!("libinput did not join the loop: {err}"))?;
    loop_handle
        .insert_source(notifier, move |event, _, state| match event {
            SessionEvent::PauseSession => state.pause(),
            SessionEvent::ActivateSession => state.resume(),
        })
        .map_err(|err| format!("the seat did not join the loop: {err}"))?;
    loop_handle
        .insert_source(desk_wake, move |_, _, state| {
            while let Ok(call) = desk_server.calls.try_recv() {
                // A `shot` leaves the dispatch and waits for the frame
                // that fills it, so the caller hears once the file is
                // written.
                let Some(call) = crate::drive::take_shot(state, call) else {
                    continue;
                };
                let answer = desk::answer(call.request.clone(), state);
                call.answer(answer);
            }
        })
        .map_err(|err| format!("the desk socket did not join the loop: {err}"))?;
    // The hands reader wakes the loop for each frame it hands over, the
    // way the desk socket does for a call.
    let (hands_ping, hands_wake) =
        make_ping().map_err(|err| format!("the hands reader's wake: {err}"))?;
    state.hands.set_wake(Arc::new(move || hands_ping.ping()));
    loop_handle
        .insert_source(hands_wake, |_, _, state| state.hands_pass())
        .map_err(|err| format!("the hands reader did not join the loop: {err}"))?;

    let udev = UdevBackend::new(&seat_name).map_err(|err| format!("udev did not start: {err}"))?;
    let cards: Vec<(u64, std::path::PathBuf)> = udev
        .device_list()
        .map(|(id, path)| (id, path.to_path_buf()))
        .collect();
    for (id, path) in cards {
        match DrmNode::from_dev_id(id) {
            Ok(node) => state.card_added(node, &path),
            Err(err) => log::warn!("{} is not a graphics card: {err}", path.display()),
        }
    }
    loop_handle
        .insert_source(udev, |event, _, state| match event {
            UdevEvent::Added { device_id, path } => match DrmNode::from_dev_id(device_id) {
                Ok(node) => state.card_added(node, &path),
                Err(err) => log::warn!("{} is not a graphics card: {err}", path.display()),
            },
            UdevEvent::Changed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    state.card_changed(node);
                }
            }
            UdevEvent::Removed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    state.card_removed(node);
                }
            }
        })
        .map_err(|err| format!("udev did not join the loop: {err}"))?;
    if state.screens.is_empty() {
        if state.awaits_seat() {
            log::info!(
                "no screen is on yet, because the seat is on another terminal; switch to this \
                 terminal with the console keys, and the compositor takes the screen"
            );
        } else {
            log::warn!(
                "no monitor is connected to {primary}; the compositor waits for one to be plugged in"
            );
        }
    }
    state.announce_dmabuf(&handle);

    // SAFETY: the only other thread this process runs now is the desk
    // socket's reader, which reads the socket and never the environment,
    // and no program has started yet.
    unsafe {
        std::env::set_var("WAYLAND_DISPLAY", &socket_name);
        std::env::remove_var("DISPLAY");
        std::env::set_var(
            coder_desk::protocol::SOCKET_VAR,
            state.session.desk_socket.as_os_str(),
        );
    }
    log::info!(
        "the compositor listens on {socket_name}, and its desk socket is {}",
        state.session.desk_socket.display()
    );

    while state.running {
        if let Err(err) = events.dispatch(Some(WAKE_EVERY), &mut state) {
            log::warn!("the event loop stopped: {err}");
            break;
        }
        display
            .dispatch_clients(&mut state)
            .map_err(|err| format!("dispatch: {err}"))?;
        state.space.refresh();
        state.popups.cleanup();
        display
            .flush_clients()
            .map_err(|err| format!("flush: {err}"))?;
    }
    Ok(())
}

impl Coder {
    /// Announces the dmabuf global with the renderer's formats and the card
    /// clients should allocate on, and explicit sync when the card supports
    /// it.
    fn announce_dmabuf(&mut self, display: &smithay::reexports::wayland_server::DisplayHandle) {
        let Some(hardware) = self.hardware() else {
            return;
        };
        let primary = hardware.primary;
        let formats = match hardware.gpus.single_renderer(&primary) {
            Ok(renderer) => renderer.dmabuf_formats(),
            Err(err) => {
                log::warn!(
                    "the renderer on {primary} did not start, so no client can share a buffer: {err}"
                );
                return;
            }
        };
        let import_fd = hardware
            .devices
            .values()
            .find(|card| card.render == primary)
            .map(|card| card.manager.device().device_fd().clone());
        match DmabufFeedbackBuilder::new(primary.dev_id(), formats).build() {
            Ok(feedback) => {
                let global = self
                    .dmabuf_state
                    .create_global_with_default_feedback::<Coder>(display, &feedback);
                self.dmabuf_global = Some(global);
            }
            Err(err) => log::warn!("the dmabuf feedback was not built: {err}"),
        }
        match import_fd {
            Some(fd) if supports_syncobj_eventfd(&fd) => {
                self.syncobj_state = Some(DrmSyncobjState::new::<Coder>(display, fd));
                log::info!("explicit sync is on for {primary}");
            }
            Some(_) => log::info!("{primary} has no syncobj_eventfd, so explicit sync is off"),
            None => {}
        }
    }

    /// Opens one graphics card through the seat and starts a screen for each
    /// monitor on it. A card that is not the one the screens draw with is
    /// left alone.
    fn card_added(&mut self, node: DrmNode, path: &Path) {
        let events = self.events.clone();
        let Some(hardware) = self.hardware() else {
            return;
        };
        let render = render_node(node);
        if render != hardware.primary {
            log::info!(
                "{} is not the card the screens draw with, {}, so its monitors stay dark",
                path.display(),
                hardware.primary
            );
            return;
        }
        let opened = hardware
            .session
            .open(
                path,
                OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
            )
            .map_err(|err| format!("the seat did not open {}: {err}", path.display()));
        let fd = match opened {
            Ok(fd) => DrmDeviceFd::new(DeviceFd::from(fd)),
            Err(err) => {
                log::warn!("{err}");
                return;
            }
        };
        // A card opened while the seat is on another terminal cannot reset
        // its connectors: logind holds DRM master for the active session,
        // and the reset fails with `Permission denied`. The card opens
        // without the reset, and `resume` resets it and reads its
        // connectors when the seat arrives.
        let readiness = Readiness::at_open(hardware.session.is_active());
        let (drm, notifier) = match DrmDevice::new(fd.clone(), readiness == Readiness::Driving) {
            Ok(opened) => opened,
            Err(err) => {
                log::warn!("{} did not open for modesetting: {err}", path.display());
                return;
            }
        };
        let gbm = match GbmDevice::new(fd) {
            Ok(gbm) => gbm,
            Err(err) => {
                log::warn!("GBM did not open {}: {err}", path.display());
                return;
            }
        };
        let nvidia = drm
            .get_driver()
            .map(|driver| {
                driver
                    .name()
                    .to_string_lossy()
                    .to_lowercase()
                    .contains("nvidia")
            })
            .unwrap_or(false);
        if let Err(err) = hardware.gpus.as_mut().add_node(render, gbm.clone()) {
            log::warn!("the renderer did not start on {render}: {err}");
            return;
        }
        let render_formats = match hardware.gpus.single_renderer(&render) {
            Ok(mut renderer) => renderer
                .as_mut()
                .egl_context()
                .dmabuf_render_formats()
                .clone(),
            Err(err) => {
                log::warn!("the renderer on {render} did not start: {err}");
                return;
            }
        };
        let token = events.insert_source(notifier, move |event, _, state| match event {
            DrmEvent::VBlank(crtc) => state.vblank(node, crtc),
            DrmEvent::Error(err) => log::warn!("the card {node} reported: {err}"),
        });
        let token = match token {
            Ok(token) => token,
            Err(err) => {
                log::warn!("the card {node} did not join the loop: {err}");
                return;
            }
        };
        let allocator = GbmAllocator::new(
            gbm.clone(),
            GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
        );
        let exporter = GbmFramebufferExporter::new(gbm.clone(), Some(render));
        let manager = Manager::new(
            drm,
            allocator,
            exporter,
            Some(gbm),
            COLOR_FORMATS,
            render_formats,
        );
        log::info!(
            "opened {} ({})",
            path.display(),
            if nvidia {
                "the NVIDIA driver"
            } else {
                "not NVIDIA"
            }
        );
        hardware.devices.insert(
            node,
            Card {
                token,
                render,
                manager,
                tracker: Tracker::default(),
                screens: HashMap::new(),
                nvidia,
                readiness,
            },
        );
        match readiness {
            Readiness::Driving => self.card_changed(node),
            Readiness::Waiting => log::info!(
                "the seat is on another terminal, so {} waits for it: switch to this terminal \
                 with the console keys, and the compositor takes the screen",
                path.display()
            ),
        }
    }

    /// Whether a card opened from another terminal is still waiting for
    /// the seat to arrive on this one.
    fn awaits_seat(&mut self) -> bool {
        self.hardware().is_some_and(|hardware| {
            hardware
                .devices
                .values()
                .any(|card| card.readiness == Readiness::Waiting)
        })
    }

    /// Reads a card's connectors again and starts or drops a screen for
    /// each monitor that arrived or left.
    fn card_changed(&mut self, node: DrmNode) {
        let Some(card) = self
            .hardware()
            .and_then(|hardware| hardware.devices.get_mut(&node))
        else {
            return;
        };
        let drm = card.manager.device();
        let resources = match drm.resource_handles() {
            Ok(resources) => resources,
            Err(err) => {
                log::warn!("the card {node} did not list its connectors: {err}");
                return;
            }
        };
        let infos: Vec<connector::Info> = resources
            .connectors()
            .iter()
            .filter_map(|handle| drm.get_connector(*handle, true).ok())
            .collect();
        let seen: Vec<Seen> = infos
            .iter()
            .map(|info| {
                let mut crtcs: Vec<u32> = Vec::new();
                for encoder in info.encoders() {
                    let Ok(encoder) = drm.get_encoder(*encoder) else {
                        continue;
                    };
                    for crtc in resources.filter_crtcs(encoder.possible_crtcs()) {
                        let raw = u32::from(crtc);
                        if !crtcs.contains(&raw) {
                            crtcs.push(raw);
                        }
                    }
                }
                Seen {
                    connector: u32::from(info.handle()),
                    name: connector_name(info),
                    connected: info.state() == connector::State::Connected,
                    crtcs,
                }
            })
            .collect();
        let crtcs: Vec<crtc::Handle> = resources.crtcs().to_vec();
        let changes = card.tracker.scan(&seen);
        for change in changes {
            match change {
                Change::Connected {
                    connector,
                    crtc,
                    name,
                } => {
                    let info = infos
                        .iter()
                        .find(|info| u32::from(info.handle()) == connector)
                        .cloned();
                    let handle = crtcs.iter().copied().find(|held| u32::from(*held) == crtc);
                    if let (Some(info), Some(handle)) = (info, handle) {
                        self.connector_connected(node, info, handle, name);
                    }
                }
                Change::Disconnected { crtc, name, .. } => {
                    if let Some(handle) =
                        crtcs.iter().copied().find(|held| u32::from(*held) == crtc)
                    {
                        self.connector_disconnected(node, handle, &name);
                    }
                }
                Change::NoCrtc { name } => log::warn!(
                    "the monitor on {name} stays dark: every CRTC it can reach drives another screen"
                ),
            }
        }
    }

    /// Starts a screen on a monitor that arrived.
    fn connector_connected(
        &mut self,
        node: DrmNode,
        info: connector::Info,
        crtc: crtc::Handle,
        name: String,
    ) {
        let Some(drm_mode) = preferred_mode(&info) else {
            log::warn!("the monitor on {name} reports no mode");
            return;
        };
        let mode = Mode::from(drm_mode);
        let (width_mm, height_mm) = info.size().unwrap_or((0, 0));
        let output = Output::new(
            name.clone(),
            PhysicalProperties {
                size: (width_mm as i32, height_mm as i32).into(),
                subpixel: info.subpixel().into(),
                make: "Unknown".into(),
                model: name.clone(),
            },
        );
        let global = output.create_global::<Coder>(&self.display);
        self.add_output(output.clone(), mode, 1.0);

        let started = {
            let Some(hardware) = self.hardware() else {
                return;
            };
            let Some(card) = hardware.devices.get_mut(&node) else {
                return;
            };
            let planes = card.manager.device().planes(&crtc).map(|mut planes| {
                // Scanning a client's buffer out on an overlay plane breaks
                // on the NVIDIA driver, so the frame composites instead.
                if card.nvidia {
                    planes.overlay.clear();
                }
                planes
            });
            match (planes, hardware.gpus.single_renderer(&card.render)) {
                (Ok(planes), Ok(mut renderer)) => card
                    .manager
                    .initialize_output::<_, Element<UdevRenderer<'_>>>(
                        crtc,
                        drm_mode,
                        &[info.handle()],
                        &output,
                        Some(planes),
                        &mut renderer,
                        &DrmOutputRenderElements::default(),
                    )
                    .map_err(|err| err.to_string()),
                (Err(err), _) => Err(format!("the CRTC's planes did not read: {err}")),
                (_, Err(err)) => Err(format!("the renderer did not start: {err}")),
            }
        };
        match started {
            Ok(drm_output) => {
                let refresh = refresh_of(&mode);
                if let Some(card) = self
                    .hardware()
                    .and_then(|hardware| hardware.devices.get_mut(&node))
                {
                    card.screens.insert(
                        crtc,
                        Screen {
                            name: name.clone(),
                            global,
                            output: drm_output,
                            queued: false,
                            scheduled: false,
                            refresh,
                        },
                    );
                }
                log::info!(
                    "the monitor on {name} is on at {}x{} and {} hertz",
                    mode.size.w,
                    mode.size.h,
                    mode.refresh / 1000
                );
                self.schedule_draw(node, crtc, Duration::ZERO);
            }
            Err(err) => {
                log::warn!("the monitor on {name} did not start: {err}");
                self.display.remove_global::<Coder>(global);
                self.remove_output(&name);
            }
        }
    }

    /// Drops the screen of a monitor that left.
    fn connector_disconnected(&mut self, node: DrmNode, crtc: crtc::Handle, name: &str) {
        let removed = self
            .hardware()
            .and_then(|hardware| hardware.devices.get_mut(&node))
            .and_then(|card| card.screens.remove(&crtc));
        if let Some(screen) = removed {
            self.display.remove_global::<Coder>(screen.global);
        }
        self.remove_output(name);
    }

    /// Drops every screen of a card that left the seat.
    fn card_removed(&mut self, node: DrmNode) {
        let Some(screens) = self
            .hardware()
            .and_then(|hardware| hardware.devices.get_mut(&node))
            .map(|card| {
                card.tracker.clear();
                card.screens
                    .iter()
                    .map(|(crtc, screen)| (*crtc, screen.name.clone()))
                    .collect::<Vec<_>>()
            })
        else {
            return;
        };
        for (crtc, name) in screens {
            self.connector_disconnected(node, crtc, &name);
        }
        let events = self.events.clone();
        if let Some(hardware) = self.hardware()
            && let Some(card) = hardware.devices.remove(&node)
        {
            hardware.gpus.as_mut().remove_node(&card.render);
            events.remove(card.token);
        }
        log::info!("the card {node} left the seat");
    }

    /// A monitor showed the frame the backend queued.
    fn vblank(&mut self, node: DrmNode, crtc: crtc::Handle) {
        let Some(screen) = self
            .hardware()
            .and_then(|hardware| hardware.devices.get_mut(&node))
            .and_then(|card| card.screens.get_mut(&crtc))
        else {
            return;
        };
        screen.queued = false;
        if let Err(err) = screen.output.frame_submitted() {
            log::warn!("the frame on {} was not shown: {err}", screen.name);
        }
        self.schedule_draw(node, crtc, Duration::ZERO);
    }

    /// Draws a screen after a delay, unless a draw is already scheduled.
    fn schedule_draw(&mut self, node: DrmNode, crtc: crtc::Handle, after: Duration) {
        let events = self.events.clone();
        let Some(screen) = self
            .hardware()
            .and_then(|hardware| hardware.devices.get_mut(&node))
            .and_then(|card| card.screens.get_mut(&crtc))
        else {
            return;
        };
        if screen.scheduled {
            return;
        }
        screen.scheduled = true;
        let inserted = events.insert_source(Timer::from_duration(after), move |_, _, state| {
            state.draw_screen(node, crtc);
            TimeoutAction::Drop
        });
        if let Err(err) = inserted {
            log::warn!("a redraw was not scheduled: {err}");
            screen.scheduled = false;
        }
    }

    /// Draws one screen, sends it to the monitor when anything changed, and
    /// answers the copies a screencopy client asked of it.
    fn draw_screen(&mut self, node: DrmNode, crtc: crtc::Handle) {
        let overlay = self.overlay();
        let time = self.started.elapsed();
        let Coder {
            graphics,
            space,
            decor,
            screencopy: copies,
            drive,
            outputs,
            ..
        } = self;
        let Graphics::Udev(hardware) = graphics else {
            return;
        };
        let active = hardware.session.is_active();
        let flags = hardware.frame_flags;
        let Some(card) = hardware.devices.get_mut(&node) else {
            return;
        };
        let Some(screen) = card.screens.get_mut(&crtc) else {
            return;
        };
        screen.scheduled = false;
        if !active || screen.queued {
            return;
        }
        let refresh = screen.refresh;
        let name = screen.name.clone();
        let Some(output) = outputs.iter().find(|held| held.name() == name).cloned() else {
            return;
        };
        let mut renderer = match hardware.gpus.single_renderer(&card.render) {
            Ok(renderer) => renderer,
            Err(err) => {
                log::warn!("the renderer on {} did not start: {err}", card.render);
                return;
            }
        };
        let elements = match render::output_elements(&mut renderer, space, &output, &overlay, decor)
        {
            Ok(elements) => elements,
            Err(err) => {
                log::warn!("{err}");
                return;
            }
        };
        let drawn = screen
            .output
            .render_frame(&mut renderer, &elements, layout::BACKGROUND, flags);
        let mut again = Some(refresh);
        match drawn {
            Ok(frame) => {
                if frame.needs_sync()
                    && let PrimaryPlaneElement::Swapchain(element) = &frame.primary_element
                    && element.sync.wait().is_err()
                {
                    log::debug!("the frame's fence was interrupted");
                }
                let empty = frame.is_empty;
                drop(frame);
                if !empty {
                    match screen.output.queue_frame(()) {
                        Ok(()) => {
                            screen.queued = true;
                            again = None;
                        }
                        Err(err) => log::warn!("the frame on {name} was not queued: {err}"),
                    }
                }
            }
            Err(err) => log::warn!("the frame on {name} did not draw: {err}"),
        }
        let waiting = copies.take_for(&name);
        let shots = drive.take_shots(&name);
        if !waiting.is_empty() || !shots.is_empty() {
            let mode = output
                .current_mode()
                .map(|mode| layout::Screen {
                    width: mode.size.w,
                    height: mode.size.h,
                })
                .unwrap_or(layout::Screen {
                    width: 1,
                    height: 1,
                });
            let scale = output.current_scale().fractional_scale();
            if !waiting.is_empty() {
                screencopy::draw_copies(waiting, &mut renderer, &elements, mode, scale, time);
            }
            if !shots.is_empty() {
                // The screen is drawn again into a buffer of this
                // compositor's own, because the frame above went to the
                // monitor and the DRM compositor keeps it.
                let pixels = screencopy::render_screen(&mut renderer, &elements, mode, scale);
                crate::drive::write_shots(shots, &pixels, mode, screencopy::Rows::TopDown);
            }
        }
        drop(elements);
        drop(renderer);
        self.send_frames(&output);
        if let Some(after) = again {
            self.schedule_draw(node, crtc, after);
        }
    }

    /// The seat left this TTY: stop reading input and drawing.
    fn pause(&mut self) {
        let Some(hardware) = self.hardware() else {
            return;
        };
        log::info!("the seat left this terminal, so the compositor pauses");
        hardware.libinput.suspend();
        for card in hardware.devices.values_mut() {
            card.manager.pause();
        }
    }

    /// The seat came back to this TTY: take the devices back and redraw
    /// every screen. A card that opened while the seat was on another
    /// terminal takes the screen now: the seat holds DRM master for this
    /// terminal, so its connectors reset and read for the first time.
    fn resume(&mut self) {
        let mut screens = Vec::new();
        let mut first_reads = Vec::new();
        {
            let Some(hardware) = self.hardware() else {
                return;
            };
            log::info!("the seat is back on this terminal, so the compositor resumes");
            if hardware.libinput.resume().is_err() {
                log::warn!("libinput did not resume, so input may not reach the compositor");
            }
            for (node, card) in &mut hardware.devices {
                if let Err(err) = card.manager.activate(false) {
                    log::warn!("the card {node} did not come back: {err}");
                }
                if card.readiness.seat_arrived() {
                    log::info!(
                        "the seat is on this terminal now, so the card {node} takes the screen"
                    );
                    if let Err(err) = card.manager.device_mut().reset_state() {
                        log::warn!("the card {node} did not reset its connectors: {err}");
                    }
                    first_reads.push(*node);
                }
                for (crtc, screen) in &mut card.screens {
                    screen.queued = false;
                    screen.scheduled = false;
                    screens.push((*node, *crtc));
                }
            }
        }
        for node in first_reads {
            self.card_changed(node);
        }
        for (node, crtc) in screens {
            self.schedule_draw(node, crtc, Duration::ZERO);
        }
    }

    /// An input device arrived: a keyboard lights its LEDs and a touchpad
    /// taps to click.
    fn input_device_added(&mut self, mut device: input::Device) {
        let leds = self.keyboard.led_state();
        log::info!("the input device {} arrived", device.name());
        if device.config_tap_finger_count() > 0 {
            let _ = device.config_tap_set_enabled(true);
        }
        let Some(hardware) = self.hardware() else {
            return;
        };
        if device.has_capability(DeviceCapability::Keyboard) {
            device.led_update(leds.into());
            hardware.keyboards.push(device);
        }
    }

    /// An input device left.
    fn input_device_removed(&mut self, device: &input::Device) {
        log::info!("the input device {} left", device.name());
        if let Some(hardware) = self.hardware() {
            hardware.keyboards.retain(|held| held != device);
        }
    }
}

/// The name a screen on a connector carries: the kind of port and its
/// number, such as `DP-2` or `HDMI-A-3`, which is the name the kernel and
/// Hyprland give it.
fn connector_name(info: &connector::Info) -> String {
    format!("{}-{}", info.interface().as_str(), info.interface_id())
}

/// The mode a monitor prefers, or its first mode when it prefers none.
fn preferred_mode(info: &connector::Info) -> Option<smithay::reexports::drm::control::Mode> {
    info.modes()
        .iter()
        .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
        .or_else(|| info.modes().first())
        .copied()
}

/// The time between two refreshes of a mode, or a sixtieth of a second for
/// a mode that reports no rate.
fn refresh_of(mode: &Mode) -> Duration {
    refresh_interval(mode.refresh)
}

/// The time between two refreshes at a rate in millihertz.
pub fn refresh_interval(millihertz: i32) -> Duration {
    if millihertz <= 0 {
        return Duration::from_micros(16_667);
    }
    Duration::from_secs_f64(1000.0 / f64::from(millihertz))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_monitor_at_sixty_hertz_refreshes_every_sixtieth_of_a_second() {
        let interval = refresh_interval(60_000);
        assert!((interval.as_secs_f64() - 1.0 / 60.0).abs() < 1e-9);
        let fast = refresh_interval(143_998);
        assert!(fast < Duration::from_millis(7));
    }

    #[test]
    fn a_mode_with_no_rate_refreshes_at_sixty_hertz() {
        assert_eq!(refresh_interval(0), Duration::from_micros(16_667));
        assert_eq!(refresh_interval(-5), Duration::from_micros(16_667));
    }
}
