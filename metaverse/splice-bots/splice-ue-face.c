// Splice's C face for a native host (metaverse-engine/export/unreal/aethyr_host.h): the same decisions as
// splice-face.c makes for the browser, which is none. It keeps the four stores and asks
// animelab-style.coax for the anime block each frame. Pointers are pointers here.
#include "splice.h"
#include "Animelab-style.h"
#include "aethyr_host.h"
#include <stdbool.h>
#include <string.h>
// The world as the jam build plays it, baked in by splice-export WORLD_DIR: the live lane's
// document as the API answers it, and its content tables. With it the face answers the page's
// requests itself (no server, no network), and the world opens at the first frame.
#if defined(__has_include)
#if __has_include("aethyr_world.h")
#include "aethyr_world.h"
#define AETHYR_WORLD_BAKED 1
#endif
#endif

#define WORLD_WORDS (150000u * 6u)   /* splice-bots.coax SP_WORLD_WORDS × VERTEX_WORDS */
#define LIVE_WORDS (150000u * 6u)
static uint32_t world[WORLD_WORDS];
static uint32_t live[LIVE_WORDS];
// The edit layer's stores, as splice-face.c keeps them (layer-draw.coax): store 2 every kind of
// placed piece's mesh, written once, then your walk poses; store 3 a row for each placed piece and
// one for you. aethyr_instanced hands a host the draws of them.
#define PIECE_WORDS ((80000u + 10u * 30000u) * 6u)    /* layer-draw.coax SD_PIECE_WORDS */
#define PLACE_WORDS ((65u + 540u) * 8u)   /* layer-draw.coax SD_PLACE_WORDS: a row a placed piece, the player's, then the solar farm's 540 banks (farm.coax FF_BANKS) */
static uint32_t pieces[PIECE_WORDS];
static uint32_t places[PLACE_WORDS];
// The world's import pack: a native host reads none yet, so a kind whose mesh is in the pack draws nothing.
static uint8_t imports[1];
static float style[128];            /* STYLE_FLOATS */
static uint32_t frames;

static coax_v_t_Splice at(void *state) { return (coax_v_t_Splice){(coax_t_Splice *)state, 1}; }

size_t aethyr_state_size(void) { return sizeof(coax_t_Splice); }
// A new state's declared defaults (coax_default_T, as splice-face.c gives the browser's): zeroed
// memory is not its start (the editor's clock, the world's revisions are -1). A baked world: the
// page's facts name the live site, so it follows the world `splice-bots` on its live lane.
void aethyr_state_init(void *state) {
  coax_default_t_Splice((coax_t_Splice *)state);
#ifdef AETHYR_WORLD_BAKED
  static const char page[] = "https://openagents.com/worlds/splice-bots";
  splice_message(at(state), 600, (coax_string){page, (int64_t)(sizeof page - 1)});
#endif
}
#ifdef AETHYR_WORLD_BAKED
// The page's request answered from the baked world, as the API would: the document, the content
// tables, and no events (nothing is published after it). Host id 7, answered at once.
static char baked_answer[sizeof AETHYR_WORLD_STATE + sizeof AETHYR_WORLD_TABLES + 16];
static void baked_requests(void *state) {
  for (int k = 0; k < 4 && splice_req_wants(at(state)); k++) {
    coax_string u = splice_req_url(at(state));
    const char *body = AETHYR_WORLD_STATE; size_t n = sizeof AETHYR_WORLD_STATE - 1;
    int is_events = 0, is_content = 0;
    for (int64_t i = 0; i + 8 <= u.count; i++) {
      if (!memcmp(u.data + i, "/events?", 8)) is_events = 1;
      if (i + 9 <= u.count && !memcmp(u.data + i, "/content?", 9)) is_content = 1;
    }
    if (is_events) { body = ""; n = 0; }
    if (is_content) { body = AETHYR_WORLD_TABLES; n = sizeof AETHYR_WORLD_TABLES - 1; }
    memcpy(baked_answer, "200\n", 4); memcpy(baked_answer + 4, body, n);
    splice_req_sent(at(state), 7);
    splice_message(at(state), 512 + 7, (coax_string){baked_answer, (int64_t)(n + 4)});
  }
}
#endif
uint32_t aethyr_max_vertices(void) { return (uint32_t)coaxial_splice_max_vertices(); }
uint32_t aethyr_vertex_floats(void) { return (uint32_t)coaxial_splice_vertex_floats(); }
uint32_t aethyr_uniform_floats(void) { return (uint32_t)coaxial_splice_uniform_floats(); }

