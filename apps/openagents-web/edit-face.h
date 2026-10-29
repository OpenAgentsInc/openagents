// The shared edit layer's face hook (verse/edit/edit.coax; docs/verse/realm-edit.md): a realm's C
// face includes this after its generated header and calls, with a view of its RealmEdit:
//   edit_face_drain(e)               in its drain: sends the layer's one request (coaxial_http)
//   edit_face_message(e, kind, text) first in coaxial_message: true when it was the layer's
// Everything else (keys, the pointer, drawing, actions) goes through the realm's own Coax hooks.
// It decides nothing.
#ifndef EDIT_FACE_H
#define EDIT_FACE_H
#include <stdbool.h>
#include <stdint.h>

static inline void edit_face_drain(coax_v_t_RealmEdit e) {
  coax_t_RealmEdit *x = e.data;
  if (!x->req_ready) return;
  int32_t id = coaxial_http((uint32_t)(uintptr_t)x->req_url.e, (uint32_t)x->req_url_n,
                            (uint32_t)(uintptr_t)x->req_body.e, (uint32_t)x->req_body_n, (int32_t)x->req_flags);
  redit_requested(e, id);
}
static inline bool edit_face_message(coax_v_t_RealmEdit e, int32_t kind, coax_string text) {
  return redit_message(e, kind, text);
}

#endif
