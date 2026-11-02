/* Chapter 59's C face: scalar Wasm exports over search.coax. It holds the
   search state and the downloaded bytes in static storage (Coax has no heap)
   and decides nothing: every check and every number comes from Coax. */
#include "search.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_BswState state;
static uint8_t manifest_buf[4096], encoder_buf[4u << 20], index_buf[8u << 20], text_buf[65536], out_buf[1u << 20];
static coax_v_t_BswState st(void) { return (coax_v_t_BswState){&state, 1}; }
static coax_v_u8 view(uint8_t *p, uint32_t n) { return (coax_v_u8){p, n}; }

/* Where the page copies a file: 0 manifest, 1 encoder, 2 index, 3 text (a query or a query file), 4 output. */
EXPORT("search_buffer") uint32_t search_buffer(uint32_t kind) {
  uint8_t *b[] = {manifest_buf, encoder_buf, index_buf, text_buf, out_buf};
  return kind < 5 ? (uint32_t)(uintptr_t)b[kind] : 0;
}
EXPORT("search_capacity") uint32_t search_capacity(uint32_t kind) {
  uint32_t c[] = {sizeof manifest_buf, sizeof encoder_buf, sizeof index_buf, sizeof text_buf, sizeof out_buf};
  return kind < 5 ? c[kind] : 0;
}
EXPORT("search_state_bytes") uint32_t search_state_bytes(void) { return sizeof state; }
EXPORT("search_init") void search_init(void) { bsw_init(st()); }
EXPORT("search_load") int32_t search_load(uint32_t m, uint32_t e, uint32_t i) {
  if (m > sizeof manifest_buf || e > sizeof encoder_buf || i > sizeof index_buf) return -1;
  return (int32_t)bsw_load(st(), view(manifest_buf, m), view(encoder_buf, e), view(index_buf, i));
}
EXPORT("search_message") int32_t search_message(void) { return (int32_t)bsw_message(st(), view(out_buf, sizeof out_buf)); }
EXPORT("search_queries") int32_t search_queries(uint32_t n) {
  if (n > sizeof text_buf) return -1;
  return (int32_t)bsw_queries(st(), view(text_buf, n), view(out_buf, sizeof out_buf));
}
EXPORT("search_query") int32_t search_query(uint32_t n, uint32_t k) {
  if (n > sizeof text_buf) return -1;
  return (int32_t)bsw_search(st(), view(text_buf, n), k, view(out_buf, sizeof out_buf));
}
EXPORT("search_count") int32_t search_count(uint32_t what) { return (int32_t)bsw_counts(st(), what); }
