// First-party GCP oa-boat: Boat-v1-compatible sandbox control plane (#14128).
// Policy, sizing, auth, validation, rate limits, TTL reaping, usage math, and
// JSON codecs are #authority Coax (boat-service-policy.coax / boat-service-wire.coax).
// This C file owns platform effects only: HTTP/1.1 sockets, GCE Compute Engine
// REST calls (libcurl), process groups, byte-offset NDJSON attach streams, and files.
//
// Env:
//   PORT / --listen HOST:PORT   Bind address (default 127.0.0.1:8088)
//   OA_BOAT_API_KEY             Required bearer token (>= 8 chars)
//   OA_BOAT_BACKEND             "local" (default) or "gce"
//   OA_BOAT_GCE_PROJECT         GCP project (gce backend)
//   OA_BOAT_GCE_ZONE            GCP zone (default us-central1-a)
//   OA_BOAT_GCE_BASE            Compute Engine REST base override (mock GCE testing)
//   OA_BOAT_GCE_TOKEN           Static OAuth token override (else GCE metadata server)
//   OA_BOAT_MAX_ACTIVE          Max concurrent active sandboxes (default 8)
#define _POSIX_C_SOURCE 200809L
#include "boat-service-wire.generated.h"
#include <arpa/inet.h>
#include <ctype.h>
#include <curl/curl.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <poll.h>
#include <pthread.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#define MAX_BOXES 32
#define MAX_PROCS 128
#define MAX_SNAPS 32
#define MAX_ENVS  16
#define BS_FILE_MAX (5 * 1024 * 1024)
#define MAX_REQ_BODY (6 * 1024 * 1024)

static coax_string cs(const char *s) { return (coax_string){s ? s : "", s ? (int64_t)strlen(s) : 0}; }
static coax_v_u8 vb(void *p, size_t n) { return (coax_v_u8){(uint8_t *)p, (int64_t)n}; }

typedef struct {
  bool used;
  int id;
  pid_t pid;
  char box_id[16];
  char cmd_id[32];
  char out_path[256];
  char err_path[256];
  bool running;
  int exit_code;
} proc_rec;

typedef struct {
  bool used;
  char id[16];
  char name[72];
  char size[16];
  char env_name[72];
  char ip[64];
  char idem_key[128];
  char root_dir[256];
  char created_iso[32];
  char archive_iso[32];
  int state; // BS_STATE_*
  int64_t ttl_s;
  int64_t started_s;
  int64_t accrued_s;
} box_rec;

typedef struct {
  bool used;
  char name[72];
  char source_id[16];
  char created_iso[32];
  int64_t size_bytes;
} snap_rec;

typedef struct {
  bool used;
  char id[32];
  char name[72];
  bool is_default;
} env_rec;

static pthread_mutex_t g_mu = PTHREAD_MUTEX_INITIALIZER;
static box_rec g_boxes[MAX_BOXES];
static proc_rec g_procs[MAX_PROCS];
static snap_rec g_snaps[MAX_SNAPS];
static env_rec g_envs[MAX_ENVS];
static int g_next_pid = 1000;
static int g_next_req = 1;
static int g_min_starts = 0, g_hr_starts = 0, g_day_starts = 0;
static int g_max_active = 8;
static char g_api_key[256] = "";
static char g_backend[16] = "local";
static char g_work_root[200] = "";
static char g_gce_project[128] = "openagentsgemini";
static char g_gce_zone[64] = "us-central1-a";
static char g_gce_image[256] = "projects/openagentsgemini/global/images/family/oa-sandbox";
static char g_gce_base[256] = "https://compute.googleapis.com/compute/v1";
static char g_gce_token[512] = "";
static char g_ssh_key_copy[256] = "";
static char g_ssh_pubkey[1024] = "";
static pthread_mutex_t g_tok_mu = PTHREAD_MUTEX_INITIALIZER;
static char g_meta_tok[2048] = "";
static int64_t g_meta_tok_exp = 0;

static bool gce_is_live(void) {
  return strcmp(g_backend, "gce") == 0 && strncmp(g_gce_base, "https://compute.googleapis.com", 30) == 0;
}

static int64_t now_sec(void) { return (int64_t)time(NULL); }

static void iso_of(int64_t sec, char out[32]) {
  time_t t = (time_t)sec;
  struct tm tm;
  gmtime_r(&t, &tm);
  strftime(out, 32, "%Y-%m-%dT%H:%M:%SZ", &tm);
}

static void make_box_id(char out[16]) {
  static const char alpha[] = "23456789abcdefghjkmnpqrstuvwxyz"; // 31 chars (Boat alphabet: no 0,1,i,l,o)
  unsigned int seed = (unsigned int)time(NULL) ^ (unsigned int)getpid() ^ (unsigned int)(g_next_req * 2654435761u);
  out[0] = 'b'; out[1] = 'x'; out[2] = '_';
  for (int i = 0; i < 8; i++) {
    seed = seed * 1664525u + 1013904223u;
    out[3 + i] = alpha[(seed >> 16) % 31];
  }
  out[11] = 0;
}

static void mkdir_p(const char *path) {
  char tmp[512];
  snprintf(tmp, sizeof tmp, "%s", path);
  for (char *p = tmp + 1; *p; p++) {
    if (*p == '/') { *p = 0; (void)mkdir(tmp, 0700); *p = '/'; }
  }
  (void)mkdir(tmp, 0700);
}

static void reap_expired_locked(char del_ids[][16], int *del_n, int del_cap) {
  int64_t now = now_sec();
  if (del_n) *del_n = 0;
  for (int i = 0; i < MAX_BOXES; i++) {
    if (!g_boxes[i].used) continue;
    if (bs_c_ttl_expired(g_boxes[i].state, g_boxes[i].started_s, g_boxes[i].ttl_s, now)) {
      if (g_boxes[i].started_s > 0 && now > g_boxes[i].started_s) {
        g_boxes[i].accrued_s += now - g_boxes[i].started_s;
      }
      g_boxes[i].started_s = 0;
      g_boxes[i].state = 6; // archived
      if (del_ids && del_n && *del_n < del_cap && strcmp(g_backend, "gce") == 0) {
        snprintf(del_ids[*del_n], 16, "%s", g_boxes[i].id);
        (*del_n)++;
      }
    }
  }
}

static int active_count_locked(void) {
  int n = 0;
  for (int i = 0; i < MAX_BOXES; i++) {
    if (g_boxes[i].used && bs_c_state_up(g_boxes[i].state)) n++;
  }
  return n;
}

static box_rec *find_box_locked(const char *id) {
  for (int i = 0; i < MAX_BOXES; i++) {
    if (g_boxes[i].used && strcmp(g_boxes[i].id, id) == 0) return &g_boxes[i];
  }
  return NULL;
}

typedef struct {
  char *buf;
  size_t len;
  size_t cap;
} curl_buf;

static size_t curl_collect_cb(void *ptr, size_t sz, size_t nm, void *ud) {
  size_t n = sz * nm;
  curl_buf *cb = (curl_buf *)ud;
  if (cb && cb->buf && cb->cap > 1) {
    size_t avail = cb->cap - 1 - cb->len;
    size_t take = n < avail ? n : avail;
    if (take > 0) { memcpy(cb->buf + cb->len, ptr, take); cb->len += take; cb->buf[cb->len] = 0; }
  }
  return n;
}

static bool extract_json_str(const char *json, const char *key, char *out, size_t cap) {
  char pat[128];
  snprintf(pat, sizeof pat, "\"%s\"", key);
  const char *p = strstr(json, pat);
  if (!p) return false;
  p += strlen(pat);
  while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n') p++;
  if (*p != ':') return false;
  p++;
  while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n') p++;
  if (*p != '"') return false;
  p++;
  size_t n = 0;
  while (*p && *p != '"' && n + 1 < cap) {
    if (*p == '\\' && p[1]) p++;
    out[n++] = *p++;
  }
  out[n] = 0;
  return *p == '"' && n > 0;
}

static int64_t extract_json_int(const char *json, const char *key, int64_t defv) {
  char pat[128];
  snprintf(pat, sizeof pat, "\"%s\"", key);
  const char *p = strstr(json, pat);
  if (!p) return defv;
  p += strlen(pat);
  while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n') p++;
  if (*p != ':') return defv;
  p++;
  while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n') p++;
  if (!isdigit((unsigned char)*p) && *p != '-') return defv;
  return strtoll(p, NULL, 10);
}

static bool gce_get_token(char *out, size_t cap) {
  if (g_gce_token[0]) { snprintf(out, cap, "%s", g_gce_token); return true; }
  pthread_mutex_lock(&g_tok_mu);
  int64_t now = now_sec();
  if (g_meta_tok[0] && now + 60 < g_meta_tok_exp) {
    snprintf(out, cap, "%s", g_meta_tok);
    pthread_mutex_unlock(&g_tok_mu);
    return true;
  }
  pthread_mutex_unlock(&g_tok_mu);

  CURL *c = curl_easy_init();
  if (!c) return false;
  char resp[4096] = "";
  curl_buf cb = {resp, 0, sizeof resp};
  struct curl_slist *hdrs = curl_slist_append(NULL, "Metadata-Flavor: Google");
  curl_easy_setopt(c, CURLOPT_URL, "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token");
  curl_easy_setopt(c, CURLOPT_HTTPHEADER, hdrs);
  curl_easy_setopt(c, CURLOPT_WRITEFUNCTION, curl_collect_cb);
  curl_easy_setopt(c, CURLOPT_WRITEDATA, &cb);
  curl_easy_setopt(c, CURLOPT_TIMEOUT, 5L);
  CURLcode rc = curl_easy_perform(c);
  long code = 0;
  if (rc == CURLE_OK) curl_easy_getinfo(c, CURLINFO_RESPONSE_CODE, &code);
  curl_slist_free_all(hdrs);
  curl_easy_cleanup(c);
  if (rc != CURLE_OK || code != 200) {
    fprintf(stderr, "oa-boat: metadata token fetch failed (rc=%d http=%ld)\n", (int)rc, code);
    return false;
  }
  char tok[2048] = "";
  if (!extract_json_str(resp, "access_token", tok, sizeof tok)) return false;
  int64_t exp_in = extract_json_int(resp, "expires_in", 600);
  if (exp_in < 120) exp_in = 120;
  pthread_mutex_lock(&g_tok_mu);
  snprintf(g_meta_tok, sizeof g_meta_tok, "%s", tok);
  g_meta_tok_exp = now_sec() + exp_in;
  snprintf(out, cap, "%s", g_meta_tok);
  pthread_mutex_unlock(&g_tok_mu);
  return true;
}

