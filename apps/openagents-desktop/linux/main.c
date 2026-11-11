// Firm reason: 6 (the app's process, files and window; every decision is Coax).
// OpenAgents for Linux and the Mac: the chat component (chat.h, chat.c: the Zeron-fork chat screen,
// its sessions, atlas, room and dictation) filling a window of the shared host
// (apps/shared/host/linux/ui-host.h: Wayland and Vulkan on Linux, apps/shared/host/apple/ui-host.m on
// the Mac, GitHub #13156). This file is the app around it: the options, the Mac's menu bar, the
// pictures and their coax drive operations, and the window's loop.
//
//   OpenAgents [--demo] [--cwd PATH] [--settings PATH] [--size WxH] [--dpr K] [--seconds N] [--dictation on|off] [--skin zeron|everglade|verse]
//              [--dictation-recorder PROGRAM]
//   OpenAgents --shot OUT.png [--size W,H] [--dpr K] [--demo] [--do "OP ; OP"] [--drive SOCKET] [--real-time] [--tree OUT.txt]
//              [--dictation-fixture | --dictation-recorder PROGRAM] [--dictation-stand-in WORDS]
//   OpenAgents --drive SOCKET [--agent-clock] ...   a window that coax drive can drive
// The operations are coax drive's (apps/shared/host/drive.c: clock +MS, clock settle [within MS], screenshot,
// quit), the chat's (chat.h oa_chat_drive: tree, tap ID, type ID TEXT, send, state last ...) and, on the Mac,
// this app's (menu TITLE, shortcut TITLE, clip TEXT, mark TEXT, commit TEXT, state pasteboard, state window).
// A picture runs them in order on the agent clock, printing each answer, then answers --drive's socket until
// `quit`, then is written, and with --tree its element table (`tree`) as text beside it.
// `coax/web/tool.sh drive native SOCKET OP...` sends them to a running app.
// With --dictation-fixture, which never runs a recorder, `dictation feed FILE` gives the recognizer a file's
// samples (16 kHz mono 16-bit, a WAV or its bare samples). With --dictation-recorder instead, PROGRAM runs in
// pw-record's place with its arguments (a check's stand-in, which writes a recorded clip as 48 kHz mono 32-bit
// float samples) on the wall clock, so such a picture takes --real-time: `clock +MS` then also waits MS.
// --dictation-stand-in WORDS, for a picture only: a stand-in recognizer that answers WORDS; no model is opened.
//
// Dictation (the former Sources/Zeron.swift's dictation members; dictation-host.h): hold Ctrl+D, or the microphone
// under the composer, and speak; the words go into the draft when you let go, and are never sent.
// It is off until you turn it on: Ctrl+Shift+D in the window, or --dictation on. It is kept in settings.json.
//   OpenAgents --mcp-room SOCKET AGENT    the room's MCP server, which Codex starts
#include "chat-app.h"

#ifdef __APPLE__
// ---- the Mac's menu bar (Zeron's crates/ui/src/app_menus.rs for what this app has), as the former Sources/main.swift had it

enum { C_DICTATION = 1, C_DICTATION_INPUT, C_NEW, C_NEW_IN_FOLDER, C_CLOSE_TAB, C_UNDO, C_REDO, C_CUT, C_COPY, C_PASTE, C_SELECT_ALL,
       C_SIDEBAR, C_QUIT, C_CLOSE_WINDOW, C_INPUTS = 100 };
