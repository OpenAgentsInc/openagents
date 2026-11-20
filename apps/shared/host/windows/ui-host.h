// Firm reason: 6 (native Win32 and Vulkan ABI; every app's behaviour is Coax).
// The shared Windows host for apps drawn with the component system (GitHub #13782): the same
// contract as the Linux host (../linux/ui-host.h, the hosts' one C face), served by ui-host.c on
// a Win32 window with a Vulkan surface (vulkan-1.dll, VK_KHR_win32_surface) and the drawing the
// Vulkan hosts share (../vulkan-ui.c). Its keyboard, pointer, wheel, input method, clipboard,
// dropped files and screen scale are Windows': Control is the primary modifier there, so a chord
// with it comes to the app as Command (8), as the page reads chords; Alt is Option (4).
#ifndef UI_HOST_WINDOWS_H
#define UI_HOST_WINDOWS_H
#include "../linux/ui-host.h"

// The clipboard's text as UTF-8 (at most cap bytes, not NUL-ended): its length, 0 when there is none.
int ui_paste_text(UiHost *h, char *out, size_t cap);
// Files on the clipboard (copied in Explorer), each handed to the app's file callback (request 0): how many.
int ui_paste_files(UiHost *h);
// The window (an HWND), for a dialog's owner; NULL offscreen.
void *ui_window(UiHost *h);
#endif
