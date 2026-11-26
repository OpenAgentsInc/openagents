// WorkOS authentication & data sovereignty backend (docs/auth/2026-11-26-workos-coax-sdk.md, #14100).
//
// Mediates WorkOS Magic Auth, Google/GitHub/Apple OAuth, Enterprise SSO, multi-org selection,
// and native PKCE / CLI device exchange behind OA_WORKOS_ENABLED while preserving OpenAgents'
// hybrid acct_ spine and Option A1 opaque sessions (__Host-oa_session for web, device-bound
// oa_at_/oa_rt_ with OA-Device-Proof for native apps) at L1.
//
// All cryptographic verification (WorkOS-Signature HMAC-SHA256 for webhooks ±180s and AuthKit
// Actions ±30s, signed Action verdicts, PKCE S256, constant-time state compare, RS256 JWKS
// verification, and WorkOS OpenAPI request building) runs in the checked-in #authority Coax SDK
// (coax/lib/workos/adapters/c_unit.coax -> workos-sdk-unit.c).
#include "workos.h"
#include "auth-internal.h"
#include "http_client.h"
#include "workos-sdk.generated.h"

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define WOS_PENDING_COOKIE "__Host-oa_wos_pending"
#define WOS_STATE_COOKIE   "__Host-oa_wos_state"
#define WOS_ORG_COOKIE     "__Host-oa_wos_org"

enum { WOS_SIG_OK = 0, WOS_JWT_OK = 0, WOS_ERR_KIND_ORG_SELECT = 1, WOS_ACTION_AUTH = 1, WOS_ACTION_REG = 2 };

static coax_string cs(const char *s) { return (coax_string){s ? s : "", (int64_t)(s ? strlen(s) : 0)}; }
static const char *env_or(const char *name, const char *fallback) {
  const char *v = getenv(name);
  return (v && *v) ? v : fallback;
}
static bool workos_flag_on(void) {
  const char *v = getenv("OA_WORKOS_ENABLED");
  return v && !strcmp(v, "1");
}
static bool workos_enabled_for_request(const request *q) {
  if (!workos_flag_on()) return false;
  // Listed first-party sites (gamebuilder.org) remain untouched on existing email codes in Phase 3
  // unless OA_WORKOS_GAMEBUILDER_ENABLED=1 is explicitly enabled in Phase 5.
  if (auth_listed_site(q)) {
    const char *gb = getenv("OA_WORKOS_GAMEBUILDER_ENABLED");
    return gb && !strcmp(gb, "1");
  }
  return true;
}
static const char *workos_base_url(void) { return env_or("OA_WORKOS_BASE_URL", "https://api.workos.com"); }
static const char *workos_api_key(void) { return env_or("OA_WORKOS_API_KEY", ""); }
static const char *workos_client_id(void) { return env_or("OA_WORKOS_CLIENT_ID", ""); }
static const char *workos_webhook_secret(void) { return env_or("OA_WORKOS_WEBHOOK_SECRET", ""); }
static const char *workos_action_secret(void) { return env_or("OA_WORKOS_ACTION_SECRET", ""); }
static bool workos_allow_local(void) {
  const char *b = workos_base_url();
  return !strncmp(b, "http://127.0.0.1:", 17) || !strncmp(b, "http://localhost:", 17);
}
static bool workos_configured(void) {
  return workos_api_key()[0] != 0 && workos_client_id()[0] != 0;
}

static void workos_http(const char *method, const char *url, const char *body, size_t body_len, bool with_auth, http_client_res *res) {
  char auth_hdr[512];
  const char *hdrs[5];
  int nh = 0;
  if (with_auth && workos_api_key()[0]) {
    snprintf(auth_hdr, sizeof auth_hdr, "Authorization: Bearer %s", workos_api_key());
    hdrs[nh++] = auth_hdr;
  }
  if (body && body_len > 0) hdrs[nh++] = "Content-Type: application/json";
  hdrs[nh++] = "Accept: application/json";
  hdrs[nh++] = "User-Agent: openagents-api/workos-coax-sdk";
  hdrs[nh] = NULL;
  http_client_req req = {
    .method = method,
    .url = url,
    .body = body,
    .body_len = body_len,
    .headers = hdrs,
    .connect_timeout_ms = 5000,
    .total_timeout_ms = 15000,
    .max_response_bytes = 262144,
    .allow_http_local = workos_allow_local(),
  };
  http_client_send(&req, res);
}

static bool same_site_next(const char *s) {
  if (!s || s[0] != '/') return false;
  if (s[1] == '/' || s[1] == '\\') return false;
  if (!strncmp(s, "/login", 6) && strncmp(s, "/login/device", 13) != 0) return false;
  for (const unsigned char *p = (const unsigned char *)s; *p; p++)
    if (*p <= ' ' || *p >= 127 || *p == '\\') return false;
  return strlen(s) < 256;
}

static char *body_copy(const request *q) {
  char *b = malloc(q->body_len + 1);
  if (!b) return NULL;
  if (q->body_len) memcpy(b, q->body, q->body_len);
  b[q->body_len] = 0;
  return b;
}

static void radar_context(const request *q, char ip[128], char ua[256]) {
  auth_visitor(q, ip);
  if (!api_header(q, "User-Agent", ua, 256)) ua[0] = 0;
}

// Upsert a WorkOS user/session record locally from an authenticate response body so
// freeze/logout-everywhere can revoke sessions even before the next Events API poll.
static void mirror_auth_response(PGconn *db, const char *auth_json, char uid_out[128], char email_out[300], bool *verified_out,
                                 char org_out[128], char sid_out[128], char method_out[64]) {
  uid_out[0] = 0; email_out[0] = 0; *verified_out = false; org_out[0] = 0; sid_out[0] = 0; method_out[0] = 0;
  const char *v[1] = {auth_json};
  PGresult *r = PQexecParams(
      db,
      "WITH doc AS (SELECT $1::jsonb j) "
      "SELECT coalesce(j->'user'->>'id', ''), "
      "       coalesce(j->'user'->>'email', ''), "
      "       coalesce((j->'user'->>'email_verified')::boolean, false), "
      "       coalesce(j->>'organization_id', ''), "
      "       coalesce(j->>'authentication_method', ''), "
      "       coalesce((j->'user')::text, ''), "
      "       coalesce(j->>'access_token', '') "
      "FROM doc",
      1, NULL, v, NULL, NULL, 0);
  char user_raw[8192] = "", atok[4096] = "";
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    snprintf(uid_out, 128, "%s", PQgetvalue(r, 0, 0));
    snprintf(email_out, 300, "%s", PQgetvalue(r, 0, 1));
    *verified_out = !strcmp(PQgetvalue(r, 0, 2), "t");
    snprintf(org_out, 128, "%s", PQgetvalue(r, 0, 3));
    snprintf(method_out, 64, "%s", PQgetvalue(r, 0, 4));
    snprintf(user_raw, sizeof user_raw, "%s", PQgetvalue(r, 0, 5));
    snprintf(atok, sizeof atok, "%s", PQgetvalue(r, 0, 6));
  }
  PQclear(r);

  // Extract WorkOS session id (sid) from access_token JWT payload if present.
  if (atok[0]) {
    const char *dot1 = strchr(atok, '.');
    const char *dot2 = dot1 ? strchr(dot1 + 1, '.') : NULL;
    if (dot1 && dot2 && dot2 > dot1 + 1) {
      size_t plen = (size_t)(dot2 - (dot1 + 1));
      if (plen < 3072) {
        char b64[3080];
        memcpy(b64, dot1 + 1, plen);
        for (size_t i = 0; i < plen; i++) { if (b64[i] == '-') b64[i] = '+'; else if (b64[i] == '_') b64[i] = '/'; }
        while (plen % 4 != 0) b64[plen++] = '=';
        b64[plen] = 0;
        const char *pv[1] = {b64};
        PGresult *pr = PQexecParams(db, "SELECT coalesce((convert_from(decode($1, 'base64'), 'UTF8')::jsonb)->>'sid', '')", 1, NULL, pv, NULL, NULL, 0);
        if (PQresultStatus(pr) == PGRES_TUPLES_OK && PQntuples(pr) == 1) snprintf(sid_out, 128, "%s", PQgetvalue(pr, 0, 0));
        PQclear(pr);
      }
    }
  }

  if (uid_out[0] && user_raw[0] == '{') {
    const char *uv[2] = {uid_out, user_raw};
    PQclear(PQexecParams(
        db,
        "INSERT INTO workos.users (id, _raw, updated_at, deleted_at) VALUES ($1, $2::jsonb, now(), NULL) "
        "ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = NULL",
        2, NULL, uv, NULL, NULL, 0));
  }
  if (sid_out[0] && uid_out[0]) {
    const char *sv[3] = {sid_out, uid_out, org_out[0] ? org_out : NULL};
    PQclear(PQexecParams(
        db,
        "INSERT INTO workos.sessions (id, _raw, updated_at, deleted_at) "
        "VALUES ($1, jsonb_strip_nulls(jsonb_build_object('id', $1, 'user_id', $2, 'organization_id', $3, 'status', 'active')), now(), NULL) "
        "ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = NULL",
        3, NULL, sv, NULL, NULL, 0));
  }
}

