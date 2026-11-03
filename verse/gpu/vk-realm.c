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
// DIR holds spec.txt and its files. --size is what the passes draw at (default: the screen's,
// or the spec's offscreen); on the screen it is scaled to the mode's size. --headless runs
// FRAMES frames at 1/60 s each (as realm-run and wasm-run do), drawing every one but the first, and writes the last frame's targets as
// float RGBA (vk-passes' OUT). Escape held a second, the --seconds limit, SIGINT or SIGTERM end it on the
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
#else
#include "vk-spec.h"
#endif
#include "vkd.h"
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
static volatile sig_atomic_t stopping;
static void on_signal(int s) { (void)s; stopping = 1; }
static double now_s(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return t.tv_sec + t.tv_nsec / 1e9;
}

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
// the file's time is set to now at most once a second, so the session can tell how long the Verse
// has been left alone (os/verse-pylon-policy.coax decides what that means).
static const char *activity;
static volatile int touched;
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
bool realm_key(void *state, int key) { (void)state; touched = 1; coaxial_key(H.state, key, 0); return true; }
bool realm_key_up(void *state, int key) { (void)state; touched = 1; coaxial_key_up(H.state, key, 0); return true; }

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
static void to_pointer(void *user, float x, float y, int phase) { (void)user; touched = 1; coaxial_pointer(H.state, x, y, phase, 0, 1); }
static void to_wheel(void *user, float dy) { (void)user; touched = 1; coaxial_wheel(H.state, dy); }
static Mice mice;
static int cursor;
static Vkp *window_p;          // the window's presenter (--window): its size, to scale the pointer
static float draw_w, draw_h;   // what the realm draws at
static VkDesk desk;            // --desk: Linux programs on a panel over the realm (vkdesk.h)
// F8: the realm's own screen for the desk, when its face has one (the Grid's: newland-local.coax)
__attribute__((weak)) float coaxial_panel_rect(uint32_t state, int32_t k);
static float panel_draw_w, panel_draw_h;
static int world_rect(int32_t r[4], uint32_t w, uint32_t h) {
  if (!coaxial_panel_rect || coaxial_panel_rect(H.state, 4) <= 0 || panel_draw_w <= 0) return 0;
  float sx = (float)w / panel_draw_w, sy = (float)h / panel_draw_h;
  for (int k = 0; k < 4; k++) r[k] = (int32_t)(coaxial_panel_rect(H.state, k) * (k % 2 ? sy : sx));
  return r[2] > r[0] && r[3] > r[1];
}
// The pointer to the desk first, in the screen's pixels: 1 when the desk took it.
static int pointer_to_desk(float sx, float sy, int phase) { return vkdesk_pointer(sx, sy, phase == 0 ? 1 : phase == 2 ? 0 : -1); }
static void to_moved(void *user, float x, float y) { (void)user; touched = 1; vkdesk_pointer(x, y, -1); }
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
static void record(VkCommandBuffer cmd, void *user) { vk_spec_record(user, cmd); }
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
    if (at == frame) { if (down) coaxial_key(H.state, key, 0); else coaxial_key_up(H.state, key, 0); }
    if (*p == ',') p++;
  }
}

