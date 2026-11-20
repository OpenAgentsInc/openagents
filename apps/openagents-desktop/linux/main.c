// Firm reason: 6 (native Linux Wayland/Vulkan/D-Bus host entry point; UI, store, worker, worktree, daemon and update logic are Coax).
// Consolidated OpenAgents Desktop host for Linux (GitHub #13781).
// Drives apps/openagents-desktop/page.coax via host.h (apps/shared/host/linux/ui-host.c),
// native file/folder dialogs via apps/shared/host/linux/portal.c, and all persistence,
// worker, sub-agent, worktree, daemon and signed update logic via linux/core.coax
// (coax/lib/agent/desktop/).
#define _GNU_SOURCE
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#include "host.h"
#include "portal.h"
#include "core.h"

#define OA_APP_VERSION "0.1.0"
#define OA_APP_BUILD 1
#define OA_PORTAL_FOLDER 1
#define OA_PORTAL_ATTACH 2
#define OA_LINE_CAP (1024 * 1024)
#define OA_SAVE_CAP (4 * 1024 * 1024)
#define OA_SUBAGENTS 8

typedef struct {
  void *page;
  Portal *portal;
  coax_t_OaCore *core;
  char exe_dir[PATH_MAX];
  char resources[PATH_MAX];
  char worker_bin[PATH_MAX];
  char daemon_bin[PATH_MAX];
  char folder[PATH_MAX];
  char worktree_path[PATH_MAX];
  char worktree_branch[256];
  char worktree_repo[PATH_MAX];
  char chat_id[128];
  int sub_agent_id[OA_SUBAGENTS];
  int sub_active[OA_SUBAGENTS];
  char *line_buf;
  char *save_buf;
} DesktopApp;

static coax_string cs(const char *s) {
  if (!s) return (coax_string){NULL, 0};
  return (coax_string){s, (int64_t)strlen(s)};
}
static coax_v_u8 vu(char *buf, int cap) {
  return (coax_v_u8){(uint8_t *)buf, cap};
}
static coax_v_t_OaCore vc(DesktopApp *a) {
  return (coax_v_t_OaCore){a->core, 1};
}

static void dirname_of(const char *path, char *out, size_t cap) {
  snprintf(out, cap, "%s", path);
  char *slash = strrchr(out, '/');
  if (slash) *slash = '\0';
  else snprintf(out, cap, ".");
}

static void resolve_exe_dir(const char *argv0, char *out, size_t cap) {
  char self[PATH_MAX];
  ssize_t n = readlink("/proc/self/exe", self, sizeof(self) - 1);
  if (n > 0) {
    self[n] = '\0';
    dirname_of(self, out, cap);
    return;
  }
  dirname_of(argv0 ? argv0 : ".", out, cap);
}

static int file_exists(const char *path) {
  struct stat st;
  return path && path[0] && stat(path, &st) == 0;
}

static void format_folder_shown(const char *folder, char *out, size_t cap) {
  const char *home = getenv("HOME");
  if (home && home[0]) {
    size_t hl = strlen(home);
    if (strncmp(folder, home, hl) == 0 && (folder[hl] == '/' || folder[hl] == '\0')) {
      snprintf(out, cap, "~%s", folder + hl);
      return;
    }
  }
  snprintf(out, cap, "%s", folder);
}

static void spawn_xdg_open(const char *target) {
  if (!target || !target[0]) return;
  pid_t pid = fork();
  if (pid == 0) {
    execlp("xdg-open", "xdg-open", target, (char *)NULL);
    _exit(127);
  }
}

static void refresh_chats(DesktopApp *a) {
  char list[65536];
  int n = (int)oa_core_store_lines(vc(a), vu(list, (int)sizeof(list)));
  list[n > 0 && n < (int)sizeof(list) ? n : 0] = '\0';
  cd_chats(a->page, list);
  cd_current(a->page, a->chat_id);
}

static void refresh_settings_info(DesktopApp *a) {
  char ck[256] = {0}, ok[256] = {0};
  oa_core_key_read(cs("claude.key"), vu(ck, (int)sizeof(ck)));
  oa_core_key_read(cs("openrouter.key"), vu(ok, (int)sizeof(ok)));
  cd_settings_info(a->page, ck[0] ? "configured" : "", ok[0] ? "configured" : "", OA_APP_VERSION);
}

