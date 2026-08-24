//! The owned Rust PDK for OpenAgents WASM plugins.
//!
//! A plugin author writes one function over serde types:
//!
//! ```ignore
//! use openagents_pdk::{plugin_entry, Refusal};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Deserialize)]
//! struct Input { text: String }
//!
//! #[derive(Serialize)]
//! struct Output { chars: usize }
//!
//! fn handle(input: Input) -> Result<Output, Refusal> {
//!     Ok(Output { chars: input.text.chars().count() })
//! }
//!
//! plugin_entry!(handle);
//! ```
//!
//! and the [`plugin_entry!`] macro generates the whole `packet-v0` ABI:
//! the `packet_alloc` export the host allocates through, the
//! `handle_packet(ptr, len) -> u64` export, the serde decode of the input
//! packet, the `{"ok": ...}` / `{"refusal": ...}` envelope on the way out,
//! and the `(ptr << 32) | len` packing of the return word. Authors never
//! see a pointer.
//!
//! ## The packet-v0 contract, as this crate owns it
//!
//! - The input packet is the UTF-8 JSON encoding of the tool arguments.
//!   A packet that does not decode into the handler's input type is
//!   answered with a `bad_packet` refusal, not a trap.
//! - The output packet is UTF-8 JSON: `{"ok": <output>}` on success,
//!   `{"refusal": {"code": ..., "reason": ...}}` otherwise. Refusals are
//!   values on both sides of the boundary; the PDK never panics on bad
//!   input and the handler returns `Result`, never throws.
//! - The output buffer is deliberately leaked. The host reads it
//!   immediately and drops the instance after one call — one instance per
//!   invocation is the host's isolation model — so a free export would be
//!   ceremony.
//!
//! ## Host capabilities
//!
//! [`read_mounted_file`] is the first host import: available only when the
//! plugin's manifest declares read-only mounts, and answered by the host
//! with either the file bytes or a typed refusal (`mount_denied`,
//! `file_unreadable`, `file_too_large`). A plugin that never calls it
//! links no imports at all — the compiler strips the unused extern — so a
//! pure-compute plugin still passes the host's empty-import inspection.

use serde::de::DeserializeOwned;
use serde::Serialize;

// Re-exported so plugin crates need only `openagents-pdk` in [dependencies].
pub use serde;
pub use serde_json;

/// Why the plugin would not do what was asked. Returned, never thrown.
///
/// The code set mirrors the host's guest-visible refusal codes, so a
/// refusal born on either side of the boundary reads the same in the
/// output packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub code: RefusalCode,
    pub reason: String,
}

/// The closed set of guest-side refusal codes for `packet-v0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalCode {
    /// The input packet does not decode into the handler's input type,
    /// or an output failed to encode.
    BadPacket,
    /// The plugin was asked for something it does not do.
    Unsupported,
    /// The host refused a mounted-file read: the path is outside every
    /// declared mount, absolute, or reaches through a symlink.
    MountDenied,
    /// The host could not read the mounted file (missing, a directory,
    /// or an I/O failure).
    FileUnreadable,
    /// The mounted file exceeds the host's per-file size bound.
    FileTooLarge,
    /// The plugin's own invariant broke. A bug, stated as a value.
    Internal,
}

impl RefusalCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            RefusalCode::BadPacket => "bad_packet",
            RefusalCode::Unsupported => "unsupported",
            RefusalCode::MountDenied => "mount_denied",
            RefusalCode::FileUnreadable => "file_unreadable",
            RefusalCode::FileTooLarge => "file_too_large",
            RefusalCode::Internal => "internal",
        }
    }

    /// The code for a host-authored refusal packet. Unknown codes fold to
    /// [`RefusalCode::Internal`]; the caller keeps the raw text in the reason.
    fn parse(code: &str) -> Option<Self> {
        match code {
            "bad_packet" => Some(RefusalCode::BadPacket),
            "unsupported" => Some(RefusalCode::Unsupported),
            "mount_denied" => Some(RefusalCode::MountDenied),
            "file_unreadable" => Some(RefusalCode::FileUnreadable),
            "file_too_large" => Some(RefusalCode::FileTooLarge),
            "internal" => Some(RefusalCode::Internal),
            _ => None,
        }
    }
}

impl Refusal {
    pub fn new(code: RefusalCode, reason: impl Into<String>) -> Self {
        Refusal { code, reason: reason.into() }
    }

    pub fn bad_packet(reason: impl Into<String>) -> Self {
        Refusal::new(RefusalCode::BadPacket, reason)
    }

