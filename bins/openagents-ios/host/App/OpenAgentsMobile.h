#ifndef OPENAGENTS_MOBILE_H
#define OPENAGENTS_MOBILE_H

#include <stddef.h>
#include <stdint.h>

// Output belongs to Rust and must be released once with
// openagents_mobile_buffer_free. An empty buffer means no view.
typedef struct {
    uint8_t *data;
    size_t len;
} OpenAgentsMobileBuffer;

OpenAgentsMobileBuffer openagents_mobile_home(void);
void openagents_mobile_buffer_free(OpenAgentsMobileBuffer buffer);

#endif
