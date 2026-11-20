#ifndef UI_HOST_STUB_H
#define UI_HOST_STUB_H
#include <stddef.h>
#include <stdint.h>
#include "ui-host.h"

// The Mac-only calls host.c makes (apps/shared/host/apple/ui-host-apple.h), which the stub
// implements and Linux's ui-host.h does not declare. build.sh includes this header first in
// carrier.c: undeclared, ui_view returned an int, and the truncated pointer crashed on Linux (#13734).
#ifndef __APPLE__
void *ui_view(UiHost *h);
int ui_paste_text(UiHost *h, char *out, size_t cap);
#endif

// Accessors for headless test assertions on a UiHost returned by cd_view(page).
int ui_stub_last_vertices(UiHost *h);
int ui_stub_max_vertices(UiHost *h);
double ui_stub_last_frame_ms(UiHost *h);
void ui_stub_set_size(UiHost *host, float w, float h, float dpr);
void ui_stub_pointer(UiHost *h, float x, float y, int phase, int clicks, int mods);
void ui_stub_wheel(UiHost *h, float dy);

#endif
