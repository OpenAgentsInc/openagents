// /realms/audio-lab's C face for the browser (#12832): scalar Wasm exports over audiolab.coax, the
// frame's sound lines for the host (coax/web/host.js playSoundText), and the bank it renders once
// audio starts: Everglade's synthesized set and the room's impulse (aethyr/audio/sfx.coax, mix.coax).
// This page records no analytics of its own.
#include "audiolab.h"
#include <stddef.h>
#include <string.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_v_t_LabState at(uint32_t state) { return (coax_v_t_LabState){(coax_t_LabState *)(uintptr_t)state, 1}; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_LabState) * 7u + offsetof(coax_t_LabState, lines) * 3u + 0x4c4142u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_LabState); }
// Its state declares no defaults the zeroed memory lacks: the first frame sets it up (lab_init).
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { (void)state; }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_lab_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_projects_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_projects_uniform_floats(); }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_lab_max_vertices() * coaxial_projects_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_projects_uniform_floats()};
  int64_t n = coaxial_lab_frame(at(state), out, u, width, height, dpr, dt, still != 0);
  return n < 0 ? 0u : (uint32_t)n;
}
EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { (void)mods; return coaxial_lab_key(at(state), key) ? 1u : 0u; }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)id;
  return coaxial_lab_pointer(at(state), x, y, phase, kind) ? 1u : 0u;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { coaxial_lab_wheel(at(state), dy); }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)coaxial_lab_cursor(at(state)); }
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)coaxial_lab_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)at(state).data->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)at(state).data->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { coaxial_lab_action_done(at(state)); }

// ---- sound: this frame's lines, and the bank rendered once audio starts
EXPORT("coaxial_sounds") uint32_t coaxial_sounds(uint32_t state) { return (uint32_t)(uintptr_t)at(state).data->lines.e; }
EXPORT("coaxial_sounds_len") uint32_t coaxial_sounds_len(uint32_t state) { return (uint32_t)coaxial_lab_sounds_len(at(state)); }
#define SOUND_BANK (3u << 20)
static uint8_t sound_bank[SOUND_BANK];
static int64_t sound_mix[300000];
static char sound_list[4096];
static uint32_t sound_list_n;
EXPORT("coaxial_sound_bank_len") uint32_t coaxial_sound_bank_len(uint32_t state) {
  (void)state;
  if (sound_list_n) return sound_list_n;
  uint32_t a = 0, n = 0;
  for (int64_t i = 0; i < coaxial_lab_sound_count(); i++) {
    uint8_t name[32];
    int64_t nn = coaxial_lab_sound_name(i, (coax_v_u8){name, sizeof name});
    int64_t b = coaxial_lab_sound_render(i, (coax_v_s64){sound_mix, sizeof sound_mix / sizeof sound_mix[0]}, (coax_v_u8){sound_bank + a, SOUND_BANK - a});
    if (nn <= 0 || b <= 0 || n + nn + 24 > sizeof sound_list) continue;
    memcpy(sound_list + n, name, (size_t)nn); n += (uint32_t)nn;
    uint32_t parts[2] = { (uint32_t)(uintptr_t)(sound_bank + a), (uint32_t)b };
    for (int p = 0; p < 2; p++) {
      char d[12]; int dn = 0; uint32_t v = parts[p];
      do { d[dn++] = (char)('0' + v % 10); v /= 10; } while (v);
      sound_list[n++] = ' ';
      while (dn) sound_list[n++] = d[--dn];
    }
    sound_list[n++] = '\n';
    a += (uint32_t)b;
  }
  sound_list_n = n;
  return n;
}
EXPORT("coaxial_sound_bank") uint32_t coaxial_sound_bank(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)sound_list; }