// Writes external_id = acct back to WorkOS (best-effort, never blocks login if WorkOS hiccups).
static void workos_sync_external_id(const char *wos_uid, const char *acct) {
  if (!wos_uid || !*wos_uid || !acct || !*acct || !workos_api_key()[0]) return;
  uint8_t url_b[512], body_b[512];
  int64_t un = 0, bn = 0;
  if (!wos_c_build_update_user_ext_id(
          cs(workos_base_url()), cs(wos_uid), cs(acct),
          (coax_v_u8){url_b, (int64_t)sizeof url_b - 1}, (coax_v_u8){body_b, (int64_t)sizeof body_b - 1},
          (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1})) return;
  url_b[un] = 0; body_b[bn] = 0;
  http_client_res res;
  workos_http("PUT", (const char *)url_b, (const char *)body_b, (size_t)bn, true, &res);
  http_client_res_free(&res);
}

// Resolves/links acct_ for a WorkOS user, enforcing email_verified=true for auto-linking.
static bool resolve_workos_account(PGconn *db, const char *wos_uid, const char *raw_email, bool email_verified,
                                   const char *provider, const char *org_id,
                                   char acct_out[40], bool *made_out, bool *auto_linked_out) {
  acct_out[0] = 0; *made_out = false; *auto_linked_out = false;
  char norm_email[300] = "", all_lookups[512] = "", primary_lookup[80] = "";
  bool have_email = raw_email && *raw_email && auth_normal_email(raw_email, norm_email, sizeof norm_email);
  bool have_lookups = have_email && auth_lookups(norm_email, all_lookups, sizeof all_lookups, primary_lookup);

  bool team = false;
  if (have_email) {
    char lower[300], team_sha[65];
    size_t k = 0; for (; norm_email[k] && k + 1 < sizeof lower; k++) lower[k] = (char)tolower((unsigned char)norm_email[k]);
    lower[k] = 0;
    auth_sha_hex(lower, team_sha);
    explicit_bzero(lower, sizeof lower);
    const char *tv[1] = {team_sha};
    PGresult *t = PQexecParams(db, "SELECT EXISTS (SELECT 1 FROM analytics.team WHERE kind = 'email_sha256' AND value = $1)", 1, NULL, tv, NULL, NULL, 0);
    team = PQresultStatus(t) == PGRES_TUPLES_OK && PQntuples(t) == 1 && !strcmp(PQgetvalue(t, 0, 0), "t");
    PQclear(t);
  }

  PQclear(PQexec(db, "BEGIN"));
  const char *v[7] = {
    wos_uid,
    (email_verified && have_lookups) ? all_lookups : NULL,
    (email_verified && have_lookups) ? primary_lookup : NULL,
    email_verified ? "t" : "f",
    team ? "t" : "f",
    provider ? provider : "",
    org_id ? org_id : "",
  };
  PGresult *r = PQexecParams(
      db,
      "SELECT acct, made, auto_linked, coalesce(email_id::text, '') "
      "FROM workos.account_resolve($1, $2::text[], $3, $4::boolean, $5::boolean, $6, $7)",
      7, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    api_log("workos: account_resolve failed: %s", PQerrorMessage(db));
    PQclear(r);
    PQclear(PQexec(db, "ROLLBACK"));
    return false;
  }
  snprintf(acct_out, 40, "%s", PQgetvalue(r, 0, 0));
  *made_out = !strcmp(PQgetvalue(r, 0, 1), "t");
  *auto_linked_out = !strcmp(PQgetvalue(r, 0, 2), "t");
  char email_id[24] = "";
  snprintf(email_id, sizeof email_id, "%s", PQgetvalue(r, 0, 3));
  PQclear(r);

  if (*made_out && email_verified && have_email && email_id[0]) {
    char sealed[1200], ad[96];
    snprintf(ad, sizeof ad, "auth.emails|sealed|%s", acct_out);
    bool ok = auth_seal(ad, norm_email, sealed, sizeof sealed);
    if (ok) {
      const char *sv[2] = {email_id, sealed};
      PGresult *sr = PQexecParams(db, "SELECT auth.email_seal($1::bigint, $2)", 2, NULL, sv, NULL, NULL, 0);
      ok = PQresultStatus(sr) == PGRES_TUPLES_OK;
      PQclear(sr);
    }
    if (!ok) {
      PQclear(PQexec(db, "ROLLBACK"));
      explicit_bzero(norm_email, sizeof norm_email);
      return false;
    }
  }
  PQclear(PQexec(db, "COMMIT"));

  if (*auto_linked_out) {
    char detail[256], masked[320] = "", words[600];
    if (have_email) auth_mask_email(norm_email, masked, sizeof masked);
    snprintf(detail, sizeof detail, "{\"provider\":\"%s\",\"workos_user_id\":\"%s\"}",
             provider && *provider ? provider : "workos", wos_uid);
    auth_audit(db, acct_out, NULL, "workos.auto_link", "ok", detail);
    snprintf(words, sizeof words,
             "A %s sign-in (%s) was linked to your OpenAgents account after email verification. "
             "If this wasn't you, freeze your account immediately at %s/settings/data.",
             provider && *provider ? provider : "WorkOS", masked[0] ? masked : "verified email", auth_origin());
    auth_notice(db, acct_out, "workos.link", "new sign-in method linked", words);
  }
  explicit_bzero(norm_email, sizeof norm_email);
  workos_sync_external_id(wos_uid, acct_out);
  return true;
}

// Saves pending_authentication_token + orgs array for organization_selection_required.
static bool save_org_pending(PGconn *db, const char *err_body, const char *pending_tok, bool app,
                             char tok_out[65], char orgs_json_out[4096], char orgs_lines_out[2048]) {
  tok_out[0] = 0; orgs_json_out[0] = 0; orgs_lines_out[0] = 0;
  char id[33], tok[65], tsha[65];
  if (!auth_random_hex(id, 16) || !auth_random_hex(tok, 32)) return false;
  auth_sha_hex(tok, tsha);
  const char *v[6] = {id, tsha, pending_tok, err_body, app ? "app" : "web", NULL};
  PGresult *r = PQexecParams(
      db,
      "WITH orgs AS (SELECT coalesce(($4::jsonb)->'organizations', '[]'::jsonb) o) "
      "INSERT INTO auth.workos_pending (id, kind, token_sha256, pending_auth_token, orgs_json, client, expires_at) "
      "SELECT $1, 'org', $2, $3, o, $5, now() + interval '10 minutes' FROM orgs "
      "RETURNING orgs_json::text",
      5, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    PQclear(r);
    return false;
  }
  snprintf(orgs_json_out, 4096, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  snprintf(tok_out, 65, "%s", tok);

  // Also format org lines ("org <id> <name>\n") for ?as=lines clients.
  const char *ov[1] = {orgs_json_out};
  r = PQexecParams(
      db,
      "SELECT string_agg('org ' || coalesce(x->>'id', '') || ' ' || replace(coalesce(x->>'name', x->>'id', ''), E'\n', ' '), E'\n') "
      "FROM jsonb_array_elements($1::jsonb) x",
      1, NULL, ov, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0))
    snprintf(orgs_lines_out, 2048, "%s\n", PQgetvalue(r, 0, 0));
  PQclear(r);
  return true;
}

// Handles a WorkOS /user_management/authenticate response for JSON/lines routes (magic/verify, org/select, exchange).
static void finish_workos_auth_api(const request *q, PGconn *db, long status, const char *body, size_t body_len,
                                   bool app, const char *action, const char *clear_cookie_name) {
  if (status >= 200 && status < 300 && body && body_len > 0) {
    char wos_uid[128], email[300], org_id[128], wos_sid[128], method[64];
    bool email_verified = false;
    mirror_auth_response(db, body, wos_uid, email, &email_verified, org_id, wos_sid, method);
    if (!wos_uid[0]) { auth_refuse(q, 502, "Unexpected answer from authentication provider."); return; }
    char acct[40]; bool made = false, auto_linked = false;
    if (!resolve_workos_account(db, wos_uid, email, email_verified, method, org_id, acct, &made, &auto_linked)) {
      auth_refuse(q, 500, "That didn't work. Try again.");
      return;
    }
    char extra_json[320] = "", extra_lines[320] = "";
    if (org_id[0]) {
      char org_esc[200]; auth_json_esc(org_id, org_esc, sizeof org_esc);
      snprintf(extra_json, sizeof extra_json, ",\"organization_id\":\"%s\"", org_esc);
      snprintf(extra_lines, sizeof extra_lines, "organization_id %s\n", org_id);
    }
    (void)clear_cookie_name;
    // Always mint OpenAgents Option A1 session at L1 (preserving Passkey/Nostr L2/L2-fresh gates).
    auth_logged_in(q, db, acct, "workos", NULL, "L1", app, made, action, extra_json, extra_lines);
    return;
  }

  coax_t_WosApiError err;
  memset(&err, 0, sizeof err);
  wos_parse_api_error(status, (coax_string){body ? body : "", (int64_t)body_len}, cs(""), cs(""),
                      (coax_v_t_WosApiError){&err, 1});
  if (status == 403 && err.kind == WOS_ERR_KIND_ORG_SELECT && err.pending_n > 0) {
    char pat[512];
    int64_t pn = err.pending_n < (int64_t)sizeof pat - 1 ? err.pending_n : (int64_t)sizeof pat - 1;
    memcpy(pat, err.pending_auth_token.e, (size_t)pn);
    pat[pn] = 0;
    char ptok[65], orgs_json[4096], orgs_lines[2048];
    if (!save_org_pending(db, body, pat, app, ptok, orgs_json, orgs_lines)) {
      auth_refuse(q, 500, "That didn't work. Try again.");
      return;
    }
    char hdr[320] = "", json[5120], lines[2560];
    if (!app) {
      snprintf(hdr, sizeof hdr, "Set-Cookie: %s=%s; Path=/; Max-Age=600; HttpOnly; Secure; SameSite=Lax\r\n",
               WOS_ORG_COOKIE, ptok);
    }
    snprintf(json, sizeof json, "{\"state\":\"need-org\",\"pending\":\"%s\",\"organizations\":%s}", ptok, orgs_json);
    snprintf(lines, sizeof lines, "state need-org\npending %s\n%s", ptok, orgs_lines);
    auth_answer(q, 200, hdr[0] ? hdr : NULL, json, lines);
    return;
  }

  char msg[260] = "That code isn't right. Try again.";
  if (err.message_n > 0) {
    int64_t mn = err.message_n < (int64_t)sizeof msg - 1 ? err.message_n : (int64_t)sizeof msg - 1;
    memcpy(msg, err.message.e, (size_t)mn);
    msg[mn] = 0;
  }
  int out_status = (status == 400 || status == 401 || status == 403 || status == 422 || status == 429) ? (int)status : 502;
  if (status == 400) out_status = 401;
  auth_refuse(q, out_status, msg);
}

