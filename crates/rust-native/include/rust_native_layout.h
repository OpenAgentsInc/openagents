#ifndef RUST_NATIVE_LAYOUT_H
#define RUST_NATIVE_LAYOUT_H

#include <stddef.h>
#include <stdint.h>

// Rust Native transcript layout (crates/rust-native/src/layout). An
// application library exports these symbols when it enables rust-native's
// `ffi` feature. A handle belongs to one transcript. Calls on one handle must
// not overlap, but may come from any thread, so an adapter can lay out on a
// worker queue. After an update, rust_native_layout_frame returns an
// immutable frame that any thread may read while later updates run; release
// it once with rust_native_frame_release. Output buffers belong to Rust and
// must be released once with rust_native_layout_buffer_free; an empty buffer
// means failure. The measurer is called only during an update, on the
// updating thread.

typedef struct {
    uint8_t *data;
    size_t len;
} RustNativeBuffer;

// One styled range of a paragraph, in UTF-16 code units.
typedef struct {
    float size;        // points, already scaled by the reader's text size
    uint8_t weight;    // 0 regular, 1 medium, 2 semibold, 3 bold
    uint8_t italic;
    uint8_t monospace;
    uint8_t reserved;
    uint32_t start16;
    uint32_t end16;
} RustNativeTextRun;

// One broken line: its UTF-16 range (including trailing whitespace and a hard
// break), its typographic width without trailing whitespace, and metrics.
typedef struct {
    uint32_t start16;
    uint32_t end16;
    float width;
    float ascent;
    float descent;
    float leading;
} RustNativeTextLine;

typedef struct {
    uint32_t index;
    uint32_t reserved;
    uint64_t version;  // changes whenever the row's painted content can change
    float y;
    float height;
} RustNativeRowPlacement;

// Breaks `text` (UTF-8) styled by `runs` into lines at `width` points; a width
// of zero or less breaks only at hard line breaks. Write the lines and, for
// each line in order, the x offset of every run boundary strictly inside it
// (a run boundary is the start16 of every run after the first). Return 0 on
// success, 1 when a capacity is too small (set the counts to what is needed),
// and any other value on failure.
typedef int32_t (*RustNativeMeasure)(void *context,
                                     const uint8_t *text, size_t text_len,
                                     const RustNativeTextRun *runs, size_t run_count,
                                     float width,
                                     RustNativeTextLine *lines, size_t line_capacity,
                                     size_t *line_count,
                                     float *offsets, size_t offset_capacity,
                                     size_t *offset_count);

void *rust_native_layout_create(void *context, RustNativeMeasure measure);

// With rust-native's `shaping` feature: a layout that shapes text in Rust with
// the bundled fonts (Inter and JetBrains Mono, SIL Open Font License), so it
// needs no measurer. Draw each run with the face and variations
// rust_native_font_spec names for its style's font; the font files come from
// rust_native_font_data and live as long as the process.
void *rust_native_layout_create_shaped(void);

typedef struct {
    uint32_t face;     // 0 Inter, 1 Inter Italic, 2 JetBrains Mono, 3 its italic
    float weight;      // the `wght` axis value
    float optical;     // the `opsz` axis value, or 0 when the face has none
    uint8_t calt;      // whether contextual alternates stay on
    uint8_t reserved[3];
} RustNativeFontSpec;

// weight: 0 regular, 1 medium, 2 semibold, 3 bold.
RustNativeFontSpec rust_native_font_spec(float size, uint8_t weight, uint8_t italic, uint8_t monospace);
const uint8_t *rust_native_font_data(uint32_t face, size_t *length);
// Applies a JSON update: {"width", "scale", "order": [keys], "rows": [nodes
// that are new or changed], "expanded": [tool keys], "earlier": {"label",
// "loading"} or null, "curve": [[nominal, scaled] sizes, at most 16]}. With
// "source": NAME instead of order, rows, and earlier, Rust reads the rows and
// the earlier control from that published transcript source (a transcript
// node whose "source" prop is NAME) and lays out only the rows that changed.
// Returns {"count", "height", "relaid", "measured", "micros"} or {"error"}.
RustNativeBuffer rust_native_layout_update(void *handle, const uint8_t *update, size_t length);
// Writes at most `capacity` placements of the rows intersecting y0..y1 and
// returns how many rows intersect.
size_t rust_native_layout_rows(const void *handle, float y0, float y1,
                               RustNativeRowPlacement *out, size_t capacity);
float rust_native_layout_height(const void *handle);
int32_t rust_native_layout_find(const void *handle, const uint8_t *key, size_t length,
                                RustNativeRowPlacement *out);
// A row's display list as JSON: styles, texts, runs, rects, links, widgets,
// scrollers, accessibility, and copy text.
RustNativeBuffer rust_native_layout_display(void *handle, uint32_t index);
void rust_native_layout_buffer_free(RustNativeBuffer buffer);
void rust_native_layout_destroy(void *handle);

// Publishes a transcript node's rows (JSON, a node whose element is a
// transcript without a source) as the source `name`, for adapters whose rows
// arrive as JSON. An application that owns its rows in Rust publishes them
// directly instead. Returns 1 on success.
int32_t rust_native_source_publish(const uint8_t *name, size_t name_length,
                                   const uint8_t *node, size_t length);
void rust_native_source_retire(const uint8_t *name, size_t name_length);

// The handle's current layout as an immutable frame, or null. The caller owns
// it; it outlives later updates and the handle itself.
const void *rust_native_layout_frame(void *handle);
size_t rust_native_frame_count(const void *frame);
float rust_native_frame_height(const void *frame);
size_t rust_native_frame_rows(const void *frame, float y0, float y1,
                              RustNativeRowPlacement *out, size_t capacity);
int32_t rust_native_frame_find(const void *frame, const uint8_t *key, size_t length,
                               RustNativeRowPlacement *out);
// A row's key (UTF-8) and display list (JSON), or an empty buffer.
RustNativeBuffer rust_native_frame_key(const void *frame, uint32_t index);
RustNativeBuffer rust_native_frame_display(const void *frame, uint32_t index);
void rust_native_frame_release(const void *frame);

// A native text field's shared draft (rust_native::edit::mirror): the
// field reports each change as JSON and shows the state the reply names.
// Requests: {"op": "mount", "token", "max_bytes", "draft"}, {"op": "apply",
// "stamp", "change": {"op": "sync", "text", "selection": [anchor, caret],
// "marked": [start, end] or null, "at_ms"} | {"op": "select", "selection"} |
// {"op": "delete", "backwards", "at_ms"} | {"op": "undo"} | {"op": "redo"}},
// {"op": "submitted", "stamp", "text"}, and {"op": "dispose"}. Positions are
// UTF-16. Replies: {"state": {"stamp", "text", "selection", "marked",
// "can_undo", "can_redo"}} or {"error": code, "state"}. Free replies with
// rust_native_layout_buffer_free.
void *rust_native_editor_create(void);
RustNativeBuffer rust_native_editor_call(void *handle, const uint8_t *request, size_t length);
void rust_native_editor_destroy(void *handle);

// Paint-only syntax spans for one code block's text (a display list's
// code_blocks names the paragraph and language), as JSON
// [[start16, len16, [r, g, b, a]], ...] in the dark palette or, with light
// nonzero, the light one. Runs the highlighter on the calling thread: call it
// from a worker. Unknown languages return []. Free with
// rust_native_layout_buffer_free.
RustNativeBuffer rust_native_syntax_spans(const uint8_t *language, size_t language_length,
                                          const uint8_t *text, size_t length, uint8_t light);

#endif
