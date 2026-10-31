// The native realm face: what verse/kms/kms.c asks of one realm. It is the
// browser contract (coax/web/host.js, the realm's -face.c) without Wasm,
// plus the realm's light as scalar Coax (its -light.coax), because there is
// no WGSL here. A face decides nothing; the realm's Coax does.
#ifndef VERSE_KMS_REALM_H
#define VERSE_KMS_REALM_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// Named keys, as in coax/web/host.js KEYS.
enum { REALM_ESCAPE = -1, REALM_ENTER = -2, REALM_BACKSPACE = -3, REALM_LEFT = -4, REALM_RIGHT = -5, REALM_UP = -6, REALM_DOWN = -7 };

const char *realm_name(void);
size_t realm_state_size(void);
int realm_max_vertices(void);
int realm_vertex_floats(void);   // 16: position, place, shape, tint
int realm_uniform_floats(void);
// Steps and draws a frame; returns the vertex count (triangles).
int realm_frame(void *state, float *vertices, float *uniforms, float w, float h, float scale, float dt);
bool realm_key(void *state, int key);
bool realm_key_up(void *state, int key);
// 1 when the realm asks to leave (it navigated); the host then exits.
int realm_action(void *state);
// The light: one background pixel, and one stroke fragment's light.
void realm_bg(const float *uniforms, float x, float y, float rgb[3]);
float realm_fx(const float *uniforms, float px, float py, const float *shape);
#endif
