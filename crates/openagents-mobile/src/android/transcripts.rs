//! Transcript layout for the Android host: the same Rust layout the iOS host
//! reaches through `rust_native_layout.h`, measured by the Rust shaper with
//! the phone's own fonts, so Android needs no text-engine callback.
//!
//! The faces are the web's (#11120): text in the first family of the
//! `--font-sans` stack the phone has, code in the first of `--font-mono`
//! (`oa_tokens::typography`), which on Android are its system UI face,
//! Roboto, and its monospace face, Droid Sans Mono. The app bundles no font;
//! only a phone with neither file keeps the shaper's bundled face.
//!
//! A transcript handle is updated on one worker thread at a time; each update
//! registers the layout's immutable frame under its own ID, which the UI
//! thread reads until it releases it. IDs are counters, never pointers. The
//! registry bounds how many transcripts and frames can be live.
use super::{BridgeError, error};
use rust_native::layout::shape::{FontSpec, ShapingMeasurer, faces};
use rust_native::layout::{Frame, TranscriptLayout, Update};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

/// The most transcripts live at once.
pub(crate) const MAX_TRANSCRIPTS: usize = 8;
/// The most frames live at once; a transcript view holds one or two.
pub(crate) const MAX_FRAMES: usize = 64;
/// The largest update. Pulled transcripts send only a source's name.
pub(crate) const MAX_UPDATE_BYTES: usize = 4 * 1024 * 1024;
/// The most placements one query returns.
pub(crate) const MAX_PLACEMENTS: usize = 20_000;

struct Transcript {
    layout: TranscriptLayout,
    measurer: ShapingMeasurer,
}

#[derive(Default)]
struct Registry {
    transcripts: BTreeMap<i64, Arc<Mutex<Transcript>>>,
    frames: BTreeMap<i64, Arc<Frame>>,
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(Mutex::default);
static NEXT: AtomicI64 = AtomicI64::new(1);

fn registry() -> MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn next_id() -> i64 {
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Android's font files for a family the token stacks name, in the order
/// to try. A family with no file here (another platform's face, an emoji
/// face) is skipped.
pub(crate) fn system_files(family: &str) -> &'static [&'static str] {
    match family {
        // The system UI face, which Android's `sans-serif` also names.
        "ui-sans-serif" | "system-ui" | "sans-serif" => &[
            "/system/fonts/Roboto-Regular.ttf",
            "/system/fonts/RobotoStatic-Regular.ttf",
        ],
        "Noto Sans" => &["/system/fonts/NotoSans-Regular.ttf"],
        "ui-monospace" | "monospace" => &["/system/fonts/DroidSansMono.ttf"],
        _ => &[],
    }
}

/// The first file of `stack`'s families that exists on this phone.
fn resolve(stack: &[String]) -> Option<&'static str> {
    stack.iter().find_map(|family| {
        system_files(family)
            .iter()
            .copied()
            .find(|path| std::path::Path::new(path).is_file())
    })
}

/// Installs the phone's faces for every transcript, once, before the first
/// measurer. Without a text face the shaper keeps its bundled one; without
/// a code face, code draws in the text face.
#[cfg(target_os = "android")]
fn install_system_faces() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let read = |path: Option<&str>| -> Option<&'static [u8]> {
            let bytes = std::fs::read(path?).ok()?;
            Some(Box::leak(bytes.into_boxed_slice()))
        };
        if let Some(text) = read(resolve(&oa_tokens::typography::sans())) {
            let code = read(resolve(&oa_tokens::typography::mono())).unwrap_or(text);
            let _ = rust_native::layout::shape::install_faces(text, code);
        }
    });
}

pub(crate) fn create() -> Result<i64, BridgeError> {
    #[cfg(target_os = "android")]
    install_system_faces();
    let mut registry = registry();
    if registry.transcripts.len() >= MAX_TRANSCRIPTS {
        return Err(error("Too many transcript layouts"));
    }
    let id = next_id();
    registry.transcripts.insert(
        id,
        Arc::new(Mutex::new(Transcript {
            layout: TranscriptLayout::new(),
            measurer: ShapingMeasurer::new(),
        })),
    );
    Ok(id)
}

pub(crate) fn destroy(id: i64) {
    registry().transcripts.remove(&id);
}

