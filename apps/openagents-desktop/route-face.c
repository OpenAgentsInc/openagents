// The router's C face (router.coax): Mammoth's decision for one message and
// this Mac's environment, for the Swift host. Blocks while curl runs.
#include "router.h"
#include "face.h"
#include <stdlib.h>

int32_t dk_route(const uint8_t *state, int32_t length, uint8_t *out, int32_t cap) {
  static uint8_t scratch[262144], err[8192];
  coax_string s = {(const char *)state, length};
  coax_v_u8 o = {out, cap}, sc = {scratch, sizeof scratch}, e = {err, sizeof err};
  return (int32_t)coaxial_route(s, o, sc, e);
}