    pub fn unsupported(reason: impl Into<String>) -> Self {
        Refusal::new(RefusalCode::Unsupported, reason)
    }

    pub fn internal(reason: impl Into<String>) -> Self {
        Refusal::new(RefusalCode::Internal, reason)
    }
}

/// Encode `{"refusal": {"code": ..., "reason": ...}}` as an output packet.
pub fn refusal_packet(refusal: &Refusal) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "refusal": { "code": refusal.code.as_str(), "reason": refusal.reason }
    }))
    // The value is two strings; encoding cannot fail.
    .expect("a refusal always encodes")
}

/// Decode the input packet, run the handler, encode the output envelope.
///
/// This is the whole guest side of `packet-v0` minus the pointer plumbing,
/// and it is total: every path returns a packet.
pub fn run_handler<I, O, F>(input: &[u8], handler: F) -> Vec<u8>
where
    I: DeserializeOwned,
    O: Serialize,
    F: FnOnce(I) -> Result<O, Refusal>,
{
    let parsed: I = match serde_json::from_slice(input) {
        Ok(value) => value,
        Err(err) => {
            return refusal_packet(&Refusal::bad_packet(format!(
                "the input packet does not decode: {err}"
            )))
        }
    };
    match handler(parsed) {
        Ok(output) => match serde_json::to_vec(&output) {
            Ok(body) => {
                let mut packet = Vec::with_capacity(body.len() + 8);
                packet.extend_from_slice(b"{\"ok\":");
                packet.extend_from_slice(&body);
                packet.push(b'}');
                packet
            }
            Err(err) => refusal_packet(&Refusal::internal(format!(
                "the output does not encode: {err}"
            ))),
        },
        Err(refusal) => refusal_packet(&refusal),
    }
}

/// Read a file from one of the manifest's declared read-only mounts.
///
/// The path is relative to a mount root; the host confines it (no absolute
/// paths, no `..` escapes, no symlinks, a per-file size bound) and answers
/// with the bytes or a typed refusal. On a target other than
/// `wasm32-unknown-unknown` — the PDK's own unit tests, for example — the
/// import does not exist and this returns `unsupported`.
pub fn read_mounted_file(path: &str) -> Result<Vec<u8>, Refusal> {
    imp::read_mounted_file(path)
}