static UiMenuItem menus[] = {
  {"OpenAgents", "About OpenAgents", "", 0, -1, "orderFrontStandardAboutPanel:"}, {"OpenAgents", NULL, NULL, 0, -1, NULL},
  {"OpenAgents", "Enable Local Dictation — Parakeet", "", 0, C_DICTATION, NULL},
  {"OpenAgents", "Dictation Input…", "", 0, C_DICTATION_INPUT, NULL},
  {"OpenAgents", "Hold ⌘D or microphone · on-device draft only", "", 0, -1, NULL}, {"OpenAgents", NULL, NULL, 0, -1, NULL},
  {"OpenAgents", "Services", "", 0, -1, NULL}, {"OpenAgents", NULL, NULL, 0, -1, NULL},
  {"OpenAgents", "Hide OpenAgents", "h", 8, -1, "hide:"},
  {"OpenAgents", "Hide Others", "h", 8 | 4, -1, "hideOtherApplications:"},
  {"OpenAgents", "Show All", "", 0, -1, "unhideAllApplications:"}, {"OpenAgents", NULL, NULL, 0, -1, NULL},
  {"OpenAgents", "Quit OpenAgents", "q", 8, C_QUIT, NULL},
  {"File", "New Session", "n", 8, C_NEW, NULL},
  {"File", "New Session in Folder…", "n", 8 | 1, C_NEW_IN_FOLDER, NULL}, {"File", NULL, NULL, 0, -1, NULL},
  {"File", "Close Tab", "w", 8, C_CLOSE_TAB, NULL},
  // AppKit adds Dictation and Emoji & Symbols to a menu titled Edit
  {"Edit", "Undo", "z", 8, C_UNDO, NULL}, {"Edit", "Redo", "z", 8 | 1, C_REDO, NULL}, {"Edit", NULL, NULL, 0, -1, NULL},
  {"Edit", "Cut", "x", 8, C_CUT, NULL}, {"Edit", "Copy", "c", 8, C_COPY, NULL}, {"Edit", "Paste", "v", 8, C_PASTE, NULL}, {"Edit", NULL, NULL, 0, -1, NULL},
  {"Edit", "Select All", "a", 8, C_SELECT_ALL, NULL},
  {"View", "Toggle Sidebar", "s", 8, C_SIDEBAR, NULL},
  {"Window", "Minimize", "m", 8, -1, "performMiniaturize:"}, {"Window", "Zoom", "", 0, -1, "performZoom:"}, {"Window", NULL, NULL, 0, -1, NULL},
  {"Window", "Close Window", "w", 8 | 1, C_CLOSE_WINDOW, NULL},
};
enum { MENU_ITEMS = (int)(sizeof menus / sizeof menus[0]) };
static char menu_keys[MENU_ITEMS][8];   // the key equivalents settings.json gave (menus[i].key points here then)