// ---------------------------------------------------------------- routes

static void route_config(const request *q) {
  bool on = workos_enabled_for_request(q) && workos_configured();
  char json[512], lines[512], cid_esc[200];
  auth_json_esc(on ? workos_client_id() : "", cid_esc, sizeof cid_esc);
  snprintf(json, sizeof json,
           "{\"enabled\":%s,\"client_id\":\"%s\",\"providers\":[\"magic\",\"google\",\"github\",\"apple\",\"sso\"]}",
           on ? "true" : "false", cid_esc);
  snprintf(lines, sizeof lines, "enabled %d\nclient_id %s\nproviders magic,google,github,apple,sso\n",
           on ? 1 : 0, on ? workos_client_id() : "");
  auth_answer(q, 200, NULL, json, lines);
}

static void route_magic_start(const request *q, PGconn *db, const char *body) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "WorkOS login isn't enabled here."); return; }
  char raw_email[400] = "", email[300] = "", client[16] = "web";
  if (!auth_json_str(body, q->body_len, "email", raw_email, sizeof raw_email) || !auth_normal_email(raw_email, email, sizeof email)) {
    auth_refuse(q, 400, "Type your email address.");
    return;
  }
  auth_json_str(body, q->body_len, "client", client, sizeof client);
  bool app = !strcmp(client, "app");

  // Rate limit by visitor IP (reuse existing Postgres rate limiter).
  char ipk[65]; auth_visitor_key(q, ipk);
  long long left_ms = 0;
  if (!api_limit(db, "workos_magic_ip", ipk, 30, &left_ms)) {
    auth_refuse(q, 429, "Too many codes from here. Try again in a minute.");
    return;
  }

  uint8_t url_b[512], req_b[1024];
  int64_t un = 0, bn = 0;
  if (!wos_c_build_magic_auth(cs(workos_base_url()), cs(email),
                              (coax_v_u8){url_b, (int64_t)sizeof url_b - 1},
                              (coax_v_u8){req_b, (int64_t)sizeof req_b - 1},
                              (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1})) {
    auth_refuse(q, 400, "That isn't an email address.");
    return;
  }
  url_b[un] = 0; req_b[bn] = 0;
  http_client_res res;
  workos_http("POST", (const char *)url_b, (const char *)req_b, (size_t)bn, true, &res);
  if (res.status < 200 || res.status >= 300) {
    long st = res.status;
    http_client_res_free(&res);
    auth_refuse(q, st == 429 ? 429 : 502, st == 429 ? "Too many codes for this address. Try again shortly." : "The email didn't send. Try again.");
    return;
  }
  http_client_res_free(&res);

  char id[33], pend[65], psha[65];
  if (!auth_random_hex(id, 16) || !auth_random_hex(pend, 32)) { auth_refuse(q, 503, "No randomness here. Try again."); return; }
  auth_sha_hex(pend, psha);
  const char *v[4] = {id, psha, email, app ? "app" : "web"};
  PGresult *r = PQexecParams(
      db,
      "INSERT INTO auth.workos_pending (id, kind, token_sha256, email, client, expires_at) "
      "VALUES ($1, 'magic', $2, $3, $4, now() + interval '10 minutes')",
      4, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK;
  PQclear(r);
  if (!ok) { auth_refuse(q, 500, "That didn't work. Try again."); return; }

  auth_audit(db, NULL, NULL, "workos.magic_start", "ok", NULL);
  char hdr[300] = "", json[260], lines[200];
  if (!app) {
    snprintf(hdr, sizeof hdr, "Set-Cookie: %s=%s; Path=/; Max-Age=600; HttpOnly; Secure; SameSite=Lax\r\n",
             WOS_PENDING_COOKIE, pend);
    snprintf(json, sizeof json, "{\"state\":\"sent\",\"expires_in\":600,\"tries\":5}");
    snprintf(lines, sizeof lines, "state sent\nexpires_in 600\ntries 5\n");
  } else {
    snprintf(json, sizeof json, "{\"state\":\"sent\",\"pending\":\"%s\",\"expires_in\":600,\"tries\":5}", pend);
    snprintf(lines, sizeof lines, "state sent\npending %s\nexpires_in 600\ntries 5\n", pend);
  }
  auth_answer(q, 200, hdr[0] ? hdr : NULL, json, lines);
}

static void route_magic_verify(const request *q, PGconn *db, const char *body) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "WorkOS login isn't enabled here."); return; }
  char raw_code[32] = "", code[16] = "", email[300] = "", pend[80] = "", radar_id[128] = "", client[16] = "web";
  if (!auth_json_str(body, q->body_len, "code", raw_code, sizeof raw_code)) {
    auth_refuse(q, 400, "Type the 6 digits from the email.");
    return;
  }
  size_t k = 0;
  for (const char *p = raw_code; *p && k < 12; p++) if (*p >= '0' && *p <= '9') code[k++] = *p;
  code[k] = 0;
  if (k != 6) { auth_refuse(q, 400, "Type the 6 digits from the email."); return; }

  auth_json_str(body, q->body_len, "client", client, sizeof client);
  auth_json_str(body, q->body_len, "radar_auth_attempt_id", radar_id, sizeof radar_id);
  bool app = !strcmp(client, "app");

  // Prefer pending cookie/token binding; allow explicit email for stateless native clients.
  auth_cookie(q, WOS_PENDING_COOKIE, pend, sizeof pend);
  if (!pend[0]) auth_json_str(body, q->body_len, "pending", pend, sizeof pend);
  if (strlen(pend) == 64) {
    char psha[65]; auth_sha_hex(pend, psha);
    const char *pv[1] = {psha};
    PGresult *pr = PQexecParams(
        db,
        "SELECT coalesce(email, ''), client FROM auth.workos_pending "
        "WHERE kind = 'magic' AND token_sha256 = $1 AND spent_at IS NULL AND expires_at > now()",
        1, NULL, pv, NULL, NULL, 0);
    if (PQresultStatus(pr) == PGRES_TUPLES_OK && PQntuples(pr) == 1) {
      snprintf(email, sizeof email, "%s", PQgetvalue(pr, 0, 0));
      if (!strcmp(PQgetvalue(pr, 0, 1), "app")) app = true;
    }
    PQclear(pr);
  }
  if (!email[0]) {
    char raw_e[400] = "";
    if (auth_json_str(body, q->body_len, "email", raw_e, sizeof raw_e)) auth_normal_email(raw_e, email, sizeof email);
  }
  if (!email[0]) { auth_refuse(q, 409, "Ask for a code in this browser first."); return; }

  char ip[128], ua[256];
  radar_context(q, ip, ua);
  uint8_t url_b[512], req_b[2048];
  int64_t un = 0, bn = 0;
  if (!wos_c_build_auth_magic(cs(workos_base_url()), cs(workos_client_id()), cs(workos_api_key()),
                              cs(email), cs(code), cs(ip), cs(ua), cs(radar_id),
                              (coax_v_u8){url_b, (int64_t)sizeof url_b - 1},
                              (coax_v_u8){req_b, (int64_t)sizeof req_b - 1},
                              (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1})) {
    auth_refuse(q, 500, "That didn't work. Try again.");
    return;
  }
  url_b[un] = 0; req_b[bn] = 0;
  http_client_res res;
  workos_http("POST", (const char *)url_b, (const char *)req_b, (size_t)bn, true, &res);

  if (res.status >= 200 && res.status < 300 && strlen(pend) == 64) {
    char psha[65]; auth_sha_hex(pend, psha);
    const char *pv[1] = {psha};
    PQclear(PQexecParams(db, "UPDATE auth.workos_pending SET spent_at = now() WHERE token_sha256 = $1", 1, NULL, pv, NULL, NULL, 0));
  }
  finish_workos_auth_api(q, db, res.status, res.body, res.body_len, app, "login.workos_magic", WOS_PENDING_COOKIE);
  http_client_res_free(&res);
}

