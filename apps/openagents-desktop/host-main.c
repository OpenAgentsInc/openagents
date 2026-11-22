// Firm reason: 6 (native host entry logic; UI, store, worker, worktree, daemon and update logic are Coax).
// OpenAgents Desktop's entry logic on Linux and Windows (GitHub #13781, #13782): it drives
// page.coax through host.h (the shared Vulkan host: apps/shared/host/linux/ or windows/), the
// file chooser through portal.h, and all persistence, worker, sub-agent, worktree, daemon and
// signed update logic through host-core.coax (coax/lib/agent/desktop/). linux/main.c and
// windows/main.c include it after defining what differs between the two:
//   OA_PLATFORM, OA_PLATFORM_NAME             the system's name in files and as people read it
//   OA_WORKER, OA_DAEMON                      the worker's and daemon's file names
//   oa_exe_dir(argv0, out, cap)               the folder the program is in
//   oa_home(out, cap)                         the home folder ("" unknown), forward slashes
//   oa_working_dir(out, cap)                  the folder the app starts in
//   oa_absolute(path)                         whether a path names a place on its own
//   oa_temp_dir(out, cap), oa_mkdir(path)     a new private temporary folder; a folder
//   oa_open_external(target)                  a link or file in the system's own app
//   oa_fail(why), oa_started()                a startup failure told to the person; the window is up
// Paths are UTF-8 with forward slashes on both.
#include "host.h"
#include "portal.h"
#include "core.h"

#define OA_PATH 4096
#define OA_APP_VERSION "0.1.0"
#define OA_APP_BUILD 1
#define OA_PORTAL_FOLDER 1
#define OA_PORTAL_ATTACH 2
#define OA_LINE_CAP (1024 * 1024)
#define OA_SAVE_CAP (4 * 1024 * 1024)
#define OA_SUBAGENTS 8
#define OA_TERMS 8