static DmInput inputs[32];
static int ninputs;
static void on_command(void *user, int id) {
  App *a = user;
  void *state = oac_active(a);
  switch (id) {
    case C_DICTATION: oac_cancel_dictation_for_edit(a); oac_set_dictation(a, !dsk_dictation_enabled(a->desk)); break;
    case C_DICTATION_INPUT: {
      // the inputs there are, the chosen one checked
      static UiMenuItem items[34];
      char chosen[256]; dm_selected(chosen, sizeof chosen);
      ninputs = dm_inputs(inputs, 32);
      int n = 0, checked = chosen[0] ? -1 : C_INPUTS, found = 0;
      items[n++] = (UiMenuItem){"", "System default input", "", 0, C_INPUTS, NULL};
      for (int i = 0; i < ninputs; i++) {
        items[n++] = (UiMenuItem){"", inputs[i].name, "", 0, C_INPUTS + 1 + i, NULL};
        if (chosen[0] && !strcmp(chosen, inputs[i].uid)) { checked = C_INPUTS + 1 + i; found = 1; }
      }
      if (chosen[0] && !found) items[n++] = (UiMenuItem){"", "Selected input unavailable — choose an input", "", 0, -1, NULL};
      ui_popup(a->host, items, n, 16 * a->dpr, 36 * a->dpr, checked);
      break;
    }
    case C_NEW: { oac_cancel_dictation_for_edit(a); uint8_t cwd[PATH_MAX]; int k = dsk_cwd(a->desk, cwd, sizeof cwd - 1); cwd[k > 0 ? k : 0] = 0; oac_new_session(a, k > 0 ? (const char *)cwd : getenv("HOME")); break; }
    case C_NEW_IN_FOLDER: oac_cancel_dictation_for_edit(a); oac_pick_folder(a); break;
    case C_CLOSE_TAB: oac_cancel_dictation_for_edit(a); if (!dsk_close_tab(a->desk)) a->quit = 1; break;   // the last tab closes the window
    case C_UNDO: oac_cancel_dictation_for_edit(a); zr_undo(state); break;
    case C_REDO: oac_cancel_dictation_for_edit(a); zr_redo(state); break;
    case C_CUT: oac_cancel_dictation_for_edit(a); if (zr_can_cut(state)) { oac_copy_selection(a); zr_cut(state); } break;
    case C_COPY: oac_copy_selection(a); break;
    case C_PASTE: oac_paste(a); break;
    case C_SELECT_ALL: oac_cancel_dictation_for_edit(a); zr_select_all(state); break;
    case C_SIDEBAR: dsk_toggle(a->desk); break;
    case C_QUIT: case C_CLOSE_WINDOW: a->quit = 1; break;
    default:
      if (id >= C_INPUTS && id <= C_INPUTS + ninputs) {
        oac_cancel_dictation_for_edit(a);
        dm_select(id == C_INPUTS ? "" : inputs[id - C_INPUTS - 1].uid);
        oac_sync_dictation(a);
      }
  }
}
static int on_command_state(void *user, int id) {
  App *a = user;
  void *state = oac_active(a);
  int focused = zr_wants_text(state) != 0;
  switch (id) {
    case C_DICTATION: return 1 | (dsk_dictation_enabled(a->desk) ? 2 : 0);
    case C_UNDO: case C_REDO: case C_PASTE: return focused;
    case C_COPY: return zr_selected_count(state) > 0;
    case C_CUT: return zr_can_cut(state) != 0;
    default: return 1;
  }
}
static void on_file(void *user, int request, const char *path) { oa_chat_file(user, request, path); }
// "shift+alt+left" as the menus' key and mods; the key's letter, or 0 for a named key.
static int chord(const char *text, char *key, int *mods) {
  char buf[64]; snprintf(buf, sizeof buf, "%s", text);
  *mods = 0; key[0] = 0;
  for (char *part = strtok(buf, "+"); part; part = strtok(NULL, "+")) {
    if (!strcasecmp(part, "shift")) *mods |= 1; else if (!strcasecmp(part, "ctrl")) *mods |= 2;
    else if (!strcasecmp(part, "alt")) *mods |= 4; else if (!strcasecmp(part, "cmd")) *mods |= 8;
    else if (strlen(part) == 1) { key[0] = (char)(part[0] >= 'A' && part[0] <= 'Z' ? part[0] + 32 : part[0]); key[1] = 0; }
  }
  return key[0] != 0;
}
// The menu commands' keys from settings.json (zeron/desk.coax DSK_KEY_*, in that order).
static void apply_shortcuts(App *a) {
  static const int ids[5] = {C_NEW, C_NEW_IN_FOLDER, C_CLOSE_TAB, C_SIDEBAR, C_CLOSE_WINDOW};
  for (int j = 0; j < 5; j++) {
    uint8_t buf[32]; int n = dsk_shortcut(a->desk, j, buf, sizeof buf - 1);
    if (n <= 0) continue;
    buf[n] = 0;
    char key[8]; int mods;
    if (!chord((const char *)buf, key, &mods)) continue;
    for (int i = 0; i < MENU_ITEMS; i++) if (menus[i].id == ids[j]) {
      snprintf(menu_keys[i], sizeof menu_keys[i], "%s", key);
      menus[i].key = menu_keys[i]; menus[i].mods = mods;
      ui_menu_key(a->host, ids[j], key, mods);
    }
  }
}
#endif
// ---- coax drive (apps/shared/host/drive.c): the app's own operations, then the chat's (chat.h oa_chat_drive)
static int on_drive(void *user, const char *request, char *out, int cap) {
  App *a = user;
#ifdef __APPLE__
  char line[8192]; snprintf(line, sizeof line, "%s", request);
  char *arg = strchr(line, ' ');
  if (arg) *arg++ = 0; else arg = "";
  int n = 0;
  #define SAY(...) (n += snprintf(out + n, (size_t)(cap - n > 0 ? cap - n : 0), __VA_ARGS__), n = n < cap ? n : cap - 1)
  if (!strcmp(line, "menu")) {   // a menu item, as the menu bar sends it (when it is enabled)
    int found = 0;
    for (int i = 0; i < MENU_ITEMS; i++) if (menus[i].title && !strcmp(menus[i].title, arg) && menus[i].id > 0) {
      found = 1;
      if (on_command_state(a, menus[i].id) & 1) on_command(a, menus[i].id); else SAY("menu %s is disabled\n", arg);
    }
    if (!found) SAY("no menu item %s\n", arg);
    return n;
  }
  if (!strcmp(line, "shortcut")) {   // a menu command's key as the menu bar has it
    for (int i = 0; i < MENU_ITEMS; i++) if (menus[i].title && !strcmp(menus[i].title, arg)) {
      int m = menus[i].mods;
      SAY("shortcut: %s %s%s%s%s%s\n", arg, m & 1 ? "shift+" : "", m & 2 ? "ctrl+" : "", m & 4 ? "alt+" : "", m & 8 ? "cmd+" : "", menus[i].key);
    }
    return n;
  }
  if (!strcmp(line, "clip")) { ui_copy(a->host, arg, strlen(arg)); return 0; }   // the picture's own pasteboard
  if (!strcmp(line, "mark")) {   // marked text, its caret at its end (UTF-16 units)
    int units = 0;
    for (const unsigned char *p = (const unsigned char *)arg; *p; p++) units += (*p & 0xC0) != 0x80 ? ((*p >= 0xF0) ? 2 : 1) : 0;
    oac_ime_mark(a, -1, -1, arg, strlen(arg), units, units);
    return 0;
  }
  if (!strcmp(line, "commit")) { oac_ime_replace(a, -1, -1, arg, strlen(arg)); return 0; }
  if (!strcmp(line, "state") && !strcmp(arg, "pasteboard")) { static char b[1 << 16]; int k = ui_paste_text(a->host, b, sizeof b); SAY("pasteboard: %.*s\n", k, b); return n; }
  if (!strcmp(line, "state") && !strcmp(arg, "window")) {
    float f[4];
    if (ds_window(a->settings, f)) SAY("window: %dx%d at %d,%d\n", (int)f[2], (int)f[3], (int)f[0], (int)f[1]); else SAY("window: none\n");
    return n;
  }
  #undef SAY
#endif
  return oa_chat_drive(a, request, out, cap);
}
static int on_busy(void *user) { return oa_chat_busy(user); }

