// A realm's window on a Wayland desktop (docs/os/2026-11-02-verse-os.md, the session's window
// before it owns the screen): an xdg-shell toplevel, windowed or full screen, with a Vulkan
// surface for vkp, the seat's keys (evdev codes, as metaverse/kms/evdev.h reads them, shifted the
// same way) and its pointer as host.js's pointer and wheel. vk-realm.c's --window and
// --fullscreen use it in place of vkd.h, evdev.h and mice.h: no root, no VT, no grab.
// Included once, by vk-realm.c, after realm_key and realm_key_up are declared.
#ifndef VKW_H
#define VKW_H
#include <wayland-client.h>
#include "keys.h"
#include "vkp.h"
#include "xdg-decoration-unstable-v1-client-protocol.h"
#include "xdg-shell-client-protocol.h"
#include <vulkan/vulkan_wayland.h>
#include <linux/input-event-codes.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#ifdef ENGINE_WAYLAND_INPUT_V1
#include "wayland-input-sdk.h"
#endif
#ifdef ENGINE_WAYLAND_TEXT_V1
#ifndef ENGINE_WAYLAND_INPUT_V1
#error "Wayland text requires the shared keyboard adapter"
#endif
#include "wayland-text-sdk.h"
#endif

bool realm_key(void *state, int key);
bool realm_key_up(void *state, int key);

typedef struct {
  void (*pointer)(void *user, float x, float y, int phase);  // 0 down, 1 move while down, 2 up
  void (*wheel)(void *user, float dy);                        // CSS pixels, 100 a notch
  void *user;
  void (*moved)(void *user, float x, float y);                // any motion, where it is: the person is there
  int (*key)(int code, int pressed);                          // 0 up, 1 press, 2 held-focus snapshot; return 1 to take it
  int (*button)(void *user,float x,float y,uint32_t code,int pressed); // additional physical mouse buttons
  int (*focus)(void *user,int focused); // 1: host released input; 0: retain per-key legacy release
  void (*shown)(void *user,int shown);
  uint32_t (*realm_key)(void *user,int key,int mods,int down);
  void (*text)(void *user,const char *bytes,size_t length);
  int (*wants_text)(void *user);
  int (*text_rect)(void *user,float rect[4]);
  uint64_t (*text_owner)(void *user);
  void (*text_error)(void *user,const char *message);
  // The pointer came over the window: set its cursor (a desktop app's; a realm hides it and draws its own).
  void (*pointer_enter)(void *user,struct wl_pointer *pointer,uint32_t serial);
  const char *app_id;   // the window's application id; a realm's when not given
} VkwTo;

typedef struct {
  struct wl_display *display;
  struct wl_registry *registry;
  struct wl_compositor *compositor;
  struct xdg_wm_base *wm;
  struct wl_seat *seat;
  struct wl_keyboard *keyboard;
  struct wl_pointer *pointer;
  struct wl_output *output;
  struct zxdg_decoration_manager_v1 *decorations;
  struct zxdg_toplevel_decoration_v1 *decoration;
  struct wl_surface *surface;
  struct xdg_surface *xdg;
  struct xdg_toplevel *toplevel;
  VkSurfaceKHR vk_surface;
  int configured, closed, fullscreen, keyboard_focused;
  int suspended;
  int32_t want_w, want_h;   // the last toplevel configure, 0 to choose
  int32_t refresh_mhz;
  double period;            // the output's frame time, in seconds
  float x, y;               // the pointer, in the window's pixels
  int down;                 // the left button is held
  unsigned char extra_down[2]; // right and middle buttons actually forwarded
  int shift, keys_down[KEY_MAX + 1];
  unsigned char forwarded_down[KEY_MAX + 1]; // physical keys delivered to the desk
  VkwTo to;
#ifdef ENGINE_WAYLAND_TEXT_V1
  HwtSdk text_sdk;
  unsigned char pasted_down[KEY_MAX+1];
#endif
#ifdef ENGINE_WAYLAND_INPUT_V1
  HwiSdk input;
  double axis_value;
  int64_t axis_discrete,axis120;
  int axis_source,axis_has_discrete,axis_has120,axis_pending;
#endif
} Vkw;

