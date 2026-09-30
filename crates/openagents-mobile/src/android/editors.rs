//! Composer drafts and code highlighting for the Android host: the same
//! shared Rust the iOS host reaches through `rust_native_layout.h`
//! (`rust_native_editor_*` and `rust_native_syntax_spans`).
//!
//! An editor handle belongs to one `EditText`; the UI thread calls it
//! synchronously, since the shared editor is fast enough for keystrokes.
//! IDs are counters, never pointers, and the registry bounds how many
//! editors can be live.
use super::{BridgeError, error};
use rust_native::edit::mirror::Mirror;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};

/// The most composer fields live at once.
pub(crate) const MAX_EDITORS: usize = 16;
/// The largest request: a full draft (UTF-16 characters) and its framing.
pub(crate) const MAX_REQUEST_CHARS: usize = rust_native::edit::mirror::MAX_REQUEST_BYTES;

static EDITORS: LazyLock<Mutex<BTreeMap<i64, Mirror>>> = LazyLock::new(Mutex::default);
static NEXT: AtomicI64 = AtomicI64::new(1);

fn editors() -> MutexGuard<'static, BTreeMap<i64, Mirror>> {
    EDITORS.lock().unwrap_or_else(|poison| poison.into_inner())
}

pub(crate) fn create() -> Result<i64, BridgeError> {
    let mut editors = editors();
    if editors.len() >= MAX_EDITORS {
        return Err(error("Too many composer fields"));
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    editors.insert(id, Mirror::default());
    Ok(id)
}

pub(crate) fn call(id: i64, request: &str) -> Result<Vec<u8>, BridgeError> {
    let mut editors = editors();
    let editor = editors
        .get_mut(&id)
        .ok_or_else(|| error("Unknown composer field"))?;
    Ok(editor.call_json(request.as_bytes()))
}

pub(crate) fn destroy(id: i64) {
    editors().remove(&id);
}

/// Paint-only spans for one code block, as JSON `[[start16, len16, rgba]]`.
pub(crate) fn highlight(language: &str, text: &str, light: bool) -> Vec<u8> {
    let spans: Vec<_> = rust_native::syntax::highlight_utf16(language, text, light)
        .into_iter()
        .map(|span| (span.start, span.len, span.rgba))
        .collect();
    serde_json::to_vec(&spans).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editors_answer_json_and_are_bounded() {
        let id = create().unwrap();
        let reply = call(
            id,
            r#"{"op":"mount","token":"composer-1","max_bytes":8,"draft":"hi"}"#,
        )
        .unwrap();
        let reply: serde_json::Value = serde_json::from_slice(&reply).unwrap();
        assert_eq!(reply["state"]["text"], "hi");
        destroy(id);
        assert!(call(id, r#"{"op":"dispose"}"#).is_err());
        let spans: serde_json::Value =
            serde_json::from_slice(&highlight("rust", "fn main() {}\n", false)).unwrap();
        assert!(!spans.as_array().unwrap().is_empty());
    }
}
