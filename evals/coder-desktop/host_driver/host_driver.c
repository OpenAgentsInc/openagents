// Tier 1 deterministic headless host driver.
// Links the real apps/coder-desktop/host.c + page.coax (via carrier.c + gallery.o)
// against ui_host_stub.c and drives mock_worker.py processes over POSIX pipes.
#define _POSIX_C_SOURCE 200809L
#include "host.h"
#include "ui_host_stub.h"
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#define MAX_SLOTS 8
#define MAX_AGENTS 12
#define LINE_BUF_CAP 262144
#define SAVE_BUF_CAP 2097152
#define OUT_BUF_CAP 65536

typedef struct {
  pid_t pid;
  int to_child;   // write fd
  int from_child; // read fd
  char *buf;
  size_t len;
  size_t cap;
  int exited;
  int exit_code;
} WorkerProc;

typedef struct {
  void *page;
  int headless;
  char session[64];
  char cwd[512];
  WorkerProc main_w;
  WorkerProc agents[MAX_AGENTS];
  char agent_scripts[MAX_AGENTS][512];
  char agent_records[MAX_AGENTS][512];
  char default_agent_script[512];
  char default_agent_record[512];
  int actions_seen[64];
  int n_actions;
  int max_vertices_seen;
  double max_frame_ms;
  int frames_rendered;
  int lines_from_main;
  int lines_from_agents;
  int deny_next_ask;
} Slot;

static Slot g_slots[MAX_SLOTS];
static char g_mock_worker_path[1024] = "evals/coder-desktop/mock_worker/mock_worker.py";

static double now_ms(void) {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (double)ts.tv_sec * 1000.0 + (double)ts.tv_nsec / 1e6;
}

static void json_escape(FILE *out, const char *s) {
  fputc('"', out);
  if (s) {
    for (const unsigned char *p = (const unsigned char *)s; *p; p++) {
      switch (*p) {
        case '"': fputs("\\\"", out); break;
        case '\\': fputs("\\\\", out); break;
        case '\b': fputs("\\b", out); break;
        case '\f': fputs("\\f", out); break;
        case '\n': fputs("\\n", out); break;
        case '\r': fputs("\\r", out); break;
        case '\t': fputs("\\t", out); break;
        default:
          if (*p < 0x20) fprintf(out, "\\u%04x", *p);
          else fputc(*p, out);
      }
    }
  }
  fputc('"', out);
}

static void wp_init(WorkerProc *w) {
  memset(w, 0, sizeof *w);
  w->pid = -1;
  w->to_child = -1;
  w->from_child = -1;
}

static void wp_close(WorkerProc *w) {
  if (w->to_child >= 0) { close(w->to_child); w->to_child = -1; }
  if (w->from_child >= 0) { close(w->from_child); w->from_child = -1; }
  if (w->pid > 0) {
    int status = 0;
    pid_t r = waitpid(w->pid, &status, WNOHANG);
    if (r == 0) {
      kill(w->pid, SIGTERM);
      waitpid(w->pid, &status, 0);
    }
    w->pid = -1;
  }
  free(w->buf);
  w->buf = NULL;
  w->len = 0;
  w->cap = 0;
}

static int wp_spawn(WorkerProc *w, const char *script_path, const char *record_path) {
  wp_close(w);
  wp_init(w);
  int p_in[2], p_out[2];
  if (pipe(p_in) != 0) return -1;
  if (pipe(p_out) != 0) { close(p_in[0]); close(p_in[1]); return -1; }

  pid_t pid = fork();
  if (pid < 0) {
    close(p_in[0]); close(p_in[1]); close(p_out[0]); close(p_out[1]);
    return -1;
  }
  if (pid == 0) {
    dup2(p_in[0], STDIN_FILENO);
    dup2(p_out[1], STDOUT_FILENO);
    close(p_in[0]); close(p_in[1]); close(p_out[0]); close(p_out[1]);
    if (record_path && record_path[0]) {
      execlp("python3", "python3", g_mock_worker_path, "--script", script_path, "--record", record_path, (char *)NULL);
    } else {
      execlp("python3", "python3", g_mock_worker_path, "--script", script_path, (char *)NULL);
    }
    _exit(127);
  }
  close(p_in[0]);
  close(p_out[1]);
  int flags = fcntl(p_out[0], F_GETFL, 0);
  if (flags >= 0) fcntl(p_out[0], F_SETFL, flags | O_NONBLOCK);
  w->pid = pid;
  w->to_child = p_in[1];
  w->from_child = p_out[0];
  w->cap = LINE_BUF_CAP;
  w->buf = malloc(w->cap);
  w->len = 0;
  w->exited = 0;
  w->exit_code = 0;
  return 0;
}

