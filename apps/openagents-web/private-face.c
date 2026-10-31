// A private project page's C face for the browser: scalar Wasm exports over
// private.coax, and the requests it asks for (a read of the state, or one
// post) handed to the host's HTTP import (coax/web/host.js) with their
// answers wanted. The page records no analytics: what it shows is private.
#include "private.h"
#include "Qr-memory.h"
#include <stddef.h>
#include <string.h>
// coax/lib/nostr/weblogin.coax (its own header repeats types private.h already has)
_Bool weblogin_hex(coax_v_u8 entropy, coax_v_u8 out);
int64_t weblogin_derive(coax_string hex, coax_v_u8 words, coax_v_u8 npub);
_Bool weblogin_check(coax_string typed, coax_v_u8 hex);
int64_t weblogin_sign(coax_string hex, coax_string url, int64_t created_at, coax_string body, coax_v_u8 aux, coax_v_u8 out);

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))
IMPORT("coaxial_http") int32_t coaxial_http(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len, int32_t flags);
IMPORT("coaxial_now_seconds") int64_t coaxial_now_seconds(void);
// The browser's secure random (crypto.getRandomValues) under the shared Coax entropy policy (coax/lib/crypto/entropy.coax).
#define COAX_ENTROPY_HEADER "Entropy.h"
#include "../../coax/platform/entropy-wasm.c"
IMPORT("coaxial_random") uint32_t coaxial_random(uint32_t out, int32_t length);

// The host keeps one PrivateHost: the PrivateState and the buffers it sees, bound again before every call.
static coax_v_t_PrivateState at(uint32_t state) {
  coax_t_PrivateHost *h = (coax_t_PrivateHost *)(uintptr_t)state;
  coaxial_private_bind((coax_v_t_PrivateHost){h, 1});
  return (coax_v_t_PrivateState){h->s.e, 1};
}

static void wipe(volatile uint8_t *p, size_t n) { while (n--) *p++ = 0; }
// /login's words account: what private.coax asks for in acct_want (WANT_*), done in Coax
// (coax/lib/nostr/weblogin.coax). The entropy, words and key never leave this memory.
static void do_account(uint32_t state) {
  coax_t_PrivateState *s = at(state).data;
  int64_t want = s->acct_want;
  if (!want) return;
  int ok = 0;
  if (want == 1) {   // WANT_NEW: 16 bytes from the browser, as hex, then derive
    uint8_t e[16];
    if (coaxial_random((uint32_t)(uintptr_t)e, 16) == 0 && weblogin_hex((coax_v_u8){e, 16}, (coax_v_u8){s->acct_hex.e, 32})) { s->acct_hex_n = 32; want = 2; }
    wipe(e, sizeof e);
    if (want == 1) { coaxial_private_acct_done(at(state), 0); return; }
  }
  if (want == 2) {   // WANT_DERIVE
    int64_t both = s->acct_hex_n != 32 ? -1 : weblogin_derive((coax_string){(const char *)s->acct_hex.e, 32}, (coax_v_u8){s->acct_words.e, (int64_t)sizeof s->acct_words.e},
                                                          (coax_v_u8){s->acct_npub.e, (int64_t)sizeof s->acct_npub.e});
    ok = both > 0;
    if (ok) { s->acct_words_n = both / 256; s->acct_npub_n = both % 256; }
  } else if (want == 3) {   // WANT_SIGN
    uint8_t aux[32];
    if (coaxial_random((uint32_t)(uintptr_t)aux, 32) != 0) memset(aux, 0, sizeof aux);
    static const char body[] = "{\"kind\":\"login\"}";
    int64_t n = s->acct_hex_n != 32 ? -1 : weblogin_sign((coax_string){(const char *)s->acct_hex.e, 32}, (coax_string){(const char *)s->acct_url.e, s->acct_url_n}, coaxial_now_seconds(),
                                                         (coax_string){body, (int64_t)sizeof body - 1}, (coax_v_u8){aux, 32},
                                                         (coax_v_u8){s->acct_auth.e, (int64_t)sizeof s->acct_auth.e});
    wipe(aux, sizeof aux);
    ok = n > 0; s->acct_auth_n = ok ? n : 0;
  } else if (want == 4) {   // WANT_CHECK
    ok = weblogin_check((coax_string){(const char *)s->swap.e, s->swap_n}, (coax_v_u8){s->swap_hex.e, 32});
  }
  coaxial_private_acct_done(at(state), ok != 0);
}

// What private.coax asks for, once there's a free slot (flag 4: answer wanted; 1: POST; 8: the player key).
static void send_asks(uint32_t state) {
  do_account(state);
  coax_t_PrivateState *s = at(state).data;
  if (s->qr_want) {   // a phone code's URL: its QR module grid (coax/lib/qr)
    /* a tip's invoice in capitals, alphanumeric (~487 characters is past byte mode's 412); a URL in byte mode */
    coax_t_QrResult r = s->qr_text_n > 0
      ? qr_encode_alnum((coax_v_u8){s->qr_text.e, s->qr_text_n}, (coax_v_u8){s->qr.e, (int64_t)sizeof s->qr.e})
      : qr_encode((coax_v_u8){s->code_url.e, s->code_n}, (coax_v_u8){s->qr.e, (int64_t)sizeof s->qr.e});
    coaxial_private_qr(at(state), r.status, r.size);
  }
  if (s->ask_n > 0) {
    int32_t id = coaxial_http((uint32_t)(uintptr_t)s->ask.e, (uint32_t)s->ask_n, 0, 0, 4);
    if (id >= 0) coaxial_private_asked(at(state), id);
  }
  if (s->post_n > 0) {
    int32_t id = coaxial_http((uint32_t)(uintptr_t)s->post.e, (uint32_t)s->post_n, (uint32_t)(uintptr_t)s->body.e, (uint32_t)s->body_n,
                               (int32_t)coaxial_private_post_flags(at(state)));
    if (id >= 0) coaxial_private_posted(at(state), id);
  }
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_PrivateHost) * 7u + offsetof(coax_t_PrivateHost, inbox) * 3u + sizeof(coax_t_PrivateState) * 5u + 0x505256u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_PrivateHost); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_private_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_projects_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_projects_uniform_floats(); }
EXPORT("coaxial_page") void coaxial_page(uint32_t state, int32_t page) { coaxial_private_page(at(state), page); }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_private_max_vertices() * coaxial_projects_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_projects_uniform_floats()};
  int64_t n = coaxial_private_frame(at(state), out, u, width, height, dpr, dt, still != 0);
  send_asks(state);
  return n < 0 ? 0u : (uint32_t)n;
}
EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  uint32_t used = coaxial_private_key(at(state), key, mods) ? 1u : 0u;
  send_asks(state);
  return used;
}
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)id;
  uint32_t used = coaxial_private_pointer(at(state), x, y, phase, kind) ? 1u : 0u;
  send_asks(state);
  return used;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { coaxial_private_wheel(at(state), dy); }
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { return (uint32_t)(uintptr_t)at(state).data->inbox.data; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return (uint32_t)coaxial_private_inbox_cap(); }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, int32_t length) { coaxial_private_message(at(state), kind, length); send_asks(state); }
EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) { coaxial_private_hash(at(state), (int64_t)length); }
EXPORT("coaxial_wants_text") int32_t coaxial_wants_text(uint32_t state) { return coaxial_private_wants_text(at(state)) ? 1 : 0; }
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)at(state).data->field.e; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)coaxial_private_cursor(at(state)); }
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)coaxial_private_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)at(state).data->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)at(state).data->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { coaxial_private_action_done(at(state)); }