static Vkw *vkw_;   // the one window the listeners serve

#ifdef ENGINE_WAYLAND_TEXT_V1
static void vkw_compose_cancel(void*user){Vkw*w=user;if(w->input.compose)xkb_compose_state_reset(w->input.compose);}
#endif
static void vkw_text_sync(Vkw*w,int boundary){
#ifdef ENGINE_WAYLAND_TEXT_V1
 if(!w->to.realm_key)return;
 if(boundary)hwt_boundary_sdk(&w->text_sdk);
 hwt_sync(&w->text_sdk);
#else
 (void)w;(void)boundary;
#endif
}
static void vkw_text_invalidate(Vkw*w){
#ifdef ENGINE_WAYLAND_TEXT_V1
 if(w->to.realm_key){hwt_boundary_sdk(&w->text_sdk);memset(w->pasted_down,0,sizeof w->pasted_down);if(w->input.compose)xkb_compose_state_reset(w->input.compose);}
#else
 (void)w;
#endif
}
static int vkw_clipboard_set(Vkw*w,const char*bytes,size_t length){
#ifdef ENGINE_WAYLAND_TEXT_V1
 return w->to.realm_key&&hwt_copy(&w->text_sdk,bytes,length);
#else
 (void)w;(void)bytes;(void)length;return 0;
#endif
}

static void vkw_ping(void *d, struct xdg_wm_base *wm, uint32_t serial) { (void)d; xdg_wm_base_pong(wm, serial); }
static const struct xdg_wm_base_listener vkw_wm_listener = {.ping = vkw_ping};
static void vkw_xdg_configure(void *d, struct xdg_surface *x, uint32_t serial) { (void)d; xdg_surface_ack_configure(x, serial); vkw_->configured = 1; }
static const struct xdg_surface_listener vkw_xdg_listener = {.configure = vkw_xdg_configure};
static void vkw_top_configure(void *d, struct xdg_toplevel *t, int32_t w, int32_t h, struct wl_array *s) {
  (void)d;(void)t;vkw_->want_w=w;vkw_->want_h=h;
#ifdef XDG_TOPLEVEL_STATE_SUSPENDED_SINCE_VERSION
  int suspended=0;uint32_t *state;wl_array_for_each(state,s)if(*state==XDG_TOPLEVEL_STATE_SUSPENDED)suspended=1;
  if(suspended!=vkw_->suspended){vkw_->suspended=suspended;if(suspended)vkw_text_invalidate(vkw_);if(vkw_->to.shown)vkw_->to.shown(vkw_->to.user,!suspended);}
#ifdef ENGINE_WAYLAND_TEXT_V1
  if(vkw_->to.realm_key)hwt_focus(&vkw_->text_sdk,vkw_->keyboard_focused&&!suspended);
#endif
#else
  (void)s;
#endif
}
static void vkw_top_close(void *d, struct xdg_toplevel *t) { (void)d; (void)t; vkw_->closed = 1; }
#ifdef XDG_TOPLEVEL_CONFIGURE_BOUNDS_SINCE_VERSION
static void vkw_top_bounds(void *d,struct xdg_toplevel *t,int32_t w,int32_t h){(void)d;(void)t;(void)w;(void)h;}
#endif
#ifdef XDG_TOPLEVEL_WM_CAPABILITIES_SINCE_VERSION
static void vkw_top_caps(void *d,struct xdg_toplevel *t,struct wl_array *caps){(void)d;(void)t;(void)caps;}
#endif
static const struct xdg_toplevel_listener vkw_top_listener = {.configure = vkw_top_configure, .close = vkw_top_close,
#ifdef XDG_TOPLEVEL_CONFIGURE_BOUNDS_SINCE_VERSION
  .configure_bounds=vkw_top_bounds,
#endif
#ifdef XDG_TOPLEVEL_WM_CAPABILITIES_SINCE_VERSION
  .wm_capabilities=vkw_top_caps,
#endif
};

