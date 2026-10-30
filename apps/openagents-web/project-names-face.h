// The project page's NIP-19 names, put in its buffers by the face before a
// frame: the address and key once the page knows its key, and the open
// task's nevent (relay hint, the project as author, kind 1621) once a task is
// open. project-names.coax encodes them, its own library (wasm.sh's LIBRARY
// argument), as nip01-verify.coax is /nip/01's. Included by project-face.c
// and by coax/web/project/nip19.test.ts, which checks the drawn names.
#include "Project-names.h"   // wasm.sh writes it for the LIBRARY argument
static void project_names(coax_t_ProjectState *page, coax_t_Work *work) {
  if (!work->named && work->key.count == 64) {
    work->naddr_n = name_naddr(30621, work->key, work->slug, work->relay, (coax_v_u8){work->naddr_buf.e, (int64_t)sizeof work->naddr_buf.e});
    work->npub_n = name_npub(work->key, (coax_v_u8){work->npub_buf.e, (int64_t)sizeof work->npub_buf.e});
    work->named = 1;
  }
  if (page->open > 0 && page->nevent_for != page->open && page->event_n == 64) {
    coax_string id = {(const char *)page->event.e, page->event_n};
    page->nevent_n = name_nevent(id, work->relay, work->key, 1621, (coax_v_u8){page->nevent.e, (int64_t)sizeof page->nevent.e});
    page->nevent_for = page->open;
  }
}