static bool discover_sso_connection(PGconn *db, const char *domain, char conn_id[128], char conn_name[128], char org_id[128]) {
  (void)db;
  conn_id[0] = 0; conn_name[0] = 0; org_id[0] = 0;
  if (!domain || !*domain) return false;
  uint8_t url_b[512]; int64_t un = 0;
  if (!wos_c_build_list_connections(cs(workos_base_url()), cs(domain),
                                    (coax_v_u8){url_b, (int64_t)sizeof url_b - 1}, (coax_v_s64){&un, 1})) return false;
  url_b[un] = 0;
  http_client_res res;
  workos_http("GET", (const char *)url_b, NULL, 0, true, &res);
  bool found = false;
  if (res.status == 200 && res.body && res.body_len > 0) {
    const char *v[1] = {res.body};
    PGresult *r = PQexecParams(
        db,
        "SELECT coalesce(c->>'id', ''), coalesce(c->>'name', ''), coalesce(c->>'organization_id', '') "
        "FROM jsonb_array_elements(coalesce(($1::jsonb)->'data', '[]'::jsonb)) c "
        "WHERE coalesce(c->>'state', 'active') = 'active' LIMIT 1",
        1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && PQgetvalue(r, 0, 0)[0]) {
      snprintf(conn_id, 128, "%s", PQgetvalue(r, 0, 0));
      snprintf(conn_name, 128, "%s", PQgetvalue(r, 0, 1));
      snprintf(org_id, 128, "%s", PQgetvalue(r, 0, 2));
      found = true;
    }
    PQclear(r);
  }
  http_client_res_free(&res);
  return found;
}

static void route_device_start(const request *q, PGconn *db) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "WorkOS login isn't enabled here."); return; }
  char ipk[65]; auth_visitor_key(q, ipk);
  long long left_ms = 0;
  if (!api_limit(db, "workos_magic_ip", ipk, 30, &left_ms)) {
    auth_refuse(q, 429, "Too many login attempts from here. Try again in a minute.");
    return;
  }
  char url[512], req_b[256], cid_esc[200];
  auth_json_esc(workos_client_id(), cid_esc, sizeof cid_esc);
  snprintf(url, sizeof url, "%s/user_management/authorize/device", workos_base_url());
  int bn = snprintf(req_b, sizeof req_b, "{\"client_id\":\"%s\"}", cid_esc);
  http_client_res res;
  workos_http("POST", url, req_b, (size_t)bn, true, &res);
  if (res.status < 200 || res.status >= 300 || !res.body) {
    long st = res.status;
    http_client_res_free(&res);
    auth_refuse(q, st == 429 ? 429 : 502, "Could not start WorkOS device authorization.");
    return;
  }
  auth_audit(db, NULL, NULL, "workos.device_start", "ok", NULL);
  auth_answer(q, 200, NULL, res.body, NULL);
  http_client_res_free(&res);
}

static void route_sso_discover(const request *q, PGconn *db, const char *body) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "WorkOS login isn't enabled here."); return; }
  char email[300] = "", domain[200] = "";
  if (auth_json_str(body, q->body_len, "domain", domain, sizeof domain) && domain[0]) {
    // domain provided directly
  } else if (auth_json_str(body, q->body_len, "email", email, sizeof email)) {
    const char *at = strchr(email, '@');
    if (at && at[1]) snprintf(domain, sizeof domain, "%s", at + 1);
  }
  for (char *p = domain; *p; p++) *p = (char)tolower((unsigned char)*p);
  char cid[128] = "", cname[128] = "", oid[128] = "";
  bool found = domain[0] && discover_sso_connection(db, domain, cid, cname, oid);
  char json[640], lines[512], cid_e[180], cname_e[180], oid_e[180];
  auth_json_esc(cid, cid_e, sizeof cid_e);
  auth_json_esc(cname, cname_e, sizeof cname_e);
  auth_json_esc(oid, oid_e, sizeof oid_e);
  snprintf(json, sizeof json, "{\"sso\":%s,\"connection_id\":\"%s\",\"name\":\"%s\",\"organization_id\":\"%s\"}",
           found ? "true" : "false", cid_e, cname_e, oid_e);
  snprintf(lines, sizeof lines, "sso %d\nconnection_id %s\nname %s\norganization_id %s\n",
           found ? 1 : 0, cid, cname, oid);
  auth_answer(q, 200, NULL, json, lines);
}

static void route_oauth_start(const request *q, PGconn *db) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "WorkOS login isn't enabled here."); return; }
  char provider[64] = "", domain[200] = "", conn_id[128] = "", org_id[128] = "", login_hint[256] = "", next_path[256] = "/", mode[16] = "";
  api_query(q, "provider", provider, sizeof provider);
  api_query(q, "domain", domain, sizeof domain);
  api_query(q, "connection_id", conn_id, sizeof conn_id);
  api_query(q, "organization_id", org_id, sizeof org_id);
  api_query(q, "login_hint", login_hint, sizeof login_hint);
  api_query(q, "mode", mode, sizeof mode);
  char raw_next[256] = "";
  if (api_query(q, "next", raw_next, sizeof raw_next) && same_site_next(raw_next)) snprintf(next_path, sizeof next_path, "%s", raw_next);

  // Normalize friendly provider aliases.
  if (!strcasecmp(provider, "google")) snprintf(provider, sizeof provider, "GoogleOAuth");
  else if (!strcasecmp(provider, "github")) snprintf(provider, sizeof provider, "GitHubOAuth");
  else if (!strcasecmp(provider, "apple")) snprintf(provider, sizeof provider, "AppleOAuth");
  else if (!strcasecmp(provider, "sso")) provider[0] = 0;

  // Reject any password or phone provider attempts explicitly.
  if (provider[0] && strcmp(provider, "GoogleOAuth") && strcmp(provider, "GitHubOAuth") && strcmp(provider, "AppleOAuth")) {
    auth_refuse(q, 400, "Unsupported sign-in provider.");
    return;
  }
  if (!provider[0] && !conn_id[0] && domain[0]) {
    char cname[128] = "", dorg[128] = "";
    if (!discover_sso_connection(db, domain, conn_id, cname, dorg)) {
      auth_refuse(q, 404, "No Enterprise SSO connection found for that domain.");
      return;
    }
    if (!org_id[0] && dorg[0]) snprintf(org_id, sizeof org_id, "%s", dorg);
  }
  if (!provider[0] && !conn_id[0] && !org_id[0]) {
    auth_refuse(q, 400, "Choose Google, GitHub, Apple, or enter your company domain for SSO.");
    return;
  }

  unsigned char ver_ent[32], st_ent[32];
  uint8_t verifier[128], challenge[64], state[64];
  if (!auth_random(ver_ent, sizeof ver_ent) || !auth_random(st_ent, sizeof st_ent)) {
    auth_refuse(q, 503, "No randomness here. Try again.");
    return;
  }
  int64_t vn = wos_pkce_verifier_from_entropy((coax_v_u8){ver_ent, 32}, (coax_v_u8){verifier, 127});
  int64_t sn = wos_state_from_entropy((coax_v_u8){st_ent, 32}, (coax_v_u8){state, 63});
  explicit_bzero(ver_ent, sizeof ver_ent);
  explicit_bzero(st_ent, sizeof st_ent);
  if (vn <= 0 || sn <= 0) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
  verifier[vn] = 0; state[sn] = 0;
  int64_t cn = wos_pkce_challenge((coax_string){(const char *)verifier, vn}, (coax_v_u8){challenge, 63});
  if (cn <= 0) { explicit_bzero(verifier, sizeof verifier); auth_refuse(q, 500, "That didn't work. Try again."); return; }
  challenge[cn] = 0;

  char redirect_uri[512];
  snprintf(redirect_uri, sizeof redirect_uri, "%s/api/auth/workos/callback", auth_origin());
  coax_t_WosAuthUrlParams params;
  memset(&params, 0, sizeof params);
  params.client_id = cs(workos_client_id());
  params.redirect_uri = cs(redirect_uri);
  params.provider = cs(provider);
  params.connection_id = cs(conn_id);
  params.organization_id = cs(org_id);
  params.domain_hint = cs(domain);
  params.login_hint = cs(login_hint);
  params.state = (coax_string){(const char *)state, sn};
  params.code_challenge = (coax_string){(const char *)challenge, cn};
  params.code_challenge_method = cs("S256");

  uint8_t url_b[2048];
  int64_t un = wos_authorize_url(cs(workos_base_url()), params, (coax_v_u8){url_b, (int64_t)sizeof url_b - 1});
  if (un <= 0) { explicit_bzero(verifier, sizeof verifier); auth_refuse(q, 500, "That didn't work. Try again."); return; }
  url_b[un] = 0;

  char id[33], ssha[65];
  if (!auth_random_hex(id, 16)) { explicit_bzero(verifier, sizeof verifier); auth_refuse(q, 503, "No randomness here. Try again."); return; }
  auth_sha_hex((const char *)state, ssha);
  const char *v[4] = {id, ssha, (const char *)verifier, next_path};
  PGresult *r = PQexecParams(
      db,
      "INSERT INTO auth.workos_pending (id, kind, token_sha256, verifier, next_path, client, expires_at) "
      "VALUES ($1, 'oauth', $2, $3, $4, 'web', now() + interval '10 minutes')",
      4, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK;
  PQclear(r);
  explicit_bzero(verifier, sizeof verifier);
  if (!ok) { auth_refuse(q, 500, "That didn't work. Try again."); return; }

  char hdr[2600];
  if (!strcmp(mode, "json") || strstr(q->query, "as=lines")) {
    snprintf(hdr, sizeof hdr, "Set-Cookie: %s=%s; Path=/; Max-Age=600; HttpOnly; Secure; SameSite=Lax\r\n",
             WOS_STATE_COOKIE, (const char *)state);
    char json[2300], lines[2200], uesc[2200];
    auth_json_esc((const char *)url_b, uesc, sizeof uesc);
    snprintf(json, sizeof json, "{\"url\":\"%s\"}", uesc);
    snprintf(lines, sizeof lines, "url %s\n", (const char *)url_b);
    auth_answer(q, 200, hdr, json, lines);
    return;
  }
  snprintf(hdr, sizeof hdr,
           "Location: %s\r\n"
           "Set-Cookie: %s=%s; Path=/; Max-Age=600; HttpOnly; Secure; SameSite=Lax\r\n",
           (const char *)url_b, WOS_STATE_COOKIE, (const char *)state);
  api_respond(q->fd, 303, "text/plain", hdr, "", 0);
}

