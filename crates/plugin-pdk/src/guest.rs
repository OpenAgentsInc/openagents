//! Guest-side helpers for `openagents.plugin-packet.v1`.
//!
//! A guest crate writes one [`Handler`]: a function from the request and a
//! [`Host`] to a JSON value or a [`Refusal`]. [`crate::export_guest!`]
//! exports it as `oa_alloc`, `oa_free`, and `oa_handle` on `wasm32`, and
//! links the one host import, `oa_host.call`, through which a
//! `snapshot-read` guest lists and reads the handles its invocation was
//! granted.
//!
//! Everything except the exports and the import is ordinary Rust that
//! builds on any target, so a guest's logic runs in native tests against
//! [`MemoryHost`], which answers the same list, metadata, and read calls
//! from files held in memory.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::{Request, Response, VERSION};

/// The largest host-call response a guest accepts, in bytes.
pub const RESPONSE_CAP: usize = 256 * 1024;

/// The bytes one read call asks for.
pub const READ_CHUNK: usize = 64 * 1024;

/// The entries one listing call asks for.
pub const LIST_PAGE: usize = 256;

/// The host couldn't parse the call, or the call named something invalid.
pub const MALFORMED: i32 = -1;
/// The operation isn't supported on that handle, such as reading a
/// directory.
pub const UNSUPPORTED: i32 = -2;
/// The call would cross the invocation's read budget.
pub const BUDGET: i32 = -3;
/// The response didn't fit the guest's buffer or the output budget.
pub const TOO_LARGE: i32 = -4;
/// The handle, cursor, or version isn't this invocation's.
pub const STALE: i32 = -5;

/// Why a guest returned no value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// `unsupported_input` or `refused`.
    pub status: &'static str,
    /// Bounded refusal text.
    pub reason: String,
}

impl Refusal {
    /// The input isn't one this operation reads.
    #[must_use]
    pub fn unsupported(reason: impl Into<String>) -> Self {
        Refusal {
            status: "unsupported_input",
            reason: reason.into(),
        }
    }

    /// The guest declines to answer.
    #[must_use]
    pub fn refused(reason: impl Into<String>) -> Self {
        Refusal {
            status: "refused",
            reason: reason.into(),
        }
    }
}

/// One guest operation: the request and the host, to a value or a refusal.
pub type Handler = fn(&Request, &mut dyn Host) -> Result<Value, Refusal>;

/// The `oa_host.call` import: one JSON request in, one JSON response or a
/// negative error code out.
pub trait Host {
    /// Send one import request.
    ///
    /// # Errors
    ///
    /// Returns the host's negative error code, such as [`BUDGET`].
    fn call(&mut self, request: &Value) -> Result<Value, i32>;
}

/// One entry a listing returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// The invocation-scoped token that reads the entry.
    pub handle: String,
    /// The entry's logical name, such as `workspace/src/lib.rs`.
    pub name: String,
    /// `file`, `directory`, or `symlink`.
    pub kind: String,
}

/// A directory's entries, and whether the listing reached its end.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    /// Entries in listing order.
    pub entries: Vec<Listed>,
    /// False when `max_entries` stopped the listing early.
    pub complete: bool,
}

/// A file's bytes, and whether they are all of the bytes the snapshot
/// holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Read {
    /// The bytes read, from offset zero.
    pub bytes: Vec<u8>,
    /// False when a bound stopped the read before the end of the file.
    pub complete: bool,
}

