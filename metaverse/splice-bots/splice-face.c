// Splice's C face for the browser (coax/web/host.js): splice-bots.coax is the page (the Mars world and
// its frame); this file decides nothing. It keeps the two stores the scene draws from, and asks
// animelab-style.coax (Anime Lab's style library) for the anime block each frame, which the page
// copies into the uniforms. Every pointer is an offset into the host's WebAssembly.Memory.
#include "splice-bots.h"
#include "Animelab-style.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_Splice *splice_state(uint32_t state) { return (coax_t_Splice *)(uintptr_t)state; }
static coax_v_t_Splice at(uint32_t state) { return (coax_v_t_Splice){splice_state(state), 1}; }
#define WORLD_WORDS (150000u * 6u)   /* splice-bots.coax SP_WORLD_WORDS × VERTEX_WORDS */
#define LIVE_WORDS (150000u * 6u)
static uint32_t world[WORLD_WORDS];
static uint32_t live[LIVE_WORDS];
#define PIECE_WORDS ((80000u + 10u * 30000u) * 6u)    /* layer-draw.coax SD_PIECE_WORDS: every kind of placed piece's mesh, written once (SD_KIND_VERTS), then the player's ten walk poses */
#define PLACE_WORDS ((65u + 540u) * 8u)   /* layer-draw.coax SD_PLACE_WORDS: a row a placed piece, the player's, then the solar farm's 540 banks (farm.coax FF_BANKS) */
static uint32_t pieces[PIECE_WORDS];
static uint32_t places[PLACE_WORDS];
#define IMPORT_BYTES 1048576u         /* pieces.coax SP_IMPORT_PACK: the world's import pack, as it arrives and as it was last read whole */
static uint8_t imports_in[IMPORT_BYTES];
static uint8_t imports[IMPORT_BYTES];
static float style[128];            /* STYLE_FLOATS */
static uint32_t frames;
static bool golden;

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Splice) * 1000003u + offsetof(coax_t_Splice, world_n) * 13u + 0x53504c43u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Splice); }
// A new state's declared defaults (coax_default_T, from the compiler). The host calls this first,
// and again on a code reload that changes the state's layout, before it carries the old fields over.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Splice(splice_state(state)); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_splice_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_splice_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_splice_uniform_floats(); }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

// The world `splice` (world.coax): the page's one request at a time goes to the host, and its
// answer comes back through the inbox (512 + the host's id).
__attribute__((import_module("env"), import_name("coaxial_http"))) int32_t coaxial_http(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len, int32_t flags);
static void world_request(uint32_t state) {
  if (!splice_req_wants(at(state))) return;
  coax_string url = splice_req_url(at(state)), body = splice_req_body(at(state));
  splice_req_sent(at(state), coaxial_http((uint32_t)(uintptr_t)url.data, (uint32_t)url.count, (uint32_t)(uintptr_t)body.data, (uint32_t)body.count, (int32_t)splice_req_flags(at(state))));
}

// The host calls this as each engine starts, after the state is kept (coax/web/host.js loadEngine):
// on a code reload it dropped the old engine's requests, so the one the world had out is asked again.
EXPORT("coaxial_page") void coaxial_page(uint32_t state, int32_t page) { (void)page; splice_engine_started(at(state)); }

// A fact of the page for a headless check (coaxial.fieldProbe): its place in the world, and from 100 the run's.
// The editor shell's model of this page's world (coax/web/host.js carries it to the shell on ?edit).
EXPORT("coaxial_edit_rev") int32_t coaxial_edit_rev(uint32_t state) { return (int32_t)splice_edit_rev(at(state)); }
EXPORT("coaxial_edit_model") uint32_t coaxial_edit_model(uint32_t state) { return (uint32_t)(uintptr_t)splice_edit_model(at(state)).data; }
EXPORT("coaxial_edit_model_len") uint32_t coaxial_edit_model_len(uint32_t state) { return (uint32_t)splice_edit_model(at(state)).count; }
EXPORT("coaxial_field_probe") int32_t coaxial_field_probe(uint32_t state, int32_t field) { return (int32_t)splice_world_fact(at(state), field); }

