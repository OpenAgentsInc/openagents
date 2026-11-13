/* Thin ABI projection for the same Coax journey in native and Wasm. */
#include "query-world.h"
#include <stdint.h>
__attribute__((export_name("query_world_word")))
int64_t query_word(int64_t mode) { return query_world_word(mode); }
