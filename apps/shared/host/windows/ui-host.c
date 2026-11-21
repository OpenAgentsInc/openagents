// Firm reason: 6 (native Win32 and Vulkan ABI; every app's behaviour is Coax).
// The shared Windows host (ui-host.h, GitHub #13782): a Win32 window with a Vulkan surface
// (VK_KHR_win32_surface, vulkan-1.dll loaded by vk-load.c) and the Vulkan hosts' drawing
// (../vulkan-ui.c, the same pipeline, atlas and composition as Linux's). It holds no behaviour:
// it turns the window's messages into the app's callbacks.
//
// The window is per-monitor DPI aware (the app's manifest and SetProcessDpiAwarenessContext), so
// its pixels are the screen's and the scale is the monitor's DPI over 96. Keys: a named key
// (Esc, Enter, arrows...) or a chord with Control comes as key, with Control reported as Command
// (8), the page's primary modifier; typed text comes from WM_CHAR, and an input method's marked
// text through the app's UiIme when it has one (else the system's own composition window and
// WM_CHAR). The clipboard is CF_UNICODETEXT and CF_HDROP; files dropped on the window come as
// the app's file callback. Dragging files over the window does not show "Drop to attach" yet
// (that needs an OLE drop target); the drop itself works.
#define WIN32_LEAN_AND_MEAN
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif
#include "ui-host.h"
#include "../drive.h"
#include "vk-load.h"
#include "vkc.h"
#include "vks.h"
#include "vkp.h"
#include <windows.h>
#include <windowsx.h>
#include <imm.h>
#include <shellapi.h>
#include <vulkan/vulkan_win32.h>
#include <math.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define UI_VERTEX_BYTES 112   /* 28 floats: components/zeron/gpui.coax GP_VERTEX_FLOATS */
#define UI_CLASS L"OpenAgentsUiHost"

typedef struct {VksPipe pipe;VksPass pass;VksBinding binds[4];int target,source,clear;} UiCompPipe;
struct UiHost {
  UiDrive drive;   // coax drive (../drive.c; no socket on Windows yet)
  UiApp app;
  int offscreen;
  Vkc v;
  Vkp p;
  // the drawing (../vulkan-ui.c)
  uint32_t *words; size_t nwords;
  VkcBuffer verts, uniform;
  VksImage mono, color, target;
  int has_atlas, has_target, has_pipe;
  UiComposition composition;int composed;
  VksImage compose_targets[16],images[16];int ncompose_targets;
  uint64_t image_generation[16];
  UiCompPipe cp[64];int ncp;uint64_t compose_signature;uint32_t image_mask;uint32_t *comp_words;size_t comp_nwords;
  VksPipe pipe;
  VksPass pass;
  VksBinding binds[3];
  uint32_t w, rows;
  float dpr;
  double last;
  uint64_t shown; int stale;
  float *frame;
  double down_at; float down_x, down_y; int clicks, held;
  // the window
  HWND hwnd;
  VkSurfaceKHR surface;
  uint32_t want_w, want_h;   // the client area, in pixels
  int closed, focused, cursor_kind, sizing, drawing;
  WCHAR high;                // a UTF-16 high surrogate waiting for its low half (WM_CHAR)
  int delivered[256];        // the key each virtual key gave on its way down, for its way up
};

static double ui_now(void) {
  static LARGE_INTEGER hz;
  LARGE_INTEGER t;
  if (!hz.QuadPart) QueryPerformanceFrequency(&hz);
  QueryPerformanceCounter(&t);
  return (double)t.QuadPart / (double)hz.QuadPart;
}
// No swapchain image: a short sleep. An unchanged frame: the window's messages, a frame's time at most.
static void ui_host_nap(UiHost *h) { (void)h; Sleep(2); }
static void ui_host_idle(UiHost *h) { (void)h; MsgWaitForMultipleObjectsEx(0, NULL, 16, QS_ALLINPUT, MWMO_INPUTAVAILABLE); }
#include "../vulkan-ui.c"

// ---- text

static int ui_utf8(const WCHAR *w, int n, char *out, int cap) {
  int k = WideCharToMultiByte(CP_UTF8, 0, w, n, out, cap, NULL, NULL);
  return k < 0 ? 0 : k;
}
// A path the system gave, with forward slashes (the desktop core's form).
static void ui_forward(char *s) { for (; *s; s++) if (*s == '\\') *s = '/'; }

