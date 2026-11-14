// /chat's C face for the browser: scalar Wasm exports over chat-web.coax,
// for coax/web/dom-host.js. The page records no analytics. The host keeps one
// ChatWeb, calls dom_frame each frame it is asked to, applies the commands
// at dom_out, and reports presses, typing and framed nodes' sizes. When the
// page has a request to send (a message for the model), the face hands it to
// the host's dom_stream, and the answer's pieces come back as events 4 and 5
// (chat-web.coax CW_PIECE, CW_END).
#include "chat-web.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))
// A request to this site (a POST of the body, a GET with none) whose answer comes back in pieces as it
// arrives; one at a time (a new one ends the last).
IMPORT("dom_stream") void dom_stream(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len);
IMPORT("dom_stream_stop") void dom_stream_stop(void);
// A setting kept in this browser (Settings, This machine), given back as event 12 when the page starts.
IMPORT("dom_keep") void dom_keep(uint32_t at, uint32_t len);
// An address on this site: pushed, the page stays and the address bar shows it (the app shell drew it);
// else the browser goes there.
IMPORT("dom_go") void dom_go(uint32_t at, uint32_t len, uint32_t push);

static coax_v_t_ChatWeb at(uint32_t state) { return (coax_v_t_ChatWeb){(coax_t_ChatWeb *)(uintptr_t)state, 1}; }
static coax_t_ChatWeb *raw(uint32_t state) { return (coax_t_ChatWeb *)(uintptr_t)state; }

// What chat-web.coax asks of the host: to end the stream, or to send its request.
static void asks(uint32_t state) {
  coax_t_ChatWeb *s = raw(state);
  if (coaxial_chatweb_stop(at(state))) dom_stream_stop();
  int64_t kept = coaxial_chatweb_keep_len(at(state));
  if (kept > 0) { dom_keep((uint32_t)(uintptr_t)s->keep.e, (uint32_t)kept); coaxial_chatweb_keep_done(at(state)); }
  int64_t go = coaxial_chatweb_go_len(at(state));
  if (go > 0) { uint32_t push = coaxial_chatweb_go_push(at(state)) ? 1u : 0u; coaxial_chatweb_go_done(at(state)); dom_go((uint32_t)(uintptr_t)s->go.e, (uint32_t)go, push); }
  if (coaxial_chatweb_asking(at(state))) {   // a POST of its body, or a GET when it has none
    int64_t n = coaxial_chatweb_ask(at(state));
    dom_stream((uint32_t)(uintptr_t)s->door.e, (uint32_t)s->door_n, (uint32_t)(uintptr_t)s->request.e, (uint32_t)(n < 0 ? 0 : n));
    coaxial_chatweb_asked(at(state));
  }
}

EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_ChatWeb); }
// A new state's declared defaults (coax_default_T, from the compiler). The host calls this first.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_ChatWeb(raw(state)); }
// One frame: bytes of commands at dom_out to apply, 0 for none, -1 to clear the page and ask again.
EXPORT("dom_frame") int32_t dom_frame(uint32_t state, float dt, uint32_t reduced) {
  int32_t n = (int32_t)coaxial_chatweb_frame(at(state), dt, reduced != 0);
  asks(state);   // a frame may ask too: the account as the page opens, a Claude turn read each second
  return n;
}
EXPORT("dom_out") uint32_t dom_out(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->dom.e[0].out.e; }
EXPORT("dom_busy") uint32_t dom_busy(uint32_t state) { return coaxial_chatweb_busy(at(state)) ? 1u : 0u; }
EXPORT("dom_inbox") uint32_t dom_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.e; }
EXPORT("dom_inbox_cap") uint32_t dom_inbox_cap(void) { return (uint32_t)coaxial_chatweb_inbox_cap(); }
EXPORT("dom_event") void dom_event(uint32_t state, int32_t kind, int32_t slot, int32_t length) { coaxial_chatweb_event(at(state), kind, slot, length); asks(state); }
EXPORT("dom_size") void dom_size(uint32_t state, int32_t slot, float w, float h) { coaxial_chatweb_size(at(state), slot, w, h); }
EXPORT("dom_clip") uint32_t dom_clip(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->clip.e; }
EXPORT("dom_clip_len") uint32_t dom_clip_len(uint32_t state) { return (uint32_t)coaxial_chatweb_clip(at(state)); }
EXPORT("dom_clip_done") void dom_clip_done(uint32_t state) { coaxial_chatweb_clip_done(at(state)); }
// An action for the page that opened this conversation (the editor: host.js): a hosted turn's plan for its world (8),
// or the composer's voice held or let go (7) and an answer to speak (9).
EXPORT("dom_act_kind") int32_t dom_act_kind(uint32_t state) { return (int32_t)coaxial_chatweb_act_kind(at(state)); }
EXPORT("dom_act") uint32_t dom_act(uint32_t state) { return (uint32_t)(uintptr_t)(coaxial_chatweb_act_planned(at(state)) ? raw(state)->planned.e : raw(state)->vact.e); }
EXPORT("dom_act_len") uint32_t dom_act_len(uint32_t state) { return (uint32_t)coaxial_chatweb_act_len(at(state)); }
EXPORT("dom_act_done") void dom_act_done(uint32_t state) { coaxial_chatweb_act_done(at(state)); }
