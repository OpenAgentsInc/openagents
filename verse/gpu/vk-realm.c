// vk-realm: a realm drawn every frame natively by its own shaders (docs/verse/native-gpu.md,
// gpu-realm-host): its Coax built with gcc (verse/native/realm-host.h, the face host.js
// drives) and the passes host.js draws, as the spec everglade-pass --emit writes (the live
// WGSL lowered by coax spirv, the layers decoded in Coax), kept on the GPU (vk-spec.h). Each
// frame the realm runs, its uniforms, stores, particles, draw list and own vertices (the HUD) go into the spec's
// buffers in place, its counts are set, and the spec is recorded and presented.
//   vk-realm DIR PACK [--facts FILE] [--extra N,...] [--size WxH] [--scale K] [--golden]
//            [--seconds N] [--demo] [--shot OUT.ppm] [--shot-at S]       on the screen (vkd.h)
//            [--net DIR] [--origin HOST[:PORT]] [--name NAME] [--leave-to VT]
//   vk-realm DIR PACK ... --window [--fullscreen] [--size WxH] ...          in a window on the Wayland desktop (vkw.h)
//   vk-realm DIR PACK ... --headless FRAMES [--out OUT.f32]                 offscreen
//            [--fps N] [--frames-dir DIR [--film-start A] [--film-end B]]  film frames (#13003)
//            [--samples N] [--shutter S] [--shutter-position start|center|end] [--seed K]
//            [--fstop F --focus M] [--film-view U] [--no-jitter] [--film-audio OUT.wav]  the film profile (#13004)
// DIR holds spec.txt and its files. --size is what the passes draw at (default: the screen's,
// or the spec's offscreen); on the screen it is scaled to the mode's size. --headless runs
// FRAMES frames at 1/60 s each (as realm-run and wasm-run do), drawing every one but the first, and writes the last frame's targets as
// float RGBA (vk-passes' OUT). --fps N steps each frame 1/N s instead (the realm's own fixed
// step and interpolation decide the world at that time; the simulation is never stepped
// differently). --frames-dir writes frames A..B (frame indices, default all) to DIR as
// frame-NNNNNN.rgba16f (the --raw-out layout: half RGBA of each last target in turn), with
// DIR/film.txt (size, targets, fps, range) and DIR/timing.txt (open, then each written
// frame's step, GPU and read-back seconds); metaverse-engine/tools/film-frames.coax makes them
// review PNGs, hashes, meta.json and the encode. The film profile (metaverse-engine/render/film.coax) makes each
// written frame the average of N samples: the realm advanced to each sample's time (stratified in
// the shutter, whole nanoseconds, so it runs the steps one sample would), its view-projection
// (the 16 floats at uniform U, --film-view) moved by the sample's pixel jitter and, with --fstop
// and --focus, its eye on the aperture; each sample's final targets are read back and added in
// f32, and the mean is written. --film-audio renders the realm's own mixer with no device, frame
// by frame at 48 kHz, into OUT.wav. Escape held a second, the --seconds limit, SIGINT or SIGTERM end it on the
// screen; display.sh-style launchers (verse/gpu/realm.sh) switch to a free VT first.
// --net DIR gives the realm its network and what it keeps (realm-host.h, rh_net_open): its
// requests go to HOST (default openagents.com, port 443) through DIR/../net or ./net (the
// helper, verse/native/net.coax), and its player key, place and bar (as NAME, default
// everglade) are files in DIR. Without it the realm is offline, as the checks run it.
#define _GNU_SOURCE
// the kms face's key calls under names of vk-realm's own: the realm module (haunted-house, sandbox,
// drop, built) has a Coax realm_key of its own
#define realm_key vk_realm_key
#define realm_key_up vk_realm_key_up
#include "evdev.h"
#include "face.h"
#include "mice.h"
#include "realm-host.h"
#if defined(__has_include)
#if __has_include("RenderIdentity.h")
#include "RenderIdentity.h"
#endif
#endif
#ifdef AETHYR_RENDER_TYPED
#include "render-realm.h"
#include "linux-window-session.h"
#else
#include "vk-spec.h"
#endif
#include "vkd.h"
// The film profile (metaverse-engine/render/film.coax as C, built beside this file by realm.sh and
// build-linux-world.coax); a build without it refuses the film flags.
#if defined(__has_include)
#if __has_include("film.h")
#include "film.h"
#define VK_FILM 1
#endif
#endif
#ifndef VK_FILM
#define VK_FILM 0
static int32_t film_host_admit(int32_t a,float b,int32_t c,int32_t d,int32_t e,int32_t f){(void)a;(void)b;(void)c;(void)d;(void)e;(void)f;return 0;}
static int64_t film_host_sample_ns(int32_t a,float b,int32_t c,int64_t d,int64_t e,int32_t f,int32_t g){(void)a;(void)b;(void)c;(void)d;(void)e;(void)f;(void)g;return 0;}
static float film_host_jitter(int32_t a,float b,int32_t c,int64_t d,int64_t e,int32_t f,int32_t g){(void)a;(void)b;(void)c;(void)d;(void)e;(void)f;(void)g;return 0;}
static float film_host_lens(int32_t a,float b,int32_t c,int64_t d,int64_t e,int32_t f,int32_t g){(void)a;(void)b;(void)c;(void)d;(void)e;(void)f;(void)g;return 0;}
typedef struct { float *data; int64_t count; } vk_film_view;
#define coax_v_f32 vk_film_view
static _Bool film_host_matrix(vk_film_view m,float a,float b,float c,float d,float e,float f,float g,float h){(void)m;(void)a;(void)b;(void)c;(void)d;(void)e;(void)f;(void)g;(void)h;return 0;}
#endif
#include "vkw.h"
#include "vkdesk.h"
#include "vkhands.h"
#include <math.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#include <sys/resource.h>
#include <fcntl.h>
#include <linux/kd.h>
#include <linux/vt.h>
#include <sys/ioctl.h>

#define MAX_FRAMES 100000

// --leave-to N (the boot session, docs/os/2026-11-02-verse-os.md V0): the realm runs on its own
// VT, which this process's login owns, so no root is needed. While it runs the console is kept
// in graphics mode (no text drawn over the realm); on leaving it goes back to text and VT N shows.
static int vt_fd = -1;
// A crash gives the console back too, or the VT would stay dark with nothing drawing it.
static void vt_on_crash(int sig) {
  if (vt_fd >= 0) ioctl(vt_fd, KDSETMODE, KD_TEXT);
  signal(sig, SIG_DFL);
  raise(sig);
}
static void vt_hold(void) {
  vt_fd = open("/dev/tty", O_RDWR | O_CLOEXEC);
  if (vt_fd >= 0 && ioctl(vt_fd, KDSETMODE, KD_GRAPHICS) < 0) { close(vt_fd); vt_fd = -1; }
  if (vt_fd >= 0) { signal(SIGSEGV, vt_on_crash); signal(SIGBUS, vt_on_crash); signal(SIGABRT, vt_on_crash); signal(SIGFPE, vt_on_crash); }
}
static void vt_leave(int to) {
  if (vt_fd < 0) return;
  ioctl(vt_fd, KDSETMODE, KD_TEXT);
  if (to > 0) ioctl(vt_fd, VT_ACTIVATE, to);
  close(vt_fd);
  vt_fd = -1;
}
static RealmHost H;
static uint64_t producer_calls;
static Vkw *text_window;
static int to_text_rect(void *,float[4]);
static const char *image_request_observer;
extern int32_t guard_http(cx_face_addr,uint32_t,cx_face_addr,uint32_t,uint32_t,int64_t);
static volatile int touched;
#ifdef AETHYR_RENDER_TYPED
static int64_t recovery_observed[23];
#endif
// Optional native shell effects; ordinary realms keep the existing defaults.
__attribute__((weak)) const char *coaxial_window_title(void);
__attribute__((weak)) const char *coaxial_window_app_id(void);
__attribute__((weak)) int coaxial_should_close(cx_face_addr state);
__attribute__((weak)) void coaxial_close(cx_face_addr state);
__attribute__((weak)) void coaxial_hover(cx_face_addr state,float x,float y);
__attribute__((weak)) double coaxial_frame_parts(cx_face_addr state,int32_t field);
__attribute__((weak)) int32_t coaxial_net_probe(cx_face_addr state,int32_t field);
__attribute__((weak)) int32_t coaxial_wants_text(cx_face_addr state);
__attribute__((weak)) cx_face_addr coaxial_field(cx_face_addr state);
__attribute__((weak)) uint32_t coaxial_host_copy_span(uint64_t,uint64_t,uint64_t,uint64_t);
static VkQueryPool benchmark_queries;
static int benchmark_timestamps=1;
static FILE *benchmark_file;
static uint64_t benchmark_rows;
static double benchmark_first,benchmark_begin;
static double benchmark_cpu_begin,benchmark_replay_begin,benchmark_replay_end;
static double benchmark_process_cpu(void){struct rusage r;getrusage(RUSAGE_SELF,&r);return r.ru_utime.tv_sec+r.ru_utime.tv_usec/1e6+r.ru_stime.tv_sec+r.ru_stime.tv_usec/1e6;}
static double benchmark_thread_cpu(void){struct timespec t;clock_gettime(CLOCK_THREAD_CPUTIME_ID,&t);return t.tv_sec+t.tv_nsec/1e9;}
static volatile sig_atomic_t stopping;
static void on_signal(int s) { (void)s; stopping = 1; }
static double now_s(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return t.tv_sec + t.tv_nsec / 1e9;
}
static int64_t now_us(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (int64_t)t.tv_sec*1000000+t.tv_nsec/1000;}
static int to_focus(void *user,int focused){(void)user;if(rh_host_version()!=1)return 0;if(!focused&&text_window)vkw_text_invalidate(text_window);rh_lifecycle_event(&H,focused?2:3,now_us(),0);return 1;}
static uint32_t to_realm_key(void *user,int key,int mods,int down){(void)user;touched = 1;return rh_key(&H,key,mods,down);}
static void to_text(void *user,const char *bytes,size_t length){(void)user;(void)rh_text(&H,bytes,length);}
static int to_wants_text(void *user){(void)user;return coaxial_wants_text?coaxial_wants_text(H.state):0;}
static uint64_t to_text_owner(void *user){return to_wants_text(user)?(uint64_t)H.state:0;}
static void to_text_error(void *user,const char *message){(void)user;(void)message;rh_refuse(&H);}
static int to_clipboard(void *user,const char *bytes,size_t length){return vkw_clipboard_set(user,bytes,length);}
static void to_shown(void *user,int shown){(void)user;if(!shown&&text_window)vkw_text_invalidate(text_window);rh_lifecycle_event(&H,shown?5:4,now_us(),0);rh_send(&H,shown?602:601,"",0);}

