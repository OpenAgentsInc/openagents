// Firm reason: 6 (the chat's process effects, files and atlas; every decision is Coax).
// The OpenAgents chat as a component (chat.h): the Zeron-fork chat screen (zeron/screen.coax,
// zeron/desk.coax) on the shared host (apps/shared/host/linux/ui-host.h: Wayland and Vulkan on
// Linux, apps/shared/host/apple/ui-host.m on the Mac, GitHub #13156). It makes the sessions,
// builds each chat's glyph atlas (in Coax on Linux, with CoreText on the Mac:
// apps/shared/host/apple/ct-atlas.m), hands frames and input across, and takes the chat's actions:
// the open panel, the pasteboard's files and dropped files, the room's socket, and dictation from
// CoreAudio (apps/openagents-cli/dictate-mac.m) on the Mac where Linux runs a recorder.
// The OpenAgents app (main.c) and Everglade's Workshop hold it.
#include "chat-app.h"

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
void *oac_active(App *a) { void *s = dsk_active(a->desk); return s ? s : a->first; }
static coax_v_t_Atlas atlas_of(void *state) { return (coax_v_t_Atlas){((coax_t_ZeronState *)state)->atlas.e, 1}; }
#ifndef __APPLE__
static coax_v_t_RaPlan plan_of(App *a) { return (coax_v_t_RaPlan){&a->plan, 1}; }
static size_t mono_bytes(App *a) { return (size_t)coaxial_ra_width(plan_of(a)) * (size_t)coaxial_ra_height(plan_of(a)); }
static size_t color_bytes(App *a) { return (size_t)coaxial_ra_width(plan_of(a)) * (size_t)coaxial_ra_color_height(plan_of(a)) * 4; }
#endif

#ifdef __APPLE__
static const CtApi ct_api = {zr_glyph_count, zr_glyph_code, zr_font_count, zr_font_size, zr_font_weight, zr_icon_count, zr_icon_size, zr_icon_name,
                             zr_set_font, zr_set_icon, zr_set_atlas, zr_set_dynamic, zr_next_cluster, zr_cluster, zr_measured, zr_next_cell, zr_cell, zr_cell_ready};
static size_t cache_bytes(App *a) { return (size_t)ct_atlas_width(a->ct) * (size_t)(ct_atlas_height(a->ct) - ct_atlas_cache_y0(a->ct)); }
static size_t color_bytes(App *a) { return (size_t)ct_atlas_width(a->ct) * (size_t)ct_atlas_color_height(a->ct) * 4; }
// CoreText's fonts and icons at this device scale; every chat's atlas starts again.
static int atlas_setup(App *a) {
  for (int i = 0; i < a->natlases; i++) { free(a->atlases[i].mono); free(a->atlases[i].color); }
  a->natlases = 0; a->bound = NULL;
  ct_atlas_free(a->ct); free(a->full); a->full = NULL;
  a->ct = ct_atlas_new(&ct_api, a->resources, a->dpr);
  if (!a->ct) return 0;
  size_t n = (size_t)ct_atlas_width(a->ct) * ct_atlas_height(a->ct);
  a->full = malloc(n);
  if (!a->full) return 0;
  memcpy(a->full, ct_atlas_mono(a->ct), n);
  return 1;
}
// A chat's own glyph cache, made when the chat is first shown.
static ChatAtlas *atlas_for(App *a, void *state) {
  for (int i = 0; i < a->natlases; i++) if (a->atlases[i].state == state) return &a->atlases[i];
  if (a->natlases == MAX_CHATS) return NULL;
  ChatAtlas *c = &a->atlases[a->natlases];
  c->state = state; c->mono = calloc(1, cache_bytes(a)); c->color = calloc(1, color_bytes(a));
  if (!c->mono || !c->color) return NULL;
  ct_atlas_bind(a->ct, state);
  a->natlases++;
  return c;
}
static void atlas_upload(App *a, ChatAtlas *c) {
  uint32_t w = ct_atlas_width(a->ct), y0 = ct_atlas_cache_y0(a->ct);
  memcpy(a->full + (size_t)y0 * w, c->mono, cache_bytes(a));
  ui_atlas(a->host, a->full, w, ct_atlas_height(a->ct), c->color, w, ct_atlas_color_height(a->ct));
  a->bound = c->state;
}
// The graphemes and cells the last frame wanted, drawn; the rows that changed, uploaded.
static void atlas_serve(App *a, ChatAtlas *c, void *shown) {
  int y0, y1, cy0, cy1;
  if (ct_atlas_serve(a->ct, shown, c->mono, c->color, &y0, &y1, &cy0, &cy1) <= 0 || a->bound != shown) return;
  uint32_t w = ct_atlas_width(a->ct), top = ct_atlas_cache_y0(a->ct);
  if (y1 > y0) memcpy(a->full + (size_t)y0 * w, c->mono + (size_t)(y0 - (int)top) * w, (size_t)(y1 - y0) * w);
  ui_atlas_rows(a->host, a->full, (uint32_t)y0, (uint32_t)y1, c->color, (uint32_t)cy0, (uint32_t)cy1);
}
#else
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
  // the atlas widens to fit 8,192 rows (the smallest limit of the hosts); past that it would not upload
  if (!coaxial_ra_plan(plan_of(a), a->dpr, (coax_v_f32){a->sizes, a->nfonts}, (coax_v_f32){a->icon_sizes, a->nicons})) {
    fprintf(stderr, "the glyph atlas at %gx does not fit in 8192 by 8192\n", (double)a->dpr); return 0;
  }
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
static void atlas_serve(App *a, ChatAtlas *c, void *shown) {
  coaxial_ra_clean(plan_of(a));
  if (coaxial_ra_serve(atlas_of(shown), plan_of(a), (coax_v_t_RaFonts){&a->fonts, 1}, (coax_v_s64){a->weights, a->nfonts}, (coax_v_f32){a->sizes, a->nfonts},
                       (coax_v_u8){c->mono, (int64_t)mono_bytes(a)}, (coax_v_t_RaWork){a->work, 1}) > 0) atlas_upload(a, c);
}
#endif

// Ctrl is what Command is on the Mac: the app's chords are written for the Mac's modifiers (8 is Command).
static int app_mods(int mods) { return (mods & 1) | (mods & 4) | ((mods & (2 | 8)) ? 8 : 0); }

void oac_copy_selection(App *a) {
  void *state = oac_active(a);
  int n = zr_selected_count(state);
  if (n <= 0) return;
  uint8_t *b = malloc((size_t)n + 1);
  if (b && zr_selected(state, b, n) == n) ui_copy(a->host, (const char *)b, (size_t)n);
  free(b);
}
// ---- dictation (the former Sources/Zeron.swift): the host never submits. The Coax binds a result to the one
// unchanged composer it began in, and Enter and Send stay the only ways to send.

