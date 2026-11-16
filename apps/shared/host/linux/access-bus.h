// Firm reason: 6 (AT-SPI application registration and GLib scheduling effects).
#ifndef OA_UI_ACCESS_BUS_H
#define OA_UI_ACCESS_BUS_H
#include "access.h"
// Call on the UI thread. The host retains only the committed ATK objects.
int ui_access_bus_start(UiAccess *access);
void ui_access_bus_pump(void);
void ui_access_bus_stop(void);
#endif