static void set_folder(DesktopApp *a, const char *folder) {
  if (!folder || !folder[0]) return;
  snprintf(a->folder, sizeof(a->folder), "%s", folder);
  char shown[PATH_MAX];
  format_folder_shown(a->folder, shown, sizeof(shown));
  cd_folder(a->page, shown);
  cd_branch(a->page, a->worktree_branch);
}

static void save_current_chat(DesktopApp *a) {
  if (!a->chat_id[0]) {
    oa_core_new_id(vu(a->chat_id, (int)sizeof(a->chat_id)));
  }
  int n = cd_save(a->page, a->save_buf, OA_SAVE_CAP);
  if (n <= 0) return;
  char title[256] = "New chat";
  if (cd_title(a->page, title, (int)sizeof(title)) <= 0) snprintf(title, sizeof(title), "New chat");
  oa_core_store_keep(
      vc(a),
      cs(a->chat_id),
      cs(title),
      cs(a->folder),
      (coax_string){a->save_buf, n},
      cs(a->worktree_path),
      cs(a->worktree_branch),
      cs(a->worktree_repo));
  refresh_chats(a);
}

static void open_chat_by_id(DesktopApp *a, const char *id) {
  if (!id || !id[0]) return;
  if (cd_running(a->page)) return;
  int n = (int)oa_core_store_text(vc(a), cs(id), vu(a->save_buf, OA_SAVE_CAP));
  if (n <= 0) return;
  snprintf(a->chat_id, sizeof(a->chat_id), "%s", id);
  cd_load(a->page, a->save_buf);
  char folder[PATH_MAX] = {0};
  if (oa_core_store_folder(vc(a), cs(id), vu(folder, (int)sizeof(folder))) > 0) {
    set_folder(a, folder);
  }
  cd_current(a->page, a->chat_id);
}

static const char *effective_cwd(DesktopApp *a) {
  if (a->worktree_path[0] && file_exists(a->worktree_path)) return a->worktree_path;
  return a->folder;
}

static void maybe_prepare_worktree(DesktopApp *a) {
  if (!oa_core_worktrees_on() || a->worktree_path[0]) return;
  if (!a->chat_id[0]) oa_core_new_id(vu(a->chat_id, (int)sizeof(a->chat_id)));
  char wp[PATH_MAX] = {0}, wb[256] = {0}, wr[PATH_MAX] = {0}, why[512] = {0};
  if (oa_core_wt_create(vc(a), cs(a->folder), cs(a->chat_id),
                        vu(wp, (int)sizeof(wp)), vu(wb, (int)sizeof(wb)),
                        vu(wr, (int)sizeof(wr)), vu(why, (int)sizeof(why)))) {
    snprintf(a->worktree_path, sizeof(a->worktree_path), "%s", wp);
    snprintf(a->worktree_branch, sizeof(a->worktree_branch), "%s", wb);
    snprintf(a->worktree_repo, sizeof(a->worktree_repo), "%s", wr);
    cd_branch(a->page, a->worktree_branch);
  }
}

static int ensure_worker(DesktopApp *a) {
  if (oa_core_worker_alive(vc(a))) return 1;
  if (!a->worker_bin[0]) {
    cd_failed(a->page, "mcworker binary not found");
    return 0;
  }
  maybe_prepare_worktree(a);
  char err[512] = {0};
  if (!oa_core_worker_start(vc(a), cs(a->worker_bin), cs(effective_cwd(a)), cs(""), vu(err, (int)sizeof(err)))) {
    cd_failed(a->page, err[0] ? err : "could not start worker");
    return 0;
  }
  return 1;
}

static void stop_all_subagents(DesktopApp *a) {
  for (int i = 0; i < OA_SUBAGENTS; i++) {
    if (a->sub_active[i]) {
      oa_core_subagent_stop(vc(a), i, 1);
      a->sub_active[i] = 0;
      a->sub_agent_id[i] = -1;
    }
  }
}