static int wp_write_all(WorkerProc *w, const char *data, int n) {
  if (w->to_child < 0 || n <= 0) return -1;
  int off = 0;
  while (off < n) {
    ssize_t wr = write(w->to_child, data + off, (size_t)(n - off));
    if (wr < 0) {
      if (errno == EINTR) continue;
      return -1;
    }
    off += (int)wr;
  }
  return 0;
}

static void slot_record_action(Slot *s, int act) {
  if (act != 0 && s->n_actions < 64) {
    s->actions_seen[s->n_actions++] = act;
  }
}

static void slot_render(Slot *s) {
  if (!s->page) return;
  if (s->headless) {
    cd_tick(s->page, 1.0f / 60.0f);
    s->frames_rendered++;
    return;
  }
  cd_frame(s->page);
  s->frames_rendered++;
  UiHost *h = (UiHost *)cd_view(s->page);
  int v = ui_stub_last_vertices(h);
  double ms = ui_stub_last_frame_ms(h);
  if (v > s->max_vertices_seen) s->max_vertices_seen = v;
  if (ms > s->max_frame_ms) s->max_frame_ms = ms;
}

// Flush all host->worker actions, answers, and sub-agent spawns for one slot.
// Returns 1 if any work was dispatched.
static int slot_dispatch(Slot *s) {
  if (!s->page) return 0;
  static char out[OUT_BUF_CAP];
  int did = 0;

  // 1. Poll page action
  int act = cd_action(s->page);
  while (act != 0) {
    slot_record_action(s, act);
    did = 1;
    if (act == 1) { // CP_ASK_SEND
      int n = cd_task(s->page, s->session, s->cwd, out, (int)sizeof(out) - 1);
      if (n > 0) {
        wp_write_all(&s->main_w, out, n);
      }
    } else if (act == 3) { // CP_ASK_STOP
      int n = cd_stop_line(out, (int)sizeof(out) - 1);
      if (n > 0) {
        wp_write_all(&s->main_w, out, n);
      }
      for (int j = 0; j < MAX_AGENTS; j++) {
        if (s->agents[j].to_child >= 0 && n > 0) {
          wp_write_all(&s->agents[j], out, n);
        }
      }
    } else if (act == 4) { // CP_ASK_NEW -> forget
      int n = cd_forget_line(out, (int)sizeof(out) - 1);
      if (n > 0 && s->main_w.to_child >= 0) {
        wp_write_all(&s->main_w, out, n);
      }
    }
    act = cd_action(s->page);
  }

  // 2. Main worker answer/results lines
  for (;;) {
    int n = cd_answer_line(s->page, out, (int)sizeof(out) - 1);
    if (n <= 0) break;
    out[n] = '\0';
    if (s->deny_next_ask && strstr(out, "\"answer\":true") != NULL) {
      // Rewrite allow answer to deny answer for deterministic MWW_ASK deny testing.
      char *pos = strstr(out, "\"answer\":true");
      char denied[512];
      size_t prefix = (size_t)(pos - out);
      int dn = snprintf(denied, sizeof(denied), "%.*s\"answer\":false%s",
                        (int)prefix, out, pos + strlen("\"answer\":true"));
      if (dn > 0 && dn < (int)sizeof(denied)) {
        wp_write_all(&s->main_w, denied, dn);
      }
      s->deny_next_ask = 0;
    } else {
      wp_write_all(&s->main_w, out, n);
    }
    did = 1;
  }

  // 3. Sub-agents to start
  for (;;) {
    int j = cd_agent_next(s->page);
    if (j < 0 || j >= MAX_AGENTS) break;
    const char *sc = s->agent_scripts[j][0] ? s->agent_scripts[j] : s->default_agent_script;
    const char *rc = s->agent_records[j][0] ? s->agent_records[j] : s->default_agent_record;
    if (sc[0]) {
      wp_spawn(&s->agents[j], sc, rc);
      char asess[80];
      snprintf(asess, sizeof(asess), "%s-a%d", s->session, j);
      int n = cd_agent_task(s->page, j, asess, s->cwd, out, (int)sizeof(out) - 1);
      if (n > 0) {
        wp_write_all(&s->agents[j], out, n);
      }
    } else {
      cd_agent_failed(s->page, j, "no mock sub-agent script configured");
    }
    did = 1;
  }

  // 4. Sub-agent answer lines (both allow and quiet-loop resume deny)
  for (int j = 0; j < MAX_AGENTS; j++) {
    if (s->agents[j].to_child < 0) continue;
    for (;;) {
      int n = cd_agent_answer_line(s->page, j, out, (int)sizeof(out) - 1);
      if (n <= 0) break;
      wp_write_all(&s->agents[j], out, n);
      did = 1;
    }
  }

  return did;
}

