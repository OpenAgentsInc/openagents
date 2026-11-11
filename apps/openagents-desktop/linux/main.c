// Firm reason: 6 (the Linux app's process, files and window; every decision is Coax).
// OpenAgents for Linux: the host of the Zeron-fork chat screen (zeron/screen.coax,
// zeron/desk.coax) on the shared Linux host (apps/shared/host/linux/ui-host.h). It is
// what Sources/main.swift and Sources/Zeron.swift are on the Mac, less what Linux
// does not have yet (see docs/apps/2026-11-10-apps-roadmap.md Step 1): it makes the
// sessions, builds each chat's glyph atlas in Coax, hands frames and input across,
// and takes the app's actions.
//
//   OpenAgents [--demo] [--cwd PATH] [--size WxH] [--dpr K] [--seconds N]
//   OpenAgents --shot OUT.png [--size W,H] [--dpr K] [--demo] [--do "STEP ; STEP"]
// Steps: wait SECONDS, fixture open|closed|table, newchat, sidebar, type TEXT,
// send TEXT, click X,Y[,N], move X,Y, wheel DY, key NAME, status, last, desk.
#define _GNU_SOURCE
#include "zeron.h"
#include "face.h"
#include "ui-host.h"
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#define MAX_CHATS 64

typedef struct { void *state; uint8_t *mono, *color; } ChatAtlas;
typedef struct {
  UiHost *host;
  void *desk, *settings, *first;
  char resources[PATH_MAX];
  // the atlas: the fonts' bytes, the plan, the rasterizer's working memory, and each chat's pixels
  coax_t_RaFonts fonts;
  coax_t_RaPlan plan;
  coax_t_RaWork *work;
  float sizes[16], icon_sizes[64];
  int64_t weights[16];
  int nfonts, nicons;
  char *svg[64]; size_t svg_n[64];
  ChatAtlas atlases[MAX_CHATS];
  int natlases;
  void *bound;        // the chat whose atlas the GPU holds
  float dpr;
} App;

static uint8_t *slurp(const char *dir, const char *sub, const char *name, const char *ext, size_t *n) {
  char path[PATH_MAX];
  snprintf(path, sizeof path, "%s/%s/%s%s", dir, sub, name, ext);
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "OpenAgents: missing %s\n", path); return NULL; }
  fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
  uint8_t *p = malloc((size_t)size + 1);
  if (!p || fread(p, 1, (size_t)size, f) != (size_t)size) { fclose(f); free(p); return NULL; }
  fclose(f); p[size] = 0; *n = (size_t)size;
  return p;
}
static void *active(App *a) { void *s = dsk_active(a->desk); return s ? s : a->first; }
static coax_v_t_Atlas atlas_of(void *state) { return (coax_v_t_Atlas){((coax_t_ZeronState *)state)->atlas.e, 1}; }
static coax_v_t_RaPlan plan_of(App *a) { return (coax_v_t_RaPlan){&a->plan, 1}; }
static size_t mono_bytes(App *a) { return (size_t)coaxial_ra_width(plan_of(a)) * (size_t)coaxial_ra_height(plan_of(a)); }
static size_t color_bytes(App *a) { return (size_t)coaxial_ra_width(plan_of(a)) * (size_t)coaxial_ra_color_height(plan_of(a)) * 4; }

