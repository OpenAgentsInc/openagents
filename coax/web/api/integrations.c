// WorkOS Pipes GitHub integration routes (#14126):
//   GET  /api/integrations/github/status     (JSON or ?as=lines)
//   POST /api/integrations/github/connect    (returns WorkOS Pipes OAuth authorize URL)
//   GET  /api/integrations/github/repos      (sanitized repo list via on-demand Pipes token)
//   POST /api/integrations/github/disconnect (revokes user's WorkOS Pipes GitHub connection)
//
// Policy & wire codecs live in coax/lib/workos/pipes_policy.coax & pipes_wire.coax.
// OAuth tokens are never stored in Postgres, never written to logs, and wiped immediately
// from stack buffers via explicit_bzero after the GitHub API call completes.
#include "api.h"
#include "auth-internal.h"
#include "http_client.h"
#include "workos.h"
#include "workos-sdk.generated.h"

#include <stdlib.h>
#include <string.h>

static coax_string is_cs(const char *s) { return (coax_string){s ? s : "", (int64_t)(s ? strlen(s) : 0)}; }

static const char *github_api_base(void) {
  const char *v = getenv("OA_GITHUB_API_BASE");
  return (v && *v) ? v : "https://api.github.com";
}

static bool github_allow_local(void) {
  const char *b = github_api_base();
  return !strncmp(b, "http://127.0.0.1:", 17) || !strncmp(b, "http://localhost:", 17);
}

static void integ_respond_json(int fd, int status, const void *data, size_t n) {
  api_respond(fd, status, "application/json", "Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n", data, n);
}

static void integ_respond_lines(int fd, int status, const void *data, size_t n) {
  api_respond(fd, status, "text/plain; charset=utf-8", "Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n", data, n);
}