// Drain available bytes from a WorkerProc stdout fd and feed complete lines.
// Returns 1 if any line or EOF was processed.
static int wp_drain(WorkerProc *w, Slot *s, int agent_idx) {
  if (w->from_child < 0) return 0;
  int did = 0;
  char tmp[16384];
  for (;;) {
    ssize_t r = read(w->from_child, tmp, sizeof(tmp));
    if (r > 0) {
      did = 1;
      if (w->len + (size_t)r + 1 > w->cap) {
        size_t ncap = (w->cap ? w->cap * 2 : LINE_BUF_CAP);
        while (ncap < w->len + (size_t)r + 1) ncap *= 2;
        char *nb = realloc(w->buf, ncap);
        if (!nb) break;
        w->buf = nb;
        w->cap = ncap;
      }
      memcpy(w->buf + w->len, tmp, (size_t)r);
      w->len += (size_t)r;
      // Extract complete lines
      size_t start = 0;
      for (size_t i = 0; i < w->len; i++) {
        if (w->buf[i] == '\n') {
          w->buf[i] = '\0';
          if (i > start && w->buf[i - 1] == '\r') w->buf[i - 1] = '\0';
          if (agent_idx < 0) {
            cd_line(s->page, w->buf + start);
            s->lines_from_main++;
          } else {
            cd_agent_line(s->page, agent_idx, w->buf + start);
            s->lines_from_agents++;
          }
          start = i + 1;
        }
      }
      if (start > 0) {
        memmove(w->buf, w->buf + start, w->len - start);
        w->len -= start;
      }
    } else if (r == 0) {
      // EOF from worker
      did = 1;
      close(w->from_child);
      w->from_child = -1;
      if (w->to_child >= 0) { close(w->to_child); w->to_child = -1; }
      if (w->pid > 0) {
        int status = 0;
        waitpid(w->pid, &status, 0);
        w->pid = -1;
        w->exited = 1;
        if (WIFEXITED(status)) w->exit_code = WEXITSTATUS(status);
        else if (WIFSIGNALED(status)) w->exit_code = 128 + WTERMSIG(status);
      }
      if (agent_idx < 0) {
        if (cd_running(s->page)) {
          char why[128];
          snprintf(why, sizeof(why), "worker exited unexpectedly (code %d)", w->exit_code);
          cd_failed(s->page, why);
        }
      } else {
        if (cd_agent_running(s->page, agent_idx)) {
          char why[128];
          snprintf(why, sizeof(why), "sub-agent %d exited unexpectedly (code %d)", agent_idx, w->exit_code);
          cd_agent_failed(s->page, agent_idx, why);
        }
      }
      break;
    } else {
      if (errno == EINTR) continue;
      if (errno == EAGAIN || errno == EWOULDBLOCK) break;
      break;
    }
  }
  return did;
}

static int slot_is_idle(Slot *s) {
  if (!s->page) return 1;
  if (cd_running(s->page)) return 0;
  return 1;
}