// Realm scripts (#13467): the scripts the tables name, the frame out and the result in. The host
// (coax/web/host.js, "A realm's scripts") copies the frame to the page's script module and the result
// back into the inbox.
static uint8_t script_frame[32768];
EXPORT("coaxial_script_wants") uint32_t coaxial_script_wants(uint32_t state) { return (uint32_t)(uintptr_t)splice_script_wants(at(state)).data; }
EXPORT("coaxial_script_wants_len") uint32_t coaxial_script_wants_len(uint32_t state) { return (uint32_t)splice_script_wants(at(state)).count; }
EXPORT("coaxial_script_frame") uint32_t coaxial_script_frame(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)script_frame; }
EXPORT("coaxial_script_frame_len") uint32_t coaxial_script_frame_len(uint32_t state) { return (uint32_t)splice_script_frame(at(state), (coax_v_u8){script_frame, sizeof script_frame}); }

// This browser's storage (coax/web/STORAGE.md): your character in the crew page's record (slot 0)
// and the run this browser keeps (slot 1); the host answers through the inbox (768 + slot).
__attribute__((import_module("env"), import_name("coaxial_storage_put"))) int32_t coaxial_storage_put(int32_t slot, int32_t token, uint32_t space, int32_t space_len, int32_t version, uint32_t key, int32_t key_len, uint32_t bytes, int32_t bytes_len);
__attribute__((import_module("env"), import_name("coaxial_storage_get"))) int32_t coaxial_storage_get(int32_t slot, int32_t token, uint32_t space, int32_t space_len, int32_t version, uint32_t key, int32_t key_len);
static const char crew_space[] = "crew", crew_key[] = "character";   /* crew-player.coax CP_STORE_SPACE, CP_STORE_KEY */
static const char run_space[] = "splice", run_key[] = "run";         /* front.coax FR_STORE_SPACE, FR_STORE_KEY */
static void store(uint32_t state) {
  int64_t wants = splice_store_wants(at(state));
  int32_t slot = wants <= 2 ? 0 : 1;
  uint32_t space = (uint32_t)(uintptr_t)(slot == 0 ? crew_space : run_space), key = (uint32_t)(uintptr_t)(slot == 0 ? crew_key : run_key);
  int32_t space_len = slot == 0 ? 4 : 6, key_len = slot == 0 ? 9 : 3;
  if (wants == 1 || wants == 3) {
    splice_store_asked(at(state), wants, coaxial_storage_get(slot, (int32_t)wants, space, space_len, 1, key, key_len) == 0);
  } else if (wants == 2 || wants == 4) {
    coax_string text = splice_store_text(at(state), wants);
    splice_store_asked(at(state), wants, coaxial_storage_put(slot, (int32_t)wants, space, space_len, 1, key, key_len, (uint32_t)(uintptr_t)text.data, (int32_t)text.count) == 0);
  }
}

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_splice_max_vertices() * coaxial_splice_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_splice_uniform_floats()};
  for (int i = 0; i < 128; i++) style[i] = 0;
  /* each scene picks its own look: its anime variant (metaverse/style-preset.coax) and clock */
  animelab_style_block(splice_variant(at(state)), animelab_phone(width, dpr), splice_clock(at(state)), (coax_v_f32){style, 128});
  int64_t n = splice_frame(at(state), out, u, width, height, dpr, dt, (still != 0) || golden,
                           (coax_v_u32){world, WORLD_WORDS}, (coax_v_u32){live, LIVE_WORDS},
                           (coax_v_u32){pieces, PIECE_WORDS}, (coax_v_u32){places, PLACE_WORDS}, (coax_v_f32){style, 128}, (coax_v_u8){imports, IMPORT_BYTES});
  frames += 1;
  world_request(state);
  store(state);
  return n < 0 ? 0u : (uint32_t)n;
}

// The world's import pack (GitHub #13420): the page's one asset, a data asset the host fetches and,
// on the dev server, fetches again by content hash when an import changes it. The host copies the
// bytes to where coaxial_asset says; splice-bots.coax reads them whole or keeps the copy from before.
// A world with no imports has none: the asset is optional, and the page opens without it.
EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) { (void)state; return i == 0 && bytes <= sizeof imports_in ? (uint32_t)(uintptr_t)imports_in : 0u; }
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(uint32_t state, int32_t i) { (void)state; return i == 0 ? (uint32_t)sizeof imports_in : 0u; }
EXPORT("coaxial_asset_optional_v1") uint32_t coaxial_asset_optional_v1(int32_t i) { (void)i; return 1; }
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) {
  if (i == 0 && bytes > 0 && bytes <= sizeof imports_in) splice_imports(at(state), (coax_string){(const char *)imports_in, (int64_t)bytes}, (coax_v_u8){imports, IMPORT_BYTES});
}