// The fonts and icons, read once; the plan for this device scale.
static int atlas_setup(App *a) {
  static const char *const files[4] = {"Geist", "Geist-Medium", "Geist-SemiBold", "GeistMono"};
  for (int w = 0; w < 4; w++) {
    size_t n; uint8_t *bytes = slurp(a->resources, "fonts", files[w], ".ttf", &n);
    if (!bytes || !coaxial_ra_open((coax_v_t_RaFonts){&a->fonts, 1}, w, (coax_v_u8){bytes, (int64_t)n})) { fprintf(stderr, "OpenAgents: %s is not a TrueType font\n", files[w]); return 0; }
  }
  a->nfonts = zr_font_count(); a->nicons = zr_icon_count();
  if (a->nfonts > 16 || a->nicons > 64) return 0;
  for (int k = 0; k < a->nfonts; k++) { a->sizes[k] = zr_font_size(k); a->weights[k] = zr_font_weight(k); }
  for (int i = 0; i < a->nicons; i++) {
    uint8_t name[64]; int n = zr_icon_name(i, name, 63); name[n] = 0;
    a->icon_sizes[i] = zr_icon_size(i);
    a->svg[i] = (char *)slurp(a->resources, "icons", (const char *)name, ".svg", &a->svg_n[i]);
  }
  a->work = calloc(1, sizeof *a->work);
  coaxial_ra_plan(plan_of(a), a->dpr, (coax_v_f32){a->sizes, a->nfonts}, (coax_v_f32){a->icon_sizes, a->nicons});
  return a->work != NULL;
}
// A chat's own atlas (its glyph cache is its own), built when the chat is first shown.
static ChatAtlas *atlas_for(App *a, void *state) {
  for (int i = 0; i < a->natlases; i++) if (a->atlases[i].state == state) return &a->atlases[i];
  if (a->natlases == MAX_CHATS) return NULL;
  ChatAtlas *c = &a->atlases[a->natlases];
  c->state = state; c->mono = calloc(1, mono_bytes(a)); c->color = calloc(1, color_bytes(a));
  if (!c->mono || !c->color) return NULL;
  coax_v_u8 px = {c->mono, (int64_t)mono_bytes(a)};
  coax_v_t_RaWork work = {a->work, 1};
  for (int k = 0; k < a->nfonts; k++) coaxial_ra_font(atlas_of(state), plan_of(a), k, (coax_v_t_RaFonts){&a->fonts, 1}, a->weights[k], a->sizes[k], px, work);
  for (int i = 0; i < a->nicons; i++) if (a->svg[i]) coaxial_ra_icon(atlas_of(state), plan_of(a), i, (coax_string){a->svg[i], (int64_t)a->svg_n[i]}, px, work);
  coaxial_ra_finish(atlas_of(state), plan_of(a));
  a->natlases++;
  return c;
}
static void atlas_upload(App *a, ChatAtlas *c) {
  ui_atlas(a->host, c->mono, (uint32_t)coaxial_ra_width(plan_of(a)), (uint32_t)coaxial_ra_height(plan_of(a)), c->color,
           (uint32_t)coaxial_ra_width(plan_of(a)), (uint32_t)coaxial_ra_color_height(plan_of(a)));
  a->bound = c->state;
}

// Ctrl is what Command is on the Mac: the app's chords are written for the Mac's modifiers (8 is Command).
static int app_mods(int mods) { return (mods & 1) | (mods & 4) | ((mods & (2 | 8)) ? 8 : 0); }

static void copy_selection(App *a) {
  void *state = active(a);
  int n = zr_selected_count(state);
  if (n <= 0) return;
  uint8_t *b = malloc((size_t)n + 1);
  if (b && zr_selected(state, b, n) == n) ui_copy(a->host, (const char *)b, (size_t)n);
  free(b);
}
static void new_session(App *a, const char *cwd) {
  void *chat = zr_new();
  if (!chat) return;
  zr_attach(chat, (const uint8_t *)cwd, (int32_t)strlen(cwd));
  ds_defaults(a->settings, chat);
  dsk_add(a->desk, chat, (const uint8_t *)cwd, (int32_t)strlen(cwd));
}