// ---- the keyboard

// The modifiers as the app reads them: 1 Shift, 8 Control (the primary modifier, Command's
// place), 4 Alt. Control and Alt together are AltGr, which types characters: neither is reported.
int ui_mods(UiHost *h) {
  if (h->offscreen) return 0;
  int shift = GetKeyState(VK_SHIFT) < 0, control = GetKeyState(VK_CONTROL) < 0, alt = GetKeyState(VK_MENU) < 0;
  return (shift ? 1 : 0) | (control && !alt ? 8 : 0) | (alt && !control ? 4 : 0);
}
int ui_focused(UiHost *h) { return h->offscreen ? 0 : h->focused; }
// A virtual key's named key (ui-host.h: -1 Escape, -2 Enter, ...), 0 for any other.
static int ui_named(UiHost *h, WPARAM vk) {
  switch (vk) {
    case VK_ESCAPE: return -1; case VK_RETURN: return -2; case VK_BACK: return -3;
    case VK_LEFT: return -4; case VK_RIGHT: return -5; case VK_UP: return -6; case VK_DOWN: return -7;
    case VK_HOME: return -8; case VK_END: return -9; case VK_PRIOR: return -10; case VK_NEXT: return -11;
    case VK_DELETE: return -12; case VK_TAB: return -15;
    case VK_SHIFT: return h->app.modifier_keys ? -13 : 0;
    case VK_CONTROL: return h->app.modifier_keys ? -14 : 0;
    case VK_MENU: return h->app.modifier_keys ? -16 : 0;
    default: return 0;
  }
}
static int ui_key_down(UiHost *h, WPARAM vk) {
  if (!h->app.key || vk > 255) return 0;
  int mods = ui_mods(h), key = ui_named(h, vk);
  // a chord with Control: the key's own character, unshifted and lower case ("c" for Control-C)
  if (!key && (mods & 8)) {
    UINT ch = MapVirtualKeyW((UINT)vk, MAPVK_VK_TO_CHAR) & 0x7fffffff;
    if (ch >= 'A' && ch <= 'Z') ch += 32;
    if (ch >= 32) key = (int)ch;
  }
  if (!key) return 0;
  int took = h->app.key(h->app.user, key, mods, 1);
  h->delivered[vk] = key;
  return took;
}
static void ui_key_up(UiHost *h, WPARAM vk) {
  if (vk > 255 || !h->delivered[vk]) return;
  int key = h->delivered[vk];
  h->delivered[vk] = 0;
  if (h->app.key) h->app.key(h->app.user, key, ui_mods(h), 0);
}
// A typed character (WM_CHAR, UTF-16): text for the field with the keyboard, else a key.
static void ui_char(UiHost *h, WCHAR c) {
  if (IS_HIGH_SURROGATE(c)) { h->high = c; return; }
  WCHAR pair[2] = {c, 0}; int n = 1;
  if (IS_LOW_SURROGATE(c)) { if (!h->high) return; pair[0] = h->high; pair[1] = c; n = 2; }
  h->high = 0;
  if (c < 32 || c == 127) return;   // Enter, Tab, Backspace and Esc came as named keys
  if (ui_mods(h) & 8) return;       // a chord's character (Control-Space) came as its key
  char bytes[8]; int k = ui_utf8(pair, n, bytes, sizeof bytes);
  if (k <= 0) return;
  // the field with the keyboard takes it as typed text: through text, else as the input method's
  // insert at the caret (-1), as the Mac host's insertText does
  if (h->app.wants_text && h->app.wants_text(h->app.user)) {
    if (h->app.text) { h->app.text(h->app.user, bytes, (size_t)k); return; }
    if (h->app.ime) { h->app.ime->replace(h->app.user, -1, -1, bytes, (size_t)k); return; }
  }
  if (h->app.key) {
    uint32_t scalar = n == 2 ? 0x10000u + (((uint32_t)pair[0] - 0xD800u) << 10) + ((uint32_t)pair[1] - 0xDC00u) : c;
    h->app.key(h->app.user, (int)scalar, ui_mods(h), 1);
  }
}

// ---- the input method (the app's UiIme: UTF-16 units of the field, device pixels)

