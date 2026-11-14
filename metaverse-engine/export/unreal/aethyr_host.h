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

/* The scene's stores: vertices in order, six 32-bit words each (metaverse-engine/render/cel-mesh.coax),
 * or a store of rows (below). Store 0 is the world, written once; store 1 what moves, every frame. */
uint32_t aethyr_stores(void);
const uint32_t *aethyr_store(void *state, int k);
uint32_t aethyr_store_bytes(void *state, int k);
uint32_t aethyr_store_version(void *state, int k); /* changes when the store's bytes do */
uint32_t aethyr_draw_count(void *state);          /* the page's draws: the two stores and every instanced draw */
/* The instanced draws (a world's placed pieces, and anything else drawn as a mesh written once in its
 * own space): six words a draw into out (cap words), returns how many draws. A draw is the mesh's
 * store, its first vertex and its vertices, then the rows' store, the first row and the rows. A
 * row is eight floats: x, y, z, 0, cos turn, sin turn, 0, 0. The mesh is turned about y (its +z goes
 * to (sin turn, 0, cos turn)), then moved to x, y, z, once for each row. */
int aethyr_instanced(void *state, uint32_t *out, int cap);
/* The world's one request at a time (the browser host's coaxial_http): 0 when it has none, else its
 * flags (1 POST, 4 the answer is wanted), and the URL (a path on the world's site) and body through
 * the pointers. The host says what id it gave it with aethyr_request_sent (below 0: it could not take
 * it now), and the answer comes as message 512 + id, "STATUS\n" then the body (status 0: no answer).
 * Ids are 0 to 7. */
int aethyr_request(void *state, const char **url, size_t *url_length, const char **body, size_t *body_length);
void aethyr_request_sent(void *state, int id);

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
 * each, kind, variant, x, y, z, yaw, scale, phase. A variant is what the thing is doing, as its
 * world numbers it; phase is where it is in its movement's cycle (radians, a whole turn a cycle).
 * A hidden kind is listed but not written to the live store. aethyr_thing_first and _count give thing k's vertices in the live store. */
int aethyr_things(void *state, float *out, int cap);
int aethyr_thing_first(void *state, int k);
int aethyr_thing_count(void *state, int k);
void aethyr_thing_hide(void *state, int kind, int hide);
/* What has happened since the last call, oldest first, one number each (the manifest names
 * them); the list is emptied. Returns how many. */
int aethyr_events(void *state, float *out, int cap);

/* For a host's effects (docs/unreal/2026-11-14-what-belongs-in-unreal.md, section 5). A host asks for
 * events one way or the other, not both: either call empties the list.
 * aethyr_event_records: what has happened since the last call, oldest first, aethyr_event_floats()
 * floats each (nine): the event's number, its place x, y, z in metres, its direction as a unit vector
 * (0, 0, 0: none), its strength (1 is ordinary), and the id of the thing it belongs to (-1: none).
 * aethyr_thing_states: the last frame's things, aethyr_thing_state_floats() floats each (sixteen): id,
 * kind, variant, flags, place x, y, z, yaw, velocity x, y, z (metres a second), a level from 0 to 1
 * (how far through what it is doing), the place its action is at x, y, z (a drill's bit; else its own
 * place), and 0. The first records are aethyr_things' in its order; after them come things the world
 * draws as part of its scene and does not list. The manifest names the fields and the flags' bits
 * ("event_record", "thing_state"). An id is the kind times 64 plus the thing's place among its kind. */
int aethyr_event_floats(void);
int aethyr_event_records(void *state, float *out, int cap);
int aethyr_thing_state_floats(void);
int aethyr_thing_states(void *state, float *out, int cap);

#ifdef __cplusplus
}
#endif
#endif