static bool gce_call_body(const char *method, const char *path, const char *body, long *out_code, char *resp_buf, size_t resp_cap) {
  CURL *c = curl_easy_init();
  if (!c) return false;
  char url[512];
  snprintf(url, sizeof url, "%s%s", g_gce_base, path);
  struct curl_slist *hdrs = curl_slist_append(NULL, "Content-Type: application/json");
  char tok[2048] = "", auth[2100] = "";
  if (g_gce_token[0] || gce_is_live()) {
    if (gce_get_token(tok, sizeof tok) && tok[0]) {
      snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
      hdrs = curl_slist_append(hdrs, auth);
    }
  }
  if (resp_buf && resp_cap > 0) resp_buf[0] = 0;
  curl_buf cb = {resp_buf, 0, resp_cap};
  curl_easy_setopt(c, CURLOPT_URL, url);
  curl_easy_setopt(c, CURLOPT_CUSTOMREQUEST, method);
  curl_easy_setopt(c, CURLOPT_HTTPHEADER, hdrs);
  curl_easy_setopt(c, CURLOPT_WRITEFUNCTION, curl_collect_cb);
  curl_easy_setopt(c, CURLOPT_WRITEDATA, &cb);
  curl_easy_setopt(c, CURLOPT_TIMEOUT, 20L);
  if (body && *body) curl_easy_setopt(c, CURLOPT_POSTFIELDS, body);
  CURLcode rc = curl_easy_perform(c);
  long code = 0;
  if (rc == CURLE_OK) curl_easy_getinfo(c, CURLINFO_RESPONSE_CODE, &code);
  if (out_code) *out_code = code;
  curl_slist_free_all(hdrs);
  curl_easy_cleanup(c);
  return rc == CURLE_OK && code >= 200 && code < 300;
}

static void gce_vm_name(const char *box_id, char out[32]) {
  // GCE instance names must satisfy RFC 1035: [a-z]([-a-z0-9]{0,61}[a-z0-9])? (no underscores).
  size_t n = 0;
  out[n++] = 'o'; out[n++] = 'a'; out[n++] = '-';
  for (size_t i = 0; box_id[i] && n + 1 < 32; i++) {
    char c = box_id[i];
    out[n++] = (c == '_') ? '-' : (char)tolower((unsigned char)c);
  }
  out[n] = 0;
}

static bool gce_create_vm(const box_rec *b) {
  const char *mt = strcmp(b->size, "small") == 0 ? "e2-standard-2"
                 : strcmp(b->size, "large") == 0 ? "c3d-highcpu-8" : "e2-standard-4";
  char vm[32], path[256], body[4096], meta[1536] = "", resp[1024] = "";
  gce_vm_name(b->id, vm);
  snprintf(path, sizeof path, "/projects/%s/zones/%s/instances", g_gce_project, g_gce_zone);
  if (g_ssh_pubkey[0]) {
    snprintf(meta, sizeof meta,
             ",\"metadata\":{\"items\":[{\"key\":\"enable-oslogin\",\"value\":\"FALSE\"},"
             "{\"key\":\"block-project-ssh-keys\",\"value\":\"TRUE\"},"
             "{\"key\":\"ssh-keys\",\"value\":\"user:%s user\"}]}",
             g_ssh_pubkey);
  }
  // Pre-baked oa-sandbox boot disk (autoDelete), no external IP (Cloud NAT egress),
  // no project-admin SA (empty serviceAccounts), labeled for reaper & billing.
  snprintf(body, sizeof body,
           "{\"name\":\"%s\",\"machineType\":\"zones/%s/machineTypes/%s\","
           "\"disks\":[{\"boot\":true,\"autoDelete\":true,\"initializeParams\":"
           "{\"sourceImage\":\"%s\",\"diskSizeGb\":\"30\"}}],"
           "\"networkInterfaces\":[{\"network\":\"global/networks/default\"}],"
           "\"serviceAccounts\":[],"
           "\"tags\":{\"items\":[\"oa-boat-sandbox\"]},"
           "\"labels\":{\"oa-boat-id\":\"%s\",\"oa-boat-size\":\"%s\",\"oa-boat-sec\":\"0\"}%s}",
           vm, g_gce_zone, mt, g_gce_image, b->id, b->size, meta);
  long code = 0;
  bool ok = gce_call_body("POST", path, body, &code, resp, sizeof resp);
  if (!ok) fprintf(stderr, "oa-boat: gce_create_vm(%s) failed HTTP %ld: %.300s\n", b->id, code, resp);
  return ok;
}

static void gce_instance_op(const char *id, const char *method, const char *suffix) {
  char vm[32], path[256], resp[512] = "";
  gce_vm_name(id, vm);
  snprintf(path, sizeof path, "/projects/%s/zones/%s/instances/%s%s", g_gce_project, g_gce_zone, vm, suffix);
  long code = 0;
  bool ok = gce_call_body(method, path, "", &code, resp, sizeof resp);
  if (!ok && code != 404) fprintf(stderr, "oa-boat: gce %s %s failed HTTP %ld: %.200s\n", method, path, code, resp);
}

static int sh_quote(const char *src, char *dst, size_t cap) {
  size_t n = 0;
  if (cap < 3) return -1;
  dst[n++] = '\'';
  for (const char *p = src; *p; p++) {
    if (*p == '\'') {
      if (n + 5 >= cap) return -1;
      dst[n++] = '\''; dst[n++] = '\\'; dst[n++] = '\''; dst[n++] = '\'';
    } else {
      if (n + 2 >= cap) return -1;
      dst[n++] = *p;
    }
  }
  dst[n++] = '\'';
  dst[n] = 0;
  return (int)n;
}

static int gce_ssh_exec_capture(const char *ip, const char *remote_cmd,
                                const char *stdin_buf, size_t stdin_len,
                                char *out_buf, size_t out_cap, size_t *out_len,
                                char *err_buf, size_t err_cap, size_t *err_len,
                                int64_t timeout_s, bool *timed_out) {
  if (out_len) *out_len = 0;
  if (err_len) *err_len = 0;
  if (timed_out) *timed_out = false;
  int inp[2] = {-1, -1}, outp[2] = {-1, -1}, errp[2] = {-1, -1};
  if (pipe(outp) != 0 || pipe(errp) != 0) return -1;
  if (stdin_buf != NULL && pipe(inp) != 0) {
    close(outp[0]); close(outp[1]); close(errp[0]); close(errp[1]);
    return -1;
  }
  char target[96];
  snprintf(target, sizeof target, "user@%s", ip);
  pid_t child = fork();
  if (child < 0) {
    if (inp[0] >= 0) { close(inp[0]); close(inp[1]); }
    close(outp[0]); close(outp[1]); close(errp[0]); close(errp[1]);
    return -1;
  }
  if (child == 0) {
    setpgid(0, 0);
    if (stdin_buf != NULL) {
      dup2(inp[0], 0);
      close(inp[0]); close(inp[1]);
    } else {
      int dn = open("/dev/null", O_RDONLY);
      if (dn >= 0) { dup2(dn, 0); close(dn); }
    }
    dup2(outp[1], 1); dup2(errp[1], 2);
    close(outp[0]); close(outp[1]); close(errp[0]); close(errp[1]);
    execlp("ssh", "ssh",
           "-i", g_ssh_key_copy,
           "-o", "BatchMode=yes",
           "-o", "StrictHostKeyChecking=no",
           "-o", "UserKnownHostsFile=/dev/null",
           "-o", "ConnectTimeout=10",
           "-o", "ServerAliveInterval=15",
           "-o", "LogLevel=ERROR",
           target, remote_cmd, (char *)NULL);
    _exit(127);
  }
  if (inp[0] >= 0) close(inp[0]);
  close(outp[1]); close(errp[1]);

  if (inp[1] >= 0) {
    int fl = fcntl(inp[1], F_GETFL, 0);
    if (fl >= 0) (void)fcntl(inp[1], F_SETFL, fl | O_NONBLOCK);
    if (stdin_len == 0) { close(inp[1]); inp[1] = -1; }
  }
  {
    int fl = fcntl(outp[0], F_GETFL, 0);
    if (fl >= 0) (void)fcntl(outp[0], F_SETFL, fl | O_NONBLOCK);
    fl = fcntl(errp[0], F_GETFL, 0);
    if (fl >= 0) (void)fcntl(errp[0], F_SETFL, fl | O_NONBLOCK);
  }

  size_t in_off = 0, so = 0, se = 0;
  int64_t deadline = timeout_s > 0 ? now_sec() + timeout_s : 0;
  char discard[1024];
  while (inp[1] >= 0 || outp[0] >= 0 || errp[0] >= 0) {
    if (deadline > 0 && now_sec() >= deadline) {
      if (timed_out) *timed_out = true;
      kill(-child, SIGKILL);
      break;
    }
    struct pollfd pfds[3];
    int nfds = 0, idx_in = -1, idx_out = -1, idx_err = -1;
    if (inp[1] >= 0) { idx_in = nfds; pfds[nfds++] = (struct pollfd){inp[1], POLLOUT, 0}; }
    if (outp[0] >= 0) { idx_out = nfds; pfds[nfds++] = (struct pollfd){outp[0], POLLIN, 0}; }
    if (errp[0] >= 0) { idx_err = nfds; pfds[nfds++] = (struct pollfd){errp[0], POLLIN, 0}; }
    int pr = poll(pfds, (nfds_t)nfds, 500);
    if (pr < 0) { if (errno == EINTR) continue; break; }
    if (idx_in >= 0 && (pfds[idx_in].revents & (POLLOUT | POLLERR | POLLHUP))) {
      ssize_t w = write(inp[1], stdin_buf + in_off, stdin_len - in_off);
      if (w > 0) {
        in_off += (size_t)w;
        if (in_off >= stdin_len) { close(inp[1]); inp[1] = -1; }
      } else if (w < 0 && errno != EAGAIN && errno != EINTR) {
        close(inp[1]); inp[1] = -1;
      }
    }
    if (idx_out >= 0 && (pfds[idx_out].revents & (POLLIN | POLLHUP | POLLERR))) {
      ssize_t r;
      if (out_buf && so + 1 < out_cap) {
        r = read(outp[0], out_buf + so, out_cap - 1 - so);
        if (r > 0) so += (size_t)r;
      } else {
        r = read(outp[0], discard, sizeof discard);
      }
      if (r == 0 || (r < 0 && errno != EAGAIN && errno != EINTR)) { close(outp[0]); outp[0] = -1; }
    }
    if (idx_err >= 0 && (pfds[idx_err].revents & (POLLIN | POLLHUP | POLLERR))) {
      ssize_t r;
      if (err_buf && se + 1 < err_cap) {
        r = read(errp[0], err_buf + se, err_cap - 1 - se);
        if (r > 0) se += (size_t)r;
      } else {
        r = read(errp[0], discard, sizeof discard);
      }
      if (r == 0 || (r < 0 && errno != EAGAIN && errno != EINTR)) { close(errp[0]); errp[0] = -1; }
    }
  }
  if (inp[1] >= 0) close(inp[1]);
  if (outp[0] >= 0) close(outp[0]);
  if (errp[0] >= 0) close(errp[0]);
  if (out_buf && out_cap > 0) out_buf[so] = 0;
  if (err_buf && err_cap > 0) err_buf[se] = 0;
  if (out_len) *out_len = so;
  if (err_len) *err_len = se;

  int st = 0;
  while (waitpid(child, &st, 0) < 0 && errno == EINTR) {}
  return WIFEXITED(st) ? WEXITSTATUS(st) : WIFSIGNALED(st) ? (128 + WTERMSIG(st)) : -1;
}