uint32_t aethyr_frame(void *state, float *vertices, float *uniforms, float width, float height, float dpr, float dt, int still) {
  coax_v_f32 out = {vertices, coaxial_splice_max_vertices() * coaxial_splice_vertex_floats()};
  coax_v_f32 u = {uniforms, coaxial_splice_uniform_floats()};
  for (int i = 0; i < 128; i++) style[i] = 0;
  animelab_style_block(splice_variant(at(state)), animelab_phone(width, dpr), splice_clock(at(state)), (coax_v_f32){style, 128});
  int64_t n = splice_frame(at(state), out, u, width, height, dpr, dt, still != 0,
                           (coax_v_u32){world, WORLD_WORDS}, (coax_v_u32){live, LIVE_WORDS},
                           (coax_v_u32){pieces, PIECE_WORDS}, (coax_v_u32){places, PLACE_WORDS}, (coax_v_f32){style, 128}, (coax_v_u8){imports, 0});
  frames += 1;
#ifdef AETHYR_WORLD_BAKED
  baked_requests(state);
#endif
  return n < 0 ? 0u : (uint32_t)n;
}

uint32_t aethyr_stores(void) { return 4; }
const uint32_t *aethyr_store(void *state, int k) { (void)state; return k == 0 ? world : k == 1 ? live : k == 2 ? pieces : places; }
uint32_t aethyr_store_bytes(void *state, int k) {
  if (k == 3) { int64_t rows = splice_place_n(at(state)); return rows <= 0 ? 0u : (uint32_t)(rows * 8 * 4); }
  int64_t n = k == 0 ? splice_world_n(at(state)) : k == 1 ? splice_live_n(at(state)) : splice_piece_n(at(state));
  return n <= 0 ? 0u : (uint32_t)(n * 6 * 4);
}
uint32_t aethyr_store_version(void *state, int k) {
  return k == 0 ? (uint32_t)splice_world_version(at(state)) : k == 1 ? frames : k == 2 ? (uint32_t)splice_piece_version(at(state)) : (uint32_t)splice_place_version(at(state));
}
uint32_t aethyr_draw_count(void *state) { int64_t n = splice_draw_count(at(state)); return n < 0 ? 0u : (uint32_t)n; }
// The page's draws from the third on are instanced (splice-bots.coax splice_frame): a kind's mesh in
// the piece store, its instances the place store's rows from the base instance less 2 (gpu/splice-bots.coax sp_row).
int aethyr_instanced(void *state, uint32_t *out, int cap) {
  const uint32_t *d = ((coax_t_Splice *)state)->draws.e;
  int n = 0;
  for (int64_t k = 2; k < splice_draw_count(at(state)) && (n + 1) * 6 <= cap; k++) {
    const uint32_t *r = d + k * 4;
    uint32_t base = r[2] & 0x7fffffffu;
    if (base < 2 || r[1] == 0 || r[3] == 0) continue;
    out[n * 6] = 2; out[n * 6 + 1] = r[0]; out[n * 6 + 2] = r[1];
    out[n * 6 + 3] = 3; out[n * 6 + 4] = base - 2; out[n * 6 + 5] = r[3];
    n++;
  }
  return n;
}
// The world `splice-bots` (world.coax): the page's one request at a time, as splice-face.c hands it to the browser.
int aethyr_request(void *state, const char **url, size_t *url_length, const char **body, size_t *body_length) {
  if (!splice_req_wants(at(state))) return 0;
  coax_string u = splice_req_url(at(state)), b = splice_req_body(at(state));
  *url = u.data; *url_length = (size_t)u.count; *body = b.data; *body_length = (size_t)b.count;
  return (int)splice_req_flags(at(state));
}
void aethyr_request_sent(void *state, int id) { splice_req_sent(at(state), id); }

int aethyr_key(void *state, int key, int down) { return splice_key(at(state), key, down != 0) ? 1 : 0; }
int aethyr_pointer(void *state, float x, float y, int phase) { return splice_pointer(at(state), x, y, phase) ? 1 : 0; }
int aethyr_mouselook(void *state) { return (int)splice_mouselook(at(state)); }
void aethyr_look_delta(void *state, float dx, float dy) { splice_look(at(state), dx, dy); }
void aethyr_pointer_lock(void *state, int status) { splice_lock(at(state), status); }
void aethyr_message(void *state, int kind, const char *body, size_t length) {
  splice_message(at(state), kind, (coax_string){body, (int64_t)length});
}
int aethyr_fact(void *state, int k) { return (int)splice_fact(at(state), k); }
// what moves, by kind, and what has happened (splice-game.coax, "Things and events")
int aethyr_things(void *state, float *out, int cap) { return (int)splice_things(at(state), (coax_v_f32){out, cap}); }
int aethyr_thing_first(void *state, int k) { return (int)splice_thing_first(at(state), k); }
int aethyr_thing_count(void *state, int k) { return (int)splice_thing_count(at(state), k); }
void aethyr_thing_hide(void *state, int kind, int hide) { splice_thing_hide(at(state), kind, hide != 0); }
int aethyr_events(void *state, float *out, int cap) { return (int)splice_events(at(state), (coax_v_f32){out, cap}); }
