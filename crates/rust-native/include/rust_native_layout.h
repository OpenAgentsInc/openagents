#ifndef RUST_NATIVE_LAYOUT_H
#define RUST_NATIVE_LAYOUT_H

#include <stddef.h>
#include <stdint.h>

// Rust Native transcript layout (crates/rust-native/src/layout). An
// application library exports these symbols when it enables rust-native's
// `ffi` feature. A handle belongs to one transcript; create, call, and
// destroy it on one thread. Output buffers belong to Rust and must be released
// once with rust_native_layout_buffer_free; an empty buffer means failure.

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
// Applies a JSON update: {"width", "scale", "order": [keys], "rows": [nodes
// that are new or changed], "expanded": [tool keys], "earlier": {"label",
// "loading"} or null}. Returns {"count", "height", "relaid", "measured",
// "micros"} or {"error"}.
RustNativeBuffer rust_native_layout_update(void *handle, const uint8_t *update, size_t length);
// Writes at most `capacity` placements of the rows intersecting y0..y1 and
// returns how many rows intersect.
size_t rust_native_layout_rows(const void *handle, float y0, float y1,
                               RustNativeRowPlacement *out, size_t capacity);
float rust_native_layout_height(const void *handle);
int32_t rust_native_layout_find(const void *handle, const uint8_t *key, size_t length,
                                RustNativeRowPlacement *out);
// A row's display list as JSON: styles, texts, runs, rects, links, widgets,
// accessibility, and copy text.
RustNativeBuffer rust_native_layout_display(void *handle, uint32_t index);
void rust_native_layout_buffer_free(RustNativeBuffer buffer);
void rust_native_layout_destroy(void *handle);

#endif
