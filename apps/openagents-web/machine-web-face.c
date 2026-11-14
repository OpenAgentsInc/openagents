// Settings, This machine's C face for the browser (#13529): scalar Wasm exports over machine-web.coax, for
// coax/web/dom-host.js, which opens this module in a node of chat-web's Settings (k_module) and draws what it
// builds. The host keeps one MachineWeb, tells it the browser's name and the page's address (event 16) and the world
// (event 13), calls dom_frame each frame it is asked to, and reports presses, typing and framed nodes' sizes. Your
// agent's requests go through the host's dom_stream (events 4 and 5 answer them).
//
// Your computer's wire (metaverse/edit/computer-wire.coax, linked beside this module) moves bytes and decides
// nothing here: this browser's record (IndexedDB, coax/web/STORAGE.md), the relay socket, the browser's random bytes
// and the clock go in (coax/web/dom-wire.js gives them); the pane's ops go to the wire, and the wire's frames and lines
// come back out. The socket opens only while a computer is paired, asked or being forgotten.
#include "machine-web.h"
#include "Computer-wire.h"
// The browser's secure random (crypto.getRandomValues) under the shared Coax entropy policy (coax/lib/crypto/entropy.coax).
#define COAX_ENTROPY_HEADER "Entropy.h"
#include "../../coax/platform/entropy-wasm.c"
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))
// A request to this site (a POST of the body) whose answer comes back in pieces; one at a time.
IMPORT("dom_stream") void dom_stream(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len);
// Asks the host for the browser's name and the page's address as it starts (event 16).
IMPORT("dom_facts") void dom_facts(void);
IMPORT("coaxial_socket_open") int32_t coaxial_socket_open(uint32_t url, int32_t url_len);
IMPORT("coaxial_socket_send") void coaxial_socket_send(int32_t id, uint32_t ptr, int32_t len);
IMPORT("coaxial_socket_close") void coaxial_socket_close(int32_t id);
IMPORT("coaxial_storage_put") int32_t coaxial_storage_put(int32_t slot, int32_t token, uint32_t np, int32_t nn, int32_t version, uint32_t kp, int32_t kn, uint32_t vp, int32_t vn);
IMPORT("coaxial_storage_get") int32_t coaxial_storage_get(int32_t slot, int32_t token, uint32_t np, int32_t nn, int32_t version, uint32_t kp, int32_t kn);
IMPORT("coaxial_now_seconds") int64_t coaxial_now_seconds(void);
IMPORT("coaxial_random") uint32_t coaxial_random(uint32_t out, int32_t length);

#define MW_HEARD 17
#define CF_SLOT 0
#define CF_STORE_KIND (768 + CF_SLOT)
// The record is the one the realm pages kept before (#12642): the same namespace and key, so a browser paired there
// stays paired here.
static const char cf_ns[] = "openagents.computer";
static const char cf_key[] = "browser";
static coax_t_ComputerWire cf_wire;
static int32_t cf_sock = -1, cf_load = 0, cf_token = 1;   // cf_load: 0 not asked, 1 asked, 2 read
static int64_t cf_tried = 0;
static bool cf_fresh_key = false, cf_keep_again = false;   // after Forget, the next Pair makes a new key
static uint8_t cf_frame[24576], cf_lines[16384], cf_rec[4096], cf_pane[2048];

static coax_v_t_MachineWeb at(uint32_t state) { return (coax_v_t_MachineWeb){(coax_t_MachineWeb *)(uintptr_t)state, 1}; }
static coax_t_MachineWeb *raw(uint32_t state) { return (coax_t_MachineWeb *)(uintptr_t)state; }
static coax_v_t_ComputerWire cf_w(void) { return (coax_v_t_ComputerWire){&cf_wire, 1}; }
static coax_v_t_RealmComputer cf_pc(uint32_t state) { return (coax_v_t_RealmComputer){raw(state)->pc.e, 1}; }
static coax_string cf_str(const void *p, int64_t n) { return (coax_string){(const char *)p, n}; }
static void cf_wipe(volatile uint8_t *p, size_t n) { while (n--) *p++ = 0; }
static bool cf_random(uint8_t *out, int32_t n) { return coaxial_random((uint32_t)(uintptr_t)out, n) == 0; }