bool integrations_route(const request *q) {
  if (strncmp(q->path, "/api/integrations/github/", 25) != 0) return false;
  const char *op = q->path + 25;
  bool is_status = !strcmp(op, "status");
  bool is_connect = !strcmp(op, "connect");
  bool is_repos = !strcmp(op, "repos");
  bool is_disconnect = !strcmp(op, "disconnect");
  if (!is_status && !is_connect && !is_repos && !is_disconnect) return false;

  const auth_principal *p = auth_principal_of(q);
  if (!p || p->kind != 2 || !p->account[0]) {
    api_error(q->fd, 401, "Log in first.");
    return true;
  }

  bool post = !strcmp(q->method, "POST");
  bool get = !strcmp(q->method, "GET");
  if ((is_status || is_repos) && !get) { api_error(q->fd, 405, "GET this."); return true; }
  if ((is_connect || is_disconnect) && !post) { api_error(q->fd, 405, "POST this."); return true; }
  if (post && !auth_same_site(q)) { api_error(q->fd, 403, "Cross-site write refused."); return true; }
  if (post && !wos_c_pipes_admit(p->kind, p->level >= 1, is_cs(p->account), p->limited, p->frozen != 0)) {
    api_error(q->fd, 403, "This account cannot manage GitHub connections.");
    return true;
  }

  PGconn *db = api_db();
  if (!db) { api_error(q->fd, 503, "Account storage unavailable."); return true; }
  if (!api_limit(db, "integrations-github", p->account, 60, NULL)) {
    api_error(q->fd, 429, "Try again shortly.");
    return true;
  }

  char as_q[16] = {0};
  api_query(q, "as", as_q, sizeof as_q);
  bool as_lines = !strcmp(as_q, "lines");

  char uid[128] = "", org[128] = "";
  bool has_wos = workos_lookup_identity(db, p->account, uid, org);

  if (is_status) {
    bool connected = false;
    uint8_t scopes_b[256] = {0};
    int64_t scopes_n = 0;
    if (has_wos && workos_pipes_configured()) {
      uint8_t url_b[512];
      int64_t un = 0;
      if (wos_c_build_pipes_providers(is_cs(workos_pipes_base_url()), is_cs(uid), is_cs(org),
                                      (coax_v_u8){url_b, (int64_t)sizeof url_b - 1}, (coax_v_s64){&un, 1})) {
        url_b[un] = 0;
        http_client_res res;
        workos_api_request("GET", (const char *)url_b, NULL, 0, &res);
        if (res.status == 200 && res.body && res.body_len > 0) {
          connected = wos_c_pipes_parse_provider_connected(
              (coax_string){res.body, (int64_t)res.body_len}, is_cs("github"),
              (coax_v_u8){scopes_b, (int64_t)sizeof scopes_b - 1}, (coax_v_s64){&scopes_n, 1});
        }
        http_client_res_free(&res);
      }
    }
    scopes_b[scopes_n >= 0 && scopes_n < (int64_t)sizeof scopes_b ? scopes_n : 0] = 0;
    uint8_t out[1024];
    int64_t n = wos_c_pipes_status_write(connected, is_cs("github"), is_cs((const char *)scopes_b), is_cs(""),
                                         as_lines, (coax_v_u8){out, sizeof out});
    if (n < 1) { api_error(q->fd, 503, "GitHub status unavailable."); return true; }
    if (as_lines) integ_respond_lines(q->fd, 200, out, (size_t)n);
    else integ_respond_json(q->fd, 200, out, (size_t)n);
    return true;
  }

  if (is_connect) {
    if (!workos_pipes_configured()) { api_error(q->fd, 503, "WorkOS Pipes is not configured."); return true; }
    if (!has_wos) { api_error(q->fd, 409, "Sign in with WorkOS to connect GitHub."); return true; }
    uint8_t req_rt[260] = {0};
    int64_t rtn = wos_c_pipes_connect_req_read((coax_string){q->body, (int64_t)q->body_len},
                                               (coax_v_u8){req_rt, 256});
    if (rtn < 0) { api_error(q->fd, 400, "Invalid GitHub connect request."); return true; }
    req_rt[rtn] = 0;
    const char *origin = getenv("OA_API_ORIGIN");
    if (!origin || !*origin) origin = "https://openagents.com";
    if (!wos_c_pipes_return_to_ok(is_cs(origin), is_cs((const char *)req_rt))) {
      api_error(q->fd, 400, "Invalid return_to redirect.");
      return true;
    }
    char full_rt[512];
    if (rtn == 0) snprintf(full_rt, sizeof full_rt, "%s/settings/account?github=connected", origin);
    else if (req_rt[0] == '/') snprintf(full_rt, sizeof full_rt, "%s%s", origin, (const char *)req_rt);
    else snprintf(full_rt, sizeof full_rt, "%s", (const char *)req_rt);

    uint8_t url_b[512], body_b[1024];
    int64_t un = 0, bn = 0;
    if (!wos_c_build_pipes_authorize(
            is_cs(workos_pipes_base_url()), is_cs("github"), is_cs(uid), is_cs(org), is_cs(full_rt),
            (coax_v_u8){url_b, (int64_t)sizeof url_b - 1}, (coax_v_u8){body_b, (int64_t)sizeof body_b - 1},
            (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1})) {
      api_error(q->fd, 400, "Could not build GitHub connect request.");
      return true;
    }
    url_b[un] = 0; body_b[bn] = 0;
    http_client_res res;
    workos_api_request("POST", (const char *)url_b, (const char *)body_b, (size_t)bn, &res);
    uint8_t auth_url[2050] = {0};
    int64_t aun = (res.status >= 200 && res.status < 300 && res.body && res.body_len > 0)
                      ? wos_c_pipes_parse_authorize_url((coax_string){res.body, (int64_t)res.body_len},
                                                        workos_pipes_allow_local(),
                                                        (coax_v_u8){auth_url, 2048})
                      : -1;
    http_client_res_free(&res);
    if (aun < 10) {
      api_error(q->fd, 502, "WorkOS Pipes did not return a valid GitHub authorization URL.");
      return true;
    }
    auth_url[aun] = 0;
    uint8_t out[2304];
    int64_t on = wos_c_pipes_connect_write(is_cs("github"), is_cs((const char *)auth_url), (coax_v_u8){out, sizeof out});
    if (on < 1) { api_error(q->fd, 503, "Connect response unavailable."); return true; }
    integ_respond_json(q->fd, 200, out, (size_t)on);
    return true;
  }

  if (is_repos) {
    char token[512] = {0}, err[128] = {0};
    if (!workos_pipes_github_token(db, p->account, token, sizeof token, err, sizeof err)) {
      explicit_bzero(token, sizeof token);
      api_error(q->fd, 412, *err ? err : "GitHub is not connected.");
      return true;
    }
    char gh_url[512], auth_hdr[600];
    snprintf(gh_url, sizeof gh_url, "%s/user/repos?sort=updated&per_page=50", github_api_base());
    snprintf(auth_hdr, sizeof auth_hdr, "Authorization: Bearer %s", token);
    explicit_bzero(token, sizeof token);
    const char *hdrs[] = {
      auth_hdr,
      "Accept: application/vnd.github+json",
      "X-GitHub-Api-Version: 2022-11-28",
      "User-Agent: openagents-api",
      NULL
    };
    http_client_req req = {
      .method = "GET",
      .url = gh_url,
      .body = NULL,
      .body_len = 0,
      .headers = hdrs,
      .connect_timeout_ms = 5000,
      .total_timeout_ms = 15000,
      .max_response_bytes = 524288,
      .allow_http_local = github_allow_local(),
    };
    http_client_res res;
    http_client_send(&req, &res);
    explicit_bzero(auth_hdr, sizeof auth_hdr);
    if (res.status != 200 || !res.body || res.body_len < 2) {
      int st = (res.status == 401 || res.status == 403) ? 412 : 502;
      http_client_res_free(&res);
      api_error(q->fd, st, st == 412 ? "GitHub authorization expired. Reconnect GitHub." : "GitHub repositories unavailable.");
      return true;
    }
    coax_t_JvNode *nodes = calloc(8192, sizeof *nodes);
    uint8_t *out = calloc(1, 65536);
    int64_t on = (nodes && out) ? wos_c_pipes_repos_filter(
                                      (coax_string){res.body, (int64_t)res.body_len}, as_lines,
                                      (coax_v_t_JvNode){nodes, 8192}, (coax_v_u8){out, 65536})
                                : -1;
    free(nodes);
    http_client_res_free(&res);
    if (on < 0) {
      free(out);
      api_error(q->fd, 502, "Could not parse GitHub repositories.");
      return true;
    }
    if (as_lines) integ_respond_lines(q->fd, 200, out, (size_t)on);
    else integ_respond_json(q->fd, 200, out, (size_t)on);
    free(out);
    return true;
  }

  // POST /api/integrations/github/disconnect
  if (!workos_pipes_configured()) { api_error(q->fd, 503, "WorkOS Pipes is not configured."); return true; }
  if (!has_wos) { api_error(q->fd, 404, "GitHub is not connected."); return true; }
  uint8_t url_b[512];
  int64_t un = 0;
  if (!wos_c_build_pipes_disconnect(is_cs(workos_pipes_base_url()), is_cs(uid), is_cs("github"), is_cs(org),
                                    (coax_v_u8){url_b, (int64_t)sizeof url_b - 1}, (coax_v_s64){&un, 1})) {
    api_error(q->fd, 400, "Could not build disconnect request.");
    return true;
  }
  url_b[un] = 0;
  http_client_res res;
  workos_api_request("DELETE", (const char *)url_b, NULL, 0, &res);
  bool ok = (res.status == 200 || res.status == 202 || res.status == 204 || res.status == 404);
  http_client_res_free(&res);
  if (!ok) { api_error(q->fd, 502, "Could not disconnect GitHub."); return true; }
  uint8_t out[256];
  int64_t on = wos_c_pipes_disconnect_write(is_cs("github"), true, (coax_v_u8){out, sizeof out});
  if (on < 1) { api_error(q->fd, 503, "Disconnect response unavailable."); return true; }
  integ_respond_json(q->fd, 200, out, (size_t)on);
  return true;
}