static bool gce_init_ssh(void) {
  const char *src = getenv("OA_BOAT_SSH_KEY_FILE");
  if (!src || !*src) src = "/var/run/oa-boat/ssh_key";
  FILE *fp = fopen(src, "rb");
  if (!fp) {
    fprintf(stderr, "oa-boat: cannot open GCE ssh key file %s: %s\n", src, strerror(errno));
    return false;
  }
  char kbuf[8192];
  size_t kn = fread(kbuf, 1, sizeof kbuf - 1, fp);
  fclose(fp);
  if (kn < 32) {
    fprintf(stderr, "oa-boat: GCE ssh key file %s too short (%zu bytes)\n", src, kn);
    return false;
  }
  snprintf(g_ssh_key_copy, sizeof g_ssh_key_copy, "%s/ssh_key", g_work_root);
  int fd = open(g_ssh_key_copy, O_CREAT | O_WRONLY | O_TRUNC, 0600);
  if (fd < 0) { fprintf(stderr, "oa-boat: cannot write %s: %s\n", g_ssh_key_copy, strerror(errno)); return false; }
  (void)fchmod(fd, 0600);
  size_t off = 0;
  while (off < kn) {
    ssize_t w = write(fd, kbuf + off, kn - off);
    if (w <= 0) { if (errno == EINTR) continue; break; }
    off += (size_t)w;
  }
  close(fd);

  int pfd[2];
  if (pipe(pfd) != 0) return false;
  pid_t child = fork();
  if (child == 0) {
    close(pfd[0]);
    dup2(pfd[1], 1);
    close(pfd[1]);
    execlp("ssh-keygen", "ssh-keygen", "-y", "-f", g_ssh_key_copy, (char *)NULL);
    _exit(127);
  }
  close(pfd[1]);
  size_t pn = 0;
  ssize_t r;
  while (pn + 1 < sizeof g_ssh_pubkey && (r = read(pfd[0], g_ssh_pubkey + pn, sizeof g_ssh_pubkey - 1 - pn)) > 0) pn += (size_t)r;
  close(pfd[0]);
  int st = 0;
  while (waitpid(child, &st, 0) < 0 && errno == EINTR) {}
  while (pn > 0 && (g_ssh_pubkey[pn - 1] == '\n' || g_ssh_pubkey[pn - 1] == '\r' || g_ssh_pubkey[pn - 1] == ' ')) pn--;
  g_ssh_pubkey[pn] = 0;
  // Keep strictly "<type> <base64>" (strip any trailing comment so JSON and GCE ssh-keys metadata stay canonical).
  {
    char *sp1 = strchr(g_ssh_pubkey, ' ');
    if (sp1) {
      char *sp2 = strchr(sp1 + 1, ' ');
      if (sp2) { *sp2 = 0; pn = (size_t)(sp2 - g_ssh_pubkey); }
    }
  }
  if (!WIFEXITED(st) || WEXITSTATUS(st) != 0 || pn < 20) {
    fprintf(stderr, "oa-boat: ssh-keygen -y failed (status=%d pub_len=%zu)\n", WIFEXITED(st) ? WEXITSTATUS(st) : -1, pn);
    return false;
  }
  fprintf(stderr, "oa-boat: loaded GCE ssh pubkey (%zu bytes)\n", pn);
  return true;
}

typedef struct {
  int slot;
  char id[16];
} gce_wait_arg;

static void *gce_wait_ready_thread(void *arg) {
  gce_wait_arg *wa = (gce_wait_arg *)arg;
  int slot = wa->slot;
  char id[16];
  snprintf(id, sizeof id, "%s", wa->id);
  free(wa);

  char vm[32], path[256];
  gce_vm_name(id, vm);
  snprintf(path, sizeof path, "/projects/%s/zones/%s/instances/%s", g_gce_project, g_gce_zone, vm);
  char ip[64] = "";
  for (int attempt = 0; attempt < 60; attempt++) {
    pthread_mutex_lock(&g_mu);
    bool alive = g_boxes[slot].used && strcmp(g_boxes[slot].id, id) == 0 && g_boxes[slot].state == 1;
    pthread_mutex_unlock(&g_mu);
    if (!alive) return NULL;

    char resp[16384] = "";
    long code = 0;
    if (gce_call_body("GET", path, "", &code, resp, sizeof resp)) {
      char st_str[32] = "";
      if (extract_json_str(resp, "status", st_str, sizeof st_str) && strcmp(st_str, "RUNNING") == 0) {
        if (extract_json_str(resp, "networkIP", ip, sizeof ip) && ip[0]) {
          pthread_mutex_lock(&g_mu);
          if (g_boxes[slot].used && strcmp(g_boxes[slot].id, id) == 0) snprintf(g_boxes[slot].ip, sizeof g_boxes[slot].ip, "%s", ip);
          pthread_mutex_unlock(&g_mu);
          fprintf(stderr, "oa-boat: sandbox %s GCE VM RUNNING at %s\n", id, ip);
          break;
        }
      }
    }
    struct timespec ts = {2, 0};
    nanosleep(&ts, NULL);
  }
  if (!ip[0]) {
    fprintf(stderr, "oa-boat: sandbox %s timed out waiting for GCE RUNNING/networkIP\n", id);
    pthread_mutex_lock(&g_mu);
    if (g_boxes[slot].used && strcmp(g_boxes[slot].id, id) == 0 && g_boxes[slot].state == 1) g_boxes[slot].state = 7;
    pthread_mutex_unlock(&g_mu);
    return NULL;
  }

  for (int attempt = 0; attempt < 60; attempt++) {
    pthread_mutex_lock(&g_mu);
    bool alive = g_boxes[slot].used && strcmp(g_boxes[slot].id, id) == 0 && g_boxes[slot].state == 1;
    pthread_mutex_unlock(&g_mu);
    if (!alive) return NULL;

    char ob[256] = "", eb[256] = "";
    size_t ol = 0, el = 0;
    bool to = false;
    int ec = gce_ssh_exec_capture(ip, "(sudo -n mkdir -p /workspace && sudo -n chown user:user /workspace) 2>/dev/null || true; echo OA_READY", NULL, 0, ob, sizeof ob, &ol, eb, sizeof eb, &el, 15, &to);
    if (ec == 0 && strstr(ob, "OA_READY") != NULL) {
      pthread_mutex_lock(&g_mu);
      if (g_boxes[slot].used && strcmp(g_boxes[slot].id, id) == 0 && g_boxes[slot].state == 1) {
        g_boxes[slot].state = 2; // ready
        g_boxes[slot].started_s = now_sec();
        iso_of(g_boxes[slot].started_s + g_boxes[slot].ttl_s, g_boxes[slot].archive_iso);
      }
      pthread_mutex_unlock(&g_mu);
      fprintf(stderr, "oa-boat: sandbox %s SSH ready at %s (attempt=%d)\n", id, ip, attempt);
      return NULL;
    }
    if (attempt == 0 || attempt == 3 || attempt == 10 || attempt == 20 || attempt == 40) {
      fprintf(stderr, "oa-boat: sandbox %s ssh wait attempt=%d ec=%d to=%d err=%.160s\n", id, attempt, ec, to ? 1 : 0, eb);
    }
    struct timespec ts = {2, 0};
    nanosleep(&ts, NULL);
  }
  fprintf(stderr, "oa-boat: sandbox %s timed out waiting for SSH at %s\n", id, ip);
  pthread_mutex_lock(&g_mu);
  if (g_boxes[slot].used && strcmp(g_boxes[slot].id, id) == 0 && g_boxes[slot].state == 1) g_boxes[slot].state = 7;
  pthread_mutex_unlock(&g_mu);
  return NULL;
}

static void send_all(int fd, const void *buf, size_t len) {
  const uint8_t *p = (const uint8_t *)buf;
  while (len > 0) {
    ssize_t w = write(fd, p, len);
    if (w <= 0) { if (errno == EINTR) continue; break; }
    p += w; len -= (size_t)w;
  }
}

static void send_json(int fd, int status, const char *extra_hdr, const void *body, size_t len) {
  const char *reason = status == 200 ? "OK" : status == 201 ? "Created" : status == 400 ? "Bad Request"
                     : status == 401 ? "Unauthorized" : status == 404 ? "Not Found" : status == 409 ? "Conflict"
                     : status == 413 ? "Payload Too Large" : status == 429 ? "Too Many Requests" : "Internal Server Error";
  char hdr[512];
  int hn = snprintf(hdr, sizeof hdr,
                    "HTTP/1.1 %d %s\r\nContent-Type: application/json\r\nCache-Control: no-store\r\n%sContent-Length: %zu\r\nConnection: close\r\n\r\n",
                    status, reason, extra_hdr ? extra_hdr : "", len);
  send_all(fd, hdr, (size_t)hn);
  if (len > 0) send_all(fd, body, len);
}

static void send_err(int fd, int status, const char *code, const char *msg, const char *req_id, const char *extra_hdr) {
  uint8_t buf[1024];
  int64_t n = bs_w_error(status, cs(code), cs(msg), cs(req_id), vb(buf, sizeof buf));
  if (n > 0) send_json(fd, status, extra_hdr, buf, (size_t)n);
  else send_json(fd, status, extra_hdr, "{\"ok\":false}", 12);
}

typedef struct {
  int slot;
  pid_t child;
} reaper_arg;

static void *proc_reaper_thread(void *arg) {
  reaper_arg *ra = (reaper_arg *)arg;
  int slot = ra->slot;
  pid_t child = ra->child;
  free(ra);
  int st = 0;
  while (waitpid(child, &st, 0) < 0 && errno == EINTR) {}
  int ec = WIFEXITED(st) ? WEXITSTATUS(st) : WIFSIGNALED(st) ? (128 + WTERMSIG(st)) : -1;
  pthread_mutex_lock(&g_mu);
  if (slot >= 0 && slot < MAX_PROCS && g_procs[slot].used && g_procs[slot].pid == child) {
    g_procs[slot].running = false;
    g_procs[slot].exit_code = ec;
  }
  pthread_mutex_unlock(&g_mu);
  return NULL;
}

static void url_decode(const char *src, char *dst, size_t cap) {
  size_t n = 0;
  for (size_t i = 0; src[i] && n + 1 < cap; i++) {
    if (src[i] == '%' && isxdigit((unsigned char)src[i + 1]) && isxdigit((unsigned char)src[i + 2])) {
      char h[3] = {src[i + 1], src[i + 2], 0};
      dst[n++] = (char)strtol(h, NULL, 16);
      i += 2;
    } else if (src[i] == '+') {
      dst[n++] = ' ';
    } else {
      dst[n++] = src[i];
    }
  }
  dst[n] = 0;
}

static bool query_param(const char *qs, const char *key, char *out, size_t cap) {
  if (!qs || !*qs) return false;
  size_t kl = strlen(key);
  const char *p = qs;
  while (*p) {
    const char *amp = strchr(p, '&');
    size_t len = amp ? (size_t)(amp - p) : strlen(p);
    if (len > kl && strncmp(p, key, kl) == 0 && p[kl] == '=') {
      char raw[1024];
      size_t vl = len - kl - 1;
      if (vl >= sizeof raw) vl = sizeof raw - 1;
      memcpy(raw, p + kl + 1, vl);
      raw[vl] = 0;
      url_decode(raw, out, cap);
      return true;
    }
    if (!amp) break;
    p = amp + 1;
  }
  return false;
}

static int64_t file_size_of(const char *path) {
  struct stat st;
  return stat(path, &st) == 0 ? (int64_t)st.st_size : 0;
}

