// Tier 1 Instant Cloud Tools execution route (#14127):
//   POST /api/tools/cloud/v1
// Supports read-only web_search (Exa), web_fetch (SSRF-guarded HTTPS), and
// GitHub REST API tools (github_tree, github_read_file, github_search_code,
// github_list_issues, github_get_pr) using transient WorkOS Pipes tokens
// (wiped immediately with explicit_bzero; never persisted or logged).
#include "api.h"
#include "auth-internal.h"
#include "cloud-tools-wire.generated.h"
#include "http_client.h"
#include "workos.h"

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static coax_string ct_cs(const char *s) { return (coax_string){s ? s : "", (int64_t)(s ? strlen(s) : 0)}; }

static const char *ct_github_base(void) {
  const char *v = getenv("OA_GITHUB_API_BASE");
  return (v && *v) ? v : "https://api.github.com";
}

static const char *ct_exa_base(void) {
  const char *v = getenv("OA_EXA_API_BASE");
  return (v && *v) ? v : "https://api.exa.ai";
}

static bool ct_is_local(const char *b) {
  return !strncmp(b, "http://127.0.0.1:", 17) || !strncmp(b, "http://localhost:", 17);
}

static size_t ct_url_encode(const char *src, char *out, size_t cap) {
  static const char hex[] = "0123456789ABCDEF";
  size_t n = 0;
  for (const unsigned char *p = (const unsigned char *)src; *p && n + 4 < cap; p++) {
    unsigned char c = *p;
    if (isalnum(c) || c == '-' || c == '_' || c == '.' || c == '~') {
      out[n++] = (char)c;
    } else if (c == ' ') {
      out[n++] = '+';
    } else {
      out[n++] = '%';
      out[n++] = hex[(c >> 4) & 0xF];
      out[n++] = hex[c & 0xF];
    }
  }
  out[n] = 0;
  return n;
}

static void ct_respond(int fd, const char *tool, bool ok, const char *output, const char *err) {
  size_t cap = 32768;
  uint8_t *buf = calloc(1, cap);
  if (!buf) { api_error(fd, 503, "Memory unavailable."); return; }
  int64_t n = cld_c_tool_res_write(ct_cs(tool), ok, ct_cs(output), ct_cs(err), (coax_v_u8){buf, (int64_t)cap});
  if (n < 1) {
    // Retry with truncated output if needed.
    char short_out[4096];
    snprintf(short_out, sizeof short_out, "%.3900s\n[truncated]", output ? output : "");
    n = cld_c_tool_res_write(ct_cs(tool), ok, ct_cs(short_out), ct_cs(err), (coax_v_u8){buf, (int64_t)cap});
  }
  if (n < 1) { free(buf); api_error(fd, 503, "Tool response unavailable."); return; }
  api_respond(fd, 200, "application/json", "Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n", buf, (size_t)n);
  free(buf);
}

// Detect whether a fetched body looks like HTML rather than JSON/plain text.
static bool ct_looks_html(const char *src, size_t len) {
  size_t i = 0;
  while (i < len && (src[i] == ' ' || src[i] == '\t' || src[i] == '\r' || src[i] == '\n')) i++;
  if (i >= len || src[i] != '<') return false;
  if (i + 9 <= len && !strncasecmp(src + i, "<!doctype", 9)) return true;
  if (i + 5 <= len && !strncasecmp(src + i, "<html", 5)) return true;
  if (i + 5 <= len && !strncasecmp(src + i, "<head", 5)) return true;
  if (i + 5 <= len && !strncasecmp(src + i, "<body", 5)) return true;
  return true;
}

// Copy non-HTML text/JSON with bounded length, normalizing control bytes while preserving <...> syntax.
static size_t ct_copy_plain(const char *src, size_t len, char *out, size_t cap) {
  size_t n = 0;
  for (size_t i = 0; i < len && n + 2 < cap; i++) {
    unsigned char uc = (unsigned char)src[i];
    if (uc == '\r') continue;
    if (uc == '\n' || uc == '\t' || uc >= 32) out[n++] = (char)uc;
  }
  out[n] = 0;
  return n;
}

