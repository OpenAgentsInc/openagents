/* Firm reason: 1 (scalar browser ABI and the host's imports; the site is Coax). */
// gamebuilder.org's C face for the browser: scalar Wasm exports over
// the page (web/page.coax, the module web/site.coax or web/site-agents.coax), for coax/web/dom-host.js. The host keeps one Site, calls
// dom_frame each frame it is asked to, applies the commands at dom_out, and
// reports presses, typing, sizes, answers and the address. After each frame
// and each event this does what the site asks of the host: puts an address in
// the bar without loading a page (dom_go), asks the browser's storage for what the site keeps there
// (coaxial_storage_get and _put), and sends the login's or the chat's request (a
// POST of the body, or a GET with none, to this site only: dom_stream), whose
// answer comes back as events 4 and 5.
// The module's header: site.h, or /agents' module's (web/site-agents-face.c names site-agents.h).
#ifndef GS_HEADER
#define GS_HEADER "site.h"
#endif
#include GS_HEADER
#include <stddef.h>
#include <string.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))

IMPORT("coaxial_random") uint32_t coaxial_random(uint32_t out, int32_t length);
#undef EXPORT
#define COAX_ENTROPY_HEADER GS_HEADER
#include "../../coax/platform/entropy-wasm.c"

IMPORT("dom_stream") void dom_stream(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len);
IMPORT("dom_go") void dom_go(uint32_t at, uint32_t len, uint32_t push);
// The browser's byte storage (coax/web/STORAGE.md, given by dom-wire.js): what is built is kept in this browser
// (web/keep.coax decides the namespace, the keys and the bytes). 0: accepted, and the reply is event 768 + slot.
IMPORT("coaxial_storage_put") int32_t coaxial_storage_put(int32_t slot, int32_t token, uint32_t ns, int32_t ns_len, int32_t version, uint32_t key, int32_t key_len, uint32_t bytes, int32_t bytes_len);
IMPORT("coaxial_storage_get") int32_t coaxial_storage_get(int32_t slot, int32_t token, uint32_t ns, int32_t ns_len, int32_t version, uint32_t key, int32_t key_len);

static coax_v_t_Site at(uint32_t state) { return (coax_v_t_Site){(coax_t_Site *)(uintptr_t)state, 1}; }
static coax_t_Site *raw(uint32_t state) { return (coax_t_Site *)(uintptr_t)state; }
static void asks(uint32_t state) {
  int64_t go = gs_go(at(state));
  if (go > 0) dom_go((uint32_t)(uintptr_t)raw(state)->go.e, (uint32_t)go, 1u);
  // the storage requests the site has waiting (a few at most: one a slot): asked, and one refused at once is told so
  for (int k = 0; k < 8; k++) {
    int64_t op = gs_keep_next(at(state));
    if (op == 0) break;
    coax_t_Site *s = raw(state);
    int32_t slot = (int32_t)gs_keep_slot(at(state)), token = (int32_t)gs_keep_token(at(state)), version = (int32_t)gs_keep_version();
    uint32_t ns = (uint32_t)(uintptr_t)s->ns.e, key = (uint32_t)(uintptr_t)s->keep.e[0].key.e;
    int32_t ns_len = (int32_t)gs_keep_ns_len(), key_len = (int32_t)gs_keep_key_len(at(state)), refused;
    if (op == 1) refused = coaxial_storage_get(slot, token, ns, ns_len, version, key, key_len);
    else {
      int64_t which = gs_keep_which(at(state));
      uint8_t *bytes = which == 1 ? s->keep.e[0].doc.e : which == 2 ? s->keep.e[0].set.e : which == 3 ? s->keep.e[0].link.e : s->keep.e[0].game.e;
      refused = coaxial_storage_put(slot, token, ns, ns_len, version, key, key_len, (uint32_t)(uintptr_t)bytes, (int32_t)gs_keep_len(at(state)));
    }
    if (refused != 0) gs_keep_refused(at(state), refused);
  }
  if (!gs_wants(at(state))) return;
  gs_sent(at(state));
  dom_stream((uint32_t)(uintptr_t)raw(state)->req_url.e, (uint32_t)gs_url_len(at(state)), (uint32_t)(uintptr_t)raw(state)->req_body.e, (uint32_t)gs_body_len(at(state)));
}

EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Site); }
// A new state's declared defaults (coax_default_T, from the compiler). The host calls this first.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) {
  coax_default_t_Site(raw(state));
  uint8_t nonce[16] = {0};
  uint32_t result = coaxial_random((uint32_t)(uintptr_t)nonce, sizeof nonce);
  gs_entropy(at(state), (coax_v_u8){nonce, sizeof nonce}, result == 0);
  memset(nonce, 0, sizeof nonce);
}
// One frame: bytes of commands at dom_out to apply, 0 for none, -1 to clear the page and ask again.
EXPORT("dom_frame") int32_t dom_frame(uint32_t state, float dt, uint32_t reduced) { int32_t n = (int32_t)gs_frame(at(state), dt, reduced != 0); asks(state); return n; }
EXPORT("dom_out") uint32_t dom_out(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->dom.e[0].out.e; }
EXPORT("dom_busy") uint32_t dom_busy(uint32_t state) { return gs_busy(at(state)) ? 1u : 0u; }
EXPORT("dom_inbox") uint32_t dom_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.e; }
EXPORT("dom_inbox_cap") uint32_t dom_inbox_cap(void) { return (uint32_t)gs_inbox_cap(); }
EXPORT("dom_event") void dom_event(uint32_t state, int32_t kind, int32_t slot, int32_t length) { gs_event(at(state), kind, slot, length); asks(state); }
EXPORT("dom_size") void dom_size(uint32_t state, int32_t slot, float w, float h) { gs_size(at(state), slot, w, h); }
// The copy button's text (a command line's code, /login/cli): the host writes it to the clipboard after the frame, then says so.
EXPORT("dom_clip") uint32_t dom_clip(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->clip.e; }
EXPORT("dom_clip_len") uint32_t dom_clip_len(uint32_t state) { return (uint32_t)gs_clip_len(at(state)); }
EXPORT("dom_clip_done") void dom_clip_done(uint32_t state) { gs_clip_done(at(state)); }