static void cancel_dictation(App *a) {
  if (a->dictation_ticket) { zr_dictation_cancel(a->dictation_chat); a->dictation_ticket = 0; }
  if (a->dictation) dn_cancel(a->dictation);
}
void oac_cancel_dictation_for_edit(App *a) { if (a->dictation_ticket) cancel_dictation(a); }
void oac_sync_dictation(App *a) {
  if (a->dictation_ticket && (a->dictation_chat != oac_active(a) || dsk_room_shown(a->desk))) cancel_dictation(a);
  zr_dictation_chord(oac_active(a), 1);   // Ctrl is what Command is on the Mac: the key to hold is Ctrl+D
  zr_dictation_status(oac_active(a), dsk_dictation_enabled(a->desk), a->dictation_phase, a->dictation_outcome, a->dictation_fault, a->dictation_preparing);
}
static void begin_dictation(App *a, int source) {
  if (!a->dictation) return;
  oac_cancel_dictation_for_edit(a); oac_sync_dictation(a);
  void *origin = oac_active(a);
  uint64_t ticket = dn_begin(a->dictation);
  if (!ticket) return;
  if (zr_dictation_begin(origin, (int64_t)ticket, source)) { a->dictation_ticket = ticket; a->dictation_chat = origin; }
  else dn_cancel(a->dictation);
}
static void on_dictation_state(void *user, int phase, int outcome, int fault, int preparing) {
  App *a = user;
  a->dictation_phase = phase; a->dictation_outcome = outcome; a->dictation_fault = fault; a->dictation_preparing = preparing;
  oac_sync_dictation(a);
  if (a->dictation_ticket && zr_dictation_source(a->dictation_chat) == 0) a->dictation_ticket = 0;
}
static void on_dictation_words(void *user, uint64_t ticket, const uint8_t *text, int n) {
  App *a = user;
  if (!a->dictation_ticket || a->dictation_ticket != ticket) return;
  int current = oac_active(a) == a->dictation_chat && !dsk_room_shown(a->desk);
  int inserted = zr_dictation_apply(a->dictation_chat, (int64_t)ticket, current, text, n);
  a->dictation_ticket = 0;
  a->dictation_phase = 1; a->dictation_outcome = inserted ? 5 : 3;
  if (!inserted) dn_cancel(a->dictation);
  oac_sync_dictation(a);
}
// The user's consent, kept in settings.json (the Mac's Enable Local Dictation menu item).
void oac_set_dictation(App *a, int on) {
  oac_cancel_dictation_for_edit(a);
  dsk_dictation_set(a->desk, on);
  if (a->dictation) dn_enable(a->dictation, dsk_dictation_enabled(a->desk));
  oac_save_settings(a); oac_sync_dictation(a);
}
// Capture needs the window to have the keyboard (zeron/dictation-ui.coax); it ends when it loses it.
static void focus_dictation(App *a, int focused) {
  if (focused == a->focused) return;
  a->focused = focused;
  a->dictation_key = 0;   // a key let go while another window had the keyboard is never heard of
  if (zr_dictation_focus(focused, focused)) { if (a->dictation) dn_activate(a->dictation); }
  else { oac_cancel_dictation_for_edit(a); if (a->dictation) dn_deactivate(a->dictation); }
}

// ---- what is kept between runs (the former Sources/Zeron.swift saveSettings, storeFile, flushStore)

static void make_dirs(const char *file) {   // the folders a file's path names
  char dir[PATH_MAX]; snprintf(dir, sizeof dir, "%s", file);
  for (char *c = dir + 1; *c; c++) if (*c == '/') { *c = 0; mkdir(dir, 0700); *c = '/'; }
}
uint8_t *oac_read_file(const char *path, size_t *n) {
  FILE *f = fopen(path, "rb");
  if (!f) return NULL;
  fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
  uint8_t *p = size >= 0 ? malloc((size_t)size + 1) : NULL;
  if (!p || fread(p, 1, (size_t)size, f) != (size_t)size) { fclose(f); free(p); return NULL; }
  fclose(f); *n = (size_t)size;
  return p;
}
static int store_file(App *a, int i, char *out, size_t cap) {
  uint8_t id[64]; int n = dsk_store_id(a->desk, i, id, sizeof id - 1);
  if (!a->settings_path[0] || n <= 0) return 0;
  id[n] = 0;
  char dir[PATH_MAX]; snprintf(dir, sizeof dir, "%s", a->settings_path);
  char *slash = strrchr(dir, '/');
  if (!slash) return 0;
  *slash = 0;
  return snprintf(out, cap, "%s/sessions/%s.jsonl", dir, id) < (int)cap;
}
// What each session finished since the last call (at quit, force: all it has) goes on the end of its file.
static void flush_store(App *a, int force) {
  static uint8_t out[1 << 22];
  if (!a->settings_path[0]) return;
  for (int i = 0; i < dsk_count(a->desk); i++) {
    int n = dsk_store_take(a->desk, i, force, out, sizeof out);
    char path[PATH_MAX];
    if (n <= 0 || !store_file(a, i, path, sizeof path)) continue;
    make_dirs(path);
    int fd = open(path, O_WRONLY | O_CREAT | O_APPEND, 0600);
    if (fd < 0) continue;
    for (int at = 0; at < n;) { ssize_t k = write(fd, out + at, (size_t)(n - at)); if (k <= 0) break; at += (int)k; }
    close(fd);
  }
}
void oac_save_settings(App *a) {
  static uint8_t out[1 << 20];
  if (!a->settings_path[0]) return;
  float f[4] = {0, 0, (float)a->w, (float)a->h};
#ifdef __APPLE__
  if (!a->host || !ui_window_frame(a->host, f)) { f[2] = (float)a->w / (a->dpr > 0 ? a->dpr : 1); f[3] = (float)a->h / (a->dpr > 0 ? a->dpr : 1); }
#endif
  int n = ds_save(a->desk, f[0], f[1], f[2], f[3], out, sizeof out);
  if (n <= 0) return;
  char tmp[PATH_MAX + 8]; snprintf(tmp, sizeof tmp, "%s.new", a->settings_path);
  make_dirs(tmp);
  int fd = open(tmp, O_WRONLY | O_CREAT | O_TRUNC, 0600);
  if (fd < 0) return;
  int ok = write(fd, out, (size_t)n) == n;
  if (close(fd) == 0 && ok) rename(tmp, a->settings_path); else unlink(tmp);
}
// A session in the desk, with a file of its own when sessions are kept.
static void add_chat(App *a, void *chat, const char *cwd) {
  int i = dsk_add(a->desk, chat, (const uint8_t *)cwd, (int32_t)strlen(cwd));
  if (!a->settings_path[0] || i < 0) return;
  uint8_t r[16]; char id[40];
  FILE *f = fopen("/dev/urandom", "rb");
  if (!f || fread(r, 1, 16, f) != 16) { if (f) fclose(f); return; }
  fclose(f);
  r[6] = (uint8_t)((r[6] & 0x0f) | 0x40); r[8] = (uint8_t)((r[8] & 0x3f) | 0x80);   // a version 4 UUID, as the Mac names them
  snprintf(id, sizeof id, "%02x%02x%02x%02x-%02x%02x-%02x%02x-%02x%02x-%02x%02x%02x%02x%02x%02x", r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8], r[9], r[10], r[11], r[12], r[13], r[14], r[15]);
  dsk_set_store(a->desk, i, (const uint8_t *)id, 36);
}
// Every session as it was at quit, each in its own chat, then the tabs. How many came back.
static int restore(App *a, const char *cwd) {
  size_t n; uint8_t *json = a->settings_path[0] ? oac_read_file(a->settings_path, &n) : NULL;
  if (!json) return 0;
  int count = ds_load(a->settings, a->desk, json, (int32_t)n);
  for (int i = 0; i < count; i++) {
    uint8_t buf[PATH_MAX]; int m = ds_restore_cwd(a->settings, i, buf, sizeof buf - 1);
    buf[m > 0 ? m : 0] = 0;
    const char *dir = m > 0 ? (const char *)buf : cwd;
    void *chat = i == 0 ? a->first : zr_new();
    if (!chat) continue;
    zr_attach(chat, (const uint8_t *)dir, (int32_t)strlen(dir));
    add_chat(a, chat, dir);
    int at = dsk_count(a->desk) - 1;
    ds_restore(a->settings, a->desk, i, at);
    char path[PATH_MAX]; size_t tn; uint8_t *text = store_file(a, at, path, sizeof path) ? oac_read_file(path, &tn) : NULL;
    if (text) { dsk_store_load(a->desk, at, text, (int32_t)tn); free(text); }
  }
  free(json);
  if (count > 0) ds_restore_tabs(a->settings, a->desk, 0);
  return count;
}