/// The handle the host granted for the workspace: the one named
/// `workspace`, or else the first handle.
#[must_use]
pub fn root(request: &Request) -> Option<String> {
    request
        .handles
        .get("workspace")
        .or_else(|| request.handles.values().next())
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// The path of an entry relative to the granted root: `workspace/a/b`
/// becomes `a/b`.
#[must_use]
pub fn relative(name: &str) -> &str {
    name.split_once('/').map_or(name, |(_, rest)| rest)
}

/// List a directory handle, at most `max_entries` entries.
///
/// # Errors
///
/// Returns the host's error code when the first page fails.
pub fn list(host: &mut dyn Host, handle: &str, max_entries: usize) -> Result<Listing, i32> {
    let mut listing = Listing::default();
    let mut cursor = Value::Null;
    loop {
        let left = max_entries.saturating_sub(listing.entries.len());
        if left == 0 {
            return Ok(listing);
        }
        let page = host.call(&json!({
            "v": 1,
            "handle": handle,
            "operation": "list",
            "args": {"cursor": cursor, "max_entries": left.min(LIST_PAGE)}
        }));
        let page = match page {
            Ok(page) => page,
            Err(code) if listing.entries.is_empty() => return Err(code),
            Err(_) => return Ok(listing),
        };
        for entry in page["entries"].as_array().into_iter().flatten() {
            let (Some(handle), Some(name)) = (entry["handle"].as_str(), entry["name"].as_str())
            else {
                continue;
            };
            listing.entries.push(Listed {
                handle: handle.to_string(),
                name: name.to_string(),
                kind: entry["type"].as_str().unwrap_or("file").to_string(),
            });
        }
        match &page["next_cursor"] {
            Value::String(next) => cursor = Value::String(next.clone()),
            _ => {
                listing.complete = true;
                return Ok(listing);
            }
        }
    }
}

/// The size of the file a handle names, as the snapshot holds it.
///
/// # Errors
///
/// Returns the host's error code.
pub fn size(host: &mut dyn Host, handle: &str) -> Result<u64, i32> {
    let value = host.call(&json!({
        "v": 1,
        "handle": handle,
        "operation": "metadata",
        "args": {}
    }))?;
    value["size"].as_u64().ok_or(MALFORMED)
}

/// Read at most `max_bytes` of a file handle from offset zero, in chunks
/// of at most [`READ_CHUNK`]. The file's size comes first, so a small file
/// asks for no more than it holds and a nearly spent read budget still
/// reads it. A budget that runs out after some bytes were read returns
/// those bytes, marked incomplete.
///
/// # Errors
///
/// Returns the host's error code when no bytes could be read.
pub fn read(host: &mut dyn Host, handle: &str, max_bytes: usize) -> Result<Read, i32> {
    let mut read = Read::default();
    let total = size(host, handle).map_or(usize::MAX, |size| {
        usize::try_from(size).unwrap_or(usize::MAX)
    });
    if total == 0 {
        read.complete = true;
        return Ok(read);
    }
    loop {
        let chunk = READ_CHUNK
            .min(max_bytes.saturating_sub(read.bytes.len()))
            .min(total.saturating_sub(read.bytes.len()));
        if chunk == 0 {
            read.complete = read.bytes.len() >= total;
            return Ok(read);
        }
        let value = host.call(&json!({
            "v": 1,
            "handle": handle,
            "operation": "read",
            "args": {"offset": read.bytes.len(), "max_bytes": chunk}
        }));
        let value = match value {
            Ok(value) => value,
            Err(code) if read.bytes.is_empty() => return Err(code),
            Err(_) => return Ok(read),
        };
        let bytes = value["bytes_base64"]
            .as_str()
            .map_or(Some(Vec::new()), decode_base64)
            .ok_or(MALFORMED)?;
        let eof = value["eof"].as_bool().unwrap_or(true);
        let empty = bytes.is_empty();
        read.bytes.extend_from_slice(&bytes);
        if eof {
            read.complete = true;
            return Ok(read);
        }
        if empty {
            return Ok(read);
        }
    }
}

/// Answer one packet: parse the request, check its version, run the
/// handler, and serialize the response.
#[must_use]
pub fn respond(input: &[u8], host: &mut dyn Host, handler: Handler) -> Vec<u8> {
    let response = match serde_json::from_slice::<Request>(input) {
        Err(_) => refusal(String::new(), &Refusal::unsupported("request")),
        Ok(request) if request.v != VERSION || !request.requires.is_empty() => {
            refusal(request.invocation, &Refusal::unsupported("version"))
        }
        Ok(request) => match handler(&request, host) {
            Ok(value) => Response {
                v: VERSION.to_string(),
                requires: Vec::new(),
                invocation: request.invocation,
                status: "ok".to_string(),
                value,
                reason: None,
            },
            Err(refused) => refusal(request.invocation, &refused),
        },
    };
    serde_json::to_vec(&response).unwrap_or_default()
}

fn refusal(invocation: String, refused: &Refusal) -> Response {
    Response {
        v: VERSION.to_string(),
        requires: Vec::new(),
        invocation,
        status: refused.status.to_string(),
        value: Value::Null,
        reason: Some(refused.reason.chars().take(200).collect()),
    }
}

/// Decode padded base64, as the host's read call encodes it.
#[must_use]
pub fn decode_base64(text: &str) -> Option<Vec<u8>> {
    fn value(byte: u8) -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some(u32::from(byte - b'A')),
            b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let pad = usize::from(chunk[2] == b'=') + usize::from(chunk[3] == b'=');
        let mut packed = 0_u32;
        for (index, byte) in chunk.iter().enumerate() {
            let digit = if *byte == b'=' && index >= 4 - pad {
                0
            } else {
                value(*byte)?
            };
            packed = (packed << 6) | digit;
        }
        out.push((packed >> 16) as u8);
        if pad < 2 {
            out.push((packed >> 8) as u8);
        }
        if pad < 1 {
            out.push(packed as u8);
        }
    }
    Some(out)
}