static void vkw_geometry(void *d, struct wl_output *o, int32_t x, int32_t y, int32_t pw, int32_t ph, int32_t sub, const char *make, const char *model, int32_t tr) {
  (void)d; (void)o; (void)x; (void)y; (void)pw; (void)ph; (void)sub; (void)make; (void)model; (void)tr;
}
static void vkw_mode(void *d, struct wl_output *o, uint32_t flags, int32_t w, int32_t h, int32_t refresh) {
  (void)d; (void)o; (void)w; (void)h;
  if (flags & WL_OUTPUT_MODE_CURRENT) vkw_->refresh_mhz = refresh;
}
static void vkw_output_done(void *d, struct wl_output *o) { (void)d; (void)o; }
static void vkw_output_scale(void *d, struct wl_output *o, int32_t s) { (void)d; (void)o; (void)s; }
static const struct wl_output_listener vkw_output_listener = {.geometry = vkw_geometry, .mode = vkw_mode, .done = vkw_output_done, .scale = vkw_output_scale};

static void vkw_keymap(void *d, struct wl_keyboard *k, uint32_t f, int32_t fd, uint32_t n) {
 (void)d;(void)k;
#ifdef ENGINE_WAYLAND_INPUT_V1
 if(vkw_->to.realm_key){if(!hwi_sdk_keymap(&vkw_->input,f,fd,n)){fprintf(stderr,"vk-realm: keyboard map refused\n");vkw_->closed=1;}return;}
#endif
 (void)f;(void)n;close(fd);
}
static void vkw_kb_enter(void *d, struct wl_keyboard *k, uint32_t s, struct wl_surface *w, struct wl_array *keys) {
  (void)d; (void)k; (void)s; (void)w;
  if(vkw_->to.focus)vkw_->to.focus(vkw_->to.user,1);
  vkw_->keyboard_focused=1;
#ifdef ENGINE_WAYLAND_TEXT_V1
  if(vkw_->to.realm_key){vkw_->text_sdk.serial=s;hwt_focus(&vkw_->text_sdk,!vkw_->suspended);}
#endif
  unsigned char actual[KEY_MAX+1] = {0};
  const uint32_t *codes = keys->data;
  for (size_t i=0; i<keys->size/sizeof *codes; i++) if (codes[i]<=KEY_MAX) actual[codes[i]]=1;
  vkw_->shift = actual[KEY_LEFTSHIFT]+actual[KEY_RIGHTSHIFT];
  for (int c=0; c<=KEY_MAX; c++) {
    if (vkw_->forwarded_down[c] && !actual[c]) {
      if (vkw_->to.key) vkw_->to.key(c,0);
      vkw_->forwarded_down[c]=0;
    }
    if (vkw_->keys_down[c] && !actual[c]) { realm_key_up(NULL,vkw_->keys_down[c]); vkw_->keys_down[c]=0; }
    // A snapshot preserves real physical ownership without retriggering panel toggles or voice.
    if (actual[c] && !vkw_->forwarded_down[c] && !vkw_->keys_down[c] && vkw_->to.key && vkw_->to.key(c,2)) vkw_->forwarded_down[c]=1;
  }
}
// Focus gone: every key held is let go, so none sticks while the window is behind another.
static void vkw_kb_leave(void *d, struct wl_keyboard *k, uint32_t s, struct wl_surface *w) {
  (void)d; (void)k; (void)s; (void)w;
  int released=vkw_->to.focus?vkw_->to.focus(vkw_->to.user,0):0;
  vkw_->keyboard_focused=0;
  vkw_text_invalidate(vkw_);
#ifdef ENGINE_WAYLAND_TEXT_V1
  if(vkw_->to.realm_key)hwt_focus(&vkw_->text_sdk,0);
#endif
  for (int c = 0; c <= KEY_MAX; c++) {
    if (vkw_->forwarded_down[c]) { if (vkw_->to.key) vkw_->to.key(c, 0); vkw_->forwarded_down[c] = 0; }
    if (vkw_->keys_down[c]) { if(!released)realm_key_up(NULL, vkw_->keys_down[c]);vkw_->keys_down[c] = 0; }
  }
  vkw_->shift = 0;
#ifdef ENGINE_WAYLAND_INPUT_V1
  hwi_sdk_reset(&vkw_->input);
#endif
}
// evdev codes, as the seat sends them; shifted and released as evdev.h does.
static void vkw_key(void *d, struct wl_keyboard *k, uint32_t serial, uint32_t time, uint32_t code, uint32_t state) {
  (void)d; (void)k; (void)serial; (void)time;
  if (code > KEY_MAX) return;
  int pressed = state == WL_KEYBOARD_KEY_STATE_PRESSED;
  if (code == KEY_LEFTSHIFT || code == KEY_RIGHTSHIFT) vkw_->shift += pressed ? 1 : (vkw_->shift > 0 ? -1 : 0);
  if (!pressed && vkw_->forwarded_down[code]) {
    if (vkw_->to.key) vkw_->to.key((int)code, 0);
    vkw_->forwarded_down[code] = 0; return;
  }
  if (vkw_->to.key && !vkw_->keys_down[code] && vkw_->to.key((int)code, pressed)) {
    vkw_->forwarded_down[code] = pressed; return;
  }
#ifdef ENGINE_WAYLAND_INPUT_V1
  if(vkw_->to.realm_key){
#ifdef ENGINE_WAYLAND_TEXT_V1
   HwtSdk*t=&vkw_->text_sdk;t->serial=serial;vkw_text_sync(vkw_,pressed&&hwt_boundary(code));
   if(!pressed&&vkw_->pasted_down[code]){vkw_->pasted_down[code]=0;return;}
   if(pressed&&hwt_paste(code,hwi_sdk_mods(&vkw_->input),hwt_wants(t))){vkw_->pasted_down[code]=1;if(!hwt_paste_start(t))hwt_error(t,"Clipboard unavailable");return;}
   if(pressed&&t->preedit&&vkw_->input.state){xkb_keysym_t sym=xkb_state_key_get_one_sym(vkw_->input.state,code+8);if(hwi_text(hwt_wants(t),hwi_sdk_mods(&vkw_->input),hwi_key(sym,xkb_keysym_to_utf32(sym)))){vkw_->input.text_down[code]=1;return;}}
#endif
   hwi_sdk_key(&vkw_->input,vkw_->to.user,code,pressed,vkw_->to.wants_text?vkw_->to.wants_text(vkw_->to.user):0,vkw_->to.realm_key,vkw_->to.text);vkw_text_sync(vkw_,0);return;}
#endif
  int r = pressed ? realm_key_shifted((int)code, vkw_->shift > 0) : vkw_->keys_down[code];
  if (!r) return;
  if (pressed) { vkw_->keys_down[code] = r; realm_key(NULL, r); }
  else { vkw_->keys_down[code] = 0; realm_key_up(NULL, r); }
}
static void vkw_mods(void *d, struct wl_keyboard *k, uint32_t serial, uint32_t a, uint32_t b, uint32_t c, uint32_t g) {
 (void)d;(void)k;(void)serial;
#ifdef ENGINE_WAYLAND_INPUT_V1
 if(vkw_->to.realm_key){hwi_sdk_modifiers(&vkw_->input,a,b,c,g);return;}
#endif
 (void)a;(void)b;(void)c;(void)g;
}
static void vkw_repeat(void *d, struct wl_keyboard *k, int32_t rate, int32_t delay) { (void)d; (void)k; (void)rate; (void)delay; }
static const struct wl_keyboard_listener vkw_kb_listener = {.keymap = vkw_keymap, .enter = vkw_kb_enter, .leave = vkw_kb_leave, .key = vkw_key, .modifiers = vkw_mods, .repeat_info = vkw_repeat};

