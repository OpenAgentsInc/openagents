// The shared edit layer's face hook (metaverse/edit/edit.coax; docs/metaverse/realm-edit.md): a realm's C
// face includes this after its generated header and calls, with a view of its RealmEdit:
//   edit_face_drain(e)               in its drain: sends the layer's one request (coaxial_http)
//   edit_face_message(e, kind, text) first in coaxial_message: true when it was the layer's
// A realm whose board the editor shell edits (#13478) defines EDIT_FACE_WORLD before including this, and
// adds one line at file scope:
//   EDIT_FACE_SHELL(ED)              what the editor shell is told (docs/protocol/editor-model-v1.md). ED is
//                                    the face's view of its RealmEdit as an expression of `state`
// Its drain then steps the realm's world and sends the world's one request, and its messages include the
// world's answers and the shell's batches (metaverse/edit/realm-page.coax). A realm that hands the layer its
// messages from its own Coax calls redit_message_shell there, where another calls redit_message. A face that
// does not define it links none of that.
// Everything else (keys, the pointer, drawing, actions) goes through the realm's own Coax hooks.
// It decides nothing.
#ifndef EDIT_FACE_H
#define EDIT_FACE_H
#include <stdbool.h>
#include "face-address.h"

#ifdef EDIT_FACE_WORLD
static inline void edit_face_world(coax_v_t_RealmEdit e);
#endif
static inline void edit_face_drain(coax_v_t_RealmEdit e) {
  coax_t_RealmEdit *x = e.data;
#ifdef EDIT_FACE_WORLD
  edit_face_world(e);
#endif
  if (!x->req_ready) return;
  int32_t id = coaxial_http((cx_face_addr)(uintptr_t)x->req_url.e, (uint32_t)x->req_url_n,
                            (cx_face_addr)(uintptr_t)x->req_body.e, (uint32_t)x->req_body_n, (int32_t)x->req_flags);
  redit_requested(e, id);
}
#ifdef EDIT_FACE_WORLD
// The realm's world: its step, then its one request. It asks only when the visitor may edit and opened the page with ?edit.
static inline void edit_face_world(coax_v_t_RealmEdit e) {
  redit_world_step(e);
  if (!redit_world_wants(e)) return;
  coax_string url = redit_world_url(e), body = redit_world_body(e);
  redit_world_sent(e, coaxial_http((cx_face_addr)(uintptr_t)url.data, (uint32_t)url.count,
                                   (cx_face_addr)(uintptr_t)body.data, (uint32_t)body.count, (int32_t)redit_world_flags(e)));
}
#endif
static inline bool edit_face_message(coax_v_t_RealmEdit e, int32_t kind, coax_string text) {
#ifdef EDIT_FACE_WORLD
  return redit_message_shell(e, kind, text);
#else
  return redit_message(e, kind, text);
#endif
}

// The exports the host reads to open the editor shell over this page and to tell it the realm's world:
// a count that changes when the model does, the model's bytes, and whether this visitor may edit (coaxial_edit_own:
// the realm's server said they are its owner or an admin).
#ifdef EDIT_FACE_WORLD
#define EDIT_FACE_SHELL(ED) \
  __attribute__((export_name("coaxial_edit_rev"))) int32_t coaxial_edit_rev(uint32_t state) { (void)state; return (int32_t)redit_edit_rev(ED); } \
  __attribute__((export_name("coaxial_edit_model"))) uint32_t coaxial_edit_model(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)redit_edit_model(ED).data; } \
  __attribute__((export_name("coaxial_edit_model_len"))) uint32_t coaxial_edit_model_len(uint32_t state) { (void)state; return (uint32_t)redit_edit_model(ED).count; } \
  __attribute__((export_name("coaxial_edit_own"))) uint32_t coaxial_edit_own(uint32_t state) { (void)state; return redit_edit_own(ED) ? 1u : 0u; }
#endif

#endif