static void stream_attach(int fd, int proc_id, int64_t out_off, int64_t err_off, const char *req_id) {
  char out_path[256] = "", err_path[256] = "";
  int slot = -1;
  pthread_mutex_lock(&g_mu);
  for (int i = 0; i < MAX_PROCS; i++) {
    if (g_procs[i].used && g_procs[i].id == proc_id) {
      slot = i;
      snprintf(out_path, sizeof out_path, "%s", g_procs[i].out_path);
      snprintf(err_path, sizeof err_path, "%s", g_procs[i].err_path);
      break;
    }
  }
  pthread_mutex_unlock(&g_mu);
  if (slot < 0) { send_err(fd, 404, "not_found", "process not found", req_id, NULL); return; }

  const char *hdr = "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n";
  send_all(fd, hdr, strlen(hdr));
  char line[256];
  int ln = snprintf(line, sizeof line, "{\"type\":\"attached\",\"processId\":%d,\"stdoutOffset\":%lld,\"stderrOffset\":%lld}\n",
                    proc_id, (long long)out_off, (long long)err_off);
  send_all(fd, line, (size_t)ln);

  uint8_t chunk[4096], frame[16384];
  for (int ticks = 0; ticks < 4800; ticks++) {
    bool sent_any = false;
    int ofd = open(out_path, O_RDONLY);
    if (ofd >= 0) {
      if (lseek(ofd, (off_t)out_off, SEEK_SET) == (off_t)out_off) {
        ssize_t r = read(ofd, chunk, sizeof chunk);
        if (r > 0) {
          out_off += r;
          int64_t fn = bs_w_frame_data(cs("stdout"), (coax_string){(const char *)chunk, r}, out_off, vb(frame, sizeof frame));
          if (fn > 0) send_all(fd, frame, (size_t)fn);
          sent_any = true;
        }
      }
      close(ofd);
    }
    int efd = open(err_path, O_RDONLY);
    if (efd >= 0) {
      if (lseek(efd, (off_t)err_off, SEEK_SET) == (off_t)err_off) {
        ssize_t r = read(efd, chunk, sizeof chunk);
        if (r > 0) {
          err_off += r;
          int64_t fn = bs_w_frame_data(cs("stderr"), (coax_string){(const char *)chunk, r}, err_off, vb(frame, sizeof frame));
          if (fn > 0) send_all(fd, frame, (size_t)fn);
          sent_any = true;
        }
      }
      close(efd);
    }
    bool running = false;
    int ec = 0;
    pthread_mutex_lock(&g_mu);
    if (g_procs[slot].used) { running = g_procs[slot].running; ec = g_procs[slot].exit_code; }
    pthread_mutex_unlock(&g_mu);
    if (!running && !sent_any) {
      // Drain once more to ensure zero trailing bytes missed after exit.
      int64_t osz = file_size_of(out_path), esz = file_size_of(err_path);
      if (osz > out_off || esz > err_off) continue;
      ln = snprintf(line, sizeof line, "{\"type\":\"exit\",\"status\":\"exited\",\"exitCode\":%d,\"stdoutOffset\":%lld,\"stderrOffset\":%lld}\n",
                    ec, (long long)out_off, (long long)err_off);
      send_all(fd, line, (size_t)ln);
      return;
    }
    if (!sent_any) {
      struct timespec ts = {0, 25 * 1000 * 1000};
      nanosleep(&ts, NULL);
    }
  }
  // Stream slice reached 120s while process is still running: emit explicit reconnect cursor.
  ln = snprintf(line, sizeof line, "{\"type\":\"reconnect\",\"stdoutOffset\":%lld,\"stderrOffset\":%lld}\n",
                (long long)out_off, (long long)err_off);
  send_all(fd, line, (size_t)ln);
}

