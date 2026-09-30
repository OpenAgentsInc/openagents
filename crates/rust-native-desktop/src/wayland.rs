//! The window's Wayland seat, beside winit's and on winit's own connection:
//! drag and drop of files and images, and the clipboard, read and written
//! by this process itself.
//!
//! winit 0.30 has no drag and drop on Wayland, and reading the clipboard
//! through a helper (`wl-paste`) or `arboard` needs the compositor's
//! data-control protocol, which GNOME and CoderOS's compositor do not
//! offer. A client with keyboard focus reads and sets the clipboard through
//! the core `wl_data_device` on any compositor, so this module keeps one:
//!
//! - **Drop.** Over the window it accepts `text/uri-list` (a file manager's
//!   files), else PNG or JPEG pixels (a browser's image). Files arrive as
//!   their paths; pixels are written to a private file first, under
//!   `$XDG_RUNTIME_DIR/openagents/drops`, so both reach the application
//!   the same way winit's `DroppedFile` does on X11, macOS, and Windows.
//! - **Paste.** The compositor sends the focused client the current
//!   clipboard offer; [`paste`] asks it for the first wanted type.
//! - **Copy.** [`copy`] offers text as this window's own selection, with
//!   the serial of the last key or button, and serves it until another
//!   client takes the clipboard.
//!
//! Events for these objects are dispatched on a thread of their own
//! (`wayland-seat`), which reads the shared socket through libwayland's
//! prepare/read protocol, so neither thread waits on the other. Nothing is
//! read from the clipboard except when the person pastes.

use crate::transfer;
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::Duration;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_data_device::{self, WlDataDevice};
use wayland_client::protocol::wl_data_device_manager::{DndAction, WlDataDeviceManager};
use wayland_client::protocol::wl_data_offer::{self, WlDataOffer};
use wayland_client::protocol::wl_data_source::{self, WlDataSource};
use wayland_client::protocol::wl_keyboard::{self, WlKeyboard};
use wayland_client::protocol::wl_pointer::{self, WlPointer};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::{self, Capability, WlSeat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum, event_created_child};

/// How long a paste or a drop waits for the other client to hand over its
/// bytes.
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(5);

/// The types an offer carries, as the compositor announced them.
#[derive(Default)]
struct Mimes(Mutex<Vec<String>>);

impl Mimes {
    fn of(offer: &WlDataOffer) -> Vec<String> {
        offer
            .data::<Mimes>()
            .and_then(|mimes| mimes.0.lock().ok().map(|list| list.clone()))
            .unwrap_or_default()
    }
}

/// What the other threads read and change: the clipboard as the
/// compositor last offered it, and what this window offers.
struct Seat {
    connection: Connection,
    queue: QueueHandle<State>,
    manager: WlDataDeviceManager,
    device: WlDataDevice,
    /// The serial of the last key or button on this window's surfaces.
    serial: Option<u32>,
    /// The current clipboard offer, while this window has focus.
    selection: Option<WlDataOffer>,
    /// This window's own selection and the text it serves.
    source: Option<(WlDataSource, Arc<Vec<u8>>)>,
}

static SEAT: OnceLock<Arc<Mutex<Seat>>> = OnceLock::new();

/// A drag over the window.
struct Drag {
    offer: WlDataOffer,
    action: DndAction,
}