typedef struct {
  void *page;
  Portal *portal;
  coax_t_OaCore *core;
  char exe_dir[OA_PATH];
  char resources[OA_PATH];
  char worker_bin[OA_PATH];
  char claude_worker_bin[OA_PATH];
  char agy_worker_bin[OA_PATH];
  char daemon_bin[OA_PATH];
  char agy_bin[OA_PATH];
  char agy_model[128];
  int agy_mode;
  int worker_is_agy;   // the running main worker: 0 Microcoder, 1 Antigravity, 2 Claude
  int foreman_tier;
  char folder[OA_PATH];
  char worktree_path[OA_PATH];
  char worktree_branch[256];
  char worktree_repo[OA_PATH];
  char chat_id[128];
  char update_url[1024];
  char update_version[128];
  char update_sha[65];
  int64_t update_size;
  int dialog, dialog_status;
  int interactive;
  int sub_agent_id[OA_SUBAGENTS];
  int sub_active[OA_SUBAGENTS];
  int term_tab_id[OA_TERMS];
  int term_handle[OA_TERMS];
  int term_pid[OA_TERMS];
  int term_active[OA_TERMS];
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

static int file_exists(const char *path) {
  struct stat st;
  return path && path[0] && stat(path, &st) == 0;
}

static void format_folder_shown(const char *folder, char *out, size_t cap) {
  char hb[OA_PATH];
  const char *home = oa_home(hb, sizeof hb);
  if (home && home[0]) {
    size_t hl = strlen(home);
    if (strncmp(folder, home, hl) == 0 && (folder[hl] == '/' || folder[hl] == '\0')) {
      snprintf(out, cap, "~%s", folder + hl);
      return;
    }
  }
  snprintf(out, cap, "%s", folder);
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
  oa_core_key_read(cs("claude-key"), vu(ck, (int)sizeof(ck)));
  oa_core_key_read(cs("openrouter-key"), vu(ok, (int)sizeof(ok)));
  cd_settings_info(a->page, ck[0] ? "configured" : "", ok[0] ? "configured" : "", OA_APP_VERSION);
  const char *agy_status = a->agy_worker_bin[0] ? (a->agy_bin[0] ? a->agy_bin : "agyworker ready") : "agyworker not bundled";
  cd_settings_agy(a->page, agy_status, a->agy_mode, a->agy_model[0] ? a->agy_model : "default", "");
  int64_t tier = a->foreman_tier;
  if (oa_core_daemon_read_foreman(vc(a), (coax_v_s64){&tier, 1})) {
    a->foreman_tier = (int)tier;
  }
  cd_foreman(a->page, 0, a->foreman_tier);
}

static void set_folder(DesktopApp *a, const char *folder) {
  if (!folder || !folder[0]) return;
  snprintf(a->folder, sizeof(a->folder), "%s", folder);
  char shown[OA_PATH];
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
  char folder[OA_PATH] = {0};
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
  char wp[OA_PATH] = {0}, wb[256] = {0}, wr[OA_PATH] = {0}, why[512] = {0};
  if (oa_core_wt_create(vc(a), cs(a->folder), cs(a->chat_id),
                        vu(wp, (int)sizeof(wp)), vu(wb, (int)sizeof(wb)),
                        vu(wr, (int)sizeof(wr)), vu(why, (int)sizeof(why)))) {
    snprintf(a->worktree_path, sizeof(a->worktree_path), "%s", wp);
    snprintf(a->worktree_branch, sizeof(a->worktree_branch), "%s", wb);
    snprintf(a->worktree_repo, sizeof(a->worktree_repo), "%s", wr);
    cd_branch(a->page, a->worktree_branch);
  }
}

// The worker's extra environment: the OpenRouter key kept in settings (ask_key), which
// Microcoder reads there (coder/tui/microcoder_keys.coax), plus optional Antigravity CLI/model.
static void worker_extra(DesktopApp *a, int is_agy, char *out, size_t cap) {
  char key[1024] = {0};
  size_t pos = 0;
  out[0] = '\0';
  if (oa_core_key_read(cs("openrouter-key"), vu(key, (int)sizeof key)) > 0) {
    pos += (size_t)snprintf(out + pos, cap - pos, "OPENROUTER_API_KEY=%s\n", key);
  }
  memset(key, 0, sizeof key);
  if (is_agy && a) {
    if (a->agy_bin[0] && pos < cap) pos += (size_t)snprintf(out + pos, cap - pos, "CODER_AGY_BIN=%s\n", a->agy_bin);
    if (a->agy_model[0] && pos < cap) pos += (size_t)snprintf(out + pos, cap - pos, "CODER_AGY_MODEL=%s\n", a->agy_model);
  }
}

static int ensure_worker(DesktopApp *a) {
  int want_agy = cd_model_agy(a->page) != 0 && a->agy_worker_bin[0] != '\0';
  // a chat on a Claude model runs on the Claude worker (kind 2), as its sub-agents do
  int want_claude = !want_agy && cd_model_claude(a->page) != 0 && a->claude_worker_bin[0] != '\0';
  int want_kind = want_agy ? 1 : want_claude ? 2 : 0;
  if (oa_core_worker_alive(vc(a))) {
    if (a->worker_is_agy == want_kind) return 1;
    if (!cd_running(a->page)) {
      oa_core_worker_stop(vc(a), 0);
    } else {
      return 1;
    }
  }
  const char *bin = want_agy ? a->agy_worker_bin : want_claude ? a->claude_worker_bin : a->worker_bin;
  if (!bin[0]) {
    cd_failed(a->page, want_agy ? "agyworker binary not found" : "mcworker binary not found");
    return 0;
  }
  maybe_prepare_worktree(a);
  char err[512] = {0};
  char extra[2200];
  worker_extra(a, want_agy, extra, sizeof extra);
  int started = oa_core_worker_start(vc(a), cs(bin), cs(effective_cwd(a)), cs(extra), vu(err, (int)sizeof(err)));
  memset(extra, 0, sizeof extra);
  if (!started) {
    cd_failed(a->page, err[0] ? err : "could not start worker");
    return 0;
  }
  a->worker_is_agy = want_kind;
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
      char err[512] = {0}, extra[2200];
      int want_claude = cd_agent_claude(a->page, next) != 0 && a->claude_worker_bin[0] != '\0';
      const char *sub_bin = want_claude ? a->claude_worker_bin : a->worker_bin;
      worker_extra(a, 0, extra, sizeof extra);
      int started = oa_core_subagent_start(vc(a), slot, cs(sub_bin), cs(effective_cwd(a)), cs(extra), vu(err, (int)sizeof(err)));
      memset(extra, 0, sizeof extra);
      if (!started) {
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
  char ver[128] = {0}, url[1024] = {0}, sha[65] = {0}, notes[2048] = {0}, err[256] = {0};
  int64_t size = 0;
  int64_t r = oa_core_update_verify_full(
      (coax_string){buf, (int64_t)n}, cs(OA_APP_VERSION), OA_APP_BUILD, 0,
      vu(ver, (int)sizeof(ver)), vu(url, (int)sizeof(url)),
      vu(sha, (int)sizeof(sha)), (coax_v_s64){&size, 1},
      vu(notes, (int)sizeof(notes)), vu(err, (int)sizeof(err)));
  if (!a || !a->page) return;
  if (r > 0) {
    char label[256];
    snprintf(label, sizeof(label), "Update %s available", ver);
    snprintf(a->update_url, sizeof a->update_url, "%s", url);
    snprintf(a->update_version, sizeof a->update_version, "%s", ver);
    snprintf(a->update_sha, sizeof a->update_sha, "%s", sha);
    a->update_size = size;
    cd_update(a->page, 1, label, url);   // CP_UPD_AVAILABLE: the pill downloads, verifies & swaps (case 12)
  } else if (r == 0) {
    cd_update(a->page, 0, "", "");
    cd_toast(a->page, "OpenAgents is up to date.", 0);
  } else {
    cd_update(a->page, 4, "Update verification failed", err);
  }
}

static void check_update_from_feed(DesktopApp *a) {
  const char *manifest = getenv("OPENAGENTS_UPDATE_MANIFEST");
  if (manifest && manifest[0]) {
    check_update_from_file(a, manifest);
    return;
  }
  char url[1024] = {0};
  const char *override_url = getenv("OPENAGENTS_UPDATE_URL");
  if (override_url && override_url[0]) snprintf(url, sizeof url, "%s", override_url);
  else oa_core_update_url(vu(url, (int)sizeof url));
  if (!url[0]) { cd_update(a->page, 4, "Update check failed", "No update feed URL configured"); return; }
  char tmp[OA_PATH] = {0}, file[OA_PATH + 32];
  if (!oa_temp_dir(tmp, sizeof tmp)) { cd_update(a->page, 4, "Update check failed", "Could not create temp directory"); return; }
  snprintf(file, sizeof file, "%s/manifest.json", tmp);
  if (!oa_download_url(url, file)) {
    cd_update(a->page, 4, "Update check failed", "Could not fetch signed update feed");
    return;
  }
  check_update_from_file(a, file);
  remove(file);
}

static void install_verified_update(DesktopApp *a) {
  if (!a->update_url[0]) { cd_update(a->page, 0, "", ""); return; }
  if (!a->update_sha[0] || a->update_size <= 0) {
    oa_open_external(a->update_url);
    return;
  }
  cd_update(a->page, 2, "Downloading update...", a->update_version);
  char tmp[OA_PATH] = {0}, staged[OA_PATH + 64];
  if (!oa_temp_dir(tmp, sizeof tmp)) {
    cd_update(a->page, 4, "Update failed", "Could not create temp directory");
    return;
  }
  const char *base = strrchr(a->update_url, '/');
  const char *name = (base && base[1]) ? base + 1 : "openagents-update.bin";
  snprintf(staged, sizeof staged, "%s/%s", tmp, name);
  if (!oa_download_url(a->update_url, staged)) {
    cd_update(a->page, 4, "Update download failed", "Could not download update payload");
    return;
  }
  char err[256] = {0};
  if (!oa_core_update_verify_archive(cs(staged), cs(a->update_sha), a->update_size, vu(err, (int)sizeof err))) {
    remove(staged);
    cd_update(a->page, 4, "Update verification failed", err[0] ? err : "SHA-256 or size mismatch");
    return;
  }
  size_t nl = strlen(name);
  int is_archive = (nl > 4 && (strcmp(name + nl - 4, ".zip") == 0 || strcmp(name + nl - 4, ".deb") == 0 || strcmp(name + nl - 4, ".msi") == 0)) ||
                   (nl > 7 && strcmp(name + nl - 7, ".tar.gz") == 0);
  if (is_archive) {
    cd_update(a->page, 3, "Verified update downloaded", staged);
    oa_open_external(staged);
    return;
  }
  if (oa_swap_self(staged)) {
    cd_update(a->page, 3, "Update installed — restart OpenAgents", a->update_version);
    cd_toast(a->page, "Verified update installed. Restart OpenAgents to run the new version.", 0);
  } else {
    cd_update(a->page, 4, "Update swap failed", "Verified download kept in temp folder");
    oa_open_external(tmp);
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
  char full[OA_PATH];
  if (oa_absolute(path)) snprintf(full, sizeof(full), "%s", path);
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
  char name[OA_PATH] = {0};
  if (cd_file_name(a->page, name, (int)sizeof(name)) <= 0) return;
  int n = cd_file_text(a->page, a->save_buf, OA_SAVE_CAP);
  if (n < 0) { cd_file_save_failed(a->page, "file too large"); return; }
  char full[OA_PATH];
  if (oa_absolute(name)) snprintf(full, sizeof(full), "%s", name);
  else snprintf(full, sizeof(full), "%s/%s", effective_cwd(a), name);
  FILE *f = fopen(full, "wb");
  if (!f) { cd_file_save_failed(a->page, strerror(errno)); return; }
  size_t w = fwrite(a->save_buf, 1, (size_t)n, f);
  fclose(f);
  if ((int)w != n) { cd_file_save_failed(a->page, "short write"); return; }
  cd_file_saved(a->page, a->save_buf, n);
}

// A path the page names, whole: as it is when it names a place on its own, else under the
// working folder.
static void full_path(DesktopApp *a, const char *path, char *out, size_t cap) {
  if (oa_absolute(path)) snprintf(out, cap, "%s", path);
  else snprintf(out, cap, "%s/%s", effective_cwd(a), path);
}

// Dialogs drawn by the page (page/overlay.coax): one at a time, answered in pump_dialog.
enum { OA_DIALOG_NONE, OA_DIALOG_CLAUDE, OA_DIALOG_OPENROUTER, OA_DIALOG_BUDGET, OA_DIALOG_GOAL, OA_DIALOG_RENAME, OA_DIALOG_DELETE, OA_DIALOG_AGY, OA_DIALOG_CLEAR_FOREMAN };

static void show_dialog(DesktopApp *a, int kind, int confirm, int cancel) {
  a->dialog = kind;
  a->dialog_status = cd_goal_status(a->page);
  cd_dialog_show(a->page, confirm, cancel, 0);
}

// The Claude or OpenRouter key, kept as the Mac app keeps it (store.coax dc_key_save).
static void ask_key(DesktopApp *a, int kind) {
  int claude = kind == OA_DIALOG_CLAUDE;
  char kept[1024] = {0};
  oa_core_key_read(cs(claude ? "claude-key" : "openrouter-key"), vu(kept, (int)sizeof kept));
  char body[512];
  if (claude) snprintf(body, sizeof body, "Claude sub-agents use this key. Paste an Anthropic API key (sk-ant-...). OpenAgents keeps it in a file only you can read.%s", kept[0] ? " A key is set now." : "");
  else snprintf(body, sizeof body, "Microcoder uses this key to reach models through OpenRouter. Paste your key (sk-or-...). OpenAgents keeps it in a file only you can read.%s", kept[0] ? " A key is set now." : "");
  memset(kept, 0, sizeof kept);
  cd_dialog_begin(a->page, 1, claude ? "Claude Key" : "OpenRouter Key", body);
  cd_dialog_field(a->page, "", "", claude ? "sk-ant-..." : "sk-or-...", 1, 1);
  cd_dialog_button(a->page, "Remove Key", 2, 1);
  cd_dialog_button(a->page, "Cancel", 1, 0);
  cd_dialog_button(a->page, "Save", 0, 0);
  show_dialog(a, kind, 2, 1);
}

static void ask_budget(DesktopApp *a) {
  char body[256], value[64];
  snprintf(body, sizeof body, "Set the most this chat may spend, in US dollars. Spent so far: $%.2f.", (double)cd_cost_usd_micros(a->page) / 1e6);
  snprintf(value, sizeof value, "%.2f", (double)cd_budget_usd_micros(a->page) / 1e6);
  cd_dialog_begin(a->page, 0, "Chat Max Budget", body);
  cd_dialog_field(a->page, "", value, "50.00", 0, 1);
  cd_dialog_button(a->page, "Cancel", 1, 0);
  cd_dialog_button(a->page, "Save", 0, 0);
  show_dialog(a, OA_DIALOG_BUDGET, 1, 0);
}

static void ask_goal(DesktopApp *a) {
  int status = cd_goal_status(a->page);
  char current[4097] = {0};
  if (cd_goal_objective(a->page, current, (int)sizeof current) <= 0) current[0] = '\0';
  cd_dialog_begin(a->page, 1, status == 0 ? "Set Thread Goal" : "Edit Thread Goal",
                  "Describe a long-running objective. Coder works on it across turns until it is done, paused, or reaches the chat budget.");
  cd_dialog_field(a->page, "", current, "", 0, 5);
  int n = 0;
  if (status != 0) { cd_dialog_button(a->page, "Clear", 1, 1); cd_dialog_button(a->page, status == 1 ? "Pause" : "Resume", 1, 1); n = 2; }
  cd_dialog_button(a->page, "Cancel", 1, 0);
  cd_dialog_button(a->page, status == 0 ? "Start Goal" : "Save Goal", 0, 0);
  show_dialog(a, OA_DIALOG_GOAL, n + 1, n);
}

static void ask_rename(DesktopApp *a) {
  if (!a->chat_id[0]) { cd_toast(a->page, "Send a message first: a new chat has nothing to rename.", 0); return; }
  char title[256] = {0};
  if (cd_title(a->page, title, (int)sizeof title) <= 0) title[0] = '\0';
  cd_dialog_begin(a->page, 0, "Rename Chat", "Enter a new title for this chat.");
  cd_dialog_field(a->page, "", title, "Chat title", 0, 1);
  cd_dialog_button(a->page, "Cancel", 1, 0);
  cd_dialog_button(a->page, "Rename", 0, 0);
  show_dialog(a, OA_DIALOG_RENAME, 1, 0);
  cd_dialog_select_all(a->page);
}

static void ask_delete(DesktopApp *a) {
  if (!a->chat_id[0]) return;
  if (cd_running(a->page)) { cd_toast(a->page, "Stop the reply before you delete this chat.", 0); return; }
  cd_dialog_begin(a->page, 0, "Delete Chat", "This chat and its messages are removed from this computer.");
  cd_dialog_button(a->page, "Cancel", 1, 0);
  cd_dialog_button(a->page, "Delete", 2, 0);
  show_dialog(a, OA_DIALOG_DELETE, 1, 0);
}

static void ask_agy(DesktopApp *a) {
  cd_dialog_begin(a->page, 1, "Antigravity CLI",
                  "Path to the Antigravity CLI (agy / agy.cmd). Leave blank to use agy on PATH or ~/.openagents/runtimes/agy.");
  cd_dialog_field(a->page, "", a->agy_bin, "agy", 0, 1);
  cd_dialog_button(a->page, "Reset", 1, 1);
  cd_dialog_button(a->page, "Cancel", 1, 0);
  cd_dialog_button(a->page, "Save", 0, 0);
  show_dialog(a, OA_DIALOG_AGY, 2, 1);
}

static void ask_clear_foreman(DesktopApp *a) {
  cd_dialog_begin(a->page, 0, "Clear Foreman history?",
                  "This removes the Foreman conversation history. Managed chats are not deleted.");
  cd_dialog_button(a->page, "Cancel", 1, 0);
  cd_dialog_button(a->page, "Clear History", 2, 0);
  show_dialog(a, OA_DIALOG_CLEAR_FOREMAN, 1, 0);
}

static void new_chat(DesktopApp *a);

// The answer to the dialog that is up, once the person gave one.
static void pump_dialog(DesktopApp *a) {
  if (!a->dialog) return;
  int answer = cd_dialog_answer(a->page);
  if (answer == -2) return;
  int kind = a->dialog;
  a->dialog = OA_DIALOG_NONE;
  char value[4097] = {0};
  if (cd_dialog_value(a->page, 0, value, (int)sizeof value) < 0) value[0] = '\0';
  char *v = value;
  while (*v == ' ' || *v == '\n' || *v == '\r' || *v == '\t') v++;
  size_t vn = strlen(v);
  while (vn > 0 && (v[vn - 1] == ' ' || v[vn - 1] == '\n' || v[vn - 1] == '\r' || v[vn - 1] == '\t')) v[--vn] = '\0';
  switch (kind) {
    case OA_DIALOG_CLAUDE: case OA_DIALOG_OPENROUTER: {
      const char *file = kind == OA_DIALOG_CLAUDE ? "claude-key" : "openrouter-key";
      if (answer == 2 && vn > 0) cd_toast(a->page, oa_core_key_save(cs(file), cs(v)) ? "Key saved." : "The key could not be saved.", 0);
      else if (answer == 0) cd_toast(a->page, oa_core_key_save(cs(file), cs("")) ? "Key removed." : "The key could not be removed.", 0);
      memset(value, 0, sizeof value);
      refresh_settings_info(a);
      break;
    }
    case OA_DIALOG_BUDGET: {
      if (answer != 1) break;
      char clean[64]; size_t n = 0;
      for (const char *p = v; *p && n + 1 < sizeof clean; p++) if (*p != '$' && *p != ',') clean[n++] = *p;
      clean[n] = '\0';
      char *end = NULL;
      double dollars = strtod(clean, &end);
      if (end == clean || *end || !(dollars > 0) || dollars > 1000000) { cd_toast(a->page, "Enter an amount in dollars, such as 50.00.", 0); break; }
      cd_set_budget_usd_micros(a->page, (int64_t)(dollars * 1e6 + 0.5));
      break;
    }
    case OA_DIALOG_GOAL: {
      int shift = a->dialog_status != 0 ? 2 : 0;
      if (answer == shift + 1) { if (vn > 0) cd_goal_set(a->page, v); }
      else if (shift && answer == 1) cd_goal_set_status(a->page, a->dialog_status == 1 ? 2 : 1);
      else if (shift && answer == 0) cd_goal_clear(a->page);
      break;
    }
    case OA_DIALOG_RENAME:
      if (answer == 1 && vn > 0 && a->chat_id[0]) { oa_core_store_rename(vc(a), cs(a->chat_id), cs(v)); refresh_chats(a); }
      break;
    case OA_DIALOG_DELETE:
      if (answer == 1 && a->chat_id[0] && !cd_running(a->page)) {
        char id[128]; snprintf(id, sizeof id, "%s", a->chat_id);
        new_chat(a);
        oa_core_store_remove(vc(a), cs(id));
        refresh_chats(a);
      }
      break;
    case OA_DIALOG_AGY:
      if (answer == 2) {
        snprintf(a->agy_bin, sizeof a->agy_bin, "%s", v);
        refresh_settings_info(a);
        cd_toast(a->page, a->agy_bin[0] ? "Antigravity CLI path updated." : "Antigravity CLI reset to auto-detect.", 0);
      } else if (answer == 0) {
        a->agy_bin[0] = '\0';
        refresh_settings_info(a);
        cd_toast(a->page, "Antigravity CLI reset to auto-detect.", 0);
      }
      break;
    case OA_DIALOG_CLEAR_FOREMAN:
      if (answer == 1) {
        if (strcmp(a->chat_id, "foreman") == 0) new_chat(a);
        oa_core_store_remove(vc(a), cs("foreman"));
        refresh_chats(a);
        cd_toast(a->page, "Foreman history cleared.", 0);
      }
      break;
    default: break;
  }
  if (cd_settings_page(a->page) < 0) cd_focus(a->page, 1);
}

// The working folder's files, for the composer's @ list.
static void list_files(DesktopApp *a) {
  int n = oa_core_files(cs(effective_cwd(a)), vu(a->save_buf, OA_SAVE_CAP));
  a->save_buf[n > 0 ? n : 0] = '\0';
  cd_files(a->page, a->save_buf);
}

// A new id for a steer or a queued message.
static void new_id(char *out, int cap) { oa_core_new_id(vu(out, cap)); }

// The tray and the composer while a reply runs (cd_action 25; page/followups.coax).
static void handle_followup(DesktopApp *a) {
  int op = cd_followup_op(a->page), item = cd_followup_item(a->page);
  char id[128] = {0}, text[16384] = {0};
  int alive = oa_core_worker_alive(vc(a));
  int n = 0;
  switch (op) {
    case 20: case 21: case 9: { // steer the draft, queue it, or queue a steer the reply did not take
      if (op == 9) {
        if (item < 0 || cd_steer_text(a->page, item, text, (int)sizeof text) <= 0) return;
      } else if (cd_draft_message(a->page, text, (int)sizeof text) <= 0 || !text[0]) return;
      if (!alive) { cd_toast(a->page, "The reply ended. Send the message again.", 0); return; }
      new_id(id, (int)sizeof id);
      if (op == 20) n = cd_steer_line(a->page, id, text, a->line_buf, OA_LINE_CAP);
      if (n <= 0) {
        int o = cd_queue_line(a->page, "open", "", 0, 0, a->chat_id, a->line_buf, OA_LINE_CAP);
        if (o > 0) oa_core_worker_write(vc(a), (coax_string){a->line_buf, o});
        n = cd_queue_message_line(a->page, "add", id, text, a->chat_id, effective_cwd(a), a->line_buf, OA_LINE_CAP);
      }
      if (n > 0) oa_core_worker_write(vc(a), (coax_string){a->line_buf, n});
      if (op == 9) cd_steer_dismiss(a->page, item);
      else { cd_set_text(a->page, ""); cd_attach_clear(a->page); }
      break;
    }
    case 22: { // the draft is the edited item's new text
      int at = cd_followup_edited(a->page);
      if (at >= 0 && alive && cd_queue_item_id(a->page, at, id, (int)sizeof id) > 0 && cd_draft(a->page, text, (int)sizeof text) > 0 && text[0]) {
        n = cd_queue_message_line(a->page, "update", id, text, a->chat_id, effective_cwd(a), a->line_buf, OA_LINE_CAP);
        if (n > 0) oa_core_worker_write(vc(a), (coax_string){a->line_buf, n});
      }
      cd_set_text(a->page, "");
      cd_followup_editing(a->page, -1);
      break;
    }
    case 23: cd_set_text(a->page, ""); cd_followup_editing(a->page, -1); break;
    case 5: // edit a queued item in the composer
      if (item >= 0 && cd_queue_item_text(a->page, item, text, (int)sizeof text) > 0) {
        cd_followup_editing(a->page, item);
        cd_set_text(a->page, text);
        cd_focus(a->page, 1);
      }
      break;
    case 7: case 24: // send the waiting steer now (stop and resubmit), or stop the reply
      n = cd_interrupt_line(a->page, op == 7, a->line_buf, OA_LINE_CAP);
      if (n > 0 && alive) oa_core_worker_write(vc(a), (coax_string){a->line_buf, n});
      break;
    case 1: case 2: case 3: case 4: case 6: { // resume, clear, delete, move up, send now
      static const char *const ops[] = {"", "resume", "clear", "delete", "move", "", "promote"};
      if (op != 1 && op != 2 && (item < 0 || cd_queue_item_id(a->page, item, id, (int)sizeof id) <= 0)) return;
      if (!alive && !ensure_worker(a)) return;
      n = cd_queue_line(a->page, ops[op], id, op == 4 ? (item > 0 ? item - 1 : 0) : 0, op == 6, a->chat_id, a->line_buf, OA_LINE_CAP);
      if (n > 0) oa_core_worker_write(vc(a), (coax_string){a->line_buf, n});
      break;
    }
    default: break;
  }
}

// What a control asks for that this app does not have yet: said, so no control does nothing.
static void not_here(DesktopApp *a, const char *what) {
  char msg[256];
  snprintf(msg, sizeof msg, "%s is in the Mac app. It is not on %s yet.", what, OA_PLATFORM_NAME);
  cd_toast(a->page, msg, 0);
}

// The shown chat copied into a new chat with the same turns and folder, then opened (/fork).
static void fork_shown(DesktopApp *a) {
  int n = cd_save(a->page, a->save_buf, OA_SAVE_CAP);
  if (n <= 0 || cd_running(a->page)) return;
  char title[256] = "New chat", forked[300], id[128] = {0};
  if (cd_title(a->page, title, (int)sizeof title) <= 0) snprintf(title, sizeof title, "New chat");
  size_t tl = strlen(title);
  if (tl >= 7 && strcmp(title + tl - 7, " (fork)") == 0) snprintf(forked, sizeof forked, "%s", title);
  else snprintf(forked, sizeof forked, "%s (fork)", title);
  oa_core_new_id(vu(id, (int)sizeof id));
  if (!oa_core_store_keep(vc(a), cs(id), cs(forked), cs(a->folder), (coax_string){a->save_buf, n}, cs(""), cs(""), cs(""))) {
    cd_toast(a->page, "The chat could not be copied.", 1);
    return;
  }
  refresh_chats(a);
  open_chat_by_id(a, id);
}

// A new chat: the worker forgets the shown one, which stays kept.
static void new_chat(DesktopApp *a) {
  int n = cd_forget_line(a->line_buf, OA_LINE_CAP);
  if (n > 0 && oa_core_worker_alive(vc(a))) oa_core_worker_write(vc(a), (coax_string){a->line_buf, n});
  stop_all_subagents(a);
  a->chat_id[0] = '\0';
  a->worktree_path[0] = '\0';
  a->worktree_branch[0] = '\0';
  a->worktree_repo[0] = '\0';
  cd_set_text(a->page, "");
  cd_branch(a->page, "");
  cd_current(a->page, "");
  cd_focus(a->page, 1);
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
    case 2: cd_focus(a->page, 1); break;
    case 3: { // stop run
      int n = cd_stop_line(a->line_buf, OA_LINE_CAP);
      if (n > 0 && oa_core_worker_alive(vc(a))) oa_core_worker_write(vc(a), (coax_string){a->line_buf, n});
      stop_all_subagents(a);
      break;
    }
    case 4: new_chat(a); break;
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
    case 8: ask_budget(a); break;
    case 9: ask_goal(a); break;
    case 10: { // pinned chat drag reorder
      char id[128] = {0};
      int to = -1;
      if (cd_pin_move(a->page, id, (int)sizeof(id), &to) > 0 && to >= 0) {
        oa_core_store_move_pin(vc(a), cs(id), to);
      }
      refresh_chats(a);
      break;
    }
    case 11: { // open Foreman chat
      if (cd_running(a->page)) { cd_toast(a->page, "Stop the current reply before switching to Foreman.", 0); break; }
      if (strcmp(a->chat_id, "foreman") == 0) { cd_focus(a->page, 1); break; }
      int n = (int)oa_core_store_text(vc(a), cs("foreman"), vu(a->save_buf, OA_SAVE_CAP));
      if (n > 0) {
        open_chat_by_id(a, "foreman");
      } else {
        new_chat(a);
        snprintf(a->chat_id, sizeof a->chat_id, "foreman");
        cd_current(a->page, "foreman");
      }
      cd_foreman(a->page, 0, a->foreman_tier);
      cd_focus(a->page, 1);
      break;
    }
    case 12: // the update pill: download, verify SHA-256 + size, and swap/open
      install_verified_update(a);
      break;
    case 18: { // Foreman permission tier
      int tier = cd_settings_tier(a->page);
      a->foreman_tier = tier;
      oa_core_daemon_update_foreman(vc(a), tier, cs(""));
      cd_foreman(a->page, 0, a->foreman_tier);
      break;
    }
    case 19: { // Foreman audit log
      char hb[OA_PATH] = {0}, audit[OA_PATH + 64];
      const char *home = oa_home(hb, sizeof hb);
      if (home && home[0]) {
        snprintf(audit, sizeof audit, "%s/.openagents/coder/foreman-audit.jsonl", home);
        if (!file_exists(audit)) {
          FILE *f = fopen(audit, "ab");
          if (f) fclose(f);
        }
        oa_open_external(audit);
      }
      break;
    }
    case 20: ask_clear_foreman(a); break;
    case 13: if (cd_settings_page(a->page) < 0) cd_focus(a->page, 1); else refresh_settings_info(a); break;
    case 14: cd_zoom(a->page, 1); break;
    case 15: cd_zoom(a->page, -1); break;
    case 16: ask_key(a, OA_DIALOG_CLAUDE); break;
    case 17: ask_key(a, OA_DIALOG_OPENROUTER); break;
    case 21: oa_open_external("https://openagents.com/login"); break;
    case 22: check_update_from_feed(a); break;
    case 23: cd_focus(a->page, 2); break;
    case 24: { // a file a row or a reply names: opened in the file pane
      char path[OA_PATH] = {0};
      if (cd_open_path(a->page, path, (int)sizeof(path)) > 0) handle_file_open(a, path);
      break;
    }
    case 25: handle_followup(a); break;
    case 26: { // the command palette: rename, delete and stop all need the Mac app's dialogs
      int cmd = cd_palette_command(a->page);
      if (cmd == 16) ask_rename(a);
      else if (cmd == 17) ask_delete(a);
      else if (cmd == 18) handle_action(a, 3);
      break;
    }
    case 27: { // attach file via the file chooser
      if (a->portal) portal_pick(a->portal, OA_PORTAL_ATTACH, 0, "Attach file", a->folder);
      break;
    }
    case 28: list_files(a); break;
    case 29: cd_zoom_set(a->page, 2); break;
    case 30: break;   // a sub-agent stopped: pump_subagents reads cd_stop_agent each pass
    case 31: { // an http(s) link: the system browser
      char url[2048] = {0};
      if (cd_open_url(a->page, url, (int)sizeof(url)) > 0 && (strncmp(url, "https://", 8) == 0 || strncmp(url, "http://", 7) == 0)) oa_open_external(url);
      break;
    }
    case 32: {
      // Cross-platform Coax Lean 4 & Riemann Hypothesis Proof Workbench (#13946)
      cd_gym_set(a->page, 1);
      cd_gym_mode_set(a->page, 2);
      char hb[OA_PATH], ap[OA_PATH];
      const char *home = oa_home(hb, sizeof hb);
      if (home && home[0]) {
        snprintf(ap, sizeof ap, "%s/.openagents/lean/index/rh-atlas.json", home);
        FILE *f = fopen(ap, "rb");
        if (f) {
          fseek(f, 0, SEEK_END);
          long sz = ftell(f);
          fseek(f, 0, SEEK_SET);
          if (sz > 0 && sz < 262144) {
            char *buf = (char *)malloc((size_t)sz + 1);
            if (buf) {
              size_t got = fread(buf, 1, (size_t)sz, f);
              buf[got] = 0;
              cd_lean_atlas_set(a->page, buf);
              free(buf);
            }
          }
          fclose(f);
        }
      }
      break;
    }
    case 150: {
      if (cd_gym_mode(a->page) != 2) {
        cd_gym_mode_set(a->page, 2);
        cd_toast(a->page, "Benchmarks & 3D PLE require macOS; Lean · RH Proof Workbench runs natively here.", 0);
      }
      break;
    }
    case 156: {
      char path[OA_PATH] = {0};
      if (cd_lean_req_str(a->page, path, (int)sizeof(path)) > 0) {
        cd_gym_set(a->page, 0);
        handle_file_open(a, path);
      }
      break;
    }
    case 33: ask_agy(a); break;
    case 34:
      a->agy_mode = cd_settings_agy_mode(a->page);
      refresh_settings_info(a);
      break;
    case 35:
      cd_toast(a->page, "Run 'agy auth login' in a terminal to sign in to Antigravity.", 0);
      break;
    case 97: {
      // Settings > Models: the model new chats start on, kept beside the keys
      char pick[16];
      snprintf(pick, sizeof pick, "%s%d", cd_orchestrator(a->page) ? "claude:" : "neon:", cd_orchestrator_model(a->page));
      if (!oa_core_key_save(cs("orchestrator"), cs(pick))) cd_toast(a->page, "The setting could not be saved.", 0);
      break;
    }
    case 98: {
      // Settings > Models: each Claude model's thinking level, kept beside the keys as four digits
      char levels[8];
      for (int k = 0; k < 4; k++) levels[k] = (char)('0' + cd_claude_effort(a->page, k));
      levels[4] = '\0';
      if (!oa_core_key_save(cs("claude-effort"), cs(levels))) cd_toast(a->page, "The setting could not be saved.", 0);
      break;
    }
    case 36: break;   // Experimental features: the page shows or hides them; Foreman and the Gym are Mac-only
    case 37: fork_shown(a); break;
    case 40: handle_file_save(a); break;
    case 41: {
      char name[OA_PATH] = {0}, full[OA_PATH];
      if (cd_file_name(a->page, name, (int)sizeof(name)) > 0) { full_path(a, name, full, sizeof full); oa_open_external(full); }
      break;
    }
    case 42: cd_file_close(a->page); break;
    case 52: {
      int op = cd_term_op(a->page);
      int tid = cd_term_op_tab(a->page);
      if (tid <= 0) tid = cd_term_active_id(a->page);
      int slot = -1;
      for (int i = 0; i < OA_TERMS; i++) {
        if (a->term_active[i] && a->term_tab_id[i] == tid) { slot = i; break; }
      }
      if (op == 1) {
        if (slot < 0) {
          for (int i = 0; i < OA_TERMS; i++) { if (!a->term_active[i]) { slot = i; break; } }
          if (slot >= 0) {
            char cwd[OA_PATH] = {0};
            if (cd_term_tab_cwd(a->page, tid, cwd, (int)sizeof(cwd)) <= 0) snprintf(cwd, sizeof(cwd), "%s", a->folder);
            int pid = -1;
            int h = cd_pty_spawn(NULL, cwd, 100, 28, &pid);
            if (h >= 0) {
              a->term_active[slot] = 1;
              a->term_tab_id[slot] = tid;
              a->term_handle[slot] = h;
              a->term_pid[slot] = pid;
            }
          }
        }
      } else if (op == 2) {
        if (slot >= 0) {
          cd_pty_close(a->term_handle[slot], a->term_pid[slot]);
          a->term_active[slot] = 0;
          a->term_handle[slot] = -1;
          a->term_pid[slot] = 0;
        }
      } else if (op == 3) {
        int cols = 0, rows = 0;
        if (slot >= 0 && cd_term_resize_due(a->page, &cols, &rows)) {
          cd_pty_resize(a->term_handle[slot], cols, rows);
        }
      } else if (op == 4) {
        char inbuf[4096];
        int n = cd_term_input_out(a->page, inbuf, (int)sizeof(inbuf));
        if (slot >= 0 && n > 0) (void)cd_pty_write(a->term_handle[slot], inbuf, n);
      } else if (op == 5) {
        char sel[16384] = {0};
        int sn = cd_term_selected(a->page, sel, (int)sizeof(sel));
        if (sn > 0) {
          char hb[OA_PATH], dir[OA_PATH], path[OA_PATH];
          const char *home = oa_home(hb, sizeof hb);
          if (home && home[0]) {
            snprintf(dir, sizeof dir, "%s/.openagents/terminal-context", home);
            oa_mkdir(dir);
            snprintf(path, sizeof path, "%s/term-%d.txt", dir, tid);
            FILE *f = fopen(path, "wb");
            if (f) { fwrite(sel, 1, (size_t)sn, f); fclose(f); cd_attach(a->page, path); }
          }
        }
      }
      break;
    }
    default: break;
  }
}

static void pump_terms(DesktopApp *a) {
  char buf[4096];
  for (int i = 0; i < OA_TERMS; i++) {
    if (!a->term_active[i]) continue;
    for (int guard = 0; guard < 8; guard++) {
      int n = cd_pty_read(a->term_handle[i], buf, (int)sizeof(buf));
      if (n > 0) {
        cd_term_feed(a->page, a->term_tab_id[i], buf, n);
      } else if (n < 0) {
        cd_term_exited(a->page, a->term_tab_id[i], 0);
        cd_pty_close(a->term_handle[i], a->term_pid[i]);
        a->term_active[i] = 0;
        a->term_handle[i] = -1;
        a->term_pid[i] = 0;
        break;
      } else {
        break;
      }
    }
  }
  int cols = 0, rows = 0;
  if (cd_term_resize_due(a->page, &cols, &rows)) {
    int tid = cd_term_active_id(a->page);
    for (int i = 0; i < OA_TERMS; i++) {
      if (a->term_active[i] && a->term_tab_id[i] == tid) {
        cd_pty_resize(a->term_handle[i], cols, rows);
        break;
      }
    }
  }
}

static void stop_all_terms(DesktopApp *a) {
  for (int i = 0; i < OA_TERMS; i++) {
    if (a->term_active[i]) {
      cd_pty_close(a->term_handle[i], a->term_pid[i]);
      a->term_active[i] = 0;
      a->term_handle[i] = -1;
      a->term_pid[i] = 0;
    }
  }
}

static void pump_host(DesktopApp *a) {
  if (a->portal) portal_poll(a->portal, on_portal_chosen, a);
  pump_dialog(a);
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
  pump_terms(a);
  if (cd_save_due(a->page)) save_current_chat(a);
  if (a->interactive) {
    static int oa_last_running = 0;
    int now_running = cd_running(a->page) ? 1 : 0;
    if (now_running != oa_last_running) {
      oa_keep_awake(now_running);
      if (oa_last_running && !now_running) {
        oa_notify_os("OpenAgents", "Agent turn completed");
      }
      oa_last_running = now_running;
    }
  }
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

// Headless end-to-end smoke test (GitHub #13781, #13782):
// 1. Opens page.coax offscreen on Vulkan via ui-host.c and renders frames (+ optional PNG).
// 2. Runs a Microcoder turn through page.coax + desktop core store persistence in a temp dir.
// 3. Exercises git worktree creation through desktop core.
// 4. Exercises signed update envelope verification through desktop core.
static int run_smoke(DesktopApp *a, const char *shot_out) {
  char tmp[OA_PATH];
  if (!oa_temp_dir(tmp, sizeof tmp)) { fprintf(stderr, "smoke: could not make a temporary folder\n"); return 1; }

  char store_dir[OA_PATH], daemon_dir[OA_PATH];
  snprintf(store_dir, sizeof(store_dir), "%s/chats", tmp);
  snprintf(daemon_dir, sizeof(daemon_dir), "%s/daemon", tmp);
  oa_mkdir(store_dir);
  oa_mkdir(daemon_dir);

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
  if (!cd_set_text(a->page, "Summarize host readiness")) {
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
  cd_line(a->page, "{\"kind\":\"reply\",\"text\":\"Host ready on Vulkan with desktop core.\"}");
  cd_line(a->page, "{\"kind\":\"done\"}");
  save_current_chat(a);

  // Reload the persisted chat from disk and verify it round-trips.
  if (oa_core_store_count(vc(a)) < 1) {
    fprintf(stderr, "smoke: persisted chat count < 1\n");
    return 1;
  }
  int loaded = (int)oa_core_store_text(vc(a), cs(a->chat_id), vu(a->save_buf, OA_SAVE_CAP));
  if (loaded <= 0 || !strstr(a->save_buf, "Host ready")) {
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
  char repo_dir[OA_PATH], cmd[OA_PATH * 2];
  snprintf(repo_dir, sizeof(repo_dir), "%s/repo", tmp);
  // double quotes: both sh and cmd.exe take them; README.md is made by the program, not touch
  snprintf(cmd, sizeof(cmd), "git init -q \"%s\"", repo_dir);
  int repo_ok = system(cmd) == 0;
  char readme[OA_PATH];
  snprintf(readme, sizeof readme, "%s/README.md", repo_dir);
  FILE *rf = repo_ok ? fopen(readme, "wb") : NULL;
  if (rf) fclose(rf);
  snprintf(cmd, sizeof(cmd),
           "git -C \"%s\" -c user.email=smoke@openagents.com -c user.name=Smoke add README.md && "
           "git -C \"%s\" -c user.email=smoke@openagents.com -c user.name=Smoke commit -q -m init",
           repo_dir, repo_dir);
  if (rf && system(cmd) == 0) {
    char wp[OA_PATH] = {0}, wb[256] = {0}, wr[OA_PATH] = {0}, why[512] = {0};
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

  // 5. The worker beside the program (the Windows zip's mcworker.exe) starts and says it is ready.
  if (file_exists(a->worker_bin)) {
    char err[512] = {0}, extra[1100];
    worker_extra(a, 0, extra, sizeof extra);
    if (!oa_core_worker_start(vc(a), cs(a->worker_bin), cs(tmp), cs(extra), vu(err, (int)sizeof(err)))) {
      fprintf(stderr, "smoke: the worker did not start: %s\n", err);
      return 1;
    }
    int ready = 0;
    for (int i = 0; i < 100 && !ready; i++) {
      int n = (int)oa_core_worker_read(vc(a), 100, vu(a->line_buf, OA_LINE_CAP));
      if (n == -2) break;
      if (n > 0 && strstr(a->line_buf, "\"kind\":\"ready\"")) ready = 1;
    }
    oa_core_worker_stop(vc(a), 0);
    if (!ready) { fprintf(stderr, "smoke: the worker never said it was ready\n"); return 1; }
    printf("smoke: worker %s ready\n", a->worker_bin);
  }

  // 6. Live integrated terminal PTY smoke check (#13869): spawn shell, send echo, read output, resize, close.
  {
    int tid = cd_term_new_tab(a->page, "smoke", tmp);
    int pty_pid = -1;
    int pty_h = cd_pty_spawn(NULL, tmp, 80, 24, &pty_pid);
    if (tid <= 0 || pty_h < 0) {
      fprintf(stderr, "smoke: cd_pty_spawn failed (tid=%d, h=%d)\n", tid, pty_h);
      return 1;
    }
    (void)cd_pty_resize(pty_h, 100, 30);
#ifdef _WIN32
    const char *probe = "echo OA_PTY_OK\r\n";
#else
    const char *probe = "echo OA_PTY_OK\n";
#endif
    (void)cd_pty_write(pty_h, probe, (int)strlen(probe));
    int saw_pty = 0;
    char acc[8192];
    int acc_n = 0;
    acc[0] = '\0';
    for (int i = 0; i < 150 && !saw_pty; i++) {
      if (i == 25 || i == 75) (void)cd_pty_write(pty_h, probe, (int)strlen(probe));
      char rbuf[1024];
      int n = cd_pty_read(pty_h, rbuf, (int)sizeof(rbuf));
      if (n > 0) {
        cd_term_feed(a->page, tid, rbuf, n);
        int take = n < ((int)sizeof(acc) - 1 - acc_n) ? n : ((int)sizeof(acc) - 1 - acc_n);
        if (take > 0) { memcpy(acc + acc_n, rbuf, (size_t)take); acc_n += take; acc[acc_n] = '\0'; }
        if (strstr(acc, "OA_PTY_OK")) saw_pty = 1;
      } else {
#ifdef _WIN32
        Sleep(20);
#else
        struct timespec ts = {0, 20000000L};
        nanosleep(&ts, NULL);
#endif
      }
    }
    cd_pty_close(pty_h, pty_pid);
    if (!saw_pty) {
      fprintf(stderr, "smoke: PTY never echoed OA_PTY_OK (got %d bytes)\n", acc_n);
      return 1;
    }
    printf("smoke: pty ok (%s, %d bytes)\n", OA_PLATFORM, acc_n);
  }

  cd_close(a->page);
  a->page = NULL;
  printf("openagents-desktop-%s smoke: ok (chat=%s, store=%d)\n", OA_PLATFORM, a->chat_id, (int)oa_core_store_count(vc(a)));
  return 0;
}

// The chat flow test (linux/flow-test.sh runs it): the real app's
// handlers (handle_action, pump_host, save_current_chat) on an offscreen page, a real worker
// process (--worker; the test gives the mock worker, never a model), and the chat store at the
// XDG data home. It sends a message, waits for the reply, makes a new chat and sends another,
// and checks that the list holds both chats, that both survive a restart, and that an index the
// store can't read is refused, never written over.
#define FLOW_W 1100
#define FLOW_H 760
static int flow_fail(const char *step, const char *why) {
  fprintf(stderr, "flow: FAIL %s: %s\n", step, why);
  return 1;
}
static void flow_ok(const char *step, const char *detail) {
  printf("flow: ok %s%s%s\n", step, detail && detail[0] ? ": " : "", detail ? detail : "");
  fflush(stdout);
}
static void flow_sleep_ms(int ms) {
#ifdef _WIN32
  Sleep((DWORD)ms);
#else
  struct timespec ts = {ms / 1000, (long)(ms % 1000) * 1000000L};
  nanosleep(&ts, NULL);
#endif
}
// Pumps the host and renders until the run has ended and its chat is kept (or ms passes).
static int flow_until_idle(DesktopApp *a, int ms) {
  int saw_running = 0;
  for (int t = 0; t < ms / 10; t++) {
    pump_host(a);
    if (!cd_shot_frame(a->page, FLOW_W, FLOW_H, 1.0f / 60.0f)) return 0;
    if (cd_running(a->page)) saw_running = 1;
    else if (saw_running || t > 20) {
      for (int k = 0; k < 5; k++) { pump_host(a); cd_shot_frame(a->page, FLOW_W, FLOW_H, 1.0f / 60.0f); }
      return 1;
    }
    flow_sleep_ms(10);
  }
  return 0;
}
static int flow_send(DesktopApp *a, const char *text) {
  if (!cd_set_text(a->page, text) || !cd_submit(a->page)) return 0;
  return flow_until_idle(a, 15000);
}
static int flow_count_lines(const char *list) {
  if (!list[0]) return 0;
  int n = 1;
  for (const char *p = list; *p; p++) if (*p == '\n') n++;
  return n;
}
static int flow_read_file(const char *path, char *buf, int cap) {
  FILE *f = fopen(path, "rb");
  if (!f) return -1;
  int n = (int)fread(buf, 1, (size_t)cap - 1, f);
  fclose(f);
  buf[n] = '\0';
  return n;
}
static int flow_write_file(const char *path, const char *text) {
  FILE *f = fopen(path, "wb");
  if (!f) return 0;
  size_t n = strlen(text);
  int ok = fwrite(text, 1, n, f) == n;
  fclose(f);
  return ok;
}
static int run_flow(DesktopApp *a, const char *shot_out) {
  char store[OA_PATH] = {0}, detail[OA_PATH + 128], list[65536];
  if (!oa_core_init(vc(a), cs(""), cs(""))) return flow_fail("init", "the chat store did not open");
  oa_core_store_path(vc(a), vu(store, (int)sizeof(store)));
#ifndef _WIN32
  const char *xdg = getenv("XDG_DATA_HOME");
  if (!xdg || !xdg[0] || strncmp(store, xdg, strlen(xdg)) != 0) return flow_fail("store", "the store is not under XDG_DATA_HOME");
#endif
  snprintf(detail, sizeof(detail), "store at %s, %d chats", store, (int)oa_core_store_count(vc(a)));
  flow_ok("store", detail);
  if (oa_core_store_count(vc(a)) != 0) return flow_fail("store", "the test store is not empty");

  a->page = cd_open_ex(a->resources, 1, FLOW_W, FLOW_H);
  if (!a->page) return flow_fail("page", "cd_open_ex(offscreen) failed");
  set_folder(a, a->folder);
  refresh_settings_info(a);
  refresh_chats(a);
  for (int i = 0; i < 4; i++) if (!cd_shot_frame(a->page, FLOW_W, FLOW_H, 1.0f / 60.0f)) return flow_fail("page", "a frame did not render");
  flow_ok("page", "rendered offscreen on Vulkan");

  // 1. A message to the worker, and its reply.
  if (!flow_send(a, "flow test first message")) return flow_fail("send-1", "the run did not end");
  char first[128];
  snprintf(first, sizeof(first), "%s", a->chat_id);
  if (!first[0]) return flow_fail("send-1", "no chat id");
  if (!oa_core_worker_alive(vc(a))) return flow_fail("send-1", "the worker is not running");
  int n = (int)oa_core_store_text(vc(a), cs(first), vu(a->save_buf, OA_SAVE_CAP));
  if (n <= 0 || !strstr(a->save_buf, "flow test first message")) return flow_fail("send-1", "the kept chat lacks the message");
  if (!strstr(a->save_buf, "First reply from the worker")) return flow_fail("reply-1", "the kept chat lacks the worker's reply");
  n = cd_save(a->page, a->save_buf, OA_SAVE_CAP);
  if (n <= 0 || !strstr(a->save_buf, "First reply from the worker")) return flow_fail("reply-1", "the page does not show the reply");
  flow_ok("chat-1", first);

  // 2. A new chat (the New chat button), and a second message.
  if (!cd_new(a->page)) return flow_fail("new-chat", "cd_new refused");
  pump_host(a);
  if (a->chat_id[0]) return flow_fail("new-chat", "the app kept the old chat id");
  oa_core_store_lines(vc(a), vu(list, (int)sizeof(list)));
  if (flow_count_lines(list) != 1) return flow_fail("new-chat", "the list lost the first chat");
  if (!flow_send(a, "flow test second message")) return flow_fail("send-2", "the run did not end");
  char second[128];
  snprintf(second, sizeof(second), "%s", a->chat_id);
  if (!second[0] || strcmp(first, second) == 0) return flow_fail("send-2", "the second message did not start a new chat");
  n = (int)oa_core_store_text(vc(a), cs(second), vu(a->save_buf, OA_SAVE_CAP));
  if (n <= 0 || !strstr(a->save_buf, "Second reply from the worker")) return flow_fail("reply-2", "the kept chat lacks the second reply");
  if (strstr(a->save_buf, "First reply from the worker")) return flow_fail("reply-2", "the new chat carries the first chat's turns");
  flow_ok("chat-2", second);

  // 3. The list holds both, newest first.
  oa_core_store_lines(vc(a), vu(list, (int)sizeof(list)));
  if (flow_count_lines(list) != 2 || !strstr(list, first) || !strstr(list, second)) return flow_fail("list", "the list does not hold both chats");
  if (strncmp(list, second, strlen(second)) != 0) return flow_fail("list", "the newest chat is not first");
  // Kept times are wall-clock seconds (the Mac app reads them as dates), not 1970.
  {
    char ib[OA_PATH + 16], idx[65536];
    snprintf(ib, sizeof(ib), "%s/index.json", store);
    const char *cr = flow_read_file(ib, idx, (int)sizeof(idx)) > 0 ? strstr(idx, "\"created\":") : NULL;
    if (!cr || atof(cr + 10) < 1600000000.0) return flow_fail("list", "a chat's created time is not now");
  }
  flow_ok("list", "2 chats, newest first, created now");

  // 4. Opening the first chat from the list shows its reply again.
  open_chat_by_id(a, first);
  for (int i = 0; i < 3; i++) cd_shot_frame(a->page, FLOW_W, FLOW_H, 1.0f / 60.0f);
  n = cd_save(a->page, a->save_buf, OA_SAVE_CAP);
  if (strcmp(a->chat_id, first) != 0 || n <= 0 || !strstr(a->save_buf, "First reply from the worker")) return flow_fail("open", "the first chat did not open with its reply");
  flow_ok("open", "the first chat reopened with its reply");
  if (shot_out && shot_out[0]) {
    if (!cd_shot_write(a->page, shot_out)) return flow_fail("shot", shot_out);
    flow_ok("shot", shot_out);
  }

  // 5. A restart reads both chats back from disk.
  coax_t_OaCore *again = calloc(1, sizeof(coax_t_OaCore));
  if (!again) return flow_fail("restart", "out of memory");
  coax_v_t_OaCore va = {again, 1};
  if (!oa_core_init(va, cs(""), cs("")) || oa_core_store_count(va) != 2) { free(again); return flow_fail("restart", "the store did not hold 2 chats after a restart"); }
  free(again);
  flow_ok("restart", "2 chats read back");

  // 6. An index the store can't read is never written over (the Mac wipe of 4b957c4de3).
  char index[OA_PATH + 16], saved[65536], after[65536];
  snprintf(index, sizeof(index), "%s/index.json", store);
  if (flow_read_file(index, saved, (int)sizeof(saved)) <= 0) return flow_fail("refuse", "no index.json");
  const char *damaged = "{\"chats\": [ this is not json";
  if (!flow_write_file(index, damaged)) return flow_fail("refuse", "could not damage the index");
  int kept = (int)oa_core_store_keep(vc(a), cs(first), cs("Overwrite"), cs(a->folder), cs("{}"), cs(""), cs(""), cs(""));
  if (kept) return flow_fail("refuse", "a keep wrote over an index it could not read");
  if (flow_read_file(index, after, (int)sizeof(after)) < 0 || strcmp(after, damaged) != 0) return flow_fail("refuse", "the damaged index was changed");
  if (!flow_write_file(index, saved)) return flow_fail("refuse", "could not put the index back");
  oa_core_store_lines(vc(a), vu(list, (int)sizeof(list)));
  if (flow_count_lines(list) != 2) return flow_fail("refuse", "the list did not come back");
  flow_ok("refuse", "a damaged index is left as it is; the list comes back when it reads again");

  // 7. A chat another process keeps meanwhile is not dropped by this one's next save.
  coax_t_OaCore *other = calloc(1, sizeof(coax_t_OaCore));
  if (!other) return flow_fail("shared", "out of memory");
  coax_v_t_OaCore vo = {other, 1};
  char third[128] = {0};
  oa_core_new_id(vu(third, (int)sizeof(third)));
  int ok3 = oa_core_init(vo, cs(""), cs("")) &&
            oa_core_store_keep(vo, cs(third), cs("Kept elsewhere"), cs(a->folder), cs("{}"), cs(""), cs(""), cs(""));
  free(other);
  if (!ok3) return flow_fail("shared", "the other store could not keep a chat");
  save_current_chat(a);
  oa_core_store_lines(vc(a), vu(list, (int)sizeof(list)));
  if (flow_count_lines(list) != 3 || !strstr(list, third)) return flow_fail("shared", "this app's save dropped a chat kept elsewhere");
  flow_ok("shared", "3 chats after a save beside another writer");

  stop_all_terms(a);
  stop_all_subagents(a);
  if (oa_core_worker_alive(vc(a))) oa_core_worker_stop(vc(a), 0);
  cd_close(a->page);
  a->page = NULL;
  printf("openagents-desktop-%s flow: ok\n", OA_PLATFORM);
  return 0;
}

#ifndef _WIN32
// The input test on Linux (linux/input-test.sh): the real window in a headless compositor, typed into
// with a virtual keyboard (wtype), pressed with a virtual pointer (vpointer) and pasted into from the
// clipboard (wl-copy), as a person would; every check reads the page back here. The keyboard is one
// this test holds and lets go of, so the seat losing its keyboard and getting one again is covered.
static FILE *input_pointer_in, *input_pointer_out;
static pid_t input_keyboard = -1, input_pointer_pid = -1;
static int input_fail(const char *step, const char *why) { fprintf(stderr, "input: FAIL %s: %s\n", step, why); return 1; }
static void input_ok(const char *step, const char *detail) { printf("input: ok %s: %s\n", step, detail); fflush(stdout); }
static int input_frames(DesktopApp *a, int ms) {
  for (int t = 0; t < ms / 16 + 1; t++) { if (!cd_step(a->page)) return 0; pump_host(a); }
  return 1;
}
static pid_t input_spawn(const char *const argv[], int *to_child, int *from_child) {
  int in[2] = {-1, -1}, out[2] = {-1, -1};
  // close-on-exec, or every later child (the worker, wtype) holds vpointer's stdin open and it never ends
  if (to_child && pipe2(in, O_CLOEXEC) != 0) return -1;
  if (from_child && pipe2(out, O_CLOEXEC) != 0) return -1;
  pid_t pid = fork();
  if (pid == 0) {
    if (to_child) { dup2(in[0], 0); close(in[0]); close(in[1]); }
    if (from_child) { dup2(out[1], 1); close(out[0]); close(out[1]); }
    execvp(argv[0], (char *const *)argv);
    _exit(127);
  }
  if (to_child) { close(in[0]); *to_child = in[1]; }
  if (from_child) { close(out[1]); *from_child = out[0]; }
  return pid;
}
// A command to its end, the window kept drawing meanwhile; 1 when it exited 0.
static int input_run(DesktopApp *a, const char *const argv[]) {
  pid_t pid = input_spawn(argv, NULL, NULL);
  if (pid < 0) return 0;
  int status = 0;
  for (int t = 0; t < 1000; t++) {
    pid_t got = waitpid(pid, &status, WNOHANG);
    if (got == pid) return WIFEXITED(status) && WEXITSTATUS(status) == 0;
    if (!input_frames(a, 10)) break;
  }
  kill(pid, SIGKILL); waitpid(pid, NULL, 0);
  return 0;
}
static int input_type(DesktopApp *a, const char *text) { const char *argv[] = {"wtype", text, NULL}; return input_run(a, argv) && input_frames(a, 150); }
static int input_key(DesktopApp *a, const char *mod, const char *key) {
  const char *with[] = {"wtype", "-M", mod, "-k", key, "-m", mod, NULL}, *plain[] = {"wtype", "-k", key, NULL};
  return input_run(a, mod ? with : plain) && input_frames(a, 150);
}
// The held keyboard: on (1) a virtual keyboard joins the seat and stays; off (0) it goes.
static int input_hold_keyboard(DesktopApp *a, int on) {
  if (on && input_keyboard < 0) { const char *argv[] = {"wtype", "-s", "600000", NULL}; input_keyboard = input_spawn(argv, NULL, NULL); }
  else if (!on && input_keyboard > 0) { kill(input_keyboard, SIGTERM); waitpid(input_keyboard, NULL, 0); input_keyboard = -1; }
  return input_frames(a, 400);
}
static int input_pointer(DesktopApp *a, const char *cmd) {
  char reply[64];
  if (!input_pointer_in || fprintf(input_pointer_in, "%s\n", cmd) < 0 || fflush(input_pointer_in) != 0) return 0;
  if (!fgets(reply, sizeof reply, input_pointer_out) || strncmp(reply, "ok", 2) != 0) return 0;
  return input_frames(a, 120);
}
// A click in the middle of the row with this id, where the page draws it.
static int input_press(DesktopApp *a, const char *id) {
  float b[4]; char cmd[96];
  if (!cd_box(a->page, id, b)) return 0;
  snprintf(cmd, sizeof cmd, "move %d %d", (int)(b[0] + b[2] / 2), (int)(b[1] + b[3] / 2));
  return input_pointer(a, cmd) && input_pointer(a, "click");
}
static int input_draft_is(DesktopApp *a, const char *want, char *got, int cap) {
  for (int t = 0; t < 60; t++) {
    int n = cd_draft(a->page, got, cap); got[n < 0 ? 0 : n] = '\0';
    if (!strcmp(got, want)) return 1;
    if (!input_frames(a, 50)) return 0;
  }
  return 0;
}
static int input_until_idle(DesktopApp *a, int ms) {
  int saw = 0;
  for (int t = 0; t < ms / 20; t++) {
    if (!input_frames(a, 20)) return 0;
    if (cd_running(a->page)) saw = 1;
    else if (saw || t > 25) return input_frames(a, 100);
  }
  return 0;
}
static int input_step(DesktopApp *a, const char *shot_out) {
  char got[4096], why[4200], first[128];
  if (!input_hold_keyboard(a, 1)) return input_fail("keyboard", "the window closed");
  // 1. Typed keys reach the composer, which has the keyboard when the window opens.
  const char *typed = "input test first message";
  if (!input_type(a, typed)) return input_fail("type", "wtype failed");
  if (!input_draft_is(a, typed, got, (int)sizeof got)) { snprintf(why, sizeof why, "the composer holds \"%s\"", got); return input_fail("type", why); }
  input_ok("type", "typed keys reach the composer");
  // 2. Return sends it; the worker's reply shows and is kept.
  if (!input_key(a, NULL, "Return") || !input_until_idle(a, 15000)) return input_fail("send", "the run did not end");
  int n = cd_save(a->page, a->save_buf, OA_SAVE_CAP);
  if (n <= 0 || !strstr(a->save_buf, typed) || !strstr(a->save_buf, "Reply from the worker")) return input_fail("send", "the page does not show the message and its reply");
  snprintf(first, sizeof first, "%s", a->chat_id);
  input_ok("send", "Return sent it and the reply shows");
  // 3. A click on New chat empties the page.
  if (!input_pointer(a, "move 5 5")) return input_fail("pointer", "vpointer did not answer");
  if (!input_press(a, "coder.new")) return input_fail("new-chat", "no New chat row to press");
  n = cd_title(a->page, got, (int)sizeof got);
  if (a->chat_id[0] && !strcmp(a->chat_id, first)) return input_fail("new-chat", "the click on New chat left the first chat open");
  input_ok("new-chat", "a click on New chat opened a new chat");
  // 4. The seat loses its keyboard and gets one again: typing still reaches the composer.
  if (!input_hold_keyboard(a, 0) || !input_hold_keyboard(a, 1)) return input_fail("replug", "the window closed");
  if (!input_type(a, "after replug")) return input_fail("replug", "wtype failed");
  if (!input_draft_is(a, "after replug", got, (int)sizeof got)) { snprintf(why, sizeof why, "after the keyboard came back the composer holds \"%s\"", got); return input_fail("replug", why); }
  input_ok("replug", "a keyboard that left and came back still types");
  // 5. Control-V pastes the clipboard into the composer.
  cd_set_text(a->page, "");
  const char *copy[] = {"wl-copy", "--paste-once", "pasted from the clipboard", NULL};   // serves this paste, then ends
  if (!input_run(a, copy)) return input_fail("paste", "wl-copy failed");
  if (!input_key(a, "ctrl", "v")) return input_fail("paste", "wtype failed");
  if (!input_draft_is(a, "pasted from the clipboard", got, (int)sizeof got)) { snprintf(why, sizeof why, "after Control-V the composer holds \"%s\"", got); return input_fail("paste", why); }
  input_ok("paste", "Control-V pastes the clipboard");
  cd_set_text(a->page, "");
  // 6. A click on the first chat's row in the sidebar opens it again with its reply.
  char row[160]; snprintf(row, sizeof row, "chat.%s", first);
  if (!input_press(a, row)) { snprintf(why, sizeof why, "no sidebar row %s to press", row); return input_fail("open", why); }
  input_frames(a, 200);
  n = cd_save(a->page, a->save_buf, OA_SAVE_CAP);
  if (strcmp(a->chat_id, first) != 0 || n <= 0 || !strstr(a->save_buf, "Reply from the worker")) return input_fail("open", "the click on the chat's row did not open it with its reply");
  input_ok("open", "a click on a chat's row opens it");
  if (shot_out && shot_out[0]) {
    const char *grim[] = {"grim", shot_out, NULL};
    if (!input_run(a, grim)) return input_fail("shot", shot_out);
    input_ok("shot", shot_out);
  }
  return 0;
}
static int run_input(DesktopApp *a, const char *shot_out) {
  if (!getenv("WAYLAND_DISPLAY")) return input_fail("display", "no WAYLAND_DISPLAY (input-test.sh starts a headless compositor)");
  if (!oa_core_init(vc(a), cs(""), cs("")) || oa_core_store_count(vc(a)) != 0) return input_fail("store", "the test store did not open empty");
  a->page = cd_open_ex(a->resources, 0, 1200, 820);
  if (!a->page) return input_fail("window", "cd_open_ex failed");
  set_folder(a, a->folder);
  refresh_settings_info(a);
  refresh_chats(a);
  if (!input_frames(a, 800)) return input_fail("window", "the window closed");
  char vp[OA_PATH]; snprintf(vp, sizeof vp, "%s/vpointer", a->exe_dir);
  const char *vargv[] = {vp, "1400", "900", NULL};
  int to = -1, from = -1;
  input_pointer_pid = input_spawn(vargv, &to, &from);
  if (input_pointer_pid > 0) { input_pointer_in = fdopen(to, "w"); input_pointer_out = fdopen(from, "r"); }
  int status = input_step(a, shot_out);
  if (input_pointer_in) fclose(input_pointer_in);
  if (input_pointer_out) fclose(input_pointer_out);
  if (input_pointer_pid > 0) waitpid(input_pointer_pid, NULL, 0);
  input_hold_keyboard(a, 0);
  stop_all_terms(a);
  stop_all_subagents(a);
  if (oa_core_worker_alive(vc(a))) oa_core_worker_stop(vc(a), 0);
  cd_close(a->page);
  a->page = NULL;
  if (status == 0) printf("openagents-desktop-%s input: ok\n", OA_PLATFORM);
  return status;
}
#endif

// The app: argv as UTF-8. Its exit status.
static int oa_main(int argc, char **argv) {
  DesktopApp app;
  memset(&app, 0, sizeof(app));
  app.core = calloc(1, sizeof(coax_t_OaCore));
  app.line_buf = malloc(OA_LINE_CAP);
  app.save_buf = malloc(OA_SAVE_CAP);
  if (!app.core || !app.line_buf || !app.save_buf) {
    oa_fail("OpenAgents ran out of memory.");
    return 1;
  }

  oa_exe_dir(argv[0], app.exe_dir, sizeof(app.exe_dir));
  snprintf(app.resources, sizeof(app.resources), "%s/resources", app.exe_dir);
  if (!file_exists(app.resources)) snprintf(app.resources, sizeof(app.resources), "%s", app.exe_dir);
  snprintf(app.worker_bin, sizeof(app.worker_bin), "%s/%s", app.exe_dir, OA_WORKER);
  if (!file_exists(app.worker_bin)) snprintf(app.worker_bin, sizeof(app.worker_bin), "%s", OA_WORKER);
  snprintf(app.claude_worker_bin, sizeof(app.claude_worker_bin), "%s/%s", app.exe_dir, OA_CLAUDE_WORKER);
  if (!file_exists(app.claude_worker_bin)) app.claude_worker_bin[0] = '\0';
  snprintf(app.agy_worker_bin, sizeof(app.agy_worker_bin), "%s/%s", app.exe_dir, OA_AGY_WORKER);
  if (!file_exists(app.agy_worker_bin)) app.agy_worker_bin[0] = '\0';
  snprintf(app.daemon_bin, sizeof(app.daemon_bin), "%s/%s", app.exe_dir, OA_DAEMON);
  if (!file_exists(app.daemon_bin)) {
    snprintf(app.daemon_bin, sizeof(app.daemon_bin), "%s/%s", app.exe_dir, OA_DAEMON_ALT);
    if (!file_exists(app.daemon_bin)) app.daemon_bin[0] = '\0';
  }

  oa_working_dir(app.folder, sizeof(app.folder));

  int smoke = 0, flow = 0, input = 0;
  const char *shot = NULL;
  const char *update_file = NULL;
  for (int i = 1; i < argc; i++) {
    if (strcmp(argv[i], "--smoke") == 0) smoke = 1;
    else if (strcmp(argv[i], "--flow-test") == 0) flow = 1;
    else if (strcmp(argv[i], "--input-test") == 0) input = 1;
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

#ifndef _WIN32
  if (input) {
    int rc = run_input(&app, shot);
    free(app.core); free(app.line_buf); free(app.save_buf);
    return rc;
  }
#else
  (void)input;
#endif
  if (flow) {
    int rc = run_flow(&app, shot);
    free(app.core); free(app.line_buf); free(app.save_buf);
    return rc;
  }

  // An index it can't read leaves the app running with an empty list: every write refuses
  // (host-core.coax oa_store_fresh) rather than saving a list of one chat over the user's chats.
  int store_ok = (int)oa_core_init(vc(&app), cs(""), cs(""));
  if (!store_ok) fprintf(stderr, "openagents-desktop: the chat list could not be read; chats are not saved until it can be\n");
  if (app.daemon_bin[0]) oa_core_daemon_ensure(vc(&app), cs(app.daemon_bin), 1500);

  app.page = cd_open_ex(app.resources, shot != NULL, 1200, 820);
  if (!app.page) {
    fprintf(stderr, "openagents-desktop: could not open UI host (resources=%s)\n", app.resources);
    oa_fail("OpenAgents could not open its window.");
    return 1;
  }
  app.portal = shot ? NULL : portal_open();
  oa_started();
  cd_claude(app.page, app.claude_worker_bin[0] ? 1 : 0);
  {
    // each Claude model's thinking level, as Settings > Models kept it (four digits, 0 low to 4 max)
    char levels[8] = {0};
    if (oa_core_key_read(cs("claude-effort"), vu(levels, (int)sizeof levels)) >= 4) {
      for (int k = 0; k < 4; k++) if (levels[k] >= '0' && levels[k] <= '4') cd_claude_effort_set(app.page, k, levels[k] - '0');
    }
  }
  {
    // the model new chats start on, as Settings > Models kept it ("claude:N" or "neon:N")
    char pick[16] = {0};
    if (oa_core_key_read(cs("orchestrator"), vu(pick, (int)sizeof pick)) > 0) {
      int on = strncmp(pick, "claude:", 7) == 0;
      const char *num = strchr(pick, ':');
      cd_orchestrator_set(app.page, on, num && num[1] >= '0' && num[1] <= '3' ? num[1] - '0' : 1);
    }
  }
  cd_agy(app.page, app.agy_worker_bin[0] ? 1 : 0);
  set_folder(&app, app.folder);
  refresh_settings_info(&app);
  refresh_chats(&app);
  if (!store_ok) cd_toast(app.page, "Your chat list could not be read. New chats are not saved until it can be.", 1);

  int status = 0;
  if (shot) {
    for (int i = 0; i < 4; i++) cd_shot_frame(app.page, 1200, 820, 1.0f / 60.0f);
    if (!cd_shot_write(app.page, shot)) {
      fprintf(stderr, "openagents-desktop: could not write %s\n", shot);
      status = 1;
    }
  } else {
    app.interactive = 1;
    while (cd_step(app.page)) {
      cd_env(app.page, cd_focused(app.page), 0);
      pump_host(&app);
    }
    oa_keep_awake(0);
  }

  stop_all_terms(&app);
  stop_all_subagents(&app);
  if (oa_core_worker_alive(vc(&app))) oa_core_worker_stop(vc(&app), 0);
  if (app.portal) { portal_cancel(app.portal); portal_close(app.portal); }
  cd_close(app.page);
  free(app.core); free(app.line_buf); free(app.save_buf);
  return status;
}