static int ui_ime_on(UiHost *h) { return h->app.ime && h->app.wants_text && h->app.wants_text(h->app.user); }
// The candidate window under the caret.
static void ui_ime_place(UiHost *h, HIMC imc) {
  float r[4] = {0};
  int at = h->app.ime->range(h->app.user, 0);
  if (!h->app.ime->rect(h->app.user, at, r)) return;
  COMPOSITIONFORM cf = {.dwStyle = CFS_POINT, .ptCurrentPos = {(LONG)r[0], (LONG)r[1]}};
  ImmSetCompositionWindow(imc, &cf);
  CANDIDATEFORM cand = {.dwIndex = 0, .dwStyle = CFS_EXCLUDE, .ptCurrentPos = {(LONG)r[0], (LONG)(r[1] + r[3])},
                        .rcArea = {(LONG)r[0], (LONG)r[1], (LONG)(r[0] + r[2] + 1), (LONG)(r[1] + r[3])}};
  ImmSetCandidateWindow(imc, &cand);
}
// One of the composition's strings as UTF-8 (a malloc'd buffer, NUL-ended), with its UTF-16 length.
static char *ui_ime_string(HIMC imc, DWORD which, int *units, int *bytes) {
  LONG size = ImmGetCompositionStringW(imc, which, NULL, 0);
  *units = 0; *bytes = 0;
  if (size < 0) return NULL;
  WCHAR *w = malloc((size_t)size + sizeof(WCHAR));
  if (!w) return NULL;
  ImmGetCompositionStringW(imc, which, w, (DWORD)size);
  *units = (int)(size / (LONG)sizeof(WCHAR));
  char *s = malloc((size_t)*units * 3 + 1);
  if (s) { *bytes = ui_utf8(w, *units, s, *units * 3); s[*bytes] = 0; }
  free(w);
  return s;
}
static void ui_ime_composition(UiHost *h, LPARAM what) {
  const UiIme *ime = h->app.ime;
  HIMC imc = ImmGetContext(h->hwnd);
  if (!imc) return;
  int units = 0, bytes = 0;
  if (what & GCS_RESULTSTR) {
    char *s = ui_ime_string(imc, GCS_RESULTSTR, &units, &bytes);
    int ms = ime->range(h->app.user, 2), me = ime->range(h->app.user, 3);
    if (s) ime->replace(h->app.user, ms >= 0 ? ms : -1, ms >= 0 ? me : -1, s, (size_t)bytes);
    free(s);
  }
  if (what & GCS_COMPSTR) {
    char *s = ui_ime_string(imc, GCS_COMPSTR, &units, &bytes);
    int ms = ime->range(h->app.user, 2), me = ime->range(h->app.user, 3);
    if (units == 0) {
      if (ms >= 0) { ime->mark(h->app.user, ms, me, "", 0, 0, 0); ime->unmark(h->app.user); }
    } else if (s) {
      LONG cursor = (what & GCS_CURSORPOS) ? ImmGetCompositionStringW(imc, GCS_CURSORPOS, NULL, 0) : units;
      if (cursor < 0 || cursor > units) cursor = units;
      ime->mark(h->app.user, ms >= 0 ? ms : -1, ms >= 0 ? me : -1, s, (size_t)bytes, (int)cursor, (int)cursor);
    }
    free(s);
  }
  ui_ime_place(h, imc);
  ImmReleaseContext(h->hwnd, imc);
}

// ---- the clipboard and files