// 16 random hex digits from the kernel.
static void random_id(char out[17]) {
  unsigned char b[8] = {0};
  FILE *f = fopen("/dev/urandom", "rb");
  if (!f || fread(b, 1, 8, f) != 8) { if (f) fclose(f); f = NULL; for (int i = 0; i < 8; i++) b[i] = (unsigned char)(rand() ^ (int)time(NULL)); }
  if (f) fclose(f);
  for (int i = 0; i < 8; i++) snprintf(out + 2 * i, 3, "%02x", b[i]);
}
// Online, the page facts host.js gives a browser: this machine's visitor (made once, kept in
// DIR/visitor, as localStorage keeps a browser's), a new tab each run (fresh), and the time.
static char *online_facts(char *text, size_t *n, const char *dir) {
  char path[1100], visitor[17] = {0}, tab[17];
  snprintf(path, sizeof path, "%s/visitor", dir);
  mkdir(dir, 0700);
  FILE *f = fopen(path, "r");
  if (!f || fread(visitor, 1, 16, f) != 16) {
    random_id(visitor);
    FILE *o = fopen(path, "w");
    if (o) { fputs(visitor, o); fclose(o); }
  }
  if (f) fclose(f);
  random_id(tab);
  struct timespec t;
  clock_gettime(CLOCK_REALTIME, &t);
  char *out = malloc(*n + 256);
  size_t k = 0;
  for (char *line = text; line < text + *n;) {
    char *e = memchr(line, '\n', (size_t)(text + *n - line));
    size_t len = e ? (size_t)(e - line) : (size_t)(text + *n - line);
    if (len > 6 && !strncmp(line, "visitor=", 8)) k += (size_t)sprintf(out + k, "visitor=%s\n", visitor);
    else if (len > 3 && !strncmp(line, "tab=", 4)) k += (size_t)sprintf(out + k, "tab=%s\n", tab);
    else if (len > 3 && !strncmp(line, "now=", 4)) k += (size_t)sprintf(out + k, "now=%lld\n", (long long)t.tv_sec * 1000 + t.tv_nsec / 1000000);
    else { memcpy(out + k, line, len); k += len; out[k++] = '\n'; }
    line += len + 1;
  }
  explicit_bzero(text, *n);   // the facts may be secret (--facts-once): no copy left behind
  free(text);
  *n = k;
  return out;
}

// --activity FILE (docs/os/2026-11-02-verse-os.md, V5): every input marks the person present, and
// the file's time is set to now at most once a second, so the session can tell how long the Metaverse
// has been left alone (os/verse-pylon-policy.coax decides what that means).
static const char *activity;
static double touched_at;
static void touch_activity(double now) {
  if (!activity || !touched || now - touched_at < 1) return;
  touched = 0; touched_at = now;
  int fd = open(activity, O_WRONLY | O_CREAT | O_CLOEXEC, 0600);
  if (fd >= 0) { futimens(fd, NULL); close(fd); }
}
// --agents FILE (V4): the agents on this machine, as verse-session writes them (os/verse-agents.coax);
// read again when the file's time changes (looked at once a second) and sent to the realm as
// message 610, which the Grid draws as figures by the spawn (newland-local.coax).
#define RH_AGENTS_KIND 610
static const char *agents_file;
static long long agents_mtime;
static double agents_at;
static char *slurp(const char *path, size_t *n);
static void agents_poll(double now) {
  if (!agents_file || now - agents_at < 1) return;
  agents_at = now;
  struct stat st;
  if (stat(agents_file, &st)) return;
  long long m = (long long)st.st_mtim.tv_sec * 1000000000LL + st.st_mtim.tv_nsec;
  if (m == agents_mtime) return;
  size_t n;
  char *text = slurp(agents_file, &n);
  if (!text) return;
  agents_mtime = m;
  rh_send(&H, RH_AGENTS_KIND, text, n);
  free(text);
}
// evdev's keys (verse/kms/evdev.h) into the face; the codes are host.js's KEYS.
bool realm_key(void *state, int key) { (void)state; touched = 1; rh_key(&H,key,0,1); return true; }
bool realm_key_up(void *state, int key) { (void)state; touched = 1; rh_key(&H,key,0,0); return true; }

// The spec's files that are the realm's: its uniforms, stores and particles (with their
// capacities, so the buffers hold what they grow to) and the pack's indices.
#ifndef AETHYR_RENDER_TYPED
static const void *file(void *user, const char *name, size_t *size, size_t *cap) {
  (void)user;
  if (!strcmp(name, "u.bin")) { *size = *cap = (size_t)H.uniform_floats * 4; return rh_ptr(H.uniforms); }
  if (!strcmp(name, "hud.bin")) { *size = 16; *cap = (size_t)H.max_vertices * H.floats * 4; return rh_ptr(H.vertices); }
  if (name[0] == 's' && name[1] >= '0' && name[1] <= '9' && !strcmp(name + 2, ".bin")) {
    int k = name[1] - '0';
    *size = coaxial_store_bytes(H.state, k) & ~3u;
    *cap = coaxial_store_cap(k);
    if (*size > *cap) *size = *cap;
    return rh_ptr(coaxial_store(H.state, k));
  }
  if (!strcmp(name, "puffs.bin")) {
    *size = coaxial_particle_bytes(H.state) & ~3u;
    *cap = coaxial_particle_cap();
    if (*cap < 16) *cap = 16;
    if (*size > *cap) *size = *cap;
    return rh_ptr(coaxial_particles(H.state));
  }
  const RhEntry *e = !strcmp(name, "index.bin") ? rh_entry(&H, RH_INDICES) : !strcmp(name, "vertices.bin") ? rh_entry(&H, RH_VERTICES) : NULL;
  if (e) { *size = *cap = e->length; return e->bytes; }
  return NULL;  // the rest from the directory (the layers, the draw list's first frame)
}
#endif