// The pointer: hidden over the realm, which draws its own.
static void vkw_pt_enter(void *d, struct wl_pointer *p, uint32_t serial, struct wl_surface *s, wl_fixed_t x, wl_fixed_t y) {
  (void)d; (void)s;
  if (vkw_->to.pointer_enter) vkw_->to.pointer_enter(vkw_->to.user, p, serial);
  else wl_pointer_set_cursor(p, serial, NULL, 0, 0);
  vkw_->x = (float)wl_fixed_to_double(x);
  vkw_->y = (float)wl_fixed_to_double(y);
}
static void vkw_pt_leave(void *d, struct wl_pointer *p, uint32_t serial, struct wl_surface *s) {
  (void)d; (void)p; (void)serial; (void)s;
  if (vkw_->to.realm_key) return;
  if (vkw_->down) { vkw_->down = 0; if (vkw_->to.pointer) vkw_->to.pointer(vkw_->to.user, vkw_->x, vkw_->y, 2); }
  for(int i=0;i<2;i++) if(vkw_->extra_down[i]) {
    if(vkw_->to.button)vkw_->to.button(vkw_->to.user,vkw_->x,vkw_->y,BTN_RIGHT+i,0);
    vkw_->extra_down[i]=0;
  }
}
static void vkw_pt_motion(void *d, struct wl_pointer *p, uint32_t time, wl_fixed_t x, wl_fixed_t y) {
  (void)d; (void)p; (void)time;
  vkw_->x = (float)wl_fixed_to_double(x);
  vkw_->y = (float)wl_fixed_to_double(y);
  if (vkw_->down && vkw_->to.pointer) vkw_->to.pointer(vkw_->to.user, vkw_->x, vkw_->y, 1);
  if (vkw_->to.moved) vkw_->to.moved(vkw_->to.user, vkw_->x, vkw_->y);
}
static void vkw_pt_button(void *d, struct wl_pointer *p, uint32_t serial, uint32_t time, uint32_t button, uint32_t state) {
  (void)d; (void)p; (void)serial; (void)time;
#ifdef ENGINE_WAYLAND_TEXT_V1
  vkw_->text_sdk.serial=serial;if(state==WL_POINTER_BUTTON_STATE_PRESSED)vkw_text_sync(vkw_,1);
#endif
  if (button != BTN_LEFT) {
    if(button!=BTN_RIGHT && button!=BTN_MIDDLE)return;
    int i=(int)button-BTN_RIGHT,pressed=state==WL_POINTER_BUTTON_STATE_PRESSED;
    if(!pressed && vkw_->extra_down[i]) {
      if(vkw_->to.button)vkw_->to.button(vkw_->to.user,vkw_->x,vkw_->y,button,0);
      vkw_->extra_down[i]=0;
    } else if(pressed && !vkw_->extra_down[i] && vkw_->to.button && vkw_->to.button(vkw_->to.user,vkw_->x,vkw_->y,button,1))vkw_->extra_down[i]=1;
    return;
  }
  vkw_->down = state == WL_POINTER_BUTTON_STATE_PRESSED;
  if (vkw_->to.pointer) vkw_->to.pointer(vkw_->to.user, vkw_->x, vkw_->y, vkw_->down ? 0 : 2);
  vkw_text_sync(vkw_,0);
}
static void vkw_pt_axis(void *d, struct wl_pointer *p, uint32_t time, uint32_t axis, wl_fixed_t value) {
 (void)d;(void)time;
 if(axis!=WL_POINTER_AXIS_VERTICAL_SCROLL)return;
#ifdef ENGINE_WAYLAND_INPUT_V1
 if(vkw_->to.realm_key){vkw_->axis_value+=wl_fixed_to_double(value);vkw_->axis_pending=1;
  if(wl_proxy_get_version((struct wl_proxy*)p)>=WL_POINTER_FRAME_SINCE_VERSION)return;
  if(vkw_->to.wheel)vkw_->to.wheel(vkw_->to.user,(float)hwi_wheel(vkw_->axis_value,0,false,0,false,0));vkw_->axis_value=0;vkw_->axis_pending=0;return;}
#endif
 (void)p;if(vkw_->to.wheel)vkw_->to.wheel(vkw_->to.user,(float)wl_fixed_to_double(value)*10);
}
static void vkw_pt_frame(void *d, struct wl_pointer *p) {
 (void)d;(void)p;
#ifdef ENGINE_WAYLAND_INPUT_V1
 if(vkw_->to.realm_key&&vkw_->axis_pending&&vkw_->to.wheel)vkw_->to.wheel(vkw_->to.user,(float)hwi_wheel(vkw_->axis_value,vkw_->axis_discrete,vkw_->axis_has_discrete,vkw_->axis120,vkw_->axis_has120,vkw_->axis_source));
 vkw_->axis_value=0;vkw_->axis_discrete=0;vkw_->axis120=0;vkw_->axis_has_discrete=0;vkw_->axis_has120=0;vkw_->axis_pending=0;vkw_->axis_source=0;
#endif
}
static void vkw_pt_source(void *d, struct wl_pointer *p, uint32_t source) {
 (void)d;(void)p;
#ifdef ENGINE_WAYLAND_INPUT_V1
 vkw_->axis_source=(int)source;
#else
 (void)source;
#endif
}
static void vkw_pt_stop(void *d, struct wl_pointer *p, uint32_t time, uint32_t axis) { (void)d;(void)p;(void)time;(void)axis; }
static void vkw_pt_discrete(void *d, struct wl_pointer *p, uint32_t axis, int32_t value) {
 (void)d;(void)p;
#ifdef ENGINE_WAYLAND_INPUT_V1
 if(axis==WL_POINTER_AXIS_VERTICAL_SCROLL){vkw_->axis_discrete+=value;vkw_->axis_has_discrete=1;vkw_->axis_pending=1;}
#else
 (void)axis;(void)value;
#endif
}
#if defined(ENGINE_WAYLAND_INPUT_V1) && defined(WL_POINTER_AXIS_VALUE120_SINCE_VERSION)
static void vkw_pt_value120(void *d,struct wl_pointer *p,uint32_t axis,int32_t value){(void)d;(void)p;if(axis==WL_POINTER_AXIS_VERTICAL_SCROLL){vkw_->axis120+=value;vkw_->axis_has120=1;vkw_->axis_pending=1;}}
#endif
static const struct wl_pointer_listener vkw_pt_listener = {.enter = vkw_pt_enter, .leave = vkw_pt_leave, .motion = vkw_pt_motion, .button = vkw_pt_button,
  .axis = vkw_pt_axis, .frame = vkw_pt_frame, .axis_source = vkw_pt_source, .axis_stop = vkw_pt_stop, .axis_discrete = vkw_pt_discrete,
#if defined(ENGINE_WAYLAND_INPUT_V1) && defined(WL_POINTER_AXIS_VALUE120_SINCE_VERSION)
 .axis_value120=vkw_pt_value120,
#endif
};