// ---- attachments (the former Sources/Zeron.swift attach): a file is copied into the app's uploads folder as
// {id8}-{name}, then handed to the chat with its bytes when it is small enough to send inline.

#define UPLOAD_MAX (24 << 20)
#define INLINE_MAX (5 << 20)
static void hand_over(App *a, const char *path, const char *name, const uint8_t *bytes, size_t n, int failure) {
  zr_attach_file(oac_active(a), (const uint8_t *)path, (int32_t)strlen(path), (const uint8_t *)name, (int32_t)strlen(name), bytes, (int32_t)n, failure);
}
void oac_attach(App *a, const char *path) {
  const char *name = strrchr(path, '/'); name = name ? name + 1 : path;
  struct stat st;
  if (stat(path, &st) || !S_ISREG(st.st_mode)) { hand_over(a, "", name, NULL, 0, 2); return; }   // not a file
  if (st.st_size > UPLOAD_MAX) { hand_over(a, "", name, NULL, 0, 1); return; }                    // too large
  size_t n; uint8_t *bytes = oac_read_file(path, &n);
  if (!bytes) { hand_over(a, "", name, NULL, 0, 3); return; }                                     // unreadable
  // the staged name: [A-Za-z0-9._-] kept and the rest "_", its last 80 characters, "upload" for none
  char kept[128]; size_t len = strlen(name), from = len > 80 ? len - 80 : 0, k = 0;
  for (size_t i = from; i < len; i++) { char c = name[i]; kept[k++] = (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '.' || c == '-' || c == '_' ? c : '_'; }
  kept[k] = 0;
  uint8_t r[4] = {0}; FILE *f = fopen("/dev/urandom", "rb");
  if (f) { if (fread(r, 1, 4, f) != 4) memset(r, 0, 4); fclose(f); }
  const char *home = getenv("HOME");
  char dest[PATH_MAX]; snprintf(dest, sizeof dest, "%s/.openagents/desktop/uploads/%02x%02x%02x%02x-%s", home ? home : "/tmp", r[0], r[1], r[2], r[3], k ? kept : "upload");
  make_dirs(dest);
  int fd = open(dest, O_WRONLY | O_CREAT | O_TRUNC, 0600);
  int ok = fd >= 0 && write(fd, bytes, n) == (ssize_t)n;
  if (fd >= 0 && close(fd)) ok = 0;
  if (!ok) hand_over(a, "", name, NULL, 0, 3);
  else hand_over(a, dest, name, n <= INLINE_MAX ? bytes : NULL, n <= INLINE_MAX ? n : 0, 0);
  free(bytes);
}

void oac_new_session(App *a, const char *cwd) {
  oac_cancel_dictation_for_edit(a);
  void *chat = zr_new();
  if (!chat) return;
  zr_attach(chat, (const uint8_t *)cwd, (int32_t)strlen(cwd));
  ds_defaults(a->settings, chat);
  add_chat(a, chat, cwd);
  oac_save_settings(a);
}

// ---- the room (the former Sources/RoomSocket.swift): the app listens on a Unix socket; each Codex session's MCP
// server (this program run with --mcp-room) connects, sends one JSON line a tool call and reads one back.
// Only bytes move here; the desk (zeron/desk.coax) does the rest.

// This program's own path (the room's MCP server runs it again).
static ssize_t self_path(char *out, size_t cap) {
#ifdef __APPLE__
  char raw[PATH_MAX]; uint32_t size = sizeof raw;
  if (_NSGetExecutablePath(raw, &size) || !realpath(raw, out)) return -1;
  (void)cap;
  return (ssize_t)strlen(out);
#else
  ssize_t n = readlink("/proc/self/exe", out, cap - 1);
  if (n > 0) out[n] = 0;
  return n;
#endif
}
static int room_address(const char *path, struct sockaddr_un *a) {
  memset(a, 0, sizeof *a); a->sun_family = AF_UNIX;
  if (strlen(path) >= sizeof a->sun_path) return 0;
  strcpy(a->sun_path, path);
  return 1;
}
static void room_open(App *a) {
#ifdef __APPLE__
  const char *dir = getenv("TMPDIR");
#else
  const char *dir = getenv("XDG_RUNTIME_DIR");
#endif
  struct sockaddr_un addr;
  a->room_fd = -1;
  for (int i = 0; i < 16; i++) a->room_conns[i] = -1;
  size_t dn = dir && *dir ? strlen(dir) : 0;
  snprintf(a->room_path, sizeof a->room_path, "%.*s/oa-room-%d.sock", (int)(dn && dir[dn - 1] == '/' ? dn - 1 : dn), dn ? dir : "/tmp", (int)getpid());
  if (!dn) snprintf(a->room_path, sizeof a->room_path, "/tmp/oa-room-%d.sock", (int)getpid());
  unlink(a->room_path);
#ifdef __APPLE__
  int fd = socket(AF_UNIX, SOCK_STREAM, 0);
  if (fd >= 0) { fcntl(fd, F_SETFL, O_NONBLOCK); fcntl(fd, F_SETFD, FD_CLOEXEC); }
#else
  int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
#endif
  if (fd < 0 || !room_address(a->room_path, &addr) || bind(fd, (struct sockaddr *)&addr, sizeof addr) || listen(fd, 16)) { if (fd >= 0) close(fd); return; }
  chmod(a->room_path, 0600);
  a->room_fd = fd;
  char exe[PATH_MAX]; ssize_t n = self_path(exe, sizeof exe);
  if (n > 0) dsk_set_room(a->desk, (const uint8_t *)exe, (int32_t)n, (const uint8_t *)a->room_path, (int32_t)strlen(a->room_path));
}
static void room_pump(App *a) {
  if (a->room_fd < 0) return;
  for (;;) {
#ifdef __APPLE__
    int c = accept(a->room_fd, NULL, NULL);
    if (c < 0) break;
    fcntl(c, F_SETFL, O_NONBLOCK); fcntl(c, F_SETFD, FD_CLOEXEC);
    int one = 1; setsockopt(c, SOL_SOCKET, SO_NOSIGPIPE, &one, sizeof one);
#else
    int c = accept4(a->room_fd, NULL, NULL, SOCK_NONBLOCK | SOCK_CLOEXEC);
    if (c < 0) break;
#endif
    int slot = -1;
    for (int i = 0; i < 16; i++) if (a->room_conns[i] < 0) { slot = i; break; }
    if (slot < 0) { close(c); continue; }
    a->room_conns[slot] = c; a->room_n[slot] = 0;
  }
  for (int i = 0; i < 16; i++) {
    int c = a->room_conns[i];
    if (c < 0) continue;
    for (;;) {
      ssize_t n = read(c, a->room_buf[i] + a->room_n[i], sizeof a->room_buf[i] - (size_t)a->room_n[i]);
      if (n == 0 || a->room_n[i] == (int)sizeof a->room_buf[i]) { close(c); a->room_conns[i] = -1; break; }
      if (n < 0) break;
      a->room_n[i] += (int)n;
    }
    for (char *nl; a->room_conns[i] >= 0 && (nl = memchr(a->room_buf[i], 10, (size_t)a->room_n[i]));) {
      int len = (int)(nl - a->room_buf[i]);
      dsk_room_line(a->desk, c, (const uint8_t *)a->room_buf[i], len);
      memmove(a->room_buf[i], nl + 1, (size_t)(a->room_n[i] - len - 1)); a->room_n[i] -= len + 1;
    }
  }
  static uint8_t out[16384];
  for (int32_t conn = -1;;) {
    int n = dsk_room_out(a->desk, &conn, out, sizeof out - 1);
    if (n <= 0) break;
    int known = 0;
    for (int i = 0; i < 16; i++) if (a->room_conns[i] == conn) known = 1;
    if (!known) continue;
    out[n] = 10;
    for (int at = 0; at <= n;) {
#ifdef __APPLE__
      ssize_t k = send(conn, out + at, (size_t)(n + 1 - at), 0);   // SO_NOSIGPIPE is set on the connection
#else
      ssize_t k = send(conn, out + at, (size_t)(n + 1 - at), MSG_NOSIGNAL);
#endif
      if (k > 0) { at += (int)k; continue; }
      if (k < 0 && errno == EAGAIN) { struct timespec nap = {0, 1000000}; nanosleep(&nap, NULL); continue; }
      break;
    }
  }
}
// --mcp-room SOCKET AGENT: Codex's MCP server for the room. Codex writes JSON-RPC on stdin; room_mcp
// (zeron/desk.coax) answers it, or turns a tools/call into a line for the app's socket, whose one-line
// answer room_mcp_result turns into the result Codex reads.
static int mcp_room(const char *path, int agent) {
  static uint8_t out[1 << 20]; static char line[1 << 20], reply[1 << 20];
  struct sockaddr_un addr;
  int fd = socket(AF_UNIX, SOCK_STREAM, 0);
  if (fd >= 0) fcntl(fd, F_SETFD, FD_CLOEXEC);
  if (fd >= 0 && (!room_address(path, &addr) || connect(fd, (struct sockaddr *)&addr, sizeof addr))) { close(fd); fd = -1; }
  FILE *app = fd >= 0 ? fdopen(fd, "r+") : NULL;
  while (fgets(line, sizeof line, stdin)) {
    size_t n = strlen(line);
    while (n && (line[n - 1] == 10 || line[n - 1] == 13)) n--;
    int32_t len = 0;
    int kind = room_mcp((const uint8_t *)line, (int32_t)n, agent, out, sizeof out, &len);
    if (kind == 1) { fwrite(out, 1, (size_t)len, stdout); fputc(10, stdout); fflush(stdout); }
    if (kind != 2) continue;
    const char *answer = "{\"rpc\":null,\"text\":\"The OpenAgents app is not running.\",\"error\":true}";
    if (app) {
      fwrite(out, 1, (size_t)len, app); fputc(10, app); fflush(app);
      answer = fgets(reply, sizeof reply, app) ? reply : "{\"rpc\":null,\"text\":\"The OpenAgents app closed.\",\"error\":true}";
    }
    size_t rn = strlen(answer);
    while (rn && (answer[rn - 1] == 10 || answer[rn - 1] == 13)) rn--;
    int m = room_mcp_result((const uint8_t *)answer, (int32_t)rn, out, sizeof out);
    fwrite(out, 1, (size_t)(m > 0 ? m : 0), stdout); fputc(10, stdout); fflush(stdout);
  }
  return 0;
}

// What the file chooser came back with: files to attach (request 1, or 0 dropped or pasted), or the folder a new session works in (2).
static void on_chosen(void *user, int request, const char *path) { App *a = user; if (request == 2) oac_new_session(a, path); else oac_attach(a, path); }
// New session in a folder: the chooser, opened at the shown chat's folder.
void oac_pick_folder(App *a) {
  uint8_t cwd[PATH_MAX]; int m = dsk_cwd(a->desk, cwd, sizeof cwd - 1);
  cwd[m > 0 ? m : 0] = 0;
#ifdef __APPLE__
  if (!a->offscreen) { ui_choose(a->host, 2, 1, 0, "The folder the new session works in", "New Session", (const char *)cwd); return; }
#else
  if (portal_pick(a->portal, 2, 1, "The folder the new session works in", (const char *)cwd)) return;
#endif
  oac_new_session(a, m > 0 ? (const char *)cwd : getenv("HOME"));
}
// The paperclip: any files, several at once.
static void pick_files(App *a) {
#ifdef __APPLE__
  ui_choose(a->host, 1, 0, 1, "Attach files", NULL, NULL);
#else
  if (!portal_pick(a->portal, 1, 0, "Attach files", NULL)) fprintf(stderr, "OpenAgents: no file chooser on this desktop\n");
#endif
}

#ifdef __APPLE__
void oac_paste(App *a) {
  void *state = oac_active(a);
  oac_cancel_dictation_for_edit(a);
  // files and images are attachments (staged, not typed)
  if (zr_wants_text(state) && ui_paste_files(a->host) > 0) return;
  static char text[1 << 20];
  int n = ui_paste_text(a->host, text, sizeof text);
  if (n <= 0) return;
  zr_unmark(state);
  zr_paste(state, (const uint8_t *)text, n);
}
// The input method over the composer (the former Sources/Zeron.swift's NSTextInputClient): its ranges in UTF-16 units.
void oac_ime_replace(void *user, int from, int to, const char *bytes, size_t n) { App *a = user; oac_cancel_dictation_for_edit(a); zr_replace(oac_active(a), from, to, (const uint8_t *)bytes, (int32_t)n); }
void oac_ime_mark(void *user, int from, int to, const char *bytes, size_t n, int sf, int st) { App *a = user; oac_cancel_dictation_for_edit(a); zr_mark(oac_active(a), from, to, (const uint8_t *)bytes, (int32_t)n, sf, st); }
static void ime_unmark(void *user) { App *a = user; zr_unmark(oac_active(a)); }
static int ime_range(void *user, int which) { App *a = user; return zr_range16(oac_active(a), which); }
static int ime_text(void *user, int from, int to, char *out, int cap) { App *a = user; return zr_text16(oac_active(a), from, to, (uint8_t *)out, cap); }
// the composer's rect is the chat's; the chat sits right of the sidebar
static int ime_rect(void *user, int at, float rect[4]) {
  App *a = user;
  if (!zr_rect16(oac_active(a), at, rect)) return 0;
  rect[0] = (rect[0] + dsk_side(a->desk)) * a->dpr; rect[1] *= a->dpr; rect[2] *= a->dpr; rect[3] *= a->dpr;
  return 1;
}
static int ime_index(void *user, float x, float y) { App *a = user; return zr_index16(oac_active(a), x - dsk_side(a->desk) * a->dpr, y); }
const UiIme oac_ime = {oac_ime_replace, oac_ime_mark, ime_unmark, ime_range, ime_text, ime_rect, ime_index};
#endif

static int on_frame(void *user, float *vertices, float w, float h, float dpr, float dt) {
  App *a = user;
#ifdef __APPLE__
  if (a->quit) ui_quit(a->host);
  // the window moved to a screen of another scale: the glyphs again, at it
  if (!a->offscreen && dpr > 0 && dpr != a->dpr) { a->dpr = dpr; if (!atlas_setup(a)) return 0; }
#endif
  void *shown = oac_active(a);
  ChatAtlas *c = atlas_for(a, shown);
  if (!c) return 0;
  if (a->bound != shown) atlas_upload(a, c);
  a->w = (uint32_t)w; a->h = (uint32_t)h;
  if (a->dictation) {
    if (!a->offscreen) focus_dictation(a, ui_focused(a->host));
    dn_pump(a->dictation);
  }
  oac_sync_dictation(a);
  room_pump(a);
  flush_store(a, 0);
  int n = dsk_frame(a->desk, vertices, w, h, dpr, dt);
  // the graphemes and cells this frame wanted, for the next
  atlas_serve(a, c, shown);
#ifndef __APPLE__
  portal_poll(a->portal, on_chosen, a);
#endif
  if (dsk_take_action(a->desk) == 1) oac_pick_folder(a);   // New session in a folder: the chooser, opened at the shown chat's folder
  int own = dsk_cursor(a->desk);
  ui_cursor(a->host, own >= 0 ? own : zr_cursor(shown));
  // a notice's button: the command to run (in Terminal on the Mac; said on stderr until Linux opens a terminal for it)
  static const char *const commands[6] = {NULL, "claude /login", "curl -fsSL https://claude.ai/install.sh | bash", NULL, "codex login", "npm install -g @openai/codex"};
  int action = zr_take_action(shown);
#ifdef __APPLE__
  if (action > 0 && action < 6 && commands[action] && !a->offscreen) ui_terminal(commands[action]);
#else
  if (action > 0 && action < 6 && commands[action]) fprintf(stderr, "OpenAgents: run this in a terminal: %s\n", commands[action]);
#endif
  if (action == 3) pick_files(a);   // the paperclip
  static uint8_t copied[65536];
  int k = zr_take_copy(shown, copied, sizeof copied);
  if (k > 0) ui_copy(a->host, (const char *)copied, (size_t)k);
  return n;
}
static void on_pointer(void *user, float x, float y, int phase, int clicks, int mods) {
  App *a = user;
  // the microphone under the composer: held, it records, and the pointer does nothing else until it is let go
  int held = a->pointer_down;
  a->pointer_down = phase == 0 || (phase == 1 && held);
  if (phase == 0) {
    if (dsk_dictation_hit(a->desk, x, y)) { begin_dictation(a, 2); return; }
    oac_cancel_dictation_for_edit(a);
  } else if (zr_dictation_source(oac_active(a)) == 2 && (phase == 2 || held)) {
    if (phase == 2 && a->dictation) dn_end(a->dictation);
    return;
  }
  dsk_pointer(a->desk, x, y, phase, clicks, app_mods(mods));
}
// GPUI's pixel delta: the Mac passes lines times the line height (22.75); a wheel notch here is 100, three lines.
static void on_wheel(void *user, float dy) { App *a = user; dsk_wheel(a->desk, -dy * 0.6825f); }
static int on_wants_text(void *user) { App *a = user; return !dsk_room_shown(a->desk) && zr_wants_text(oac_active(a)); }
static void on_text(void *user, const char *bytes, size_t n) { App *a = user; oac_cancel_dictation_for_edit(a); if (on_wants_text(a)) zr_text(oac_active(a), (const uint8_t *)bytes, (int32_t)n); }
static int on_key(void *user, int key, int mods, int down) {
  App *a = user;
  if (key == -13 || key == -14 || key == 0) return 0;   // Shift and Control by themselves
  void *state = oac_active(a);
  int m = app_mods(mods);
  // Dictation's keys first, as the Coax names them (zeron/dictation-ui.coax): AppKit's key code 2 is D and
  // 53 is Escape. It says what a press or a release means: 1 begin, 2 end, 3 cancel, 4 a repeat to swallow.
  int code = key == 'd' || key == 'D' ? 2 : key == -1 ? 53 : -1;
  int repeated = down && code == 2 && a->dictation_key;
  if (code == 2) a->dictation_key = down;
  int action = zr_dictation_key(state, code, m, down, repeated, ((coax_t_ZeronState *)state)->composer.e[0].ed.e[0].marked, dsk_room_shown(a->desk));
  if (!down) {   // a release: nothing else takes one
    if (action == 2 && a->dictation) dn_end(a->dictation);
    return action == 2;
  }
  if (action == 1) { begin_dictation(a, 1); return 1; }
  if (action == 3) { oac_cancel_dictation_for_edit(a); return 1; }
  if (action == 4) return 1;
  oac_cancel_dictation_for_edit(a);
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
    if (c == 'c') { oac_copy_selection(a); return 1; }
#ifdef __APPLE__
    if (c == 'v') { oac_paste(a); return 1; }
#endif
    if (c == 'x') { if (zr_can_cut(state)) { oac_copy_selection(a); zr_cut(state); } return 1; }
    if (c == 'a') { zr_select_all(state); return 1; }
    if (c == 'z') { if (m & 1) zr_redo(state); else zr_undo(state); return 1; }
    if (c == 'n') { uint8_t cwd[PATH_MAX]; int k = dsk_cwd(a->desk, cwd, sizeof cwd - 1); cwd[k > 0 ? k : 0] = 0; oac_new_session(a, k > 0 ? (const char *)cwd : getenv("HOME")); return 1; }
    if (c == 'w') { if (!dsk_close_tab(a->desk)) return 0; return 1; }
    if (c == 'b') { dsk_toggle(a->desk); return 1; }
    if (c == 'd' && (m & 1)) { oac_set_dictation(a, !dsk_dictation_enabled(a->desk)); return 1; }   // Ctrl+Shift+D: dictation on or off
  }
  if (dsk_room_shown(a->desk) || !zr_wants_text(state)) return 0;
  return zr_key(state, c, m);
}