// The pointer: the mice into the face, as host.js's pointer and wheel events, and drawn as a
// dot after the frame's own vertices (fx_vs's 16 floats: the place in device pixels and the
// place in the stroke, its shape, tint and bar; a stroke of length 0 is a round glow).
static void to_pointer(void *user, float x, float y, int phase) { (void)user; touched = 1; rh_pointer(&H,x,y,phase,0,1); }
static void to_wheel(void *user, float dy) { (void)user; touched = 1; rh_wheel(&H,dy); }
static Mice mice;
static int cursor;
static Vkp *window_p;          // the window's presenter (--window): its size, to scale the pointer
static float draw_w, draw_h;
#ifdef AETHYR_RENDER_TYPED
static int64_t session_observed[89];
__attribute__((weak)) int32_t coaxial_field_probe(cx_face_addr,int32_t);
__attribute__((weak)) int64_t rhn_socket_queue_info(int32_t);
__attribute__((weak)) void rhn_socket_pool_shutdown(void);
static void session_snapshot(int row,Vkp *p,RhgRealm *s,uint64_t presented){
 int64_t *q=session_observed+row*16;
 q[0]=producer_calls;q[1]=presented;q[2]=coaxial_net_probe?coaxial_net_probe(H.state,0):-1;q[3]=coaxial_net_probe?coaxial_net_probe(H.state,1):-1;
 q[4]=coaxial_field_probe?coaxial_field_probe(H.state,0):0;q[5]=coaxial_field_probe?coaxial_field_probe(H.state,2):0;
 q[6]=rh_audio_info(&H,0);q[7]=rh_audio_info(&H,23);q[8]=rh_audio_info(&H,9);q[9]=rh_audio_info(&H,30);
 q[10]=p->w;q[11]=p->h;q[12]=rhgr_info(s,1);q[13]=rhg_ledger_info(1);q[14]=rhg_ledger_info(2);q[15]=coaxial_net_probe?coaxial_net_probe(H.state,6):-1;
 if(rhn_socket_queue_info)fprintf(stderr,"vk-realm: window network row=%d local=%d emitted=%lld delivered=%lld waiting=%lld outgoing=%lld\n",row,coaxial_net_probe?coaxial_net_probe(H.state,98):-1,(long long)rhn_socket_queue_info(7),(long long)rhn_socket_queue_info(8),(long long)rhn_socket_queue_info(6),(long long)rhn_socket_queue_info(2));
}
#endif
static int to_text_rect(void *user,float rect[4]){(void)user;if(!text_window||!window_p||!draw_w||!draw_h||!coaxial_field||!coaxial_host_copy_span)return 0;
 cx_face_addr at=coaxial_field(H.state);if(!coaxial_host_copy_span(at,16,16,rh_address_limit()))return 0;float source[4];memcpy(source,rh_ptr(at),sizeof source);
 for(int i=0;i<4;i++)rect[i]=source[i]*(i%2?(float)window_p->h/draw_h:(float)window_p->w/draw_w);return 1;
}
static VkDesk desk;            // --desk: Linux programs on a panel over the realm (vkdesk.h)
// F8: the realm's own screen for the desk, when its face has one (the Grid's: newland-local.coax)
__attribute__((weak)) float coaxial_panel_rect(cx_face_addr state, int32_t k);
// an agent clicked in the realm (the Grid's): the session opens its transcript on the desk
__attribute__((weak)) int32_t coaxial_os_request(cx_face_addr state);
static void os_requests(void) {
  if (!coaxial_os_request || desk.pid <= 0) return;
  int32_t a = coaxial_os_request(H.state);
  if (a < 0) return;
  const char *run = getenv("VERSE_RUN");
  if (!run) return;
  char path[1100]; snprintf(path, sizeof path, "%s/requests", run);
  FILE *f = fopen(path, "a");
  if (f) { if (a >= 1000) fprintf(f, "talk %d\n", (int)(a - 1000)); else fprintf(f, "agent %d\n", (int)a); fclose(f); }
  // show where it opens: the desk's screen in the world, unless the desk already shows
  if (desk.mode == PANEL_CLOSED) desk.mode = verse_panel_next(desk.mode, 66);
  fprintf(stderr, "vk-realm: agent %d's transcript asked for\n", (int)a);
}
static float panel_draw_w, panel_draw_h;
// the world's screen's four corners (the face's k 5..12), in the swapchain's pixels
static int world_rect(int32_t c[8], uint32_t w, uint32_t h) {
  if (!coaxial_panel_rect || coaxial_panel_rect(H.state, 4) <= 0 || panel_draw_w <= 0) return 0;
  float sx = (float)w / panel_draw_w, sy = (float)h / panel_draw_h;
  for (int k = 0; k < 8; k++) c[k] = (int32_t)(coaxial_panel_rect(H.state, 5 + k) * (k % 2 ? sy : sx));
  // what stands in front of the screen, from the realm (its k 13, 14 + 4i ..)
  int n = (int)coaxial_panel_rect(H.state, 13);
  desk.occ_n = 0;
  for (int i = 0; i < n && i < 16; i++) {
    for (int j = 0; j < 4; j++) desk.occ[i][j] = (int32_t)(coaxial_panel_rect(H.state, 14 + 4 * i + j) * (j % 2 ? sy : sx));
    desk.occ_n++;
  }
  return 1;
}
// The pointer to the desk first, in the screen's pixels: 1 when the desk took it.
static int pointer_to_desk(float sx, float sy, int phase) { return vkdesk_pointer(sx, sy, phase == 0 ? 1 : phase == 2 ? 0 : -1); }
static void to_moved(void *user, float x, float y) { (void)user; touched = 1; vkdesk_pointer(x, y, -1); if(coaxial_hover)coaxial_hover(H.state,window_p&&window_p->w?x*draw_w/window_p->w:x,window_p&&window_p->h?y*draw_h/window_p->h:y); }
static int to_extra_button(void *user,float x,float y,uint32_t code,int pressed) {
  (void)user;touched=1;return vkdesk_button(x,y,code,pressed);
}
// the bare screen's mice are in the realm's draw pixels; the desk's panel is in the screen's
static Vkp *screen_p;
static void to_pointer_screen(void *user, float x, float y, int phase) {
  float sx = screen_p && draw_w > 0 ? x * (float)screen_p->w / draw_w : x, sy = screen_p && draw_h > 0 ? y * (float)screen_p->h / draw_h : y;
  if (pointer_to_desk(sx, sy, phase)) { touched = 1; return; }
  to_pointer(user, x, y, phase);
}
static int windowed_now;      // the window's path (to_pointer_window) or the bare screen's
static VkHands hands;         // --hands: the camera's hand as the pointer (vkhands.h)
static void to_pointer_window(void *user, float x, float y, int phase);
// the hand's point, in the screen's pixels, into the path a mouse's would take
static void hand_to(float x, float y, int phase) {
  touched = 1;
  // VERSE_HANDS_LOG: each press and release, and every 30th move, on stderr (for checking a hand)
  static int moves; static int log_on = -1;
  if (log_on < 0) log_on = getenv("VERSE_HANDS_LOG") != NULL;
  if (log_on && (phase != 1 || moves++ % 30 == 0)) fprintf(stderr, "vk-realm: hand %s at %.0f, %.0f\n", phase == 0 ? "down" : phase == 2 ? "up" : "moves", x, y);
  if (windowed_now) { to_pointer_window(NULL, x, y, phase); return; }
  if (pointer_to_desk(x, y, phase)) return;
  float dx = screen_p && screen_p->w ? x * draw_w / (float)screen_p->w : x, dy = screen_p && screen_p->h ? y * draw_h / (float)screen_p->h : y;
  mice.x = dx; mice.y = dy; mice.down = phase != 2;
  to_pointer(NULL, dx, dy, phase);
}
static void to_pointer_window(void *user, float x, float y, int phase) {
  if (pointer_to_desk(x, y, phase)) { touched = 1; return; }
  float sx = window_p && window_p->w ? draw_w / (float)window_p->w : 1, sy = window_p && window_p->h ? draw_h / (float)window_p->h : 1;
  mice.x = x * sx; mice.y = y * sy; mice.down = phase != 2;
  to_pointer(user, mice.x, mice.y, phase);
}
static uint32_t with_cursor(uint32_t n) {
  if (!cursor || H.floats != 16 || n > (uint32_t)H.max_vertices || 6 > (uint32_t)H.max_vertices-n) return n;
  float *v = (float *)rh_ptr(H.vertices) + (size_t)n * 16, r = 14;
  static const float corner[6][2] = {{-1, -1}, {1, -1}, {1, 1}, {-1, -1}, {1, 1}, {-1, 1}};
  for (int i = 0; i < 6; i++, v += 16) {
    float dx = corner[i][0] * r, dy = corner[i][1] * r;
    float one[16] = {mice.x + dx, mice.y + dy, dx, dy, 0, mice.down ? 4.5f : 3.0f, 1, 6, 1, 1, 1, 1, 0, 0, 0, 0};
    memcpy(v, one, sizeof one);
  }
  return n + 6;
}
static uint32_t produce_frame(float width,float height,float scale,float dt){
  producer_calls++;
#ifdef AETHYR_RENDER_TYPED
  // Preserve the producer's requested count for Coax admission. The legacy
  // rh_frame helper clips it and is retained only by the legacy spec path.
  return coaxial_frame(H.state,H.vertices,H.uniforms,width,height,scale,dt,0);
#else
  return rh_frame(&H,width,height,scale,dt);
#endif
}

static uint32_t hud_vertices;   // the last frame's own vertices (rh_frame's count)
#ifndef AETHYR_RENDER_TYPED
typedef struct { VkcBuffer *buf; uint32_t version; size_t cap; } Store;
static Store stores[4];
static VkcBuffer *uniforms, *puffs, *hud;

// The frame's data into the spec, as host.js writes its buffers.
static void update(VkSpec *s) {
  memcpy(uniforms->map, rh_ptr(H.uniforms), (size_t)H.uniform_floats * 4);
  if (hud) memcpy(hud->map, rh_ptr(H.vertices), (size_t)hud_vertices * H.floats * 4);
  vk_spec_let(s, "hud", hud ? hud_vertices : 0);
  vk_spec_let(s, "surface", coaxial_surface_vertices(H.state));   // the water lab's sea
  vk_spec_let(s, "cv", coaxial_caster_vertices(H.state));          // the fixture's casters, when painterly
  vk_spec_let(s, "ci", coaxial_caster_instances(H.state));
  for (int k = 0; k < 4; k++) {
    Store *st = &stores[k];
    if (!st->buf) continue;
    uint32_t version = coaxial_store_version(H.state, k);
    if (version == st->version) continue;
    size_t bytes = coaxial_store_bytes(H.state, k) & ~3u;
    if (bytes > st->cap) bytes = st->cap;
    memcpy(st->buf->map, rh_ptr(coaxial_store(H.state, k)), bytes);
    st->version = version;
  }
  if (puffs) {
    size_t bytes = coaxial_particle_bytes(H.state) & ~3u, cap = coaxial_particle_cap();
    memcpy(puffs->map, rh_ptr(coaxial_particles(H.state)), bytes < cap ? bytes : cap);
  }
  uint32_t all = coaxial_draw_count(H.state), water = coaxial_water_draws(H.state);
  if (water > all) water = all;
  vk_spec_let(s, "all", all);
  vk_spec_let(s, "scene", all - water);
  vk_spec_let(s, "blades", coaxial_grass_instances(H.state));
  vk_spec_let(s, "sparks", coaxial_particle_instances(H.state));
  vk_spec_draws(s, all ? rh_ptr(coaxial_draws(H.state)) : NULL);
}
static void record(VkCommandBuffer cmd, void *user) {
  if(benchmark_queries){vkCmdResetQueryPool(cmd,benchmark_queries,0,2);vkCmdWriteTimestamp(cmd,VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT,benchmark_queries,0);}
  vk_spec_record(user, cmd);
  if(benchmark_queries)vkCmdWriteTimestamp(cmd,VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT,benchmark_queries,1);
}
#else
// The shared executor has completed the frame. Presentation only copies its
// image; it does not interpret or redraw the graph.
static void record(VkCommandBuffer cmd, void *user) { (void)cmd; (void)user; }
#endif

static char *slurp(const char *path, size_t *n) {
  FILE *f = fopen(path, "rb");
  if (!f) return NULL;
  fseek(f, 0, SEEK_END);
  long size = ftell(f);
  fseek(f, 0, SEEK_SET);
  char *p = malloc((size_t)size + 1);
  *n = fread(p, 1, (size_t)size, f);
  fclose(f);
  return p;
}
static float half(uint16_t h) {
  int s = h >> 15, e = (h >> 10) & 31, m = h & 1023;
  float v = e == 0 ? ldexpf((float)m, -24) : e == 31 ? (m ? NAN : INFINITY) : ldexpf(1.0f + m / 1024.0f, e - 15);
  return s ? -v : v;
}
// f32 to binary16, rounding to nearest even (the targets' format), for a film's resolved mean.
static uint16_t to_half(float f) {
  uint32_t x; memcpy(&x, &f, 4);
  uint32_t sign = (x >> 16) & 0x8000, e = (x >> 23) & 255, m = x & 0x7fffff;
  if (e == 255) return (uint16_t)(sign | 0x7c00 | (m ? 0x200 : 0));
  int ex = (int)e - 127 + 15;
  if (ex >= 31) return (uint16_t)(sign | 0x7c00);
  if (ex <= 0) {
    if (ex < -10) return (uint16_t)sign;
    m |= 0x800000;
    int shift = 14 - ex;
    uint32_t h = m >> shift, rest = m & ((1u << shift) - 1), halfway = 1u << (shift - 1);
    if (rest > halfway || (rest == halfway && (h & 1))) h++;
    return (uint16_t)(sign | h);
  }
  uint32_t h = (uint32_t)ex << 10 | m >> 13, rest = m & 0x1fff;
  if (rest > 0x1000 || (rest == 0x1000 && (h & 1))) h++;
  return (uint16_t)(sign | h);
}
static int cmp_double(const void *a, const void *b) { double x = *(const double *)a, y = *(const double *)b; return x < y ? -1 : x > y; }

