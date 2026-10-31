// vk-realm: a realm drawn every frame natively by its own shaders (docs/verse/native-gpu.md,
// gpu-realm-host): its Coax built with gcc (verse/native/realm-host.h, the face host.js
// drives) and the passes host.js draws, as the spec everglade-pass --emit writes (the live
// WGSL lowered by coax spirv, the layers decoded in Coax), kept on the GPU (vk-spec.h). Each
// frame the realm runs, its uniforms, stores, particles, draw list and own vertices (the HUD) go into the spec's
// buffers in place, its counts are set, and the spec is recorded and presented.
//   vk-realm DIR PACK [--facts FILE] [--extra N,...] [--size WxH] [--scale K] [--golden]
//            [--seconds N] [--demo] [--shot OUT.ppm] [--shot-at S]       on the screen (vkd.h)
//            [--net DIR] [--origin HOST[:PORT]] [--name NAME]
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
#include "evdev.h"
#include "face.h"
#include "mice.h"
#include "realm-host.h"
#include "vk-spec.h"
#include "vkd.h"
#include <math.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

#define MAX_FRAMES 100000
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
  free(text);
  *n = k;
  return out;
}

// evdev's keys (verse/kms/evdev.h) into the face; the codes are host.js's KEYS.
bool realm_key(void *state, int key) { (void)state; coaxial_key(H.state, key, 0); return true; }
bool realm_key_up(void *state, int key) { (void)state; coaxial_key_up(H.state, key, 0); return true; }

// The spec's files that are the realm's: its uniforms, stores and particles (with their
// capacities, so the buffers hold what they grow to) and the pack's indices.
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

// The pointer: the mice into the face, as host.js's pointer and wheel events, and drawn as a
// dot after the frame's own vertices (fx_vs's 16 floats: the place in device pixels and the
// place in the stroke, its shape, tint and bar; a stroke of length 0 is a round glow).
static void to_pointer(void *user, float x, float y, int phase) { (void)user; coaxial_pointer(H.state, x, y, phase, 0, 1); }
static void to_wheel(void *user, float dy) { (void)user; coaxial_wheel(H.state, dy); }
static Mice mice;
static int cursor;
static uint32_t with_cursor(uint32_t n) {
  if (!cursor || H.floats != 16 || n + 6 > (uint32_t)H.max_vertices) return n;
  float *v = (float *)rh_ptr(H.vertices) + (size_t)n * 16, r = 14;
  static const float corner[6][2] = {{-1, -1}, {1, -1}, {1, 1}, {-1, -1}, {1, 1}, {-1, 1}};
  for (int i = 0; i < 6; i++, v += 16) {
    float dx = corner[i][0] * r, dy = corner[i][1] * r;
    float one[16] = {mice.x + dx, mice.y + dy, dx, dy, 0, mice.down ? 4.5f : 3.0f, 1, 6, 1, 1, 1, 1, 0, 0, 0, 0};
    memcpy(v, one, sizeof one);
  }
  return n + 6;
}

typedef struct { VkcBuffer *buf; uint32_t version; size_t cap; } Store;
static Store stores[4];
static VkcBuffer *uniforms, *puffs, *hud;
static uint32_t hud_vertices;   // the last frame's own vertices (rh_frame's count)