static const char *value(int argc, char **argv, const char *flag) {
  for (int i = 1; i + 1 < argc; i++) if (!strcmp(argv[i], flag)) return argv[i + 1];
  return NULL;
}
static int has(int argc, char **argv, const char *flag) { for (int i = 1; i < argc; i++) if (!strcmp(argv[i], flag)) return 1; return 0; }

static volatile sig_atomic_t stopping;
static void on_signal(int sig) { (void)sig; stopping = 1; }

static int on_frame(void *user, float *v, float w, float h, float dpr, float dt) { return oa_chat_frame(user, v, w, h, dpr, dt); }
static void on_pointer(void *user, float x, float y, int phase, int clicks, int mods) { oa_chat_pointer(user, x, y, phase, clicks, mods); }
static void on_wheel(void *user, float dy) { oa_chat_wheel(user, dy); }
static int on_key(void *user, int key, int mods, int down) { return oa_chat_key(user, key, mods, down); }
static void on_text(void *user, const char *bytes, size_t n) { oa_chat_text(user, bytes, n); }
static int on_wants_text(void *user) { return oa_chat_wants_text(user); }

// This program's own folder: the resources sit beside it (fonts/, icons/, zeron.spv), or in the bundle's Resources on the Mac.
static int resources_of(char *out, size_t cap) {
#ifdef __APPLE__
  char raw[PATH_MAX]; uint32_t size = sizeof raw;
  if (_NSGetExecutablePath(raw, &size) || !realpath(raw, out)) return 0;
#else
  ssize_t n = readlink("/proc/self/exe", out, cap - 1);
  if (n <= 0) return 0;
  out[n] = 0;
#endif
  (void)cap;
  char *slash = strrchr(out, '/');
  if (slash) *slash = 0;
#ifdef __APPLE__
  // OpenAgents.app/Contents/MacOS/OpenAgents: its Resources are beside MacOS
  slash = strrchr(out, '/');
  if (slash && !strcmp(slash, "/MacOS")) strcpy(slash, "/Resources");
#endif
  return 1;
}

