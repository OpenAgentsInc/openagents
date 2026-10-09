#ifndef OPENAGENTS_MOBILE_H
#define OPENAGENTS_MOBILE_H

#include <stddef.h>
#include <stdbool.h>
#include <stdint.h>

// Transcript layout, exported by rust-native's `ffi` feature.
#include "../../../../crates/rust-native/include/rust_native_layout.h"

// Calls for one handle are serialized on one non-UI queue. Input is borrowed
// only for the call. Output belongs to Rust and must be released once with
// openagents_mobile_buffer_free; an empty buffer means the call failed.
typedef struct {
    uint8_t *data;
    size_t len;
} OpenAgentsMobileBuffer;

void *openagents_mobile_create(const uint8_t *configuration, size_t length);
OpenAgentsMobileBuffer openagents_mobile_call(void *handle, const uint8_t *request, size_t length);
void openagents_mobile_buffer_free(OpenAgentsMobileBuffer buffer);
void openagents_mobile_destroy(void *handle);
// Attach a photo's encoded bytes (PNG or JPEG, at most 8 MiB) to the open
// chat's draft; answers with the app packet. On the handle's queue.
OpenAgentsMobileBuffer openagents_mobile_attach_image(void *handle, const uint8_t *name, size_t name_length,
                                                      const uint8_t *bytes, size_t length);
// The encoded bytes the chat's image surface `resource` ("image:{id}")
// shows, or an empty buffer. On the handle's queue.
OpenAgentsMobileBuffer openagents_mobile_image(void *handle, const uint8_t *resource, size_t length);

// Blocks the calling thread until the app packet changes: returns Rust's
// change count once it differs from `seen`, or `seen` after `timeout_ms` (at
// most 60 seconds). Call it from a thread of its own, never the handle's
// queue; it takes no handle. Then ask for a packet with {"op":"changed"}.
uint64_t openagents_mobile_wait(uint64_t seen, uint32_t timeout_ms);
// Whether the Coder tab shows. While it does and its chat is live,
// openagents_mobile_wait also returns once a second.
void openagents_mobile_coder_shown(bool shown);
/* Whether this build shows the preview features (the Verse, the Gym,
   Trainer, Playtest, Tailnet): built with OPENAGENTS_MOBILE_PREVIEW=on. */
bool openagents_mobile_preview(void);

// The Verse tab's world. Create, call, and destroy it on the main thread while
// its CAMetalLayer stays alive. Results are released with
// openagents_mobile_buffer_free; an empty result means the call failed.
void *openagents_verse_create(void *layer, const uint8_t *configuration, size_t length);
OpenAgentsMobileBuffer openagents_verse_create_error(void);
OpenAgentsMobileBuffer openagents_verse_call(void *handle, const uint8_t *request, size_t length);
void openagents_verse_destroy(void *handle);
// Connects the world's Everglade studio to the paired computer whose host key
// is the UTF-8 `host`, under the grant the app handle holds for it. Answers
// {"connected":true,"rights":[...]} or {"connected":false,"error":"..."}.
// Call it on the main thread while no other call uses the app handle.
OpenAgentsMobileBuffer openagents_verse_studio_connect(void *verse, void *app, const uint8_t *host,
                                                       size_t length);

#endif
