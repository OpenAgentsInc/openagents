// Firm reason: 6 (opaque SDK lifetime boundary; no generated types cross Swift).
// The app set's gallery as a native surface (host.c; components/app/INTEGRATION.md, GitHub
// #13645): the compiled gallery page (gallery.coax) on the shared component host, in its own
// Metal view. The iOS dev menu (apps/openagents-ios/DevGallery.swift) shows it. Rectangles are in
// points from the view's top left.
#ifndef OPENAGENTS_APP_GALLERY_HOST_H
#define OPENAGENTS_APP_GALLERY_HOST_H
// resources: the folder holding the component shader, fonts and icons. NULL when it cannot open.
void *og_open(const char *resources);
void *og_view(void *gallery);
// One frame. Call it while og_moving says 1, and once after anything changes.
void og_frame(void *gallery);
int og_moving(void *gallery);
// Called when a touch on the surface changed something.
void og_on_change(void *gallery, void (*changed)(void *context), void *context);
// The window's safe area in points, and reduced motion.
void og_env(void *gallery, float safe_top, float safe_bottom, int reduced);
// What the gallery asks, once: 0 nothing, 1 close the dev menu.
int og_action(void *gallery);
// Frames the surface refused to commit: 0 in a healthy page.
int og_refused(void *gallery);
// The committed semantic rows, for VoiceOver (apps/shared/native/KitAccess.swift), as the chat page's
// (apps/openagents-ios/host.h oc_rows): how many, a number that changes when any does, row i (its
// visible box, role, state, parent, focus place; its id and what a screen reader speaks), its value,
// a press by id, and VoiceOver's scroll a page (dir 1 down, -1 up).
int og_rows(void *gallery);
int og_rows_stamp(void *gallery);
int og_row(void *gallery, int i, float row[8], char *id, int id_cap, char *label, int label_cap);
int og_row_value(void *gallery, int i, char *out, int cap);
int og_activate(void *gallery, const char *id);
int og_scroll_page(void *gallery, int dir);
// Scrolls the gallery so the section with this row id is at the top (the dev menu's -OADevGallerySection).
int og_reveal(void *gallery, const char *id);
void og_close(void *gallery);
#endif
