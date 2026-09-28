//! The Verse tab: Verse's bare world on a native Metal layer. It is the plaza's
//! ground grid in the neutral palette with Coder's player and its touch and
//! motion controls, and other players' avatars. It reads no chats or
//! computers. The world, controls, presence, and rendering are Coder's shared
//! mobile Verse surface ([`coder_mobile::VerseHandle`]); this module only
//! admits the host's configuration and carries its C ABI.
//!
//! With a world key, the tab joins the bare world's own NIP-MV world on the
//! public relay while it is active, for avatar presence alone: no chat,
//! gestures, companion, or profile. The world key is a separate protected
//! identity; the device key that holds host grants never signs world events.
//!
//! Create, call, and destroy a handle on the main thread while its
//! CAMetalLayer stays alive. Requests and replies are Coder's native Verse
//! JSON (`coder.verse.v1`).
use crate::{OpenAgentsMobileBuffer, buffer};
use coder_mobile::{BarePresence, VerseHandle};
use serde::Deserialize;
use std::cell::RefCell;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

thread_local! { static CREATE_ERROR: RefCell<Option<String>> = const { RefCell::new(None) }; }

/// A world request carries no panel feeds, so it stays small.
const MAX_REQUEST_BYTES: usize = 4096;

/// The host's mount: the layer's drawable size in pixels, its scale, whether
/// the layer is set up for extended dynamic range, and the world identity.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    width: u32,
    height: u32,
    scale: f32,
    #[serde(default)]
    hdr: bool,
    /// The world identity's secret as 64 hexadecimal characters: a key kept
    /// only for world presence, never the device key. Without it the world
    /// stays offline.
    #[serde(default)]
    world_secret_hex: Option<String>,
}

impl Config {
    fn presence(&self) -> Result<Option<BarePresence>, String> {
        let Some(secret) = &self.world_secret_hex else {
            return Ok(None);
        };
        if secret.len() != 64 || !secret.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("Invalid Verse world identity".into());
        }
        Ok(Some(BarePresence {
            secret_hex: secret.clone(),
            relay: None,
        }))
    }
}

/// # Safety
/// `layer` must be a live CAMetalLayer owned by the calling main thread, and
/// `bytes` must point to `len` readable bytes. Destroy the returned handle
/// before releasing the layer. A null result means the world could not start;
/// `openagents_verse_create_error` says why.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_verse_create(
    layer: *mut c_void,
    bytes: *const u8,
    len: usize,
) -> *mut VerseHandle {
    CREATE_ERROR.with(|error| *error.borrow_mut() = None);
    if layer.is_null() || bytes.is_null() || len == 0 || len > 1024 {
        return ptr::null_mut();
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
        let config: Config = serde_json::from_slice(bytes)
            .map_err(|_| "Invalid native Verse configuration".to_owned())?;
        let presence = config.presence()?;
        unsafe {
            VerseHandle::create_bare(
                layer,
                config.width,
                config.height,
                config.scale,
                config.hdr,
                presence,
            )
        }
    }));
    match result {
        Ok(Ok(handle)) => Box::into_raw(Box::new(handle)),
        error => {
            let message = match error {
                Ok(Err(message)) => message,
                _ => "The native renderer could not initialize".into(),
            };
            CREATE_ERROR.with(|error| *error.borrow_mut() = Some(message));
            ptr::null_mut()
        }
    }
}

/// Why the last `openagents_verse_create` on this thread failed, as UTF-8.
/// Empty when it did not fail. Free the result with
/// `openagents_mobile_buffer_free`.
#[unsafe(no_mangle)]
pub extern "C" fn openagents_verse_create_error() -> OpenAgentsMobileBuffer {
    buffer(CREATE_ERROR.with(|error| error.borrow().clone().unwrap_or_default().into_bytes()))
}

/// # Safety
/// `handle` must be a live handle from `openagents_verse_create`, used on its
/// creating main thread, and `bytes` must point to `len` readable bytes. Free
/// the result once with `openagents_mobile_buffer_free`. An empty result
/// means the request failed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_verse_call(
    handle: *mut VerseHandle,
    bytes: *const u8,
    len: usize,
) -> OpenAgentsMobileBuffer {
    if handle.is_null() || bytes.is_null() || len == 0 || len > MAX_REQUEST_BYTES {
        return buffer(vec![]);
    }
    catch_unwind(AssertUnwindSafe(|| {
        let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
        unsafe { &mut *handle }
            .call_bytes(bytes)
            .unwrap_or_default()
    }))
    .map(buffer)
    .unwrap_or_else(|_| buffer(vec![]))
}

/// # Safety
/// The handle must come from `openagents_verse_create`, be used on its
/// creating main thread with no call in progress, and not be destroyed
/// already. Its layer must stay alive until this returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_verse_destroy(handle: *mut VerseHandle) {
    if !handle.is_null() {
        unsafe { drop(Box::from_raw(handle)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_refuses_a_missing_layer_and_invalid_configuration() {
        let config = br#"{"width":1170,"height":2532,"scale":3.0}"#;
        let created =
            unsafe { openagents_verse_create(ptr::null_mut(), config.as_ptr(), config.len()) };
        assert!(created.is_null());
        // A non-null layer on macOS reaches the renderer, which requires iOS;
        // the configuration is checked first.
        let mut fake = 0u8;
        let layer = (&raw mut fake).cast::<c_void>();
        let bad = br#"{"width":1,"height":1,"scale":1.0,"relay":"wss://x"}"#;
        assert!(unsafe { openagents_verse_create(layer, bad.as_ptr(), bad.len()) }.is_null());
        let error = openagents_verse_create_error();
        let text = unsafe { std::slice::from_raw_parts(error.data, error.len) };
        assert_eq!(text, b"Invalid native Verse configuration");
        unsafe { crate::openagents_mobile_buffer_free(error) };
        // A world key must be exactly a 32-byte secret in hexadecimal.
        for key in ["zz".repeat(32), "11".repeat(31)] {
            let bad = serde_json::to_vec(&serde_json::json!({
                "width": 1, "height": 1, "scale": 1.0, "world_secret_hex": key,
            }))
            .unwrap();
            assert!(unsafe { openagents_verse_create(layer, bad.as_ptr(), bad.len()) }.is_null());
            let error = openagents_verse_create_error();
            let text = unsafe { std::slice::from_raw_parts(error.data, error.len) };
            assert_eq!(text, b"Invalid Verse world identity");
            unsafe { crate::openagents_mobile_buffer_free(error) };
        }
        #[cfg(not(target_os = "ios"))]
        {
            assert!(
                unsafe { openagents_verse_create(layer, config.as_ptr(), config.len()) }.is_null()
            );
            let error = openagents_verse_create_error();
            assert!(error.len > 0);
            unsafe { crate::openagents_mobile_buffer_free(error) };
        }
    }
}