// The scene: draws of vertices in order (no index). Store 0 is the Mars world, written once; store 1
// what moves, written every frame; store 2 the meshes of the kinds of placed piece, written once;
// store 3 a row for each placed piece, written when the edit layer changes. A kind's placed
// pieces are one draw of its mesh, an instance each (gpu/splice-bots.coax scene_vs).
/* the sounds to play now, a line each (sound.coax): read once a frame, after its length */
static uint8_t sounds[2048];
EXPORT("coaxial_sounds_len") uint32_t coaxial_sounds_len(uint32_t state) { return (uint32_t)splice_sounds(at(state), (coax_v_u8){sounds, sizeof sounds}); }
EXPORT("coaxial_sounds") uint32_t coaxial_sounds(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)sounds; }
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)splice_state(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)splice_draw_count(at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 4; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) {
  return k == 0 ? (uint32_t)sizeof world : k == 1 ? (uint32_t)sizeof live : k == 2 ? (uint32_t)sizeof pieces : (uint32_t)sizeof places;
}
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) {
  (void)state;
  return (uint32_t)(uintptr_t)(k == 0 ? world : k == 1 ? live : k == 2 ? pieces : places);
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) {
  if (k == 3) { int64_t rows = splice_place_n(at(state)); return rows <= 0 ? 0u : (uint32_t)(rows * 8 * 4); }
  int64_t n = k == 0 ? splice_world_n(at(state)) : k == 1 ? splice_live_n(at(state)) : splice_piece_n(at(state));
  return n <= 0 ? 0u : (uint32_t)(n * 6 * 4);
}
// The world and the kinds' meshes are written once and copied to the GPU once; the placed pieces'
// rows are copied when an edit changes them.
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) {
  return k == 0 ? (uint32_t)splice_world_version(at(state)) : k == 1 ? frames : k == 2 ? (uint32_t)splice_piece_version(at(state)) : (uint32_t)splice_place_version(at(state));
}

// Keys and the pointer go to the game (WASD, E, F, the edit bay's keys and buttons).
EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { (void)mods; return splice_key(at(state), key, true) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { (void)mods; return splice_key(at(state), key, false) ? 1u : 0u; }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)kind; (void)id; return splice_pointer(at(state), x, y, phase) ? 1u : 0u;
}
// Mouse look: the host holds the pointer while the smuggler walks and frees it in the edit bay.
EXPORT("coaxial_mouselook") uint32_t coaxial_mouselook(uint32_t state) { return (uint32_t)splice_mouselook(at(state)); }
EXPORT("coaxial_look_delta") void coaxial_look_delta(uint32_t state, float dx, float dy) { splice_look(at(state), dx, dy); }
EXPORT("coaxial_pointer_lock") void coaxial_pointer_lock(uint32_t state, int32_t status) { splice_lock(at(state), status); }
// The interface is ink on paper: the overlay blends source over (gpu/splice-bots.coax fx_fs).
EXPORT("coaxial_fx_alpha_v1") uint32_t coaxial_fx_alpha_v1(void) { return 1; }

static uint8_t inbox[131072];   /* the page's facts (kind 600), a driver's command (kind 700), a message from the page that holds this realm's frame (kind 720), and the world's answers: a document fits (world.coax SPW_INBOX) */
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
// A driver's command (coax drive, kind 700) is answered in the state; the host reads the answer here.
EXPORT("coaxial_drive_out") uint32_t coaxial_drive_out(uint32_t state) { return (uint32_t)(uintptr_t)splice_drive_out(at(state)).data; }
EXPORT("coaxial_drive_out_len") uint32_t coaxial_drive_out_len(uint32_t state) { return (uint32_t)splice_drive_out(at(state)).count; }
EXPORT("coaxial_script_result") uint32_t coaxial_script_result(uint32_t state, uint32_t length) {
  if (length > sizeof inbox) return 0;
  return splice_script_result(at(state), (coax_v_u8){inbox, (int64_t)length}) ? 1u : 0u;
}
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  splice_message(at(state), kind, (coax_string){(const char *)inbox, (int64_t)length});
  world_request(state);
  store(state);
}
