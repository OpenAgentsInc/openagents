//! The C interface to transcript layout, for native adapters. The application
//! libraries that link Rust Native enable the `ffi` feature, which exports
//! these symbols; `include/rust_native_layout.h` declares them.
//!
//! A handle belongs to one transcript. Calls on a handle must not overlap, but
//! they may come from any thread, so an adapter can lay out on a worker. Each
//! update publishes a frame (`rust_native_layout_frame`): an immutable,
//! reference-counted snapshot that any thread may read while the next update
//! runs. Every call catches panics, bounds its input, and reports failure as
//! an empty buffer, zero, or a null handle. The adapter's measurer is called
//! only during `update`, on the calling thread.

use super::measure::{Line, MeasureRun, Measured, Measurer};
use super::{Frame, Placement, TranscriptLayout, Update};
use crate::layout::display::Weight;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::Arc;

/// The most bytes one update may carry.
pub const MAX_UPDATE_BYTES: usize = 32 * 1024 * 1024;

#[repr(C)]
pub struct RustNativeBuffer {
    pub data: *mut u8,
    pub len: usize,
}

/// One styled range of a paragraph for the measurer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct RustNativeTextRun {
    pub size: f32,
    /// 0 regular, 1 medium, 2 semibold, 3 bold.
    pub weight: u8,
    pub italic: u8,
    pub monospace: u8,
    pub reserved: u8,
    pub start16: u32,
    pub end16: u32,
}

/// One line the measurer broke.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct RustNativeTextLine {
    pub start16: u32,
    pub end16: u32,
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    pub leading: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct RustNativeRowPlacement {
    pub index: u32,
    pub reserved: u32,
    pub version: u64,
    pub y: f32,
    pub height: f32,
}

/// Breaks `text` (UTF-8, `text_len` bytes) styled by `runs` into lines at
/// `width` points; a `width` of zero or less breaks only at hard breaks.
/// Writes lines and, per line, the x offset of each run boundary strictly
/// inside it. Returns 0 on success, 1 when a capacity is too small (the
/// counts then hold what is needed), and any other value on failure.
pub type RustNativeMeasure = Option<
    unsafe extern "C" fn(
        context: *mut c_void,
        text: *const u8,
        text_len: usize,
        runs: *const RustNativeTextRun,
        run_count: usize,
        width: f32,
        lines: *mut RustNativeTextLine,
        line_capacity: usize,
        line_count: *mut usize,
        offsets: *mut f32,
        offset_capacity: usize,
        offset_count: *mut usize,
    ) -> i32,
>;

struct Platform {
    context: *mut c_void,
    measure: unsafe extern "C" fn(
        *mut c_void,
        *const u8,
        usize,
        *const RustNativeTextRun,
        usize,
        f32,
        *mut RustNativeTextLine,
        usize,
        *mut usize,
        *mut f32,
        usize,
        *mut usize,
    ) -> i32,
}

impl Measurer for Platform {
    fn measure(&mut self, text: &str, runs: &[MeasureRun], width: Option<f32>) -> Option<Measured> {
        let runs: Vec<RustNativeTextRun> = runs
            .iter()
            .map(|r| RustNativeTextRun {
                size: r.font.size,
                weight: match r.font.weight {
                    Weight::Regular => 0,
                    Weight::Medium => 1,
                    Weight::Semibold => 2,
                    Weight::Bold => 3,
                },
                italic: u8::from(r.font.italic),
                monospace: u8::from(r.font.mono),
                reserved: 0,
                start16: r.start16,
                end16: r.end16,
            })
            .collect();
        let units = text.encode_utf16().count();
        let mut lines = vec![RustNativeTextLine::default(); 16];
        let mut offsets = vec![0f32; 32];
        for _ in 0..2 {
            let mut line_count = 0usize;
            let mut offset_count = 0usize;
            // SAFETY: every pointer is valid for the stated length for the
            // duration of the call; the adapter writes within capacities.
            let status = unsafe {
                (self.measure)(
                    self.context,
                    text.as_ptr(),
                    text.len(),
                    runs.as_ptr(),
                    runs.len(),
                    width.unwrap_or(0.0),
                    lines.as_mut_ptr(),
                    lines.len(),
                    &mut line_count,
                    offsets.as_mut_ptr(),
                    offsets.len(),
                    &mut offset_count,
                )
            };
            match status {
                0 if line_count <= lines.len() && offset_count <= offsets.len() => {
                    return Some(Measured {
                        lines: lines[..line_count]
                            .iter()
                            .map(|l| Line {
                                start16: l.start16,
                                end16: l.end16,
                                width: l.width,
                                ascent: l.ascent,
                                descent: l.descent,
                                leading: l.leading,
                            })
                            .collect(),
                        offsets: offsets[..offset_count].to_vec(),
                    });
                }
                1 if line_count <= units + 1 && offset_count <= (units + 1) * runs.len().max(1) => {
                    lines.resize(line_count.max(1), RustNativeTextLine::default());
                    offsets.resize(offset_count.max(1), 0.0);
                }
                _ => return None,
            }
        }
        None
    }
}