static void pump_subagents(DesktopApp *a) {
  int next = cd_agent_next(a->page);
  while (next >= 0) {
    int slot = -1;
    for (int i = 0; i < OA_SUBAGENTS; i++) {
      if (!a->sub_active[i]) { slot = i; break; }
    }
    if (slot < 0) {
      cd_agent_failed(a->page, next, "sub-agent limit reached");
    } else {
      char err[512] = {0};
      if (!oa_core_subagent_start(vc(a), slot, cs(a->worker_bin), cs(effective_cwd(a)), cs(""), vu(err, (int)sizeof(err)))) {
        cd_agent_failed(a->page, next, err[0] ? err : "could not start sub-agent worker");
      } else {
        a->sub_active[slot] = 1;
        a->sub_agent_id[slot] = next;
        int n = cd_agent_task(a->page, next, a->chat_id, effective_cwd(a), a->line_buf, OA_LINE_CAP);
        if (n > 0) oa_core_subagent_write(vc(a), slot, (coax_string){a->line_buf, n});
      }
    }
    next = cd_agent_next(a->page);
  }

  int stop_ag = cd_stop_agent(a->page);
  if (stop_ag >= 0) {
    for (int i = 0; i < OA_SUBAGENTS; i++) {
      if (a->sub_active[i] && a->sub_agent_id[i] == stop_ag) {
        oa_core_subagent_stop(vc(a), i, 1);
        a->sub_active[i] = 0;
        a->sub_agent_id[i] = -1;
      }
    }
  }

  for (int i = 0; i < OA_SUBAGENTS; i++) {
    if (!a->sub_active[i]) continue;
    int ag = a->sub_agent_id[i];
    int n = (int)oa_core_subagent_read(vc(a), i, 0, vu(a->line_buf, OA_LINE_CAP));
    if (n > 0) {
      cd_agent_line(a->page, ag, a->line_buf);
      int an = cd_agent_answer_line(a->page, ag, a->line_buf, OA_LINE_CAP);
      if (an > 0) oa_core_subagent_write(vc(a), i, (coax_string){a->line_buf, an});
    } else if (n == -2) {
      if (cd_agent_running(a->page, ag)) cd_agent_failed(a->page, ag, "sub-agent exited");
      a->sub_active[i] = 0;
      a->sub_agent_id[i] = -1;
    }
    if (a->sub_active[i] && !cd_agent_running(a->page, ag)) {
      oa_core_subagent_stop(vc(a), i, 1);
      a->sub_active[i] = 0;
      a->sub_agent_id[i] = -1;
    }
  }
}

static void check_update_from_file(DesktopApp *a, const char *path) {
  FILE *f = fopen(path, "rb");
  if (!f) {
    if (a && a->page) cd_update(a->page, 4, "Update check failed", "Could not read update manifest");
    return;
  }
  char buf[65536];
  size_t n = fread(buf, 1, sizeof(buf) - 1, f);
  fclose(f);
  buf[n] = '\0';
  char ver[128] = {0}, url[1024] = {0}, notes[2048] = {0}, err[256] = {0};
  int64_t r = oa_core_update_verify(
      (coax_string){buf, (int64_t)n}, cs(OA_APP_VERSION), OA_APP_BUILD, 0,
      vu(ver, (int)sizeof(ver)), vu(url, (int)sizeof(url)),
      vu(notes, (int)sizeof(notes)), vu(err, (int)sizeof(err)));
  if (!a || !a->page) return;
  if (r > 0) {
    char label[256];
    snprintf(label, sizeof(label), "Update %s available", ver);
    cd_update(a->page, 2, label, url);
  } else if (r == 0) {
    cd_update(a->page, 1, "Up to date", OA_APP_VERSION);
  } else {
    cd_update(a->page, 4, "Update verification failed", err);
  }
}

static void on_portal_chosen(void *user, int request, const char *path) {
  DesktopApp *a = user;
  if (!a || !path || !path[0]) return;
  if (request == OA_PORTAL_FOLDER) {
    a->worktree_path[0] = '\0';
    a->worktree_branch[0] = '\0';
    a->worktree_repo[0] = '\0';
    set_folder(a, path);
  } else if (request == OA_PORTAL_ATTACH) {
    cd_attach(a->page, path);
  }
}