static void handle_client(int fd) {
  size_t cap = 65536, len = 0;
  char *buf = malloc(cap);
  if (!buf) { close(fd); return; }
  char *hdr_end = NULL;
  while (len < 65535) {
    ssize_t r = read(fd, buf + len, cap - 1 - len);
    if (r <= 0) break;
    len += (size_t)r;
    buf[len] = 0;
    hdr_end = strstr(buf, "\r\n\r\n");
    if (hdr_end) break;
  }
  if (!hdr_end) { free(buf); close(fd); return; }

  size_t hdr_len = (size_t)(hdr_end - buf) + 4;
  size_t content_len = 0;
  char auth_hdr[300] = "", idem_hdr[128] = "", confirm_hdr[64] = "";
  {
    char *line = strstr(buf, "\r\n");
    while (line && line < hdr_end) {
      line += 2;
      char *next = strstr(line, "\r\n");
      if (!next || next > hdr_end) break;
      size_t ll = (size_t)(next - line);
      if (ll > 15 && strncasecmp(line, "Content-Length:", 15) == 0) {
        content_len = (size_t)strtoull(line + 15, NULL, 10);
      } else if (ll > 14 && strncasecmp(line, "Authorization:", 14) == 0) {
        const char *v = line + 14; while (v < next && *v == ' ') v++;
        if ((size_t)(next - v) > 7 && strncasecmp(v, "Bearer ", 7) == 0) v += 7;
        size_t vl = (size_t)(next - v); if (vl >= sizeof auth_hdr) vl = sizeof auth_hdr - 1;
        memcpy(auth_hdr, v, vl); auth_hdr[vl] = 0;
      } else if (ll > 16 && strncasecmp(line, "Idempotency-Key:", 16) == 0) {
        const char *v = line + 16; while (v < next && *v == ' ') v++;
        size_t vl = (size_t)(next - v); if (vl >= sizeof idem_hdr) vl = sizeof idem_hdr - 1;
        memcpy(idem_hdr, v, vl); idem_hdr[vl] = 0;
      } else if (ll > 23 && strncasecmp(line, "X-Ascii-Confirm-Delete:", 23) == 0) {
        const char *v = line + 23; while (v < next && *v == ' ') v++;
        size_t vl = (size_t)(next - v); if (vl >= sizeof confirm_hdr) vl = sizeof confirm_hdr - 1;
        memcpy(confirm_hdr, v, vl); confirm_hdr[vl] = 0;
      }
      line = next;
    }
  }
  if (content_len > MAX_REQ_BODY) {
    send_err(fd, 413, "file_too_large", "request body exceeds limit", "req_big", NULL);
    free(buf); close(fd); return;
  }
  if (hdr_len + content_len + 1 > cap) {
    char *nb = realloc(buf, hdr_len + content_len + 1);
    if (!nb) { free(buf); close(fd); return; }
    buf = nb; cap = hdr_len + content_len + 1;
  }
  while (len < hdr_len + content_len) {
    ssize_t r = read(fd, buf + len, hdr_len + content_len - len);
    if (r <= 0) break;
    len += (size_t)r;
  }
  buf[len] = 0;

  char method[16] = "", target[2048] = "";
  if (sscanf(buf, "%15s %2047s", method, target) != 2) { free(buf); close(fd); return; }
  char *body = buf + hdr_len;
  size_t body_len = len > hdr_len ? len - hdr_len : 0;

  char path[2048], qs[2048] = "";
  char *qmark = strchr(target, '?');
  if (qmark) {
    size_t pl = (size_t)(qmark - target);
    memcpy(path, target, pl); path[pl] = 0;
    snprintf(qs, sizeof qs, "%s", qmark + 1);
  } else {
    snprintf(path, sizeof path, "%s", target);
  }

  // Strip optional /api/v1 prefix so both /api/v1/* and /* work seamlessly.
  const char *rel = path;
  if (strncmp(rel, "/api/v1", 7) == 0 && (rel[7] == '/' || rel[7] == 0)) rel += 7;

  char req_id[32];
  char del_ids[MAX_BOXES][16];
  int del_n = 0;
  pthread_mutex_lock(&g_mu);
  snprintf(req_id, sizeof req_id, "req_%d", g_next_req++);
  reap_expired_locked(del_ids, &del_n, MAX_BOXES);
  pthread_mutex_unlock(&g_mu);
  for (int i = 0; i < del_n; i++) {
    fprintf(stderr, "oa-boat: TTL expired for %s — deleting GCE VM\n", del_ids[i]);
    gce_instance_op(del_ids[i], "DELETE", "");
  }

  if (strcmp(method, "GET") == 0 && (strcmp(path, "/health") == 0 || strcmp(rel, "/health") == 0)) {
    send_json(fd, 200, NULL, "{\"ok\":true}", 11);
    free(buf); close(fd); return;
  }

  if (!bs_c_auth_ok(cs(g_api_key), cs(auth_hdr))) {
    send_err(fd, 401, "unauthorized", "invalid api key", req_id, NULL);
    free(buf); close(fd); return;
  }

  // GET /limits
  if (strcmp(method, "GET") == 0 && strcmp(rel, "/limits") == 0) {
    uint8_t out[2048];
    pthread_mutex_lock(&g_mu);
    int64_t n = bs_w_limits(active_count_locked(), g_max_active, g_min_starts, g_hr_starts, g_day_starts, vb(out, sizeof out));
    pthread_mutex_unlock(&g_mu);
    send_json(fd, 200, NULL, out, (size_t)n);
    free(buf); close(fd); return;
  }

  // GET /environments
  if (strcmp(method, "GET") == 0 && strcmp(rel, "/environments") == 0) {
    char out[4096]; size_t n = (size_t)snprintf(out, sizeof out, "{\"ok\":true,\"environments\":[");
    pthread_mutex_lock(&g_mu);
    bool first = true;
    for (int i = 0; i < MAX_ENVS; i++) {
      if (!g_envs[i].used) continue;
      n += (size_t)snprintf(out + n, sizeof out - n, "%s{\"id\":\"%s\",\"name\":\"%s\",\"isDefault\":%s}",
                            first ? "" : ",", g_envs[i].id, g_envs[i].name, g_envs[i].is_default ? "true" : "false");
      first = false;
    }
    pthread_mutex_unlock(&g_mu);
    n += (size_t)snprintf(out + n, sizeof out - n, "]}");
    send_json(fd, 200, NULL, out, n);
    free(buf); close(fd); return;
  }

  // POST /environments
  if (strcmp(method, "POST") == 0 && strcmp(rel, "/environments") == 0) {
    char name[72] = "";
    int64_t nn = bs_r_str((coax_string){body, (int64_t)body_len}, cs("name"), vb(name, sizeof name - 1));
    if (nn < 1 || !bs_c_name_valid(cs(name))) { send_err(fd, 400, "invalid_request", "invalid environment name", req_id, NULL); free(buf); close(fd); return; }
    name[nn] = 0;
    pthread_mutex_lock(&g_mu);
    int slot = -1;
    for (int i = 0; i < MAX_ENVS; i++) if (!g_envs[i].used) { slot = i; break; }
    if (slot >= 0) {
      g_envs[slot].used = true;
      snprintf(g_envs[slot].id, sizeof g_envs[slot].id, "env_%d", slot + 1);
      snprintf(g_envs[slot].name, sizeof g_envs[slot].name, "%s", name);
      g_envs[slot].is_default = (slot == 0);
    }
    pthread_mutex_unlock(&g_mu);
    if (slot < 0) { send_err(fd, 429, "limit_exceeded", "too many environments", req_id, NULL); }
    else {
      char out[256];
      int n = snprintf(out, sizeof out, "{\"ok\":true,\"environment\":{\"id\":\"env_%d\",\"name\":\"%s\",\"isDefault\":%s}}", slot + 1, name, slot == 0 ? "true" : "false");
      send_json(fd, 201, NULL, out, (size_t)n);
    }
    free(buf); close(fd); return;
  }

  // PUT /environments/{id}/vars/{key}
  if (strcmp(method, "PUT") == 0 && strncmp(rel, "/environments/", 14) == 0 && strstr(rel, "/vars/") != NULL) {
    send_json(fd, 200, NULL, "{\"ok\":true}", 11);
    free(buf); close(fd); return;
  }

  // GET /saved-snapshots
  if (strcmp(method, "GET") == 0 && strcmp(rel, "/saved-snapshots") == 0) {
    char out[8192]; size_t n = (size_t)snprintf(out, sizeof out, "{\"ok\":true,\"snapshots\":[");
    pthread_mutex_lock(&g_mu);
    bool first = true;
    for (int i = 0; i < MAX_SNAPS; i++) {
      if (!g_snaps[i].used) continue;
      n += (size_t)snprintf(out + n, sizeof out - n, "%s{\"name\":\"%s\",\"status\":\"ready\",\"sourceSandboxId\":\"%s\",\"sizeBytes\":%lld,\"createdAt\":\"%s\"}",
                            first ? "" : ",", g_snaps[i].name, g_snaps[i].source_id, (long long)g_snaps[i].size_bytes, g_snaps[i].created_iso);
      first = false;
    }
    pthread_mutex_unlock(&g_mu);
    n += (size_t)snprintf(out + n, sizeof out - n, "]}");
    send_json(fd, 200, NULL, out, n);
    free(buf); close(fd); return;
  }

  // POST /saved-snapshots
  if (strcmp(method, "POST") == 0 && strcmp(rel, "/saved-snapshots") == 0) {
    char sid[32] = "", sname[72] = "";
    int64_t sn = bs_r_str((coax_string){body, (int64_t)body_len}, cs("sandboxId"), vb(sid, sizeof sid - 1));
    int64_t nn = bs_r_str((coax_string){body, (int64_t)body_len}, cs("name"), vb(sname, sizeof sname - 1));
    if (sn > 0) sid[sn] = 0;
    if (nn > 0) sname[nn] = 0;
    if (!bs_c_id_valid(cs(sid)) || !bs_c_name_valid(cs(sname))) {
      send_err(fd, 400, "invalid_request", "invalid snapshot parameters", req_id, NULL); free(buf); close(fd); return;
    }
    pthread_mutex_lock(&g_mu);
    int slot = -1;
    for (int i = 0; i < MAX_SNAPS; i++) if (!g_snaps[i].used) { slot = i; break; }
    if (slot >= 0) {
      g_snaps[slot].used = true;
      snprintf(g_snaps[slot].name, sizeof g_snaps[slot].name, "%s", sname);
      snprintf(g_snaps[slot].source_id, sizeof g_snaps[slot].source_id, "%s", sid);
      g_snaps[slot].size_bytes = 104857600;
      iso_of(now_sec(), g_snaps[slot].created_iso);
    }
    pthread_mutex_unlock(&g_mu);
    char out[512];
    int n = snprintf(out, sizeof out, "{\"ok\":true,\"snapshot\":{\"name\":\"%s\",\"status\":\"ready\",\"sourceSandboxId\":\"%s\",\"sizeBytes\":104857600,\"createdAt\":\"%s\"}}",
                     sname, sid, slot >= 0 ? g_snaps[slot].created_iso : "");
    send_json(fd, 201, NULL, out, (size_t)n);
    free(buf); close(fd); return;
  }

  // DELETE /saved-snapshots/{name}
  if (strcmp(method, "DELETE") == 0 && strncmp(rel, "/saved-snapshots/", 17) == 0) {
    const char *sname = rel + 17;
    bool found = false;
    pthread_mutex_lock(&g_mu);
    for (int i = 0; i < MAX_SNAPS; i++) {
      if (g_snaps[i].used && strcmp(g_snaps[i].name, sname) == 0) { g_snaps[i].used = false; found = true; break; }
    }
    pthread_mutex_unlock(&g_mu);
    if (!found) send_err(fd, 404, "not_found", "snapshot not found", req_id, NULL);
    else send_json(fd, 200, NULL, "{\"ok\":true}", 11);
    free(buf); close(fd); return;
  }

  // GET /sandboxes
  if (strcmp(method, "GET") == 0 && strcmp(rel, "/sandboxes") == 0) {
    char f_state[32] = "", f_pref[72] = "";
    query_param(qs, "state", f_state, sizeof f_state);
    query_param(qs, "namePrefix", f_pref, sizeof f_pref);
    size_t out_cap = 65536;
    char *out = malloc(out_cap);
    size_t n = (size_t)snprintf(out, out_cap, "{\"ok\":true,\"sandboxes\":[");
    pthread_mutex_lock(&g_mu);
    bool first = true;
    for (int i = 0; i < MAX_BOXES; i++) {
      if (!g_boxes[i].used) continue;
      if (f_pref[0] && strncmp(g_boxes[i].name, f_pref, strlen(f_pref)) != 0) continue;
      uint8_t item[1024];
      int64_t in = bs_w_sandbox_env(cs(g_boxes[i].id), cs(g_boxes[i].name), g_boxes[i].state, cs(g_boxes[i].size),
                                    cs(g_boxes[i].ip), cs(g_boxes[i].env_name), g_boxes[i].ttl_s,
                                    cs(g_boxes[i].archive_iso), cs(g_boxes[i].created_iso), vb(item, sizeof item));
      if (in <= 0) continue;
      // Extract nested "sandbox":OBJ from envelope
      char *sp = strstr((char *)item, "\"sandbox\":");
      if (!sp) continue;
      sp += 10;
      size_t sl = (size_t)((char *)item + in - sp);
      if (sl > 0 && sp[sl - 1] == '}') sl--;
      if (n + sl + 64 < out_cap) {
        if (!first) out[n++] = ',';
        memcpy(out + n, sp, sl); n += sl;
        first = false;
      }
    }
    pthread_mutex_unlock(&g_mu);
    n += (size_t)snprintf(out + n, out_cap - n, "],\"pageInfo\":{\"hasMore\":false}}");
    send_json(fd, 200, NULL, out, n);
    free(out); free(buf); close(fd); return;
  }

  // POST /sandboxes
  if (strcmp(method, "POST") == 0 && strcmp(rel, "/sandboxes") == 0) {
    char size[16] = "default", name[72] = "", env_name[72] = "";
    int64_t sn = bs_r_str((coax_string){body, (int64_t)body_len}, cs("type"), vb(size, sizeof size - 1));
    if (sn > 0) size[sn] = 0;
    int64_t nn = bs_r_str((coax_string){body, (int64_t)body_len}, cs("name"), vb(name, sizeof name - 1));
    if (nn > 0) name[nn] = 0;
    int64_t en = bs_r_str((coax_string){body, (int64_t)body_len}, cs("environment"), vb(env_name, sizeof env_name - 1));
    if (en > 0) env_name[en] = 0;
    int64_t ttl_s = bs_r_int((coax_string){body, (int64_t)body_len}, cs("ttlSeconds"), 3600);

    pthread_mutex_lock(&g_mu);
    if (idem_hdr[0]) {
      for (int i = 0; i < MAX_BOXES; i++) {
        if (g_boxes[i].used && strcmp(g_boxes[i].idem_key, idem_hdr) == 0) {
          uint8_t out[1024];
          int64_t on = bs_w_sandbox_env(cs(g_boxes[i].id), cs(g_boxes[i].name), g_boxes[i].state, cs(g_boxes[i].size),
                                        cs(g_boxes[i].ip), cs(g_boxes[i].env_name), g_boxes[i].ttl_s,
                                        cs(g_boxes[i].archive_iso), cs(g_boxes[i].created_iso), vb(out, sizeof out));
          pthread_mutex_unlock(&g_mu);
          send_json(fd, 200, NULL, out, (size_t)on);
          free(buf); close(fd); return;
        }
      }
    }
    int adm = (int)bs_c_admit_create(cs(size), ttl_s, active_count_locked(), g_max_active, g_min_starts, g_hr_starts, g_day_starts);
    if (adm != 200) {
      pthread_mutex_unlock(&g_mu);
      send_err(fd, adm, adm == 429 ? "rate_limited" : "invalid_request", adm == 429 ? "sandbox start limit reached" : "invalid create request", req_id, adm == 429 ? "Retry-After: 5\r\n" : NULL);
      free(buf); close(fd); return;
    }
    if (name[0] && !bs_c_name_valid(cs(name))) {
      pthread_mutex_unlock(&g_mu);
      send_err(fd, 400, "invalid_request", "invalid sandbox name", req_id, NULL);
      free(buf); close(fd); return;
    }
    int slot = -1;
    for (int i = 0; i < MAX_BOXES; i++) if (!g_boxes[i].used) { slot = i; break; }
    if (slot < 0) {
      pthread_mutex_unlock(&g_mu);
      send_err(fd, 429, "limit_exceeded", "sandbox table full", req_id, NULL);
      free(buf); close(fd); return;
    }
    box_rec b = {0};
    b.used = true;
    make_box_id(b.id);
    snprintf(b.name, sizeof b.name, "%s", name[0] ? name : b.id);
    snprintf(b.size, sizeof b.size, "%s", size);
    snprintf(b.env_name, sizeof b.env_name, "%s", env_name);
    snprintf(b.idem_key, sizeof b.idem_key, "%s", idem_hdr);
    snprintf(b.ip, sizeof b.ip, "%s", gce_is_live() ? "" : strcmp(g_backend, "gce") == 0 ? "10.128.0.10" : "127.0.0.1");
    snprintf(b.root_dir, sizeof b.root_dir, "%s/%s", g_work_root, b.id);
    b.state = gce_is_live() ? 1 : 2; // provisioning on live GCE until RUNNING + SSH ready
    b.ttl_s = ttl_s > 0 ? ttl_s : 3600;
    b.started_s = now_sec();
    iso_of(b.started_s, b.created_iso);
    iso_of(b.started_s + b.ttl_s, b.archive_iso);
    g_boxes[slot] = b;
    g_min_starts++; g_hr_starts++; g_day_starts++;
    pthread_mutex_unlock(&g_mu);

    char fs_dir[300], pr_dir[300];
    snprintf(fs_dir, sizeof fs_dir, "%s/fs/tmp", b.root_dir);
    snprintf(pr_dir, sizeof pr_dir, "%s/proc", b.root_dir);
    mkdir_p(fs_dir);
    mkdir_p(pr_dir);

    if (strcmp(g_backend, "gce") == 0) {
      if (!gce_create_vm(&b)) {
        pthread_mutex_lock(&g_mu);
        g_boxes[slot].used = false;
        pthread_mutex_unlock(&g_mu);
        send_err(fd, 502, "gce_create_failed", "GCE instance create failed", req_id, NULL);
        free(buf); close(fd); return;
      }
      if (gce_is_live()) {
        gce_wait_arg *wa = malloc(sizeof *wa);
        if (wa) {
          wa->slot = slot;
          snprintf(wa->id, sizeof wa->id, "%s", b.id);
          pthread_t th;
          if (pthread_create(&th, NULL, gce_wait_ready_thread, wa) == 0) pthread_detach(th);
          else free(wa);
        }
      }
    }

    uint8_t out[1024];
    int64_t on = bs_w_sandbox_env(cs(b.id), cs(b.name), b.state, cs(b.size), cs(b.ip), cs(b.env_name),
                                  b.ttl_s, cs(b.archive_iso), cs(b.created_iso), vb(out, sizeof out));
    send_json(fd, 201, NULL, out, (size_t)on);
    free(buf); close(fd); return;
  }

  // Routes under /sandboxes/{id}...
  if (strncmp(rel, "/sandboxes/", 11) == 0) {
    const char *p = rel + 11;
    const char *sl = strchr(p, '/');
    char bid[32] = "";
    size_t bl = sl ? (size_t)(sl - p) : strlen(p);
    if (bl >= sizeof bid) bl = sizeof bid - 1;
    memcpy(bid, p, bl); bid[bl] = 0;
    const char *sub = sl ? sl : "";

    if (!bs_c_id_valid(cs(bid))) {
      send_err(fd, 404, "not_found", "invalid sandbox id", req_id, NULL);
      free(buf); close(fd); return;
    }

    // GET /sandboxes/{id}
    if (strcmp(method, "GET") == 0 && sub[0] == 0) {
      pthread_mutex_lock(&g_mu);
      box_rec *b = find_box_locked(bid);
      if (!b) { pthread_mutex_unlock(&g_mu); send_err(fd, 404, "not_found", "sandbox not found", req_id, NULL); free(buf); close(fd); return; }
      uint8_t out[1024];
      int64_t on = bs_w_sandbox_env(cs(b->id), cs(b->name), b->state, cs(b->size), cs(b->ip), cs(b->env_name),
                                    b->ttl_s, cs(b->archive_iso), cs(b->created_iso), vb(out, sizeof out));
      pthread_mutex_unlock(&g_mu);
      send_json(fd, 200, NULL, out, (size_t)on);
      free(buf); close(fd); return;
    }

    // PATCH /sandboxes/{id}
    if (strcmp(method, "PATCH") == 0 && sub[0] == 0) {
      int64_t ttl_s = bs_r_int((coax_string){body, (int64_t)body_len}, cs("ttlSeconds"), -1);
      char nname[72] = "";
      int64_t nn = bs_r_str((coax_string){body, (int64_t)body_len}, cs("name"), vb(nname, sizeof nname - 1));
      if (nn > 0) nname[nn] = 0;
      pthread_mutex_lock(&g_mu);
      box_rec *b = find_box_locked(bid);
      if (!b) { pthread_mutex_unlock(&g_mu); send_err(fd, 404, "not_found", "sandbox not found", req_id, NULL); free(buf); close(fd); return; }
      if (nname[0] && bs_c_name_valid(cs(nname))) snprintf(b->name, sizeof b->name, "%s", nname);
      if (ttl_s > 0 && ttl_s <= 86400) { b->ttl_s = ttl_s; iso_of(now_sec() + ttl_s, b->archive_iso); }
      uint8_t out[1024];
      int64_t on = bs_w_sandbox_env(cs(b->id), cs(b->name), b->state, cs(b->size), cs(b->ip), cs(b->env_name),
                                    b->ttl_s, cs(b->archive_iso), cs(b->created_iso), vb(out, sizeof out));
      pthread_mutex_unlock(&g_mu);
      send_json(fd, 200, NULL, out, (size_t)on);
      free(buf); close(fd); return;
    }

    // DELETE /sandboxes/{id}
    if (strcmp(method, "DELETE") == 0 && sub[0] == 0) {
      if (!bs_c_confirm_delete_ok(cs(bid), cs(confirm_hdr))) {
        send_err(fd, 400, "delete_confirmation_required", "X-Ascii-Confirm-Delete must match sandbox id", req_id, NULL);
        free(buf); close(fd); return;
      }
      pthread_mutex_lock(&g_mu);
      box_rec *b = find_box_locked(bid);
      if (!b) { pthread_mutex_unlock(&g_mu); send_err(fd, 404, "not_found", "sandbox not found", req_id, NULL); free(buf); close(fd); return; }
      b->used = false;
      pthread_mutex_unlock(&g_mu);
      if (strcmp(g_backend, "gce") == 0) gce_instance_op(bid, "DELETE", "");
      send_json(fd, 200, NULL, "{\"ok\":true,\"status\":\"deleted\"}", 30);
      free(buf); close(fd); return;
    }

    // POST /sandboxes/{id}/stop
    if (strcmp(method, "POST") == 0 && strcmp(sub, "/stop") == 0) {
      pthread_mutex_lock(&g_mu);
      box_rec *b = find_box_locked(bid);
      if (!b) { pthread_mutex_unlock(&g_mu); send_err(fd, 404, "not_found", "sandbox not found", req_id, NULL); free(buf); close(fd); return; }
      int64_t now = now_sec();
      if (b->started_s > 0 && now > b->started_s) b->accrued_s += now - b->started_s;
      b->started_s = 0;
      b->state = 6; // archived
      pthread_mutex_unlock(&g_mu);
      if (strcmp(g_backend, "gce") == 0) gce_instance_op(bid, "POST", "/stop");
      send_json(fd, 200, NULL, "{\"ok\":true,\"status\":\"archived\"}", 31);
      free(buf); close(fd); return;
    }

    // POST /sandboxes/{id}/resume
    if (strcmp(method, "POST") == 0 && strcmp(sub, "/resume") == 0) {
      char nsize[16] = "";
      int64_t sn = bs_r_str((coax_string){body, (int64_t)body_len}, cs("type"), vb(nsize, sizeof nsize - 1));
      if (sn > 0) nsize[sn] = 0;
      int64_t ttl_s = bs_r_int((coax_string){body, (int64_t)body_len}, cs("ttlSeconds"), -1);
      pthread_mutex_lock(&g_mu);
      box_rec *b = find_box_locked(bid);
      if (!b) { pthread_mutex_unlock(&g_mu); send_err(fd, 404, "not_found", "sandbox not found", req_id, NULL); free(buf); close(fd); return; }
      int slot = (int)(b - g_boxes);
      if (nsize[0]) snprintf(b->size, sizeof b->size, "%s", nsize);
      if (ttl_s > 0 && ttl_s <= 86400) b->ttl_s = ttl_s;
      b->state = gce_is_live() ? 1 : 2; // provisioning on live GCE until RUNNING + SSH ready
      b->started_s = now_sec();
      iso_of(b->started_s + b->ttl_s, b->archive_iso);
      uint8_t out[1024];
      int64_t on = bs_w_sandbox_env(cs(b->id), cs(b->name), b->state, cs(b->size), cs(b->ip), cs(b->env_name),
                                    b->ttl_s, cs(b->archive_iso), cs(b->created_iso), vb(out, sizeof out));
      pthread_mutex_unlock(&g_mu);
      if (strcmp(g_backend, "gce") == 0) {
        gce_instance_op(bid, "POST", "/start");
        if (gce_is_live()) {
          gce_wait_arg *wa = malloc(sizeof *wa);
          if (wa) {
            wa->slot = slot;
            snprintf(wa->id, sizeof wa->id, "%s", bid);
            pthread_t th;
            if (pthread_create(&th, NULL, gce_wait_ready_thread, wa) == 0) pthread_detach(th);
            else free(wa);
          }
        }
      }
      send_json(fd, 200, NULL, out, (size_t)on);
      free(buf); close(fd); return;
    }

    // GET /sandboxes/{id}/usage
    if (strcmp(method, "GET") == 0 && strcmp(sub, "/usage") == 0) {
      pthread_mutex_lock(&g_mu);
      box_rec *b = find_box_locked(bid);
      if (!b) { pthread_mutex_unlock(&g_mu); send_err(fd, 404, "not_found", "sandbox not found", req_id, NULL); free(buf); close(fd); return; }
      bool up = bs_c_state_up(b->state);
      int64_t sec = b->accrued_s;
      if (up && b->started_s > 0) { int64_t d = now_sec() - b->started_s; if (d > 0) sec += d; }
      uint8_t out[512];
      int64_t on = bs_w_usage(cs(b->size), sec, up, vb(out, sizeof out));
      pthread_mutex_unlock(&g_mu);
      send_json(fd, 200, NULL, out, (size_t)on);
      free(buf); close(fd); return;
    }

    // POST /sandboxes/{id}/sshkey
    if (strcmp(method, "POST") == 0 && strcmp(sub, "/sshkey") == 0) {
      pthread_mutex_lock(&g_mu);
      box_rec *b = find_box_locked(bid);
      if (!b || !bs_c_state_up(b->state)) {
        pthread_mutex_unlock(&g_mu);
        send_err(fd, b ? 400 : 404, b ? "machine_not_running" : "not_found", "sandbox not running", req_id, NULL);
        free(buf); close(fd); return;
      }
      char out[512];
      int n = snprintf(out, sizeof out,
                       "{\"ok\":true,\"sshUser\":\"user\",\"machineIp\":\"%s\",\"sshEndpoint\":\"user@%s\",\"hostKey\":\"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOaBoatTestHostKey000000000000000000000000\"}",
                       b->ip, b->ip);
      pthread_mutex_unlock(&g_mu);
      send_json(fd, 200, NULL, out, (size_t)n);
      free(buf); close(fd); return;
    }

    // PUT /sandboxes/{id}/files & GET /sandboxes/{id}/files?path=...
    if (strcmp(sub, "/files") == 0) {
      pthread_mutex_lock(&g_mu);
      box_rec *b = find_box_locked(bid);
      int bst = b ? b->state : 0;
      char broot[256] = "", bip[64] = "";
      if (b) { snprintf(broot, sizeof broot, "%s", b->root_dir); snprintf(bip, sizeof bip, "%s", b->ip); }
      pthread_mutex_unlock(&g_mu);
      if (!b) { send_err(fd, 404, "not_found", "sandbox not found", req_id, NULL); free(buf); close(fd); return; }

      if (strcmp(method, "PUT") == 0) {
        char fpath[1025] = "";
        int64_t pn = bs_r_str((coax_string){body, (int64_t)body_len}, cs("path"), vb(fpath, sizeof fpath - 1));
        if (pn > 0) fpath[pn] = 0;
        char *content = malloc(BS_FILE_MAX + 1);
        int64_t cn = content ? bs_r_str((coax_string){body, (int64_t)body_len}, cs("content"), vb(content, BS_FILE_MAX)) : -1;
        int adm = (int)bs_c_admit_file(bst, cs(fpath), cn >= 0 ? cn : -1);
        if (adm != 200 || cn < 0) {
          free(content);
          send_err(fd, adm != 200 ? adm : 400, adm == 413 ? "file_too_large" : adm == 409 ? "machine_not_running" : "invalid_request", "file write refused", req_id, NULL);
          free(buf); close(fd); return;
        }
        if (gce_is_live()) {
          char qp[4200], rcmd[9000], ob[256], eb[512];
          size_t ol = 0, el = 0; bool to = false;
          if (!bip[0] || sh_quote(fpath, qp, sizeof qp) < 0) {
            free(content); send_err(fd, 500, "io_error", "invalid gce file target", req_id, NULL); free(buf); close(fd); return;
          }
          snprintf(rcmd, sizeof rcmd, "d=$(dirname -- %s) && mkdir -p -- \"$d\" && cat > %s", qp, qp);
          int ec = gce_ssh_exec_capture(bip, rcmd, content, (size_t)cn, ob, sizeof ob, &ol, eb, sizeof eb, &el, 60, &to);
          free(content);
          if (ec != 0) { send_err(fd, 500, "io_error", "cannot write file on GCE VM", req_id, NULL); free(buf); close(fd); return; }
          send_json(fd, 200, NULL, "{\"ok\":true}", 11);
          free(buf); close(fd); return;
        }
        char full[1400], dirp[1400];
        snprintf(full, sizeof full, "%s/fs%s", broot, fpath);
        snprintf(dirp, sizeof dirp, "%s", full);
        char *ls = strrchr(dirp, '/');
        if (ls) { *ls = 0; mkdir_p(dirp); }
        FILE *fp = fopen(full, "wb");
        if (!fp) { free(content); send_err(fd, 500, "io_error", "cannot write file", req_id, NULL); free(buf); close(fd); return; }
        if (cn > 0) (void)fwrite(content, 1, (size_t)cn, fp);
        fclose(fp);
        free(content);
        send_json(fd, 200, NULL, "{\"ok\":true}", 11);
        free(buf); close(fd); return;
      }
      if (strcmp(method, "GET") == 0) {
        char fpath[1025] = "";
        if (!query_param(qs, "path", fpath, sizeof fpath)) { send_err(fd, 400, "invalid_request", "missing path", req_id, NULL); free(buf); close(fd); return; }
        int adm = (int)bs_c_admit_file(bst, cs(fpath), 0);
        if (adm != 200) { send_err(fd, adm, adm == 409 ? "machine_not_running" : "invalid_request", "file read refused", req_id, NULL); free(buf); close(fd); return; }
        if (gce_is_live()) {
          char qp[4200], rcmd[9000], eb[512];
          if (!bip[0] || sh_quote(fpath, qp, sizeof qp) < 0) {
            send_err(fd, 500, "io_error", "invalid gce file path", req_id, NULL); free(buf); close(fd); return;
          }
          snprintf(rcmd, sizeof rcmd, "test -f %s && cat -- %s", qp, qp);
          char *fbuf = malloc(BS_FILE_MAX + 2);
          size_t rd = 0, el = 0; bool to = false;
          int ec = fbuf ? gce_ssh_exec_capture(bip, rcmd, NULL, 0, fbuf, BS_FILE_MAX + 2, &rd, eb, sizeof eb, &el, 60, &to) : -1;
          if (!fbuf || ec != 0) { free(fbuf); send_err(fd, 404, "not_found", "file not found", req_id, NULL); free(buf); close(fd); return; }
          if (rd > BS_FILE_MAX) { free(fbuf); send_err(fd, 413, "file_too_large", "file too large", req_id, NULL); free(buf); close(fd); return; }
          size_t ocap = rd * 2 + 2048;
          uint8_t *out = malloc(ocap);
          int64_t on = bs_w_file(cs(fpath), (coax_string){fbuf, (int64_t)rd}, false, (int64_t)rd, vb(out, ocap));
          send_json(fd, 200, NULL, out, on > 0 ? (size_t)on : 0);
          free(out); free(fbuf); free(buf); close(fd); return;
        }
        char full[1400];
        snprintf(full, sizeof full, "%s/fs%s", broot, fpath);
        FILE *fp = fopen(full, "rb");
        if (!fp) { send_err(fd, 404, "not_found", "file not found", req_id, NULL); free(buf); close(fd); return; }
        fseek(fp, 0, SEEK_END);
        long fsz = ftell(fp);
        fseek(fp, 0, SEEK_SET);
        if (fsz < 0 || fsz > BS_FILE_MAX) { fclose(fp); send_err(fd, 413, "file_too_large", "file too large", req_id, NULL); free(buf); close(fd); return; }
        char *fbuf = malloc((size_t)fsz + 1);
        size_t rd = fsz > 0 ? fread(fbuf, 1, (size_t)fsz, fp) : 0;
        fclose(fp);
        fbuf[rd] = 0;
        size_t ocap = rd * 2 + 2048;
        uint8_t *out = malloc(ocap);
        int64_t on = bs_w_file(cs(fpath), (coax_string){fbuf, (int64_t)rd}, false, (int64_t)rd, vb(out, ocap));
        send_json(fd, 200, NULL, out, on > 0 ? (size_t)on : 0);
        free(out); free(fbuf); free(buf); close(fd); return;
      }
    }

    // POST /sandboxes/{id}/commands
    if (strcmp(method, "POST") == 0 && strcmp(sub, "/commands") == 0) {
      pthread_mutex_lock(&g_mu);
      box_rec *b = find_box_locked(bid);
      int bst = b ? b->state : 0;
      char broot[256] = "", bip[64] = "";
      if (b) { snprintf(broot, sizeof broot, "%s", b->root_dir); snprintf(bip, sizeof bip, "%s", b->ip); }
      pthread_mutex_unlock(&g_mu);
      if (!b) { send_err(fd, 404, "not_found", "sandbox not found", req_id, NULL); free(buf); close(fd); return; }

      char *cmd = malloc(65537);
      char cwd[512] = "";
      int64_t cl = cmd ? bs_r_str((coax_string){body, (int64_t)body_len}, cs("command"), vb(cmd, 65536)) : -1;
      if (cl > 0) cmd[cl] = 0;
      int64_t cwdl = bs_r_str((coax_string){body, (int64_t)body_len}, cs("cwd"), vb(cwd, sizeof cwd - 1));
      if (cwdl > 0) cwd[cwdl] = 0;
      bool detached = bs_r_bool((coax_string){body, (int64_t)body_len}, cs("detached"));
      int64_t timeout_s = bs_r_int((coax_string){body, (int64_t)body_len}, cs("timeoutSeconds"), 0);

      int adm = (int)bs_c_admit_command(bst, cl, timeout_s, detached);
      if (adm != 200) {
        free(cmd);
        send_err(fd, adm, adm == 409 ? "machine_not_running" : "invalid_request", "command refused", req_id, NULL);
        free(buf); close(fd); return;
      }

      // Compute working directory inside sandbox root (local backend)
      char run_cwd[768];
      if (cwd[0] == '/') {
        snprintf(run_cwd, sizeof run_cwd, "%s/fs%s", broot, cwd);
        if (!gce_is_live()) mkdir_p(run_cwd);
      } else {
        snprintf(run_cwd, sizeof run_cwd, "%s/fs", broot);
      }

      if (detached) {
        pthread_mutex_lock(&g_mu);
        int slot = -1;
        for (int i = 0; i < MAX_PROCS; i++) if (!g_procs[i].used) { slot = i; break; }
        int pid_id = g_next_pid++;
        if (slot >= 0) {
          g_procs[slot] = (proc_rec){0};
          g_procs[slot].used = true;
          g_procs[slot].id = pid_id;
          g_procs[slot].running = true;
          snprintf(g_procs[slot].box_id, sizeof g_procs[slot].box_id, "%s", bid);
          snprintf(g_procs[slot].cmd_id, sizeof g_procs[slot].cmd_id, "cmd_%d", pid_id);
          snprintf(g_procs[slot].out_path, sizeof g_procs[slot].out_path, "%s/proc/%d.out", broot, pid_id);
          snprintf(g_procs[slot].err_path, sizeof g_procs[slot].err_path, "%s/proc/%d.err", broot, pid_id);
        }
        pthread_mutex_unlock(&g_mu);
        if (slot < 0) { free(cmd); send_err(fd, 429, "limit_exceeded", "too many detached processes", req_id, NULL); free(buf); close(fd); return; }

        char *rcmd = NULL;
        if (gce_is_live()) {
          size_t qcap = (size_t)cl * 4 + 16;
          char *qcmd = malloc(qcap);
          char qcwd[2200] = "";
          rcmd = malloc(qcap + 2560);
          if (!bip[0] || !qcmd || !rcmd || sh_quote(cmd, qcmd, qcap) < 0 || (cwd[0] == '/' && sh_quote(cwd, qcwd, sizeof qcwd) < 0)) {
            free(qcmd); free(rcmd); free(cmd);
            pthread_mutex_lock(&g_mu); g_procs[slot].used = false; pthread_mutex_unlock(&g_mu);
            send_err(fd, 500, "exec_error", "cannot prepare GCE detached command", req_id, NULL);
            free(buf); close(fd); return;
          }
          if (cwd[0] == '/') {
            snprintf(rcmd, qcap + 2560, "echo $$ > /tmp/oa-pid-%d; mkdir -p -- %s && cd -- %s && exec sh -c %s", pid_id, qcwd, qcwd, qcmd);
          } else {
            snprintf(rcmd, qcap + 2560, "echo $$ > /tmp/oa-pid-%d; exec sh -c %s", pid_id, qcmd);
          }
          free(qcmd);
        }

        int ofd = open(g_procs[slot].out_path, O_CREAT | O_WRONLY | O_TRUNC, 0600);
        int efd = open(g_procs[slot].err_path, O_CREAT | O_WRONLY | O_TRUNC, 0600);
        pid_t child = fork();
        if (child == 0) {
          setpgid(0, 0);
          int dn = open("/dev/null", O_RDONLY);
          if (dn >= 0) { dup2(dn, 0); close(dn); }
          if (ofd >= 0) { dup2(ofd, 1); close(ofd); }
          if (efd >= 0) { dup2(efd, 2); close(efd); }
          if (rcmd) {
            char target[96];
            snprintf(target, sizeof target, "user@%s", bip);
            execlp("ssh", "ssh",
                   "-i", g_ssh_key_copy,
                   "-o", "BatchMode=yes",
                   "-o", "StrictHostKeyChecking=no",
                   "-o", "UserKnownHostsFile=/dev/null",
                   "-o", "ConnectTimeout=10",
                   "-o", "ServerAliveInterval=15",
                   "-o", "LogLevel=ERROR",
                   target, rcmd, (char *)NULL);
          } else {
            (void)chdir(run_cwd);
            execl("/bin/sh", "sh", "-c", cmd, (char *)NULL);
          }
          _exit(127);
        }
        if (ofd >= 0) close(ofd);
        if (efd >= 0) close(efd);
        free(rcmd);
        free(cmd);
        pthread_mutex_lock(&g_mu);
        g_procs[slot].pid = child;
        pthread_mutex_unlock(&g_mu);
        reaper_arg *ra = malloc(sizeof *ra);
        ra->slot = slot; ra->child = child;
        pthread_t th;
        pthread_create(&th, NULL, proc_reaper_thread, ra);
        pthread_detach(th);

        uint8_t out[256];
        int64_t on = bs_w_detached(pid_id, cs(g_procs[slot].cmd_id), vb(out, sizeof out));
        send_json(fd, 200, NULL, out, (size_t)on);
        free(buf); close(fd); return;
      }

      // Synchronous command
      if (gce_is_live()) {
        size_t qcap = (size_t)cl * 4 + 16;
        char *qcmd = malloc(qcap);
        char qcwd[2200] = "";
        char *rcmd = malloc(qcap + 2560);
        if (!bip[0] || !qcmd || !rcmd || sh_quote(cmd, qcmd, qcap) < 0 || (cwd[0] == '/' && sh_quote(cwd, qcwd, sizeof qcwd) < 0)) {
          free(qcmd); free(rcmd); free(cmd);
          send_err(fd, 500, "exec_error", "cannot prepare GCE command", req_id, NULL);
          free(buf); close(fd); return;
        }
        if (cwd[0] == '/') snprintf(rcmd, qcap + 2560, "mkdir -p -- %s && cd -- %s && sh -c %s", qcwd, qcwd, qcmd);
        else snprintf(rcmd, qcap + 2560, "sh -c %s", qcmd);
        free(qcmd); free(cmd);
        char so[65536], se[16384];
        size_t son = 0, sen = 0; bool to = false;
        int ec = gce_ssh_exec_capture(bip, rcmd, NULL, 0, so, sizeof so, &son, se, sizeof se, &sen, timeout_s > 0 ? timeout_s : 120, &to);
        free(rcmd);
        uint8_t *out = malloc(163840);
        int64_t on = bs_w_command_res(cs(req_id), ec, to, (coax_string){so, (int64_t)son}, (coax_string){se, (int64_t)sen}, vb(out, 163840));
        send_json(fd, 200, NULL, out, on > 0 ? (size_t)on : 0);
        free(out); free(buf); close(fd); return;
      }
      int outp[2], errp[2];
      if (pipe(outp) != 0 || pipe(errp) != 0) { free(cmd); send_err(fd, 500, "pipe_failed", "pipe failed", req_id, NULL); free(buf); close(fd); return; }
      pid_t child = fork();
      if (child == 0) {
        setpgid(0, 0);
        (void)chdir(run_cwd);
        int dn = open("/dev/null", O_RDONLY);
        if (dn >= 0) { dup2(dn, 0); close(dn); }
        dup2(outp[1], 1); dup2(errp[1], 2);
        close(outp[0]); close(outp[1]); close(errp[0]); close(errp[1]);
        execl("/bin/sh", "sh", "-c", cmd, (char *)NULL);
        _exit(127);
      }
      close(outp[1]); close(errp[1]);
      free(cmd);
      char so[65536], se[16384];
      size_t son = 0, sen = 0;
      ssize_t r;
      while (son < sizeof so - 1 && (r = read(outp[0], so + son, sizeof so - 1 - son)) > 0) son += (size_t)r;
      while (sen < sizeof se - 1 && (r = read(errp[0], se + sen, sizeof se - 1 - sen)) > 0) sen += (size_t)r;
      close(outp[0]); close(errp[0]);
      int st = 0;
      while (waitpid(child, &st, 0) < 0 && errno == EINTR) {}
      int ec = WIFEXITED(st) ? WEXITSTATUS(st) : -1;
      uint8_t *out = malloc(163840);
      int64_t on = bs_w_command_res(cs(req_id), ec, false, (coax_string){so, (int64_t)son}, (coax_string){se, (int64_t)sen}, vb(out, 163840));
      send_json(fd, 200, NULL, out, on > 0 ? (size_t)on : 0);
      free(out); free(buf); close(fd); return;
    }

    // /sandboxes/{id}/commands/{pid}[/stream]
    if (strncmp(sub, "/commands/", 10) == 0) {
      const char *pp = sub + 10;
      char *endp = NULL;
      int proc_id = (int)strtol(pp, &endp, 10);
      const char *tail = endp ? endp : "";
      if (strcmp(method, "GET") == 0 && strcmp(tail, "/stream") == 0) {
        char o_s[32] = "0", e_s[32] = "0";
        query_param(qs, "stdoutOffset", o_s, sizeof o_s);
        query_param(qs, "stderrOffset", e_s, sizeof e_s);
        stream_attach(fd, proc_id, atoll(o_s), atoll(e_s), req_id);
        free(buf); close(fd); return;
      }
      if (strcmp(method, "GET") == 0 && tail[0] == 0) {
        pthread_mutex_lock(&g_mu);
        proc_rec *pr = NULL;
        for (int i = 0; i < MAX_PROCS; i++) if (g_procs[i].used && g_procs[i].id == proc_id && strcmp(g_procs[i].box_id, bid) == 0) { pr = &g_procs[i]; break; }
        if (!pr) { pthread_mutex_unlock(&g_mu); send_err(fd, 404, "not_found", "process not found", req_id, NULL); free(buf); close(fd); return; }
        uint8_t out[512];
        int64_t on = bs_w_proc_status(cs(pr->running ? "running" : "exited"), pr->running, pr->exit_code,
                                      file_size_of(pr->out_path), file_size_of(pr->err_path), vb(out, sizeof out));
        pthread_mutex_unlock(&g_mu);
        send_json(fd, 200, NULL, out, (size_t)on);
        free(buf); close(fd); return;
      }
      if (strcmp(method, "DELETE") == 0 && tail[0] == 0) {
        pthread_mutex_lock(&g_mu);
        proc_rec *pr = NULL;
        for (int i = 0; i < MAX_PROCS; i++) if (g_procs[i].used && g_procs[i].id == proc_id && strcmp(g_procs[i].box_id, bid) == 0) { pr = &g_procs[i]; break; }
        pid_t target_pid = (pr && pr->running) ? pr->pid : 0;
        box_rec *bx = find_box_locked(bid);
        char bip[64] = "";
        if (bx) snprintf(bip, sizeof bip, "%s", bx->ip);
        pthread_mutex_unlock(&g_mu);
        if (!pr) { send_err(fd, 404, "not_found", "process not found", req_id, NULL); free(buf); close(fd); return; }
        if (gce_is_live() && bip[0]) {
          char kcmd[256], ob[64], eb[64]; size_t ol = 0, el = 0; bool to = false;
          snprintf(kcmd, sizeof kcmd, "p=$(cat /tmp/oa-pid-%d 2>/dev/null) && kill -9 -$p $p 2>/dev/null || true", proc_id);
          (void)gce_ssh_exec_capture(bip, kcmd, NULL, 0, ob, sizeof ob, &ol, eb, sizeof eb, &el, 10, &to);
        }
        if (target_pid > 0) kill(-target_pid, SIGKILL);
        send_json(fd, 200, NULL, "{\"ok\":true,\"status\":\"killed\"}", 29);
        free(buf); close(fd); return;
      }
    }
  }

  send_err(fd, 404, "not_found", "route not found", req_id, NULL);
  free(buf); close(fd);
}

