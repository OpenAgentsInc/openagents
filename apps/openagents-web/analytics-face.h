// The C side of analytics.coax for a page's face: hands the Track's batch to
// the host's HTTP import (coax/web/host.js, "HTTP") after every call into the
// page, as the faces drain their socket pipes. It decides nothing.
#pragma once
#include <stdint.h>

#ifndef COAXIAL_IMPORT
#define COAXIAL_IMPORT(name) __attribute__((import_module("env"), import_name(name)))
#endif
COAXIAL_IMPORT("coaxial_http") int32_t coaxial_http(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len, int32_t flags);

static inline void track_drain(coax_t_Track *t) {
  if (t->out_n <= 0) return;
  coax_string url = track_url();
  coaxial_http((uint32_t)(uintptr_t)url.data, (uint32_t)url.count, (uint32_t)(uintptr_t)t->out.e, (uint32_t)t->out_n, (int32_t)t->flags);
  track_sent((coax_v_t_Track){t, 1});
}
static inline coax_string track_cstr(const char *s) { int64_t n = 0; while (s[n]) n++; return (coax_string){s, n}; }