// Travel (--go FILE): a navigation that opens another realm writes its slug to FILE, and the run
// ends with status 3 so the launcher (verse/gpu/realm.sh) starts that realm; one that is the
// web's (a tip page, a site) is ignored, as before.
static int travelled;
static int went_to(const char *file) {
  char slug[64];
  if (!rh_went(slug, sizeof slug)) return 0;
  FILE *f = fopen(file, "w");
  if (!f) { fprintf(stderr, "vk-realm: cannot write %s\n", file); return 0; }
  fprintf(f, "%s\n", slug);
  fclose(f);
  fprintf(stderr, "vk-realm: it goes to %s\n", slug);
  return 1;
}
// Headless keys, as realm-run's --keys: F:KEY:DOWN,... (KEY a code point or host.js's negative keys).
static void press_keys(const char *keys, int frame) {
  const char *p = keys;
  while (*p) {
    int at = (int)strtol(p, (char **)&p, 10);
    if (*p != ':') return;
    int key = (int)strtol(p + 1, (char **)&p, 10);
    if (*p != ':') return;
    int down = (int)strtol(p + 1, (char **)&p, 10);
    if (at == frame) rh_key(&H,key,0,down);
    if (*p == ',') p++;
  }
}

#ifdef AETHYR_RENDER_TYPED
// Diagnostic physical effects only. The Coax observer admits these raw facts;
// it also verifies the complete SDK/product source fence outside this process.
static void frame_image(RhgRealm *s){if(!rh_http_image_due())return;size_t bytes=0;uint8_t *image=rhgr_jpeg(s,&bytes);rh_http_image_ready(image,bytes);if(image_request_observer){FILE *f=fopen(image_request_observer,"wb");int64_t row[4]={(int64_t)producer_calls,(int64_t)bytes,rh_http_image_due(),image!=NULL};if(f){fwrite(row,sizeof row,1,f);fclose(f);}}free(image);}
static int world_replacement(RhgRealm *s,Vkc *v,uint32_t w,uint32_t h,float scale,char *error,size_t error_bytes){
  VksImage *graph=rhgr_graph(s),*display=rhgr_screen(s);
  if(!graph||!display||graph->format!=VK_FORMAT_R16G16B16A16_SFLOAT||display->format!=VK_FORMAT_R8G8B8A8_UNORM)return 0;
  size_t graph_bytes=(size_t)w*h*8,display_bytes=(size_t)w*h*4;
  uint8_t *before=malloc(graph_bytes),*after=malloc(graph_bytes),*display_before=malloc(display_bytes),*display_after=malloc(display_bytes),*arena=malloc(H.arena_size);
  if(!before||!after||!display_before||!display_after||!arena){free(before);free(after);free(display_before);free(display_after);free(arena);return 0;}
  vks_read(v,graph,before,graph_bytes);vks_read(v,display,display_before,display_bytes);memcpy(arena,H.arena,H.arena_size);
  RhgHandle old=rhgr_vertex_handle(s);int old_live=rhg_ledger_live(&old);uint64_t calls=producer_calls;void *old_arena=H.arena;cx_face_addr old_state=H.state;
  recovery_observed[0]=1;recovery_observed[1]=sizeof(cx_face_addr)*8;recovery_observed[2]=(int64_t)calls;
  recovery_observed[3]=rhgr_info(s,2);recovery_observed[4]=rhgr_info(s,1);recovery_observed[5]=rhgr_info(s,4)>0;
  rh_lifecycle_event(&H,8,now_us(),0);rhgr_drop_gpu(s);recovery_observed[6]=rhg_ledger_info(1);recovery_observed[7]=rhgr_info(s,0);recovery_observed[8]=rhgr_info(s,1);
  vkc_close(v);vkc_open(v);
  int ok=rhgr_device(s,v,error,error_bytes)&&rhgr_prepare(s,w,h,COAX_RenderTextureFormat_rgba8,error,error_bytes)&&rhgr_frame(s,hud_vertices,w,h,0,COAX_RenderTextureFormat_rgba16f,error,error_bytes);
  if(ok){
    vks_read(v,rhgr_graph(s),after,graph_bytes);vks_read(v,rhgr_screen(s),display_after,display_bytes);
    recovery_observed[9]=H.arena==old_arena&&H.state==old_state&&!memcmp(arena,H.arena,H.arena_size);
    recovery_observed[10]=!memcmp(before,after,graph_bytes);recovery_observed[11]=!memcmp(display_before,display_after,display_bytes);
    recovery_observed[12]=producer_calls==calls;recovery_observed[13]=rhgr_info(s,5)>0;recovery_observed[14]=old_live&&!rhg_ledger_live(&old);
    rhgr_deliver_media(s);rh_lifecycle_event(&H,9,now_us(),0);
    ok=rhgr_prepare(s,w,h,COAX_RenderTextureFormat_rgba8,error,error_bytes);
    if(ok){hud_vertices=produce_frame((float)w,(float)h,scale,1.0f/60);ok=rhgr_frame(s,hud_vertices,w,h,0,COAX_RenderTextureFormat_rgba16f,error,error_bytes);if(ok){rhgr_deliver_media(s);rh_after(&H,0);}}
    recovery_observed[15]=ok&&producer_calls==calls+1;recovery_observed[16]=v->validating;recovery_observed[17]=vkc_validation_errors;recovery_observed[18]=vkc_validation_warnings;
  }
  free(before);free(after);free(display_before);free(display_after);free(arena);return ok;
}
#endif