// Minimal bounded HTML-to-text extractor (strips <script>/<style> blocks and HTML tags).
static size_t ct_extract_text(const char *src, size_t len, char *out, size_t cap) {
  size_t n = 0;
  bool in_tag = false, in_skip = false, prev_ws = true;
  for (size_t i = 0; i < len && n + 2 < cap; i++) {
    char c = src[i];
    if (!in_tag && c == '<') {
      if (i + 7 < len && (!strncasecmp(src + i, "<script", 7) || !strncasecmp(src + i, "<style", 6))) in_skip = true;
      else if (i + 8 < len && (!strncasecmp(src + i, "</script", 8) || !strncasecmp(src + i, "</style", 7))) in_skip = false;
      in_tag = true;
      continue;
    }
    if (in_tag) {
      if (c == '>') in_tag = false;
      continue;
    }
    if (in_skip) continue;
    unsigned char uc = (unsigned char)c;
    if (uc == '\r' || uc == '\t' || uc == ' ') {
      if (!prev_ws) { out[n++] = ' '; prev_ws = true; }
    } else if (uc == '\n') {
      if (!prev_ws) { out[n++] = '\n'; prev_ws = true; }
    } else if (uc >= 32) {
      out[n++] = c;
      prev_ws = false;
    }
  }
  out[n] = 0;
  return n;
}

// Slice lines [from_line .. to_line] with line numbers into bounded output buffer.
static size_t ct_slice_lines(const char *src, size_t len, int64_t from_line, int64_t to_line, char *out, size_t cap) {
  if (from_line < 1) from_line = 1;
  int64_t line_no = 1;
  size_t i = 0, n = 0;
  bool truncated = false;
  while (i < len) {
    size_t e = i;
    while (e < len && src[e] != '\n') e++;
    size_t l_len = e - i;
    if (l_len > 0 && src[i + l_len - 1] == '\r') l_len--;
    if (line_no >= from_line && (to_line <= 0 || line_no <= to_line)) {
      char prefix[24];
      int pn = snprintf(prefix, sizeof prefix, "%lld: ", (long long)line_no);
      if (pn < 0 || n + (size_t)pn + l_len + 32 >= cap) { truncated = true; break; }
      memcpy(out + n, prefix, (size_t)pn); n += (size_t)pn;
      memcpy(out + n, src + i, l_len); n += l_len;
      out[n++] = '\n';
    }
    if (to_line > 0 && line_no >= to_line) break;
    i = (e < len) ? e + 1 : len;
    line_no++;
  }
  if (truncated && n + 20 < cap) {
    int tn = snprintf(out + n, cap - n, "\n[truncated]");
    if (tn > 0) n += (size_t)tn;
  }
  out[n] = 0;
  return n;
}