pub struct RustNativeLayout {
    layout: TranscriptLayout,
    measurer: Box<dyn Measurer>,
}

fn buffer(bytes: Vec<u8>) -> RustNativeBuffer {
    let mut bytes = bytes.into_boxed_slice();
    let result = RustNativeBuffer {
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

/// Creates a layout that measures through `measure`, called with `context`.
///
/// # Safety
/// `measure` must follow the `RustNativeMeasure` contract for the handle's
/// lifetime, and `context` must stay valid until the handle is destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_layout_create(
    context: *mut c_void,
    measure: RustNativeMeasure,
) -> *mut RustNativeLayout {
    let Some(measure) = measure else {
        return ptr::null_mut();
    };
    catch_unwind(|| {
        Box::into_raw(Box::new(RustNativeLayout {
            layout: TranscriptLayout::new(),
            measurer: Box::new(Platform { context, measure }),
        }))
    })
    .unwrap_or(ptr::null_mut())
}

/// Creates a layout that shapes text in Rust with the bundled fonts
/// (`layout::shape`), so it needs no measurer. The adapter draws each run
/// with the face and variations `rust_native_font_spec` names.
#[cfg(feature = "shaping")]
#[unsafe(no_mangle)]
pub extern "C" fn rust_native_layout_create_shaped() -> *mut RustNativeLayout {
    catch_unwind(|| {
        Box::into_raw(Box::new(RustNativeLayout {
            layout: TranscriptLayout::new(),
            measurer: Box::new(super::shape::ShapingMeasurer::new()),
        }))
    })
    .unwrap_or(ptr::null_mut())
}

/// How to draw a display-list font with the bundled faces.
#[cfg(feature = "shaping")]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct RustNativeFontSpec {
    /// Which face `rust_native_font_data` returns: always 0, Paper Mono's
    /// variable font.
    pub face: u32,
    /// The `wght` axis value.
    pub weight: f32,
    /// The `opsz` axis value, or 0 when the face has no such axis.
    pub optical: f32,
    /// Whether contextual alternates (`calt`) stay on.
    pub calt: u8,
    pub reserved: [u8; 3],
}

/// The face and variations for a display-list font: its size, weight (0
/// regular, 1 medium, 2 semibold, 3 bold), italic, and monospace.
#[cfg(feature = "shaping")]
#[unsafe(no_mangle)]
pub extern "C" fn rust_native_font_spec(
    size: f32,
    weight: u8,
    italic: u8,
    monospace: u8,
) -> RustNativeFontSpec {
    let spec = super::shape::FontSpec::of(super::display::Font {
        family: Default::default(),
        size,
        weight: match weight {
            1 => Weight::Medium,
            2 => Weight::Semibold,
            3 => Weight::Bold,
            _ => Weight::Regular,
        },
        italic: italic != 0,
        mono: monospace != 0,
    });
    RustNativeFontSpec {
        face: spec.face as u32,
        weight: spec.weight,
        optical: spec.optical,
        calt: u8::from(spec.calt),
        reserved: [0; 3],
    }
}

/// A bundled face's font file, which lives as long as the process, and its
/// length in `len`; null for an unknown face.
///
/// # Safety
/// `len` must be writable.
#[cfg(feature = "shaping")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_font_data(face: u32, len: *mut usize) -> *const u8 {
    let Some(data) = super::shape::FACES.get(face as usize) else {
        return ptr::null();
    };
    if !len.is_null() {
        unsafe { len.write(data.len()) };
    }
    data.as_ptr()
}