int ui_copy(UiHost *h, const char *bytes, size_t n) {
  if (h->offscreen || !OpenClipboard(h->hwnd)) return 0;
  int units = MultiByteToWideChar(CP_UTF8, 0, bytes, (int)n, NULL, 0);
  HGLOBAL mem = GlobalAlloc(GMEM_MOVEABLE, ((size_t)units + 1) * sizeof(WCHAR));
  int ok = 0;
  if (mem) {
    WCHAR *w = GlobalLock(mem);
    MultiByteToWideChar(CP_UTF8, 0, bytes, (int)n, w, units);
    w[units] = 0;
    GlobalUnlock(mem);
    EmptyClipboard();
    ok = SetClipboardData(CF_UNICODETEXT, mem) != NULL;
    if (!ok) GlobalFree(mem);
  }
  CloseClipboard();
  return ok;
}
int ui_paste_text(UiHost *h, char *out, size_t cap) {
  if (h->offscreen || !cap || !OpenClipboard(h->hwnd)) return 0;
  int n = 0;
  HANDLE mem = GetClipboardData(CF_UNICODETEXT);
  const WCHAR *w = mem ? GlobalLock(mem) : NULL;
  if (w) {
    // Windows ends lines with CR LF; the app's text has LF
    size_t units = wcslen(w);
    WCHAR *plain = malloc((units + 1) * sizeof *plain);
    if (plain) {
      size_t k = 0;
      for (size_t i = 0; i < units; i++) if (!(w[i] == L'\r' && w[i + 1] == L'\n')) plain[k++] = w[i];
      n = ui_utf8(plain, (int)k, out, (int)cap);
      free(plain);
    }
    GlobalUnlock(mem);
  }
  CloseClipboard();
  return n;
}
// Each file in a drop or a clipboard's file list, to the app.
static int ui_files(UiHost *h, HDROP drop) {
  UINT count = DragQueryFileW(drop, 0xFFFFFFFF, NULL, 0);
  for (UINT i = 0; i < count && h->app.file; i++) {
    WCHAR w[32768];
    if (!DragQueryFileW(drop, i, w, 32768)) continue;
    char path[32768 * 3];
    int n = ui_utf8(w, -1, path, sizeof path);
    if (n <= 0) continue;
    ui_forward(path);
    h->app.file(h->app.user, 0, path);
  }
  return (int)count;
}
int ui_paste_files(UiHost *h) {
  if (h->offscreen || !h->app.file || !IsClipboardFormatAvailable(CF_HDROP) || !OpenClipboard(h->hwnd)) return 0;
  HANDLE drop = GetClipboardData(CF_HDROP);
  int n = drop ? ui_files(h, (HDROP)drop) : 0;
  CloseClipboard();
  return n;
}
void *ui_window(UiHost *h) { return h->offscreen ? NULL : h->hwnd; }

// ---- the pointer

void ui_cursor(UiHost *h, int kind) {
  if (kind == h->cursor_kind) return;
  h->cursor_kind = kind;
  POINT at; RECT client;
  if (!h->offscreen && GetCursorPos(&at) && ScreenToClient(h->hwnd, &at) && GetClientRect(h->hwnd, &client) && PtInRect(&client, at))
    SendMessageW(h->hwnd, WM_SETCURSOR, (WPARAM)h->hwnd, MAKELPARAM(HTCLIENT, WM_MOUSEMOVE));
}
static void ui_cursor_set(UiHost *h) {
  static const LPCWSTR shapes[5] = {IDC_ARROW, IDC_IBEAM, IDC_HAND, IDC_SIZEWE, IDC_CROSS};
  int kind = h->cursor_kind < 0 || h->cursor_kind > 4 ? 0 : h->cursor_kind;
  SetCursor(LoadCursorW(NULL, shapes[kind]));
}
static void ui_button(UiHost *h, LPARAM at, int phase, int button) {
  if (h->app.button) h->app.button(h->app.user, (float)GET_X_LPARAM(at), (float)GET_Y_LPARAM(at), phase, button, ui_mods(h));
}

// ---- the window

