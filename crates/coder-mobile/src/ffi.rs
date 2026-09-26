//! The native caller owns each handle and returned buffer until its matching
//! release call. Calls for one handle are serialized by the native worker queue.
use crate::{App, Config, Request};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
};

#[repr(C)]
pub struct CoderMobileBuffer {
    pub data: *mut u8,
    pub len: usize,
}

pub(crate) fn buffer(bytes: Vec<u8>) -> CoderMobileBuffer {
    let mut bytes = bytes.into_boxed_slice();
    let result = CoderMobileBuffer {
        data: bytes.as_mut_ptr(),
        len: bytes.len(),
    };
    std::mem::forget(bytes);
    result
}
fn failure() -> CoderMobileBuffer {
    buffer(br#"{"schema":"coder.mobile.v1","public_key":"","paired":false,"reading":false,"status":"Unavailable","error":"Native bridge request failed.","follow_target":null,"follow_page":null,"view":null}"#.to_vec())
}

/// # Safety
/// `bytes` must point to `len` readable bytes for this call. The returned handle
/// must be destroyed once and may be used only from one serial native queue.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn coder_mobile_create(bytes: *const u8, len: usize) -> *mut App {
    if bytes.is_null() || len == 0 || len > 16 * 1024 {
        return ptr::null_mut();
    }
    catch_unwind(AssertUnwindSafe(|| {
        let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
        let config: Config = serde_json::from_slice(bytes).ok()?;
        App::new(config)
            .ok()
            .map(|app| Box::into_raw(Box::new(app)))
    }))
    .ok()
    .flatten()
    .unwrap_or(ptr::null_mut())
}

/// # Safety
/// `handle` must be a live handle from `coder_mobile_create`, with exclusive
/// access. `bytes` must point to `len` readable bytes. Free the returned buffer
/// once using `coder_mobile_buffer_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn coder_mobile_call(
    handle: *mut App,
    bytes: *const u8,
    len: usize,
) -> CoderMobileBuffer {
    if handle.is_null() || bytes.is_null() || len == 0 || len > 128 * 1024 {
        return failure();
    }
    catch_unwind(AssertUnwindSafe(|| {
        let request: Request =
            match serde_json::from_slice(unsafe { std::slice::from_raw_parts(bytes, len) }) {
                Ok(request) => request,
                Err(_) => return failure(),
            };
        let packet = unsafe { &mut *handle }.call(request);
        match serde_json::to_vec(&packet) {
            Ok(bytes) if bytes.len() <= 1024 * 1024 => buffer(bytes),
            _ => failure(),
        }
    }))
    .unwrap_or_else(|_| failure())
}

/// # Safety
/// The buffer must be an unmodified, not-yet-freed result of `coder_mobile_call`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn coder_mobile_buffer_free(value: CoderMobileBuffer) {
    if !value.data.is_null() {
        unsafe {
            drop(Box::from_raw(ptr::slice_from_raw_parts_mut(
                value.data, value.len,
            )));
        }
    }
}

/// # Safety
/// The handle must have been created here, have no in-flight calls, and not
/// have been destroyed already. Its native callbacks must be detached first.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn coder_mobile_destroy(handle: *mut App) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}
