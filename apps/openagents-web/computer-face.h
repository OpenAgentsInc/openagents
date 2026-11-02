// YOUR COMPUTER's wire for a realm's C face (openagents#12642; verse/edit/computer.coax is the pane,
// verse/edit/computer-wire.coax the wire). A face that links the wire unit includes this after its
// generated header and edit-face.h, and calls:
//   computer_face_drain(e)                in its drain, after edit_face_drain
//   computer_face_message(e, kind, text)  first in coaxial_message: true when it was the wire's
// It moves bytes and decides nothing: this browser's record (IndexedDB, coax/web/STORAGE.md), the
// relay socket, the browser's random bytes and the clock go in; the pane's ops go to the wire, and
// the wire's frames and lines come back out. The socket opens only for someone who may edit the
// realm, once the pane is open or this browser is paired.
#ifndef COMPUTER_FACE_H
#define COMPUTER_FACE_H
#include "Computer-wire.h"
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#ifndef COMPUTER_IMPORT
#define COMPUTER_IMPORT(name) __attribute__((import_module("env"), import_name(name)))
#endif
COMPUTER_IMPORT("coaxial_socket_open") int32_t coaxial_socket_open(uint32_t url, int32_t url_len);
COMPUTER_IMPORT("coaxial_socket_send") void coaxial_socket_send(int32_t id, uint32_t ptr, int32_t len);
COMPUTER_IMPORT("coaxial_storage_put") int32_t coaxial_storage_put(int32_t slot, int32_t token, uint32_t np, int32_t nn, int32_t version, uint32_t kp, int32_t kn, uint32_t vp, int32_t vn);
COMPUTER_IMPORT("coaxial_storage_get") int32_t coaxial_storage_get(int32_t slot, int32_t token, uint32_t np, int32_t nn, int32_t version, uint32_t kp, int32_t kn);
COMPUTER_IMPORT("coaxial_now_seconds") int64_t coaxial_now_seconds(void);
COMPUTER_IMPORT("coaxial_random") uint32_t coaxial_random(uint32_t out, int32_t length);

#define CF_SLOT 7
#define CF_STORE_KIND (768 + CF_SLOT)
static const char cf_ns[] = "openagents.computer";
static const char cf_key[] = "browser";
static coax_t_ComputerWire cf_wire;
static int32_t cf_sock = -1, cf_load = 0, cf_token = 1;   // cf_load: 0 not asked, 1 asked, 2 read
static int64_t cf_tried = 0;
static bool cf_fresh_key = false, cf_keep_again = false;                          // after FORGET, the next PAIR makes a new key
static uint8_t cf_frame[24576], cf_lines[16384], cf_rec[4096], cf_pane[2048];

static coax_v_t_ComputerWire cf_w(void) { return (coax_v_t_ComputerWire){&cf_wire, 1}; }
static coax_v_t_RealmComputer cf_pc(coax_v_t_RealmEdit e) { return (coax_v_t_RealmComputer){e.data->pc.e, 1}; }
static coax_string cf_str(const void *p, int64_t n) { return (coax_string){(const char *)p, n}; }
static void cf_wipe(volatile uint8_t *p, size_t n) { while (n--) *p++ = 0; }
static bool cf_random(uint8_t *out, int32_t n) { return coaxial_random((uint32_t)(uintptr_t)out, n) == 0; }