// Phase 5 (#14129): POST /api/chat/sandbox/meter/v1
// Settles metered Cloud Sandbox runtime seconds against inference.credit and chat.turns_v2.
// Strictly enforces 0 msat for compute='local' (user's own enrolled computer).
static void sandbox_meter_handle(const request *q) {
  if (strcmp(q->method, "POST") != 0) { api_error(q->fd, 405, "POST this."); return; }
  const auth_principal *p = auth_principal_of(q);
  if (!p || p->kind != 2 || !p->account[0]) { api_error(q->fd, 401, "Log in first."); return; }
  if (!auth_same_site(q)) { api_error(q->fd, 403, "Cross-site write refused."); return; }
  if (!cld_c_tool_admit(p->kind, p->level >= 1, ct_cs(p->account), p->limited, p->frozen != 0)) {
    api_error(q->fd, 403, "This account cannot settle sandbox metering.");
    return;
  }
  coax_t_CpMeterReq mr;
  if (!cp_c_meter_req_read((coax_string){q->body, (int64_t)q->body_len}, (coax_v_t_CpMeterReq){&mr, 1})) {
    api_error(q->fd, 400, "Invalid sandbox metering request.");
    return;
  }
  char turn_id[40] = {0}, req_size[16] = {0};
  memcpy(turn_id, mr.turn.e, (size_t)mr.turn_n);
  if (mr.size_n > 0) memcpy(req_size, mr.size.e, (size_t)mr.size_n);
  long long seconds = (long long)mr.seconds;

  PGconn *db = api_db();
  if (!db) { api_error(q->fd, 503, "Account storage unavailable."); return; }
  PGresult *tx = PQexec(db, "BEGIN");
  if (!tx || PQresultStatus(tx) != PGRES_COMMAND_OK) { if (tx) PQclear(tx); api_error(q->fd, 503, "Transaction unavailable."); return; }
  PQclear(tx);

  const char *qv[2] = {p->account, turn_id};
  PGresult *tr = PQexecParams(db,
    "SELECT compute, workspace, cost_msat, sandbox_seconds FROM chat.turns_v2 WHERE account = $1 AND id = $2 FOR UPDATE",
    2, NULL, qv, NULL, NULL, 0);
  if (!tr || PQresultStatus(tr) != PGRES_TUPLES_OK) {
    if (tr) PQclear(tr);
    PQclear(PQexec(db, "ROLLBACK"));
    api_error(q->fd, 503, "Turn lookup failed.");
    return;
  }
  if (PQntuples(tr) != 1) {
    PQclear(tr);
    PQclear(PQexec(db, "ROLLBACK"));
    api_error(q->fd, 404, "Turn not found.");
    return;
  }
  char compute[16] = {0}, ws[96] = {0};
  snprintf(compute, sizeof compute, "%s", PQgetvalue(tr, 0, 0));
  snprintf(ws, sizeof ws, "%s", PQgetvalue(tr, 0, 1));
  long long prev_cost = atoll(PQgetvalue(tr, 0, 2));
  PQclear(tr);

  const char *sz = *req_size ? req_size : (!strncmp(ws, "sandbox:", 8) ? ws + 8 : "small");
  long long usd_per_btc = 100000;
  PGresult *br = PQexec(db, "SELECT usd_per_btc FROM inference.btc_usd LIMIT 1");
  if (br && PQresultStatus(br) == PGRES_TUPLES_OK && PQntuples(br) == 1) {
    long long v = atoll(PQgetvalue(br, 0, 0));
    if (v > 0) usd_per_btc = v;
  }
  if (br) PQclear(br);

  int64_t debit = cp_c_turn_debit_msat(ct_cs(compute), ct_cs(sz), (int64_t)seconds, (int64_t)usd_per_btc);
  if (debit < 0) {
    PQclear(PQexec(db, "ROLLBACK"));
    api_error(q->fd, 400, "Turn compute tier or sandbox size is not meterable.");
    return;
  }

  bool is_sandbox = !strcmp(compute, "sandbox");
  bool free_acct = p->team || p->admin;
  long long billed = (is_sandbox && !free_acct) ? (long long)debit : 0;
  long long new_total = prev_cost + (is_sandbox ? (long long)debit : 0);

  if (is_sandbox) {
    char sec_s[32], deb_s[32];
    snprintf(sec_s, sizeof sec_s, "%lld", seconds);
    snprintf(deb_s, sizeof deb_s, "%lld", (long long)debit);
    const char *uv[4] = {p->account, turn_id, sec_s, deb_s};
    PGresult *ur = PQexecParams(db,
      "UPDATE chat.turns_v2 SET sandbox_seconds = sandbox_seconds + $3::bigint, cost_msat = cost_msat + $4::bigint WHERE account = $1 AND id = $2",
      4, NULL, uv, NULL, NULL, 0);
    if (!ur || PQresultStatus(ur) != PGRES_COMMAND_OK) {
      if (ur) PQclear(ur);
      PQclear(PQexec(db, "ROLLBACK"));
      api_error(q->fd, 503, "Could not update turn compute cost.");
      return;
    }
    PQclear(ur);
  }

  const char *who = p->pubkey[0] ? p->pubkey : p->account;
  if (billed > 0) {
    char bil_s[32];
    snprintf(bil_s, sizeof bil_s, "%lld", billed);
    const char *dv[3] = {who, p->account, bil_s};
    PGresult *dr = PQexecParams(db,
      "UPDATE inference.credit SET msat = GREATEST(0, msat - $3::bigint), updated = now() WHERE pubkey = $1 OR pubkey = $2",
      3, NULL, dv, NULL, NULL, 0);
    if (!dr || PQresultStatus(dr) != PGRES_COMMAND_OK) {
      if (dr) PQclear(dr);
      PQclear(PQexec(db, "ROLLBACK"));
      api_error(q->fd, 503, "Could not debit compute credit.");
      return;
    }
    PQclear(dr);
  }

  long long credit_left = 0;
  const char *cv[2] = {who, p->account};
  PGresult *cr = PQexecParams(db,
    "SELECT coalesce((SELECT msat FROM inference.credit WHERE pubkey = $1 OR pubkey = $2 ORDER BY msat DESC LIMIT 1), 0)",
    2, NULL, cv, NULL, NULL, 0);
  if (cr && PQresultStatus(cr) == PGRES_TUPLES_OK && PQntuples(cr) == 1) {
    credit_left = atoll(PQgetvalue(cr, 0, 0));
  }
  if (cr) PQclear(cr);

  PGresult *cm = PQexec(db, "COMMIT");
  if (!cm || PQresultStatus(cm) != PGRES_COMMAND_OK) {
    if (cm) PQclear(cm);
    PQclear(PQexec(db, "ROLLBACK"));
    api_error(q->fd, 503, "Could not commit metering settlement.");
    return;
  }
  PQclear(cm);

  uint8_t out[512];
  int64_t n = cp_c_meter_res_write(ct_cs(turn_id), ct_cs(compute), (int64_t)seconds,
                                   is_sandbox ? debit : 0, new_total, credit_left, (coax_v_u8){out, sizeof out});
  if (n < 1) { api_error(q->fd, 503, "Could not encode metering response."); return; }
  api_respond(q->fd, 200, "application/json", "Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n", out, (size_t)n);
}