// ---- the component's calls (chat.h)

OaChat *oa_chat_new(const OaChatOptions *o) {
  App *a = calloc(1, sizeof *a);
  if (!a) return NULL;
  snprintf(a->resources, sizeof a->resources, "%s", o->resources ? o->resources : ".");
  snprintf(a->settings_path, sizeof a->settings_path, "%s", o->settings && !o->demo ? o->settings : "");
  a->dpr = o->dpr > 0 ? o->dpr : 1;
  a->offscreen = o->offscreen;
  a->room_fd = -1;
  a->desk = dsk_new(); a->settings = ds_new(); a->first = zr_new();
  if (!a->desk || !a->settings || !a->first) { free(a); return NULL; }
  if (o->skin && !dsk_set_skin(a->desk, (const uint8_t *)o->skin, (int32_t)strlen(o->skin))) {
    fprintf(stderr, "OpenAgents: no skin named %s (zeron, everglade, verse)\n", o->skin); free(a); return NULL;
  }
  if (o->room) room_open(a);
  const char *cwd = o->cwd ? o->cwd : getenv("HOME");
  if (!cwd) cwd = "/";
  if (o->still_clock) dsk_still(a->desk);
  if (o->demo) { zr_demo(a->first); dsk_add(a->desk, a->first, (const uint8_t *)cwd, (int32_t)strlen(cwd)); }
  else if (!restore(a, cwd)) {
    zr_attach(a->first, (const uint8_t *)cwd, (int32_t)strlen(cwd));
    ds_defaults(a->settings, a->first);
    add_chat(a, a->first, cwd);
  }
  // Dictation (the former Sources/Zeron.swift configureDictation): in a window; for a picture, only with a fixture
  // or a stand-in recorder.
  const char *setting = o->dictation_setting;
  if (setting && (!strcmp(setting, "on") || !strcmp(setting, "off"))) { dsk_dictation_set(a->desk, !strcmp(setting, "on")); a->setting_changed = 1; }
  a->dictation_fixture = o->offscreen && o->dictation_fixture;
  a->dictation_clock = 1000;
  // a picture never runs the microphone's recorder: it has the fixture, or a program named in its place
  if (o->dictation && (!o->offscreen || a->dictation_fixture || o->dictation_recorder)) {
    char model[PATH_MAX]; dn_model_dir(model, sizeof model);
    a->dictation = dn_new(a->dictation_fixture, model, o->offscreen ? o->dictation_stand_in : NULL, o->dictation_recorder, (DictationTo){a, on_dictation_state, on_dictation_words});
  }
  if (!a->dictation) a->dictation_fixture = 0;
  return a;
}
int oa_chat_start(OaChat *a, UiHost *host) {
  a->host = host;
#ifdef __APPLE__
  if (!a->offscreen) a->dpr = ui_dpr(host);   // the screen's scale
#endif
  ui_set_dpr(host, a->dpr);
  if (!atlas_setup(a)) return 0;
  if (a->setting_changed) { oac_save_settings(a); a->setting_changed = 0; }   // with the window's place
#ifndef __APPLE__
  if (!a->offscreen) a->portal = portal_open();
#endif
  if (a->dictation) {
    dn_enable(a->dictation, dsk_dictation_enabled(a->desk));
    if (a->offscreen) dn_activate(a->dictation);   // a picture has no keyboard to be given
    oac_sync_dictation(a);
  }
  return 1;
}
uint32_t oa_chat_max_vertices(void) { return (uint32_t)zr_max_vertices(); }
int oa_chat_frame(OaChat *a, float *vertices, float w, float h, float dpr, float dt) { return on_frame(a, vertices, w, h, dpr, dt); }
void oa_chat_pointer(OaChat *a, float x, float y, int phase, int clicks, int mods) { on_pointer(a, x, y, phase, clicks, mods); }
void oa_chat_wheel(OaChat *a, float dy) { on_wheel(a, dy); }
int oa_chat_key(OaChat *a, int key, int mods, int down) { return on_key(a, key, mods, down); }
void oa_chat_text(OaChat *a, const char *bytes, size_t n) { on_text(a, bytes, n); }
int oa_chat_wants_text(OaChat *a) { return on_wants_text(a); }
void oa_chat_file(OaChat *a, int request, const char *path) { on_chosen(a, request, path); }
int oa_chat_live(OaChat *a) { return zr_live(oac_active(a)) != 0; }
void oa_chat_close(OaChat *a) {
  if (!a) return;
  oac_cancel_dictation_for_edit(a);
  dn_close(a->dictation); a->dictation = NULL;
  flush_store(a, 1);
  oac_save_settings(a);
#ifndef __APPLE__
  portal_close(a->portal); a->portal = NULL;
#endif
  dsk_quit(a->desk);
  if (a->room_fd >= 0) { close(a->room_fd); unlink(a->room_path); a->room_fd = -1; }
}
int oa_chat_mcp_room(const char *path, int agent) { return mcp_room(path, agent); }

