#ifndef UI_HOST_STUB_H
#define UI_HOST_STUB_H
#include <stdint.h>
#include "ui-host.h"

// Accessors for headless test assertions on a UiHost returned by cd_view(page).
int ui_stub_last_vertices(UiHost *h);
int ui_stub_max_vertices(UiHost *h);
double ui_stub_last_frame_ms(UiHost *h);
void ui_stub_set_size(UiHost *host, float w, float h, float dpr);
void ui_stub_pointer(UiHost *h, float x, float y, int phase, int clicks, int mods);
void ui_stub_wheel(UiHost *h, float dy);

#endif