static void route_callback(const request *q, PGconn *db) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "WorkOS login isn't enabled here."); return; }
  char code[512] = "", state[128] = "", err_param[128] = "", cookie_state[128] = "";
  api_query(q, "code", code, sizeof code);
  api_query(q, "state", state, sizeof state);
  api_query(q, "error", err_param, sizeof err_param);
  auth_cookie(q, WOS_STATE_COOKIE, cookie_state, sizeof cookie_state);

  if (err_param[0] || !code[0] || !state[0] || !cookie_state[0] ||
      !wos_state_equal(cs(cookie_state), cs(state))) {
    char hdr[320];
    snprintf(hdr, sizeof hdr,
             "Location: /login?error=oauth_failed\r\n"
             "Set-Cookie: %s=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n",
             WOS_STATE_COOKIE);
    api_respond(q->fd, 303, "text/plain", hdr, "", 0);
    return;
  }

  char ssha[65], verifier[128] = "", next_path[256] = "/";
  auth_sha_hex(state, ssha);
  const char *v[1] = {ssha};
  PGresult *r = PQexecParams(
      db,
      "UPDATE auth.workos_pending SET spent_at = now() "
      "WHERE kind = 'oauth' AND token_sha256 = $1 AND spent_at IS NULL AND expires_at > now() "
      "RETURNING coalesce(verifier, ''), coalesce(next_path, '/')",
      1, NULL, v, NULL, NULL, 0);
  bool found = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  if (found) {
    snprintf(verifier, sizeof verifier, "%s", PQgetvalue(r, 0, 0));
    if (same_site_next(PQgetvalue(r, 0, 1))) snprintf(next_path, sizeof next_path, "%s", PQgetvalue(r, 0, 1));
  }
  PQclear(r);
  if (!found || !verifier[0]) {
    auth_refuse(q, 401, "That sign-in expired. Start again.");
    return;
  }

  char ip[128], ua[256];
  radar_context(q, ip, ua);
  uint8_t url_b[512], req_b[2048];
  int64_t un = 0, bn = 0;
  bool built = wos_c_build_auth_code(cs(workos_base_url()), cs(workos_client_id()), cs(workos_api_key()),
                                     cs(code), cs(verifier), cs(ip), cs(ua),
                                     (coax_v_u8){url_b, (int64_t)sizeof url_b - 1},
                                     (coax_v_u8){req_b, (int64_t)sizeof req_b - 1},
                                     (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1});
  explicit_bzero(verifier, sizeof verifier);
  if (!built) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
  url_b[un] = 0; req_b[bn] = 0;
  http_client_res res;
  workos_http("POST", (const char *)url_b, (const char *)req_b, (size_t)bn, true, &res);

  if (res.status >= 200 && res.status < 300 && res.body && res.body_len > 0) {
    char wos_uid[128], email[300], org_id[128], wos_sid[128], method[64];
    bool email_verified = false;
    mirror_auth_response(db, res.body, wos_uid, email, &email_verified, org_id, wos_sid, method);
    http_client_res_free(&res);
    char acct[40]; bool made = false, auto_linked = false;
    if (!wos_uid[0] || !resolve_workos_account(db, wos_uid, email, email_verified, method, org_id, acct, &made, &auto_linked)) {
      auth_refuse(q, 500, "That didn't work. Try again.");
      return;
    }
    auth_login_web_redirect(q, db, acct, "workos", "L1", made, "login.workos_oauth", next_path);
    return;
  }

  coax_t_WosApiError err;
  memset(&err, 0, sizeof err);
  wos_parse_api_error(res.status, (coax_string){res.body ? res.body : "", (int64_t)res.body_len}, cs(""), cs(""),
                      (coax_v_t_WosApiError){&err, 1});
  if (res.status == 403 && err.kind == WOS_ERR_KIND_ORG_SELECT && err.pending_n > 0) {
    char pat[512];
    int64_t pn = err.pending_n < (int64_t)sizeof pat - 1 ? err.pending_n : (int64_t)sizeof pat - 1;
    memcpy(pat, err.pending_auth_token.e, (size_t)pn);
    pat[pn] = 0;
    char ptok[65], orgs_json[4096], orgs_lines[2048];
    bool saved = save_org_pending(db, res.body, pat, false, ptok, orgs_json, orgs_lines);
    http_client_res_free(&res);
    if (!saved) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
    char hdr[600];
    snprintf(hdr, sizeof hdr,
             "Location: /login#wos_org=%s\r\n"
             "Set-Cookie: %s=%s; Path=/; Max-Age=600; HttpOnly; Secure; SameSite=Lax\r\n"
             "Set-Cookie: %s=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n",
             ptok, WOS_ORG_COOKIE, ptok, WOS_STATE_COOKIE);
    api_respond(q->fd, 303, "text/plain", hdr, "", 0);
    return;
  }
  http_client_res_free(&res);
  char hdr[320];
  snprintf(hdr, sizeof hdr,
           "Location: /login?error=oauth_exchange_failed\r\n"
           "Set-Cookie: %s=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n",
           WOS_STATE_COOKIE);
  api_respond(q->fd, 303, "text/plain", hdr, "", 0);
}

static void route_org_pending(const request *q, PGconn *db) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "WorkOS login isn't enabled here."); return; }
  char ptok[80] = "";
  auth_cookie(q, WOS_ORG_COOKIE, ptok, sizeof ptok);
  if (!ptok[0]) api_query(q, "pending", ptok, sizeof ptok);
  if (strlen(ptok) != 64) { auth_refuse(q, 404, "No pending organization selection."); return; }
  char psha[65]; auth_sha_hex(ptok, psha);
  const char *v[1] = {psha};
  PGresult *r = PQexecParams(
      db,
      "SELECT coalesce(orgs_json::text, '[]'), "
      "       coalesce((SELECT string_agg('org ' || coalesce(x->>'id','') || ' ' || replace(coalesce(x->>'name', x->>'id',''), E'\n',' '), E'\n') "
      "                 FROM jsonb_array_elements(coalesce(orgs_json, '[]'::jsonb)) x), '') "
      "FROM auth.workos_pending "
      "WHERE kind = 'org' AND token_sha256 = $1 AND spent_at IS NULL AND expires_at > now()",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    PQclear(r);
    auth_refuse(q, 410, "That sign-in expired. Start again.");
    return;
  }
  char json[4600], lines[2400];
  snprintf(json, sizeof json, "{\"state\":\"need-org\",\"organizations\":%s}", PQgetvalue(r, 0, 0));
  snprintf(lines, sizeof lines, "state need-org\n%s\n", PQgetvalue(r, 0, 1));
  PQclear(r);
  auth_answer(q, 200, NULL, json, lines);
}

static void route_org_select(const request *q, PGconn *db, const char *body) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "WorkOS login isn't enabled here."); return; }
  char org_id[128] = "", ptok[80] = "", client[16] = "web";
  if (!auth_json_str(body, q->body_len, "organization_id", org_id, sizeof org_id) || !org_id[0]) {
    auth_refuse(q, 400, "Choose an organization.");
    return;
  }
  auth_json_str(body, q->body_len, "client", client, sizeof client);
  bool app = !strcmp(client, "app");
  auth_cookie(q, WOS_ORG_COOKIE, ptok, sizeof ptok);
  if (!ptok[0]) auth_json_str(body, q->body_len, "pending", ptok, sizeof ptok);
  if (strlen(ptok) != 64) { auth_refuse(q, 409, "Start your sign-in again."); return; }

  char psha[65], pat[512] = "";
  auth_sha_hex(ptok, psha);
  const char *v[1] = {psha};
  PGresult *r = PQexecParams(
      db,
      "SELECT coalesce(pending_auth_token, ''), client FROM auth.workos_pending "
      "WHERE kind = 'org' AND token_sha256 = $1 AND spent_at IS NULL AND expires_at > now()",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    snprintf(pat, sizeof pat, "%s", PQgetvalue(r, 0, 0));
    if (!strcmp(PQgetvalue(r, 0, 1), "app")) app = true;
  }
  PQclear(r);
  if (!pat[0]) { auth_refuse(q, 410, "That sign-in expired. Start again."); return; }

  char ip[128], ua[256];
  radar_context(q, ip, ua);
  uint8_t url_b[512], req_b[2048];
  int64_t un = 0, bn = 0;
  if (!wos_c_build_auth_org_select(cs(workos_base_url()), cs(workos_client_id()), cs(workos_api_key()),
                                   cs(pat), cs(org_id), cs(ip), cs(ua),
                                   (coax_v_u8){url_b, (int64_t)sizeof url_b - 1},
                                   (coax_v_u8){req_b, (int64_t)sizeof req_b - 1},
                                   (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1})) {
    auth_refuse(q, 500, "That didn't work. Try again.");
    return;
  }
  url_b[un] = 0; req_b[bn] = 0;
  http_client_res res;
  workos_http("POST", (const char *)url_b, (const char *)req_b, (size_t)bn, true, &res);
  if (res.status >= 200 && res.status < 300) {
    PQclear(PQexecParams(db, "UPDATE auth.workos_pending SET spent_at = now() WHERE token_sha256 = $1", 1, NULL, v, NULL, NULL, 0));
  }
  finish_workos_auth_api(q, db, res.status, res.body, res.body_len, app, "login.workos_org", WOS_ORG_COOKIE);
  http_client_res_free(&res);
}