static void handle_file_open(DesktopApp *a, const char *path) {
  if (!path || !path[0]) return;
  char full[PATH_MAX];
  if (path[0] == '/') snprintf(full, sizeof(full), "%s", path);
  else snprintf(full, sizeof(full), "%s/%s", effective_cwd(a), path);
  struct stat st;
  if (stat(full, &st) != 0) {
    cd_file_failed(a->page, path, strerror(errno));
    return;
  }
  FILE *f = fopen(full, "rb");
  if (!f) {
    cd_file_failed(a->page, path, strerror(errno));
    return;
  }
  int most = cd_file_most();
  if (most <= 0 || most > OA_SAVE_CAP) most = OA_SAVE_CAP;
  int n = (int)fread(a->save_buf, 1, (size_t)most, f);
  fclose(f);
  cd_file_open(a->page, path, a->save_buf, n, (long long)st.st_size);
}

static void handle_file_save(DesktopApp *a) {
  char name[PATH_MAX] = {0};
  if (cd_file_name(a->page, name, (int)sizeof(name)) <= 0) return;
  int n = cd_file_text(a->page, a->save_buf, OA_SAVE_CAP);
  if (n < 0) { cd_file_save_failed(a->page, "file too large"); return; }
  char full[PATH_MAX];
  if (name[0] == '/') snprintf(full, sizeof(full), "%s", name);
  else snprintf(full, sizeof(full), "%s/%s", effective_cwd(a), name);
  FILE *f = fopen(full, "wb");
  if (!f) { cd_file_save_failed(a->page, strerror(errno)); return; }
  size_t w = fwrite(a->save_buf, 1, (size_t)n, f);
  fclose(f);
  if ((int)w != n) { cd_file_save_failed(a->page, "short write"); return; }
  cd_file_saved(a->page, a->save_buf, n);
}

static void handle_action(DesktopApp *a, int act) {
  switch (act) {
    case 1: { // draft submitted
      if (!a->chat_id[0]) oa_core_new_id(vu(a->chat_id, (int)sizeof(a->chat_id)));
      if (!ensure_worker(a)) break;
      int n = cd_task(a->page, a->chat_id, effective_cwd(a), a->line_buf, OA_LINE_CAP);
      if (n > 0) oa_core_worker_write(vc(a), (coax_string){a->line_buf, n});
      save_current_chat(a);
      break;
    }
    case 3: { // stop run
      int n = cd_stop_line(a->line_buf, OA_LINE_CAP);
      if (n > 0 && oa_core_worker_alive(vc(a))) oa_core_worker_write(vc(a), (coax_string){a->line_buf, n});
      stop_all_subagents(a);
      break;
    }
    case 4: { // new chat
      int n = cd_forget_line(a->line_buf, OA_LINE_CAP);
      if (n > 0 && oa_core_worker_alive(vc(a))) oa_core_worker_write(vc(a), (coax_string){a->line_buf, n});
      stop_all_subagents(a);
      a->chat_id[0] = '\0';
      a->worktree_path[0] = '\0';
      a->worktree_branch[0] = '\0';
      a->worktree_repo[0] = '\0';
      cd_branch(a->page, "");
      cd_current(a->page, "");
      break;
    }
    case 5: { // copy code block
      int n = cd_copied(a->page, a->line_buf, OA_LINE_CAP);
      if (n > 0) cd_copy(a->page, a->line_buf, n);
      break;
    }
    case 6: { // choose working folder
      if (a->portal) portal_pick(a->portal, OA_PORTAL_FOLDER, 1, "Choose working folder", a->folder);
      break;
    }
    case 7: { // open chat from sidebar
      char id[128] = {0};
      if (cd_open_id(a->page, id, (int)sizeof(id)) > 0) open_chat_by_id(a, id);
      break;
    }
    case 8: { // open file path in pane
      char path[PATH_MAX] = {0};
      if (cd_open_path(a->page, path, (int)sizeof(path)) > 0) handle_file_open(a, path);
      break;
    }
    case 9: { // open external URL
      char url[2048] = {0};
      if (cd_open_url(a->page, url, (int)sizeof(url)) > 0) spawn_xdg_open(url);
      break;
    }
    case 14: cd_zoom(a->page, 1); break;
    case 15: cd_zoom(a->page, -1); break;
    case 22: { // check for updates
      const char *manifest = getenv("OPENAGENTS_UPDATE_MANIFEST");
      if (manifest && manifest[0]) check_update_from_file(a, manifest);
      else cd_update(a->page, 1, "Up to date", OA_APP_VERSION);
      break;
    }
    case 27: { // attach file via portal
      if (a->portal) portal_pick(a->portal, OA_PORTAL_ATTACH, 0, "Attach file", a->folder);
      break;
    }
    case 29: cd_zoom_set(a->page, 2); break;
    case 40: handle_file_save(a); break;
    case 41: {
      char name[PATH_MAX] = {0};
      if (cd_file_name(a->page, name, (int)sizeof(name)) > 0) spawn_xdg_open(name);
      break;
    }
    case 42: cd_file_close(a->page); break;
    default: break;
  }
}

