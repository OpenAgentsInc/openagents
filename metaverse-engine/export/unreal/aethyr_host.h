/* The host interface of a Metaverse Engine world for a native host (the Unreal bridge,
 * docs/unreal/2026-11-12-aethyr-to-unreal-exporter-spec.md). It is the browser host's
 * interface (coax/web/host.js, a world's -face.c) with real pointers: the web face passes every
 * pointer as a 32-bit offset into WebAssembly.Memory, which a 64-bit target cannot hold.
 * A world's -ue-face.c defines these; the host only calls them. Plain C, no engine headers. */
#ifndef AETHYR_HOST_H
#define AETHYR_HOST_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

size_t aethyr_state_size(void);
void aethyr_state_init(void *state);              /* state: aethyr_state_size() zeroed bytes */
uint32_t aethyr_max_vertices(void);               /* the overlay stream's capacity, in vertices */
uint32_t aethyr_vertex_floats(void);              /* floats per overlay vertex */
uint32_t aethyr_uniform_floats(void);
/* One frame. Writes the overlay stream and the uniform block; returns the overlay's vertices. */
uint32_t aethyr_frame(void *state, float *vertices, float *uniforms, float width, float height, float dpr, float dt, int still);

/* The scene's stores: vertices in order, six 32-bit words each (metaverse-engine/render/cel-mesh.coax). */
uint32_t aethyr_stores(void);
const uint32_t *aethyr_store(void *state, int k);
uint32_t aethyr_store_bytes(void *state, int k);
uint32_t aethyr_store_version(void *state, int k); /* changes when the store's bytes do */
uint32_t aethyr_draw_count(void *state);

int aethyr_key(void *state, int key, int down);
int aethyr_pointer(void *state, float x, float y, int phase);
int aethyr_mouselook(void *state);                /* 1: the host should capture the mouse */
void aethyr_look_delta(void *state, float dx, float dy);
void aethyr_pointer_lock(void *state, int status);
void aethyr_message(void *state, int kind, const char *body, size_t length);
/* A fact of the run for checks and logs; what each number means is the world's (its -ue-face.c). */
int aethyr_fact(void *state, int k);

/* What moves, by kind, for a host that draws some kinds with its own models. aethyr_things
 * copies the last frame's list into out (cap floats) and returns how many things: eight floats
 * each, kind, variant, x, y, z, yaw, scale, phase. A hidden kind is listed but not written to the
 * live store. aethyr_thing_first and _count give thing k's vertices in the live store. */
int aethyr_things(void *state, float *out, int cap);
int aethyr_thing_first(void *state, int k);
int aethyr_thing_count(void *state, int k);
void aethyr_thing_hide(void *state, int kind, int hide);
/* What has happened since the last call, oldest first, one number each (the manifest names
 * them); the list is emptied. Returns how many. */
int aethyr_events(void *state, float *out, int cap);

#ifdef __cplusplus
}
#endif
#endif
