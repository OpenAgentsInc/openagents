// Firm reason: 6 (the ATK/AT-SPI SDK projection of the shared committed-row ABI).
#ifndef OA_UI_ACCESS_H
#define OA_UI_ACCESS_H
#include "ui-host.h"
#include <atk/atk.h>
typedef struct UiAccess UiAccess;
UiAccess *ui_access_create(const UiApp *app);
int ui_access_sync(UiAccess *access);
AtkObject *ui_access_window(UiAccess *access); // borrowed; ATK clients retain their own references
void ui_access_destroy(UiAccess *access);
#endif