static void pump_host(DesktopApp *a) {
  if (a->portal) portal_poll(a->portal, on_portal_chosen, a);
  for (int guard = 0; guard < 16; guard++) {
    int act = cd_action(a->page);
    if (act == 0) break;
    handle_action(a, act);
  }
  if (oa_core_worker_alive(vc(a))) {
    for (int guard = 0; guard < 16; guard++) {
      int n = (int)oa_core_worker_read(vc(a), 0, vu(a->line_buf, OA_LINE_CAP));
      if (n > 0) {
        cd_line(a->page, a->line_buf);
      } else if (n == -2) {
        if (cd_running(a->page)) cd_failed(a->page, "worker exited");
        break;
      } else {
        break;
      }
    }
    int an = cd_answer_line(a->page, a->line_buf, OA_LINE_CAP);
    if (an > 0) oa_core_worker_write(vc(a), (coax_string){a->line_buf, an});
  }
  pump_subagents(a);
  if (cd_save_due(a->page)) save_current_chat(a);
}

static int run_update_cli(const char *path) {
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "openagents-desktop: cannot open %s\n", path); return 1; }
  char buf[65536];
  size_t n = fread(buf, 1, sizeof(buf) - 1, f);
  fclose(f);
  buf[n] = '\0';
  char ver[128] = {0}, url[1024] = {0}, notes[2048] = {0}, err[256] = {0};
  int64_t r = oa_core_update_verify(
      (coax_string){buf, (int64_t)n}, cs(OA_APP_VERSION), OA_APP_BUILD, 1,
      vu(ver, (int)sizeof(ver)), vu(url, (int)sizeof(url)),
      vu(notes, (int)sizeof(notes)), vu(err, (int)sizeof(err)));
  if (r > 0) { printf("update: %s %s\n", ver, url); return 0; }
  if (r == 0) { printf("up-to-date: %s\n", OA_APP_VERSION); return 0; }
  fprintf(stderr, "update invalid: %s\n", err);
  return 1;
}