/// The dispatch thread's state.
struct State {
    seat: Arc<Mutex<Seat>>,
    wl_seat: WlSeat,
    keyboard: Option<WlKeyboard>,
    pointer: Option<WlPointer>,
    drag: Option<Drag>,
    drops: mpsc::Sender<PathBuf>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

/// Dropped files, for the window to hand to the application.
pub struct Drops(mpsc::Receiver<PathBuf>);

impl Drops {
    /// The files dropped since the last call.
    pub fn take(&self) -> Vec<PathBuf> {
        self.0.try_iter().collect()
    }
}

/// Joins the seat of `window`'s Wayland connection. `None` on X11, or when
/// the compositor has no data device; `wake` runs after a drop arrives.
pub fn attach(
    window: &winit::window::Window,
    wake: impl Fn() + Send + Sync + 'static,
) -> Option<Drops> {
    use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
    if SEAT.get().is_some() {
        return None;
    }
    let RawDisplayHandle::Wayland(handle) = window.display_handle().ok()?.as_raw() else {
        return None;
    };
    // SAFETY: the display is winit's, and it outlives this process's use
    // of it: the window, and so the event loop that owns the display, lives
    // until the process exits.
    let backend = unsafe {
        wayland_backend::client::Backend::from_foreign_display(handle.display.as_ptr().cast())
    };
    let connection = Connection::from_backend(backend);
    let (globals, mut queue) = registry_queue_init::<State>(&connection).ok()?;
    let qh = queue.handle();
    let manager: WlDataDeviceManager = globals.bind(&qh, 1..=3, ()).ok()?;
    let wl_seat: WlSeat = globals.bind(&qh, 1..=5, ()).ok()?;
    let device = manager.get_data_device(&wl_seat, &qh, ());
    let seat = Arc::new(Mutex::new(Seat {
        connection: connection.clone(),
        queue: qh,
        manager,
        device,
        serial: None,
        selection: None,
        source: None,
    }));
    SEAT.set(seat.clone()).ok()?;
    let (send, receive) = mpsc::channel();
    let mut state = State {
        seat,
        wl_seat,
        keyboard: None,
        pointer: None,
        drag: None,
        drops: send,
        wake: Arc::new(wake),
    };
    std::thread::Builder::new()
        .name("wayland-seat".into())
        .spawn(move || while queue.blocking_dispatch(&mut state).is_ok() {})
        .ok()?;
    let _ = connection.flush();
    if std::env::var_os("OPENAGENTS_CLIPBOARD_TRACE").is_some() {
        eprintln!("clipboard: joined the window's Wayland seat");
    }
    Some(Drops(receive))
}

/// Whether this process reads and writes the clipboard itself.
pub fn attached() -> bool {
    SEAT.get().is_some()
}

/// The types the clipboard offers now; `None` without an offer (this
/// window has no focus, or the clipboard is empty).
pub fn offered() -> Option<Vec<String>> {
    let seat = SEAT.get()?.lock().ok()?;
    seat.selection.as_ref().map(Mimes::of)
}

/// Reads the clipboard as the first of `wanted` it offers: the type and
/// the bytes. Blocks up to a few seconds while the other client writes, so
/// it runs off the window's thread.
pub fn paste(wanted: &[&str]) -> Option<(String, Vec<u8>)> {
    let (offer, connection) = {
        let seat = SEAT.get()?.lock().ok()?;
        (seat.selection.clone()?, seat.connection.clone())
    };
    let mime = transfer::pick(&Mimes::of(&offer), wanted)?;
    let bytes = receive(&connection, &offer, mime)?;
    Some((mime.to_owned(), bytes))
}

/// Offers `text` as the clipboard, as this window's selection. `false`
/// before any key or click on the window, when the compositor would refuse.
pub fn copy(text: &str) -> bool {
    set_selection(Some(text.as_bytes().to_vec()))
}

/// Empties the clipboard if this window owns it.
pub fn clear() -> bool {
    set_selection(None)
}

fn set_selection(content: Option<Vec<u8>>) -> bool {
    let Some(seat) = SEAT.get() else {
        return false;
    };
    let Ok(mut seat) = seat.lock() else {
        return false;
    };
    let Some(serial) = seat.serial else {
        return false;
    };
    let source = content.map(|bytes| {
        let source = seat.manager.create_data_source(&seat.queue, ());
        for mime in transfer::TEXT {
            source.offer((*mime).to_owned());
        }
        (source, Arc::new(bytes))
    });
    seat.device
        .set_selection(source.as_ref().map(|(s, _)| s), serial);
    if let Some((old, _)) = std::mem::replace(&mut seat.source, source) {
        old.destroy();
    }
    seat.connection.flush().is_ok()
}

/// Asks `offer` for `mime` through a pipe and reads what the other client
/// writes, up to [`transfer::MAX_BYTES`] and [`TRANSFER_TIMEOUT`].
fn receive(connection: &Connection, offer: &WlDataOffer, mime: &str) -> Option<Vec<u8>> {
    let (mut reader, writer) = std::io::pipe().ok()?;
    offer.receive(mime.to_owned(), writer.as_fd());
    drop(writer);
    connection.flush().ok()?;
    let (send, answer) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("wayland-transfer".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            let read = (&mut reader)
                .take(transfer::MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes);
            let _ = send.send(
                read.ok()
                    .filter(|_| bytes.len() <= transfer::MAX_BYTES)
                    .map(|_| bytes),
            );
        })
        .ok()?;
    answer.recv_timeout(TRANSFER_TIMEOUT).ok().flatten()
}

/// Where dropped pixels are written: `$XDG_RUNTIME_DIR/openagents/drops`
/// (`0700`), else a private folder under the temporary directory.
fn drop_dir() -> Option<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join("openagents"))
        .unwrap_or_else(|| {
            let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
            std::env::temp_dir().join(format!("openagents-{user}"))
        });
    let dir = base.join("drops");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .ok()?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).ok()?;
    // Pixels from earlier drops were read long ago.
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let old = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|at| at.elapsed().ok())
                .is_some_and(|age| age > Duration::from_secs(600));
            if old {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
    Some(dir)
}