/// Encode bytes as padded base64.
#[must_use]
pub fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// A host that answers from files in memory, laid out as the Coder
/// runtime grants a workspace: one directory, `workspace`, whose handle is
/// `root`, listing each file as `workspace/<relative path>`.
///
/// It keeps the host's read accounting: a read asking for more than the
/// budget has left fails with [`BUDGET`].
#[derive(Debug, Clone)]
pub struct MemoryHost {
    files: BTreeMap<String, Vec<u8>>,
    /// Snapshot bytes the invocation may still read.
    pub read_left: usize,
    /// Host calls answered so far.
    pub calls: usize,
}

impl MemoryHost {
    /// A host over `files`, each named by its workspace-relative path, with
    /// an unbounded read budget.
    #[must_use]
    pub fn new<N: Into<String>, B: Into<Vec<u8>>>(files: impl IntoIterator<Item = (N, B)>) -> Self {
        MemoryHost {
            files: files
                .into_iter()
                .map(|(name, bytes)| (format!("workspace/{}", name.into()), bytes.into()))
                .collect(),
            read_left: usize::MAX,
            calls: 0,
        }
    }

    /// A host over every file under `root`, named by its `/`-separated
    /// path relative to `root`, in sorted order. For native fixture tests.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn from_dir(root: &std::path::Path) -> Self {
        fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, Vec<u8>)>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            let mut paths: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
            paths.sort();
            for path in paths {
                if path.is_dir() {
                    walk(root, &path, out);
                } else if let (Ok(bytes), Ok(relative)) =
                    (std::fs::read(&path), path.strip_prefix(root))
                {
                    let name = relative
                        .components()
                        .map(|part| part.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("/");
                    out.push((name, bytes));
                }
            }
        }
        let mut files = Vec::new();
        walk(root, root, &mut files);
        Self::new(files)
    }

    /// The same host with a read budget.
    #[must_use]
    pub fn with_read_budget(mut self, bytes: usize) -> Self {
        self.read_left = bytes;
        self
    }

    /// A request for `operation` with `input`, granting the `workspace`
    /// handle.
    #[must_use]
    pub fn request(operation: &str, input: Value) -> Request {
        let mut handles = serde_json::Map::new();
        handles.insert("workspace".to_string(), Value::String("root".to_string()));
        Request {
            v: VERSION.to_string(),
            requires: Vec::new(),
            invocation: "test".to_string(),
            operation: operation.to_string(),
            input,
            handles,
        }
    }
}