// Native PKCE exchange (iOS/macOS/Android) and CLI WorkOS JWT handoff -> Option A1 oa_at_/oa_rt_.
static void route_exchange(const request *q, PGconn *db, const char *body) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "WorkOS login isn't enabled here."); return; }
  char code[512] = "", verifier[256] = "", access_token[4096] = "";
  auth_json_str(body, q->body_len, "code", code, sizeof code);
  auth_json_str(body, q->body_len, "code_verifier", verifier, sizeof verifier);
  auth_json_str(body, q->body_len, "access_token", access_token, sizeof access_token);

  if (code[0]) {
    // Native PKCE (code + code_verifier) exchanged confidentially by oa-api with WorkOS.
    if (verifier[0] && !wos_pkce_verifier_ok(cs(verifier))) {
      auth_refuse(q, 400, "Invalid PKCE code_verifier.");
      return;
    }
    char ip[128], ua[256];
    radar_context(q, ip, ua);
    uint8_t url_b[512], req_b[2048];
    int64_t un = 0, bn = 0;
    if (!wos_c_build_auth_code(cs(workos_base_url()), cs(workos_client_id()), cs(workos_api_key()),
                               cs(code), cs(verifier), cs(ip), cs(ua),
                               (coax_v_u8){url_b, (int64_t)sizeof url_b - 1},
                               (coax_v_u8){req_b, (int64_t)sizeof req_b - 1},
                               (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1})) {
      auth_refuse(q, 400, "Invalid exchange request.");
      return;
    }
    url_b[un] = 0; req_b[bn] = 0;
    http_client_res res;
    workos_http("POST", (const char *)url_b, (const char *)req_b, (size_t)bn, true, &res);
    finish_workos_auth_api(q, db, res.status, res.body, res.body_len, true, "login.workos_pkce", NULL);
    http_client_res_free(&res);
    return;
  }

  if (access_token[0]) {
    // CLI RFC 8628 device flow handoff: verify WorkOS RS256 access_token against live JWKS via jwt.coax!
    uint8_t jwks_url[512];
    int64_t jun = wos_jwks_url(cs(workos_base_url()), cs(workos_client_id()), (coax_v_u8){jwks_url, (int64_t)sizeof jwks_url - 1});
    if (jun <= 0) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
    jwks_url[jun] = 0;
    http_client_res jres;
    workos_http("GET", (const char *)jwks_url, NULL, 0, false, &jres);
    if (jres.status != 200 || !jres.body) {
      http_client_res_free(&jres);
      auth_refuse(q, 502, "Could not verify WorkOS session token.");
      return;
    }
    coax_t_WosJwtClaims claims;
    memset(&claims, 0, sizeof claims);
    uint32_t jerr = wos_jwt_verify_session(cs(access_token), (coax_string){jres.body, (int64_t)jres.body_len},
                                           cs(""), (int64_t)time(NULL), (coax_v_t_WosJwtClaims){&claims, 1});
    http_client_res_free(&jres);
    if (jerr != WOS_JWT_OK || claims.sub_n <= 0) {
      auth_refuse(q, 401, "That WorkOS token isn't valid.");
      return;
    }
    char wos_uid[128] = "", org_id[128] = "", wos_sid[128] = "";
    int64_t sn = claims.sub_n < (int64_t)sizeof wos_uid - 1 ? claims.sub_n : (int64_t)sizeof wos_uid - 1;
    memcpy(wos_uid, claims.sub.e, (size_t)sn); wos_uid[sn] = 0;
    if (claims.org_id_n > 0) {
      int64_t on = claims.org_id_n < (int64_t)sizeof org_id - 1 ? claims.org_id_n : (int64_t)sizeof org_id - 1;
      memcpy(org_id, claims.org_id.e, (size_t)on); org_id[on] = 0;
    }
    if (claims.sid_n > 0) {
      int64_t kn = claims.sid_n < (int64_t)sizeof wos_sid - 1 ? claims.sid_n : (int64_t)sizeof wos_sid - 1;
      memcpy(wos_sid, claims.sid.e, (size_t)kn); wos_sid[kn] = 0;
    }
    // Fetch user profile from WorkOS to read authoritative email + email_verified.
    char uurl[640];
    if (!wos_path_segment_ok(cs(wos_uid))) { auth_refuse(q, 400, "Invalid user id."); return; }
    snprintf(uurl, sizeof uurl, "%s/user_management/users/%s", workos_base_url(), wos_uid);
    http_client_res ures;
    workos_http("GET", uurl, NULL, 0, true, &ures);
    char email[300] = ""; bool email_verified = false;
    if (ures.status == 200 && ures.body) {
      const char *uv[1] = {ures.body};
      PGresult *ur = PQexecParams(db, "SELECT coalesce(($1::jsonb)->>'email',''), coalesce((($1::jsonb)->>'email_verified')::boolean, false)",
                                  1, NULL, uv, NULL, NULL, 0);
      if (PQresultStatus(ur) == PGRES_TUPLES_OK && PQntuples(ur) == 1) {
        snprintf(email, sizeof email, "%s", PQgetvalue(ur, 0, 0));
        email_verified = !strcmp(PQgetvalue(ur, 0, 1), "t");
      }
      PQclear(ur);
      const char *mv[2] = {wos_uid, ures.body};
      PQclear(PQexecParams(db, "INSERT INTO workos.users (id, _raw, updated_at, deleted_at) VALUES ($1, $2::jsonb, now(), NULL) "
                               "ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = NULL",
                           2, NULL, mv, NULL, NULL, 0));
    }
    http_client_res_free(&ures);
    if (wos_sid[0]) {
      const char *sv[3] = {wos_sid, wos_uid, org_id[0] ? org_id : NULL};
      PQclear(PQexecParams(db, "INSERT INTO workos.sessions (id, _raw, updated_at, deleted_at) "
                               "VALUES ($1, jsonb_strip_nulls(jsonb_build_object('id',$1,'user_id',$2,'organization_id',$3,'status','active')), now(), NULL) "
                               "ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = NULL",
                           3, NULL, sv, NULL, NULL, 0));
    }
    char acct[40]; bool made = false, auto_linked = false;
    if (!resolve_workos_account(db, wos_uid, email, email_verified, "DeviceCode", org_id, acct, &made, &auto_linked)) {
      auth_refuse(q, 500, "That didn't work. Try again.");
      return;
    }
    auth_logged_in(q, db, acct, "workos", NULL, "L1", true, made, "login.workos_device", NULL, NULL);
    return;
  }

  auth_refuse(q, 400, "Provide a PKCE code or WorkOS access_token.");
}

static void route_orgs_list(const request *q, PGconn *db) {
  const auth_principal *p = auth_principal_of(q);
  if (!p || !p->account[0]) { auth_refuse(q, 401, "Log in first."); return; }
  const char *v[1] = {p->account};
  PGresult *r = PQexecParams(
      db,
      "SELECT coalesce((SELECT last_organization_id FROM auth.workos_identities WHERE account = $1 ORDER BY last_seen_at DESC LIMIT 1), ''), "
      "       coalesce((SELECT jsonb_agg(jsonb_build_object('id', o.id, 'name', coalesce(o.name, o.id), 'status', m.status) ORDER BY o.name NULLS LAST, o.id) "
      "                   FROM auth.workos_identities wi "
      "                   JOIN workos.organization_memberships_live m ON m.user_id = wi.workos_user_id AND coalesce(m.status, 'active') = 'active' "
      "                   JOIN workos.organizations_live o ON o.id = m.organization_id "
      "                  WHERE wi.account = $1), '[]'::jsonb)::text, "
      "       coalesce((SELECT string_agg('org ' || o.id || ' ' || replace(coalesce(o.name, o.id), E'\n', ' '), E'\n' ORDER BY o.name NULLS LAST, o.id) "
      "                   FROM auth.workos_identities wi "
      "                   JOIN workos.organization_memberships_live m ON m.user_id = wi.workos_user_id AND coalesce(m.status, 'active') = 'active' "
      "                   JOIN workos.organizations_live o ON o.id = m.organization_id "
      "                  WHERE wi.account = $1), '')",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    PQclear(r);
    auth_refuse(q, 500, "That didn't work. Try again.");
    return;
  }
  char json[8192], lines[4096], cur_esc[200];
  auth_json_esc(PQgetvalue(r, 0, 0), cur_esc, sizeof cur_esc);
  snprintf(json, sizeof json, "{\"active_organization_id\":\"%s\",\"organizations\":%s}", cur_esc, PQgetvalue(r, 0, 1));
  snprintf(lines, sizeof lines, "active_organization_id %s\n%s%s",
           PQgetvalue(r, 0, 0), PQgetvalue(r, 0, 2), PQgetvalue(r, 0, 2)[0] ? "\n" : "");
  PQclear(r);
  auth_answer(q, 200, NULL, json, lines);
}