static void vkw_caps(void *d, struct wl_seat *seat, uint32_t caps) {
  (void)d;
  if ((caps & WL_SEAT_CAPABILITY_KEYBOARD) && !vkw_->keyboard) {
    vkw_->keyboard = wl_seat_get_keyboard(seat);
    wl_keyboard_add_listener(vkw_->keyboard, &vkw_kb_listener, NULL);
  }
  if ((caps & WL_SEAT_CAPABILITY_POINTER) && !vkw_->pointer) {
    vkw_->pointer = wl_seat_get_pointer(seat);
    wl_pointer_add_listener(vkw_->pointer, &vkw_pt_listener, NULL);
  }
}
static void vkw_seat_name(void *d, struct wl_seat *s, const char *name) { (void)d; (void)s; (void)name; }
static const struct wl_seat_listener vkw_seat_listener = {.capabilities = vkw_caps, .name = vkw_seat_name};

static void vkw_global(void *d, struct wl_registry *r, uint32_t name, const char *iface, uint32_t version) {
  (void)d;
#ifdef ENGINE_WAYLAND_TEXT_V1
  if(vkw_->to.realm_key&&!strcmp(iface,zwp_text_input_manager_v3_interface.name)){vkw_->text_sdk.manager=wl_registry_bind(r,name,&zwp_text_input_manager_v3_interface,1);return;}
  if(vkw_->to.realm_key&&!strcmp(iface,wl_data_device_manager_interface.name)){vkw_->text_sdk.data_manager=wl_registry_bind(r,name,&wl_data_device_manager_interface,version<3?version:3);return;}
#endif
  if (!strcmp(iface, wl_compositor_interface.name)) vkw_->compositor = wl_registry_bind(r, name, &wl_compositor_interface, 4);
  else if (!strcmp(iface, xdg_wm_base_interface.name)) {
#ifdef XDG_TOPLEVEL_STATE_SUSPENDED_SINCE_VERSION
    vkw_->wm = wl_registry_bind(r, name, &xdg_wm_base_interface, version<6?version:6);
#else
    vkw_->wm = wl_registry_bind(r, name, &xdg_wm_base_interface, version<2?version:2);
#endif
    xdg_wm_base_add_listener(vkw_->wm, &vkw_wm_listener, NULL);
  } else if (!strcmp(iface, wl_seat_interface.name) && !vkw_->seat) {
    #if defined(ENGINE_WAYLAND_INPUT_V1) && defined(WL_POINTER_AXIS_VALUE120_SINCE_VERSION)
    vkw_->seat = wl_registry_bind(r,name,&wl_seat_interface,version<8?version:8);
#else
    vkw_->seat = wl_registry_bind(r,name,&wl_seat_interface,version<5?version:5);
#endif
    wl_seat_add_listener(vkw_->seat, &vkw_seat_listener, NULL);
  } else if (!strcmp(iface, wl_output_interface.name) && !vkw_->output) {
    vkw_->output = wl_registry_bind(r, name, &wl_output_interface, 2);
    wl_output_add_listener(vkw_->output, &vkw_output_listener, NULL);
  } else if (!strcmp(iface, zxdg_decoration_manager_v1_interface.name))
    vkw_->decorations = wl_registry_bind(r, name, &zxdg_decoration_manager_v1_interface, 1);
}
static void vkw_global_remove(void *d, struct wl_registry *r, uint32_t name) { (void)d; (void)r; (void)name; }
static const struct wl_registry_listener vkw_registry_listener = {.global = vkw_global, .global_remove = vkw_global_remove};