impl Host for MemoryHost {
    fn call(&mut self, request: &Value) -> Result<Value, i32> {
        self.calls += 1;
        let handle = request["handle"].as_str().ok_or(MALFORMED)?;
        let args = &request["args"];
        match (request["operation"].as_str(), handle) {
            (Some("list"), "root") => {
                let start = match &args["cursor"] {
                    Value::String(cursor) => cursor.parse::<usize>().map_err(|_| STALE)?,
                    _ => 0,
                };
                let max = args["max_entries"].as_u64().unwrap_or(16) as usize;
                let names: Vec<&String> = self.files.keys().collect();
                if start > names.len() {
                    return Err(STALE);
                }
                let end = (start + max).min(names.len());
                let entries: Vec<Value> = names[start..end]
                    .iter()
                    .map(|name| json!({"handle": name, "name": name, "type": "file", "version": "v"}))
                    .collect();
                let next = if end < names.len() {
                    Value::String(end.to_string())
                } else {
                    Value::Null
                };
                Ok(json!({"entries": entries, "next_cursor": next, "complete": end == names.len()}))
            }
            (Some("list" | "read"), _) if !self.files.contains_key(handle) => Err(STALE),
            (Some("list"), _) => Err(UNSUPPORTED),
            (Some("metadata"), _) => {
                let bytes = self.files.get(handle).ok_or(STALE)?;
                Ok(json!({"type": "file", "size": bytes.len(), "version": "v"}))
            }
            (Some("read"), _) => {
                let offset = args["offset"].as_u64().unwrap_or(0) as usize;
                let max = args["max_bytes"].as_u64().unwrap_or(1024) as usize;
                if max > self.read_left {
                    return Err(BUDGET);
                }
                let bytes = &self.files[handle];
                if offset > bytes.len() {
                    return Err(MALFORMED);
                }
                let end = (offset + max).min(bytes.len());
                let encoded = encode_base64(&bytes[offset..end]);
                self.read_left = self.read_left.saturating_sub(encoded.len().min(max));
                Ok(json!({
                    "offset": offset,
                    "bytes_base64": encoded,
                    "eof": end == bytes.len(),
                    "version": "v"
                }))
            }
            _ => Err(UNSUPPORTED),
        }
    }
}

/// The `oa_host.call` import, on the guest's target. It keeps one
/// response buffer for the invocation, so a call doesn't allocate and clear
/// [`RESPONSE_CAP`] bytes again.
#[cfg(target_arch = "wasm32")]
pub struct WasmHost {
    response: Vec<u8>,
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "oa_host")]
unsafe extern "C" {
    #[link_name = "call"]
    fn host_call(req: i32, req_len: i32, resp: i32, resp_cap: i32) -> i32;
}

#[cfg(target_arch = "wasm32")]
impl Host for WasmHost {
    fn call(&mut self, request: &Value) -> Result<Value, i32> {
        let bytes = serde_json::to_vec(request).map_err(|_| MALFORMED)?;
        if self.response.len() != RESPONSE_CAP {
            self.response = vec![0_u8; RESPONSE_CAP];
        }
        let response = &mut self.response;
        // SAFETY: both ranges are live, disjoint allocations of this
        // instance's linear memory, and the host writes at most
        // `RESPONSE_CAP` bytes into the second.
        let written = unsafe {
            host_call(
                bytes.as_ptr() as i32,
                bytes.len() as i32,
                response.as_mut_ptr() as i32,
                RESPONSE_CAP as i32,
            )
        };
        if written < 0 {
            return Err(written);
        }
        serde_json::from_slice(&response[..written as usize]).map_err(|_| MALFORMED)
    }
}

/// `oa_alloc`: `len` bytes the host may write a packet into.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn alloc(len: i32) -> i32 {
    let Ok(size) = usize::try_from(len) else {
        return 0;
    };
    let Ok(layout) = std::alloc::Layout::from_size_align(size.max(1), 1) else {
        return 0;
    };
    // SAFETY: the layout has a non-zero size.
    let ptr = unsafe { std::alloc::alloc(layout) };
    ptr as i32
}

/// `oa_free`: release a range [`alloc`] returned.
#[cfg(target_arch = "wasm32")]
pub fn free(ptr: i32, len: i32) {
    let Ok(size) = usize::try_from(len) else {
        return;
    };
    if ptr == 0 {
        return;
    }
    let Ok(layout) = std::alloc::Layout::from_size_align(size.max(1), 1) else {
        return;
    };
    // SAFETY: the host frees only the ranges `alloc` returned, with the
    // lengths it asked for, once each.
    unsafe { std::alloc::dealloc(ptr as usize as *mut u8, layout) };
}

/// `oa_handle`: answer the packet at `ptr` and return the packed range of
/// the response.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn handle(ptr: i32, len: i32, handler: Handler) -> i64 {
    let Ok(size) = usize::try_from(len) else {
        return 0;
    };
    // SAFETY: the host wrote `len` bytes at `ptr`, a range `alloc` returned.
    let input = unsafe { std::slice::from_raw_parts(ptr as usize as *const u8, size) };
    let output = respond(
        input,
        &mut WasmHost {
            response: Vec::new(),
        },
        handler,
    );
    let out = alloc(output.len() as i32);
    if out == 0 {
        return 0;
    }
    // SAFETY: `out` is a fresh allocation of `output.len()` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(output.as_ptr(), out as usize as *mut u8, output.len());
    }
    crate::pack(out as u32, output.len() as u32)
}

