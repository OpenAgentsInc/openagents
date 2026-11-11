// What the Apple host (ui-host.m) adds to the shared contract (../linux/ui-host.h).
#ifndef UI_HOST_APPLE_H
#define UI_HOST_APPLE_H
#include "../linux/ui-host.h"
// The view frames are drawn into: on iOS the app puts it in its window and calls ui_frame from
// its display link (a UIView *); on the Mac it is the window's content view (an NSView *).
// NULL offscreen.
void *ui_view(UiHost *h);

// Rows of the atlas uploaded again after ui_atlas made it: mono and color are the whole images
// (row 0 first) at ui_atlas's sizes, and [y0, y1) and [cy0, cy1) the rows that changed.
void ui_atlas_rows(UiHost *h, const uint8_t *mono, uint32_t y0, uint32_t y1, const uint8_t *color, uint32_t cy0, uint32_t cy1);

// ---- the Mac's (each does nothing on iOS, and nothing offscreen unless it says otherwise)

// A menu bar item. Items with the same menu, one after another, make that menu; a NULL title is a
// separator. key is the key equivalent ("" none) with mods as UiApp.key's; id goes to
// UiApp.command, or selector, when set, is AppKit's own action ("hide:", "terminate:",
// "performMiniaturize:", "orderFrontStandardAboutPanel:"...). A menu titled Edit gets AppKit's
// Dictation and Emoji items; one titled Window is the windows menu; an item titled Services is the
// services menu.
typedef struct { const char *menu, *title, *key; int mods; int id; const char *selector; } UiMenuItem;
void ui_menus(UiHost *h, const UiMenuItem *items, int n);
// A command's key equivalent changed (settings.json's shortcuts).
void ui_menu_key(UiHost *h, int id, const char *key, int mods);
// A menu of items at device pixel x, y; the chosen one's id goes to UiApp.command. checked: an id shown with a mark, or -1.
void ui_popup(UiHost *h, const UiMenuItem *items, int n, float x, float y, int checked);
// The pasteboard (offscreen, a private one of the app's, so a check's Copy never replaces yours).
// Its text into out (NUL-terminated), the byte count; 0 for none.
int ui_paste_text(UiHost *h, char *out, size_t cap);
// Its files and images, each given to UiApp.file (an image as a PNG written to the temporary folder). How many.
int ui_paste_files(UiHost *h);
// The open panel: files (folders 0) or a folder (folders 1), several when many; each chosen path
// goes to UiApp.file with request. A sheet on the window; offscreen, nothing is chosen.
void ui_choose(UiHost *h, int request, int folders, int many, const char *message, const char *prompt, const char *start);
// The window's frame in points (x, y, w, h); 0 offscreen.
int ui_window_frame(UiHost *h, float f[4]);
// Frames presented since the window opened (a drawn frame that was unchanged is not presented).
uint64_t ui_presented(UiHost *h);
// The window closes at the next ui_frame (Quit), which then returns 0.
void ui_quit(UiHost *h);
// A URL opened in its app; a command run in a new Terminal window.
void ui_open_url(const char *url);
void ui_terminal(const char *command);
// Pictures: how many pixels of a differ from b by more than 48 of 255 in a channel, printed as
// "N of M pixels differ by more than 48 (P%, at most D)"; 0 when within limit percent, 1 when not
// or when they cannot be read or their sizes differ. Works without a window.
int ui_compare(const char *a, const char *b, double limit);
#endif