/// Applies a JSON [`Update`] and returns a JSON [`super::Summary`], or
/// `{"error": "..."}`. An empty buffer means the input was unreadable.
///
/// # Safety
/// `handle` must be live and used from one thread; `bytes` must point to
/// `len` readable bytes. Free the result with `rust_native_layout_buffer_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_layout_update(
    handle: *mut RustNativeLayout,
    bytes: *const u8,
    len: usize,
) -> RustNativeBuffer {
    if handle.is_null() || bytes.is_null() || len == 0 || len > MAX_UPDATE_BYTES {
        return buffer(vec![]);
    }
    catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &mut *handle };
        let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
        let update: Update = match serde_json::from_slice(bytes) {
            Ok(update) => update,
            Err(error) => return error_json(&error.to_string()),
        };
        match handle.layout.update(update, handle.measurer.as_mut()) {
            Ok(summary) => serde_json::to_vec(&summary).unwrap_or_default(),
            Err(error) => error_json(&error.to_string()),
        }
    }))
    .map(buffer)
    .unwrap_or_else(|_| buffer(vec![]))
}

fn error_json(message: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({ "error": message })).unwrap_or_default()
}

/// Writes the placements of the rows intersecting `y0..y1`, at most
/// `capacity`, and returns how many rows intersect.
///
/// # Safety
/// `handle` must be live; `out` must have room for `capacity` placements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_layout_rows(
    handle: *const RustNativeLayout,
    y0: f32,
    y1: f32,
    out: *mut RustNativeRowPlacement,
    capacity: usize,
) -> usize {
    if handle.is_null() || !y0.is_finite() || !y1.is_finite() {
        return 0;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let layout = unsafe { &(*handle).layout };
        let range = layout.rows_in(y0, y1);
        unsafe { write_placements(range.clone(), |i| layout.placement(i), out, capacity) };
        range.len()
    }))
    .unwrap_or(0)
}

/// # Safety
/// `out` must be null or have room for `capacity` placements.
unsafe fn write_placements(
    range: std::ops::Range<usize>,
    placement: impl Fn(usize) -> Option<Placement>,
    out: *mut RustNativeRowPlacement,
    capacity: usize,
) {
    if out.is_null() {
        return;
    }
    for (slot, index) in range.take(capacity).enumerate() {
        if let Some(placement) = placement(index) {
            unsafe { out.add(slot).write(placed(placement)) };
        }
    }
}

fn placed(p: Placement) -> RustNativeRowPlacement {
    RustNativeRowPlacement {
        index: p.index as u32,
        reserved: 0,
        version: p.version,
        y: p.y,
        height: p.height,
    }
}

/// The content height, including the edge insets.
///
/// # Safety
/// `handle` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_layout_height(handle: *const RustNativeLayout) -> f32 {
    if handle.is_null() {
        return 0.0;
    }
    unsafe { (*handle).layout.height() }
}

/// Finds a row by key (UTF-8) and writes its placement. Returns 1 when found.
///
/// # Safety
/// `handle` must be live; `key` must point to `len` bytes; `out` must be
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_layout_find(
    handle: *const RustNativeLayout,
    key: *const u8,
    len: usize,
    out: *mut RustNativeRowPlacement,
) -> i32 {
    if handle.is_null() || key.is_null() || out.is_null() || len > 256 {
        return 0;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let layout = unsafe { &(*handle).layout };
        let key = unsafe { std::slice::from_raw_parts(key, len) };
        let Ok(key) = std::str::from_utf8(key) else {
            return 0;
        };
        match layout.find(key).and_then(|i| layout.placement(i)) {
            Some(placement) => {
                unsafe { out.write(placed(placement)) };
                1
            }
            None => 0,
        }
    }))
    .unwrap_or(0)
}

/// Returns row `index`'s display list as JSON (see `layout::display`), or
/// an empty buffer when there is no such row.
///
/// # Safety
/// `handle` must be live and used from one thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_layout_display(
    handle: *mut RustNativeLayout,
    index: u32,
) -> RustNativeBuffer {
    if handle.is_null() {
        return buffer(vec![]);
    }
    catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &*handle };
        handle
            .layout
            .display(index as usize)
            .and_then(|display| serde_json::to_vec(display).ok())
            .unwrap_or_default()
    }))
    .map(buffer)
    .unwrap_or_else(|_| buffer(vec![]))
}

/// Returns the frame for the handle's current layout, which the caller owns
/// and releases once with `rust_native_frame_release`, or null on failure.
///
/// # Safety
/// `handle` must be live, with no other call on it in progress.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_layout_frame(handle: *mut RustNativeLayout) -> *const Frame {
    if handle.is_null() {
        return ptr::null();
    }
    catch_unwind(AssertUnwindSafe(|| {
        Arc::into_raw(unsafe { &mut *handle }.layout.frame())
    }))
    .unwrap_or(ptr::null())
}

