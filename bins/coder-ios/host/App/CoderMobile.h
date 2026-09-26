#ifndef CODER_MOBILE_H
#define CODER_MOBILE_H

#include <stddef.h>
#include <stdint.h>

// ABI version 1. Reader calls are serialized on one non-UI queue.
// Input is borrowed only for the call. Output belongs to Rust and must be
// released once with coder_mobile_buffer_free; no bytes contain a terminator.
typedef struct {
    uint8_t *data;
    size_t len;
} CoderMobileBuffer;

void *coder_mobile_create(const uint8_t *configuration, size_t length);
CoderMobileBuffer coder_mobile_call(void *handle, const uint8_t *request, size_t length);
void coder_mobile_buffer_free(CoderMobileBuffer buffer);
void coder_mobile_destroy(void *handle);

// The world handle is independent of the reader handle. Create, call, and
// destroy it on the main thread while its CAMetalLayer mount remains alive.
CoderMobileBuffer coder_verse_blueprint(void);
void *coder_verse_create(void *layer, const uint8_t *configuration, size_t length);
CoderMobileBuffer coder_verse_call(void *handle, const uint8_t *request, size_t length);
void coder_verse_destroy(void *handle);

#endif
