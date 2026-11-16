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
void og_close(void *gallery);
#endif