/// Borrows a frame for the length of a call.
///
/// # Safety
/// `frame` must be null or a live result of `rust_native_layout_frame`.
unsafe fn frame<'a>(frame: *const Frame) -> Option<&'a Frame> {
    unsafe { frame.as_ref() }
}

/// The number of rows in a frame, including the earlier control.
///
/// # Safety
/// `frame` must be null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_frame_count(value: *const Frame) -> usize {
    unsafe { frame(value) }.map_or(0, Frame::len)
}

/// A frame's content height, including the edge insets.
///
/// # Safety
/// `frame` must be null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_frame_height(value: *const Frame) -> f32 {
    unsafe { frame(value) }.map_or(0.0, Frame::height)
}

/// Writes at most `capacity` placements of the frame's rows intersecting
/// `y0..y1`, and returns how many rows intersect.
///
/// # Safety
/// `frame` must be null or live; `out` must be null or have room for
/// `capacity` placements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_frame_rows(
    value: *const Frame,
    y0: f32,
    y1: f32,
    out: *mut RustNativeRowPlacement,
    capacity: usize,
) -> usize {
    let Some(frame) = (unsafe { frame(value) }) else {
        return 0;
    };
    if !y0.is_finite() || !y1.is_finite() {
        return 0;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let range = frame.rows_in(y0, y1);
        unsafe { write_placements(range.clone(), |i| frame.placement(i), out, capacity) };
        range.len()
    }))
    .unwrap_or(0)
}

/// Finds a row of the frame by key (UTF-8) and writes its placement.
/// Returns 1 when found.
///
/// # Safety
/// `frame` must be null or live; `key` must point to `len` bytes; `out` must
/// be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_frame_find(
    value: *const Frame,
    key: *const u8,
    len: usize,
    out: *mut RustNativeRowPlacement,
) -> i32 {
    let Some(frame) = (unsafe { frame(value) }) else {
        return 0;
    };
    if key.is_null() || out.is_null() || len > 256 {
        return 0;
    }
    let key = unsafe { std::slice::from_raw_parts(key, len) };
    let Ok(key) = std::str::from_utf8(key) else {
        return 0;
    };
    match frame.find(key).and_then(|i| frame.placement(i)) {
        Some(placement) => {
            unsafe { out.write(placed(placement)) };
            1
        }
        None => 0,
    }
}

/// A frame row's key as UTF-8, or an empty buffer.
///
/// # Safety
/// `frame` must be null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_frame_key(
    value: *const Frame,
    index: u32,
) -> RustNativeBuffer {
    let key = unsafe { frame(value) }.and_then(|f| f.key(index as usize));
    buffer(key.map(|k| k.as_bytes().to_vec()).unwrap_or_default())
}

/// A frame row's display list as JSON, or an empty buffer.
///
/// # Safety
/// `frame` must be null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_frame_display(
    value: *const Frame,
    index: u32,
) -> RustNativeBuffer {
    let Some(frame) = (unsafe { frame(value) }) else {
        return buffer(vec![]);
    };
    catch_unwind(AssertUnwindSafe(|| {
        frame
            .display(index as usize)
            .and_then(|display| serde_json::to_vec(display).ok())
            .unwrap_or_default()
    }))
    .map(buffer)
    .unwrap_or_else(|_| buffer(vec![]))
}

/// Releases a frame from `rust_native_layout_frame`.
///
/// # Safety
/// `frame` must be null or an unreleased result of `rust_native_layout_frame`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_frame_release(value: *const Frame) {
    if !value.is_null() {
        drop(unsafe { Arc::from_raw(value) });
    }
}

/// Publishes a transcript node's rows and earlier control, as JSON
/// (`Node<()>` whose element is a transcript), to the source `name`, for
/// adapters whose rows arrive as JSON rather than from the application's own
/// Rust state. Returns 1 on success and 0 on failure.
///
/// # Safety
/// `name` must point to `name_len` bytes and `bytes` to `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_source_publish(
    name: *const u8,
    name_len: usize,
    bytes: *const u8,
    len: usize,
) -> i32 {
    if name.is_null() || bytes.is_null() || name_len > 96 || len == 0 || len > MAX_UPDATE_BYTES {
        return 0;
    }
    catch_unwind(|| {
        let name = unsafe { std::slice::from_raw_parts(name, name_len) };
        let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
        let (Ok(name), Ok(node)) = (
            std::str::from_utf8(name),
            serde_json::from_slice::<crate::view::Node<()>>(bytes),
        ) else {
            return 0;
        };
        let crate::view::Element::Transcript {
            children,
            earlier,
            source: None,
            ..
        } = node.element
        else {
            return 0;
        };
        let earlier = earlier.as_ref().map(super::EarlierRow::from);
        i32::from(super::source::publish(name, children, earlier).is_ok())
    })
    .unwrap_or(0)
}

