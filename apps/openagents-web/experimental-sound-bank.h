// Experimental's own synthesized effects (metaverse-engine/audio/experimental-sfx.coax through
// everglade/sound.coax), rendered once when the host starts audio and played by name: the host's
// coaxial_sound_bank and coaxial_sound_bank_len (coax/web/host.js soundBank). A face whose app loads
// everglade/sound.coax includes this once, after its own header and EXPORT: 5E Lab's and
// Everglade Experimental's (fivelab-face.c), Everglade II's (everglade2-face.c, #13058).
#ifndef EXPERIMENTAL_SOUND_BANK_H
#define EXPERIMENTAL_SOUND_BANK_H
#include <stdint.h>
#include <string.h>
#include "face-address.h"
#define XSOUND_BANK (1u << 20)
static uint8_t xsound_bank[XSOUND_BANK];
static int64_t xsound_mix[32000];
static char xsound_list[2048];
static uint32_t xsound_list_n;
EXPORT("coaxial_sound_bank_len") uint32_t coaxial_sound_bank_len(cx_face_addr state) {
  (void)state;
  if (xsound_list_n || experimental_sound_scratch() > (int64_t)(sizeof xsound_mix / sizeof xsound_mix[0]) || experimental_sound_bytes() > (int64_t)XSOUND_BANK) return xsound_list_n;
  uint32_t at = 0, n = 0;
  for (int64_t i = 0; i < experimental_sound_count(); i++) {
    uint8_t name[32];
    int64_t nn = experimental_sound_name(i, (coax_v_u8){name, sizeof name});
    int64_t b = experimental_sound_render(i, (coax_v_s64){xsound_mix, sizeof xsound_mix / sizeof xsound_mix[0]}, (coax_v_u8){xsound_bank + at, XSOUND_BANK - at});
    if (nn <= 0 || b <= 0 || n + nn + 36 > sizeof xsound_list) continue;
    memcpy(xsound_list + n, name, (size_t)nn); n += (uint32_t)nn;
    uint64_t parts[2] = { (uint64_t)(uintptr_t)(xsound_bank + at), (uint64_t)b };   /* an address is the host's width */
    for (int p = 0; p < 2; p++) {
      char d[24]; int dn = 0; uint64_t v = parts[p];
      do { d[dn++] = (char)('0' + v % 10); v /= 10; } while (v);
      xsound_list[n++] = ' ';
      while (dn) xsound_list[n++] = d[--dn];
    }
    xsound_list[n++] = '\n';
    at += (uint32_t)b;
  }
  xsound_list_n = n;
  return n;
}
EXPORT("coaxial_sound_bank") cx_face_addr coaxial_sound_bank(cx_face_addr state) { (void)state; return (cx_face_addr)(uintptr_t)xsound_list; }
/* host contract v1 (metaverse-engine/host/README.md, Audio): the list's allocation, and the one every sound's bytes are in */
EXPORT("coaxial_sound_bank_capacity") uint32_t coaxial_sound_bank_capacity(cx_face_addr state) { (void)state; return (uint32_t)sizeof xsound_list; }
EXPORT("coaxial_sound_bank_storage") cx_face_addr coaxial_sound_bank_storage(cx_face_addr state) { (void)state; return (cx_face_addr)(uintptr_t)xsound_bank; }
EXPORT("coaxial_sound_bank_storage_bytes") uint32_t coaxial_sound_bank_storage_bytes(cx_face_addr state) { (void)state; return XSOUND_BANK; }
#endif