static void pump_slots(int single_slot, int max_ms, int stop_when_idle) {
  double deadline = now_ms() + (double)max_ms;
  int first = single_slot >= 0 ? single_slot : 0;
  int last = single_slot >= 0 ? single_slot + 1 : MAX_SLOTS;

  for (;;) {
    int any_did = 0;
    for (int si = first; si < last; si++) {
      if (!g_slots[si].page) continue;
      if (slot_dispatch(&g_slots[si])) any_did = 1;
    }

    struct pollfd pfds[MAX_SLOTS * (1 + MAX_AGENTS)];
    int p_slot[MAX_SLOTS * (1 + MAX_AGENTS)];
    int p_agent[MAX_SLOTS * (1 + MAX_AGENTS)];
    int nfds = 0;

    for (int si = first; si < last; si++) {
      Slot *s = &g_slots[si];
      if (!s->page) continue;
      if (s->main_w.from_child >= 0) {
        pfds[nfds].fd = s->main_w.from_child;
        pfds[nfds].events = POLLIN | POLLHUP;
        p_slot[nfds] = si;
        p_agent[nfds] = -1;
        nfds++;
      }
      for (int j = 0; j < MAX_AGENTS; j++) {
        if (s->agents[j].from_child >= 0) {
          pfds[nfds].fd = s->agents[j].from_child;
          pfds[nfds].events = POLLIN | POLLHUP;
          p_slot[nfds] = si;
          p_agent[nfds] = j;
          nfds++;
        }
      }
    }

    if (nfds > 0) {
      int pr = poll(pfds, (nfds_t)nfds, any_did ? 0 : 10);
      if (pr > 0) {
        for (int k = 0; k < nfds; k++) {
          if (pfds[k].revents & (POLLIN | POLLHUP | POLLERR)) {
            Slot *s = &g_slots[p_slot[k]];
            WorkerProc *w = p_agent[k] < 0 ? &s->main_w : &s->agents[p_agent[k]];
            if (wp_drain(w, s, p_agent[k])) any_did = 1;
          }
        }
      }
    }

    // Dispatch again immediately after reading lines so answers/sub-agents go out without delay
    for (int si = first; si < last; si++) {
      if (!g_slots[si].page) continue;
      if (slot_dispatch(&g_slots[si])) any_did = 1;
      slot_render(&g_slots[si]);
    }

    if (stop_when_idle) {
      int all_idle = 1;
      for (int si = first; si < last; si++) {
        if (g_slots[si].page && !slot_is_idle(&g_slots[si])) {
          all_idle = 0;
          break;
        }
      }
      if (all_idle && !any_did) break;
    }

    if (now_ms() >= deadline) break;
    if (!any_did && nfds == 0) {
      struct timespec ts = {0, 5 * 1000 * 1000};
      nanosleep(&ts, NULL);
    }
  }
}

static void print_slot_state(int si) {
  Slot *s = &g_slots[si];
  if (!s->page) {
    printf("{\"ok\":false,\"error\":\"slot not open\"}\n");
    return;
  }
  char title[256] = {0};
  char goal_obj[2048] = {0};
  cd_title(s->page, title, (int)sizeof(title) - 1);
  cd_goal_objective(s->page, goal_obj, (int)sizeof(goal_obj) - 1);
  UiHost *h = s->headless ? NULL : (UiHost *)cd_view(s->page);
  int last_v = h ? ui_stub_last_vertices(h) : 0;
  int max_v = h ? ui_stub_max_vertices(h) : 0;
  double last_ms = h ? ui_stub_last_frame_ms(h) : 0.0;
  int agents_running = 0;
  for (int j = 0; j < MAX_AGENTS; j++) {
    if (cd_agent_running(s->page, j)) agents_running++;
  }

  printf("{\"ok\":true,\"slot\":%d,\"running\":%d,\"cost_usd_micros\":%lld,\"budget_usd_micros\":%lld,\"budget_exceeded\":%d,\"goal_status\":%d,\"goal_time\":%d,\"save_due\":%d,",
         si, cd_running(s->page), (long long)cd_cost_usd_micros(s->page), (long long)cd_budget_usd_micros(s->page),
         cd_budget_exceeded(s->page), cd_goal_status(s->page), cd_goal_time(s->page), cd_save_due(s->page));
  printf("\"last_vertices\":%d,\"max_vertices_cap\":%d,\"max_vertices_seen\":%d,\"last_frame_ms\":%.3f,\"max_frame_ms\":%.3f,\"frames\":%d,",
         last_v, max_v, s->max_vertices_seen, last_ms, s->max_frame_ms, s->frames_rendered);
  printf("\"lines_from_main\":%d,\"lines_from_agents\":%d,\"agents_running\":%d,\"worker_exited\":%d,\"worker_exit_code\":%d,",
         s->lines_from_main, s->lines_from_agents, agents_running, s->main_w.exited, s->main_w.exit_code);
  printf("\"title\":"); json_escape(stdout, title);
  printf(",\"goal_objective\":"); json_escape(stdout, goal_obj);
  printf(",\"actions\":[");
  for (int i = 0; i < s->n_actions; i++) {
    if (i > 0) fputc(',', stdout);
    printf("%d", s->actions_seen[i]);
  }
  printf("]}\n");
}