static void cf_keep(coax_v_t_RealmEdit e) {
  int64_t pn = rc_keep(cf_pc(e), (coax_v_u8){cf_pane, sizeof cf_pane});
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

static inline void computer_face_drain(coax_v_t_RealmEdit e) {
  coax_t_RealmEdit *x = e.data;
  if (!x->computer || x->role < 2) return;   // REDIT_ADMIN: owners and admins only
  coax_t_RealmComputer *c = x->pc.e;
  if (cf_load == 0) {
    cf_load = 1;
    if (coaxial_storage_get(CF_SLOT, cf_token++, (uint32_t)(uintptr_t)cf_ns, sizeof cf_ns - 1, 1, (uint32_t)(uintptr_t)cf_key, sizeof cf_key - 1) != 0) {
      cf_load = 2; rc_restore(cf_pc(e), cf_str("", 0));
    }
  }
  if (cf_load != 2) return;
  if (!cw_has_key(cf_w())) { cf_new_key(); if (cw_has_key(cf_w())) cf_keep(e); }
  if (rc_new_key(cf_pc(e))) cf_fresh_key = true;
  int64_t now = coaxial_now_seconds();
  bool wanted = x->pane == 6 || c->pair != 0;   // REDIT_COMPUTER_PANE, or paired (RP_UNPAIRED is 0)
  if (wanted && cf_sock < 0 && cw_wants_socket(cf_w()) && now - cf_tried >= 5) {
    cf_tried = now;
    cw_set_relay(cf_w(), cf_str(c->relay.e, c->relay_n));   // a test relay on this machine, or the default
    coax_string url = cw_socket_url(cf_w());
    cf_sock = coaxial_socket_open((uint32_t)(uintptr_t)url.data, (int32_t)url.count);
  }
  int64_t op = rc_next_op(cf_pc(e));
  if (op != 0) {
    if (op == 1 && cf_fresh_key) { cf_fresh_key = false; cf_new_key(); cf_keep(e); }
    uint8_t r[96];
    if (cf_random(r, 96)) {
      int64_t got = cw_op(cf_w(), op, cf_str(c->host.e, c->host_n), cf_arg(c, op), cf_str(c->realm.e, c->realm_n), c->cursor,
                          cf_str(c->label.e, c->label_n), (coax_v_u8){r, 96}, now);
      // 1 sent, 0 the socket isn't ready (asked again next frame), -1 a bad key, -2 refused here
      if (got == 1 || got == -2) rc_op_sent(cf_pc(e), op, true);
      else if (got == -1) rc_op_sent(cf_pc(e), op, false);
    }
    cf_wipe(r, sizeof r);
  }
  int64_t n;
  while (cf_sock >= 0 && (n = cw_take_frame(cf_w(), (coax_v_u8){cf_frame, sizeof cf_frame})) > 0)
    coaxial_socket_send(cf_sock, (uint32_t)(uintptr_t)cf_frame, (int32_t)n);
  n = cw_take_lines(cf_w(), (coax_v_u8){cf_lines, sizeof cf_lines});
  if (n > 0) rc_reply(cf_pc(e), cf_str(cf_lines, n));
  if (rc_kept(cf_pc(e)) || cw_wants_keep(cf_w()) || cf_keep_again) cf_keep(e);
}

static inline bool computer_face_message(coax_v_t_RealmEdit e, int32_t kind, coax_string text) {
  if (kind == CF_STORE_KIND) {
    if (cf_load != 1 || text.count < 16) return true;
    const uint8_t *h = (const uint8_t *)text.data;
    uint32_t status = h[4] | h[5] << 8 | h[6] << 16 | (uint32_t)h[7] << 24;
    uint32_t len = h[8] | h[9] << 8 | h[10] << 16 | (uint32_t)h[11] << 24;
    coax_string rec = status == 0 && len <= (uint32_t)(text.count - 16) ? cf_str(text.data + 16, len) : cf_str("", 0);
    cf_load = 2;
    cw_restore(cf_w(), rec);
    rc_restore(cf_pc(e), rec);
    return true;
  }
  if (kind >= 256 && kind < 256 + 16 && cf_sock >= 0 && (kind - 256) / 4 == cf_sock) {
    int32_t ev = (kind - 256) % 4;
    uint8_t aux[32];
    if (!cf_random(aux, 32)) memset(aux, 0, sizeof aux);
    cw_socket(cf_w(), ev, text, (coax_v_u8){aux, 32}, coaxial_now_seconds());
    cf_wipe(aux, sizeof aux);
    if (ev == 2) cf_sock = -1;
    computer_face_drain(e);
    return true;
  }
  return false;
}

#endif