// Headless end-to-end smoke test (GitHub #13781):
// 1. Opens page.coax offscreen on Vulkan via ui-host.c and renders frames (+ optional PNG).
// 2. Runs a Microcoder turn through page.coax + desktop core store persistence in a temp dir.
// 3. Exercises git worktree creation through desktop core.
// 4. Exercises signed update envelope verification through desktop core.
static int run_smoke(DesktopApp *a, const char *shot_out) {
  char tmpl[] = "/tmp/oa-desktop-linux-smoke-XXXXXX";
  char *tmp = mkdtemp(tmpl);
  if (!tmp) { fprintf(stderr, "smoke: mkdtemp failed\n"); return 1; }

  char store_dir[PATH_MAX], daemon_dir[PATH_MAX];
  snprintf(store_dir, sizeof(store_dir), "%s/chats", tmp);
  snprintf(daemon_dir, sizeof(daemon_dir), "%s/daemon", tmp);
  mkdir(store_dir, 0700);
  mkdir(daemon_dir, 0700);

  if (!oa_core_init(vc(a), cs(store_dir), cs(daemon_dir))) {
    fprintf(stderr, "smoke: oa_core_init failed\n");
    return 1;
  }

  a->page = cd_open_ex(a->resources, 1, 1100, 760);
  if (!a->page) {
    fprintf(stderr, "smoke: cd_open_ex(offscreen=1) failed (resources=%s)\n", a->resources);
    return 1;
  }
  set_folder(a, tmp);
  refresh_settings_info(a);
  refresh_chats(a);

  // 1. Render initial frames offscreen on Vulkan.
  for (int i = 0; i < 4; i++) {
    if (!cd_shot_frame(a->page, 1100, 760, 1.0f / 60.0f)) {
      fprintf(stderr, "smoke: cd_shot_frame failed\n");
      return 1;
    }
  }

  // 2. Submit a prompt through page.coax, feed a worker reply, and persist to desktop core store.
  if (!cd_set_text(a->page, "Summarize Linux host readiness")) {
    fprintf(stderr, "smoke: cd_set_text failed\n");
    return 1;
  }
  if (!cd_submit(a->page)) {
    fprintf(stderr, "smoke: cd_submit failed\n");
    return 1;
  }
  if (cd_action(a->page) != 1) {
    fprintf(stderr, "smoke: expected submit action 1\n");
    return 1;
  }
  oa_core_new_id(vu(a->chat_id, (int)sizeof(a->chat_id)));
  int tn = cd_task(a->page, a->chat_id, tmp, a->line_buf, OA_LINE_CAP);
  if (tn <= 0) {
    fprintf(stderr, "smoke: cd_task returned 0\n");
    return 1;
  }
  cd_line(a->page, "{\"kind\":\"ready\",\"session\":\"smoke\"}");
  cd_line(a->page, "{\"kind\":\"reply\",\"text\":\"Linux host ready on Wayland/Vulkan with desktop core.\"}");
  cd_line(a->page, "{\"kind\":\"done\"}");
  save_current_chat(a);

  // Reload the persisted chat from disk and verify it round-trips.
  if (oa_core_store_count(vc(a)) < 1) {
    fprintf(stderr, "smoke: persisted chat count < 1\n");
    return 1;
  }
  int loaded = (int)oa_core_store_text(vc(a), cs(a->chat_id), vu(a->save_buf, OA_SAVE_CAP));
  if (loaded <= 0 || !strstr(a->save_buf, "Linux host ready")) {
    fprintf(stderr, "smoke: persisted chat did not round-trip\n");
    return 1;
  }

  // Render frames after the turn and optionally write a PNG.
  for (int i = 0; i < 3; i++) cd_shot_frame(a->page, 1100, 760, 1.0f / 60.0f);
  if (shot_out && shot_out[0] && !cd_shot_write(a->page, shot_out)) {
    fprintf(stderr, "smoke: cd_shot_write(%s) failed\n", shot_out);
    return 1;
  }

  // 3. Verify signed update envelope rejection of tampered/untrusted payloads.
  char ver[64] = {0}, url[256] = {0}, notes[256] = {0}, err[256] = {0};
  int64_t bad = oa_core_update_verify(
      cs("{\"key_id\":\"nope\",\"payload\":\"{}\",\"signature\":\"\"}"),
      cs(OA_APP_VERSION), OA_APP_BUILD, 0,
      vu(ver, (int)sizeof(ver)), vu(url, (int)sizeof(url)),
      vu(notes, (int)sizeof(notes)), vu(err, (int)sizeof(err)));
  if (bad >= 0) {
    fprintf(stderr, "smoke: untrusted update envelope was not rejected\n");
    return 1;
  }

  // 4. Verify git worktree creation in a temporary repository.
  char repo_dir[PATH_MAX], cmd[PATH_MAX * 2];
  snprintf(repo_dir, sizeof(repo_dir), "%s/repo", tmp);
  snprintf(cmd, sizeof(cmd),
           "git init -q '%s' && git -C '%s' config user.email smoke@openagents.com && "
           "git -C '%s' config user.name Smoke && touch '%s/README.md' && "
           "git -C '%s' add README.md && git -C '%s' commit -q -m init",
           repo_dir, repo_dir, repo_dir, repo_dir, repo_dir, repo_dir);
  if (system(cmd) == 0) {
    char wp[PATH_MAX] = {0}, wb[256] = {0}, wr[PATH_MAX] = {0}, why[512] = {0};
    if (!oa_core_wt_create(vc(a), cs(repo_dir), cs(a->chat_id),
                           vu(wp, (int)sizeof(wp)), vu(wb, (int)sizeof(wb)),
                           vu(wr, (int)sizeof(wr)), vu(why, (int)sizeof(why)))) {
      fprintf(stderr, "smoke: oa_core_wt_create failed: %s\n", why);
      return 1;
    }
    if (!file_exists(wp)) {
      fprintf(stderr, "smoke: worktree path missing: %s\n", wp);
      return 1;
    }
  }

  cd_close(a->page);
  a->page = NULL;
  printf("openagents-desktop-linux smoke: ok (chat=%s, store=%d)\n", a->chat_id, (int)oa_core_store_count(vc(a)));
  return 0;
}

