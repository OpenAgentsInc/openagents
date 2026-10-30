/* Test-only transport. The observer does not verify, hash or authorize. */
#include "observe.h"
#include <stdint.h>
__attribute__((export_name("observe"))) int observe(int64_t kind, int64_t created, int64_t now,
  uint32_t u, int un, uint32_t method, int mn, uint32_t payload, int pn,
  uint32_t url, int urln, uint32_t request_method, int rmn, uint32_t body_hash, int hn,
  int64_t uc, int64_t mc, int64_t pc, int malformed) {
  return n98_observe(kind,created,now,(coax_string){(const char *)(uintptr_t)u,un},
    (coax_string){(const char *)(uintptr_t)method,mn},(coax_string){(const char *)(uintptr_t)payload,pn},
    (coax_string){(const char *)(uintptr_t)url,urln},(coax_string){(const char *)(uintptr_t)request_method,rmn},
    (coax_string){(const char *)(uintptr_t)body_hash,hn},uc,mc,pc,malformed != 0);
}