static int ui_draw(UiHost *h);
static UINT ui_dpi_of(HWND hwnd) {
  typedef UINT (WINAPI *DpiForWindow)(HWND);
  static DpiForWindow get;
  if (!get) get = (DpiForWindow)(void (*)(void))GetProcAddress(GetModuleHandleW(L"user32.dll"), "GetDpiForWindow");
  UINT dpi = get && hwnd ? get(hwnd) : 96;
  return dpi ? dpi : 96;
}
static LRESULT CALLBACK ui_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
  if (msg == WM_NCCREATE) SetWindowLongPtrW(hwnd, GWLP_USERDATA, (LONG_PTR)((CREATESTRUCTW *)lp)->lpCreateParams);
  UiHost *h = (UiHost *)GetWindowLongPtrW(hwnd, GWLP_USERDATA);
  if (!h) return DefWindowProcW(hwnd, msg, wp, lp);
  switch (msg) {
    case WM_CLOSE: h->closed = 1; return 0;
    case WM_SIZE: h->want_w = LOWORD(lp); h->want_h = HIWORD(lp); h->stale = 1; return 0;
    case WM_ERASEBKGND: return 1;
    case WM_PAINT: { PAINTSTRUCT ps; BeginPaint(hwnd, &ps); EndPaint(hwnd, &ps); h->stale = 1; return 0; }
    // Windows runs its own loop while the window is moved or sized: a timer draws in it, so the
    // page follows the new size instead of freezing until the button is let go.
    case WM_ENTERSIZEMOVE: h->sizing = 1; SetTimer(hwnd, 1, 16, NULL); return 0;
    case WM_EXITSIZEMOVE: h->sizing = 0; KillTimer(hwnd, 1); return 0;
    case WM_TIMER: if (wp == 1 && h->sizing) ui_draw(h); return 0;
    case WM_DPICHANGED: {
      h->dpr = HIWORD(wp) / 96.0f;
      const RECT *r = (const RECT *)lp;
      SetWindowPos(hwnd, NULL, r->left, r->top, r->right - r->left, r->bottom - r->top, SWP_NOZORDER | SWP_NOACTIVATE);
      h->stale = 1;
      if (h->app.changed) h->app.changed(h->app.user);
      return 0;
    }
    case WM_GETMINMAXINFO: {
      MINMAXINFO *mm = (MINMAXINFO *)lp;
      if (h->app.min_width && h->app.min_height) {
        RECT r = {0, 0, (LONG)(h->app.min_width * h->dpr), (LONG)(h->app.min_height * h->dpr)};
        AdjustWindowRectEx(&r, WS_OVERLAPPEDWINDOW, FALSE, 0);
        mm->ptMinTrackSize.x = r.right - r.left; mm->ptMinTrackSize.y = r.bottom - r.top;
      }
      return 0;
    }
    case WM_SETFOCUS: h->focused = 1; if (h->app.focus) h->app.focus(h->app.user, 1); h->stale = 1; return 0;
    case WM_KILLFOCUS: h->focused = 0; memset(h->delivered, 0, sizeof h->delivered); if (h->app.focus) h->app.focus(h->app.user, 0); h->stale = 1; return 0;
    case WM_SETCURSOR: if (LOWORD(lp) == HTCLIENT) { ui_cursor_set(h); return TRUE; } break;
    case WM_LBUTTONDOWN: SetCapture(hwnd); SetFocus(hwnd); ui_seat_pointer(h, GET_X_LPARAM(lp) / h->dpr, GET_Y_LPARAM(lp) / h->dpr, 0); return 0;
    case WM_LBUTTONUP: ui_seat_pointer(h, GET_X_LPARAM(lp) / h->dpr, GET_Y_LPARAM(lp) / h->dpr, 2); ReleaseCapture(); return 0;
    case WM_MOUSEMOVE:
      if (h->held) ui_seat_pointer(h, GET_X_LPARAM(lp) / h->dpr, GET_Y_LPARAM(lp) / h->dpr, 1);
      else ui_seat_moved(h, GET_X_LPARAM(lp) / h->dpr, GET_Y_LPARAM(lp) / h->dpr);
      return 0;
    case WM_MBUTTONDOWN: ui_button(h, lp, 0, 1); return 0;
    case WM_MBUTTONUP: ui_button(h, lp, 2, 1); return 0;
    case WM_RBUTTONDOWN: ui_button(h, lp, 0, 2); return 0;
    case WM_RBUTTONUP: ui_button(h, lp, 2, 2); return 0;
    case WM_CAPTURECHANGED: if (h->held && (HWND)lp != hwnd) { POINT at; GetCursorPos(&at); ScreenToClient(hwnd, &at); ui_seat_pointer(h, at.x / h->dpr, at.y / h->dpr, 2); } return 0;
    case WM_MOUSEWHEEL: {
      // CSS pixels, 100 a notch, down positive (the Linux host's wheel)
      float dy = -(float)GET_WHEEL_DELTA_WPARAM(wp) / WHEEL_DELTA * 100.0f;
      if (h->app.wheel) h->app.wheel(h->app.user, dy);
      return 0;
    }
    case WM_KEYDOWN: case WM_SYSKEYDOWN:
      if (ui_key_down(h, wp)) return 0;
      break;
    case WM_KEYUP: case WM_SYSKEYUP: ui_key_up(h, wp); break;
    case WM_CHAR: ui_char(h, (WCHAR)wp); return 0;
    case WM_UNICHAR: if (wp == UNICODE_NOCHAR) return TRUE; break;
    case WM_IME_SETCONTEXT:
      // the page draws the marked text itself: no system composition window over it
      if (ui_ime_on(h)) lp &= ~(LPARAM)ISC_SHOWUICOMPOSITIONWINDOW;
      break;
    case WM_IME_STARTCOMPOSITION:
      if (ui_ime_on(h)) { HIMC imc = ImmGetContext(hwnd); if (imc) { ui_ime_place(h, imc); ImmReleaseContext(hwnd, imc); } return 0; }
      break;
    case WM_IME_COMPOSITION: if (ui_ime_on(h)) { ui_ime_composition(h, lp); return 0; } break;
    case WM_IME_ENDCOMPOSITION:
      if (ui_ime_on(h)) { if (h->app.ime->range(h->app.user, 2) >= 0) h->app.ime->unmark(h->app.user); return 0; }
      break;
    case WM_DROPFILES: ui_files(h, (HDROP)wp); DragFinish((HDROP)wp); return 0;
  }
  return DefWindowProcW(hwnd, msg, wp, lp);
}

