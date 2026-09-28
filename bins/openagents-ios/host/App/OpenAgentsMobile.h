#ifndef OPENAGENTS_MOBILE_H
#define OPENAGENTS_MOBILE_H

#include <stddef.h>
#include <stdint.h>

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

#endif