/// Applies a JSON update (`rust_native::layout::Update`) and returns the
/// summary with the new frame's ID as `frame`, or `{"error": ...}` when Rust
/// refused the update.
pub(crate) fn update(id: i64, request: &str) -> Result<Vec<u8>, BridgeError> {
    if request.is_empty() || request.len() > MAX_UPDATE_BYTES {
        return Err(error("Native input exceeds its size limit"));
    }
    let transcript = registry()
        .transcripts
        .get(&id)
        .cloned()
        .ok_or_else(|| error("Transcript handle is stale"))?;
    let reply = |value: serde_json::Value| serde_json::to_vec(&value).unwrap_or_default();
    let update: Update = match serde_json::from_str(request) {
        Ok(update) => update,
        Err(problem) => return Ok(reply(serde_json::json!({ "error": problem.to_string() }))),
    };
    let mut transcript = transcript
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Transcript { layout, measurer } = &mut *transcript;
    let summary = match layout.update(update, measurer) {
        Ok(summary) => summary,
        Err(problem) => return Ok(reply(serde_json::json!({ "error": problem.to_string() }))),
    };
    let frame = layout.frame();
    drop(transcript);
    let mut registry = registry();
    if registry.frames.len() >= MAX_FRAMES {
        return Err(error("Too many transcript frames are held"));
    }
    let frame_id = next_id();
    registry.frames.insert(frame_id, frame);
    let mut value = serde_json::to_value(&summary).unwrap_or_default();
    value["frame"] = frame_id.into();
    Ok(reply(value))
}

fn frame(id: i64) -> Result<Arc<Frame>, BridgeError> {
    registry()
        .frames
        .get(&id)
        .cloned()
        .ok_or_else(|| error("Transcript frame is released"))
}

pub(crate) fn release(id: i64) {
    registry().frames.remove(&id);
}

pub(crate) fn height(id: i64) -> Result<f32, BridgeError> {
    Ok(frame(id)?.height())
}

/// Four values per row: index, version, and the float bits of its top and
/// height, in points.
fn placements(frame: &Frame, range: std::ops::Range<usize>) -> Vec<i64> {
    let mut out = Vec::with_capacity(range.len().min(MAX_PLACEMENTS) * 4);
    for index in range.take(MAX_PLACEMENTS) {
        if let Some(p) = frame.placement(index) {
            out.extend([
                p.index as i64,
                p.version as i64,
                i64::from(p.y.to_bits()),
                i64::from(p.height.to_bits()),
            ]);
        }
    }
    out
}

/// The rows intersecting `y0..y1`, as [`placements`].
pub(crate) fn rows(id: i64, y0: f32, y1: f32) -> Result<Vec<i64>, BridgeError> {
    if !y0.is_finite() || !y1.is_finite() {
        return Err(error("Invalid transcript range"));
    }
    let frame = frame(id)?;
    Ok(placements(&frame, frame.rows_in(y0, y1)))
}

/// Every row, as [`placements`].
pub(crate) fn all(id: i64) -> Result<Vec<i64>, BridgeError> {
    let frame = frame(id)?;
    Ok(placements(&frame, 0..frame.len()))
}

/// Every row's key, oldest first, joined by line feeds. Keys never contain
/// one.
pub(crate) fn keys(id: i64) -> Result<String, BridgeError> {
    let frame = frame(id)?;
    let keys: Vec<&str> = (0..frame.len()).filter_map(|i| frame.key(i)).collect();
    Ok(keys.join("\n"))
}

/// A row's display list as JSON.
pub(crate) fn display(id: i64, index: i32) -> Result<Vec<u8>, BridgeError> {
    let frame = frame(id)?;
    let display = usize::try_from(index)
        .ok()
        .and_then(|index| frame.display(index))
        .ok_or_else(|| error("No such transcript row"))?;
    serde_json::to_vec(display).map_err(|_| error("The row could not be encoded"))
}

/// The face and variations for a display-list font: face, `wght`, `opsz`,
/// and whether `calt` stays on (1 or 0).
pub(crate) fn font_spec(size: f32, weight: i32, italic: bool, mono: bool) -> [f32; 4] {
    use rust_native::layout::display::{Font, Weight};
    let spec = FontSpec::of(Font {
        family: Default::default(),
        size,
        weight: match weight {
            1 => Weight::Medium,
            2 => Weight::Semibold,
            3 => Weight::Bold,
            _ => Weight::Regular,
        },
        italic,
        mono,
    });
    [
        spec.face as f32,
        spec.weight,
        spec.optical,
        f32::from(u8::from(spec.calt)),
    ]
}

/// Publishes a transcript node's rows (JSON, a transcript without a source)
/// as the source `name`, for the host's fixture. The app publishes its own
/// chats in Rust.
pub(crate) fn publish(name: &str, node: &str) -> Result<(), BridgeError> {
    if node.len() > MAX_UPDATE_BYTES {
        return Err(error("Native input exceeds its size limit"));
    }
    // Intents stay with the view; layout never reads them.
    let node: rust_native::Node<serde_json::Value> =
        serde_json::from_str(node).map_err(|_| error("Invalid transcript node"))?;
    let node = rust_native::layout::without_intents(&node);
    let rust_native::Element::Transcript {
        children,
        earlier,
        source: None,
        ..
    } = node.element
    else {
        return Err(error("Invalid transcript node"));
    };
    let earlier = earlier.as_ref().map(rust_native::layout::EarlierRow::from);
    rust_native::layout::source::publish(name, children, earlier)
        .map_err(|problem| BridgeError(problem.to_string()))
}