static void *client_thread(void *arg) {
  int fd = *(int *)arg;
  free(arg);
  handle_client(fd);
  return NULL;
}

static void *ttl_reaper_thread(void *arg) {
  (void)arg;
  for (;;) {
    struct timespec ts = {30, 0};
    nanosleep(&ts, NULL);
    char del_ids[MAX_BOXES][16];
    int del_n = 0;
    pthread_mutex_lock(&g_mu);
    reap_expired_locked(del_ids, &del_n, MAX_BOXES);
    pthread_mutex_unlock(&g_mu);
    for (int i = 0; i < del_n; i++) {
      fprintf(stderr, "oa-boat: background reaper deleting expired GCE VM %s\n", del_ids[i]);
      gce_instance_op(del_ids[i], "DELETE", "");
    }
  }
  return NULL;
}

int main(int argc, char **argv) {
  signal(SIGPIPE, SIG_IGN);
  curl_global_init(CURL_GLOBAL_DEFAULT);

  char host[64] = "127.0.0.1";
  int port = 8088;
  const char *env_port = getenv("PORT");
  if (env_port && *env_port) { snprintf(host, sizeof host, "0.0.0.0"); port = atoi(env_port); }
  for (int i = 1; i < argc; i++) {
    if (strcmp(argv[i], "--listen") == 0 && i + 1 < argc) {
      const char *a = argv[++i];
      const char *col = strrchr(a, ':');
      if (col) {
        size_t hl = (size_t)(col - a);
        if (hl >= sizeof host) hl = sizeof host - 1;
        memcpy(host, a, hl); host[hl] = 0;
        port = atoi(col + 1);
      }
    }
  }
  const char *k = getenv("OA_BOAT_API_KEY");
  if (!k || strlen(k) < 8) { fprintf(stderr, "oa-boat: OA_BOAT_API_KEY (>= 8 chars) required\n"); return 2; }
  snprintf(g_api_key, sizeof g_api_key, "%s", k);
  const char *be = getenv("OA_BOAT_BACKEND");
  if (be && *be) snprintf(g_backend, sizeof g_backend, "%s", be);
  const char *ma = getenv("OA_BOAT_MAX_ACTIVE");
  if (ma && atoi(ma) > 0) g_max_active = atoi(ma);
  const char *gp = getenv("OA_BOAT_GCE_PROJECT");
  if (gp && *gp) snprintf(g_gce_project, sizeof g_gce_project, "%s", gp);
  const char *gz = getenv("OA_BOAT_GCE_ZONE");
  if (gz && *gz) snprintf(g_gce_zone, sizeof g_gce_zone, "%s", gz);
  const char *gi = getenv("OA_BOAT_GCE_IMAGE");
  if (gi && *gi) snprintf(g_gce_image, sizeof g_gce_image, "%s", gi);
  const char *gb = getenv("OA_BOAT_GCE_BASE");
  if (gb && *gb) snprintf(g_gce_base, sizeof g_gce_base, "%s", gb);
  const char *gt = getenv("OA_BOAT_GCE_TOKEN");
  if (gt && *gt) snprintf(g_gce_token, sizeof g_gce_token, "%s", gt);

  snprintf(g_work_root, sizeof g_work_root, "/tmp/oa-boat-%d", (int)getpid());
  mkdir_p(g_work_root);
  if (gce_is_live() && !gce_init_ssh()) {
    fprintf(stderr, "oa-boat: live GCE backend requires readable OA_BOAT_SSH_KEY_FILE\n");
    return 2;
  }

  int srv = socket(AF_INET, SOCK_STREAM, 0);
  if (srv < 0) { perror("socket"); return 1; }
  int one = 1;
  setsockopt(srv, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
  struct sockaddr_in sin = {0};
  sin.sin_family = AF_INET;
  sin.sin_port = htons((uint16_t)port);
  inet_pton(AF_INET, host, &sin.sin_addr);
  if (bind(srv, (struct sockaddr *)&sin, sizeof sin) < 0 || listen(srv, 64) < 0) { perror("bind/listen"); return 1; }
  {
    pthread_t rth;
    if (pthread_create(&rth, NULL, ttl_reaper_thread, NULL) == 0) pthread_detach(rth);
  }
  fprintf(stderr, "oa-boat: listening on %s:%d (backend=%s, max_active=%d, image=%s)\n", host, port, g_backend, g_max_active, g_gce_image);

  for (;;) {
    int cfd = accept(srv, NULL, NULL);
    if (cfd < 0) { if (errno == EINTR) continue; break; }
    int *pfd = malloc(sizeof *pfd);
    if (!pfd) { close(cfd); continue; }
    *pfd = cfd;
    pthread_t th;
    if (pthread_create(&th, NULL, client_thread, pfd) == 0) pthread_detach(th);
    else { close(cfd); free(pfd); }
  }
  close(srv);
  return 0;
}
