// Firm reason: 6 (native GPU carrier: Metal, AppKit and the file system; the page's state, layout and paint are Coax).
// The phone app's screen on the Mac with no Xcode and no simulator (README.md; GitHub #13645).
// It is the iOS app's own carrier: this file includes components/gallery/linux/main.c built the way
// the app builds it (COMPONENTS_EMBEDDED, COMPONENTS_NO_GALLERY and the page's macro), so the page's
// host file, the atlas code and the generated library are the app's. Only the surface differs: the
// Mac's Metal device draws into a texture (or a window with --window) at a phone's size and scale.
//   Preview --shot OUT.png [--size WxH] [--dpr K] [--safe TOP,BOTTOM] [--keyboard H] [--text-scale K]
//           [--reduced] [--text TEXT] [--case FILE] [--tree OUT.txt]
//   Preview --window [--size WxH] [--safe TOP,BOTTOM] [--text-scale K] [--text TEXT] [--case FILE] [--seconds N]
// Sizes are in points. --shot opens no window, no Dock icon and no menu. A case file is one
// `coax drive` operation a line (# notes and blank lines are skipped); see README.md.
#define COMPONENTS_EMBEDDED 1
#define COMPONENTS_NO_GALLERY 1
#include "../../../../components/gallery/linux/main.c"
#include <mach-o/dyld.h>

// ---- the page (the one place that names it; INTEGRATION.md says what the shared app's host gives here)
#ifdef COMPONENTS_CHAT
typedef Chat Page;
#define PAGE_STATE coax_t_ChatPage
#define PAGE_ATLAS(p) ((p)->s->atlas.e)
#define PAGE_MAX_VERTICES() ((uint32_t)chat_max_vertices())
#define PAGE_SCALE() chat_scale()
#define PAGE_FRAME oc_on_frame
#define PAGE_POINTER oc_on_pointer
#define PAGE_DRIVE oc_on_drive
#define PAGE_ENV(p, top, bottom, keyboard, focused, editor, reduced) oc_env(p, top, bottom, keyboard, focused, editor, reduced)
#define PAGE_SET_TEXT(p, text) oc_set_text(p, text)
#define PAGE_ACTIVATE(p, id) oc_activate(p, id)
#define PAGE_MOVING(p) oc_moving(p)
#define PAGE_REFUSED(p) oc_refused(p)
#else
#error "Preview: build with the page's macro (COMPONENTS_CHAT)"
#endif

static const char *pv_value(int argc, char **argv, const char *name) {
  for (int i = 1; i + 1 < argc; i++) if (!strcmp(argv[i], name)) return argv[i + 1];
  return NULL;
}
static int pv_flag(int argc, char **argv, const char *name) {
  for (int i = 1; i < argc; i++) if (!strcmp(argv[i], name)) return 1;
  return 0;
}

typedef struct { Page *p; UiHost *host; uint32_t w, h; float dpr; } Pv;

// One operation. The page answers first; `tap ID` and `type ID TEXT` it does not know go the way the
// phone sends them (a press on the row, the platform editor's whole text).
static int pv_op(Pv *v, const char *op, char *out, int cap) {
  int n = ui_drive(v->host, op, out, cap);
  if (n > 0 && !strncmp(out, "unknown operation", 17)) {
    if (!strncmp(op, "tap ", 4)) n = snprintf(out, (size_t)cap, "%s", PAGE_ACTIVATE(v->p, op + 4) ? "" : "tap: nothing answered");
    else if (!strncmp(op, "type ", 5)) {
      const char *text = strchr(op + 5, ' ');
      n = snprintf(out, (size_t)cap, "%s", PAGE_SET_TEXT(v->p, text ? text + 1 : "") ? "" : "type: the text is too long");
    }
  }
  ui_drive(v->host, "clock +17", (char[64]){0}, 64);
  return n;
}
// A case file's operations, in order. 0 when the file cannot be read.
static int pv_case(Pv *v, const char *path) {
  FILE *f = fopen(path, "r");
  if (!f) { fprintf(stderr, "Preview: cannot read %s\n", path); return 0; }
  static char line[1 << 14], out[1 << 16];
  while (fgets(line, sizeof line, f)) {
    size_t n = strlen(line);
    while (n && (line[n - 1] == '\n' || line[n - 1] == '\r')) line[--n] = 0;
    if (!n || line[0] == '#') continue;
    int k = pv_op(v, line, out, (int)sizeof out - 1);
    out[k < 0 ? 0 : k] = 0;
    if (k > 0 && strncmp(line, "tree", 4) && strncmp(line, "counts", 6)) fprintf(stderr, "Preview: %s -> %s\n", line, out);
  }
  fclose(f);
  return 1;
}
// Frames until the page says nothing is moving (glyphs asked for in one frame are drawn in the next).
static void pv_settle(Pv *v) {
  char out[64];
  for (int i = 0; i < 4 || (i < 240 && PAGE_MOVING(v->p)); i++) ui_drive(v->host, "clock +17", out, sizeof out);
}

