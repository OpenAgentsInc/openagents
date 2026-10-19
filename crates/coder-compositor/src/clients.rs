//! Admitting a Wayland client, and how much its connection may hold.
//!
//! The server half of `wayland-backend` disconnects a client when an event
//! no longer fits in its outgoing buffer. The default buffer is 4 KiB above
//! the kernel's socket buffer (about 208 KiB on Linux), so a client whose
//! main thread stalls for a few seconds while the pointer moves over it is
//! dropped: every motion and frame event queues for it, the buffer fills,
//! and the client sees only a broken pipe on its next flush (#12175).
//! Both backends admit clients here, with a buffer large enough to carry
//! a stalled client through a long pause.

use std::io;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use smithay::reexports::wayland_server::backend::ClientData;
use smithay::reexports::wayland_server::{Client, DisplayHandle};

use crate::state::ClientState;

/// The most bytes of events the compositor holds for one client that is
/// not reading, above what the kernel's socket buffer holds. At 1000
/// pointer events a second (a motion and a frame are 60 bytes together)
/// this lasts more than a minute, and the buffer only grows while events
/// wait.
pub const CLIENT_BUFFER_BYTES: usize = 4 << 20;

/// Admits one client that connected to the compositor's socket.
pub fn admit(handle: &mut DisplayHandle, stream: UnixStream) -> io::Result<Client> {
    admit_with(handle, stream, Arc::new(ClientState::default()))
}

/// Admits one client with the given per-client data.
fn admit_with(
    handle: &mut DisplayHandle,
    stream: UnixStream,
    data: Arc<dyn ClientData>,
) -> io::Result<Client> {
    let client = handle.insert_client(stream, data)?;
    client.set_max_buffer_size(handle, CLIENT_BUFFER_BYTES);
    Ok(client)
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};

    use smithay::reexports::wayland_server::Display;
    use smithay::reexports::wayland_server::backend::{ClientId, DisconnectReason};

    use super::*;

    /// Records whether the server dropped the client.
    struct Watch(Arc<AtomicBool>);

    impl ClientData for Watch {
        fn disconnected(&self, _id: ClientId, _reason: DisconnectReason) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    /// Sends `syncs` `wl_display.sync` requests from a client that never
    /// reads, so each answer (a `wl_callback.done` and a
    /// `wl_display.delete_id`, 24 bytes) waits for it, and says whether the
    /// server still holds the client at the end.
    fn survives_unread_events(admitted: bool, syncs: u32) -> bool {
        let mut display: Display<()> = Display::new().expect("a display");
        let mut handle = display.handle();
        let (server, mut client) = UnixStream::pair().expect("a socket pair");
        let gone = Arc::new(AtomicBool::new(false));
        let watch = Arc::new(Watch(gone.clone()));
        if admitted {
            admit_with(&mut handle, server, watch).expect("admitted");
        } else {
            handle.insert_client(server, watch).expect("inserted");
        }

        let mut next_id = 2u32;
        while next_id < syncs + 2 {
            let mut batch = Vec::with_capacity(256 * 12);
            for _ in 0..256 {
                batch.extend_from_slice(&1u32.to_ne_bytes());
                batch.extend_from_slice(&(12u32 << 16).to_ne_bytes());
                batch.extend_from_slice(&next_id.to_ne_bytes());
                next_id += 1;
            }
            if client.write_all(&batch).is_err() {
                return false;
            }
            let _ = display.dispatch_clients(&mut ());
            let _ = display.flush_clients();
            if gone.load(Ordering::SeqCst) {
                return false;
            }
        }
        !gone.load(Ordering::SeqCst)
    }

    #[test]
    fn the_default_buffer_drops_a_client_that_stops_reading() {
        // About 960 KiB of answers: past the kernel's buffer and 4 KiB.
        assert!(!survives_unread_events(false, 40_000));
    }

    #[test]
    fn an_admitted_client_survives_a_long_stall() {
        assert!(survives_unread_events(true, 40_000));
    }
}
