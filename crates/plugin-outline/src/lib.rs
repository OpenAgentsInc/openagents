//! Diagnostic outline guest.
//!
//! `echo` returns the input value. `outline` lists the names of a granted
//! snapshot when this build imports `oa_host.call`. The guest allocates
//! from one arena in linear memory and does not call the host allocator.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

use plugin_pdk::{Request, Response, VERSION, pack};
use serde_json::Value;

const HEAP_BYTES: usize = 1024 * 1024;

struct Arena {
    bytes: UnsafeCell<Vec<u8>>,
    bump: AtomicUsize,
}

// The guest is one Wasm instance. Exports are not called from two threads,
// and the host does not re-enter `oa_alloc` while a host call is on the stack.
unsafe impl Sync for Arena {}

static ARENA: Arena = Arena {
    bytes: UnsafeCell::new(Vec::new()),
    bump: AtomicUsize::new(0),
};

fn heap() -> &'static mut Vec<u8> {
    let heap = unsafe { &mut *ARENA.bytes.get() };
    if heap.is_empty() {
        heap.resize(HEAP_BYTES, 0);
    }
    heap
}

fn alloc(len: i32) -> i32 {
    if len < 0 {
        return 0;
    }
    let len = len as u32 as usize;
    let heap = heap();
    let start = ARENA.bump.fetch_add(len, Ordering::Relaxed);
    if start.checked_add(len).is_none_or(|end| end > heap.len()) {
        return 0;
    }
    let base = heap.as_mut_ptr() as usize;
    (base + start) as i32
}

fn read(ptr: i32, len: i32) -> Option<Vec<u8>> {
    if len < 0 || (ptr == 0 && len != 0) {
        return None;
    }
    let heap = heap();
    let base = heap.as_ptr() as usize;
    let ptr = ptr as u32 as usize;
    let len = len as u32 as usize;
    let offset = ptr.checked_sub(base)?;
    let end = offset.checked_add(len)?;
    if end > heap.len() {
        return None;
    }
    Some(heap[offset..end].to_vec())
}

fn write_at(ptr: i32, bytes: &[u8]) -> bool {
    let heap = heap();
    let base = heap.as_ptr() as usize;
    let ptr = ptr as u32 as usize;
    let Some(offset) = ptr.checked_sub(base) else {
        return false;
    };
    let Some(end) = offset.checked_add(bytes.len()) else {
        return false;
    };
    if end > heap.len() {
        return false;
    }
    heap[offset..end].copy_from_slice(bytes);
    true
}

#[cfg(feature = "snapshot")]
#[link(wasm_import_module = "oa_host")]
unsafe extern "C" {
    #[link_name = "call"]
    fn host_call(req: i32, req_len: i32, resp: i32, resp_cap: i32) -> i32;
}

fn write_response(response: &Response) -> i64 {
    let Ok(bytes) = serde_json::to_vec(response) else {
        return 0;
    };
    let ptr = alloc(bytes.len() as i32);
    if ptr == 0 && !bytes.is_empty() {
        return 0;
    }
    if !write_at(ptr, &bytes) {
        return 0;
    }
    pack(ptr as u32, bytes.len() as u32)
}

fn refused(request: &Request, status: &str, reason: &str) -> Response {
    Response {
        v: VERSION.to_string(),
        requires: Vec::new(),
        invocation: request.invocation.clone(),
        status: status.to_string(),
        value: Value::Null,
        reason: Some(reason.to_string()),
    }
}

#[cfg(feature = "snapshot")]
fn outline_entries(request: &Request) -> Vec<String> {
    let Some(handle) = request.handles.values().find_map(Value::as_str) else {
        return Vec::new();
    };
    let handle = handle.to_string();
    let import = serde_json::json!({
        "v": 1,
        "handle": handle,
        "operation": "list",
        "args": {"cursor": null, "max_entries": 32}
    });
    let Ok(bytes) = serde_json::to_vec(&import) else {
        return Vec::new();
    };
    let req_ptr = alloc(bytes.len() as i32);
    if !write_at(req_ptr, &bytes) {
        return Vec::new();
    }
    let cap = 8192;
    let resp_ptr = alloc(cap);
    let written = unsafe { host_call(req_ptr, bytes.len() as i32, resp_ptr, cap) };
    if written < 0 {
        return Vec::new();
    }
    let Some(resp) = read(resp_ptr, written) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_slice::<Value>(&resp) else {
        return Vec::new();
    };
    value["entries"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(not(feature = "snapshot"))]
fn outline_entries(_request: &Request) -> Vec<String> {
    Vec::new()
}

#[unsafe(no_mangle)]
pub extern "C" fn oa_alloc(len: i32) -> i32 {
    alloc(len)
}

#[unsafe(no_mangle)]
pub extern "C" fn oa_free(_ptr: i32, _len: i32) {}

#[unsafe(no_mangle)]
pub extern "C" fn oa_handle(ptr: i32, len: i32) -> i64 {
    let Some(bytes) = read(ptr, len) else {
        return 0;
    };
    let Ok(request) = serde_json::from_slice::<Request>(&bytes) else {
        return write_response(&Response {
            v: VERSION.to_string(),
            requires: Vec::new(),
            invocation: String::new(),
            status: "unsupported_input".to_string(),
            value: Value::Null,
            reason: Some("request".to_string()),
        });
    };
    if request.v != VERSION || !request.requires.is_empty() {
        return write_response(&refused(&request, "unsupported_input", "version"));
    }
    let value = match request.operation.as_str() {
        "echo" => request.input.clone(),
        "outline" => serde_json::json!({
            "kind": "outline",
            "entries": outline_entries(&request)
        }),
        _ => return write_response(&refused(&request, "unsupported_input", "operation")),
    };
    write_response(&Response {
        v: VERSION.to_string(),
        requires: Vec::new(),
        invocation: request.invocation,
        status: "ok".to_string(),
        value,
        reason: None,
    })
}