static void cf_keep(uint32_t state) {
  int64_t pn = rc_keep(cf_pc(state), (coax_v_u8){cf_pane, sizeof cf_pane});
  int64_t n = cw_keep(cf_w(), cf_str(cf_pane, pn), (coax_v_u8){cf_rec, sizeof cf_rec});
  cf_keep_again = coaxial_storage_put(CF_SLOT, cf_token++, (uint32_t)(uintptr_t)cf_ns, sizeof cf_ns - 1, 1, (uint32_t)(uintptr_t)cf_key, sizeof cf_key - 1,
                      (uint32_t)(uintptr_t)cf_rec, (int32_t)n) != 0;   // busy: kept again next frame
  cf_wipe(cf_rec, sizeof cf_rec);
}
static void cf_new_key(void) {
  uint8_t r[32];
  if (cf_random(r, 32)) cw_new_key(cf_w(), (coax_v_u8){r, 32});
  cf_wipe(r, sizeof r);
}
// What the pane names for an op: the pairing request, the thread, or the words.
static coax_string cf_arg(coax_t_RealmComputer *c, int64_t op) {
  if (op == 2 || op == 3) return cf_str(c->request.e, c->request_n);
  if (op == 5 || op == 6) return cf_str(c->thread.e, c->thread_n);
  if (op == 4) return cf_str(c->ask.e, c->ask_n);
  return cf_str("", 0);
}

// The wire, each frame and after each event it hears: the record read, a key made, the socket opened while it is
// wanted, the pane's next op sent, the wire's frames out and its lines back to the pane.
static void drain(uint32_t state) {
  coax_t_MachineWeb *s = raw(state);
  if (!s->told) return;
  coax_t_RealmComputer *c = s->pc.e;
  if (cf_load == 0) {
    cf_load = 1;
    if (coaxial_storage_get(CF_SLOT, cf_token++, (uint32_t)(uintptr_t)cf_ns, sizeof cf_ns - 1, 1, (uint32_t)(uintptr_t)cf_key, sizeof cf_key - 1) != 0) {
      cf_load = 2; rc_restore(cf_pc(state), cf_str("", 0));
    }
  }
  if (cf_load != 2) return;
  if (!cw_has_key(cf_w())) { cf_new_key(); if (cw_has_key(cf_w())) cf_keep(state); }
  if (rc_new_key(cf_pc(state))) cf_fresh_key = true;
  int64_t now = coaxial_now_seconds();
  if (coaxial_machineweb_wants_socket(at(state)) && cf_sock < 0 && cw_wants_socket(cf_w()) && now - cf_tried >= 5) {
    cf_tried = now;
    cw_set_relay(cf_w(), cf_str(c->relay.e, c->relay_n));   // a test relay on this machine, or the default
    coax_string url = cw_socket_url(cf_w());
    cf_sock = coaxial_socket_open((uint32_t)(uintptr_t)url.data, (int32_t)url.count);
  }
  int64_t op = rc_next_op(cf_pc(state));
  if (op != 0) {
    if (op == 1 && cf_fresh_key) { cf_fresh_key = false; cf_new_key(); cf_keep(state); }
    uint8_t r[96];
    if (cf_random(r, 96)) {
      int64_t got = cw_op(cf_w(), op, cf_str(c->host.e, c->host_n), cf_arg(c, op), cf_str(c->realm.e, c->realm_n), c->cursor,
                          cf_str(c->label.e, c->label_n), (coax_v_u8){r, 96}, now);
      // 1 sent, 0 the socket isn't ready (asked again next frame), -1 a bad key, -2 refused here
      if (got == 1 || got == -2) rc_op_sent(cf_pc(state), op, true);
      else if (got == -1) rc_op_sent(cf_pc(state), op, false);
    }
    cf_wipe(r, sizeof r);
  }
  int64_t n;
  while (cf_sock >= 0 && (n = cw_take_frame(cf_w(), (coax_v_u8){cf_frame, sizeof cf_frame})) > 0)
    coaxial_socket_send(cf_sock, (uint32_t)(uintptr_t)cf_frame, (int32_t)n);
  n = cw_take_lines(cf_w(), (coax_v_u8){cf_lines, sizeof cf_lines});
  if (n > 0) { rc_reply(cf_pc(state), cf_str(cf_lines, n)); coaxial_machineweb_event(at(state), MW_HEARD, 0, 0); }
  if (rc_kept(cf_pc(state)) || cw_wants_keep(cf_w()) || cf_keep_again) cf_keep(state);
  // no computer to talk to: the socket closes
  if (cf_sock >= 0 && !coaxial_machineweb_wants_socket(at(state)) && op == 0) { coaxial_socket_close(cf_sock); }
}

