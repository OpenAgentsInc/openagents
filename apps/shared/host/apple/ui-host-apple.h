// What the Apple host (ui-host.m) adds to the shared contract (../linux/ui-host.h).
#ifndef UI_HOST_APPLE_H
#define UI_HOST_APPLE_H
#include "../linux/ui-host.h"
// The view frames are drawn into: on iOS the app puts it in its window and calls ui_frame from
// its display link (a UIView *); on the Mac it is the window's content view (an NSView *).
// NULL offscreen.
void *ui_view(UiHost *h);
#endif