bool cloud_tools_route(const request *q) {
  if (strcmp(q->path, "/api/chat/sandbox/meter/v1") == 0) { sandbox_meter_handle(q); return true; }
  if (strcmp(q->path, "/api/tools/cloud/v1") != 0) return false;
  if (strcmp(q->method, "POST") != 0) { api_error(q->fd, 405, "POST this."); return true; }
  const auth_principal *p = auth_principal_of(q);
  if (!p || p->kind != 2 || !p->account[0]) { api_error(q->fd, 401, "Log in first."); return true; }
  if (!auth_same_site(q)) { api_error(q->fd, 403, "Cross-site write refused."); return true; }
  if (!cld_c_tool_admit(p->kind, p->level >= 1, ct_cs(p->account), p->limited, p->frozen != 0)) {
    api_error(q->fd, 403, "This account cannot run cloud tools.");
    return true;
  }
  PGconn *db = api_db();
  if (!db) { api_error(q->fd, 503, "Account storage unavailable."); return true; }
  if (!api_limit(db, "cloud-tools", p->account, 60, NULL)) {
    api_error(q->fd, 429, "Cloud tool rate limit reached. Try again shortly.");
    return true;
  }

  bool allow_local_fetch = getenv("OA_CLOUD_TOOLS_ALLOW_LOCAL") && !strcmp(getenv("OA_CLOUD_TOOLS_ALLOW_LOCAL"), "1");
  coax_t_CldToolReq *req = calloc(1, sizeof *req);
  if (!req) { api_error(q->fd, 503, "Memory unavailable."); return true; }
  if (!cld_c_tool_req_read((coax_string){q->body, (int64_t)q->body_len}, allow_local_fetch, (coax_v_t_CldToolReq){req, 1})) {
    free(req);
    api_error(q->fd, 400, "Invalid cloud tool request or disallowed argument.");
    return true;
  }

  char tool[33] = {0}, repo[161] = {0}, path[513] = {0}, ref[129] = {0}, query[257] = {0}, url[1025] = {0}, state[17] = {0};
  memcpy(tool, req->tool.e, (size_t)req->tool_n);
  memcpy(repo, req->repo.e, (size_t)req->repo_n);
  memcpy(path, req->path.e, (size_t)req->path_n);
  memcpy(ref, req->ref.e, (size_t)req->ref_n);
  memcpy(query, req->query.e, (size_t)req->query_n);
  memcpy(url, req->url.e, (size_t)req->url_n);
  memcpy(state, req->state.e, (size_t)req->state_n);
  int64_t kind = req->kind, num = req->num, from_line = req->from_line, to_line = req->to_line;
  free(req);

  // 1) web_search via Exa API
  if (kind == 1) {
    const char *exa_key = getenv("OA_EXA_API_KEY");
    if (!exa_key || !*exa_key) {
      ct_respond(q->fd, tool, false, "", "Web search is not configured on this server (OA_EXA_API_KEY missing).");
      return true;
    }
    char exa_url[512], key_hdr[300], q_esc[600], body[1024];
    snprintf(exa_url, sizeof exa_url, "%s/search", ct_exa_base());
    snprintf(key_hdr, sizeof key_hdr, "x-api-key: %s", exa_key);
    size_t qe = 0;
    for (const unsigned char *s = (const unsigned char *)query; *s && qe + 6 < sizeof q_esc; s++) {
      if (*s == '"' || *s == '\\') { q_esc[qe++] = '\\'; q_esc[qe++] = (char)*s; }
      else if (*s >= 32) q_esc[qe++] = (char)*s;
    }
    q_esc[qe] = 0;
    int bn = snprintf(body, sizeof body, "{\"query\":\"%s\",\"numResults\":%lld,\"contents\":{\"text\":{\"maxCharacters\":400}}}", q_esc, (long long)num);
    const char *hdrs[] = { key_hdr, "Content-Type: application/json", "Accept: application/json", NULL };
    http_client_req hr = {
      .method = "POST", .url = exa_url, .body = body, .body_len = bn > 0 ? (size_t)bn : 0,
      .headers = hdrs, .connect_timeout_ms = 5000, .total_timeout_ms = 15000,
      .max_response_bytes = 262144, .allow_http_local = ct_is_local(ct_exa_base()),
    };
    http_client_res res;
    http_client_send(&hr, &res);
    explicit_bzero(key_hdr, sizeof key_hdr);
    if (res.status != 200 || !res.body || res.body_len < 2) {
      http_client_res_free(&res);
      ct_respond(q->fd, tool, false, "", "Web search upstream failed.");
      return true;
    }
    coax_t_JvNode *nodes = calloc(4096, sizeof *nodes);
    uint8_t *out = calloc(1, 16384);
    int64_t on = (nodes && out) ? cld_c_format_exa((coax_string){res.body, (int64_t)res.body_len},
                                                   (coax_v_t_JvNode){nodes, 4096}, (coax_v_u8){out, 16383}) : -1;
    free(nodes);
    http_client_res_free(&res);
    if (on < 0) { free(out); ct_respond(q->fd, tool, false, "", "Could not parse search results."); return true; }
    out[on] = 0;
    ct_respond(q->fd, tool, true, (const char *)out, "");
    free(out);
    return true;
  }

  // 2) web_fetch (SSRF-guarded public HTTPS)
  if (kind == 2) {
    const char *hdrs[] = {
      "User-Agent: openagents-cloud-fetch/1.0",
      "Accept: text/markdown, text/plain, text/html;q=0.9, application/json;q=0.9, */*;q=0.8",
      NULL
    };
    http_client_req hr = {
      .method = "GET", .url = url, .headers = hdrs,
      .connect_timeout_ms = 5000, .total_timeout_ms = 12000,
      .max_response_bytes = 65536, .allow_http_local = allow_local_fetch,
    };
    http_client_res res;
    http_client_send(&hr, &res);
    if (res.status < 200 || res.status >= 300 || !res.body) {
      char msg[96];
      snprintf(msg, sizeof msg, res.status > 0 ? "Fetch returned HTTP %ld." : "Fetch could not reach host.", res.status);
      http_client_res_free(&res);
      ct_respond(q->fd, tool, false, "", msg);
      return true;
    }
    char *text = calloc(1, 16384);
    if (!text) { http_client_res_free(&res); api_error(q->fd, 503, "Memory unavailable."); return true; }
    if (ct_looks_html(res.body, res.body_len)) ct_extract_text(res.body, res.body_len, text, 16384);
    else ct_copy_plain(res.body, res.body_len, text, 16384);
    http_client_res_free(&res);
    ct_respond(q->fd, tool, true, text, "");
    free(text);
    return true;
  }

  // 3) GitHub REST API tools (transient WorkOS Pipes OAuth token; falls back to public API for public repos)
  char token[512] = {0}, tok_err[128] = {0}, auth_hdr[600] = {0};
  bool has_tok = workos_pipes_github_token(db, p->account, token, sizeof token, tok_err, sizeof tok_err);
  if (has_tok) {
    snprintf(auth_hdr, sizeof auth_hdr, "Authorization: Bearer %s", token);
  }
  explicit_bzero(token, sizeof token);

  char gh_url[1536] = {0};
  const char *base = ct_github_base();
  bool want_raw = (kind == 4);
  if (kind == 3) { // github_tree
    if (*path && *ref) snprintf(gh_url, sizeof gh_url, "%s/repos/%s/contents/%s?ref=%s", base, repo, path, ref);
    else if (*path) snprintf(gh_url, sizeof gh_url, "%s/repos/%s/contents/%s", base, repo, path);
    else if (*ref) snprintf(gh_url, sizeof gh_url, "%s/repos/%s/contents?ref=%s", base, repo, ref);
    else snprintf(gh_url, sizeof gh_url, "%s/repos/%s/contents", base, repo);
  } else if (kind == 4) { // github_read_file
    if (*ref) snprintf(gh_url, sizeof gh_url, "%s/repos/%s/contents/%s?ref=%s", base, repo, path, ref);
    else snprintf(gh_url, sizeof gh_url, "%s/repos/%s/contents/%s", base, repo, path);
  } else if (kind == 5) { // github_search_code
    char q_full[512], q_enc[1024];
    snprintf(q_full, sizeof q_full, "%s repo:%s", query, repo);
    ct_url_encode(q_full, q_enc, sizeof q_enc);
    snprintf(gh_url, sizeof gh_url, "%s/search/code?q=%s&per_page=20", base, q_enc);
  } else if (kind == 6) { // github_list_issues
    snprintf(gh_url, sizeof gh_url, "%s/repos/%s/issues?state=%s&per_page=20", base, repo, *state ? state : "open");
  } else if (kind == 7) { // github_get_pr
    snprintf(gh_url, sizeof gh_url, "%s/repos/%s/pulls/%lld", base, repo, (long long)num);
  }

  const char *hdrs[6];
  int hi = 0;
  if (has_tok) hdrs[hi++] = auth_hdr;
  hdrs[hi++] = want_raw ? "Accept: application/vnd.github.raw+json" : "Accept: application/vnd.github+json";
  hdrs[hi++] = "X-GitHub-Api-Version: 2022-11-28";
  hdrs[hi++] = "User-Agent: openagents-api";
  hdrs[hi] = NULL;

  http_client_req hr = {
    .method = "GET", .url = gh_url, .headers = hdrs,
    .connect_timeout_ms = 5000, .total_timeout_ms = 15000,
    .max_response_bytes = 262144, .allow_http_local = ct_is_local(base),
  };
  http_client_res res;
  http_client_send(&hr, &res);
  explicit_bzero(auth_hdr, sizeof auth_hdr);

  if (res.status != 200 || !res.body) {
    char msg[160];
    if (!has_tok && (res.status == 401 || res.status == 403 || res.status == 404)) {
      snprintf(msg, sizeof msg, "GitHub returned HTTP %ld. Connect GitHub in Settings to access private repositories or search code.", res.status);
    } else if (res.status == 404) {
      snprintf(msg, sizeof msg, "Repository, path, or pull request not found on GitHub.");
    } else if (res.status > 0) {
      snprintf(msg, sizeof msg, "GitHub API returned HTTP %ld.", res.status);
    } else {
      snprintf(msg, sizeof msg, "Could not reach GitHub API.");
    }
    http_client_res_free(&res);
    ct_respond(q->fd, tool, false, "", msg);
    return true;
  }

  if (kind == 4) { // github_read_file raw text
    char *sliced = calloc(1, 14336);
    if (!sliced) { http_client_res_free(&res); api_error(q->fd, 503, "Memory unavailable."); return true; }
    ct_slice_lines(res.body, res.body_len, from_line, to_line, sliced, 14336);
    http_client_res_free(&res);
    ct_respond(q->fd, tool, true, sliced, "");
    free(sliced);
    return true;
  }

  coax_t_JvNode *nodes = calloc(8192, sizeof *nodes);
  uint8_t *out = calloc(1, 16384);
  int64_t on = -1;
  if (nodes && out) {
    coax_string raw = {res.body, (int64_t)res.body_len};
    coax_v_t_JvNode nv = {nodes, 8192};
    coax_v_u8 ov = {out, 16383};
    if (kind == 3) on = cld_c_format_gh_tree(raw, nv, ov);
    else if (kind == 5) on = cld_c_format_gh_search(raw, nv, ov);
    else if (kind == 6) on = cld_c_format_gh_issues(raw, nv, ov);
    else if (kind == 7) on = cld_c_format_gh_pr(raw, nv, ov);
  }
  free(nodes);
  http_client_res_free(&res);
  if (on < 0) {
    free(out);
    ct_respond(q->fd, tool, false, "", "Could not parse GitHub response.");
    return true;
  }
  out[on] = 0;
  ct_respond(q->fd, tool, true, (const char *)out, "");
  free(out);
  return true;
}
