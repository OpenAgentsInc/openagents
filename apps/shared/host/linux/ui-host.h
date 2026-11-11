// Firm reason: 6 (native Wayland and Vulkan ABI; every app's behaviour is Coax).
// The shared Linux host for apps drawn with the component system
// (components/spec.md, docs/apps/2026-11-10-apps-roadmap.md Step 1, GitHub
// #13153): a Wayland window with a Vulkan surface, the seat's keyboard (XKB,
// compose, the text-input protocol and the clipboard) and pointer, and one
// pipeline that draws the components' vertices (28 floats each,
// components/zeron/gpui.coax) with the component shader, emitted from Coax as
// SPIR-V (`coax spirv apps/openagents-web/gpu/chat.coax --vertex chat_fx_vs
// --fragment chat_fx_fs`), over the glyph and icon atlas the app builds in
// Coax (components/raster/atlas.coax).
//
// It is built on what the realms' host already uses: verse/gpu/vkc (the
// device), vks (pipelines from Coax SPIR-V), vkp (the swapchain) and vkw (the
// window and its seat). An app gives it callbacks and nothing else.
#ifndef UI_HOST_H
#define UI_HOST_H
#include <stddef.h>
#include <stdint.h>

typedef struct UiHost UiHost;

typedef struct {
  const char *title, *app_id;
  uint32_t width, height;         // the window's first size, in pixels
  uint32_t max_vertices;          // vertices a frame may hold (28 floats each)
  const char *shader;             // the SPIR-V module's path
  void *user;
  // One frame: fill vertices, return how many. w and h are device pixels.
  int (*frame)(void *user, float *vertices, float w, float h, float dpr, float dt);
  // The left button and the pointer: phase 0 down, 1 moved (held or not), 2 up; clicks 1, 2, 3; mods as key's.
  void (*pointer)(void *user, float x, float y, int phase, int clicks, int mods);
  void (*wheel)(void *user, float dy);   // pixels, down positive
  // A key: a Unicode scalar, or a named key below zero (aethyr/host/wayland-input.coax: -1 Escape, -2 Enter,
  // -3 Backspace, -4..-7 Left Right Up Down, -8 Home, -9 End, -10 Page Up, -11 Page Down, -12 Delete, -15 Tab).
  // mods: 1 shift, 2 control, 4 alt, 8 logo. Return 1 when taken.
  int (*key)(void *user, int key, int mods, int down);
  void (*text)(void *user, const char *bytes, size_t n);   // typed, composed or pasted text
  int (*wants_text)(void *user);                            // a text box has the keyboard
  int (*text_rect)(void *user, float rect[4]);              // where it is, for an input method's window
  // The rest is optional; a host that has none of it leaves it unused (the Linux host, so far).
  // The input method over the text box, in UTF-16 units of its text (AppKit's NSTextInputClient):
  // with it, the host routes marked text and the box's ranges to the app instead of only text.
  const struct UiIme *ime;
  // A menu item, popup item or app command chosen (ui-host-apple.h ui_menus): its id.
  void (*command)(void *user, int id);
  // Whether a command can run now (bit 1) and whether its item is checked (bit 2).
  int (*command_state)(void *user, int id);
  // A file the user gave the app: dropped on the window, pasted, or chosen in the chooser (request 0 for a drop or a paste).
  void (*file)(void *user, int request, const char *path);
  // The window: chrome 1 draws under a transparent title bar (the app draws its own); the smallest
  // size, in points; and where it was (points, x y w h from the screen's lower left), 0 wide for centred.
  int chrome;
  uint32_t min_width, min_height;
  float place[4];
  // A game's input (Everglade's realms): the other buttons (1 the middle, 2 the right) with
  // pointer's phases; every move of the pointer in points, held by ui_hold_pointer or not; the
  // wheel as the platform reports it (points, and 1 for a trackpad's precise deltas) in place of
  // wheel; the window gaining (1) or losing (0) the keyboard; and, with modifier_keys, Shift,
  // Control and Alt as keys of their own (-13, -14, -16), down and up.
  void (*button)(void *user, float x, float y, int phase, int button, int mods);
  void (*motion)(void *user, float dx, float dy);
  void (*scroll)(void *user, float dy, int precise);
  void (*focus)(void *user, int on);
  int modifier_keys;
} UiApp;

// The input method's view of the text box (UTF-16 units). which: 0 selection start, 1 its end,
// 2 marked start (-1 none), 3 marked end, 4 the length. from and to of -1: the selection.
typedef struct UiIme {
  void (*replace)(void *user, int from, int to, const char *bytes, size_t n);
  void (*mark)(void *user, int from, int to, const char *bytes, size_t n, int sel_from, int sel_to);
  void (*unmark)(void *user);
  int (*range)(void *user, int which);
  int (*text)(void *user, int from, int to, char *out, int cap);         // UTF-8 bytes of [from, to)
  int (*rect)(void *user, int at, float rect[4]);                        // device pixels from the view's top left
  int (*index)(void *user, float x, float y);                            // device pixels
} UiIme;

// A window (offscreen 0) or no window at all (offscreen 1: ui_shot only). NULL, with why on stderr, if it cannot open.
UiHost *ui_open(const UiApp *app, int offscreen);
// The atlas: an R8 image and an RGBA8 one (straight alpha). Called again, it uploads them again (a new size makes new images).
void ui_atlas(UiHost *h, const uint8_t *mono, uint32_t w, uint32_t rows, const uint8_t *color, uint32_t cw, uint32_t crows);
// What the compositor sent, one frame, presented. 0 once the window is closed.
int ui_frame(UiHost *h);
// Offscreen: one frame of w by rows pixels, dt seconds after the last, drawn and kept. 0 if it could not.
int ui_shot_frame(UiHost *h, uint32_t w, uint32_t rows, float dt);
// The last offscreen frame as a PNG. 0 if it could not be written.
int ui_shot_write(UiHost *h, const char *png);
// The device scale frames are drawn at (1 unless set).
void ui_set_dpr(UiHost *h, float dpr);
// The pointer's shape: 0 arrow, 1 text, 2 hand, 3 resize left and right, 4 crosshair.
void ui_cursor(UiHost *h, int kind);
// Text to the clipboard. 0 if the seat would not take it.
int ui_copy(UiHost *h, const char *bytes, size_t n);
// The keyboard's modifiers now, as key's mods.
int ui_mods(UiHost *h);
// 1 while the window has the keyboard (0 offscreen).
int ui_focused(UiHost *h);
float ui_dpr(UiHost *h);
void ui_close(UiHost *h);
#endif