static void ui_dpi_aware(void) {
  typedef BOOL (WINAPI *Aware)(HANDLE);
  Aware set = (Aware)(void (*)(void))GetProcAddress(GetModuleHandleW(L"user32.dll"), "SetProcessDpiAwarenessContext");
  if (set) set((HANDLE)(intptr_t)-4);   // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
}
static WCHAR *ui_wide(const char *s) {
  int n = MultiByteToWideChar(CP_UTF8, 0, s ? s : "", -1, NULL, 0);
  WCHAR *w = malloc((size_t)(n > 0 ? n : 1) * sizeof *w);
  if (w) { w[0] = 0; MultiByteToWideChar(CP_UTF8, 0, s ? s : "", -1, w, n); }
  return w;
}
static int ui_window_open(UiHost *h) {
  HINSTANCE self = GetModuleHandleW(NULL);
  ui_dpi_aware();
  WNDCLASSEXW wc = {.cbSize = sizeof wc, .style = CS_HREDRAW | CS_VREDRAW, .lpfnWndProc = ui_proc, .hInstance = self,
                    .hIcon = LoadIconW(self, MAKEINTRESOURCEW(1)), .hCursor = LoadCursorW(NULL, IDC_ARROW), .lpszClassName = UI_CLASS,
                    .hIconSm = LoadIconW(self, MAKEINTRESOURCEW(1))};
  if (!wc.hIcon) wc.hIcon = wc.hIconSm = LoadIconW(NULL, IDI_APPLICATION);
  RegisterClassExW(&wc);
  WCHAR *title = ui_wide(h->app.title ? h->app.title : "");
  h->hwnd = CreateWindowExW(0, UI_CLASS, title, WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, CW_USEDEFAULT, 640, 480, NULL, NULL, self, h);
  free(title);
  if (!h->hwnd) { fprintf(stderr, "ui-host: could not make a window (error %lu)\n", GetLastError()); return 0; }
  h->dpr = ui_dpi_of(h->hwnd) / 96.0f;
  // the first size, in points, at the window's screen's scale, centred on its screen
  RECT r = {0, 0, (LONG)((h->app.width ? h->app.width : 1120) * h->dpr), (LONG)((h->app.height ? h->app.height : 760) * h->dpr)};
  AdjustWindowRectEx(&r, WS_OVERLAPPEDWINDOW, FALSE, 0);
  MONITORINFO mi = {.cbSize = sizeof mi};
  GetMonitorInfoW(MonitorFromWindow(h->hwnd, MONITOR_DEFAULTTOPRIMARY), &mi);
  int ww = r.right - r.left, wh = r.bottom - r.top;
  int sw = mi.rcWork.right - mi.rcWork.left, sh = mi.rcWork.bottom - mi.rcWork.top;
  if (ww > sw) ww = sw;
  if (wh > sh) wh = sh;
  SetWindowPos(h->hwnd, NULL, mi.rcWork.left + (sw - ww) / 2, mi.rcWork.top + (sh - wh) / 2, ww, wh, SWP_NOZORDER | SWP_NOACTIVATE);
  RECT client; GetClientRect(h->hwnd, &client);
  h->want_w = (uint32_t)client.right; h->want_h = (uint32_t)client.bottom;
  DragAcceptFiles(h->hwnd, TRUE);
  return 1;
}