int main(int argc, char **argv) {
  if (argc < 3) {
    fprintf(stderr, "usage: vk-realm DIR PACK [--facts FILE] [--extra N,...] [--size WxH] [--scale K] [--golden] [--seconds N] [--demo] [--shot OUT.ppm] [--net DIR] [--origin HOST[:PORT]] [--name NAME] "
                    "[--shot-at S] [--headless FRAMES] [--out OUT.f32] [--page N] [--go FILE] [--keys F:KEY:DOWN,...] [--window] [--fullscreen] [--leave-to VT] [--facts-once FILE] [--activity FILE] [--agents FILE] [--desk DIR [--desk-size WxH] [--desk-mode docked|flat]] [--hands SOCKET]\n");
    return 2;
  }
  const char *dir = argv[1], *pack = argv[2], *facts = NULL, *extra = "", *shot = NULL, *out = NULL,*raw_out=NULL;
  double seconds = 60, shot_at = 4;
  float scale = 1;
  uint32_t w = 0, h = 0;
  int golden = 0, headless = 0, demo = 0, windowed = 0, fullscreen = 0, leave_to = 0, facts_once = 0;
  const char *desk_dir = NULL, *desk_size = NULL, *desk_mode = NULL, *hands_path = NULL;
  const char *net_dir = NULL, *origin = "openagents.com", *name = "everglade", *go = NULL, *keys = "";
  int page = 0;
  for (int i = 3; i < argc; i++) {
    const char *a = argv[i], *val = i + 1 < argc ? argv[i + 1] : NULL;
    if (!strcmp(a, "--golden")) golden = 1;
    else if (!strcmp(a, "--demo")) demo = 1;
    else if (!strcmp(a, "--window")) windowed = 1;
    else if (!strcmp(a, "--fullscreen")) windowed = fullscreen = 1;
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
    else if (!strcmp(a, "--out")) out = val, i++;
    else if (!strcmp(a, "--raw-out")) raw_out = val, i++;
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
    else { fprintf(stderr, "vk-realm: unknown argument %s\n", a); return 2; }
  }
  signal(SIGINT, on_signal);
  signal(SIGTERM, on_signal);

  // The realm: as realm-run starts it (the page's facts, the viewer, the pack's parts).
  rh_open(&H, golden);
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
    char *colon = strchr(host, ':');
    if (colon) { *colon = 0; port = atoi(colon + 1); }
    char helper[1100];
    snprintf(helper, sizeof helper, "%s/net", dir);
    if (access(helper, X_OK)) snprintf(helper, sizeof helper, "./net");
    char *full = realpath(helper, NULL);
    if (!full || rh_net_open(&H, full, host, port, net_dir, name)) return 1;
    fprintf(stderr, "vk-realm: online: %s:%d through %s, kept in %s\n", host, port, full, net_dir);
    free(full);
    rh_restore(&H);
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
  // one frame so the stores exist at their sizes before the spec's buffers are made
  hud_vertices = produce_frame(320,180,scale,1.0f/60);
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
    VkwTo wto = {to_pointer_window, to_wheel, NULL, to_moved, vkdesk_key};
    vkw_open(&v, &win, "Verse", w ? w : 1600, h ? h : 900, fullscreen, wto);
    screen.period = win.period;
    vkp_open(&v, &p, win.vk_surface, (uint32_t)win.want_w, (uint32_t)win.want_h);
    if (!w) { w = p.w; h = p.h; }
    draw_w = (float)w; draw_h = (float)h;
  } else {
    if (leave_to) vt_hold();
    vkd_open(&v, &screen, "vk-realm");
    vkp_open(&v, &p, screen.surface, screen.extent.width, screen.extent.height);
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
  int frames = 0;
  const char *why = "its time was up";
  if (headless) {
    // every frame drawn and waited for: the realm's step, then the GPU's
    // the frame before the spec was opened is the first of them
    for (frames = 1; frames < headless && frames < MAX_FRAMES && !stopping; frames++) {
      double t0 = now_s();
      if (net_dir) rh_net_poll(&H);
      press_keys(keys, frames);
      hud_vertices = produce_frame((float)w,(float)h,scale,1.0f/60);
#ifdef AETHYR_RENDER_TYPED
      if(!rhgr_frame(s,hud_vertices,w,h,0,COAX_RenderTextureFormat_rgba16f,render_error,sizeof render_error)){fprintf(stderr,"vk-realm: %s\n",render_error);return 1;}last[0]=rhgr_screen(s);
#else
      update(s);
#endif
      rh_after(&H, net_dir != NULL);
      if (go && went_to(go)) { why = "it went to another realm"; travelled = 1; frames++; break; }
      double t1 = now_s();
#ifndef AETHYR_RENDER_TYPED
      vks_submit(&v, record, s);
#endif
      stepped[frames] = t1 - t0;
      took[frames] = now_s() - t1;
    }
    if (!travelled) why = "it drew its frames";
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
    while (frames < MAX_FRAMES) {
      double t = now_s();
      if (t - start >= seconds) break;
      if (stopping) { why = "a signal stopped it"; break; }
      if (windowed) {
        vkw_pump(&win);
        if (win.closed) { why = "the window was closed"; break; }
        // a new size from the compositor: a new swapchain; the realm keeps drawing at its size
        if (win.want_w > 0 && win.want_h > 0 && ((uint32_t)win.want_w != p.w || (uint32_t)win.want_h != p.h)) vkp_resize(&v, &p, (uint32_t)win.want_w, (uint32_t)win.want_h);
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
        if (held != want) { if (held) coaxial_key_up(H.state, held, 0); coaxial_key(H.state, want, 0); held = want; }
      }
      vkp_wait(&v, &p);
      t = now_s();
      touch_activity(t);
      agents_poll(t);
      vkdesk_pump(&desk, t);
      vkhands_poll(&hands, t, p.w, p.h, hand_to);
      double dt = t - prev;
      prev = t;
      if (dt > 0.1) dt = 0.1;
      rh_net_poll(&H);   // the answers that came, before the realm's step
      hud_vertices = with_cursor(produce_frame((float)w,(float)h,scale,(float)dt));
#ifdef AETHYR_RENDER_TYPED
      if(!rhgr_frame(s,hud_vertices,w,h,0,COAX_RenderTextureFormat_rgba16f,render_error,sizeof render_error)){fprintf(stderr,"vk-realm: %s\n",render_error);return 1;}last[0]=rhgr_screen(s);
#else
      update(s);
#endif
      rh_after(&H, 0);   // keeps the player, place and bar (--net); a copy a bare screen ignores
      if (go && went_to(go)) { why = "it went to another realm"; travelled = 1; break; }
      double t1 = now_s();
      if (shot && !p.shoot && !p.shot_ready && t - start >= shot_at) p.shoot = 1;
      if (vkp_present(&v, &p, record, s, last[0]->image, w, h, NULL, NULL)) { vkp_resize(&v, &p, p.w, p.h); continue; }
      stepped[frames] = t1 - t;
      took[frames] = p.waited;
      presented[frames++] = now_s();
    }
    vkp_wait(&v, &p);
    if (shot && p.shot_ready) vkd_save_ppm(shot, &p);
  }

  // the realm's step and the GPU's time (headless) or the fence wait (on the screen), as medians and 99th percentiles
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
  rhgr_close(s);
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
  rh_close(&H);
  return (vkc_validation_errors || vkc_validation_warnings) ? 1 : travelled ? 3 : 0;
}