int main(int argc, char **argv) {
  const char *shot = pv_value(argc, argv, "--shot"), *size = pv_value(argc, argv, "--size"), *dpr = pv_value(argc, argv, "--dpr");
  const char *safe = pv_value(argc, argv, "--safe"), *keyboard = pv_value(argc, argv, "--keyboard"), *scale = pv_value(argc, argv, "--text-scale");
  const char *text = pv_value(argc, argv, "--text"), *ops = pv_value(argc, argv, "--case"), *tree = pv_value(argc, argv, "--tree");
  int window = pv_flag(argc, argv, "--window");
  if (!shot == !window) { fprintf(stderr, "Preview --shot OUT.png | --window  [--size WxH] [--dpr K] [--safe TOP,BOTTOM] [--keyboard H] [--text-scale K] [--reduced] [--text TEXT] [--case FILE] [--tree OUT.txt]\n"); return 2; }
  unsigned pw = 393, ph = 852;
  if (size && sscanf(size, "%u%*[x,]%u", &pw, &ph) != 2) { fprintf(stderr, "Preview: --size WxH, in points\n"); return 2; }
  float top = 59, bottom = 34;
  if (safe && sscanf(safe, "%f,%f", &top, &bottom) != 2) { fprintf(stderr, "Preview: --safe TOP,BOTTOM, in points\n"); return 2; }
  float k = dpr ? (float)atof(dpr) : 3, zoom = scale ? (float)atof(scale) : 1, kb = keyboard ? (float)atof(keyboard) : 0;
  if (k <= 0 || zoom <= 0) return 2;
  // the glyph atlas holds a phone's 3x and no more: larger text is drawn with fewer pixels a point
  if (k * zoom > 3) k = 3 / zoom;

  static Page page;
  Page *p = &page;
  App *a = &p->app;
  uint32_t cap = sizeof a->resources;
  char exe[PATH_MAX];
  if (_NSGetExecutablePath(exe, &cap) != 0 || !realpath(exe, a->resources)) return 1;
  char *slash = strrchr(a->resources, '/');
  if (slash) *slash = 0;
  const char *given = getenv("UI_RESOURCES");
  if (given && *given) snprintf(a->resources, sizeof a->resources, "%s", given);
  char shader[PATH_MAX + 32]; snprintf(shader, sizeof shader, "%s/components.metal", a->resources);
  p->s = calloc(1, sizeof(PAGE_STATE));
  if (!p->s) return 1;
  a->atlas = PAGE_ATLAS(p); a->max_vertices = PAGE_MAX_VERTICES();
  UiApp app = {.title = "OpenAgents preview", .app_id = "com.openagents.preview", .width = pw, .height = ph, .max_vertices = a->max_vertices, .shader = shader,
               .user = p, .frame = PAGE_FRAME, .pointer = PAGE_POINTER, .drive = PAGE_DRIVE,
               .access_capacity=(uint32_t)chat_access_capacity(),.elements=oc_access_count,.element=oc_access_read,.activate=oc_access_press};
  a->host = ui_open(&app, !window);
  if (!a->host) return 1;
  // large text is drawn as the phone's Display Zoom draws it: fewer points across the same pixels
  if (!window) ui_set_dpr(a->host, k * zoom);
  a->dpr = ui_dpr(a->host) * PAGE_SCALE();
  if (!atlas_setup(a)) return 1;
  void *core=oa_core_open();if(!core)return 1;oc_attach(p,core,"");
#ifdef OA_CONVERSATION_FIXTURE
  // Qualification-only Coax root, absent from the product library.
  extern void conversation_fixture(coax_v_t_ChatPage,coax_v_t_AppCore,int64_t);
  const char *fixture_mode=getenv("OA_CONVERSATION_FIXTURE_MODE");
  conversation_fixture((coax_v_t_ChatPage){p->s,1},(coax_v_t_AppCore){core,1},fixture_mode ? atoi(fixture_mode) : OA_CONVERSATION_FIXTURE);
#endif
  Pv v = {.p = p, .host = a->host, .w = (uint32_t)(pw * k + 0.5f), .h = (uint32_t)(ph * k + 0.5f), .dpr = k};
  // the window's facts are in points of the surface, which the zoom makes larger
  PAGE_ENV(p, top / zoom, bottom / zoom, kb / zoom, kb > 0, 0, pv_flag(argc, argv, "--reduced"));
  int status = 0;
  if (window) {
    // a window on the Mac's own screen scale: the page is live, and the pointer is the finger
    if (text) PAGE_SET_TEXT(p, text);
    const char *seconds = pv_value(argc, argv, "--seconds");
    double limit = seconds ? atof(seconds) : 0;
    struct timespec t0; clock_gettime(CLOCK_MONOTONIC, &t0);
    int played = 0;
    while (ui_frame(a->host)) {
      if (!played && ops) { played = 1; pv_case(&v, ops); }
      if (limit > 0) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); if (t.tv_sec - t0.tv_sec + (t.tv_nsec - t0.tv_nsec) / 1e9 > limit) break; }
    }
  } else {
    ui_agent_clock(a->host, 1);
    ui_shot_frame(a->host, v.w, v.h, 0);
    pv_settle(&v);
    if (text) PAGE_SET_TEXT(p, text);
    if (ops && !pv_case(&v, ops)) status = 1;
    pv_settle(&v);
    static char out[1 << 16];
    char op[PATH_MAX + 16]; snprintf(op, sizeof op, "screenshot %s", shot);
    if (ui_drive(a->host, op, out, sizeof out) > 0) { fprintf(stderr, "Preview: %s\n", out); status = 1; }
    if (tree) {
      int n = ui_drive(a->host, "tree", out, (int)sizeof out - 1);
      FILE *f = fopen(tree, "w");
      if (!f || fwrite(out, 1, n < 0 ? 0 : (size_t)n, f) != (size_t)(n < 0 ? 0 : n)) { fprintf(stderr, "Preview: could not write %s\n", tree); status = 1; }
      if (f) fclose(f);
    }
    if (PAGE_REFUSED(p)) { fprintf(stderr, "Preview: %d frames passed the surface's declared capacity\n", PAGE_REFUSED(p)); status = 1; }
  }
  oa_core_close(core);gallery_free(a);
  return status;
}