static void unescape_arg(char *dst, const char *src, size_t cap) {
  size_t o = 0;
  for (const char *p = src; *p && o + 1 < cap; p++) {
    if (*p == '\\' && p[1]) {
      p++;
      if (*p == 'n') dst[o++] = '\n';
      else if (*p == 't') dst[o++] = '\t';
      else if (*p == 'r') dst[o++] = '\r';
      else dst[o++] = *p;
    } else {
      dst[o++] = *p;
    }
  }
  dst[o] = '\0';
}

int main(int argc, char **argv) {
  signal(SIGPIPE, SIG_IGN);
  for (int i = 1; i < argc; i++) {
    if (strcmp(argv[i], "--mock-worker") == 0 && i + 1 < argc) {
      snprintf(g_mock_worker_path, sizeof(g_mock_worker_path), "%s", argv[++i]);
    }
  }
  for (int i = 0; i < MAX_SLOTS; i++) {
    wp_init(&g_slots[i].main_w);
    for (int j = 0; j < MAX_AGENTS; j++) wp_init(&g_slots[i].agents[j]);
  }

  char line[65536];
  while (fgets(line, sizeof(line), stdin)) {
    size_t n = strlen(line);
    while (n > 0 && (line[n - 1] == '\n' || line[n - 1] == '\r')) line[--n] = '\0';
    if (n == 0 || line[0] == '#') continue;

    char cmd[64] = {0};
    if (sscanf(line, "%63s", cmd) != 1) continue;

    if (strcmp(cmd, "QUIT") == 0) {
      printf("{\"ok\":true}\n");
      fflush(stdout);
      break;
    } else if (strcmp(cmd, "OPEN") == 0) {
      int si = 0, headless = 0;
      char res[512] = {0};
      if (sscanf(line, "%*s %d %d %511s", &si, &headless, res) < 3 || si < 0 || si >= MAX_SLOTS) {
        printf("{\"ok\":false,\"error\":\"bad OPEN args\"}\n");
      } else {
        Slot *s = &g_slots[si];
        if (s->page) cd_close(s->page);
        memset(s, 0, sizeof *s);
        wp_init(&s->main_w);
        for (int j = 0; j < MAX_AGENTS; j++) wp_init(&s->agents[j]);
        s->headless = headless;
        snprintf(s->session, sizeof(s->session), "eval-slot-%d", si);
        snprintf(s->cwd, sizeof(s->cwd), "/tmp/coder-eval");
        s->page = headless ? cd_headless() : cd_open(res);
        if (!s->page) {
          printf("{\"ok\":false,\"error\":\"cd_open/cd_headless returned NULL\"}\n");
        } else {
          cd_env(s->page, 1, 0.0f, 1);
          slot_render(s);
          print_slot_state(si);
        }
      }
    } else if (strcmp(cmd, "CLOSE") == 0) {
      int si = 0;
      sscanf(line, "%*s %d", &si);
      if (si >= 0 && si < MAX_SLOTS) {
        Slot *s = &g_slots[si];
        wp_close(&s->main_w);
        for (int j = 0; j < MAX_AGENTS; j++) wp_close(&s->agents[j]);
        if (s->page) { cd_close(s->page); s->page = NULL; }
      }
      printf("{\"ok\":true}\n");
    } else if (strcmp(cmd, "SPAWN_WORKER") == 0) {
      int si = 0;
      char sc[512] = {0}, rc[512] = {0};
      int got = sscanf(line, "%*s %d %511s %511s", &si, sc, rc);
      if (got < 2 || si < 0 || si >= MAX_SLOTS || !g_slots[si].page) {
        printf("{\"ok\":false,\"error\":\"bad SPAWN_WORKER\"}\n");
      } else {
        int r = wp_spawn(&g_slots[si].main_w, sc, got >= 3 ? rc : "");
        printf("{\"ok\":%s,\"pid\":%d}\n", r == 0 ? "true" : "false", (int)g_slots[si].main_w.pid);
      }
    } else if (strcmp(cmd, "SET_AGENT_SCRIPT") == 0) {
      int si = 0, aj = -1;
      char sc[512] = {0}, rc[512] = {0};
      int got = sscanf(line, "%*s %d %d %511s %511s", &si, &aj, sc, rc);
      if (got < 3 || si < 0 || si >= MAX_SLOTS) {
        printf("{\"ok\":false,\"error\":\"bad SET_AGENT_SCRIPT\"}\n");
      } else {
        Slot *s = &g_slots[si];
        if (aj < 0) {
          snprintf(s->default_agent_script, sizeof(s->default_agent_script), "%s", sc);
          if (got >= 4) snprintf(s->default_agent_record, sizeof(s->default_agent_record), "%s", rc);
        } else if (aj < MAX_AGENTS) {
          snprintf(s->agent_scripts[aj], sizeof(s->agent_scripts[aj]), "%s", sc);
          if (got >= 4) snprintf(s->agent_records[aj], sizeof(s->agent_records[aj]), "%s", rc);
        }
        printf("{\"ok\":true}\n");
      }
    } else if (strcmp(cmd, "SET_TEXT") == 0) {
      int si = 0;
      const char *p = line + strlen(cmd);
      while (*p == ' ') p++;
      si = atoi(p);
      while (*p && *p != ' ') p++;
      if (*p == ' ') p++;
      static char text[32768];
      unescape_arg(text, p, sizeof(text));
      int ok = (si >= 0 && si < MAX_SLOTS && g_slots[si].page) ? cd_set_text(g_slots[si].page, text) : 0;
      printf("{\"ok\":%s}\n", ok ? "true" : "false");
    } else if (strcmp(cmd, "SUBMIT") == 0) {
      int si = 0;
      sscanf(line, "%*s %d", &si);
      int ok = (si >= 0 && si < MAX_SLOTS && g_slots[si].page) ? cd_submit(g_slots[si].page) : 0;
      printf("{\"ok\":%s}\n", ok ? "true" : "false");
    } else if (strcmp(cmd, "NEW_CHAT") == 0) {
      int si = 0;
      sscanf(line, "%*s %d", &si);
      int ok = (si >= 0 && si < MAX_SLOTS && g_slots[si].page) ? cd_new(g_slots[si].page) : 0;
      printf("{\"ok\":%s}\n", ok ? "true" : "false");
    } else if (strcmp(cmd, "SEND_RECALL") == 0) {
      int si = 0;
      sscanf(line, "%*s %d", &si);
      static char out[OUT_BUF_CAP];
      int n = (si >= 0 && si < MAX_SLOTS && g_slots[si].page) ? cd_recall_line(g_slots[si].page, out, (int)sizeof(out) - 1) : 0;
      if (n > 0) {
        out[n] = '\0';
        wp_write_all(&g_slots[si].main_w, out, n);
        printf("{\"ok\":true,\"bytes\":%d,\"line\":", n);
        json_escape(stdout, out);
        printf("}\n");
      } else {
        printf("{\"ok\":false,\"bytes\":0}\n");
      }
    } else if (strcmp(cmd, "SEND_STOP") == 0) {
      int si = 0;
      sscanf(line, "%*s %d", &si);
      char out[256];
      int n = cd_stop_line(out, (int)sizeof(out) - 1);
      if (n > 0 && si >= 0 && si < MAX_SLOTS) {
        wp_write_all(&g_slots[si].main_w, out, n);
        printf("{\"ok\":true,\"bytes\":%d}\n", n);
      } else {
        printf("{\"ok\":false}\n");
      }
    } else if (strcmp(cmd, "CLICK_STOP") == 0) {
      int si = 0;
      sscanf(line, "%*s %d", &si);
      if (si < 0 || si >= MAX_SLOTS || !g_slots[si].page || g_slots[si].headless) {
        printf("{\"ok\":false,\"error\":\"invalid slot for CLICK_STOP\"}\n");
      } else {
        Slot *s = &g_slots[si];
        slot_render(s);
        float box[6] = {0};
        int got = cd_editor(s->page, box);
        UiHost *h = (UiHost *)cd_view(s->page);
        int act = 0;
        if (got) {
          // Composer send/stop button sits at the bottom-right of the composer below the text area.
          float sx = box[0] + box[2] - 4.0f;
          float sy = box[1] + box[3] + 22.0f;
          ui_stub_pointer(h, sx, sy, 0, 1, 0);
          ui_stub_pointer(h, sx, sy, 2, 1, 0);
          act = cd_action(s->page);
          if (act != 0) slot_record_action(s, act);
          if (act == 3) {
            char out[256];
            int sn = cd_stop_line(out, (int)sizeof(out) - 1);
            if (sn > 0) wp_write_all(&s->main_w, out, sn);
          }
        }
        printf("{\"ok\":%s,\"action\":%d,\"box\":[%.1f,%.1f,%.1f,%.1f]}\n",
               act == 3 ? "true" : "false", act, box[0], box[1], box[2], box[3]);
      }
    } else if (strcmp(cmd, "PUMP") == 0) {
      int si = 0, max_ms = 2000, stop_idle = 1;
      sscanf(line, "%*s %d %d %d", &si, &max_ms, &stop_idle);
      if (si >= 0 && si < MAX_SLOTS && g_slots[si].page) {
        pump_slots(si, max_ms, stop_idle);
        print_slot_state(si);
      } else {
        printf("{\"ok\":false,\"error\":\"bad PUMP slot\"}\n");
      }
    } else if (strcmp(cmd, "PUMP_ALL") == 0) {
      int max_ms = 3000;
      sscanf(line, "%*s %d", &max_ms);
      pump_slots(-1, max_ms, 1);
      printf("{\"ok\":true}\n");
    } else if (strcmp(cmd, "FRAME") == 0) {
      int si = 0, count = 1;
      sscanf(line, "%*s %d %d", &si, &count);
      if (si >= 0 && si < MAX_SLOTS && g_slots[si].page) {
        for (int k = 0; k < count; k++) slot_render(&g_slots[si]);
        print_slot_state(si);
      } else {
        printf("{\"ok\":false}\n");
      }
    } else if (strcmp(cmd, "TAKE") == 0) {
      int to = 0, from = 1;
      sscanf(line, "%*s %d %d", &to, &from);
      int ok = (to >= 0 && to < MAX_SLOTS && from >= 0 && from < MAX_SLOTS && g_slots[to].page && g_slots[from].page)
               ? cd_take(g_slots[to].page, g_slots[from].page) : 0;
      if (ok) {
        // Move worker process ownership from `from` to `to` just like App.swift.
        wp_close(&g_slots[to].main_w);
        g_slots[to].main_w = g_slots[from].main_w;
        wp_init(&g_slots[from].main_w);
        for (int j = 0; j < MAX_AGENTS; j++) {
          wp_close(&g_slots[to].agents[j]);
          g_slots[to].agents[j] = g_slots[from].agents[j];
          wp_init(&g_slots[from].agents[j]);
        }
        slot_render(&g_slots[to]);
      }
      printf("{\"ok\":%s}\n", ok ? "true" : "false");
    } else if (strcmp(cmd, "SAVE") == 0) {
      int si = 0;
      char path[512] = {0};
      int got = sscanf(line, "%*s %d %511s", &si, path);
      if (got < 2 || si < 0 || si >= MAX_SLOTS || !g_slots[si].page) {
        printf("{\"ok\":false}\n");
      } else {
        char *buf = malloc(SAVE_BUF_CAP);
        int due = cd_save_due(g_slots[si].page);
        int sn = buf ? cd_save(g_slots[si].page, buf, SAVE_BUF_CAP - 1) : 0;
        if (sn > 0) {
          FILE *fp = fopen(path, "wb");
          if (fp) { fwrite(buf, 1, (size_t)sn, fp); fclose(fp); }
        }
        free(buf);
        printf("{\"ok\":%s,\"save_due\":%d,\"bytes\":%d}\n", sn > 0 ? "true" : "false", due, sn);
      }
    } else if (strcmp(cmd, "LOAD") == 0) {
      int si = 0;
      char path[512] = {0};
      int got = sscanf(line, "%*s %d %511s", &si, path);
      if (got < 2 || si < 0 || si >= MAX_SLOTS || !g_slots[si].page) {
        printf("{\"ok\":false}\n");
      } else {
        FILE *fp = fopen(path, "rb");
        int ok = 0;
        if (fp) {
          fseek(fp, 0, SEEK_END);
          long sz = ftell(fp);
          fseek(fp, 0, SEEK_SET);
          if (sz > 0 && sz < SAVE_BUF_CAP) {
            char *buf = malloc((size_t)sz + 1);
            if (buf) {
              size_t rd = fread(buf, 1, (size_t)sz, fp);
              buf[rd] = '\0';
              ok = cd_load(g_slots[si].page, buf);
              free(buf);
            }
          }
          fclose(fp);
        }
        if (ok) slot_render(&g_slots[si]);
        printf("{\"ok\":%s}\n", ok ? "true" : "false");
      }
    } else if (strcmp(cmd, "SET_BUDGET") == 0) {
      int si = 0;
      long long micros = 0;
      sscanf(line, "%*s %d %lld", &si, &micros);
      int ok = (si >= 0 && si < MAX_SLOTS && g_slots[si].page) ? cd_set_budget_usd_micros(g_slots[si].page, (int64_t)micros) : 0;
      printf("{\"ok\":%s}\n", ok ? "true" : "false");
    } else if (strcmp(cmd, "BUDGET_APPROVE") == 0) {
      int si = 0;
      sscanf(line, "%*s %d", &si);
      int ok = (si >= 0 && si < MAX_SLOTS && g_slots[si].page) ? cd_budget_approve(g_slots[si].page) : 0;
      printf("{\"ok\":%s}\n", ok ? "true" : "false");
    } else if (strcmp(cmd, "GOAL_SET") == 0) {
      int si = 0;
      const char *p = line + strlen(cmd);
      while (*p == ' ') p++;
      si = atoi(p);
      while (*p && *p != ' ') p++;
      if (*p == ' ') p++;
      int ok = (si >= 0 && si < MAX_SLOTS && g_slots[si].page) ? cd_goal_set(g_slots[si].page, p) : 0;
      printf("{\"ok\":%s}\n", ok ? "true" : "false");
    } else if (strcmp(cmd, "GOAL_STATUS") == 0) {
      int si = 0, st = 0;
      sscanf(line, "%*s %d %d", &si, &st);
      int ok = (si >= 0 && si < MAX_SLOTS && g_slots[si].page) ? cd_goal_set_status(g_slots[si].page, st) : 0;
      printf("{\"ok\":%s}\n", ok ? "true" : "false");
    } else if (strcmp(cmd, "GOAL_CLEAR") == 0) {
      int si = 0;
      sscanf(line, "%*s %d", &si);
      int ok = (si >= 0 && si < MAX_SLOTS && g_slots[si].page) ? cd_goal_clear(g_slots[si].page) : 0;
      printf("{\"ok\":%s}\n", ok ? "true" : "false");
    } else if (strcmp(cmd, "DENY_NEXT_ASK") == 0) {
      int si = 0;
      sscanf(line, "%*s %d", &si);
      if (si >= 0 && si < MAX_SLOTS && g_slots[si].page) {
        g_slots[si].deny_next_ask = 1;
        printf("{\"ok\":true}\n");
      } else {
        printf("{\"ok\":false}\n");
      }
    } else if (strcmp(cmd, "RESIZE") == 0) {
      int si = 0;
      float w = 1280.0f, h = 800.0f, dpr = 2.0f;
      sscanf(line, "%*s %d %f %f %f", &si, &w, &h, &dpr);
      if (si >= 0 && si < MAX_SLOTS && g_slots[si].page && !g_slots[si].headless) {
        ui_stub_set_size((UiHost *)cd_view(g_slots[si].page), w, h, dpr);
        slot_render(&g_slots[si]);
        print_slot_state(si);
      } else {
        printf("{\"ok\":false}\n");
      }
    } else if (strcmp(cmd, "ZOOM") == 0) {
      int si = 0, by = 0;
      sscanf(line, "%*s %d %d", &si, &by);
      if (si >= 0 && si < MAX_SLOTS && g_slots[si].page) {
        int row = cd_zoom(g_slots[si].page, by);
        slot_render(&g_slots[si]);
        printf("{\"ok\":true,\"zoom_row\":%d}\n", row);
      } else {
        printf("{\"ok\":false}\n");
      }
    } else if (strcmp(cmd, "STATE") == 0) {
      int si = 0;
      sscanf(line, "%*s %d", &si);
      if (si >= 0 && si < MAX_SLOTS) print_slot_state(si);
      else printf("{\"ok\":false}\n");
    } else {
      printf("{\"ok\":false,\"error\":\"unknown command\"}\n");
    }
    fflush(stdout);
  }

  for (int i = 0; i < MAX_SLOTS; i++) {
    wp_close(&g_slots[i].main_w);
    for (int j = 0; j < MAX_AGENTS; j++) wp_close(&g_slots[i].agents[j]);
    if (g_slots[i].page) cd_close(g_slots[i].page);
  }
  return 0;
}