// ---- coax drive (apps/shared/host/drive.c; docs/research/exact/04-apps-audit-2026-11-10.md): the chat's
// operations. Targets are the element table's ids (components/core/elements.coax: `tree` lists them),
// never coordinates; time moves only by the host's `clock`.
typedef struct { char *b; int cap, n; } Say;
static void say(Say *o, const char *fmt, ...) __attribute__((format(printf, 2, 3)));
static void say(Say *o, const char *fmt, ...) {
  if (o->n >= o->cap - 1) return;
  va_list ap; va_start(ap, fmt);
  int k = vsnprintf(o->b + o->n, (size_t)(o->cap - o->n), fmt, ap);
  va_end(ap);
  if (k > 0) o->n = o->n + k < o->cap - 1 ? o->n + k : o->cap - 1;
}
static void say_text(Say *o, const char *verb, void *state, int32_t (*get)(void *, uint8_t *, int32_t)) {
  uint8_t b[4096]; int n = get(state, b, sizeof b - 1);
  say(o, "%s: %.*s\n", verb, n > 0 ? n : 0, (const char *)b);
}
// --dictation-fixture: the key and the button through the window's own handlers, and a file's samples in
// place of the recorder's, on the fixture's clock.
static void fixture_key(App *a, int down) {
  if (a->dictation_fixture) dn_fixture_time(a->dictation, a->dictation_clock);
  on_key(a, 'd', down ? 2 : 0, down);
}
static void fixture_mouse(App *a, int down) {
  float at[2] = {0, 0};
  if (a->dictation_fixture) dn_fixture_time(a->dictation, a->dictation_clock);
  if (dsk_dictation_point(a->desk, at)) on_pointer(a, at[0], at[1], down ? 0 : 2, 1, 0);
}
static void fixture_audio(App *a, const char *path, Say *o) {
  size_t n; uint8_t *bytes = oac_read_file(path, &n);
  // 16 kHz mono 16-bit samples: a WAV's, after its 44-byte header, or the bare samples
  size_t from = bytes && n >= 44 && !memcmp(bytes, "RIFF", 4) && !memcmp(bytes + 8, "WAVE", 4) ? 44 : 0;
  if (!bytes || (n - from) % 2) { say(o, "dictation: %s is not 16-bit samples\n", path); free(bytes); return; }
  int64_t start = a->dictation_clock, count = (int64_t)((n - from) / 2);
  for (int64_t at = 0; at < count; at += 320) {
    float chunk[320]; int k = count - at < 320 ? (int)(count - at) : 320;
    for (int i = 0; i < k; i++) { const uint8_t *b = bytes + from + 2 * (at + i); chunk[i] = (float)(int16_t)(b[0] | b[1] << 8) / 32768; }
    int64_t stamp = start + at / 16;
    dn_fixture_time(a->dictation, stamp);
    dn_fixture_audio(a->dictation, chunk, k, 16000, stamp);
  }
  a->dictation_clock = start + count / 16;
  dn_fixture_time(a->dictation, a->dictation_clock);
  free(bytes);
}
static void fixture_tail(App *a) {   // the 200 ms of input a release waits for
  static const float quiet[320];
  if (!a->dictation_fixture) return;
  for (int i = 0; i < 10; i++) {
    dn_fixture_audio(a->dictation, quiet, 320, 16000, a->dictation_clock);
    a->dictation_clock += 20; dn_fixture_time(a->dictation, a->dictation_clock);
  }
}
// A chord as the Mac's menus write it: cmd+1, shift+down, enter. 0: none.
static int chord(char *arg, int *mods) {
  static const char *const names[] = {"escape", "enter", "backspace", "left", "right", "up", "down", "home", "end", NULL};
  int key = 0; *mods = 0;
  for (char *part = strtok(arg, "+"); part; part = strtok(NULL, "+")) {
    if (!strcmp(part, "shift")) *mods |= 1; else if (!strcmp(part, "ctrl")) *mods |= 2; else if (!strcmp(part, "alt")) *mods |= 4; else if (!strcmp(part, "cmd")) *mods |= 8;
    else if (!strcmp(part, "delete")) key = -12;
    else { key = (unsigned char)part[0]; for (int i = 0; names[i]; i++) if (!strcmp(part, names[i])) key = -1 - i; }
  }
  return key;
}
// Row id's centre in device px (the pointer's units); 0 when there is no such row.
static int centre(App *a, const char *id, float *x, float *y) {
  float box[4];
  if (!dsk_box(a->desk, id, box)) return 0;
  *x = (box[0] + box[2] / 2) * a->dpr; *y = (box[1] + box[3] / 2) * a->dpr;
  return 1;
}
int oa_chat_busy(OaChat *a) {
  for (int i = 0; i < dsk_count(a->desk); i++) if (zr_busy(dsk_chat(a->desk, i))) return 1;
  if (a->dictation && dsk_dictation_enabled(a->desk) && !dn_ready(a->dictation)) return 1;   // the model is loading
  return a->dictation && a->dictation_ticket != 0;   // a request's words are on their way
}
int oa_chat_drive(OaChat *a, const char *request, char *out, int cap) {
  Say o = {out, cap, 0};
  char line[8192]; snprintf(line, sizeof line, "%s", request);
  char *arg = strchr(line, ' ');
  if (arg) *arg++ = 0; else arg = "";
  for (char *e = arg + strlen(arg); e > arg && e[-1] == ' ';) *--e = 0;
  void *state = oac_active(a);
  float n[4] = {0}; int count = sscanf(arg, "%f,%f,%f,%f", &n[0], &n[1], &n[2], &n[3]);
  if (!strcmp(line, "tree")) { o.n = dsk_tree(a->desk, (uint8_t *)out, cap); return o.n; }
  if (!strcmp(line, "tap") || !strcmp(line, "shift-tap") || !strcmp(line, "hover")) {   // tap ID [CLICKS]
    char *id = arg, *more = strchr(arg, ' ');
    int clicks = 1;
    if (more) { *more++ = 0; clicks = atoi(more) > 0 ? atoi(more) : 1; }
    float x, y;
    if (!centre(a, id, &x, &y)) { say(&o, "no row %s in the tree\n", id); return o.n; }
    if (line[0] == 'h') { on_pointer(a, x, y, 1, 1, 0); return o.n; }
    int mods = line[0] == 's' ? 1 : 0;
    on_pointer(a, x, y, 0, clicks, mods); on_pointer(a, x, y, 2, clicks, mods);
    return o.n;
  }
  if (!strcmp(line, "drag")) {   // drag ID DX,DY: from the row's centre, by DX, DY logical px
    char *id = arg, *by = strchr(arg, ' ');
    float x, y, d[2] = {0, 0};
    if (by) { *by++ = 0; sscanf(by, "%f,%f", &d[0], &d[1]); }
    if (!centre(a, id, &x, &y)) { say(&o, "no row %s in the tree\n", id); return o.n; }
    on_pointer(a, x, y, 0, 1, 0);
    for (int k = 1; k <= 8; k++) { float t = (float)k / 8; on_pointer(a, x + d[0] * a->dpr * t, y + d[1] * a->dpr * t, 1, 1, 0); }
    on_pointer(a, x + d[0] * a->dpr, y + d[1] * a->dpr, 2, 1, 0);
    return o.n;
  }
  if (!strcmp(line, "type")) {   // type ID TEXT | type ID key CHORD | type - TEXT (into what has the keyboard)
    char *id = arg, *text = strchr(arg, ' ');
    if (text) *text++ = 0; else text = "";
    if (strcmp(id, "-")) {
      float x, y;
      if (!centre(a, id, &x, &y)) { say(&o, "no row %s in the tree\n", id); return o.n; }
      on_pointer(a, x, y, 0, 1, 0); on_pointer(a, x, y, 2, 1, 0);
    }
    if (!strncmp(text, "key ", 4)) { int mods, key = chord(text + 4, &mods); if (key) on_key(a, key, mods, 1); }
    else on_text(a, text, strlen(text));
    return o.n;
  }
  if (!strcmp(line, "key")) { int mods, key = chord(arg, &mods); if (key) on_key(a, key, mods, 1); return o.n; }
  if (!strcmp(line, "wheel") && count >= 1) { dsk_wheel(a->desk, n[0]); return o.n; }
  if (!strcmp(line, "fixture")) { zr_fixture(state, !strcmp(arg, "closed") ? 0 : !strcmp(arg, "table") ? 2 : 1); return o.n; }
  if (!strcmp(line, "newchat")) { zr_new_chat(state); return o.n; }
  if (!strcmp(line, "sidebar")) { dsk_toggle(a->desk); return o.n; }
  if (!strcmp(line, "send")) { zr_text(state, (const uint8_t *)arg, (int32_t)strlen(arg)); zr_key(state, -2, 0); return o.n; }
  if (!strcmp(line, "mode")) { static const char *const modes[] = {"claude", "psionic", "lab", "codex"}; for (int i = 0; i < 4; i++) if (!strcasecmp(arg, modes[i])) zr_choose(state, 4, i); return o.n; }
  if (!strcmp(line, "access")) { static const char *const levels[] = {"full", "edits", "supervised"}; for (int i = 0; i < 3; i++) if (!strcasecmp(arg, levels[i])) zr_choose(state, 3, i); return o.n; }
  if (!strcmp(line, "model") || !strcmp(line, "effort")) { if (!zr_choose_named(state, line[0] == 'm' ? 1 : 2, (const uint8_t *)arg, (int32_t)strlen(arg))) say(&o, "no %s named %s\n", line, arg); return o.n; }
  if (!strcmp(line, "plan")) { zr_choose(state, 5, strcmp(arg, "off") != 0); return o.n; }
  if (!strcmp(line, "open")) { zr_open_menu(state, !strcmp(arg, "access") ? 3 : 1); return o.n; }
  if (!strcmp(line, "attach")) { oac_attach(a, arg); return o.n; }
  if (!strcmp(line, "folder")) { oac_new_session(a, arg); return o.n; }   // a new session in that folder, as the folder picker makes it
  if (!strcmp(line, "room")) { dsk_show_room(a->desk); return o.n; }
  if (!strcmp(line, "closetab")) { if (!dsk_close_tab(a->desk)) say(&o, "closetab: the last tab stays\n"); return o.n; }
  if (!strcmp(line, "stop")) { zr_key(state, -1, 0); return o.n; }
  if (!strcmp(line, "scroll") && count >= 1) { zr_scroll_to(state, n[0]); return o.n; }
  if (!strcmp(line, "answer")) { if (!zr_answer(state, !strcmp(arg, "allow"))) say(&o, "answer: nothing is asking\n"); return o.n; }
  if (!strcmp(line, "dictation")) {
    if (!a->offscreen || !a->dictation) { say(&o, "dictation: none here\n"); return o.n; }
    if (!strcmp(arg, "on")) { dsk_dictation_set(a->desk, 1); dn_enable(a->dictation, 1); dn_activate(a->dictation); oac_sync_dictation(a); }
    else if (!strcmp(arg, "off")) { dsk_dictation_set(a->desk, 0); dn_enable(a->dictation, 0); oac_sync_dictation(a); }
    else if (!strcmp(arg, "down")) fixture_key(a, 1);
    else if (!strcmp(arg, "up")) { fixture_key(a, 0); fixture_tail(a); }
    else if (!strcmp(arg, "mousedown")) fixture_mouse(a, 1);
    else if (!strcmp(arg, "mouseup")) { fixture_mouse(a, 0); fixture_tail(a); }
    else if (!strcmp(arg, "cancel")) oac_cancel_dictation_for_edit(a);
    else if (!strncmp(arg, "feed ", 5) && a->dictation_fixture) fixture_audio(a, arg + 5, &o);
    else say(&o, "dictation on|off|down|up|mousedown|mouseup|cancel|feed FILE\n");
    return o.n;
  }
  if (!strcmp(line, "state")) {   // state status|last|desk|draft|dictation: what the chat says, as text
    if (!strcmp(arg, "status")) say_text(&o, "status", state, zr_status_text);
    else if (!strcmp(arg, "last")) say_text(&o, "last", state, zr_last_text);
    else if (!strcmp(arg, "desk")) {
      int shown = -1;
      for (int i = 0; i < dsk_count(a->desk); i++) if (dsk_chat(a->desk, i) == dsk_active(a->desk)) shown = i;
      say(&o, "desk: active=%d tabs=", shown);
      for (int t = 0; t < dsk_tabs(a->desk); t++) say(&o, "%s%d", t ? "," : "", dsk_tab(a->desk, t));
      say(&o, " sessions=%d collapsed=%d\n", dsk_count(a->desk), dsk_collapsed(a->desk));
    } else if (!strcmp(arg, "draft")) {
      static uint8_t b[8192]; int k = zr_dictation_draft(state, b, sizeof b);
      say(&o, "draft: %.*s\n", k > 0 ? k : 0, (const char *)b);
      say(&o, "draft-state: sent=%d parts=%d\n", zr_dictation_sent(state), zr_dictation_parts(state));
    } else if (!strcmp(arg, "dictation")) {
      int phase = 0, outcome = 0, samples = 0;
      if (a->dictation) dn_inspect(a->dictation, &phase, &outcome, &samples);
      say(&o, "dictation: phase=%d outcome=%d samples=%d\n", phase, outcome, samples);
      say(&o, "dictation-setting: enabled=%d\n", dsk_dictation_enabled(a->desk));
    } else say(&o, "state status|last|desk|draft|dictation\n");
    return o.n;
  }
  return -1;
}

