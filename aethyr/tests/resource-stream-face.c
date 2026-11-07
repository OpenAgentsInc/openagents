/* Thin ABI projection of the same Coax owner journey. */
#include "resource-stream-parity.h"
#include <stdint.h>
__attribute__((export_name("resource_stream_word")))
int64_t stream_word(int64_t index) { return resource_stream_word(index); }