// The frame's data into the spec, as host.js writes its buffers.
static void update(VkSpec *s) {
  memcpy(uniforms->map, rh_ptr(H.uniforms), (size_t)H.uniform_floats * 4);
  if (hud) memcpy(hud->map, rh_ptr(H.vertices), (size_t)hud_vertices * H.floats * 4);
  vk_spec_let(s, "hud", hud ? hud_vertices : 0);
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

int main(int argc, char **argv) {
  if (argc < 3) {
    fprintf(stderr, "usage: vk-realm DIR PACK [--facts FILE] [--extra N,...] [--size WxH] [--scale K] [--golden] [--seconds N] [--demo] [--shot OUT.ppm] [--net DIR] [--origin HOST[:PORT]] [--name NAME] "
                    "[--shot-at S] [--headless FRAMES] [--out OUT.f32]\n");
    return 2;
  }
  const char *dir = argv[1], *pack = argv[2], *facts = NULL, *extra = "", *shot = NULL, *out = NULL;
  double seconds = 60, shot_at = 4;
  float scale = 1;
  uint32_t w = 0, h = 0;
  int golden = 0, headless = 0, demo = 0;
  const char *net_dir = NULL, *origin = "openagents.com", *name = "everglade";
  for (int i = 3; i < argc; i++) {
    const char *a = argv[i], *val = i + 1 < argc ? argv[i + 1] : NULL;
    if (!strcmp(a, "--golden")) golden = 1;
    else if (!strcmp(a, "--demo")) demo = 1;
    else if (!val) { fprintf(stderr, "vk-realm: %s wants a value\n", a); return 2; }
    else if (!strcmp(a, "--facts")) facts = val, i++;
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
    else { fprintf(stderr, "vk-realm: unknown argument %s\n", a); return 2; }
  }
  signal(SIGINT, on_signal);
  signal(SIGTERM, on_signal);

  // The realm: as realm-run starts it (the page's facts, the viewer, the pack's parts).
  rh_open(&H, golden);
  if (rh_pack(&H, pack)) return 1;
  if (facts) {
    size_t n;
    char *text = slurp(facts, &n);
    if (!text) { fprintf(stderr, "vk-realm: cannot read %s\n", facts); return 1; }
    if (net_dir) text = online_facts(text, &n, net_dir);
    rh_send(&H, RH_PAGE_KIND, text, n);
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
  rh_deliver(&H, nextra, sizes);
  // one frame so the stores exist at their sizes before the spec's buffers are made
  hud_vertices = rh_frame(&H, 320, 180, scale, 1.0f / 60);
  if (chdir(dir)) { fprintf(stderr, "vk-realm: no directory %s\n", dir); return 1; }

  Vkc v;
  Vkd screen;
  static Vkp p;
  if (headless) vkc_open(&v);
  else {
    vkd_open(&v, &screen, "vk-realm");
    vkp_open(&v, &p, screen.surface, screen.extent.width, screen.extent.height);
    if (!w) { w = p.w; h = p.h; }
  }
  VkSpecFiles files = {file, NULL, w, h};
  double t_open = now_s();
  VkSpec *s = vk_spec_open(&v, "spec.txt", &files);
  if (!s) return 1;
  w = vk_spec_width(s); h = vk_spec_height(s);
  uniforms = vk_spec_buffer(s, "u");
  puffs = vk_spec_buffer(s, "puffs");
  hud = vk_spec_buffer(s, "hud");
  static const char *store_names[4] = {"heights", "objects", "pose", "water_nodes"};
  for (int k = 0; k < 4; k++) {
    stores[k].buf = vk_spec_buffer(s, store_names[k]);
    stores[k].version = coaxial_store_version(H.state, k);
    stores[k].cap = coaxial_store_cap(k);
  }
  if (!uniforms) { fprintf(stderr, "vk-realm: the spec has no u\n"); return 1; }
  int nlast;
  VksImage **last = vk_spec_last(s, &nlast);
  fprintf(stderr, "vk-realm: %s, %d passes at %ux%u, made in %.0f ms\n", v.props.deviceName, vk_spec_passes(s), w, h, (now_s() - t_open) * 1000);

  static double stepped[MAX_FRAMES], took[MAX_FRAMES], presented[MAX_FRAMES];
  int frames = 0;
  const char *why = "its time was up";
  if (headless) {
    // every frame drawn and waited for: the realm's step, then the GPU's
    // the frame before the spec was opened is the first of them
    for (frames = 1; frames < headless && frames < MAX_FRAMES && !stopping; frames++) {
      double t0 = now_s();
      if (net_dir) rh_net_poll(&H);
      hud_vertices = rh_frame(&H, (float)w, (float)h, scale, 1.0f / 60);
      update(s);
      if (net_dir) rh_after(&H, 1);
      double t1 = now_s();
      vks_submit(&v, record, s);
      stepped[frames] = t1 - t0;
      took[frames] = now_s() - t1;
    }
    why = "it drew its frames";
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
    keyboards_open();
    keyboards_hold_to_leave = 1.0;   // a press of Escape is the realm's (it closes a field); held a second, it leaves
    mice_open(&mice, (float)w, (float)h);
    cursor = mice.count > 0;
    MiceTo to = {to_pointer, to_wheel, NULL};
    fprintf(stderr, "vk-realm: on %ux%u at %.3f Hz, %d keyboards, %d mice; Escape held a second or %.0f s returns the screen\n", p.w, p.h, 1 / screen.period, keyboard_count,
            mice.count, seconds);
    double start = now_s(), prev = start;
    while (frames < MAX_FRAMES) {
      double t = now_s();
      if (t - start >= seconds) break;
      if (stopping) { why = "a signal stopped it"; break; }
      if (keyboards_read(NULL)) { why = "Escape was held"; break; }
      mice_read(&mice, (float)w, (float)h, &to);
      if (demo) {
        // walk forward and turn now and then, for a run nobody plays
        int phase = (int)((t - start) / 3) % 3;
        static int held;
        int want = phase == 0 ? 'w' : phase == 1 ? 'd' : 'w';
        if (held != want) { if (held) coaxial_key_up(H.state, held, 0); coaxial_key(H.state, want, 0); held = want; }
      }
      vkp_wait(&v, &p);
      t = now_s();
      double dt = t - prev;
      prev = t;
      if (dt > 0.1) dt = 0.1;
      rh_net_poll(&H);   // the answers that came, before the realm's step
      hud_vertices = with_cursor(rh_frame(&H, (float)w, (float)h, scale, (float)dt));
      update(s);
      rh_after(&H, 0);   // keeps the player, place and bar (--net); a navigation or a copy a bare screen ignores
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
    const char *names[2] = {"step", headless ? "gpu" : "wait"};
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
  printf("vk-realm: %s, %ux%u, %d frames, %u draws the last; it ended because %s\n", v.props.deviceName, w, h, frames, vk_spec_drawn(s), why);
  if (v.validating) printf("vk-realm: validation: %d errors, %d warnings\n", vkc_validation_errors, vkc_validation_warnings);
  vk_spec_close(&v, s);
  if (headless) vkc_close(&v);
  else {
    vkp_close(&v, &p);
    vkd_close(&v, &screen);
    keyboards_close();
    mice_close(&mice);
  }
  rh_net_close();
  rh_close(&H);
  return vkc_validation_errors ? 1 : 0;
}