/// Export a [`Handler`] as the guest's `oa_alloc`, `oa_free`, and
/// `oa_handle`. The exports exist only on `wasm32`, so the crate's native
/// tests link without the host import.
#[macro_export]
macro_rules! export_guest {
    ($handler:path) => {
        #[cfg(target_arch = "wasm32")]
        #[unsafe(no_mangle)]
        pub extern "C" fn oa_alloc(len: i32) -> i32 {
            $crate::guest::alloc(len)
        }

        #[cfg(target_arch = "wasm32")]
        #[unsafe(no_mangle)]
        pub extern "C" fn oa_free(ptr: i32, len: i32) {
            $crate::guest::free(ptr, len)
        }

        #[cfg(target_arch = "wasm32")]
        #[unsafe(no_mangle)]
        pub extern "C" fn oa_handle(ptr: i32, len: i32) -> i64 {
            $crate::guest::handle(ptr, len, $handler)
        }

        /// The exported handler, named on every target so a native build
        /// type-checks it and counts it as used.
        #[doc(hidden)]
        pub const OA_HANDLER: $crate::guest::Handler = $handler;
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips() {
        for text in ["", "a", "ab", "abc", "abcd", "hello, world"] {
            let encoded = encode_base64(text.as_bytes());
            assert_eq!(decode_base64(&encoded).unwrap(), text.as_bytes());
        }
        assert!(decode_base64("abc").is_none());
        assert!(decode_base64("ab!=").is_none());
    }

    #[test]
    fn listing_pages_and_reads_stop_at_the_budget() {
        let files: Vec<(String, Vec<u8>)> = (0..300)
            .map(|i| (format!("f{i:03}.txt"), vec![b'x'; 10]))
            .collect();
        let mut host = MemoryHost::new(files);
        let listing = list(&mut host, "root", 1000).unwrap();
        assert_eq!(listing.entries.len(), 300);
        assert!(listing.complete);
        assert_eq!(relative(&listing.entries[0].name), "f000.txt");
        let partial = list(&mut host, "root", 10).unwrap();
        assert_eq!(partial.entries.len(), 10);
        assert!(!partial.complete);

        let big = vec![b'y'; READ_CHUNK * 2 + 5];
        let mut host = MemoryHost::new([("big.bin", big.clone())]);
        let whole = read(&mut host, "workspace/big.bin", usize::MAX).unwrap();
        assert!(whole.complete);
        assert_eq!(whole.bytes, big);
        let mut host = MemoryHost::new([("big.bin", big)]).with_read_budget(READ_CHUNK + 1);
        let cut = read(&mut host, "workspace/big.bin", usize::MAX).unwrap();
        assert!(!cut.complete);
        assert_eq!(cut.bytes.len(), READ_CHUNK);
        assert_eq!(
            size(&mut host, "workspace/big.bin").unwrap(),
            (READ_CHUNK * 2 + 5) as u64
        );
    }

    #[test]
    fn respond_refuses_an_unknown_version() {
        fn echo(request: &Request, _: &mut dyn Host) -> Result<Value, Refusal> {
            Ok(request.input.clone())
        }
        let mut host = MemoryHost::new(Vec::<(String, Vec<u8>)>::new());
        let request = MemoryHost::request("echo", json!({"a": 1}));
        let ok: Value = serde_json::from_slice(&respond(
            &serde_json::to_vec(&request).unwrap(),
            &mut host,
            echo,
        ))
        .unwrap();
        assert_eq!(ok["status"], "ok");
        assert_eq!(ok["value"], json!({"a": 1}));
        let mut stale = request;
        stale.v = "other".into();
        let refused: Value = serde_json::from_slice(&respond(
            &serde_json::to_vec(&stale).unwrap(),
            &mut host,
            echo,
        ))
        .unwrap();
        assert_eq!(refused["status"], "unsupported_input");
        assert_eq!(refused["value"], Value::Null);
    }
}