/// A face's font file: the phone's text or code face, or the shaper's
/// bundled one when the phone had none.
pub(crate) fn font(face: i32) -> Result<&'static [u8], BridgeError> {
    usize::try_from(face)
        .ok()
        .and_then(|face| faces().get(face).copied())
        .ok_or_else(|| error("No such font face"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transcript_lays_out_a_published_source_and_hands_out_frames() {
        let messages: Vec<serde_json::Value> = (0..5)
            .map(|i| {
                serde_json::json!({"key": format!("m{i}"), "style": {}, "element": {"kind": "message",
                    "props": {"role": "assistant", "note": null, "children": [
                        {"key": format!("m{i}-md"), "style": {}, "element": {"kind": "markdown",
                            "props": {"blocks": [{"kind": "paragraph", "spans": [{"text": "Hello there, a reply."}]}]}}}]}}})
            })
            .collect();
        let node: rust_native::Node<()> = serde_json::from_value(serde_json::json!({
            "key": "chat", "style": {},
            "element": {"kind": "transcript", "props": {"label": "Messages", "children": messages, "earlier": null}},
        }))
        .unwrap();
        let rust_native::Element::Transcript { children, .. } = node.element else {
            unreachable!()
        };
        rust_native::layout::source::publish("android-test:chat", children, None).unwrap();
        let id = create().unwrap();
        let reply: serde_json::Value = serde_json::from_slice(
            &update(
                id,
                r#"{"width": 390, "scale": 1.0, "source": "android-test:chat"}"#,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(reply["count"], 5);
        let frame = reply["frame"].as_i64().unwrap();
        assert_eq!(keys(frame).unwrap(), "m0\nm1\nm2\nm3\nm4");
        let all = all(frame).unwrap();
        assert_eq!(all.len(), 20);
        let top = f32::from_bits(all[4 * 4 + 2] as u32);
        let tall = f32::from_bits(all[4 * 4 + 3] as u32);
        assert!((top + tall + 16.0 - height(frame).unwrap()).abs() < 0.01);
        assert_eq!(rows(frame, top, top + 1.0).unwrap()[0], 4);
        let row: serde_json::Value = serde_json::from_slice(&display(frame, 2).unwrap()).unwrap();
        assert_eq!(row["key"], "m2");
        assert!(display(frame, 9).is_err());
        let refused: serde_json::Value =
            serde_json::from_slice(&update(id, r#"{"width": 0, "scale": 1}"#).unwrap()).unwrap();
        assert!(refused["error"].is_string());
        release(frame);
        assert!(keys(frame).is_err());
        destroy(id);
        assert!(update(id, "{}").is_err());
        assert_eq!(font_spec(14.4, 0, false, true), [0.0, 400.0, 0.0, 0.0]);
        // A host test process installs no faces, so every weight, italic,
        // and monospace combination draws the shaper's one bundled face.
        for weight in 0..4 {
            for italic in [false, true] {
                for mono in [false, true] {
                    assert_eq!(font_spec(15.0, weight, italic, mono)[0], 0.0);
                }
            }
        }
        assert!(font(0).unwrap().len() > 100_000);
        assert_eq!(faces().len(), 1);
        assert!(font(1).is_err() && font(-1).is_err());
        rust_native::layout::source::retire("android-test:chat");
    }

    /// Every family the phone resolves is one the web's token stacks name,
    /// and the stacks pick Roboto for text and Droid Sans Mono for code.
    #[test]
    fn the_phone_faces_are_the_token_stacks() {
        let sans = oa_tokens::typography::sans();
        let mono = oa_tokens::typography::mono();
        for family in [
            "ui-sans-serif",
            "system-ui",
            "sans-serif",
            "Noto Sans",
            "ui-monospace",
            "monospace",
        ] {
            assert!(!system_files(family).is_empty(), "{family}");
            assert!(
                sans.iter().chain(mono.iter()).any(|named| named == family),
                "{family} is not in the token stacks"
            );
        }
        let first = |stack: &[String]| {
            stack
                .iter()
                .find_map(|family| system_files(family).first().copied())
        };
        assert_eq!(first(&sans), Some("/system/fonts/Roboto-Regular.ttf"));
        assert_eq!(first(&mono), Some("/system/fonts/DroidSansMono.ttf"));
        assert!(system_files("Paper Mono").is_empty());
        assert_eq!(resolve(&["Paper Mono".to_string()]), None);
    }
}
