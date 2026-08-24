//! Demo guest plugin for the OpenAgents coder plugin walking skeleton.
//!
//! The contract is `handle_packet(bytes) -> bytes`: the packet in is the
//! UTF-8 JSON encoding of the tool arguments, the packet out is UTF-8 JSON
//! that is either `{"ok": ...}` or `{"refusal": {"code": ..., "reason": ...}}`.
//! The plugin imports nothing — it is pure computation — so the host can
//! instantiate it with an empty import object and refuse any module that
//! asks for more.
//!
//! Memory crosses the boundary through two exports:
//!
//! - `packet_alloc(len) -> ptr` — the host asks the guest for a buffer and
//!   writes the input packet into guest linear memory.
//! - `handle_packet(ptr, len) -> u64` — returns the output packet's location
//!   packed as `(ptr << 32) | len`. The output buffer is leaked on purpose:
//!   the host reads it immediately and the instance is dropped after one
//!   call, so a free export would be ceremony for this demo. The real
//!   skeleton's PDK owns this convention.
//!
//! Input schema (mirrored in ../manifest.json):
//!   { "text": string, "spin"?: bool }
//! `spin: true` loops forever, existing solely so the host's timeout bound is
//! demonstrable against a real runaway guest.
//!
//! No dependencies, so the JSON handling is a deliberately small hand-rolled
//! scanner rather than serde. The real skeleton replaces this with an owned
//! Rust PDK that carries serde and the typed refusal enum.

use std::alloc::{alloc, Layout};

#[no_mangle]
pub extern "C" fn packet_alloc(len: u32) -> *mut u8 {
    let layout = Layout::from_size_align(len.max(1) as usize, 1).expect("layout");
    unsafe { alloc(layout) }
}

#[no_mangle]
pub extern "C" fn handle_packet(ptr: *const u8, len: u32) -> u64 {
    let input = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let out = respond(input).into_bytes();
    let out_len = out.len() as u32;
    let out_ptr = packet_alloc(out_len);
    unsafe { std::ptr::copy_nonoverlapping(out.as_ptr(), out_ptr, out_len as usize) };
    ((out_ptr as u64) << 32) | u64::from(out_len)
}

fn respond(input: &[u8]) -> String {
    let Ok(json) = std::str::from_utf8(input) else {
        return refusal("bad_packet", "the input packet is not UTF-8");
    };
    let Some(text) = extract_string_field(json, "text") else {
        return refusal("bad_packet", "the input packet has no string `text` field");
    };
    if has_true_field(json, "spin") {
        // A runaway guest, on request, so the host's timeout is testable.
        let mut n: u64 = 0;
        loop {
            n = std::hint::black_box(n.wrapping_add(1));
        }
    }

    let bytes = text.len();
    let chars = text.chars().count();
    let lines = if text.is_empty() { 0 } else { text.lines().count() };
    let words: Vec<&str> = text.split_whitespace().collect();
    let longest = words.iter().max_by_key(|word| word.len()).copied().unwrap_or("");

    let mut counts: Vec<(String, u32)> = Vec::new();
    for word in &words {
        let lowered: String = word
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_lowercase();
        if lowered.is_empty() {
            continue;
        }
        match counts.iter_mut().find(|(seen, _)| *seen == lowered) {
            Some((_, n)) => *n += 1,
            None => counts.push((lowered, 1)),
        }
    }
    let top = counts.iter().max_by_key(|(_, n)| *n);

    let mut out = String::from("{\"ok\":{");
    out.push_str(&format!(
        "\"bytes\":{bytes},\"chars\":{chars},\"words\":{},\"lines\":{lines},\"longest_word\":{}",
        words.len(),
        quote(longest)
    ));
    if let Some((word, count)) = top {
        out.push_str(&format!(",\"top_word\":{{\"word\":{},\"count\":{count}}}", quote(word)));
    }
    out.push_str("}}");
    out
}

fn refusal(code: &str, reason: &str) -> String {
    format!("{{\"refusal\":{{\"code\":{},\"reason\":{}}}}}", quote(code), quote(reason))
}

/// JSON-quote a string, escaping what must be escaped.
fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Find `"key": "<string>"` at the top level of a JSON object and decode it.
///
/// A scanner, not a parser: enough for the flat argument objects this demo's
/// manifest declares, and honest about being replaced by serde in the PDK.
fn extract_string_field(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let mut search_from = 0;
    loop {
        let at = json[search_from..].find(&needle)? + search_from;
        let mut rest = json[at + needle.len()..].chars().peekable();
        // Skip whitespace, require a colon, skip whitespace, require a quote.
        while rest.peek().is_some_and(|c| c.is_whitespace()) {
            rest.next();
        }
        if rest.next() != Some(':') {
            search_from = at + needle.len();
            continue;
        }
        while rest.peek().is_some_and(|c| c.is_whitespace()) {
            rest.next();
        }
        if rest.next() != Some('"') {
            search_from = at + needle.len();
            continue;
        }
        // Decode the JSON string.
        let mut value = String::new();
        loop {
            match rest.next()? {
                '"' => return Some(value),
                '\\' => match rest.next()? {
                    '"' => value.push('"'),
                    '\\' => value.push('\\'),
                    '/' => value.push('/'),
                    'n' => value.push('\n'),
                    'r' => value.push('\r'),
                    't' => value.push('\t'),
                    'b' => value.push('\u{8}'),
                    'f' => value.push('\u{c}'),
                    'u' => {
                        let hex: String = (0..4).filter_map(|_| rest.next()).collect();
                        let code = u32::from_str_radix(&hex, 16).ok()?;
                        value.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                    }
                    _ => return None,
                },
                c => value.push(c),
            }
        }
    }
}

/// True when `"key": true` appears in the JSON text.
fn has_true_field(json: &str, key: &str) -> bool {
    let needle = format!("\"{key}\"");
    let Some(at) = json.find(&needle) else {
        return false;
    };
    json[at + needle.len()..]
        .trim_start()
        .strip_prefix(':')
        .map(|rest| rest.trim_start().starts_with("true"))
        .unwrap_or(false)
}