static void route_orgs_switch(const request *q, PGconn *db, const char *body) {
  const auth_principal *p = auth_principal_of(q);
  if (!p || !p->account[0]) { auth_refuse(q, 401, "Log in first."); return; }
  char org_id[128] = "";
  auth_json_str(body, q->body_len, "organization_id", org_id, sizeof org_id);
  if (org_id[0]) {
    const char *cv[2] = {p->account, org_id};
    PGresult *cr = PQexecParams(
        db,
        "SELECT 1 FROM auth.workos_identities wi "
        "JOIN workos.organization_memberships_live m ON m.user_id = wi.workos_user_id "
        "WHERE wi.account = $1 AND m.organization_id = $2 AND coalesce(m.status, 'active') = 'active' LIMIT 1",
        2, NULL, cv, NULL, NULL, 0);
    bool ok = PQresultStatus(cr) == PGRES_TUPLES_OK && PQntuples(cr) == 1;
    PQclear(cr);
    if (!ok) { auth_refuse(q, 403, "You aren't a member of that organization."); return; }
  }
  const char *uv[2] = {p->account, org_id[0] ? org_id : NULL};
  PQclear(PQexecParams(db, "UPDATE auth.workos_identities SET last_organization_id = $2, last_seen_at = now() WHERE account = $1",
                       2, NULL, uv, NULL, NULL, 0));
  char json[256], lines[200], oesc[180];
  auth_json_esc(org_id, oesc, sizeof oesc);
  snprintf(json, sizeof json, "{\"state\":\"switched\",\"active_organization_id\":\"%s\"}", oesc);
  snprintf(lines, sizeof lines, "state switched\nactive_organization_id %s\n", org_id);
  auth_answer(q, 200, NULL, json, lines);
}

// Applies local session revocation when WorkOS notifies session.revoked or user.deleted.
static void apply_event_side_effects(PGconn *db, const char *ev_type, const char *ev_json) {
  if (strcmp(ev_type, "session.revoked") && strcmp(ev_type, "user.deleted")) return;
  const char *v[1] = {ev_json};
  PGresult *r = PQexecParams(
      db,
      "WITH d AS (SELECT ($1::jsonb)->'data' x) "
      "SELECT wi.account, coalesce(x->>'user_id', x->>'id', '') "
      "FROM d JOIN auth.workos_identities wi ON wi.workos_user_id = coalesce(x->>'user_id', x->>'id')",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK) {
    for (int i = 0; i < PQntuples(r); i++) {
      const char *acct = PQgetvalue(r, i, 0);
      const char *av[1] = {acct};
      PQclear(PQexecParams(db, "SELECT auth.sessions_end_all($1)", 1, NULL, av, NULL, NULL, 0));
      auth_audit(db, acct, NULL, !strcmp(ev_type, "user.deleted") ? "workos.user_deleted" : "workos.session_revoked", "ok", NULL);
    }
  }
  PQclear(r);
}

static void route_webhook(const request *q, PGconn *db, const char *body) {
  const char *secret = workos_webhook_secret();
  if (!secret || !*secret) { auth_refuse(q, 503, "WorkOS webhook secret isn't set up."); return; }
  char sig[512] = "";
  if (!api_header(q, "WorkOS-Signature", sig, sizeof sig) || !sig[0]) {
    auth_refuse(q, 401, "Missing WorkOS-Signature.");
    return;
  }
  uint32_t verr = wos_sig_verify(cs(secret), cs(sig), (coax_string){body, (int64_t)q->body_len},
                                 (int64_t)api_now_ms(), 180000);
  if (verr != WOS_SIG_OK) {
    auth_refuse(q, 401, "Invalid WorkOS webhook signature.");
    return;
  }
  const char *v[1] = {body};
  PGresult *r = PQexecParams(
      db,
      "SELECT coalesce(($1::jsonb)->>'id', ''), coalesce(($1::jsonb)->>'event', ''), "
      "       workos.upsert_event(coalesce(($1::jsonb)->>'id', ''), coalesce(($1::jsonb)->>'event', ''), $1::jsonb)",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || !PQgetvalue(r, 0, 0)[0]) {
    PQclear(r);
    auth_refuse(q, 400, "Malformed webhook event.");
    return;
  }
  char ev_type[128];
  snprintf(ev_type, sizeof ev_type, "%s", PQgetvalue(r, 0, 1));
  bool applied = !strcmp(PQgetvalue(r, 0, 2), "t");
  PQclear(r);
  if (applied) apply_event_side_effects(db, ev_type, body);
  api_json(q->fd, 200, applied ? "{\"ok\":true,\"applied\":true}" : "{\"ok\":true,\"applied\":false}");
}

static void route_action(const request *q, PGconn *db, const char *body) {
  const char *secret = workos_action_secret();
  if (!secret || !*secret) { auth_refuse(q, 503, "WorkOS Actions secret isn't set up."); return; }
  char sig[512] = "";
  if (!api_header(q, "WorkOS-Signature", sig, sizeof sig) || !sig[0]) {
    auth_refuse(q, 401, "Missing WorkOS-Signature.");
    return;
  }
  int64_t now_ms = (int64_t)api_now_ms();
  uint32_t verr = wos_sig_verify(cs(secret), cs(sig), (coax_string){body, (int64_t)q->body_len}, now_ms, 30000);
  if (verr != WOS_SIG_OK) {
    auth_refuse(q, 401, "Invalid WorkOS Action signature.");
    return;
  }

  const char *v[1] = {body};
  PGresult *r = PQexecParams(
      db,
      "WITH d AS (SELECT $1::jsonb j) "
      "SELECT coalesce(j->>'object', ''), "
      "       coalesce(j->'user'->>'id', ''), "
      "       coalesce(j->'user'->>'email', j->'user_data'->>'email', '') "
      "FROM d",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    PQclear(r);
    auth_refuse(q, 400, "Malformed Action payload.");
    return;
  }
  char obj[64], wos_uid[128], raw_email[300];
  snprintf(obj, sizeof obj, "%s", PQgetvalue(r, 0, 0));
  snprintf(wos_uid, sizeof wos_uid, "%s", PQgetvalue(r, 0, 1));
  snprintf(raw_email, sizeof raw_email, "%s", PQgetvalue(r, 0, 2));
  PQclear(r);

  int64_t kind = !strcmp(obj, "user_registration_action_context") ? WOS_ACTION_REG : WOS_ACTION_AUTH;
  bool allow = true;
  const char *deny_msg = "";

  // 1. Block frozen or deleted OpenAgents accounts (checked by workos_user_id AND by email lookup HMAC).
  if (wos_uid[0]) {
    const char *uv[1] = {wos_uid};
    r = PQexecParams(
        db,
        "SELECT a.frozen_at IS NOT NULL, a.deleted_at IS NOT NULL "
        "FROM auth.workos_identities wi JOIN auth.accounts a ON a.id = wi.account "
        "WHERE wi.workos_user_id = $1 LIMIT 1",
        1, NULL, uv, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
      if (!strcmp(PQgetvalue(r, 0, 1), "t")) { allow = false; deny_msg = "This account was deleted."; }
      else if (!strcmp(PQgetvalue(r, 0, 0), "t")) { allow = false; deny_msg = "This account is frozen. Unfreeze it at openagents.com/settings/data."; }
    }
    PQclear(r);
  }
  if (allow && raw_email[0]) {
    char norm[300], all[512], newest[80];
    if (auth_normal_email(raw_email, norm, sizeof norm) && auth_lookups(norm, all, sizeof all, newest)) {
      const char *ev[1] = {all};
      r = PQexecParams(
          db,
          "SELECT a.frozen_at IS NOT NULL, a.deleted_at IS NOT NULL "
          "FROM auth.emails e JOIN auth.accounts a ON a.id = e.account "
          "WHERE e.lookup = ANY($1::text[]) LIMIT 1",
          1, NULL, ev, NULL, NULL, 0);
      if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
        if (!strcmp(PQgetvalue(r, 0, 1), "t")) { allow = false; deny_msg = "This account was deleted."; }
        else if (!strcmp(PQgetvalue(r, 0, 0), "t")) { allow = false; deny_msg = "This account is frozen. Unfreeze it at openagents.com/settings/data."; }
      }
      PQclear(r);
      explicit_bzero(norm, sizeof norm);
    }
  }
  // 2. Optional invite-only registration gate (OA_WORKOS_INVITE_ONLY=1).
  if (allow && kind == WOS_ACTION_REG) {
    const char *inv = getenv("OA_WORKOS_INVITE_ONLY");
    if (inv && !strcmp(inv, "1")) {
      const char *allow_dom = getenv("OA_WORKOS_INVITE_DOMAINS");
      bool permitted = false;
      const char *at = strchr(raw_email, '@');
      if (allow_dom && *allow_dom && at && at[1]) {
        char dom[200]; snprintf(dom, sizeof dom, "%s", at + 1);
        for (char *p = dom; *p; p++) *p = (char)tolower((unsigned char)*p);
        const char *p = allow_dom;
        while (*p) {
          const char *comma = strchr(p, ',');
          size_t len = comma ? (size_t)(comma - p) : strlen(p);
          if (len == strlen(dom) && !strncasecmp(p, dom, len)) { permitted = true; break; }
          p = comma ? comma + 1 : p + len;
        }
      }
      if (!permitted) { allow = false; deny_msg = "Registration currently requires an invitation."; }
    }
  }

  uint8_t out[1024];
  int64_t on = wos_action_sign_response(kind, allow, cs(deny_msg), now_ms, cs(secret),
                                        (coax_v_u8){out, (int64_t)sizeof out - 1});
  if (on <= 0) { auth_refuse(q, 500, "Could not sign Action response."); return; }
  out[on] = 0;
  api_json(q->fd, 200, (const char *)out);
}