int main(int argc, char **argv) {
  benchmark_begin=now_s();
  if (argc < 3) {
    fprintf(stderr, "usage: vk-realm DIR PACK [--facts FILE] [--extra N,...] [--size WxH] [--scale K] [--golden] [--seconds N] [--demo] [--shot OUT.ppm] [--net DIR] [--origin HOST[:PORT]] [--name NAME] "
                    "[--shot-at S] [--headless FRAMES] [--out OUT.f32] [--page N] [--go FILE] [--keys F:KEY:DOWN,...] [--window] [--fullscreen] [--leave-to VT] [--facts-once FILE] [--activity FILE] [--agents FILE] [--desk DIR [--desk-size WxH] [--desk-mode docked|flat]] [--hands SOCKET]\n");
    return 2;
  }
  const char *dir = argv[1], *pack = argv[2], *facts = NULL, *extra = "", *shot = NULL, *out = NULL,*raw_out=NULL;
  double seconds = 60, shot_at = 4;
  float scale = 1;
  uint32_t w = 0, h = 0;
  int golden = 0, headless = 0, demo = 0, windowed = 0, fullscreen = 0, leave_to = 0, facts_once = 0,audio_muted=0,headless_audio=0;
  const char *desk_dir = NULL, *desk_size = NULL, *desk_mode = NULL, *hands_path = NULL;
  const char *net_dir = NULL, *origin = "openagents.com", *name = "everglade", *go = NULL, *keys = "";
  const char *replay_path=NULL,*benchmark_path=NULL,*host_recovery=NULL,*host_window=NULL;
  int page = 0;
  int sample_capacity = MAX_FRAMES;
  const char *frames_dir = NULL; double fps = 60; int film_start = 0, film_end = -1;
  int film_n = 1, film_position = 1, film_view = -1, film_jitter = 1; long long film_seed = 0;
  double film_shutter = 0, film_fstop = 0, film_focus = 0; const char *film_audio = NULL;
  for (int i = 3; i < argc; i++) {
    const char *a = argv[i], *val = i + 1 < argc ? argv[i + 1] : NULL;
    if (!strcmp(a, "--golden")) golden = 1;
    else if (!strcmp(a, "--demo")) demo = 1;
    else if (!strcmp(a, "--window")) windowed = 1;
    else if (!strcmp(a, "--fullscreen")) windowed = fullscreen = 1;
    else if (!strcmp(a,"--benchmark-no-timestamps")) benchmark_timestamps=0;
    else if (!strcmp(a,"--mute-audio")) audio_muted=1;
    else if (!strcmp(a,"--audio")) headless_audio=1;
    else if (!strcmp(a,"--no-jitter")) film_jitter=0;
    else if (!val) { fprintf(stderr, "vk-realm: %s wants a value\n", a); return 2; }
    else if (!strcmp(a, "--facts")) facts = val, i++;
    else if (!strcmp(a, "--facts-once")) facts = val, facts_once = 1, i++;   // read, then removed: facts that must not stay on disk
    else if (!strcmp(a, "--net")) net_dir = val, i++;
    else if (!strcmp(a, "--origin")) origin = val, i++;
    else if (!strcmp(a, "--name")) name = val, i++;
    else if (!strcmp(a, "--extra")) extra = val, i++;
    else if (!strcmp(a, "--size")) { if (sscanf(val, "%ux%u", &w, &h) != 2) { fprintf(stderr, "vk-realm: --size WxH\n"); return 2; } i++; }
    else if (!strcmp(a, "--scale")) scale = (float)atof(val), i++;
    else if (!strcmp(a, "--seconds")) seconds = atof(val), i++;
    else if (!strcmp(a, "--shot")) shot = val, i++;
    else if (!strcmp(a, "--shot-at")) shot_at = atof(val), i++;
    else if (!strcmp(a, "--headless")) headless = atoi(val), i++;
    else if (!strcmp(a, "--stats-frames")) sample_capacity = (int)verse_panel_sample_capacity(atoi(val)), i++;
    else if (!strcmp(a, "--out")) out = val, i++;
    else if (!strcmp(a, "--raw-out")) raw_out = val, i++;
    else if (!strcmp(a, "--fps")) fps = atof(val), i++;
    else if (!strcmp(a, "--frames-dir")) frames_dir = val, i++;
    else if (!strcmp(a, "--film-start")) film_start = atoi(val), i++;
    else if (!strcmp(a, "--film-end")) film_end = atoi(val), i++;
    else if (!strcmp(a, "--samples")) film_n = atoi(val), i++;
    else if (!strcmp(a, "--shutter")) film_shutter = atof(val), i++;
    else if (!strcmp(a, "--shutter-position")) { film_position = !strcmp(val, "start") ? 0 : !strcmp(val, "center") ? 1 : !strcmp(val, "end") ? 2 : -1; i++; }
    else if (!strcmp(a, "--seed")) film_seed = atoll(val), i++;
    else if (!strcmp(a, "--fstop")) film_fstop = atof(val), i++;
    else if (!strcmp(a, "--focus")) film_focus = atof(val), i++;
    else if (!strcmp(a, "--film-view")) film_view = atoi(val), i++;
    else if (!strcmp(a, "--film-audio")) film_audio = val, i++;
    else if (!strcmp(a, "--page")) page = atoi(val), i++;
    else if (!strcmp(a, "--leave-to")) leave_to = atoi(val), i++;
    else if (!strcmp(a, "--activity")) activity = val, touched = 1, i++;   // the person present: this file's time
    else if (!strcmp(a, "--agents")) agents_file = val, i++;                // the machine's agents, sent as they change
    else if (!strcmp(a, "--desk")) desk_dir = val, i++;                     // Linux programs on a panel (vkdesk.h)
    else if (!strcmp(a, "--desk-size")) desk_size = val, i++;
    else if (!strcmp(a, "--desk-mode")) desk_mode = val, i++;              // how the desk starts: docked or flat (default closed)
    else if (!strcmp(a, "--hands")) hands_path = val, i++;                  // the camera daemon's hands socket: your hand as the pointer  // the boot session: hold this VT, show VT N on leaving
    else if (!strcmp(a, "--go")) go = val, i++;      // travel: another realm's slug written here, and it leaves (status 3)
    else if (!strcmp(a, "--keys")) keys = val, i++;  // headless: realm-run's F:KEY:DOWN, a key held or let go at frame F
    else if (!strcmp(a,"--replay")) replay_path=val,i++;
    else if (!strcmp(a,"--benchmark")) benchmark_path=val,i++;
    else if (!strcmp(a,"--host-recovery")) host_recovery=val,i++;
    else if (!strcmp(a,"--host-image-request")) image_request_observer=val,i++;
    else if (!strcmp(a,"--host-window")) host_window=val,i++;
    else { fprintf(stderr, "vk-realm: unknown argument %s\n", a); return 2; }
  }
  signal(SIGINT, on_signal);
  if (!(fps >= 1 && fps <= 240)) { fprintf(stderr, "vk-realm: --fps must be 1..240\n"); return 2; }
  if (frames_dir && (!headless || film_start < 0 || (film_end >= 0 && (film_end < film_start || film_end >= headless)) || film_start >= headless)) {
    fprintf(stderr, "vk-realm: --frames-dir needs --headless FRAMES and 0 <= --film-start <= --film-end < FRAMES\n"); return 2;
  }
  if (!sample_capacity) { fprintf(stderr, "vk-realm: --stats-frames must be 64..100000\n"); return 2; }
  int film_multi = film_n != 1 || film_shutter != 0 || film_fstop > 0;
  if ((film_multi || film_audio) && !VK_FILM) { fprintf(stderr, "vk-realm: this build has no film profile (metaverse-engine/render/film.coax)\n"); return 2; }
  if ((film_multi || film_audio) && (!frames_dir || fps != (double)(int)fps)) { fprintf(stderr, "vk-realm: the film profile needs --frames-dir and a whole --fps\n"); return 2; }
  if (film_fstop > 0 && !(film_focus > 0)) { fprintf(stderr, "vk-realm: --fstop needs --focus METRES\n"); return 2; }
  if (film_multi && film_view < 0) { fprintf(stderr, "vk-realm: the film profile needs --film-view U, where the realm's view-projection starts in its uniforms\n"); return 2; }
  signal(SIGTERM, on_signal);

  // The realm: as realm-run starts it (the page's facts, the viewer, the pack's parts).
  rh_open(&H, golden);
  if(replay_path&&rh_replay_open(replay_path)){fprintf(stderr,"vk-realm: refused room replay\n");return 1;}
  if(benchmark_path){benchmark_file=fopen(benchmark_path,"w");if(!benchmark_file)return 1;fprintf(benchmark_file,"{\"schema\":\"everglade.native-benchmark.v1\",\"rowColumns\":[\"replay_seconds\",\"frame_interval_ms\",\"controller_river_wall_ms\",\"scene_hud_wall_ms\",\"tail_wall_ms\",\"pre_submit_wall_ms\",\"gpu_span_ms\",\"input_to_gpu_complete_ms\",\"host_encode_cpu_ms\",\"frame_thread_cpu_ms\"],\"rows\":[");if(coaxial_frame_parts)coaxial_frame_parts(H.state,-1);}
  rh_page(&H, page);
#ifndef AETHYR_RENDER_TYPED
  if (rh_pack(&H, pack)) return 1;
#else
  // The compiled Coax asset profile selects the typed route's staged pack.
  // A legacy CLI argument must never substitute another VPK for that URL.
  (void)pack;
#endif
  if (facts) {
    size_t n;
    char *text = slurp(facts, &n);
    if (facts_once) unlink(facts);
    if (!text) { fprintf(stderr, "vk-realm: cannot read %s\n", facts); return 1; }
    if (net_dir) text = online_facts(text, &n, net_dir);
    rh_send(&H, RH_PAGE_KIND, text, n);
    // secret facts: the realm has taken what it keeps; the host's copy and the inbox are wiped
    if (facts_once) { explicit_bzero(text, n); explicit_bzero(rh_ptr(coaxial_inbox(H.state)), n < coaxial_inbox_cap() ? n : coaxial_inbox_cap()); }
    free(text);
  }
  rh_send(&H, RH_VIEWER_KIND, "", 0);
  if (net_dir) {
    // HOST or HOST:PORT; the helper beside this program
    char host[256];
    snprintf(host, sizeof host, "%s", origin);
    int port = 443;
    char *colon = strrchr(host, ':');
    if(colon&&colon[1]=='/')colon=NULL;
    if (colon) { *colon = 0; port = atoi(colon + 1); }
    char helper[1100];
    snprintf(helper, sizeof helper, "%s/net", dir);
    if (access(helper, X_OK)) snprintf(helper, sizeof helper, "./net");
    char *full = realpath(helper, NULL);
    if (!full || rh_net_open(&H, full, host, port, net_dir, name)) return 1;
    fprintf(stderr, "vk-realm: online: %s:%d through %s, kept in %s\n", host, port, full, net_dir);
    free(full);
    if(rh_host_version()!=1)rh_restore(&H);
  }
  uint32_t sizes[8];
  int nextra = 0;
  for (const char *p = extra; *p && nextra < 8;) {
    sizes[nextra++] = (uint32_t)strtoul(p, (char **)&p, 10);
    if (*p == ',') p++;
  }
#ifdef AETHYR_RENDER_TYPED
  if(chdir(dir)){fprintf(stderr,"vk-realm: no directory %s\n",dir);return 1;}
  char asset_error[256];if(!rhgr_deliver(&H,AETHYR_RENDER_PROFILE_ID,asset_error,sizeof asset_error)){fprintf(stderr,"vk-realm: %s\n",asset_error);return 1;}
#else
  rh_deliver(&H, nextra, sizes);
#endif
  if(net_dir&&rh_host_version()==1)rh_restore(&H);
  if(rh_host_version()==1&&(!headless||headless_audio)&&rh_audio_enable(&H,audio_muted)<0)fprintf(stderr,"vk-realm: audio device or bank admission unavailable\n");
  // one frame so the stores exist at their sizes before the spec's buffers are made
  if(rh_host_version()!=1)hud_vertices = produce_frame(320,180,scale,1.0f/60);
#ifndef AETHYR_RENDER_TYPED
  if (chdir(dir)) { fprintf(stderr, "vk-realm: no directory %s\n", dir); return 1; }
#endif

  Vkc v;
  Vkd screen = {0};
  static Vkw win;
  static Vkp p;
  if (headless) vkc_open(&v);
  else if (windowed) {
    // the window's size, or the draw size, or 1600x900; the realm draws at the window's first size
    VkwTo wto = {.pointer=to_pointer_window,.wheel=to_wheel,.moved=to_moved,.key=vkdesk_key,.button=to_extra_button,.focus=to_focus,.shown=to_shown,.realm_key=to_realm_key,.text=to_text,.wants_text=to_wants_text,.text_rect=to_text_rect,.text_owner=to_text_owner,.text_error=to_text_error};
    vkw_open(&v, &win, coaxial_window_title?coaxial_window_title():"Verse", w ? w : 1600, h ? h : 900, fullscreen, wto);
    text_window=&win;rh_set_clipboard(&H,to_clipboard,&win);
    if(coaxial_window_app_id)xdg_toplevel_set_app_id(win.toplevel,coaxial_window_app_id());
    screen.period = win.period;
    vkp_open(&v, &p, win.vk_surface, (uint32_t)win.want_w, (uint32_t)win.want_h);
    if (!w) { w = p.w; h = p.h; }
    draw_w = (float)w; draw_h = (float)h;
  } else {
    if (leave_to) vt_hold();
    vkd_open(&v, &screen, "vk-realm");
    vkp_open(&v, &p, screen.surface, screen.extent.width, screen.extent.height);
    vkd_attach(&screen, &p);
    if (!w) { w = p.w; h = p.h; }
  }
  double t_open = now_s();
#ifdef AETHYR_RENDER_TYPED
  if(!w)w=320;if(!h)h=180;char render_error[256];
  RhgRealm *s=rhgr_open(&v,&H,AETHYR_RENDER_PROFILE_ID,AETHYR_RENDER_PACKAGE_FILE,render_error,sizeof render_error);
  if(!s){fprintf(stderr,"vk-realm: %s\n",render_error);return 1;}
  int nlast=1;VksImage *typed_last[1]={NULL};VksImage **last=typed_last;
  fprintf(stderr,"vk-realm: canonical profile%lld at %ux%u, package made in %.0f ms\n",(long long)AETHYR_RENDER_PROFILE_ID,w,h,(now_s()-t_open)*1000);
#else
  VkSpecFiles files = {file, NULL, w, h};
  VkSpec *s = vk_spec_open(&v, "spec.txt", &files);
  if (!s) return 1;
  w = vk_spec_width(s); h = vk_spec_height(s);
  uniforms = vk_spec_buffer(s, "u");
  puffs = vk_spec_buffer_of(s, "puffs.bin");   // the particles, whatever the module names them
  hud = vk_spec_buffer(s, "hud");
  // store k is the buffer the spec fills from the frame's sK.bin, whatever the shader names it
  for (int k = 0; k < 4; k++) {
    char store_file[16];
    snprintf(store_file, sizeof store_file, "s%d.bin", k);
    stores[k].buf = vk_spec_buffer_of(s, store_file);
    stores[k].version = coaxial_store_version(H.state, k);
    stores[k].cap = coaxial_store_cap(k);
  }
  if (!uniforms) { fprintf(stderr, "vk-realm: the spec has no u\n"); return 1; }
  int nlast;
  VksImage **last = vk_spec_last(s, &nlast);
  fprintf(stderr, "vk-realm: %s, %d passes at %ux%u, made in %.0f ms\n", v.props.deviceName, vk_spec_passes(s), w, h, (now_s() - t_open) * 1000);
#endif

  static double stepped[MAX_FRAMES], took[MAX_FRAMES], presented[MAX_FRAMES];
#ifdef AETHYR_RENDER_TYPED
  if(benchmark_file){fprintf(stderr,"vk-realm: benchmark timestamps require the legacy pass host\n");return 2;}
#endif
  if(benchmark_file&&benchmark_timestamps){VkQueryPoolCreateInfo q={.sType=VK_STRUCTURE_TYPE_QUERY_POOL_CREATE_INFO,.queryType=VK_QUERY_TYPE_TIMESTAMP,.queryCount=2};if(vkCreateQueryPool(v.dev,&q,NULL,&benchmark_queries)!=VK_SUCCESS)return 1;}
  int frames = 0;
  uint64_t live_frames = 0;
  const char *why = "its time was up";
  if(image_request_observer){
    const char *path="/api/host-contract/image",*body="Nostr fixture\n{\"question\":\"é🙂\",\"image\":\"old\"}";
    int id=guard_http(rh_off(path),(uint32_t)strlen(path),rh_off(body),(uint32_t)strlen(body),53,5000);
    if(id<0||!rh_http_image_due()){fprintf(stderr,"vk-realm: image request was not reserved before rendering\n");return 1;}
  }
  if (headless) {
    // every frame drawn and waited for: the realm's step, then the GPU's
    // the frame before the spec was opened is the first of them
    FILE *film_timing = NULL, *film_wav = NULL; uint16_t *film_raw = NULL; float *film_acc = NULL;
    int64_t film_at = 0, film_audio_frames = 0;   // the nanosecond the realm has reached; the sound's frames written
    if (frames_dir) {
      char path[4096];
      snprintf(path, sizeof path, "%s/film.txt", frames_dir);
      FILE *f = fopen(path, "w");
      if (!f) { fprintf(stderr, "vk-realm: cannot write %s\n", path); return 1; }
      fprintf(f, "film-frames-v1\nsize %u %u\ntargets %d\nfps %.9g\nrange %d %d\ndevice %s\n", w, h, nlast, fps, film_start,
              film_end < 0 ? headless - 1 : film_end, v.props.deviceName);
      if (fclose(f)) return 1;
      snprintf(path, sizeof path, "%s/timing.txt", frames_dir);
      film_timing = fopen(path, "w");
      film_raw = malloc((size_t)w * h * 8);
      if (!film_timing || !film_raw) { fprintf(stderr, "vk-realm: cannot write %s\n", path); return 1; }
      fprintf(film_timing, "open %.6f\n", now_s() - benchmark_begin);
      int refused = film_host_admit(film_n, (float)film_shutter, film_position, (int32_t)w, (int32_t)h, nlast);
      if (refused) { fprintf(stderr, "vk-realm: the film profile is refused (metaverse-engine/render/film.coax FilmRefusal %d)\n", refused); return 2; }
      if (film_multi) {
        film_acc = calloc((size_t)w * h * 4 * nlast, sizeof(float));
        if (!film_acc) { fprintf(stderr, "vk-realm: no memory for the film's accumulation\n"); return 1; }
      }
      if (film_audio) {
        if (rh_audio_offline(&H)) { fprintf(stderr, "vk-realm: the realm's mixer is unavailable for --film-audio\n"); return 1; }
        film_wav = fopen(film_audio, "wb");
        if (!film_wav || fwrite("RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x03\0\x02\0\x80\xbb\0\0\0\xdc\x05\0\x08\0\x20\0data\0\0\0\0", 1, 44, film_wav) != 44) { fprintf(stderr, "vk-realm: cannot write %s\n", film_audio); return 1; }
      }
    }
    for (frames = rh_host_version()==1?0:1; frames < headless && frames < MAX_FRAMES && !stopping; frames++) {
      double t0 = now_s();
      if (net_dir) rh_net_poll(&H);
      press_keys(keys, frames);
      int film_in = frames_dir && frames >= film_start && (film_end < 0 || frames <= film_end);
      int samples_now = film_multi && film_in ? film_n : 1;
      if (film_acc && film_in) memset(film_acc, 0, (size_t)w * h * 4 * nlast * sizeof(float));
      for (int sample = 0; sample < samples_now; sample++) {
      // the time this sample is drawn at: the film profile's, else the frame's (1/fps a frame)
      float dt = (float)(1.0 / fps);
      if (film_multi || film_audio) {
        int64_t t = samples_now > 1 || film_shutter > 0 ? film_host_sample_ns(film_n, (float)film_shutter, film_position, film_seed, frames + 1, (int32_t)fps, sample)
                                                         : (int64_t)(frames + 1) * 1000000000LL / (int64_t)fps;
        if (t < film_at) t = film_at;
        dt = (float)((double)(t - film_at) * 1e-9); film_at = t;
      }
#ifdef AETHYR_RENDER_TYPED
      if(!rhgr_prepare(s,w,h,COAX_RenderTextureFormat_rgba8,render_error,sizeof render_error)){fprintf(stderr,"vk-realm: %s\n",render_error);return 1;}
#endif
      hud_vertices = produce_frame((float)w,(float)h,scale,dt);
      if (samples_now > 1 || (film_multi && film_in)) {
        // the sample's place in the pixel and on the aperture, on the realm's own matrix (metaverse-engine/render/film.coax)
        float jx = film_jitter ? film_host_jitter(film_n, (float)film_shutter, film_position, film_seed, frames + 1, sample, 0) : 0;
        float jy = film_jitter ? film_host_jitter(film_n, (float)film_shutter, film_position, film_seed, frames + 1, sample, 1) : 0;
        float lu = film_host_lens(film_n, (float)film_shutter, film_position, film_seed, frames + 1, sample, 0);
        float lv = film_host_lens(film_n, (float)film_shutter, film_position, film_seed, frames + 1, sample, 1);
        if ((size_t)film_view + 16 > (size_t)H.uniform_floats ||
            !film_host_matrix((coax_v_f32){(float *)rh_ptr(H.uniforms) + film_view, 16}, (float)w, (float)h, jx, jy, lu, lv, (float)film_fstop, (float)film_focus)) {
          fprintf(stderr, "vk-realm: --film-view %d is not a view-projection in the realm's %u uniforms\n", film_view, (unsigned)H.uniform_floats); return 1;
        }
      }
#ifdef AETHYR_RENDER_TYPED
      if(!rhgr_frame(s,hud_vertices,w,h,0,COAX_RenderTextureFormat_rgba16f,render_error,sizeof render_error)){fprintf(stderr,"vk-realm: %s\n",render_error);return 1;}last[0]=rhgr_graph(s);frame_image(s);rhgr_deliver_media(s);
#else
      update(s);
#endif
      rh_after(&H, net_dir != NULL);
#ifndef AETHYR_RENDER_TYPED
      if (film_acc && film_in) vks_submit(&v, record, s);
#endif
      if (film_acc && film_in) {
        for (int i = 0; i < nlast; i++) {
          if (!last[i]) { fprintf(stderr, "vk-realm: no completed frame\n"); return 1; }
          vks_read(&v, last[i], film_raw, (size_t)w * h * 8);
          float *a = film_acc + (size_t)w * h * 4 * i;
          for (size_t k = 0; k < (size_t)w * h * 4; k++) a[k] += half(film_raw[k]);
        }
      }
      }
      if (film_wav) {
        // the sound up to this frame's time, from the same clock (48 kHz frames, rounded down)
        int64_t until = (int64_t)(frames + 1) * 48000 / (int64_t)fps, n = until - film_audio_frames;
        float block[2 * 4096];
        while (n > 0) {
          int64_t k = n < 4096 ? n : 4096;
          if (rh_audio_take(&H, block, (size_t)k) || fwrite(block, 8, (size_t)k, film_wav) != (size_t)k) { fprintf(stderr, "vk-realm: the film's sound failed\n"); return 1; }
          n -= k; film_audio_frames += k;
        }
      }
      if (go && went_to(go)) { why = "it went to another realm"; travelled = 1; frames++; break; }
      double t1 = now_s();
#ifndef AETHYR_RENDER_TYPED
      if (!(film_acc && film_in)) vks_submit(&v, record, s);
#endif
      stepped[frames] = t1 - t0;
      took[frames] = now_s() - t1;
      if (frames_dir && frames >= film_start && (film_end < 0 || frames <= film_end)) {
        double t2 = now_s();
        char path[4096];
        snprintf(path, sizeof path, "%s/frame-%06d.rgba16f", frames_dir, frames);
        FILE *f = fopen(path, "wb");
        if (!f) { fprintf(stderr, "vk-realm: cannot write %s\n", path); return 1; }
        for (int i = 0; i < nlast; i++) {
          if (!last[i]) { fprintf(stderr, "vk-realm: no completed frame\n"); return 1; }
          if (film_acc) {
            // the mean of the samples, rounded to half as the targets are
            const float *a = film_acc + (size_t)w * h * 4 * i;
            for (size_t k = 0; k < (size_t)w * h * 4; k++) film_raw[k] = to_half(a[k] / (float)samples_now);
          } else vks_read(&v, last[i], film_raw, (size_t)w * h * 8);
          if (fwrite(film_raw, 8, (size_t)w * h, f) != (size_t)w * h) return 1;
        }
        if (fclose(f)) return 1;
        fprintf(film_timing, "frame %d %.6f %.6f %.6f\n", frames, stepped[frames], took[frames], now_s() - t2);
      }
    }
    if (film_timing && fclose(film_timing)) return 1;
    if (film_wav) {
      // the RIFF and data sizes, now that the frames are known
      uint32_t data = (uint32_t)(film_audio_frames * 8), riff = data + 36;
      if (fseek(film_wav, 4, SEEK_SET) || fwrite(&riff, 4, 1, film_wav) != 1 || fseek(film_wav, 40, SEEK_SET) || fwrite(&data, 4, 1, film_wav) != 1 || fclose(film_wav)) return 1;
    }
    free(film_raw); free(film_acc);
    if (!travelled) why = "it drew its frames";
#ifdef AETHYR_RENDER_TYPED
    if(host_recovery){if(!world_replacement(s,&v,w,h,scale,render_error,sizeof render_error)){fprintf(stderr,"vk-realm: replacement failed: %s\n",render_error);return 1;}last[0]=rhgr_graph(s);}
#endif
    if(raw_out){
      FILE *f=fopen(raw_out,"wb");if(!f){fprintf(stderr,"vk-realm: cannot write raw output\n");return 1;}
      uint16_t *raw=malloc((size_t)w*h*8);if(!raw)return 1;
      for(int i=0;i<nlast;i++){if(!last[i]){fprintf(stderr,"vk-realm: no completed frame\n");return 1;}vks_read(&v,last[i],raw,(size_t)w*h*8);if(fwrite(raw,8,(size_t)w*h,f)!=(size_t)w*h)return 1;}
      free(raw);if(fclose(f))return 1;
    }
    if (out) {
      FILE *f = fopen(out, "wb");
      if (!f) { fprintf(stderr, "vk-realm: cannot write %s\n", out); return 1; }
      uint16_t *raw = malloc((size_t)w * h * 8);
      float *px = malloc((size_t)w * h * 16);
      for (int i = 0; i < nlast; i++) {
        vks_read(&v, last[i], raw, (size_t)w * h * 8);
        for (size_t k = 0; k < (size_t)w * h * 4; k++) px[k] = half(raw[k]);
        fwrite(px, 16, (size_t)w * h, f);
      }
      fclose(f);
      free(raw);free(px);
    }
  } else {
    if (windowed) { window_p = &p; cursor = 1; }
    else {
    keyboards_open();
    keyboards_hold_to_leave = 1.0;   // a press of Escape is the realm's (it closes a field); held a second, it leaves
    mice_open(&mice, (float)w, (float)h);
    cursor = mice.count > 0;
    }
    MiceTo to = {to_pointer_screen, to_wheel, NULL};
    if (!windowed) { screen_p = &p; draw_w = (float)w; draw_h = (float)h; keyboards_hook = vkdesk_key; }
    windowed_now = windowed;
    vkhands_open(&hands, hands_path);
    if (hands_path) cursor = 1;   // the hand's point is drawn, as the mice's is
    if (desk_dir) {
      uint32_t ddw = 1280, ddh = 720;
      if (desk_size) sscanf(desk_size, "%ux%u", &ddw, &ddh);
      if (!vkdesk_open(&v, &desk, desk_dir, ddw, ddh)) {
        // a start in a mode is the toggle's own key pressed once (os/verse-panel.coax decides)
        if (desk_mode && !strcmp(desk_mode, "docked")) desk.mode = verse_panel_next(desk.mode, 67);
        if (desk_mode && !strcmp(desk_mode, "flat")) desk.mode = verse_panel_next(desk.mode, 68);
        // the realm learns it has a desk, so a realm with a screen for it shows that screen
        rh_send(&H, 611, "desk=1\n", 7);
        panel_draw_w = (float)w; panel_draw_h = (float)h; desk.world = world_rect;
        if (desk_mode && !strcmp(desk_mode, "world")) desk.mode = verse_panel_next(desk.mode, 66);
        p.overlay = vkdesk_overlay; p.overlay_user = &desk; fprintf(stderr, "vk-realm: the desk is up (%ux%u); F9 shows it, F10 is flat mode\n", ddw, ddh); }
    }
    if (windowed) fprintf(stderr, "vk-realm: in a %ux%u window%s at %.3f Hz, drawn at %ux%u; closing the window or %.0f s ends it\n", p.w, p.h, fullscreen ? " (full screen)" : "", 1 / screen.period, w, h, seconds);
    else fprintf(stderr, "vk-realm: on %ux%u at %.3f Hz, %d keyboards, %d mice; Escape held a second or %.0f s returns the screen\n", p.w, p.h, 1 / screen.period, keyboard_count,
            mice.count, seconds);
    double start = now_s(), prev = start;
#ifdef AETHYR_RENDER_TYPED
    coax_t_HostLinuxWindowSession session;coax_default_t_HostLinuxWindowSession(&session);int session_hidden=0;
#endif
    while (1) {
      // The bounded timing sample is not a lifetime cap. Keep its recent half in order.
      int keep = (int)verse_panel_sample_keep(frames, sample_capacity);
      if (keep != frames) {
        memmove(stepped, stepped + frames - keep, (size_t)keep * sizeof *stepped);
        memmove(took, took + frames - keep, (size_t)keep * sizeof *took);
        memmove(presented, presented + frames - keep, (size_t)keep * sizeof *presented);
        frames = keep;
      }
      double t = now_s();
      if(benchmark_file&&rh_replay_seconds()>=70)break;
      if (t - start >= seconds) break;
      if (stopping) { why = "a signal stopped it"; break; }
      if (coaxial_should_close && coaxial_should_close(H.state)) { why = "the app requested close"; break; }
      if (windowed) {
        vkw_pump(&win);
        if (win.closed) {
          if (vkdesk_window_close(&desk)) win.closed = 0;
          else { why = "the window was closed"; break; }
        }
        // a new size from the compositor: a new swapchain; the realm keeps drawing at its size
        if (win.want_w > 0 && win.want_h > 0 && ((uint32_t)win.want_w != p.w || (uint32_t)win.want_h != p.h)) {rh_lifecycle_event(&H,1,now_us(),0);vkp_resize(&v, &p, (uint32_t)win.want_w, (uint32_t)win.want_h);}
      } else {
        if (keyboards_read(NULL)) { why = "Escape was held"; break; }
        mice_read(&mice, (float)w, (float)h, &to);
        if (mice.moved) touched = 1;
      }
      if (demo) {
        // walk forward and turn now and then, for a run nobody plays
        int phase = (int)((t - start) / 3) % 3;
        static int held;
        int want = phase == 0 ? 'w' : phase == 1 ? 'd' : 'w';
        if (held != want) { if (held) rh_key(&H,held,0,0);rh_key(&H,want,0,1); held = want; }
      }
      // Platform/network work continues even when the compositor has no
      // image or the preceding submission still owns the mutable buffers.
      touch_activity(t);agents_poll(t);vkdesk_pump(&desk,t);os_requests();
      vkhands_poll(&hands,t,p.w,p.h,hand_to);rh_net_poll(&H);rh_replay_poll(&H);
#ifdef AETHYR_RENDER_TYPED
      if(host_window){session.messages=coaxial_net_probe?coaxial_net_probe(H.state,6):-1;int effect=(int)hlws_offer((coax_v_t_HostLinuxWindowSession){&session,1},(int64_t)((now_s()-start)*1000),(int64_t)producer_calls,coaxial_net_probe?coaxial_net_probe(H.state,0):-1,coaxial_net_probe?coaxial_net_probe(H.state,1):-1);
       if(effect<0){fprintf(stderr,"vk-realm: window schedule refused: phase=%lld started=%lld last=%lld at=%lld now=%lld frames=%llu net=%d seat=%d; fence=%d reserved=%d suspended=%d life=%lld,%lld,%lld,%lld clock=%lld\n",(long long)session.phase,(long long)session.started,(long long)session.last,(long long)session.at,(long long)((now_s()-start)*1000),(unsigned long long)producer_calls,coaxial_net_probe?coaxial_net_probe(H.state,0):-1,coaxial_net_probe?coaxial_net_probe(H.state,1):-1,(int)vkGetFenceStatus(v.dev,p.fence),p.reserved,win.suspended,(long long)rh_lifecycle_info(0),(long long)rh_lifecycle_info(1),(long long)rh_lifecycle_info(2),(long long)rh_lifecycle_info(3),(long long)rh_lifecycle_info(4));why="window diagnostic refused";break;}
       if(effect==1){rh_key(&H,-2,0,1);rh_key(&H,-2,0,0);}
       if(effect==2){rh_key(&H,-2,0,1);rh_key(&H,-2,0,0);rh_key(&H,-1,0,1);rh_key(&H,-1,0,0);session_snapshot(0,&p,s,live_frames);rh_key(&H,'w',0,1);}
       if(effect==3){if(coaxial_field_probe)fprintf(stderr,"vk-realm: window held input: held=%d y=%d busy=%d typing=%d stopped=%d\n",coaxial_field_probe(H.state,10),coaxial_field_probe(H.state,11),coaxial_field_probe(H.state,12),coaxial_field_probe(H.state,13),coaxial_field_probe(H.state,14));rh_key(&H,'w',0,0);}
       if(effect==4){session_snapshot(1,&p,s,live_frames);to_shown(NULL,0);session_hidden=1;session_snapshot(2,&p,s,live_frames);}
       if(effect==5){session_snapshot(3,&p,s,live_frames);to_shown(NULL,1);session_hidden=0;}
       if(effect==6){win.want_w=640;win.want_h=360;}
       if(effect==7){fprintf(stderr,"vk-realm: window availability: fence=%d reserved=%d suspended=%d life=%lld,%lld,%lld,%lld clock=%lld\n",(int)vkGetFenceStatus(v.dev,p.fence),p.reserved,win.suspended,(long long)rh_lifecycle_info(0),(long long)rh_lifecycle_info(1),(long long)rh_lifecycle_info(2),(long long)rh_lifecycle_info(3),(long long)rh_lifecycle_info(4));session_snapshot(4,&p,s,live_frames);size_t jpeg_bytes=0;uint8_t *jpeg=rhgr_jpeg(s,&jpeg_bytes);session_observed[88]=(int64_t)jpeg_bytes;free(jpeg);why="window diagnostic finished";break;}
       if(session_hidden){struct timespec nap={0,2000000};nanosleep(&nap,NULL);continue;}
      }
#endif
      if(windowed&&win.suspended){struct timespec nap={0,2000000};nanosleep(&nap,NULL);continue;}
      int affordable=vkp_reserve(&v,&p);
      if(affordable<0){vkp_resize(&v,&p,p.w,p.h);continue;}
      if(!affordable){struct timespec nap={0,2000000};nanosleep(&nap,NULL);continue;}
#ifdef AETHYR_RENDER_TYPED
      coax_t_RenderTextureFormat physical_format=p.format.format==VK_FORMAT_R8G8B8A8_UNORM?COAX_RenderTextureFormat_rgba8:p.format.format==VK_FORMAT_B8G8R8A8_UNORM?COAX_RenderTextureFormat_bgra8:COAX_RenderTextureFormat_invalid;
      if(!rhgr_prepare(s,w,h,physical_format,render_error,sizeof render_error)){fprintf(stderr,"vk-realm: %s\n",render_error);return 1;}
#endif
      t = now_s();
      static double benchmark_last;
      double benchmark_interval=benchmark_last?t-benchmark_last:0;benchmark_last=t;
      double benchmark_cpu_start=benchmark_file?benchmark_thread_cpu():0;
      double dt = t - prev;
      prev = t;
      if (rh_host_version()!=1&&dt > 0.1) dt = 0.1;
      double replay_s=rh_replay_seconds(),input_started=0;
      static double next_input=40,release_input;
      if(benchmark_file){
        if(coaxial_net_probe&&coaxial_net_probe(H.state,0)!=2){rh_key(&H,-2,0,1);rh_key(&H,-2,0,0);}
        if(replay_s>=next_input&&replay_s<70){input_started=now_s();rh_key(&H,'w',0,1);release_input=replay_s+.04;next_input+=5;}
        if(release_input>0&&replay_s>=release_input){rh_key(&H,'w',0,0);release_input=0;}
      }
      uint32_t requested=0;int offer=rh_frame_at(&H,(float)w,(float)h,scale,now_us(),0,0,&requested);
      if(!offer){struct timespec nap={0,2000000};nanosleep(&nap,NULL);continue;}
      if(offer>0)producer_calls++;
      hud_vertices = with_cursor(offer<0?produce_frame((float)w,(float)h,scale,(float)dt):requested);
#ifdef AETHYR_RENDER_TYPED
      if(!rhgr_frame(s,hud_vertices,w,h,0,COAX_RenderTextureFormat_rgba16f,render_error,sizeof render_error)){fprintf(stderr,"vk-realm: %s\n",render_error);return 1;}last[0]=rhgr_screen(s);frame_image(s);rhgr_deliver_media(s);
#else
      update(s);
#endif
      rh_after(&H, 0);   // keeps the player, place and bar (--net); a copy a bare screen ignores
      if (go && went_to(go)) { why = "it went to another realm"; travelled = 1; break; }
      double t1 = now_s();
      if (shot && !p.shoot && !p.shot_ready && t - start >= shot_at) p.shoot = 1;
      double benchmark_encode_start=benchmark_file?benchmark_thread_cpu():0;
      if (vkp_present(&v, &p, record, s, last[0]->image, w, h, NULL, NULL)) { vkp_resize(&v, &p, p.w, p.h); continue; }
      double benchmark_cpu_end=benchmark_file?benchmark_thread_cpu():0;
      if(benchmark_file){
        // A query pool can still hold the preceding frame's available values
        // until this command buffer executes its reset. Fence this submission
        // before reading either its timestamps or its input completion time.
        if(benchmark_timestamps||input_started||!benchmark_first)vkp_wait(&v,&p);
        uint64_t stamps[2]={0};if(benchmark_timestamps&&vkGetQueryPoolResults(v.dev,benchmark_queries,0,2,sizeof stamps,stamps,sizeof(uint64_t),VK_QUERY_RESULT_64_BIT|VK_QUERY_RESULT_WAIT_BIT)!=VK_SUCCESS)return 1;
        benchmark_first=benchmark_first?benchmark_first:now_s()-benchmark_begin;
        if(replay_s>=40&&replay_s<70){
          if(!benchmark_rows){benchmark_cpu_begin=benchmark_process_cpu();benchmark_replay_begin=replay_s;}benchmark_replay_end=replay_s;
          fprintf(benchmark_file,"%s[%.6f,%.6f,%.6f,%.6f,%.6f,%.6f,%.6f,%.6f,%.6f,%.6f]",benchmark_rows++?",":"",replay_s,benchmark_interval*1000,
            coaxial_frame_parts?coaxial_frame_parts(H.state,0):0,coaxial_frame_parts?coaxial_frame_parts(H.state,1):0,coaxial_frame_parts?coaxial_frame_parts(H.state,2):0,
            (t1-t)*1000,(double)(stamps[1]-stamps[0])*v.props.limits.timestampPeriod/1e6,input_started?(now_s()-input_started)*1000:0,
            (benchmark_cpu_end-benchmark_encode_start)*1000,(benchmark_cpu_end-benchmark_cpu_start)*1000);
        }
        double left=1.0/60-(now_s()-t);if(left>0){struct timespec nap={(time_t)left,(long)((left-(time_t)left)*1e9)};nanosleep(&nap,NULL);}
      }
      stepped[frames] = t1 - t;
      took[frames] = p.waited;
      presented[frames++] = now_s();
      live_frames++;
    }
    vkp_wait(&v, &p);
    if (shot && p.shot_ready) vkd_save_ppm(shot, &p);
  }

  // the realm's step and the GPU's time (headless) or the fence wait (on the screen), as medians and 99th percentiles
  if(benchmark_file){struct rusage usage;getrusage(RUSAGE_SELF,&usage);fprintf(benchmark_file,"],\"firstFrameSeconds\":%.6f,\"peakRSSBytes\":%lld,\"processCPUSeconds\":%.6f,\"measurementSeconds\":%.6f,\"timestampInstrumented\":%s,\"roomSeated\":%s}",benchmark_first,(long long)usage.ru_maxrss*1024,benchmark_process_cpu()-benchmark_cpu_begin,benchmark_replay_end-benchmark_replay_begin,benchmark_timestamps?"true":"false",
    coaxial_net_probe&&coaxial_net_probe(H.state,0)==2?"true":"false");fclose(benchmark_file);if(benchmark_queries)vkDestroyQueryPool(v.dev,benchmark_queries,NULL);}
  if (!headless) printf("vk-realm: lifetime %llu presented frames; timing sample %d frames\n", (unsigned long long)live_frames, frames);
  static double sorted[MAX_FRAMES];
  int skip = frames > 120 ? 60 : headless ? 1 : 0, n = frames - skip;
  if (n > 0) {
    const char *names[2] = {
#ifdef AETHYR_RENDER_TYPED
      "step+render", headless ? "post-render" : "wait"
#else
      "step", headless ? "gpu" : "wait"
#endif
    };
    double *what[2] = {stepped, took};
    for (int k = 0; k < 2; k++) {
      memcpy(sorted, what[k] + skip, (size_t)n * sizeof(double));
      qsort(sorted, (size_t)n, sizeof(double), cmp_double);
      printf("vk-realm: %s median %.2f ms, 99th %.2f ms, longest %.2f ms\n", names[k], sorted[n / 2] * 1000, sorted[n * 99 / 100] * 1000, sorted[n - 1] * 1000);
    }
  }
  if (!headless && n > 1) {
    // a frame is dropped when its present comes a period or more late (FIFO shows each once)
    double period = screen.period, longest = 0;
    int dropped = 0;
    for (int i = skip + 1; i < frames; i++) {
      double gap = presented[i] - presented[i - 1];
      if (gap > longest) longest = gap;
      if (gap > period * 1.5) dropped += (int)(gap / period + 0.5) - 1;
    }
    double ran = presented[frames - 1] - presented[skip];
    printf("vk-realm: %d frames in %.1f s (%.2f a second) at %.3f Hz; dropped %d, longest gap %.1f ms\n", n, ran, (n - 1) / ran, 1 / period, dropped, longest * 1000);
    // the frame times themselves (present to present): the 95th percentile is the play check's
    int g = 0;
    for (int i = skip + 1; i < frames; i++) sorted[g++] = presented[i] - presented[i - 1];
    qsort(sorted, (size_t)g, sizeof(double), cmp_double);
    printf("vk-realm: frame time median %.2f ms, 95th %.2f ms, 99th %.2f ms\n", sorted[g / 2] * 1000, sorted[g * 95 / 100] * 1000, sorted[g * 99 / 100] * 1000);
  }
#ifdef AETHYR_RENDER_TYPED
  printf("vk-realm: %s, %ux%u, %d frames, %llu draws the last; it ended because %s\n",v.props.deviceName,w,h,frames,(unsigned long long)rhgr_draw_commands(s),why);
#else
  printf("vk-realm: %s, %ux%u, %d frames, %u draws the last; it ended because %s\n", v.props.deviceName, w, h, frames, vk_spec_drawn(s), why);
#endif
  if (v.validating) printf("vk-realm: validation: %d errors, %d warnings\n", vkc_validation_errors, vkc_validation_warnings);
#ifdef AETHYR_RENDER_TYPED
  printf("vk-realm: ledger: %lld objects, %lld words live at the last frame (epoch %lld)\n",(long long)rhg_ledger_info(1),(long long)rhg_ledger_info(2),(long long)rhg_ledger_info(0));
  int64_t closed_cpu=rhgr_close(s);
  if(host_window){session_observed[80]=v.validating;session_observed[81]=vkc_validation_errors;session_observed[82]=vkc_validation_warnings;session_observed[83]=rhg_ledger_info(1);session_observed[84]=rhg_ledger_info(2);session_observed[85]=closed_cpu;}
  if(host_recovery){recovery_observed[19]=rhg_ledger_info(1);recovery_observed[20]=rhg_ledger_info(2);recovery_observed[21]=closed_cpu;}
  printf("vk-realm: ledger: %lld objects live after close\n",(long long)rhg_ledger_info(1));
#else
  vk_spec_close(&v, s);
#endif
  if (headless) vkc_close(&v);
  else {
    vkp_close(&v, &p);
    vkdesk_close(&v, &desk);
    vkhands_close(&hands);
    if (windowed) vkw_close(&v, &win);
    else {
      vkd_close(&v, &screen);
      keyboards_close();
      mice_close(&mice);
      // leaving to another realm keeps the VT for the next one
      if (!travelled) vt_leave(leave_to);
    }
  }
  rh_net_close();
  if(coaxial_close)coaxial_close(H.state);
  rh_close(&H);
#ifdef AETHYR_RENDER_TYPED
  // Join process-owned workers before global TLS teardown. Engine retirement
  // alone does not wait for a worker's immutable job and thread-local state.
  if(rhn_socket_pool_shutdown)rhn_socket_pool_shutdown();
#endif
#ifdef AETHYR_RENDER_TYPED
  if(host_window){session_observed[86]=rh_audio_info(&H,42);session_observed[87]=rh_audio_info(&H,43);FILE *f=fopen(host_window,"wb");if(!f)return 1;int ok=fwrite(session_observed,sizeof session_observed,1,f)==1;if(fclose(f)||!ok)return 1;}
  if(host_recovery){FILE *f=fopen(host_recovery,"wb");if(!f)return 1;int ok=fwrite(recovery_observed,sizeof recovery_observed,1,f)==1;if(fclose(f)||!ok)return 1;}
#endif
  return (vkc_validation_errors || vkc_validation_warnings) ? 1 : travelled ? 3 : 0;
}
