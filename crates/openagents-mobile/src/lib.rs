//! The OpenAgents mobile app's Rust library. Rust owns the app's state and
//! builds each screen as a Rust Native view; the thin SwiftUI host decodes
//! and renders it and forwards activations and the few values it collects.
//!
//! The app has four surfaces. **Computers** is Coder's shared Computers
//! controller over the live host client: it enrolls this phone with hosts
//! (NIP-HOST), follows their presence and routes (NIP-REACH), and commands
//! them (tasks and terminals). **Coder** starts a chat with Coder on a
//! computer as a NIP-HOST task and follows it. **Chats** lists the Claude and Codex chats
//! saved on every computer paired for reading. **Tailnet** lists the devices on the user's
//! tailnet through Tailscale's control server. **Verse** mounts Verse's bare
//! world, the plaza grid with Coder's player controls, on a native Metal layer.
//! **Wallet** is a Bitcoin wallet on mainnet through Breez's Spark SDK:
//! Lightning, Spark, and on-chain receive and send, with the seed in the
//! platform's key store.

mod account;
mod amounts;
#[cfg(any(target_os = "android", test))]
mod android;
mod app;
mod chats;
mod coder_list;
mod coder_tab;
mod computers_home;
mod conversation;
mod outbox;
mod payees;
mod spark;
mod spend;
mod tailnet;
mod tailnet_view;
mod trainer;
mod transcripts;
mod verse;
mod wallet;
mod wallet_fixture;

pub use app::{App, Config, Launch, Packet, Request};

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

#[repr(C)]
pub struct OpenAgentsMobileBuffer {
    pub data: *mut u8,
    pub len: usize,
}

fn buffer(bytes: Vec<u8>) -> OpenAgentsMobileBuffer {
    let mut bytes = bytes.into_boxed_slice();
    let result = OpenAgentsMobileBuffer {
        data: if bytes.is_empty() {
            ptr::null_mut()
        } else {
            bytes.as_mut_ptr()
        },
        len: bytes.len(),
    };
    std::mem::forget(bytes);
    result
}

/// # Safety
/// `bytes` must point to `len` readable bytes for this call. Destroy the
/// returned handle once, and call it from one serial queue.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_mobile_create(bytes: *const u8, len: usize) -> *mut App {
    if bytes.is_null() || len == 0 || len > 16 * 1024 {
        return ptr::null_mut();
    }
    catch_unwind(AssertUnwindSafe(|| {
        let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
        let config: Config = serde_json::from_slice(bytes).ok()?;
        let launch: Launch = serde_json::from_slice(bytes).ok()?;
        App::open(config, launch)
            .ok()
            .map(|app| Box::into_raw(Box::new(app)))
    }))
    .ok()
    .flatten()
    .unwrap_or(ptr::null_mut())
}

/// # Safety
/// `handle` must be a live handle from `openagents_mobile_create` with
/// exclusive access, and `bytes` must point to `len` readable bytes. Free the
/// result once with `openagents_mobile_buffer_free`. An empty result means
/// the request failed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_mobile_call(
    handle: *mut App,
    bytes: *const u8,
    len: usize,
) -> OpenAgentsMobileBuffer {
    if handle.is_null() || bytes.is_null() || len == 0 || len > 128 * 1024 {
        return buffer(vec![]);
    }
    catch_unwind(AssertUnwindSafe(|| {
        let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
        let Ok(request) = serde_json::from_slice::<Request>(bytes) else {
            return vec![];
        };
        unsafe { &mut *handle }.respond(request)
    }))
    .map(buffer)
    .unwrap_or_else(|_| buffer(vec![]))
}

/// # Safety
/// The buffer must be an unmodified, not-yet-freed result from this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_mobile_buffer_free(value: OpenAgentsMobileBuffer) {
    if !value.data.is_null() {
        unsafe {
            drop(Box::from_raw(ptr::slice_from_raw_parts_mut(
                value.data, value.len,
            )))
        }
    }
}

/// # Safety
/// The handle must come from `openagents_mobile_create`, have no call in
/// progress, and not be destroyed already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_mobile_destroy(handle: *mut App) {
    if !handle.is_null() {
        unsafe { drop(Box::from_raw(handle)) }
    }
}

#[cfg(test)]
mod coder_tab_tests;
#[cfg(test)]
mod tests;