static int on_frame(void *user, float *vertices, float w, float h, float dpr, float dt) {
  App *a = user;
  void *shown = active(a);
  ChatAtlas *c = atlas_for(a, shown);
  if (!c) return 0;
  if (a->bound != shown) atlas_upload(a, c);
  int n = dsk_frame(a->desk, vertices, w, h, dpr, dt);
  // the graphemes and cells this frame wanted, for the next
  coaxial_ra_clean(plan_of(a));
  if (coaxial_ra_serve(atlas_of(shown), plan_of(a), (coax_v_t_RaFonts){&a->fonts, 1}, (coax_v_s64){a->weights, a->nfonts}, (coax_v_f32){a->sizes, a->nfonts},
                       (coax_v_u8){c->mono, (int64_t)mono_bytes(a)}, (coax_v_t_RaWork){a->work, 1}) > 0) atlas_upload(a, c);
  if (dsk_take_action(a->desk) == 1) {   // New session in a folder: no folder picker yet, so the shown chat's folder
    uint8_t cwd[PATH_MAX]; int m = dsk_cwd(a->desk, cwd, sizeof cwd - 1);
    cwd[m > 0 ? m : 0] = 0;
    new_session(a, m > 0 ? (const char *)cwd : getenv("HOME"));
  }
  int own = dsk_cursor(a->desk);
  ui_cursor(a->host, own >= 0 ? own : zr_cursor(shown));
  // a notice's button: the command to run, said on stderr until Linux opens a terminal for it
  static const char *const commands[6] = {NULL, "claude /login", "curl -fsSL https://claude.ai/install.sh | bash", NULL, "codex login", "npm install -g @openai/codex"};
  int action = zr_take_action(shown);
  if (action > 0 && action < 6 && commands[action]) fprintf(stderr, "OpenAgents: run this in a terminal: %s\n", commands[action]);
  static uint8_t copied[65536];
  int k = zr_take_copy(shown, copied, sizeof copied);
  if (k > 0) ui_copy(a->host, (const char *)copied, (size_t)k);
  return n;
}
static void on_pointer(void *user, float x, float y, int phase, int clicks, int mods) { App *a = user; dsk_pointer(a->desk, x, y, phase, clicks, app_mods(mods)); }
// GPUI's pixel delta: the Mac passes lines times the line height (22.75); a wheel notch here is 100, three lines.
static void on_wheel(void *user, float dy) { App *a = user; dsk_wheel(a->desk, -dy * 0.6825f); }
static int on_wants_text(void *user) { App *a = user; return !dsk_room_shown(a->desk) && zr_wants_text(active(a)); }
static void on_text(void *user, const char *bytes, size_t n) { App *a = user; if (on_wants_text(a)) zr_text(active(a), (const uint8_t *)bytes, (int32_t)n); }
static int on_key(void *user, int key, int mods, int down) {
  App *a = user;
  if (!down || key == -13 || key == -14 || key == 0) return 0;   // a release; Shift and Control by themselves
  void *state = active(a);
  int m = app_mods(mods);
  if (key == -15) key = -13;   // Tab, as the app names it
  if (key < 0) {
    if (dsk_key(a->desk, key, m)) return 1;
    if (dsk_room_shown(a->desk) || !zr_wants_text(state)) return 0;
    return zr_key(state, key, m);
  }
  if (!(m & 8)) return 0;   // plain typing comes as text
  int c = key >= 'A' && key <= 'Z' ? key + 32 : key;
  if (dsk_key(a->desk, c, m)) return 1;
  if (!(m & 4)) {
    if (c == 'c') { copy_selection(a); return 1; }
    if (c == 'x') { if (zr_can_cut(state)) { copy_selection(a); zr_cut(state); } return 1; }
    if (c == 'a') { zr_select_all(state); return 1; }
    if (c == 'z') { if (m & 1) zr_redo(state); else zr_undo(state); return 1; }
    if (c == 'n') { uint8_t cwd[PATH_MAX]; int k = dsk_cwd(a->desk, cwd, sizeof cwd - 1); cwd[k > 0 ? k : 0] = 0; new_session(a, k > 0 ? (const char *)cwd : getenv("HOME")); return 1; }
    if (c == 'w') { if (!dsk_close_tab(a->desk)) return 0; return 1; }
    if (c == 'b') { dsk_toggle(a->desk); return 1; }
  }
  if (dsk_room_shown(a->desk) || !zr_wants_text(state)) return 0;
  return zr_key(state, c, m);
}

// ---- --do: the steps a picture is taken after (Sources/Zeron.swift step)

static void say(const char *verb, void *state, int32_t (*get)(void *, uint8_t *, int32_t)) {
  uint8_t b[4096]; int n = get(state, b, sizeof b - 1);
  b[n > 0 ? n : 0] = 0;
  printf("%s: %s\n", verb, b);
}
static double step(App *a, char *line) {
  while (*line == ' ') line++;
  char *arg = strchr(line, ' ');
  if (arg) *arg++ = 0; else arg = "";
  for (char *e = arg + strlen(arg); e > arg && e[-1] == ' ';) *--e = 0;
  void *state = active(a);
  float n[4] = {0}; int count = sscanf(arg, "%f,%f,%f,%f", &n[0], &n[1], &n[2], &n[3]);
  if (!strcmp(line, "wait")) return atof(arg);
  if (!strcmp(line, "fixture")) zr_fixture(state, !strcmp(arg, "closed") ? 0 : !strcmp(arg, "table") ? 2 : 1);
  else if (!strcmp(line, "newchat")) zr_new_chat(state);
  else if (!strcmp(line, "sidebar")) dsk_toggle(a->desk);
  else if (!strcmp(line, "type")) zr_text(state, (const uint8_t *)arg, (int32_t)strlen(arg));
  else if (!strcmp(line, "send")) { zr_text(state, (const uint8_t *)arg, (int32_t)strlen(arg)); zr_key(state, -2, 0); }
  else if (!strcmp(line, "click") && count >= 2) { int k = count > 2 ? (int)n[2] : 1; dsk_pointer(a->desk, n[0], n[1], 0, k, 0); dsk_pointer(a->desk, n[0], n[1], 2, k, 0); }
  else if (!strcmp(line, "move") && count >= 2) dsk_pointer(a->desk, n[0], n[1], 1, 1, 0);
  else if (!strcmp(line, "wheel") && count >= 1) dsk_wheel(a->desk, n[0]);
  else if (!strcmp(line, "key")) {
    static const char *const names[] = {"escape", "enter", "backspace", "left", "right", "up", "down", "home", "end", NULL};
    for (int i = 0; names[i]; i++) if (!strcmp(arg, names[i])) { if (!dsk_key(a->desk, -1 - i, 0)) zr_key(state, -1 - i, 0); }
  }
  else if (!strcmp(line, "status")) say("status", state, zr_status_text);
  else if (!strcmp(line, "last")) say("last", state, zr_last_text);
  else if (!strcmp(line, "desk")) {
    printf("desk: active=%d tabs=", (int)(dsk_active(a->desk) ? 0 : -1));
    for (int t = 0; t < dsk_tabs(a->desk); t++) printf("%s%d", t ? "," : "", dsk_tab(a->desk, t));
    printf(" sessions=%d collapsed=%d\n", dsk_count(a->desk), dsk_collapsed(a->desk));
  }
  else if (*line) fprintf(stderr, "OpenAgents: unknown step %s\n", line);
  return 0;
}