int main(int argc, char **argv) {
  DesktopApp app;
  memset(&app, 0, sizeof(app));
  app.core = calloc(1, sizeof(coax_t_OaCore));
  app.line_buf = malloc(OA_LINE_CAP);
  app.save_buf = malloc(OA_SAVE_CAP);
  if (!app.core || !app.line_buf || !app.save_buf) {
    fprintf(stderr, "openagents-desktop: out of memory\n");
    return 1;
  }

  resolve_exe_dir(argv[0], app.exe_dir, sizeof(app.exe_dir));
  snprintf(app.resources, sizeof(app.resources), "%s/resources", app.exe_dir);
  if (!file_exists(app.resources)) snprintf(app.resources, sizeof(app.resources), "%s", app.exe_dir);
  snprintf(app.worker_bin, sizeof(app.worker_bin), "%s/mcworker", app.exe_dir);
  if (!file_exists(app.worker_bin)) snprintf(app.worker_bin, sizeof(app.worker_bin), "mcworker");
  snprintf(app.daemon_bin, sizeof(app.daemon_bin), "%s/openagents-daemon", app.exe_dir);
  if (!file_exists(app.daemon_bin)) app.daemon_bin[0] = '\0';

  if (!getcwd(app.folder, sizeof(app.folder))) snprintf(app.folder, sizeof(app.folder), ".");

  int smoke = 0;
  const char *shot = NULL;
  const char *update_file = NULL;
  for (int i = 1; i < argc; i++) {
    if (strcmp(argv[i], "--smoke") == 0) smoke = 1;
    else if (strcmp(argv[i], "--shot") == 0 && i + 1 < argc) shot = argv[++i];
    else if (strcmp(argv[i], "--resources") == 0 && i + 1 < argc) snprintf(app.resources, sizeof(app.resources), "%s", argv[++i]);
    else if (strcmp(argv[i], "--worker") == 0 && i + 1 < argc) snprintf(app.worker_bin, sizeof(app.worker_bin), "%s", argv[++i]);
    else if (strcmp(argv[i], "--daemon") == 0 && i + 1 < argc) snprintf(app.daemon_bin, sizeof(app.daemon_bin), "%s", argv[++i]);
    else if (strcmp(argv[i], "--cwd") == 0 && i + 1 < argc) snprintf(app.folder, sizeof(app.folder), "%s", argv[++i]);
    else if (strcmp(argv[i], "--check-update") == 0 && i + 1 < argc) update_file = argv[++i];
  }

  if (update_file) {
    int rc = run_update_cli(update_file);
    free(app.core); free(app.line_buf); free(app.save_buf);
    return rc;
  }

  if (smoke) {
    int rc = run_smoke(&app, shot);
    free(app.core); free(app.line_buf); free(app.save_buf);
    return rc;
  }

  if (!oa_core_init(vc(&app), cs(""), cs(""))) {
    fprintf(stderr, "openagents-desktop: could not initialize desktop core\n");
    return 1;
  }
  if (app.daemon_bin[0]) oa_core_daemon_ensure(vc(&app), cs(app.daemon_bin), 1500);

  app.page = cd_open_ex(app.resources, shot != NULL, 1200, 820);
  if (!app.page) {
    fprintf(stderr, "openagents-desktop: could not open UI host (resources=%s)\n", app.resources);
    return 1;
  }
  app.portal = shot ? NULL : portal_open();
  set_folder(&app, app.folder);
  refresh_settings_info(&app);
  refresh_chats(&app);

  int status = 0;
  if (shot) {
    for (int i = 0; i < 4; i++) cd_shot_frame(app.page, 1200, 820, 1.0f / 60.0f);
    if (!cd_shot_write(app.page, shot)) {
      fprintf(stderr, "openagents-desktop: could not write %s\n", shot);
      status = 1;
    }
  } else {
    while (cd_step(app.page)) {
      cd_env(app.page, cd_focused(app.page), 0);
      pump_host(&app);
    }
  }

  stop_all_subagents(&app);
  if (oa_core_worker_alive(vc(&app))) oa_core_worker_stop(vc(&app), 0);
  if (app.portal) { portal_cancel(app.portal); portal_close(app.portal); }
  cd_close(app.page);
  free(app.core); free(app.line_buf); free(app.save_buf);
  return status;
}
