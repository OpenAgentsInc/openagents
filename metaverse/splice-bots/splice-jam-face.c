// The jam realm's C face (/worlds/splice-bots-jam, splice-jam.coax): Splice's face (splice-face.c,
// included whole), with what the jam realm does differently. The state is a SpliceJam, whose first
// field is the Splice every one of Splice's exports takes, so splice-face.c hands it on as it is.
// The private pack's four parts are the page's assets 0 to 3 (catalog, vertices, indices, layers),
// all optional: without them the game plays in its own world. Two stores follow Splice's four: the
// jam's rows (store 4, written once the catalog is read) and its game actors (store 5, every frame).
#define SPLICE_FACE_JAM 1
#include "splice-face.c"
#include <string.h>

_Static_assert(offsetof(coax_t_SpliceJam, page) == 0, "the jam state starts with Splice's");
static coax_t_SpliceJam *jam_state(cx_face_addr state) { return (coax_t_SpliceJam *)(uintptr_t)state; }
static coax_v_t_SpliceJam jam_at(cx_face_addr state) { return (coax_v_t_SpliceJam){jam_state(state), 1}; }
static coax_t_SjStore jam_store[1];            /* the jam world read whole: static, the game holds slices into it (jam-world.coax) */
static coax_v_t_SjStore store_at(void) { return (coax_v_t_SjStore){jam_store, 1}; }
#define JAM_ROWS (8192u * 32u)                   /* jam-draw.coax SJD_ROWS x imported-read.coax IW_OBJECT */
#define JAM_ACTORS (512u * 12u)                  /* jam-world.coax SJ_ACTORS x jam-draw.coax SJD_ACTOR_FLOATS */
static float jam_rows[JAM_ROWS];
static float jam_actors[JAM_ACTORS];
static uint8_t catalog[4194304];
static uint32_t actor_frames;

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_SpliceJam) * 1000003u + offsetof(coax_t_SpliceJam, draws) * 13u + 0x534a414du);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_SpliceJam); }
// Splice's defaults in place, and the jam's (zero) beside them: the state is megabytes.
EXPORT("coaxial_state_init") void coaxial_state_init(cx_face_addr state) {
  memset(jam_state(state), 0, sizeof(coax_t_SpliceJam));
  coax_default_t_Splice(splice_state(state));
}

EXPORT("coaxial_frame")
uint32_t coaxial_frame(cx_face_addr state, cx_face_addr vertices, cx_face_addr uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_splice_max_vertices() * coaxial_splice_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_splice_uniform_floats()};
  for (int i = 0; i < 128; i++) style[i] = 0;
  animelab_style_block(splice_variant(at(state)), animelab_phone(width, dpr), splice_clock(at(state)), (coax_v_f32){style, 128});
  int64_t n = splice_jam_frame(jam_at(state), out, u, width, height, dpr, dt, (still != 0) || golden,
                               (coax_v_u32){world, WORLD_WORDS}, (coax_v_u32){live, LIVE_WORDS},
                               (coax_v_u32){pieces, PIECE_WORDS}, (coax_v_u32){places, PLACE_WORDS}, (coax_v_f32){style, 128},
                               (coax_v_f32){jam_actors, JAM_ACTORS});
  frames += 1; actor_frames += 1;
  world_request(state);
  store(state);
  return n < 0 ? 0u : (uint32_t)n;
}

// The private pack: its catalog comes into memory (asset 0); its vertices, indices and layers go to the GPU,
// and the page is told their bytes.
EXPORT("coaxial_asset") cx_face_addr coaxial_asset(cx_face_addr state, int32_t i, uint32_t bytes) { (void)state; return i == 0 && bytes <= sizeof catalog ? (cx_face_addr)(uintptr_t)catalog : 0u; }
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(cx_face_addr state, int32_t i) { (void)state; return i == 0 ? (uint32_t)sizeof catalog : 0u; }
EXPORT("coaxial_asset_optional_v1") uint32_t coaxial_asset_optional_v1(int32_t i) { (void)i; return 1; }
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(cx_face_addr state, int32_t i, uint32_t bytes) {
  if (i == 0 && bytes > 0 && bytes <= sizeof catalog) splice_jam_catalog(jam_at(state), store_at(), (coax_string){(const char *)catalog, (int64_t)bytes}, (coax_v_f32){jam_rows, JAM_ROWS});
  splice_jam_asset(jam_at(state), i, (int64_t)bytes);
}

// The scene: Splice's draws (in order, from its four stores), then the jam's (indexed, over the pack's).
EXPORT("coaxial_draws") cx_face_addr coaxial_draws(cx_face_addr state) { return (cx_face_addr)(uintptr_t)jam_state(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(cx_face_addr state) { return (uint32_t)splice_jam_draw_count(jam_at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 6; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) {
  return k == 0 ? (uint32_t)sizeof world : k == 1 ? (uint32_t)sizeof live : k == 2 ? (uint32_t)sizeof pieces : k == 3 ? (uint32_t)sizeof places : k == 4 ? (uint32_t)sizeof jam_rows : (uint32_t)sizeof jam_actors;
}
EXPORT("coaxial_store") cx_face_addr coaxial_store(cx_face_addr state, int32_t k) {
  (void)state;
  return k == 4 ? (cx_face_addr)(uintptr_t)jam_rows : k == 5 ? (cx_face_addr)(uintptr_t)jam_actors : (cx_face_addr)(uintptr_t)(k == 0 ? world : k == 1 ? live : k == 2 ? pieces : places);
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(cx_face_addr state, int32_t k) {
  if (k >= 4) { int64_t b = splice_jam_store_bytes(jam_at(state), k); return b <= 0 ? 0u : (uint32_t)b; }
  if (k == 3) { int64_t rows = splice_place_n(at(state)); return rows <= 0 ? 0u : (uint32_t)(rows * 8 * 4); }
  int64_t n = k == 0 ? splice_world_n(at(state)) : k == 1 ? splice_live_n(at(state)) : splice_piece_n(at(state));
  return n <= 0 ? 0u : (uint32_t)(n * 6 * 4);
}
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(cx_face_addr state, int32_t k) {
  if (k == 4) return (uint32_t)splice_jam_store_version(jam_at(state));
  if (k == 5) return actor_frames;
  return k == 0 ? (uint32_t)splice_world_version(at(state)) : k == 1 ? frames : k == 2 ? (uint32_t)splice_piece_version(at(state)) : (uint32_t)splice_place_version(at(state));
}
// For headless checks: splice-jam.coax splice_jam_fact (0 in the jam world, 1 to 4 the pack's parts, 5 rows, 6 draws, 7 actors...).
EXPORT("coaxial_guard_probe") int32_t coaxial_guard_probe(cx_face_addr state, int32_t field) { return (int32_t)splice_jam_fact(jam_at(state), field); }