static const char *value(int argc, char **argv, const char *flag) {
  for (int i = 1; i + 1 < argc; i++) if (!strcmp(argv[i], flag)) return argv[i + 1];
  return NULL;
}
static int has(int argc, char **argv, const char *flag) { for (int i = 1; i < argc; i++) if (!strcmp(argv[i], flag)) return 1; return 0; }

int main(int argc, char **argv) {
  static App a;
  // the resources sit beside the program: fonts/, icons/, zeron.spv
  ssize_t len = readlink("/proc/self/exe", a.resources, sizeof a.resources - 1);
  if (len <= 0) return 1;
  a.resources[len] = 0;
  char *slash = strrchr(a.resources, '/');
  if (slash) *slash = 0;
  char shader[PATH_MAX + 16]; snprintf(shader, sizeof shader, "%s/zeron.spv", a.resources);

  const char *shot = value(argc, argv, "--shot"), *size = value(argc, argv, "--size"), *dpr = value(argc, argv, "--dpr");
  unsigned w = shot ? 2640 : 1320, h = shot ? 1760 : 880;
  if (size && sscanf(size, "%u%*[x,]%u", &w, &h) != 2) { fprintf(stderr, "OpenAgents: --size WxH\n"); return 2; }
  a.dpr = dpr ? (float)atof(dpr) : shot ? 2 : 1;
  if (a.dpr <= 0) a.dpr = 1;

  UiApp app = {.title = "OpenAgents", .app_id = "com.openagents.desktop", .width = w, .height = h, .max_vertices = (uint32_t)zr_max_vertices(), .shader = shader,
               .user = &a, .frame = on_frame, .pointer = on_pointer, .wheel = on_wheel, .key = on_key, .text = on_text, .wants_text = on_wants_text};
  // the sessions first: the seat's first events come while the window opens
  a.desk = dsk_new(); a.settings = ds_new(); a.first = zr_new();
  if (!a.desk || !a.settings || !a.first) return 1;
  a.host = ui_open(&app, shot != NULL);
  if (!a.host) return 1;
  ui_set_dpr(a.host, a.dpr);
  if (!atlas_setup(&a)) return 1;
  const char *cwd = value(argc, argv, "--cwd");
  if (!cwd) cwd = getenv("HOME");
  if (!cwd) cwd = "/";
  if (has(argc, argv, "--still-clock")) dsk_still(a.desk);
  if (has(argc, argv, "--demo")) zr_demo(a.first);
  else { zr_attach(a.first, (const uint8_t *)cwd, (int32_t)strlen(cwd)); ds_defaults(a.settings, a.first); }
  dsk_add(a.desk, a.first, (const uint8_t *)cwd, (int32_t)strlen(cwd));

  int status = 0;
  if (shot) {
    const char *script = value(argc, argv, "--do");
    char *steps = strdup(script ? script : "");
    for (int i = 0; i < 3; i++) ui_shot_frame(a.host, w, h, 1 / 60.0f);
    for (char *s = steps; s && *s;) {
      char *next = strstr(s, " ; ");
      if (next) { *next = 0; next += 3; }
      double wait = step(&a, s);
      // an engine's answer comes in real time: frames a sixtieth apart until the wait is over
      struct timespec nap = {0, 16000000};
      for (double t = 0; t < wait; t += 1 / 60.0) { ui_shot_frame(a.host, w, h, 1 / 60.0f); nanosleep(&nap, NULL); }
      for (int i = 0; i < 3; i++) ui_shot_frame(a.host, w, h, 1 / 60.0f);
      s = next;
    }
    free(steps);
    if (!ui_shot_write(a.host, shot)) { fprintf(stderr, "OpenAgents: could not write %s\n", shot); status = 1; }
  } else {
    const char *seconds = value(argc, argv, "--seconds");
    double limit = seconds ? atof(seconds) : 0;
    struct timespec t0; clock_gettime(CLOCK_MONOTONIC, &t0);
    while (ui_frame(a.host)) {
      if (limit > 0) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); if (t.tv_sec - t0.tv_sec + (t.tv_nsec - t0.tv_nsec) / 1e9 > limit) break; }
    }
  }
  dsk_quit(a.desk);
  ui_close(a.host);
  return status;
}