UiHost *ui_open(const UiApp *app, int offscreen) {
  UiHost *h = calloc(1, sizeof *h);
  if (!h) return NULL;
  h->app = *app; h->offscreen = offscreen; h->dpr = 1;
  h->words = ui_slurp(app->shader, &h->nwords);
  if (!h->words) { fprintf(stderr, "ui-host: no shader at %s\n", app->shader); free(h); return NULL; }
  char why[512];
  if (!ui_vk_load(why, sizeof why)) { fprintf(stderr, "ui-host: %s\n", why); free(h->words); free(h); return NULL; }
  if (offscreen) vkc_open(&h->v);
  else {
    if (!ui_window_open(h)) { free(h->words); free(h); return NULL; }
    static const char *const instance_exts[] = {VK_KHR_SURFACE_EXTENSION_NAME, VK_KHR_WIN32_SURFACE_EXTENSION_NAME};
    static const char *const device_exts[] = {VK_KHR_SWAPCHAIN_EXTENSION_NAME};
    vkc_open_with(&h->v, instance_exts, 2, device_exts, 1);
    PFN_vkCreateWin32SurfaceKHR make = (PFN_vkCreateWin32SurfaceKHR)vkGetInstanceProcAddr(h->v.inst, "vkCreateWin32SurfaceKHR");
    VkWin32SurfaceCreateInfoKHR info = {.sType = VK_STRUCTURE_TYPE_WIN32_SURFACE_CREATE_INFO_KHR, .hinstance = GetModuleHandleW(NULL), .hwnd = h->hwnd};
    VkBool32 present = VK_FALSE;
    if (!make || make(h->v.inst, &info, NULL, &h->surface) != VK_SUCCESS ||
        vkGetPhysicalDeviceSurfaceSupportKHR(h->v.gpu, h->v.family, h->surface, &present) != VK_SUCCESS || !present) {
      fprintf(stderr, "ui-host: the graphics driver cannot draw into a window (%s). Update the graphics driver.\n", h->v.props.deviceName);
      DestroyWindow(h->hwnd); free(h->words); free(h);
      return NULL;
    }
    vkp_open(&h->v, &h->p, h->surface, h->want_w ? h->want_w : 1, h->want_h ? h->want_h : 1);
    ShowWindow(h->hwnd, app->hidden ? SW_HIDE : SW_SHOWNORMAL);
    if (!app->hidden) { SetForegroundWindow(h->hwnd); SetFocus(h->hwnd); }
  }
  h->verts = vkc_buffer(&h->v, (VkDeviceSize)app->max_vertices * UI_VERTEX_BYTES);
  h->frame = malloc((size_t)app->max_vertices * UI_VERTEX_BYTES);
  if (!h->frame) { fprintf(stderr, "ui-host: no memory for a frame's vertices\n"); return NULL; }
  h->uniform = vkc_buffer(&h->v, 16);
  h->last = ui_now();
  return h;
}
static void ui_drive_poll(UiHost *h);
static void ui_drive_close(UiHost *h);
// A frame at the window's size: minimized, the app still gets its time ten times a second.
static int ui_draw(UiHost *h) {
  if (h->drawing) return 1;
  h->drawing = 1;
  int r = 1;
  if (!h->want_w || !h->want_h || IsIconic(h->hwnd)) {
    double now = ui_now();
    if (!h->app.composition && h->has_target && now - h->last >= 0.1) { float dt = (float)(now - h->last); h->last = now; ui_fill(h, dt > 0.1f ? 0.1f : dt); }
    ui_host_idle(h);
  } else {
    if (h->want_w != h->p.w || h->want_h != h->p.h) vkp_resize(&h->v, &h->p, h->want_w, h->want_h);
    r = ui_present(h);
  }
  h->drawing = 0;
  return r;
}
int ui_frame(UiHost *h) {
  ui_drive_poll(h);
  if (h->offscreen) return 0;
  MSG m;
  while (PeekMessageW(&m, NULL, 0, 0, PM_REMOVE)) {
    if (m.message == WM_QUIT) h->closed = 1;
    TranslateMessage(&m);
    DispatchMessageW(&m);
  }
  if (h->closed) return 0;
  return ui_draw(h);
}
void ui_close(UiHost *h) {
  if (!h) return;
  ui_drive_close(h);
  ui_render_close(h);
  if (!h->offscreen) {
    vkp_close(&h->v, &h->p);
    if (h->surface) vkDestroySurfaceKHR(h->v.inst, h->surface, NULL);
  }
  vkc_close(&h->v);
  if (h->hwnd) DestroyWindow(h->hwnd);
  free(h->frame); free(h->words); free(h);
}

#include "../drive.c"