// The window, mapped, and Vulkan opened with its surface. Exits, saying why, if it cannot.
static void vkw_open(Vkc *v, Vkw *w, const char *title, uint32_t width, uint32_t height, int fullscreen, VkwTo to) {
  memset(w, 0, sizeof *w);
  vkw_ = w;
  w->to = to;
#ifdef ENGINE_WAYLAND_TEXT_V1
  w->text_sdk.paste_fd=-1;
#endif
#ifndef ENGINE_WAYLAND_INPUT_V1
  if(to.realm_key){fprintf(stderr,"vk-realm: required keyboard adapter unavailable\n");exit(1);}
#endif
  w->fullscreen = fullscreen;
  w->display = wl_display_connect(NULL);
  if (!w->display) { fprintf(stderr, "vk-realm: no Wayland display (WAYLAND_DISPLAY)\n"); exit(1); }
  w->registry = wl_display_get_registry(w->display);
  wl_registry_add_listener(w->registry, &vkw_registry_listener, NULL);
  wl_display_roundtrip(w->display);
  wl_display_roundtrip(w->display);
  if (!w->compositor || !w->wm) { fprintf(stderr, "vk-realm: the compositor has no xdg-shell\n"); exit(1); }
  w->surface = wl_compositor_create_surface(w->compositor);
#ifdef ENGINE_WAYLAND_TEXT_V1
  if(to.realm_key)hwt_open_sdk(&w->text_sdk,(HwtTo){.user=to.user,.text=to.text,.wants=to.wants_text,.rect=to.text_rect,.owner=to.text_owner,.error=to.text_error,.cancel_compose=vkw_compose_cancel,.cancel_user=w},w->surface,w->seat);
#endif
  w->xdg = xdg_wm_base_get_xdg_surface(w->wm, w->surface);
  xdg_surface_add_listener(w->xdg, &vkw_xdg_listener, NULL);
  w->toplevel = xdg_surface_get_toplevel(w->xdg);
  xdg_toplevel_add_listener(w->toplevel, &vkw_top_listener, NULL);
  xdg_toplevel_set_title(w->toplevel, title);
  xdg_toplevel_set_app_id(w->toplevel, to.app_id ? to.app_id : "com.openagents.verse");
  if (w->decorations) {
    w->decoration = zxdg_decoration_manager_v1_get_toplevel_decoration(w->decorations, w->toplevel);
    zxdg_toplevel_decoration_v1_set_mode(w->decoration, ZXDG_TOPLEVEL_DECORATION_V1_MODE_SERVER_SIDE);
  }
  if (fullscreen) xdg_toplevel_set_fullscreen(w->toplevel, NULL);
  wl_surface_commit(w->surface);
  while (!w->configured && wl_display_dispatch(w->display) >= 0) {}
  if (w->want_w > 0 && w->want_h > 0) { width = (uint32_t)w->want_w; height = (uint32_t)w->want_h; }
  w->want_w = (int32_t)width;
  w->want_h = (int32_t)height;
  w->period = w->refresh_mhz > 0 ? 1000.0 / w->refresh_mhz : 1 / 60.0;

  const char *iexts[] = {VK_KHR_SURFACE_EXTENSION_NAME, VK_KHR_WAYLAND_SURFACE_EXTENSION_NAME};
  const char *dexts[] = {VK_KHR_SWAPCHAIN_EXTENSION_NAME};
  vkc_open_with(v, iexts, 2, dexts, 1);
  VkWaylandSurfaceCreateInfoKHR wsi = {.sType = VK_STRUCTURE_TYPE_WAYLAND_SURFACE_CREATE_INFO_KHR, .display = w->display, .surface = w->surface};
  if (vkCreateWaylandSurfaceKHR(v->inst, &wsi, NULL, &w->vk_surface) != VK_SUCCESS) { fprintf(stderr, "vk-realm: no Vulkan surface on the window\n"); exit(1); }
  VkBool32 presents = VK_FALSE;
  vkGetPhysicalDeviceSurfaceSupportKHR(v->gpu, v->family, w->vk_surface, &presents);
  if (!presents) { fprintf(stderr, "vk-realm: the queue cannot present to the window\n"); exit(1); }
}

