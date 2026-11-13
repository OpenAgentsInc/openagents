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

// A layer of the app's own Metal under the components (Everglade's place, model and realms). With
// one, every frame is drawn (nothing is skipped as unchanged) unless it has changed, below. prepare gets the frame's command
// buffer (id<MTLCommandBuffer>) before its pass, for passes of the app's own; encode gets the
// pass's encoder (id<MTLRenderCommandEncoder>) before the components are drawn. depth 1 gives the
// pass a depth32Float attachment cleared to 1. Offscreen, ui_shot_frame waits for the GPU as before.
typedef struct {
  void *user;
  int depth;
  void (*prepare)(void *user, void *cmd, uint32_t w, uint32_t h, float dpr);
  void (*encode)(void *user, void *enc, uint32_t w, uint32_t h, float dpr);
  // Optional: whether the layer's picture changed since the last frame (1) or not (0). With it,
  // a frame is drawn only when this or the components changed, and ui_frame waits for events as
  // it does without a layer (an app's overlay that is still most of the time: the Metaverse Engine's chat).
  int (*changed)(void *user);
} UiLayer;
void ui_layer(UiHost *h, const UiLayer *layer);
// The components drawn only inside x, y, w by h pixels (an app that gives them part of the window, as
// Everglade's Workshop gives the chat its panel); w or h 0: the whole frame.
void ui_clip(UiHost *h, float x, float y, float w, float rows);
void *ui_device(UiHost *h);   // id<MTLDevice>
void *ui_queue(UiHost *h);    // id<MTLCommandQueue>, the one the host draws with
// Mouselook: the pointer hidden and held where it is; its moves still reach UiApp.motion.
void ui_hold_pointer(UiHost *h, int on);

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
// With no one to see it (offscreen, hidden, or a drive socket listening) no menu opens (#13182).
void ui_popup(UiHost *h, const UiMenuItem *items, int n, float x, float y, int checked);
// The pasteboard (offscreen, a private one of the app's, so a check's Copy never replaces yours).
// Its text into out (NUL-terminated), the byte count; 0 for none.
int ui_paste_text(UiHost *h, char *out, size_t cap);
// Its files and images, each given to UiApp.file (an image as a PNG written to the temporary folder). How many.
int ui_paste_files(UiHost *h);
// The open panel: files (folders 0) or a folder (folders 1), several when many; each chosen path
// goes to UiApp.file with request. A sheet on the window. With no one to see a panel (offscreen, a
// hidden window, or a coax drive socket listening) none opens: the request waits for the driver's
// `choose PATH` (each path, then `choose done` when many) or `cancel` (GitHub #13182). 1 when a
// panel opened or the request waits; 0 when there is nothing to choose with (no UiApp.file).
int ui_choose(UiHost *h, int request, int folders, int many, const char *message, const char *prompt, const char *start);
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

// A native view of the app's own in the window (an NSView *: a WKWebView, a Metal view, a text
// view). above 0 puts it under the components: the host's layer is then clear where nothing is
// drawn, and a pointer event UiApp.hit leaves goes to the views under it. above 1 puts it over
// them. Its frame and autoresizing are the app's, in ui_view's coordinates. Nothing offscreen.
void ui_child(UiHost *h, void *view, int above);
void *ui_window(UiHost *h);   // the NSWindow *; NULL offscreen
// A menu item's title now (an item made by ui_menus, by its id).
void ui_menu_title(UiHost *h, int id, const char *title);
// UiApp.life's events.
enum { UI_LIFE_ACTIVE, UI_LIFE_INACTIVE, UI_LIFE_HIDDEN, UI_LIFE_SHOWN, UI_LIFE_MINIMIZED, UI_LIFE_RESTORED, UI_LIFE_OCCLUSION, UI_LIFE_QUIT };
#endif