// What machine-web.coax asks of the host: the agent's one request.
static void asks(uint32_t state) {
  coax_t_MachineWeb *s = raw(state);
  if (coaxial_machineweb_asking(at(state))) {
    coaxial_machineweb_asked(at(state));
    dom_stream((uint32_t)(uintptr_t)s->door.e, (uint32_t)s->door_n, (uint32_t)(uintptr_t)s->body.e, (uint32_t)s->body_n);
  }
}

// The wire's own events: a record read back (768 + slot) and the relay socket's (256 + 4 * id + event).
static bool wire_event(uint32_t state, int32_t kind, int32_t length) {
  coax_t_MachineWeb *s = raw(state);
  coax_string text = cf_str(s->inbox.e, length < 0 ? 0 : length);
  if (kind == CF_STORE_KIND) {
    if (cf_load != 1 || text.count < 16) return true;
    const uint8_t *h = (const uint8_t *)text.data;
    uint32_t status = h[4] | h[5] << 8 | h[6] << 16 | (uint32_t)h[7] << 24;
    uint32_t len = h[8] | h[9] << 8 | h[10] << 16 | (uint32_t)h[11] << 24;
    coax_string rec = status == 0 && len <= (uint32_t)(text.count - 16) ? cf_str(text.data + 16, len) : cf_str("", 0);
    cf_load = 2;
    cw_restore(cf_w(), rec);
    rc_restore(cf_pc(state), rec);
    cf_wipe((uint8_t *)s->inbox.e, (size_t)text.count);   // the key doesn't stay in the inbox
    return true;
  }
  if (kind >= 768 && kind < 768 + 8) return true;   // another slot's: none
  if (kind >= 256 && kind < 256 + 16) {
    if (cf_sock < 0 || (kind - 256) / 4 != cf_sock) return true;
    int32_t ev = (kind - 256) % 4;
    uint8_t aux[32];
    if (!cf_random(aux, 32)) memset(aux, 0, sizeof aux);
    cw_socket(cf_w(), ev, text, (coax_v_u8){aux, 32}, coaxial_now_seconds());
    cf_wipe(aux, sizeof aux);
    if (ev == 2) cf_sock = -1;
    return true;
  }
  return false;
}

EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_MachineWeb); }
// A new state's declared defaults (coax_default_T, from the compiler). The host calls this first.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_MachineWeb(raw(state)); dom_facts(); }
// One frame: bytes of commands at dom_out to apply, 0 for none, -1 to clear the page and ask again.
EXPORT("dom_frame") int32_t dom_frame(uint32_t state, float dt, uint32_t reduced) {
  drain(state);
  int32_t n = (int32_t)coaxial_machineweb_frame(at(state), dt, reduced != 0);
  asks(state);
  return n;
}
EXPORT("dom_out") uint32_t dom_out(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->dom.e[0].out.e; }
EXPORT("dom_busy") uint32_t dom_busy(uint32_t state) { return coaxial_machineweb_busy(at(state)) ? 1u : 0u; }
EXPORT("dom_inbox") uint32_t dom_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.e; }
EXPORT("dom_inbox_cap") uint32_t dom_inbox_cap(void) { return (uint32_t)coaxial_machineweb_inbox_cap(); }
EXPORT("dom_event") void dom_event(uint32_t state, int32_t kind, int32_t slot, int32_t length) {
  if (wire_event(state, kind, length)) { drain(state); coaxial_machineweb_event(at(state), MW_HEARD, 0, 0); return; }
  coaxial_machineweb_event(at(state), kind, slot, length);
  drain(state);
  asks(state);
}
EXPORT("dom_size") void dom_size(uint32_t state, int32_t slot, float w, float h) { coaxial_machineweb_size(at(state), slot, w, h); }
// Text for the host to copy (Copy beside a command), until it has.
EXPORT("dom_clip") uint32_t dom_clip(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->clip.e; }
EXPORT("dom_clip_len") uint32_t dom_clip_len(uint32_t state) { return (uint32_t)coaxial_machineweb_clip(at(state)); }
EXPORT("dom_clip_done") void dom_clip_done(uint32_t state) { coaxial_machineweb_clip_done(at(state)); }