// ---------------------------------------------------------------- Events API mirror & lifecycle hooks

char *workos_poll_events(PGconn *db, const char *day) {
  (void)day;
  if (!workos_flag_on() || !workos_configured()) {
    return strdup("{\"enabled\":false,\"fetched\":0,\"applied\":0}");
  }
  char after[128] = "";
  PGresult *cr = PQexec(db, "SELECT coalesce(after_event_id, '') FROM workos.sync_cursor WHERE stream = 'events'");
  if (PQresultStatus(cr) == PGRES_TUPLES_OK && PQntuples(cr) == 1) snprintf(after, sizeof after, "%s", PQgetvalue(cr, 0, 0));
  PQclear(cr);

  const char *events_csv =
      "user.created,user.updated,user.deleted,"
      "organization.created,organization.updated,organization.deleted,"
      "organization_membership.created,organization_membership.updated,organization_membership.deleted,"
      "session.created,session.revoked";
  uint8_t url_b[2048]; int64_t un = 0;
  if (!wos_c_build_list_events(cs(workos_base_url()), cs(events_csv), cs(after), 100,
                               (coax_v_u8){url_b, (int64_t)sizeof url_b - 1}, (coax_v_s64){&un, 1})) {
    return NULL;
  }
  url_b[un] = 0;
  http_client_res res;
  workos_http("GET", (const char *)url_b, NULL, 0, true, &res);
  if (res.status != 200 || !res.body) {
    api_log("workos: events poll HTTP %ld", res.status);
    http_client_res_free(&res);
    return NULL;
  }

  const char *v[1] = {res.body};
  PGresult *er = PQexecParams(
      db,
      "SELECT coalesce(e->>'id', ''), coalesce(e->>'event', ''), e::text "
      "FROM jsonb_array_elements(coalesce(($1::jsonb)->'data', '[]'::jsonb)) e",
      1, NULL, v, NULL, NULL, 0);
  int fetched = 0, applied = 0;
  char last_id[128] = "";
  if (PQresultStatus(er) == PGRES_TUPLES_OK) {
    fetched = PQntuples(er);
    for (int i = 0; i < fetched; i++) {
      const char *eid = PQgetvalue(er, i, 0);
      const char *etype = PQgetvalue(er, i, 1);
      const char *eraw = PQgetvalue(er, i, 2);
      if (!eid[0] || !etype[0]) continue;
      snprintf(last_id, sizeof last_id, "%s", eid);
      const char *uv[3] = {eid, etype, eraw};
      PGresult *ur = PQexecParams(db, "SELECT workos.upsert_event($1, $2, $3::jsonb)", 3, NULL, uv, NULL, NULL, 0);
      bool did = PQresultStatus(ur) == PGRES_TUPLES_OK && PQntuples(ur) == 1 && !strcmp(PQgetvalue(ur, 0, 0), "t");
      PQclear(ur);
      if (did) {
        applied++;
        apply_event_side_effects(db, etype, eraw);
      }
    }
  }
  PQclear(er);
  http_client_res_free(&res);

  if (last_id[0]) {
    const char *cv[1] = {last_id};
    PQclear(PQexecParams(
        db,
        "INSERT INTO workos.sync_cursor (stream, after_event_id, updated_at) VALUES ('events', $1, now()) "
        "ON CONFLICT (stream) DO UPDATE SET after_event_id = EXCLUDED.after_event_id, updated_at = now()",
        1, NULL, cv, NULL, NULL, 0));
  }
  char *out = malloc(256);
  if (!out) return NULL;
  char cesc[180]; auth_json_esc(last_id[0] ? last_id : after, cesc, sizeof cesc);
  snprintf(out, 256, "{\"enabled\":true,\"fetched\":%d,\"applied\":%d,\"cursor\":\"%s\"}", fetched, applied, cesc);
  return out;
}

void workos_revoke_account_sessions(PGconn *db, const char *acct) {
  if (!db || !acct || !*acct || !workos_flag_on() || !workos_api_key()[0]) return;
  const char *v[1] = {acct};
  PGresult *r = PQexecParams(
      db,
      "SELECT s.id FROM auth.workos_identities wi "
      "JOIN workos.sessions_live s ON s.user_id = wi.workos_user_id "
      "WHERE wi.account = $1",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { PQclear(r); return; }
  for (int i = 0; i < PQntuples(r); i++) {
    const char *sid = PQgetvalue(r, i, 0);
    if (!sid || !*sid) continue;
    uint8_t url_b[512], req_b[512]; int64_t un = 0, bn = 0;
    if (!wos_c_build_revoke_session(cs(workos_base_url()), cs(sid),
                                    (coax_v_u8){url_b, (int64_t)sizeof url_b - 1},
                                    (coax_v_u8){req_b, (int64_t)sizeof req_b - 1},
                                    (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1})) continue;
    url_b[un] = 0; req_b[bn] = 0;
    http_client_res res;
    workos_http("POST", (const char *)url_b, (const char *)req_b, (size_t)bn, true, &res);
    if ((res.status >= 200 && res.status < 300) || res.status == 404) {
      const char *sv[1] = {sid};
      PQclear(PQexecParams(db, "UPDATE workos.sessions SET deleted_at = now(), updated_at = now() WHERE id = $1",
                           1, NULL, sv, NULL, NULL, 0));
    }
    http_client_res_free(&res);
  }
  PQclear(r);
}

int workos_process_pending_deletes(PGconn *db) {
  if (!db || !workos_flag_on() || !workos_api_key()[0]) return 0;
  PGresult *r = PQexec(db, "SELECT workos_user_id FROM workos.pending_deletes WHERE deleted_at IS NULL ORDER BY queued_at ASC LIMIT 50");
  if (PQresultStatus(r) != PGRES_TUPLES_OK) { PQclear(r); return 0; }
  int done = 0;
  for (int i = 0; i < PQntuples(r); i++) {
    const char *uid = PQgetvalue(r, i, 0);
    uint8_t url_b[512]; int64_t un = 0;
    if (!wos_c_build_delete_user(cs(workos_base_url()), cs(uid),
                                 (coax_v_u8){url_b, (int64_t)sizeof url_b - 1}, (coax_v_s64){&un, 1})) continue;
    url_b[un] = 0;
    http_client_res res;
    workos_http("DELETE", (const char *)url_b, NULL, 0, true, &res);
    if ((res.status >= 200 && res.status < 300) || res.status == 404) {
      const char *uv[1] = {uid};
      PQclear(PQexecParams(db, "UPDATE workos.pending_deletes SET deleted_at = now() WHERE workos_user_id = $1",
                           1, NULL, uv, NULL, NULL, 0));
      PQclear(PQexecParams(db, "UPDATE workos.users SET deleted_at = coalesce(deleted_at, now()), updated_at = now() WHERE id = $1",
                           1, NULL, uv, NULL, NULL, 0));
      done++;
    }
    http_client_res_free(&res);
  }
  PQclear(r);
  return done;
}

bool workos_route(const request *q, PGconn *db, const char *op) {
  if (strncmp(op, "workos/", 7)) return false;
  const char *sub = op + 7;
  bool get = !strcmp(q->method, "GET");
  bool post = !strcmp(q->method, "POST");
  if (!strcmp(sub, "config")) { if (!get) { auth_refuse(q, 405, "GET this."); return true; } route_config(q); return true; }
  if (!db) { auth_refuse(q, 503, "The database isn't answering. Try again."); return true; }
  if (!strcmp(sub, "oauth/start")) { if (!get) { auth_refuse(q, 405, "GET this."); return true; } route_oauth_start(q, db); return true; }
  if (!strcmp(sub, "callback")) { if (!get) { auth_refuse(q, 405, "GET this."); return true; } route_callback(q, db); return true; }
  if (!strcmp(sub, "org/pending")) { if (!get) { auth_refuse(q, 405, "GET this."); return true; } route_org_pending(q, db); return true; }
  if (!strcmp(sub, "orgs")) { if (!get) { auth_refuse(q, 405, "GET this."); return true; } route_orgs_list(q, db); return true; }

  // POST routes
  if (!post) { auth_refuse(q, 405, "POST this."); return true; }
  // Webhook & AuthKit Action arrive server-to-server from WorkOS (HMAC-signed, no browser Origin).
  bool s2s = !strcmp(sub, "webhook") || !strcmp(sub, "action");
  if (!s2s && !auth_first_party(q)) { auth_refuse(q, 403, "Not from this site."); return true; }
  char *body = body_copy(q);
  if (!body) { auth_refuse(q, 500, "Out of memory."); return true; }
  if (!strcmp(sub, "magic/start")) route_magic_start(q, db, body);
  else if (!strcmp(sub, "magic/verify")) route_magic_verify(q, db, body);
  else if (!strcmp(sub, "org/select")) route_org_select(q, db, body);
  else if (!strcmp(sub, "sso/discover")) route_sso_discover(q, db, body);
  else if (!strcmp(sub, "device/start")) route_device_start(q, db);
  else if (!strcmp(sub, "exchange")) route_exchange(q, db, body);
  else if (!strcmp(sub, "orgs/switch")) route_orgs_switch(q, db, body);
  else if (!strcmp(sub, "webhook")) route_webhook(q, db, body);
  else if (!strcmp(sub, "action")) route_action(q, db, body);
  else { free(body); return false; }
  free(body);
  return true;
}