/// The files a drop of `offer` carries: its paths, or its pixels written
/// to a private file.
fn dropped(connection: &Connection, offer: &WlDataOffer) -> Vec<PathBuf> {
    for mime in transfer::all(&Mimes::of(offer), transfer::DROP) {
        let Some(bytes) = receive(connection, offer, mime) else {
            continue;
        };
        if mime == "text/uri-list" {
            let paths = transfer::uri_list_paths(&bytes);
            if !paths.is_empty() {
                return paths;
            }
            continue;
        }
        if bytes.is_empty() {
            continue;
        }
        let Some(dir) = drop_dir() else {
            return vec![];
        };
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |at| at.as_nanos());
        // One folder a drop, so the attachment keeps a plain name.
        let folder = dir.join(nanos.to_string());
        let path = folder.join(format!("Dropped image.{}", transfer::extension(mime)));
        let written = std::fs::create_dir(&folder).and_then(|()| {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .and_then(|mut file| file.write_all(&bytes))
        });
        return if written.is_ok() { vec![path] } else { vec![] };
    }
    vec![]
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlDataDeviceManager, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlDataDeviceManager,
        _: <WlDataDeviceManager as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            if capabilities.contains(Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(state.wl_seat.get_keyboard(qh, ()));
            }
            if capabilities.contains(Capability::Pointer) && state.pointer.is_none() {
                state.pointer = Some(state.wl_seat.get_pointer(qh, ()));
            }
        }
    }
}

impl State {
    fn serial(&self, serial: u32) {
        if let Ok(mut seat) = self.seat.lock() {
            seat.serial = Some(serial);
        }
    }
}

impl Dispatch<WlKeyboard, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Enter { serial, .. } | wl_keyboard::Event::Key { serial, .. } => {
                state.serial(serial);
            }
            _ => {}
        }
    }
}

impl Dispatch<WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_pointer::Event::Button { serial, .. } = event {
            state.serial(serial);
        }
    }
}

impl Dispatch<WlDataDevice, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        connection: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::Enter {
                serial,
                id: Some(offer),
                ..
            } => {
                if let Some(old) = state.drag.take() {
                    old.offer.destroy();
                }
                let wanted = transfer::pick(&Mimes::of(&offer), transfer::DROP);
                offer.accept(serial, wanted.map(str::to_owned));
                if offer.version() >= 3 {
                    let actions = if wanted.is_some() {
                        DndAction::Copy | DndAction::Move
                    } else {
                        DndAction::empty()
                    };
                    offer.set_actions(actions, DndAction::Copy);
                }
                state.drag = Some(Drag {
                    offer,
                    action: DndAction::empty(),
                });
            }
            wl_data_device::Event::Leave => {
                if let Some(drag) = state.drag.take() {
                    drag.offer.destroy();
                }
            }
            wl_data_device::Event::Drop => {
                let Some(drag) = state.drag.take() else {
                    return;
                };
                let connection = connection.clone();
                let drops = state.drops.clone();
                let wake = state.wake.clone();
                // The other client writes while this thread keeps
                // dispatching; the finish waits for the bytes.
                let _ = std::thread::Builder::new()
                    .name("wayland-drop".into())
                    .spawn(move || {
                        let paths = dropped(&connection, &drag.offer);
                        if drag.offer.version() >= 3 && !drag.action.is_empty() && !paths.is_empty()
                        {
                            drag.offer.finish();
                        }
                        drag.offer.destroy();
                        let _ = connection.flush();
                        let any = !paths.is_empty();
                        for path in paths {
                            let _ = drops.send(path);
                        }
                        if any {
                            wake();
                        }
                    });
            }
            wl_data_device::Event::Selection { id } => {
                if let Ok(mut seat) = state.seat.lock()
                    && let Some(old) = std::mem::replace(&mut seat.selection, id)
                {
                    old.destroy();
                }
            }
            _ => {}
        }
    }

    event_created_child!(State, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, Mimes::default()),
    ]);
}

impl Dispatch<WlDataOffer, Mimes> for State {
    fn event(
        state: &mut Self,
        offer: &WlDataOffer,
        event: wl_data_offer::Event,
        mimes: &Mimes,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_offer::Event::Offer { mime_type } => {
                if let Ok(mut list) = mimes.0.lock() {
                    list.push(mime_type);
                }
            }
            wl_data_offer::Event::Action {
                dnd_action: WEnum::Value(action),
            } => {
                if let Some(drag) = state.drag.as_mut().filter(|drag| &drag.offer == offer) {
                    drag.action = action;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlDataSource, ()> for State {
    fn event(
        state: &mut Self,
        source: &WlDataSource,
        event: wl_data_source::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Ok(mut seat) = state.seat.lock() else {
            return;
        };
        let ours = seat
            .source
            .as_ref()
            .filter(|(own, _)| own == source)
            .map(|(_, bytes)| bytes.clone());
        match event {
            wl_data_source::Event::Send { fd, .. } => {
                let Some(bytes) = ours else {
                    return;
                };
                // A slow reader never holds the dispatch thread.
                let _ = std::thread::Builder::new()
                    .name("wayland-serve".into())
                    .spawn(move || {
                        let _ = std::fs::File::from(fd).write_all(&bytes);
                    });
            }
            wl_data_source::Event::Cancelled => {
                if ours.is_some() {
                    seat.source = None;
                }
                source.destroy();
            }
            _ => {}
        }
    }
}