/// Parse a host `read_file` answer packet: one status byte, then either
/// the file bytes (0) or a `{"code", "reason"}` refusal (1).
fn parse_host_packet(packet: &[u8]) -> Result<Vec<u8>, Refusal> {
    #[derive(serde::Deserialize)]
    struct RawRefusal {
        code: String,
        reason: String,
    }
    match packet.split_first() {
        Some((0, bytes)) => Ok(bytes.to_vec()),
        Some((1, body)) => match serde_json::from_slice::<RawRefusal>(body) {
            Ok(raw) => match RefusalCode::parse(&raw.code) {
                Some(code) => Err(Refusal::new(code, raw.reason)),
                None => Err(Refusal::internal(format!("host refusal `{}`: {}", raw.code, raw.reason))),
            },
            Err(_) => Err(Refusal::internal("the host's refusal packet does not decode")),
        },
        _ => Err(Refusal::internal("the host answered with an empty packet")),
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::{parse_host_packet, Refusal};

    #[link(wasm_import_module = "openagents")]
    extern "C" {
        /// Host capability import: `(path_ptr, path_len) -> (ptr << 32) | len`
        /// of an answer packet the host wrote into guest memory through
        /// `packet_alloc`. Present only when the manifest declares mounts.
        fn read_file(path_ptr: *const u8, path_len: u32) -> u64;
    }

    pub fn read_mounted_file(path: &str) -> Result<Vec<u8>, Refusal> {
        let packed = unsafe { read_file(path.as_ptr(), path.len() as u32) };
        let ptr = (packed >> 32) as u32 as usize as *const u8;
        let len = (packed & 0xffff_ffff) as usize;
        if ptr.is_null() {
            return Err(Refusal::internal("the host answered with a null packet"));
        }
        let packet = unsafe { core::slice::from_raw_parts(ptr, len) };
        parse_host_packet(packet)
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::Refusal;

    pub fn read_mounted_file(_path: &str) -> Result<Vec<u8>, Refusal> {
        Err(Refusal::unsupported(
            "read_mounted_file is a host capability import; it exists only inside the WASM host",
        ))
    }
}

/// The pointer plumbing behind [`plugin_entry!`]. Hidden, not private, so
/// the macro can reach it from the plugin crate.
#[doc(hidden)]
pub mod __abi {
    use super::{run_handler, Refusal};
    use serde::de::DeserializeOwned;
    use serde::Serialize;

    pub fn packet_alloc(len: u32) -> *mut u8 {
        let layout = core::alloc::Layout::from_size_align(len.max(1) as usize, 1)
            .expect("a byte-aligned layout is always valid");
        unsafe { std::alloc::alloc(layout) }
    }

    /// Leak an output packet and pack its location into the return word.
    pub fn pack_output(output: Vec<u8>) -> u64 {
        let len = output.len() as u64;
        let ptr = Box::leak(output.into_boxed_slice()).as_mut_ptr() as u64;
        (ptr << 32) | len
    }

    /// # Safety
    /// `ptr..ptr+len` must be the packet the host wrote through `packet_alloc`.
    pub unsafe fn handle_packet<I, O, F>(ptr: *const u8, len: u32, handler: F) -> u64
    where
        I: DeserializeOwned,
        O: Serialize,
        F: FnOnce(I) -> Result<O, Refusal>,
    {
        let input = core::slice::from_raw_parts(ptr, len as usize);
        pack_output(run_handler(input, handler))
    }
}

/// Generate the `packet-v0` exports around one typed handler function
/// `fn(I) -> Result<O, Refusal>` where `I: Deserialize` and `O: Serialize`.
#[macro_export]
macro_rules! plugin_entry {
    ($handler:path) => {
        #[no_mangle]
        pub extern "C" fn packet_alloc(len: u32) -> *mut u8 {
            $crate::__abi::packet_alloc(len)
        }

        #[no_mangle]
        pub extern "C" fn handle_packet(ptr: *const u8, len: u32) -> u64 {
            unsafe { $crate::__abi::handle_packet(ptr, len, $handler) }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize)]
    struct In {
        n: u32,
    }

    #[derive(Serialize)]
    struct Out {
        doubled: u32,
    }

    fn double(input: In) -> Result<Out, Refusal> {
        if input.n > 1000 {
            return Err(Refusal::unsupported("n is too large"));
        }
        Ok(Out { doubled: input.n * 2 })
    }

    #[test]
    fn a_good_packet_comes_back_wrapped_in_ok() {
        let packet = run_handler(br#"{"n": 21}"#, double);
        assert_eq!(packet, br#"{"ok":{"doubled":42}}"#);
    }

    #[test]
    fn an_undecodable_packet_is_a_bad_packet_refusal_not_a_panic() {
        let packet = run_handler(b"not json", double);
        let value: serde_json::Value = serde_json::from_slice(&packet).unwrap();
        assert_eq!(value["refusal"]["code"], "bad_packet");
    }

    #[test]
    fn a_handler_refusal_becomes_the_output_packet() {
        let packet = run_handler(br#"{"n": 2000}"#, double);
        let value: serde_json::Value = serde_json::from_slice(&packet).unwrap();
        assert_eq!(value["refusal"]["code"], "unsupported");
        assert_eq!(value["refusal"]["reason"], "n is too large");
    }

    #[test]
    fn host_ok_packets_carry_the_bytes_after_the_status_byte() {
        assert_eq!(parse_host_packet(b"\x00hello"), Ok(b"hello".to_vec()));
    }

    #[test]
    fn host_refusal_packets_decode_into_the_typed_enum() {
        let packet = b"\x01{\"code\":\"mount_denied\",\"reason\":\"outside\"}";
        let refusal = parse_host_packet(packet).unwrap_err();
        assert_eq!(refusal.code, RefusalCode::MountDenied);
        assert_eq!(refusal.reason, "outside");
    }

    #[test]
    fn unknown_host_codes_fold_to_internal_and_keep_the_raw_code() {
        let packet = b"\x01{\"code\":\"weather\",\"reason\":\"rain\"}";
        let refusal = parse_host_packet(packet).unwrap_err();
        assert_eq!(refusal.code, RefusalCode::Internal);
        assert!(refusal.reason.contains("weather"));
    }

    #[test]
    fn the_return_word_packs_pointer_high_and_length_low() {
        let word = __abi::pack_output(vec![1, 2, 3]);
        assert_eq!(word & 0xffff_ffff, 3);
        assert_ne!(word >> 32, 0);
    }

    #[test]
    fn off_wasm_the_mount_import_is_an_unsupported_refusal() {
        let refusal = read_mounted_file("anything.txt").unwrap_err();
        assert_eq!(refusal.code, RefusalCode::Unsupported);
    }
}
