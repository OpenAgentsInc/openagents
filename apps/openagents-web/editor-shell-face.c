// The editor shell's C face for the browser: scalar Wasm exports over
// metaverse-engine/editor/shell.coax, for coax/web/dom-host.js. The host keeps one EdShell, calls
// dom_frame each frame it is asked to, applies the commands at dom_out, and reports presses,
// typing and framed nodes' sizes. The page that opened it (coax/web/host.js) puts its world's
// model in the inbox (event 6) and takes the shell's actions (dom_act). On a page the dev server
// serves (event 9) the shell asks that server for the realm's code (#13421): the ask goes by the
// host's dom_stream, and the answer comes back as events 4 and 5. The world's assets (#13420) are asked of
// the world's server by the host's dom_file_send, which also sends a dropped file (event 10); its answer is event 11.
#include "editor-shell.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))

IMPORT("dom_stream") void dom_stream(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len);
IMPORT("dom_file_send") void dom_file_send(uint32_t url, uint32_t url_len, uint32_t head, uint32_t head_len);

static coax_v_t_EdShell at(uint32_t state) { return (coax_v_t_EdShell){(coax_t_EdShell *)(uintptr_t)state, 1}; }
static coax_t_EdShell *raw(uint32_t state) { return (coax_t_EdShell *)(uintptr_t)state; }

// What shell.coax asks of the host: to send its ask to the dev server, and its ask to the world's server.
static void asks(uint32_t state) {
  coax_t_EdShell *s = raw(state);
  int64_t n = coaxial_editor_asking(at(state));
  if (n > 0) {
    dom_stream((uint32_t)(uintptr_t)s->code_door.e, (uint32_t)s->code_door_n, (uint32_t)(uintptr_t)s->code_req.e, (uint32_t)n);
    coaxial_editor_asked(at(state));
  }
  // and of the world's server (#13420): its assets (a GET), or a dropped file for it (the head, then the file's bytes)
  int64_t f = coaxial_editor_fetching(at(state));
  if (f > 0) {
    dom_file_send((uint32_t)(uintptr_t)s->as_url.e, (uint32_t)s->as_url_n, (uint32_t)(uintptr_t)s->as_head.e, f == 2 ? (uint32_t)s->as_head_n : 0u);
    coaxial_editor_fetched(at(state));
  }
}

EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_EdShell); }
// A new state's declared defaults (coax_default_T, from the compiler). The host calls this first.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_EdShell(raw(state)); }
// One frame: bytes of commands at dom_out to apply, 0 for none, -1 to clear the shell and ask again.
EXPORT("dom_frame") int32_t dom_frame(uint32_t state, float dt, uint32_t reduced) { return (int32_t)coaxial_editor_frame(at(state), dt, reduced != 0); }
EXPORT("dom_out") uint32_t dom_out(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->dom.e[0].out.e; }
EXPORT("dom_busy") uint32_t dom_busy(uint32_t state) { return coaxial_editor_busy(at(state)) ? 1u : 0u; }
EXPORT("dom_inbox") uint32_t dom_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.e; }
EXPORT("dom_inbox_cap") uint32_t dom_inbox_cap(void) { return (uint32_t)coaxial_editor_inbox_cap(); }
EXPORT("dom_event") void dom_event(uint32_t state, int32_t kind, int32_t slot, int32_t length) { coaxial_editor_event(at(state), kind, slot, length); asks(state); }
// A code block's Copy: the bytes for the clipboard (0: none), and their taking.
EXPORT("dom_clip_len") uint32_t dom_clip_len(uint32_t state) { return (uint32_t)coaxial_editor_clip_len(at(state)); }
EXPORT("dom_clip") uint32_t dom_clip(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->clip.e; }
EXPORT("dom_clip_done") void dom_clip_done(uint32_t state) { coaxial_editor_clip_done(at(state)); }
EXPORT("dom_size") void dom_size(uint32_t state, int32_t slot, float w, float h) { coaxial_editor_size(at(state), slot, w, h); }
// The action that waits for the page: its kind (0: none), its bytes, and its taking.
EXPORT("dom_act_kind") int32_t dom_act_kind(uint32_t state) { return (int32_t)coaxial_editor_act_kind(at(state)); }
EXPORT("dom_act") uint32_t dom_act(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->acts.e[0].e; }
EXPORT("dom_act_len") uint32_t dom_act_len(uint32_t state) { return (uint32_t)coaxial_editor_act_len(at(state)); }
EXPORT("dom_act_done") void dom_act_done(uint32_t state) { coaxial_editor_act_done(at(state)); }
