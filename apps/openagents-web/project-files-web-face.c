// A project's attachments' C face for the browser: scalar Wasm exports over project-files-web.coax,
// for coax/web/dom-host.js. The host keeps one ProjectFilesWeb, tells it the page's address once
// (event 13, from &path=), calls dom_frame each frame it is asked to, applies the commands at dom_out,
// and reports framed nodes' sizes. The one GET (the project's list) goes through the host's
// dom_stream, and the answer comes back as events 4 and 5 (PF_PIECE, PF_END).
#include "project-files-web.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))
// A request to this site (a GET when there is no body) whose answer comes back in pieces; one at a time.
IMPORT("dom_stream") void dom_stream(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len);

static coax_v_t_ProjectFilesWeb at(uint32_t state) { return (coax_v_t_ProjectFilesWeb){(coax_t_ProjectFilesWeb *)(uintptr_t)state, 1}; }
static coax_t_ProjectFilesWeb *raw(uint32_t state) { return (coax_t_ProjectFilesWeb *)(uintptr_t)state; }

// What project-files-web.coax asks of the host: its one GET.
static void asks(uint32_t state) {
  coax_t_ProjectFilesWeb *s = raw(state);
  if (coaxial_projectfilesweb_asking(at(state))) {
    coaxial_projectfilesweb_asked(at(state));
    dom_stream((uint32_t)(uintptr_t)s->door.e, (uint32_t)s->door_n, 0, 0);
  }
}

EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_ProjectFilesWeb); }
// A new state's declared defaults (coax_default_T, from the compiler). The host calls this first.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_ProjectFilesWeb(raw(state)); }
// One frame: bytes of commands at dom_out to apply, 0 for none, -1 to clear the page and ask again.
EXPORT("dom_frame") int32_t dom_frame(uint32_t state, float dt, uint32_t reduced) {
  int32_t n = (int32_t)coaxial_projectfilesweb_frame(at(state), dt, reduced != 0);
  asks(state);
  return n;
}
EXPORT("dom_out") uint32_t dom_out(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->dom.e[0].out.e; }
EXPORT("dom_busy") uint32_t dom_busy(uint32_t state) { return coaxial_projectfilesweb_busy(at(state)) ? 1u : 0u; }
EXPORT("dom_inbox") uint32_t dom_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.e; }
EXPORT("dom_inbox_cap") uint32_t dom_inbox_cap(void) { return (uint32_t)coaxial_projectfilesweb_inbox_cap(); }
EXPORT("dom_event") void dom_event(uint32_t state, int32_t kind, int32_t slot, int32_t length) { coaxial_projectfilesweb_event(at(state), kind, slot, length); asks(state); }
EXPORT("dom_size") void dom_size(uint32_t state, int32_t slot, float w, float h) { coaxial_projectfilesweb_size(at(state), slot, w, h); }