/// Removes the source `name`.
///
/// # Safety
/// `name` must point to `name_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_source_retire(name: *const u8, name_len: usize) {
    if name.is_null() || name_len > 96 {
        return;
    }
    let name = unsafe { std::slice::from_raw_parts(name, name_len) };
    if let Ok(name) = std::str::from_utf8(name) {
        super::source::retire(name);
    }
}

/// A native text field's shared draft (`crate::edit::mirror::Mirror`), for
/// one composer field. Free it once with `rust_native_editor_destroy`.
#[unsafe(no_mangle)]
pub extern "C" fn rust_native_editor_create() -> *mut crate::edit::mirror::Mirror {
    catch_unwind(|| Box::into_raw(Box::default())).unwrap_or(ptr::null_mut())
}

/// Answers one JSON editing request (`Mirror::call_json`). Calls on one
/// handle must not overlap; the editor is fast enough for the UI thread.
///
/// # Safety
/// `handle` must be a live handle from `rust_native_editor_create`, and
/// `bytes` must point to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_editor_call(
    handle: *mut crate::edit::mirror::Mirror,
    bytes: *const u8,
    len: usize,
) -> RustNativeBuffer {
    if handle.is_null() || bytes.is_null() || len == 0 {
        return buffer(vec![]);
    }
    catch_unwind(AssertUnwindSafe(|| {
        let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
        unsafe { &mut *handle }.call_json(bytes)
    }))
    .map(buffer)
    .unwrap_or_else(|_| buffer(vec![]))
}

/// # Safety
/// The handle must come from `rust_native_editor_create`, have no call in
/// progress, and not be destroyed already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_editor_destroy(handle: *mut crate::edit::mirror::Mirror) {
    if !handle.is_null() {
        unsafe { drop(Box::from_raw(handle)) }
    }
}

/// Paint-only syntax spans for a code block's text, as JSON
/// `[[start16, len16, [r, g, b, a]], ...]` (`crate::syntax::Utf16Span`), in
/// the dark palette or, with `light` nonzero, the light one. Runs the
/// highlighter on the calling thread: call it from the adapter's worker.
/// Unknown languages and texts over `syntax::MAX_BYTES` return `[]`; an
/// empty buffer means the input was unreadable.
///
/// # Safety
/// `language` must point to `language_len` bytes and `text` to `text_len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_syntax_spans(
    language: *const u8,
    language_len: usize,
    text: *const u8,
    text_len: usize,
    light: u8,
) -> RustNativeBuffer {
    if language.is_null()
        || text.is_null()
        || language_len == 0
        || language_len > 32
        || text_len > crate::syntax::MAX_BYTES
    {
        return buffer(vec![]);
    }
    catch_unwind(|| {
        let language = unsafe { std::slice::from_raw_parts(language, language_len) };
        let text = unsafe { std::slice::from_raw_parts(text, text_len) };
        let (Ok(language), Ok(text)) = (std::str::from_utf8(language), std::str::from_utf8(text))
        else {
            return vec![];
        };
        let spans: Vec<_> = crate::syntax::highlight_utf16(language, text, light != 0)
            .into_iter()
            .map(|span| (span.start, span.len, span.rgba))
            .collect();
        serde_json::to_vec(&spans).unwrap_or_default()
    })
    .map(buffer)
    .unwrap_or_else(|_| buffer(vec![]))
}

/// # Safety
/// The buffer must be an unmodified, not-yet-freed result from this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_layout_buffer_free(value: RustNativeBuffer) {
    if !value.data.is_null() {
        unsafe {
            drop(Box::from_raw(ptr::slice_from_raw_parts_mut(
                value.data, value.len,
            )))
        }
    }
}

/// # Safety
/// The handle must come from `rust_native_layout_create`, have no call in
/// progress, and not be destroyed already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_native_layout_destroy(handle: *mut RustNativeLayout) {
    if !handle.is_null() {
        unsafe { drop(Box::from_raw(handle)) }
    }
}
