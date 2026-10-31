/* #foreign bridge over coax_entropy_native; see entropy-foreign.h. */
#include "entropy-foreign.h"
#include "entropy-native.h"
#include <string.h>

static uint8_t cxrand_out[CXRAND_MAX];
static uint8_t cxrand_scratch[CXRAND_MAX];
static int64_t cxrand_have = 0;

int64_t cxrand_fill(int64_t n) {
  cxrand_have = 0;
  memset(cxrand_out, 0, sizeof cxrand_out);
  /* Capacity is the buffer size: the Coax policy refuses n < 0 or n > it. */
  coax_t_Entropy_Result r = coax_entropy_native(cxrand_out, CXRAND_MAX, cxrand_scratch, CXRAND_MAX, n);
  if (r.error != 0) return (int64_t)r.error;
  cxrand_have = r.written;
  return 0;
}

int64_t cxrand_byte(int64_t i) {
  if (i < 0 || i >= cxrand_have) return -1;
  return cxrand_out[i];
}

int64_t cxrand_draw(coax_v_u8 out, int64_t n) {
  /* The policy admits n against out.count and the scratch; output commits to
     out only on full success. Nothing here is decided or kept. */
  return (int64_t)coax_entropy_native(out.data, out.count, cxrand_scratch, CXRAND_MAX, n).error;
}
