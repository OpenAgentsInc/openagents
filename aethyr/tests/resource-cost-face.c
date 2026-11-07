/* Thin ABI projection for identical Coax byte/codec code in Wasm. */
#include "resource-cost-parity.h"
#include <stdint.h>
__attribute__((export_name("resource_cost_word")))
int64_t resource_word(int64_t index) { return resource_cost_word(index); }