// What the compositor has sent, without waiting for more.
static void vkw_pump(Vkw *w) {
  vkw_text_sync(w,0);
  while (wl_display_prepare_read(w->display) != 0) wl_display_dispatch_pending(w->display);
  wl_display_flush(w->display);
  struct pollfd p = {.fd = wl_display_get_fd(w->display), .events = POLLIN};
  if (poll(&p, 1, 0) > 0) wl_display_read_events(w->display);
  else wl_display_cancel_read(w->display);
  wl_display_dispatch_pending(w->display);
#ifdef ENGINE_WAYLAND_TEXT_V1
  if(w->to.realm_key)hwt_poll(&w->text_sdk);
#endif
}

static void vkw_close(Vkc *v, Vkw *w) {
#ifdef ENGINE_WAYLAND_TEXT_V1
  if(w->to.realm_key)hwt_close_sdk(&w->text_sdk);
#endif
#ifdef ENGINE_WAYLAND_INPUT_V1
  hwi_sdk_close(&w->input);
#endif
  vkDestroySurfaceKHR(v->inst, w->vk_surface, NULL);
  vkc_close(v);
  if(w->decoration)zxdg_toplevel_decoration_v1_destroy(w->decoration);
  xdg_toplevel_destroy(w->toplevel);
  xdg_surface_destroy(w->xdg);
  wl_surface_destroy(w->surface);
  if(w->keyboard){if(wl_proxy_get_version((struct wl_proxy*)w->keyboard)>=3)wl_keyboard_release(w->keyboard);else wl_keyboard_destroy(w->keyboard);}
  if(w->pointer){if(wl_proxy_get_version((struct wl_proxy*)w->pointer)>=3)wl_pointer_release(w->pointer);else wl_pointer_destroy(w->pointer);}
  if(w->output)wl_output_destroy(w->output);
  if(w->seat){if(wl_proxy_get_version((struct wl_proxy*)w->seat)>=5)wl_seat_release(w->seat);else wl_seat_destroy(w->seat);}
  if(w->decorations)zxdg_decoration_manager_v1_destroy(w->decorations);
  xdg_wm_base_destroy(w->wm);
  wl_compositor_destroy(w->compositor);
  wl_registry_destroy(w->registry);
  wl_display_disconnect(w->display);
  if(vkw_==w)vkw_=NULL;
  memset(w,0,sizeof *w);
}
#endif