int main(int argc, char **argv) {
  char resources[PATH_MAX], shader[PATH_MAX + 32];
  if (!resources_of(resources, sizeof resources)) return 1;
#ifdef __APPLE__
  snprintf(shader, sizeof shader, "%s/zeron.metallib", resources);
  // --compare SHOT GOLDEN [PERCENT]: how many pixels differ by more than 48 of 255 (check.sh's goldens)
  for (int i = 1; i + 2 < argc; i++) if (!strcmp(argv[i], "--compare")) return ui_compare(argv[i + 1], argv[i + 2], i + 3 < argc ? atof(argv[i + 3]) : 0.1);
  signal(SIGPIPE, SIG_IGN);
#else
  snprintf(shader, sizeof shader, "%s/zeron.spv", resources);
#endif

  for (int i = 1; i + 2 < argc; i++) if (!strcmp(argv[i], "--mcp-room")) return oa_chat_mcp_room(argv[i + 1], atoi(argv[i + 2]));
  const char *shot = value(argc, argv, "--shot"), *size = value(argc, argv, "--size"), *dpr = value(argc, argv, "--dpr");
  unsigned w = shot ? 2640 : 1320, h = shot ? 1760 : 880;
  if (size && sscanf(size, "%u%*[x,]%u", &w, &h) != 2) { fprintf(stderr, "OpenAgents: --size WxH\n"); return 2; }
  float scale = dpr ? (float)atof(dpr) : shot ? 2 : 1;
  if (scale <= 0) scale = 1;

  // settings.json: --settings PATH, else ~/.openagents/desktop/settings.json (on the Mac, where it has always been:
  // ~/Library/Application Support/OpenAgents/settings.json); none for --demo and for a picture without --settings
  char settings_path[PATH_MAX] = "";
  const char *settings = value(argc, argv, "--settings"), *home = getenv("HOME");
  int demo = has(argc, argv, "--demo");
  if (settings) snprintf(settings_path, sizeof settings_path, "%s", settings);
#ifdef __APPLE__
  else if (!shot && !demo && home) snprintf(settings_path, sizeof settings_path, "%s/Library/Application Support/OpenAgents/settings.json", home);
#else
  else if (!shot && !demo && home) snprintf(settings_path, sizeof settings_path, "%s/.openagents/desktop/settings.json", home);
#endif
  OaChatOptions options = {.resources = resources, .settings = settings_path, .cwd = value(argc, argv, "--cwd"), .skin = value(argc, argv, "--skin"),
                           .dpr = scale, .offscreen = shot != NULL, .demo = demo, .still_clock = has(argc, argv, "--still-clock"), .room = 1,
                           .dictation_setting = value(argc, argv, "--dictation"), .dictation = 1,
                           .dictation_fixture = has(argc, argv, "--dictation-fixture"), .dictation_recorder = value(argc, argv, "--dictation-recorder"),
                           .dictation_stand_in = value(argc, argv, "--dictation-stand-in")};
  // the sessions first: the seat's first events come while the window opens
  App *a = oa_chat_new(&options);
  if (!a) return options.skin ? 2 : 1;
  UiApp app = {.title = "OpenAgents", .app_id = "com.openagents.desktop", .width = w, .height = h, .max_vertices = oa_chat_max_vertices(), .shader = shader,
               .user = a, .frame = on_frame, .pointer = on_pointer, .wheel = on_wheel, .key = on_key, .text = on_text, .wants_text = on_wants_text,
               .drive = on_drive, .busy = on_busy};
#ifdef __APPLE__
  // the Mac's window: under a transparent title bar, at least 900 by 600 points, where it was at quit
  app.chrome = 1; app.min_width = 900; app.min_height = 600;
  app.ime = &oac_ime; app.command = on_command; app.command_state = on_command_state; app.file = on_file;
  if (settings_path[0] && !demo) {
    size_t sn; uint8_t *json = oac_read_file(settings_path, &sn);
    void *peek = json ? ds_new() : NULL;
    if (peek && ds_load(peek, dsk_new(), json, (int32_t)sn) >= 0) ds_window(peek, app.place);
    free(json);
  }
#endif
  UiHost *host = ui_open(&app, shot != NULL);
  if (!host) return 1;
#ifdef __APPLE__
  ui_menus(host, menus, MENU_ITEMS);
#endif
  if (!oa_chat_start(a, host)) return 1;
#ifdef __APPLE__
  apply_shortcuts(a);
#endif

  int status = 0;
  const char *drive = value(argc, argv, "--drive");
  if (drive && !ui_drive_listen(host, drive)) { fprintf(stderr, "OpenAgents: could not listen on %s\n", drive); return 1; }
  if (shot) {
    // a picture runs on the agent clock: nothing moves between two operations but what `clock` gives
    ui_agent_clock(host, 1);
    ui_real_time(host, has(argc, argv, "--real-time"));
    for (int i = 0; i < 3; i++) ui_shot_frame(host, w, h, 1 / 60.0f);
    const char *script = value(argc, argv, "--do");
    char *ops = strdup(script ? script : ""), answer[1 << 16];
    for (char *s = ops; s && *s;) {
      char *next = strstr(s, " ; ");
      if (next) { *next = 0; next += 3; }
      int k = ui_drive(host, s, answer, sizeof answer);
      if (k > 0) printf("%.*s%s", k, answer, answer[k - 1] == '\n' ? "" : "\n");
      s = next;
    }
    free(ops);
    if (drive) ui_drive_serve(host);   // then whatever coax drive asks, until it says quit
    ui_drive(host, "clock +50", answer, sizeof answer);   // three frames, then the picture
    ui_shot_frame(host, w, h, 0);
    // --tree PATH: the element table as text beside the picture (a check compares it first, row by row)
    const char *tree = value(argc, argv, "--tree");
    if (tree) {
      int k = ui_drive(host, "tree", answer, sizeof answer);
      FILE *f = fopen(tree, "w");
      if (!f || fwrite(answer, 1, (size_t)k, f) != (size_t)k) { fprintf(stderr, "OpenAgents: could not write %s\n", tree); status = 1; }
      if (f) fclose(f);
    }
    if (!ui_shot_write(host, shot)) { fprintf(stderr, "OpenAgents: could not write %s\n", shot); status = 1; }
  } else {
    if (drive && has(argc, argv, "--agent-clock")) ui_agent_clock(host, 1);
    const char *seconds = value(argc, argv, "--seconds");
    double limit = seconds ? atof(seconds) : 0;
    // --fps SECS: the frames presented a second, after a second's warm-up, printed; then quit
    const char *fps = value(argc, argv, "--fps");
    double warm = fps ? 1 : 0;
    if (fps) limit = warm + atof(fps);
    uint64_t presented = 0;
    struct timespec t0; clock_gettime(CLOCK_MONOTONIC, &t0);
    signal(SIGINT, on_signal); signal(SIGTERM, on_signal);
    while (!stopping && ui_frame(host)) {
      struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t);
      double at = t.tv_sec - t0.tv_sec + (t.tv_nsec - t0.tv_nsec) / 1e9;
#ifdef __APPLE__
      if (fps && warm > 0 && at >= warm) { presented = ui_presented(host); warm = -warm; }
#endif
      if (limit > 0 && at > limit) break;
    }
#ifdef __APPLE__
    if (fps) printf("%.1f fps over %s s at %ux%u (scale %.0f)\n", (double)(ui_presented(host) - presented) / atof(fps), fps, a->w, a->h, (double)a->dpr);
#endif
  }
  oa_chat_close(a);
  ui_close(host);
  return status;
}
