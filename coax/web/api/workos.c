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
#include "auth-policy.generated.h"
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
  explicit_bzero(auth_hdr, sizeof auth_hdr);
}

const char *workos_pipes_base_url(void) { return workos_base_url(); }
bool workos_pipes_allow_local(void) { return workos_allow_local(); }
bool workos_pipes_configured(void) { return workos_api_key()[0] != 0; }

bool workos_lookup_identity(PGconn *db, const char *account, char uid_out[128], char org_out[128]) {
  uid_out[0] = 0;
  org_out[0] = 0;
  if (!db || !account || !*account) return false;
  const char *v[1] = {account};
  PGresult *r = PQexecParams(
      db,
      "SELECT workos_user_id, coalesce(last_organization_id, '') FROM auth.workos_identities "
      "WHERE account = $1 ORDER BY last_seen_at DESC LIMIT 1",
      1, NULL, v, NULL, NULL, 0);
  bool ok = r && PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  if (ok) {
    snprintf(uid_out, 128, "%s", PQgetvalue(r, 0, 0));
    snprintf(org_out, 128, "%s", PQgetvalue(r, 0, 1));
  }
  PQclear(r);
  return ok && uid_out[0] != 0;
}

void workos_api_request(const char *method, const char *url, const char *body, size_t body_len, void *http_res) {
  workos_http(method, url, body, body_len, true, (http_client_res *)http_res);
}

bool workos_pipes_github_token(PGconn *db, const char *account, char *token_out, size_t token_cap, char *err_out, size_t err_cap) {
  if (token_out && token_cap) token_out[0] = 0;
  if (err_out && err_cap) err_out[0] = 0;
  if (!token_out || token_cap < 16) return false;
  if (!workos_pipes_configured()) {
    if (err_out && err_cap) snprintf(err_out, err_cap, "GitHub integration is not configured.");
    return false;
  }
  char uid[128] = "", org[128] = "";
  if (!workos_lookup_identity(db, account, uid, org)) {
    if (err_out && err_cap) snprintf(err_out, err_cap, "Log in to connect GitHub.");
    return false;
  }
  uint8_t url_b[512], body_b[512];
  int64_t un = 0, bn = 0;
  if (!wos_c_build_pipes_token(
          cs(workos_base_url()), cs("github"), cs(uid), cs(org),
          (coax_v_u8){url_b, (int64_t)sizeof url_b - 1}, (coax_v_u8){body_b, (int64_t)sizeof body_b - 1},
          (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1})) {
    if (err_out && err_cap) snprintf(err_out, err_cap, "Could not build GitHub token request.");
    return false;
  }
  url_b[un] = 0;
  body_b[bn] = 0;
  http_client_res res;
  workos_http("POST", (const char *)url_b, (const char *)body_b, (size_t)bn, true, &res);
  bool ok = false;
  if (res.status == 200 && res.body && res.body_len > 0) {
    int64_t tn = wos_c_pipes_parse_token(
        (coax_string){res.body, (int64_t)res.body_len},
        (coax_v_u8){(uint8_t *)token_out, (int64_t)token_cap - 1});
    if (tn > 0) {
      token_out[tn] = 0;
      ok = true;
    }
  }
  if (!ok && err_out && err_cap) {
    snprintf(err_out, err_cap, res.status == 404 || res.status == 200 ? "GitHub is not connected." : "GitHub token unavailable.");
  }
  if (res.body && res.body_len > 0) explicit_bzero(res.body, res.body_len);
  http_client_res_free(&res);
  return ok;
}

static bool same_site_next(const char *s) {
  if (!s || s[0] != '/') return false;
  if (s[1] == '/' || s[1] == '\\') return false;
  if (!strncmp(s, "/login", 6) && strncmp(s, "/login/device", 13) != 0) return false;
  if (!strncmp(s, "/onboarding", 11) && (s[11] == 0 || s[11] == '/' || s[11] == '?')) return false;
  for (const unsigned char *p = (const unsigned char *)s; *p; p++)
    if (*p <= ' ' || *p >= 127 || *p == '\\') return false;
  return strlen(s) < 256;
}

static const char *workos_github_api_base(void) { return env_or("OA_GITHUB_API_BASE", "https://api.github.com"); }
static bool workos_github_allow_local(void) {
  const char *b = workos_github_api_base();
  return !strncmp(b, "http://127.0.0.1:", 17) || !strncmp(b, "http://localhost:", 17);
}

static void workos_gh_parse_profile(PGconn *db, const char *body, char gh_id[64], char gh_login[64],
                                    char gh_avatar[256], char gh_created[48], int *repos_out, int *followers_out) {
  if (!body || !*body) return;
  const char *v[1] = {body};
  PGresult *r = PQexecParams(
      db,
      "WITH d AS (SELECT $1::jsonb j) "
      "SELECT coalesce(j->>'id', ''), coalesce(j->>'login', ''), coalesce(j->>'avatar_url', ''), "
      "       coalesce(j->>'created_at', ''), coalesce((j->>'public_repos')::int, 0), coalesce((j->>'followers')::int, 0) "
      "FROM d",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    if (!gh_id[0] && PQgetvalue(r, 0, 0)[0]) snprintf(gh_id, 64, "%s", PQgetvalue(r, 0, 0));
    if (!gh_login[0] && PQgetvalue(r, 0, 1)[0]) snprintf(gh_login, 64, "%s", PQgetvalue(r, 0, 1));
    if (!gh_avatar[0] && PQgetvalue(r, 0, 2)[0]) snprintf(gh_avatar, 256, "%s", PQgetvalue(r, 0, 2));
    if (!gh_created[0] && PQgetvalue(r, 0, 3)[0]) snprintf(gh_created, 48, "%s", PQgetvalue(r, 0, 3));
    if (repos_out) *repos_out = atoi(PQgetvalue(r, 0, 4));
    if (followers_out) *followers_out = atoi(PQgetvalue(r, 0, 5));
  }
  PQclear(r);
}

static void workos_log_err(PGconn *db, const char *acct, const char *session, const char *event,
                           const char *stage, int http_status, const char *detail) {
  const char *st = stage ? stage : "unknown";
  const char *dt = detail ? detail : "";
  fprintf(stderr, "workos_err event=%s stage=%s acct=%s status=%d detail=%.240s\n",
          event ? event : "workos.error", st, (acct && *acct) ? acct : "-", http_status, dt);
  if (db && event && *event) {
    char st_esc[128], dt_esc[512], jbuf[768];
    auth_json_esc(st, st_esc, sizeof st_esc);
    auth_json_esc(dt, dt_esc, sizeof dt_esc);
    snprintf(jbuf, sizeof jbuf, "{\"stage\":\"%s\",\"status\":%d,\"detail\":\"%s\"}", st_esc, http_status, dt_esc);
    auth_audit(db, (acct && *acct) ? acct : NULL, (session && *session) ? session : NULL, event, "error", jbuf);
  }
}

// Extracts the immutable GitHub numeric ID (+ profile metadata when reachable) from a WorkOS
// GitHubOAuth authenticate response. Uses oauth_tokens.access_token -> GitHub /user when present,
// and always falls back to WorkOS GET /user_management/users/{id}/identities (idp_id) + public
// GitHub /user/{idp_id} so onboarding works regardless of whether WorkOS returns OAuth tokens.
static bool workos_extract_github_identity(PGconn *db, const char *acct, const char *auth_json, const char *wos_uid,
                                           char gh_id[64], char gh_login[64], char gh_avatar[256],
                                           char gh_created[48], int *repos_out, int *followers_out) {
  gh_id[0] = 0; gh_login[0] = 0; gh_avatar[0] = 0; gh_created[0] = 0;
  if (repos_out) *repos_out = 0;
  if (followers_out) *followers_out = 0;
  int gh_user_status = 0, id_status = 0;
  bool had_token = false;

  // 1. Primary path: oauth_tokens.access_token from WorkOS authenticate response -> GET /user
  if (auth_json && *auth_json) {
    char tok[512] = "";
    const char *tv[1] = {auth_json};
    PGresult *tr = PQexecParams(db, "SELECT coalesce(($1::jsonb)->'oauth_tokens'->>'access_token', '')", 1, NULL, tv, NULL, NULL, 0);
    if (PQresultStatus(tr) == PGRES_TUPLES_OK && PQntuples(tr) == 1) snprintf(tok, sizeof tok, "%s", PQgetvalue(tr, 0, 0));
    PQclear(tr);
    if (tok[0]) {
      had_token = true;
      char url[512], ahdr[600];
      snprintf(url, sizeof url, "%s/user", workos_github_api_base());
      snprintf(ahdr, sizeof ahdr, "Authorization: Bearer %s", tok);
      explicit_bzero(tok, sizeof tok);
      const char *hdrs[] = {ahdr, "Accept: application/vnd.github+json", "X-GitHub-Api-Version: 2022-11-28", "User-Agent: openagents-api", NULL};
      http_client_req req = {.method = "GET", .url = url, .headers = hdrs, .connect_timeout_ms = 5000, .total_timeout_ms = 10000, .max_response_bytes = 262144, .allow_http_local = workos_github_allow_local()};
      http_client_res gres = {0};
      http_client_send(&req, &gres);
      explicit_bzero(ahdr, sizeof ahdr);
      gh_user_status = gres.status;
      if (gres.status == 200 && gres.body && gres.body_len > 0) {
        workos_gh_parse_profile(db, gres.body, gh_id, gh_login, gh_avatar, gh_created, repos_out, followers_out);
      }
      http_client_res_free(&gres);
    }
  }

  // 2. Guaranteed fallback: WorkOS GET /user_management/users/{wos_uid}/identities -> GitHub idp_id
  char providers_seen[192] = "";
  if (!gh_id[0] && wos_uid && *wos_uid) {
    uint8_t url_b[512]; int64_t un = 0;
    if (wos_c_build_get_user_identities(cs(workos_base_url()), cs(wos_uid),
                                        (coax_v_u8){url_b, (int64_t)sizeof url_b - 1}, (coax_v_s64){&un, 1})) {
      url_b[un] = 0;
      http_client_res ires = {0};
      workos_http("GET", (const char *)url_b, NULL, 0, true, &ires);
      id_status = ires.status;
      if (ires.status == 200 && ires.body && ires.body_len > 0) {
        int64_t idn = wos_c_parse_github_idp_id((coax_string){ires.body, (int64_t)ires.body_len},
                                                (coax_v_u8){(uint8_t *)gh_id, 63});
        if (idn > 0) gh_id[idn] = 0;
        else {
          gh_id[0] = 0;
          // Extract non-secret provider names for diagnostic logging.
          const char *iv[1] = {ires.body};
          PGresult *pr = PQexecParams(
              db,
              "SELECT coalesce(string_agg(coalesce(x->>'provider', '?') || ':' || coalesce(x->>'type', '?'), ','), 'empty') "
              "FROM jsonb_array_elements(CASE WHEN jsonb_typeof($1::jsonb) = 'array' THEN $1::jsonb ELSE coalesce(($1::jsonb)->'data', '[]'::jsonb) END) x",
              1, NULL, iv, NULL, NULL, 0);
          if (PQresultStatus(pr) == PGRES_TUPLES_OK && PQntuples(pr) == 1) {
            snprintf(providers_seen, sizeof providers_seen, "%s", PQgetvalue(pr, 0, 0));
          }
          PQclear(pr);
        }
      }
      http_client_res_free(&ires);
    }
  }

  // 3. Best-effort public GitHub lookup by numeric ID to populate handle/avatar/created_at when missing.
  if (gh_id[0] && !gh_login[0]) {
    bool all_digits = true;
    size_t len = strlen(gh_id);
    if (len == 0 || len > 24) all_digits = false;
    for (size_t i = 0; i < len && all_digits; i++) if (gh_id[i] < '0' || gh_id[i] > '9') all_digits = false;
    if (all_digits) {
      char url[512];
      snprintf(url, sizeof url, "%s/user/%s", workos_github_api_base(), gh_id);
      const char *hdrs[] = {"Accept: application/vnd.github+json", "X-GitHub-Api-Version: 2022-11-28", "User-Agent: openagents-api", NULL};
      http_client_req req = {.method = "GET", .url = url, .headers = hdrs, .connect_timeout_ms = 3000, .total_timeout_ms = 5000, .max_response_bytes = 262144, .allow_http_local = workos_github_allow_local()};
      http_client_res pres = {0};
      http_client_send(&req, &pres);
      if (pres.status == 200 && pres.body && pres.body_len > 0) {
        workos_gh_parse_profile(db, pres.body, gh_id, gh_login, gh_avatar, gh_created, repos_out, followers_out);
      }
      http_client_res_free(&pres);
    }
  }
  if (!gh_id[0]) {
    char d[320];
    snprintf(d, sizeof d, "wos_uid=%s had_token=%d gh_user_status=%d identities_status=%d providers=%s",
             (wos_uid && *wos_uid) ? wos_uid : "-", had_token ? 1 : 0, gh_user_status, id_status,
             providers_seen[0] ? providers_seen : "-");
    workos_log_err(db, acct, NULL, "onboarding.github_link", "extract_github_identity", id_status ? id_status : gh_user_status, d);
  }
  return gh_id[0] != 0;
}

static bool workos_link_github_account(PGconn *db, const char *acct, const char *wos_uid, const char *auth_json,
                                       char gh_id_out[64], char gh_login_out[64],
                                       long long *newly_granted_out, bool *already_claimed_out) {
  if (gh_id_out) gh_id_out[0] = 0;
  if (gh_login_out) gh_login_out[0] = 0;
  if (newly_granted_out) *newly_granted_out = 0;
  if (already_claimed_out) *already_claimed_out = false;
  char gh_id[64] = "", gh_login[64] = "", gh_avatar[256] = "", gh_created[48] = "";
  int repos = 0, followers = 0;
  if (!workos_extract_github_identity(db, acct, auth_json, wos_uid, gh_id, gh_login, gh_avatar, gh_created, &repos, &followers)) {
    return false;
  }
  const char *v[5] = {acct, gh_id, gh_login, gh_avatar, wos_uid ? wos_uid : ""};
  PGresult *r = PQexecParams(
      db,
      "INSERT INTO auth.github_links (account_id, github_id, github_login, github_avatar, workos_user_id, linked_at, updated_at) "
      "VALUES ($1, $2, $3, $4, $5, now(), now()) "
      "ON CONFLICT (account_id) DO UPDATE SET "
      "github_id = EXCLUDED.github_id, "
      "github_login = CASE WHEN EXCLUDED.github_login <> '' THEN EXCLUDED.github_login ELSE auth.github_links.github_login END, "
      "github_avatar = CASE WHEN EXCLUDED.github_avatar <> '' THEN EXCLUDED.github_avatar ELSE auth.github_links.github_avatar END, "
      "workos_user_id = CASE WHEN EXCLUDED.workos_user_id <> '' THEN EXCLUDED.workos_user_id ELSE auth.github_links.workos_user_id END, "
      "updated_at = now()",
      5, NULL, v, NULL, NULL, 0);
  bool ok = r && PQresultStatus(r) == PGRES_COMMAND_OK;
  if (!ok) {
    workos_log_err(db, acct, NULL, "onboarding.github_link", "db_insert_github_links", 500, PQerrorMessage(db));
  }
  if (r) PQclear(r);
  if (!ok) return false;
  referral_on_github_linked(db, acct, gh_id, gh_login, gh_avatar, gh_created, repos, followers, 0, newly_granted_out, already_claimed_out);
  if (gh_id_out) snprintf(gh_id_out, 64, "%s", gh_id);
  if (gh_login_out) snprintf(gh_login_out, 64, "%s", gh_login);
  return true;
}

static bool workos_pending_is_link(PGconn *db, const char *state, const char *cookie_state) {
  const char *tok = (cookie_state && *cookie_state) ? cookie_state : state;
  if (!db || !tok || !*tok) return false;
  char ssha[65]; auth_sha_hex(tok, ssha);
  const char *v[1] = {ssha};
  PGresult *r = PQexecParams(
      db,
      "SELECT kind FROM auth.workos_pending WHERE token_sha256 = $1 AND expires_at > now() LIMIT 1",
      1, NULL, v, NULL, NULL, 0);
  bool is_link = r && PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "oauth_link");
  if (r) PQclear(r);
  return is_link;
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

static bool workos_new_account_id(char out[40]) {
  unsigned char raw[17];
  if (!auth_random(raw, sizeof raw)) return false;
  static const char alpha[] = "abcdefghijklmnopqrstuvwxyz234567";
  memcpy(out, "acct_", 5);
  for (int i = 0; i < 26; i++) {
    unsigned bit = (unsigned)(5 * i);
    unsigned v = (((unsigned)raw[bit / 8] << 8) | (unsigned)raw[bit / 8 + 1]) >> (11 - (bit % 8));
    out[5 + i] = alpha[v & 31];
  }
  out[31] = 0;
  return true;
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

  const char *prov = provider ? provider : "";
  const char *org = org_id ? org_id : "";
  char email_id[24] = "";

  PGresult *tx = PQexec(db, "BEGIN");
  if (!tx || PQresultStatus(tx) != PGRES_COMMAND_OK) {
    if (tx) PQclear(tx);
    explicit_bzero(norm_email, sizeof norm_email);
    return false;
  }
  PQclear(tx);

  // 1. Existing link by workos_user_id (skip deleted/merged tombstones).
  const char *v1[1] = {wos_uid};
  PGresult *r = PQexecParams(
      db,
      "SELECT wi.account "
      "  FROM auth.workos_identities wi "
      "  JOIN auth.accounts acc ON acc.id = wi.account "
      " WHERE wi.workos_user_id = $1 "
      "   AND acc.deleted_at IS NULL "
      "   AND acc.merged_into IS NULL "
      " LIMIT 1",
      1, NULL, v1, NULL, NULL, 0);
  if (!r || PQresultStatus(r) != PGRES_TUPLES_OK) {
    api_log("workos: account lookup failed: %s", PQerrorMessage(db));
    if (r) PQclear(r);
    PQclear(PQexec(db, "ROLLBACK"));
    explicit_bzero(norm_email, sizeof norm_email);
    return false;
  }
  if (PQntuples(r) == 1) {
    snprintf(acct_out, 40, "%s", PQgetvalue(r, 0, 0));
    PQclear(r);
    const char *uv[3] = {wos_uid, prov, org};
    PGresult *ur = PQexecParams(
        db,
        "UPDATE auth.workos_identities "
        "   SET last_seen_at = now(), "
        "       provider = coalesce(nullif($2, ''), provider), "
        "       last_organization_id = coalesce(nullif($3, ''), last_organization_id) "
        " WHERE workos_user_id = $1",
        3, NULL, uv, NULL, NULL, 0);
    if (!ur || PQresultStatus(ur) != PGRES_COMMAND_OK) {
      api_log("workos: identity touch failed: %s", PQerrorMessage(db));
      if (ur) PQclear(ur);
      PQclear(PQexec(db, "ROLLBACK"));
      explicit_bzero(norm_email, sizeof norm_email);
      return false;
    }
    PQclear(ur);
  } else {
    PQclear(r);
    // 2. Auto-link by verified email ONLY when email_verified = true.
    bool linked = false;
    if (email_verified && have_lookups) {
      const char *ev[1] = {all_lookups};
      PGresult *er = PQexecParams(
          db,
          "SELECT e.account, e.id::text "
          "  FROM auth.emails e "
          "  JOIN auth.accounts acc ON acc.id = e.account "
          " WHERE e.lookup = ANY($1::text[]) "
          "   AND acc.deleted_at IS NULL "
          "   AND acc.merged_into IS NULL "
          " ORDER BY e.is_primary DESC, e.id ASC "
          " LIMIT 1",
          1, NULL, ev, NULL, NULL, 0);
      if (!er || PQresultStatus(er) != PGRES_TUPLES_OK) {
        api_log("workos: email lookup failed: %s", PQerrorMessage(db));
        if (er) PQclear(er);
        PQclear(PQexec(db, "ROLLBACK"));
        explicit_bzero(norm_email, sizeof norm_email);
        return false;
      }
      if (PQntuples(er) == 1) {
        snprintf(acct_out, 40, "%s", PQgetvalue(er, 0, 0));
        snprintf(email_id, sizeof email_id, "%s", PQgetvalue(er, 0, 1));
        linked = true;
        *auto_linked_out = true;
      }
      PQclear(er);
    }
    if (!linked) {
      // 3. Provision a brand-new acct_ (for new users or unverified emails — never merges on unverified email).
      if (!workos_new_account_id(acct_out)) {
        PQclear(PQexec(db, "ROLLBACK"));
        explicit_bzero(norm_email, sizeof norm_email);
        return false;
      }
      const char *av[2] = {acct_out, team ? "t" : "f"};
      PGresult *ar = PQexecParams(
          db,
          "INSERT INTO auth.accounts (id, kind, team, verse_account) "
          "VALUES ($1, 'person', $2::boolean, $1)",
          2, NULL, av, NULL, NULL, 0);
      if (!ar || PQresultStatus(ar) != PGRES_COMMAND_OK) {
        api_log("workos: account insert failed: %s", PQerrorMessage(db));
        if (ar) PQclear(ar);
        PQclear(PQexec(db, "ROLLBACK"));
        explicit_bzero(norm_email, sizeof norm_email);
        return false;
      }
      PQclear(ar);
      if (email_verified && primary_lookup[0]) {
        const char *emv[2] = {acct_out, primary_lookup};
        PGresult *emr = PQexecParams(
            db,
            "INSERT INTO auth.emails (account, lookup, sealed, is_primary, verified_at) "
            "VALUES ($1, $2, '', true, now()) "
            "RETURNING id::text",
            2, NULL, emv, NULL, NULL, 0);
        if (!emr || PQresultStatus(emr) != PGRES_TUPLES_OK || PQntuples(emr) != 1) {
          api_log("workos: email insert failed: %s", PQerrorMessage(db));
          if (emr) PQclear(emr);
          PQclear(PQexec(db, "ROLLBACK"));
          explicit_bzero(norm_email, sizeof norm_email);
          return false;
        }
        snprintf(email_id, sizeof email_id, "%s", PQgetvalue(emr, 0, 0));
        PQclear(emr);
      }
      *made_out = true;
    }
    const char *iv[4] = {wos_uid, acct_out, prov, org};
    PGresult *ir = PQexecParams(
        db,
        "INSERT INTO auth.workos_identities (workos_user_id, account, provider, last_organization_id) "
        "VALUES ($1, $2, nullif($3, ''), nullif($4, '')) "
        "ON CONFLICT (workos_user_id) DO UPDATE "
        "  SET account = EXCLUDED.account, "
        "      provider = coalesce(EXCLUDED.provider, auth.workos_identities.provider), "
        "      last_organization_id = coalesce(EXCLUDED.last_organization_id, auth.workos_identities.last_organization_id), "
        "      last_seen_at = now()",
        4, NULL, iv, NULL, NULL, 0);
    if (!ir || PQresultStatus(ir) != PGRES_COMMAND_OK) {
      api_log("workos: identity link failed: %s", PQerrorMessage(db));
      if (ir) PQclear(ir);
      PQclear(PQexec(db, "ROLLBACK"));
      explicit_bzero(norm_email, sizeof norm_email);
      return false;
    }
    PQclear(ir);
  }

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
             provider && *provider ? provider : "Cloud login", masked[0] ? masked : "verified email", auth_origin());
    auth_notice(db, acct_out, "workos.link", "new log-in method linked", words);
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
  if (status == 403 && err.code_n == 27 && !memcmp(err.code.e, "email_verification_required", 27) && err.pending_n > 0) {
    char pat[512], email[300] = "";
    int64_t pn = err.pending_n < (int64_t)sizeof pat - 1 ? err.pending_n : (int64_t)sizeof pat - 1;
    memcpy(pat, err.pending_auth_token.e, (size_t)pn);
    pat[pn] = 0;
    if (body && body_len > 0) {
      const char *ev[1] = {body};
      PGresult *er = PQexecParams(db, "SELECT coalesce(($1::jsonb)->>'email', '')", 1, NULL, ev, NULL, NULL, 0);
      if (PQresultStatus(er) == PGRES_TUPLES_OK && PQntuples(er) == 1) snprintf(email, sizeof email, "%s", PQgetvalue(er, 0, 0));
      PQclear(er);
    }
    char id[33], pend[65], psha[65];
    if (!auth_random_hex(id, 16) || !auth_random_hex(pend, 32)) { auth_refuse(q, 503, "No randomness here. Try again."); return; }
    auth_sha_hex(pend, psha);
    const char *pv[5] = {id, psha, email[0] ? email : NULL, pat, app ? "app" : "web"};
    PGresult *pr = PQexecParams(
        db,
        "INSERT INTO auth.workos_pending (id, kind, token_sha256, email, pending_auth_token, client, expires_at) "
        "VALUES ($1, 'magic', $2, $3, $4, $5, now() + interval '10 minutes')",
        5, NULL, pv, NULL, NULL, 0);
    bool ok = PQresultStatus(pr) == PGRES_COMMAND_OK;
    PQclear(pr);
    if (!ok) { auth_refuse(q, 500, "That didn't work. Try again."); return; }
    char hdr[320] = "", json[300], lines[240];
    if (!app) {
      snprintf(hdr, sizeof hdr, "Set-Cookie: %s=%s; Path=/; Max-Age=600; HttpOnly; Secure; SameSite=Lax\r\n",
               WOS_PENDING_COOKIE, pend);
      snprintf(json, sizeof json, "{\"state\":\"verify-email\",\"expires_in\":600,\"tries\":5}");
      snprintf(lines, sizeof lines, "state verify-email\nexpires_in 600\ntries 5\n");
    } else {
      snprintf(json, sizeof json, "{\"state\":\"verify-email\",\"pending\":\"%s\",\"expires_in\":600,\"tries\":5}", pend);
      snprintf(lines, sizeof lines, "state verify-email\npending %s\nexpires_in 600\ntries 5\n", pend);
    }
    auth_answer(q, 200, hdr[0] ? hdr : NULL, json, lines);
    return;
  }

  char msg[260] = "Sign-in didn't work. Try again.";
  if (action && strstr(action, "magic")) snprintf(msg, sizeof msg, "That code isn't right. Try again.");
  else if (action && strstr(action, "password")) snprintf(msg, sizeof msg, "Invalid email or password.");
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
           "{\"enabled\":%s,\"client_id\":\"%s\",\"providers\":[\"magic\",\"password\",\"google\",\"apple\"]}",
           on ? "true" : "false", cid_esc);
  snprintf(lines, sizeof lines, "enabled %d\nclient_id %s\nproviders magic,password,google,apple\n",
           on ? 1 : 0, on ? workos_client_id() : "");
  auth_answer(q, 200, NULL, json, lines);
}

static void route_magic_start(const request *q, PGconn *db, const char *body) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "That login method isn't enabled here."); return; }
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
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "That login method isn't enabled here."); return; }
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
  char email_verify_pat[512] = "";
  auth_cookie(q, WOS_PENDING_COOKIE, pend, sizeof pend);
  if (!pend[0]) auth_json_str(body, q->body_len, "pending", pend, sizeof pend);
  if (strlen(pend) == 64) {
    char psha[65]; auth_sha_hex(pend, psha);
    const char *pv[1] = {psha};
    PGresult *pr = PQexecParams(
        db,
        "SELECT coalesce(email, ''), client, coalesce(pending_auth_token, '') FROM auth.workos_pending "
        "WHERE kind = 'magic' AND token_sha256 = $1 AND spent_at IS NULL AND expires_at > now()",
        1, NULL, pv, NULL, NULL, 0);
    if (PQresultStatus(pr) == PGRES_TUPLES_OK && PQntuples(pr) == 1) {
      snprintf(email, sizeof email, "%s", PQgetvalue(pr, 0, 0));
      if (!strcmp(PQgetvalue(pr, 0, 1), "app")) app = true;
      snprintf(email_verify_pat, sizeof email_verify_pat, "%s", PQgetvalue(pr, 0, 2));
    }
    PQclear(pr);
  }
  if (!email[0] && !email_verify_pat[0]) {
    char raw_e[400] = "";
    if (auth_json_str(body, q->body_len, "email", raw_e, sizeof raw_e)) auth_normal_email(raw_e, email, sizeof email);
  }
  if (!email[0] && !email_verify_pat[0]) { auth_refuse(q, 409, "Ask for a code in this browser first."); return; }

  char ip[128], ua[256];
  radar_context(q, ip, ua);
  uint8_t url_b[512], req_b[2048];
  int64_t un = 0, bn = 0;
  bool built = email_verify_pat[0]
    ? wos_c_build_auth_email_verify(cs(workos_base_url()), cs(workos_client_id()), cs(workos_api_key()),
                                    cs(email_verify_pat), cs(code), cs(ip), cs(ua),
                                    (coax_v_u8){url_b, (int64_t)sizeof url_b - 1},
                                    (coax_v_u8){req_b, (int64_t)sizeof req_b - 1},
                                    (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1})
    : wos_c_build_auth_magic(cs(workos_base_url()), cs(workos_client_id()), cs(workos_api_key()),
                             cs(email), cs(code), cs(ip), cs(ua), cs(radar_id),
                             (coax_v_u8){url_b, (int64_t)sizeof url_b - 1},
                             (coax_v_u8){req_b, (int64_t)sizeof req_b - 1},
                             (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1});
  if (!built) {
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

static void route_password_login(const request *q, PGconn *db, const char *body) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "That login method isn't enabled here."); return; }
  char raw_email[400] = "", email[300] = "", password[260] = "", radar_id[128] = "", client[16] = "web";
  if (!auth_json_str(body, q->body_len, "email", raw_email, sizeof raw_email) || !auth_normal_email(raw_email, email, sizeof email)) {
    auth_refuse(q, 400, "Type your email address.");
    return;
  }
  if (!auth_json_str(body, q->body_len, "password", password, sizeof password) || !password[0]) {
    explicit_bzero(password, sizeof password);
    auth_refuse(q, 400, "Type your password.");
    return;
  }
  auth_json_str(body, q->body_len, "client", client, sizeof client);
  auth_json_str(body, q->body_len, "radar_auth_attempt_id", radar_id, sizeof radar_id);
  bool app = !strcmp(client, "app");

  char ipk[65]; auth_visitor_key(q, ipk);
  long long left_ms = 0;
  if (!api_limit(db, "workos_magic_ip", ipk, 30, &left_ms)) {
    explicit_bzero(password, sizeof password);
    auth_refuse(q, 429, "Too many sign-in attempts from here. Try again in a minute.");
    return;
  }

  char ip[128], ua[256];
  radar_context(q, ip, ua);
  uint8_t url_b[512], req_b[2048];
  int64_t un = 0, bn = 0;
  bool built = wos_c_build_auth_password(cs(workos_base_url()), cs(workos_client_id()), cs(workos_api_key()),
                                         cs(email), cs(password), cs(ip), cs(ua), cs(radar_id),
                                         (coax_v_u8){url_b, (int64_t)sizeof url_b - 1},
                                         (coax_v_u8){req_b, (int64_t)sizeof req_b - 1},
                                         (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1});
  explicit_bzero(password, sizeof password);
  if (!built) {
    explicit_bzero(req_b, sizeof req_b);
    auth_refuse(q, 500, "That didn't work. Try again.");
    return;
  }
  url_b[un] = 0; req_b[bn] = 0;
  http_client_res res;
  workos_http("POST", (const char *)url_b, (const char *)req_b, (size_t)bn, true, &res);
  explicit_bzero(req_b, sizeof req_b);
  finish_workos_auth_api(q, db, res.status, res.body, res.body_len, app, "login.workos_password", NULL);
  http_client_res_free(&res);
}

static void route_password_register(const request *q, PGconn *db, const char *body) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "That login method isn't enabled here."); return; }
  char raw_email[400] = "", email[300] = "", password[260] = "", radar_id[128] = "", client[16] = "web";
  if (!auth_json_str(body, q->body_len, "email", raw_email, sizeof raw_email) || !auth_normal_email(raw_email, email, sizeof email)) {
    auth_refuse(q, 400, "Type your email address.");
    return;
  }
  if (!auth_json_str(body, q->body_len, "password", password, sizeof password) || strlen(password) < 8) {
    explicit_bzero(password, sizeof password);
    auth_refuse(q, 400, "Use a password of at least 8 characters.");
    return;
  }
  auth_json_str(body, q->body_len, "client", client, sizeof client);
  auth_json_str(body, q->body_len, "radar_auth_attempt_id", radar_id, sizeof radar_id);
  bool app = !strcmp(client, "app");

  char ipk[65]; auth_visitor_key(q, ipk);
  long long left_ms = 0;
  if (!api_limit(db, "workos_magic_ip", ipk, 30, &left_ms)) {
    explicit_bzero(password, sizeof password);
    auth_refuse(q, 429, "Too many sign-in attempts from here. Try again in a minute.");
    return;
  }

  char ip[128], ua[256];
  radar_context(q, ip, ua);
  uint8_t url_b[512], req_b[2048];
  int64_t un = 0, bn = 0;
  bool built = wos_c_build_create_user_password(cs(workos_base_url()), cs(email), cs(password), cs(ip), cs(ua),
                                                (coax_v_u8){url_b, (int64_t)sizeof url_b - 1},
                                                (coax_v_u8){req_b, (int64_t)sizeof req_b - 1},
                                                (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1});
  if (!built) {
    explicit_bzero(password, sizeof password);
    explicit_bzero(req_b, sizeof req_b);
    auth_refuse(q, 500, "That didn't work. Try again.");
    return;
  }
  url_b[un] = 0; req_b[bn] = 0;
  http_client_res cres;
  workos_http("POST", (const char *)url_b, (const char *)req_b, (size_t)bn, true, &cres);
  explicit_bzero(req_b, sizeof req_b);
  if (cres.status < 200 || cres.status >= 300) {
    coax_t_WosApiError err;
    memset(&err, 0, sizeof err);
    wos_parse_api_error(cres.status, (coax_string){cres.body ? cres.body : "", (int64_t)cres.body_len}, cs(""), cs(""),
                        (coax_v_t_WosApiError){&err, 1});
    char msg[260] = "Could not create account. Try signing in or using an email code.";
    if (err.message_n > 0) {
      int64_t mn = err.message_n < (int64_t)sizeof msg - 1 ? err.message_n : (int64_t)sizeof msg - 1;
      memcpy(msg, err.message.e, (size_t)mn);
      msg[mn] = 0;
    }
    int st = (cres.status == 400 || cres.status == 409 || cres.status == 422 || cres.status == 429) ? (int)cres.status : 502;
    http_client_res_free(&cres);
    explicit_bzero(password, sizeof password);
    auth_refuse(q, st, msg);
    return;
  }
  http_client_res_free(&cres);

  // Immediately authenticate the newly created user with their password.
  built = wos_c_build_auth_password(cs(workos_base_url()), cs(workos_client_id()), cs(workos_api_key()),
                                    cs(email), cs(password), cs(ip), cs(ua), cs(radar_id),
                                    (coax_v_u8){url_b, (int64_t)sizeof url_b - 1},
                                    (coax_v_u8){req_b, (int64_t)sizeof req_b - 1},
                                    (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1});
  explicit_bzero(password, sizeof password);
  if (!built) {
    explicit_bzero(req_b, sizeof req_b);
    auth_refuse(q, 500, "Account created, but sign-in didn't finish. Log in now.");
    return;
  }
  url_b[un] = 0; req_b[bn] = 0;
  http_client_res ares;
  workos_http("POST", (const char *)url_b, (const char *)req_b, (size_t)bn, true, &ares);
  explicit_bzero(req_b, sizeof req_b);
  finish_workos_auth_api(q, db, ares.status, ares.body, ares.body_len, app, "login.workos_password_register", NULL);
  http_client_res_free(&ares);
}

static void route_password_reset(const request *q, PGconn *db, const char *body) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "That login method isn't enabled here."); return; }
  char raw_email[400] = "", email[300] = "";
  if (!auth_json_str(body, q->body_len, "email", raw_email, sizeof raw_email) || !auth_normal_email(raw_email, email, sizeof email)) {
    auth_refuse(q, 400, "Type your email address.");
    return;
  }
  char ipk[65]; auth_visitor_key(q, ipk);
  long long left_ms = 0;
  if (!api_limit(db, "workos_magic_ip", ipk, 30, &left_ms)) {
    auth_refuse(q, 429, "Too many reset requests from here. Try again in a minute.");
    return;
  }
  uint8_t url_b[512], req_b[1024];
  int64_t un = 0, bn = 0;
  if (!wos_c_build_password_reset(cs(workos_base_url()), cs(email),
                                  (coax_v_u8){url_b, (int64_t)sizeof url_b - 1},
                                  (coax_v_u8){req_b, (int64_t)sizeof req_b - 1},
                                  (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1})) {
    auth_refuse(q, 400, "That isn't an email address.");
    return;
  }
  url_b[un] = 0; req_b[bn] = 0;
  http_client_res res;
  workos_http("POST", (const char *)url_b, (const char *)req_b, (size_t)bn, true, &res);
  long st = res.status;
  http_client_res_free(&res);
  if (st == 429) {
    auth_refuse(q, 429, "Too many reset requests. Try again shortly.");
    return;
  }
  auth_audit(db, NULL, NULL, "workos.password_reset", "ok", NULL);
  // Generic response avoids email enumeration even when WorkOS returns 404.
  auth_answer(q, 200, NULL,
              "{\"state\":\"sent\",\"message\":\"If an account exists for that email, we sent a password reset link.\"}",
              "state sent\nmessage If an account exists for that email, we sent a password reset link.\n");
}

static void route_password_reset_confirm(const request *q, PGconn *db, const char *body) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "That login method isn't enabled here."); return; }
  char token[512] = "", new_pw[260] = "";
  if (!auth_json_str(body, q->body_len, "token", token, sizeof token) || !token[0]) {
    auth_refuse(q, 400, "Missing password reset token.");
    return;
  }
  if (!auth_json_str(body, q->body_len, "new_password", new_pw, sizeof new_pw) || strlen(new_pw) < 8) {
    explicit_bzero(new_pw, sizeof new_pw);
    auth_refuse(q, 400, "Use a password of at least 8 characters.");
    return;
  }
  char ipk[65]; auth_visitor_key(q, ipk);
  long long left_ms = 0;
  if (!api_limit(db, "workos_magic_ip", ipk, 30, &left_ms)) {
    explicit_bzero(new_pw, sizeof new_pw);
    auth_refuse(q, 429, "Too many attempts from here. Try again in a minute.");
    return;
  }
  uint8_t url_b[512], req_b[2048];
  int64_t un = 0, bn = 0;
  bool built = wos_c_build_password_reset_confirm(cs(workos_base_url()), cs(token), cs(new_pw),
                                                  (coax_v_u8){url_b, (int64_t)sizeof url_b - 1},
                                                  (coax_v_u8){req_b, (int64_t)sizeof req_b - 1},
                                                  (coax_v_s64){&un, 1}, (coax_v_s64){&bn, 1});
  explicit_bzero(new_pw, sizeof new_pw);
  if (!built) {
    explicit_bzero(req_b, sizeof req_b);
    auth_refuse(q, 400, "Invalid password reset request.");
    return;
  }
  url_b[un] = 0; req_b[bn] = 0;
  http_client_res res;
  workos_http("POST", (const char *)url_b, (const char *)req_b, (size_t)bn, true, &res);
  explicit_bzero(req_b, sizeof req_b);
  if (res.status >= 200 && res.status < 300) {
    http_client_res_free(&res);
    auth_audit(db, NULL, NULL, "workos.password_reset_confirm", "ok", NULL);
    auth_answer(q, 200, NULL, "{\"state\":\"reset\"}", "state reset\n");
    return;
  }
  coax_t_WosApiError err;
  memset(&err, 0, sizeof err);
  wos_parse_api_error(res.status, (coax_string){res.body ? res.body : "", (int64_t)res.body_len}, cs(""), cs(""),
                      (coax_v_t_WosApiError){&err, 1});
  char msg[260] = "That reset link isn't valid or has expired.";
  if (err.message_n > 0) {
    int64_t mn = err.message_n < (int64_t)sizeof msg - 1 ? err.message_n : (int64_t)sizeof msg - 1;
    memcpy(msg, err.message.e, (size_t)mn);
    msg[mn] = 0;
  }
  int st = (res.status == 400 || res.status == 401 || res.status == 404 || res.status == 422 || res.status == 429) ? (int)res.status : 502;
  http_client_res_free(&res);
  auth_refuse(q, st, msg);
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
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "That login method isn't enabled here."); return; }
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
    auth_refuse(q, st == 429 ? 429 : 502, "Could not start device authorization.");
    return;
  }
  auth_audit(db, NULL, NULL, "workos.device_start", "ok", NULL);
  auth_answer(q, 200, NULL, res.body, NULL);
  http_client_res_free(&res);
}

static void route_sso_discover(const request *q, PGconn *db, const char *body) {
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "That login method isn't enabled here."); return; }
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
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "That login method isn't enabled here."); return; }
  char provider[64] = "", domain[200] = "", conn_id[128] = "", org_id[128] = "", login_hint[256] = "", next_path[256] = "/chat", mode[16] = "", link_q[16] = "";
  api_query(q, "provider", provider, sizeof provider);
  api_query(q, "domain", domain, sizeof domain);
  api_query(q, "connection_id", conn_id, sizeof conn_id);
  api_query(q, "organization_id", org_id, sizeof org_id);
  api_query(q, "login_hint", login_hint, sizeof login_hint);
  api_query(q, "mode", mode, sizeof mode);
  api_query(q, "link", link_q, sizeof link_q);
  bool is_link = !strcmp(link_q, "1") || !strcmp(link_q, "true");
  char raw_next[256] = "";
  if (api_query(q, "next", raw_next, sizeof raw_next) && same_site_next(raw_next)) snprintf(next_path, sizeof next_path, "%s", raw_next);
  else if (!is_link) snprintf(next_path, sizeof next_path, "/");

  // Normalize friendly provider aliases.
  if (!strcasecmp(provider, "google")) snprintf(provider, sizeof provider, "GoogleOAuth");
  else if (!strcasecmp(provider, "github")) snprintf(provider, sizeof provider, "GitHubOAuth");
  else if (!strcasecmp(provider, "apple")) snprintf(provider, sizeof provider, "AppleOAuth");
  else if (!strcasecmp(provider, "sso")) provider[0] = 0;

  // Account-linking mode (used by /onboarding to connect GitHub to the active acct_ session).
  char link_account[40] = "";
  if (is_link) {
    if (strcmp(provider, "GitHubOAuth") != 0) {
      auth_refuse(q, 400, "Only GitHub account linking is supported.");
      return;
    }
    const auth_principal *p = auth_principal_of(q);
    if (!p || p->kind != AP_PERSON || !p->account[0]) {
      if (!strcmp(mode, "json") || strstr(q->query, "as=lines")) {
        auth_refuse(q, 401, "Log in first.");
      } else {
        api_respond(q->fd, 303, "text/plain", "Location: /login?next=/onboarding\r\n", "", 0);
      }
      return;
    }
    snprintf(link_account, sizeof link_account, "%s", p->account);
    domain[0] = 0; conn_id[0] = 0; org_id[0] = 0; login_hint[0] = 0;
  }

  // Reject any unsupported OAuth/phone provider attempts explicitly (passwords use /password/*).
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
  const char *v[6] = {id, is_link ? "oauth_link" : "oauth", ssha, (const char *)verifier, next_path, is_link ? link_account : NULL};
  PGresult *r = PQexecParams(
      db,
      "INSERT INTO auth.workos_pending (id, kind, token_sha256, verifier, next_path, link_account, client, expires_at) "
      "VALUES ($1, $2, $3, $4, $5, $6, 'web', now() + interval '10 minutes')",
      6, NULL, v, NULL, NULL, 0);
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
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "That login method isn't enabled here."); return; }
  char code[512] = "", state[128] = "", err_param[128] = "", cookie_state[128] = "";
  api_query(q, "code", code, sizeof code);
  api_query(q, "state", state, sizeof state);
  api_query(q, "error", err_param, sizeof err_param);
  auth_cookie(q, WOS_STATE_COOKIE, cookie_state, sizeof cookie_state);

  if (err_param[0] || !code[0] || !state[0] || !cookie_state[0] ||
      !wos_state_equal(cs(cookie_state), cs(state))) {
    bool is_link_err = workos_pending_is_link(db, state, cookie_state);
    const auth_principal *ep = auth_principal_of(q);
    char d[256];
    snprintf(d, sizeof d, "err=%s has_code=%d has_state=%d has_cookie=%d state_eq=%d is_link=%d",
             err_param[0] ? err_param : "-", code[0] ? 1 : 0, state[0] ? 1 : 0, cookie_state[0] ? 1 : 0,
             (state[0] && cookie_state[0] && wos_state_equal(cs(cookie_state), cs(state))) ? 1 : 0,
             is_link_err ? 1 : 0);
    workos_log_err(db, ep ? ep->account : NULL, ep ? ep->session : NULL,
                   is_link_err ? "onboarding.github_link" : "login.workos_oauth", "callback_params", 400, d);
    const char *dest = is_link_err
      ? (!strcmp(err_param, "access_denied") ? "/onboarding?error=github_cancelled" : "/onboarding?error=github_link_failed")
      : "/login?error=oauth_failed";
    char hdr[320];
    snprintf(hdr, sizeof hdr,
             "Location: %s\r\n"
             "Set-Cookie: %s=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n",
             dest, WOS_STATE_COOKIE);
    api_respond(q->fd, 303, "text/plain", hdr, "", 0);
    return;
  }

  char ssha[65], pending_kind[24] = "", verifier[128] = "", next_path[256] = "/", link_account[40] = "";
  auth_sha_hex(state, ssha);
  const char *v[1] = {ssha};
  PGresult *r = PQexecParams(
      db,
      "UPDATE auth.workos_pending SET spent_at = now() "
      "WHERE kind IN ('oauth', 'oauth_link') AND token_sha256 = $1 AND spent_at IS NULL AND expires_at > now() "
      "RETURNING kind, coalesce(verifier, ''), coalesce(next_path, '/'), coalesce(link_account, '')",
      1, NULL, v, NULL, NULL, 0);
  bool found = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  if (found) {
    snprintf(pending_kind, sizeof pending_kind, "%s", PQgetvalue(r, 0, 0));
    snprintf(verifier, sizeof verifier, "%s", PQgetvalue(r, 0, 1));
    if (same_site_next(PQgetvalue(r, 0, 2))) snprintf(next_path, sizeof next_path, "%s", PQgetvalue(r, 0, 2));
    snprintf(link_account, sizeof link_account, "%s", PQgetvalue(r, 0, 3));
  }
  PQclear(r);
  if (!found || !verifier[0]) {
    const auth_principal *ep = auth_principal_of(q);
    workos_log_err(db, ep ? ep->account : NULL, ep ? ep->session : NULL, "login.workos_oauth", "pending_expired", 401, "workos_pending missing or expired");
    auth_refuse(q, 401, "That sign-in expired. Start again.");
    return;
  }

  bool is_link = !strcmp(pending_kind, "oauth_link");
  const auth_principal *link_p = NULL;
  if (is_link) {
    link_p = auth_principal_of(q);
    if (!link_p || link_p->kind != AP_PERSON || !link_account[0] || strcmp(link_p->account, link_account) != 0) {
      char d[192];
      snprintf(d, sizeof d, "has_principal=%d expected_acct=%s got_acct=%s",
               link_p ? 1 : 0, link_account[0] ? link_account : "-", (link_p && link_p->account[0]) ? link_p->account : "-");
      workos_log_err(db, link_p ? link_p->account : link_account, link_p ? link_p->session : NULL, "onboarding.github_link", "session_mismatch", 401, d);
      explicit_bzero(verifier, sizeof verifier);
      char hdr[320];
      snprintf(hdr, sizeof hdr,
               "Location: /login?next=/onboarding\r\n"
               "Set-Cookie: %s=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n",
               WOS_STATE_COOKIE);
      api_respond(q->fd, 303, "text/plain", hdr, "", 0);
      return;
    }
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

  if (is_link) {
    if (res.status >= 200 && res.status < 300 && res.body && res.body_len > 0) {
      char wos_uid[128] = "";
      const char *uv[1] = {res.body};
      PGresult *ur = PQexecParams(db, "SELECT coalesce(($1::jsonb)->'user'->>'id', '')", 1, NULL, uv, NULL, NULL, 0);
      if (PQresultStatus(ur) == PGRES_TUPLES_OK && PQntuples(ur) == 1) snprintf(wos_uid, sizeof wos_uid, "%s", PQgetvalue(ur, 0, 0));
      PQclear(ur);

      char gh_id[64] = "", gh_login[64] = "";
      long long newly = 0;
      bool already_claimed = false;
      bool linked = workos_link_github_account(db, link_account, wos_uid, res.body, gh_id, gh_login, &newly, &already_claimed);
      if (res.body && res.body_len > 0) explicit_bzero(res.body, res.body_len);
      http_client_res_free(&res);
      if (linked) {
        char detail[256], login_esc[128];
        auth_json_esc(gh_login, login_esc, sizeof login_esc);
        snprintf(detail, sizeof detail, "{\"github_id\":\"%s\",\"github_login\":\"%s\",\"granted_micros\":%lld,\"already_claimed\":%s}",
                 gh_id, login_esc, newly, already_claimed ? "true" : "false");
        auth_audit(db, link_account, link_p->session, "onboarding.github_link", "ok", detail);
        char dest[320] = "/chat";
        int64_t dn = ap_post_login_dest(true, cs(next_path), (coax_v_u8){(uint8_t *)dest, (int64_t)sizeof dest - 1});
        if (dn > 0) dest[dn] = 0;
        else snprintf(dest, sizeof dest, "/chat");
        char hdr[600];
        snprintf(hdr, sizeof hdr,
                 "Location: %s\r\n"
                 "Set-Cookie: %s=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n",
                 dest, WOS_STATE_COOKIE);
        api_respond(q->fd, 303, "text/plain", hdr, "", 0);
        return;
      }
    } else {
      // Log exact WorkOS error code/message (never secrets/tokens) so Cloud Logging + auth.audit show root cause.
      char werr[256] = "workos_auth_failed", wcode[96] = "";
      if (res.body && res.body_len > 0) {
        const char *ev[1] = {res.body};
        PGresult *er = PQexecParams(
            db,
            "WITH d AS (SELECT $1::jsonb j) "
            "SELECT coalesce(j->>'code', j->>'error', ''), coalesce(j->>'message', j->>'error_description', '') FROM d",
            1, NULL, ev, NULL, NULL, 0);
        if (PQresultStatus(er) == PGRES_TUPLES_OK && PQntuples(er) == 1) {
          snprintf(wcode, sizeof wcode, "%s", PQgetvalue(er, 0, 0));
          snprintf(werr, sizeof werr, "code=%s msg=%.160s", wcode[0] ? wcode : "-", PQgetvalue(er, 0, 1));
        }
        PQclear(er);
      }
      workos_log_err(db, link_account, link_p->session, "onboarding.github_link", "workos_authenticate", res.status, werr);
      // If WorkOS returned 403 organization_selection_required, user.id is already authenticated — still link GitHub!
      if (res.status == 403 && res.body && res.body_len > 0 && !strcmp(wcode, "organization_selection_required")) {
        char wos_uid[128] = "";
        const char *uv[1] = {res.body};
        PGresult *ur = PQexecParams(db, "SELECT coalesce(($1::jsonb)->'user'->>'id', '')", 1, NULL, uv, NULL, NULL, 0);
        if (PQresultStatus(ur) == PGRES_TUPLES_OK && PQntuples(ur) == 1) snprintf(wos_uid, sizeof wos_uid, "%s", PQgetvalue(ur, 0, 0));
        PQclear(ur);
        if (wos_uid[0]) {
          char gh_id[64] = "", gh_login[64] = "";
          long long newly = 0;
          bool already_claimed = false;
          bool linked = workos_link_github_account(db, link_account, wos_uid, res.body, gh_id, gh_login, &newly, &already_claimed);
          explicit_bzero(res.body, res.body_len);
          http_client_res_free(&res);
          if (linked) {
            char detail[256], login_esc[128];
            auth_json_esc(gh_login, login_esc, sizeof login_esc);
            snprintf(detail, sizeof detail, "{\"github_id\":\"%s\",\"github_login\":\"%s\",\"granted_micros\":%lld,\"already_claimed\":%s,\"via\":\"org_select_403\"}",
                     gh_id, login_esc, newly, already_claimed ? "true" : "false");
            auth_audit(db, link_account, link_p->session, "onboarding.github_link", "ok", detail);
            char dest[320] = "/chat";
            int64_t dn = ap_post_login_dest(true, cs(next_path), (coax_v_u8){(uint8_t *)dest, (int64_t)sizeof dest - 1});
            if (dn > 0) dest[dn] = 0;
            else snprintf(dest, sizeof dest, "/chat");
            char hdr[600];
            snprintf(hdr, sizeof hdr,
                     "Location: %s\r\n"
                     "Set-Cookie: %s=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n",
                     dest, WOS_STATE_COOKIE);
            api_respond(q->fd, 303, "text/plain", hdr, "", 0);
            return;
          }
        }
      }
      if (res.body && res.body_len > 0) explicit_bzero(res.body, res.body_len);
      http_client_res_free(&res);
    }
    char hdr[320];
    snprintf(hdr, sizeof hdr,
             "Location: /onboarding?error=github_link_failed\r\n"
             "Set-Cookie: %s=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n",
             WOS_STATE_COOKIE);
    api_respond(q->fd, 303, "text/plain", hdr, "", 0);
    return;
  }

  if (res.status >= 200 && res.status < 300 && res.body && res.body_len > 0) {
    char wos_uid[128], email[300], org_id[128], wos_sid[128], method[64];
    bool email_verified = false;
    mirror_auth_response(db, res.body, wos_uid, email, &email_verified, org_id, wos_sid, method);
    char acct[40]; bool made = false, auto_linked = false;
    if (!wos_uid[0] || !resolve_workos_account(db, wos_uid, email, email_verified, method, org_id, acct, &made, &auto_linked)) {
      if (res.body && res.body_len > 0) explicit_bzero(res.body, res.body_len);
      http_client_res_free(&res);
      auth_refuse(q, 500, "That didn't work. Try again.");
      return;
    }
    referral_maybe_redeem_cookie(q, db, acct);
    if (strstr(method, "GitHub") || strstr(method, "github")) {
      if (!workos_link_github_account(db, acct, wos_uid, res.body, NULL, NULL, NULL, NULL)) {
        referral_on_github_connected(db, acct);
      }
    }
    if (res.body && res.body_len > 0) explicit_bzero(res.body, res.body_len);
    http_client_res_free(&res);
    bool has_gh = auth_account_has_github(db, acct, NULL, 0);
    char dest[320] = "/onboarding";
    int64_t dn = ap_post_login_dest(has_gh, cs(next_path), (coax_v_u8){(uint8_t *)dest, (int64_t)sizeof dest - 1});
    if (dn > 0) dest[dn] = 0;
    else snprintf(dest, sizeof dest, "%s", has_gh ? "/chat" : "/onboarding");
    auth_login_web_redirect(q, db, acct, "workos", "L1", made, "login.workos_oauth", dest);
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
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "That login method isn't enabled here."); return; }
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
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "That login method isn't enabled here."); return; }
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
  if (!workos_enabled_for_request(q) || !workos_configured()) { auth_refuse(q, 503, "That login method isn't enabled here."); return; }
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
      auth_refuse(q, 502, "Could not verify session token.");
      return;
    }
    coax_t_WosJwtClaims claims;
    memset(&claims, 0, sizeof claims);
    uint32_t jerr = wos_jwt_verify_session(cs(access_token), (coax_string){jres.body, (int64_t)jres.body_len},
                                           cs(""), (int64_t)time(NULL), (coax_v_t_WosJwtClaims){&claims, 1});
    http_client_res_free(&jres);
    if (jerr != WOS_JWT_OK || claims.sub_n <= 0) {
      auth_refuse(q, 401, "That login token isn't valid.");
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

  auth_refuse(q, 400, "Provide a PKCE code or access_token.");
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

// Idempotent event ingestion for both webhooks and Events API polling (plain SQL; zero PL/pgSQL).
static bool workos_upsert_event(PGconn *db, const char *ev_id, const char *ev_type, const char *raw_json, bool *applied_out) {
  *applied_out = false;
  if (!ev_id || !*ev_id || !ev_type || !*ev_type || !raw_json) return false;
  PGresult *tx = PQexec(db, "BEGIN");
  if (!tx || PQresultStatus(tx) != PGRES_COMMAND_OK) { if (tx) PQclear(tx); return false; }
  PQclear(tx);

  const char *iv[3] = {ev_id, ev_type, raw_json};
  PGresult *ir = PQexecParams(
      db,
      "INSERT INTO workos.events (id, event, _raw) VALUES ($1, $2, $3::jsonb) ON CONFLICT (id) DO NOTHING RETURNING id",
      3, NULL, iv, NULL, NULL, 0);
  if (!ir || PQresultStatus(ir) != PGRES_TUPLES_OK) {
    if (ir) PQclear(ir);
    PQclear(PQexec(db, "ROLLBACK"));
    return false;
  }
  if (PQntuples(ir) == 0) {
    PQclear(ir);
    PQclear(PQexec(db, "COMMIT"));
    *applied_out = false;
    return true;
  }
  PQclear(ir);

  const char *sql = NULL;
  if (!strcmp(ev_type, "user.created") || !strcmp(ev_type, "user.updated")) {
    sql = "INSERT INTO workos.users (id, _raw, updated_at, deleted_at) "
          "SELECT d->>'id', d, now(), NULL FROM (SELECT ($1::jsonb)->'data' d) s WHERE d IS NOT NULL AND coalesce(d->>'id', '') <> '' "
          "ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = NULL";
  } else if (!strcmp(ev_type, "user.deleted")) {
    sql = "INSERT INTO workos.users (id, _raw, updated_at, deleted_at) "
          "SELECT d->>'id', d, now(), now() FROM (SELECT ($1::jsonb)->'data' d) s WHERE d IS NOT NULL AND coalesce(d->>'id', '') <> '' "
          "ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = coalesce(workos.users.deleted_at, now())";
  } else if (!strcmp(ev_type, "organization.created") || !strcmp(ev_type, "organization.updated")) {
    sql = "INSERT INTO workos.organizations (id, _raw, updated_at, deleted_at) "
          "SELECT d->>'id', d, now(), NULL FROM (SELECT ($1::jsonb)->'data' d) s WHERE d IS NOT NULL AND coalesce(d->>'id', '') <> '' "
          "ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = NULL";
  } else if (!strcmp(ev_type, "organization.deleted")) {
    sql = "INSERT INTO workos.organizations (id, _raw, updated_at, deleted_at) "
          "SELECT d->>'id', d, now(), now() FROM (SELECT ($1::jsonb)->'data' d) s WHERE d IS NOT NULL AND coalesce(d->>'id', '') <> '' "
          "ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = coalesce(workos.organizations.deleted_at, now())";
  } else if (!strcmp(ev_type, "organization_membership.created") || !strcmp(ev_type, "organization_membership.updated")) {
    sql = "INSERT INTO workos.organization_memberships (id, _raw, updated_at, deleted_at) "
          "SELECT d->>'id', d, now(), NULL FROM (SELECT ($1::jsonb)->'data' d) s WHERE d IS NOT NULL AND coalesce(d->>'id', '') <> '' "
          "ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = NULL";
  } else if (!strcmp(ev_type, "organization_membership.deleted")) {
    sql = "INSERT INTO workos.organization_memberships (id, _raw, updated_at, deleted_at) "
          "SELECT d->>'id', d, now(), now() FROM (SELECT ($1::jsonb)->'data' d) s WHERE d IS NOT NULL AND coalesce(d->>'id', '') <> '' "
          "ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = coalesce(workos.organization_memberships.deleted_at, now())";
  } else if (!strcmp(ev_type, "session.created")) {
    sql = "INSERT INTO workos.sessions (id, _raw, updated_at, deleted_at) "
          "SELECT d->>'id', d, now(), NULL FROM (SELECT ($1::jsonb)->'data' d) s WHERE d IS NOT NULL AND coalesce(d->>'id', '') <> '' "
          "ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = NULL";
  } else if (!strcmp(ev_type, "session.revoked")) {
    sql = "INSERT INTO workos.sessions (id, _raw, updated_at, deleted_at) "
          "SELECT d->>'id', d, now(), now() FROM (SELECT ($1::jsonb)->'data' d) s WHERE d IS NOT NULL AND coalesce(d->>'id', '') <> '' "
          "ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = coalesce(workos.sessions.deleted_at, now())";
  }
  if (sql) {
    const char *rv[1] = {raw_json};
    PGresult *mr = PQexecParams(db, sql, 1, NULL, rv, NULL, NULL, 0);
    if (!mr || PQresultStatus(mr) != PGRES_COMMAND_OK) {
      if (mr) PQclear(mr);
      PQclear(PQexec(db, "ROLLBACK"));
      return false;
    }
    PQclear(mr);
  }

  const char *av[1] = {ev_id};
  PGresult *ar = PQexecParams(db, "UPDATE workos.events SET applied_at = now() WHERE id = $1", 1, NULL, av, NULL, NULL, 0);
  if (!ar || PQresultStatus(ar) != PGRES_COMMAND_OK) {
    if (ar) PQclear(ar);
    PQclear(PQexec(db, "ROLLBACK"));
    return false;
  }
  PQclear(ar);

  PGresult *cm = PQexec(db, "COMMIT");
  bool ok = cm && PQresultStatus(cm) == PGRES_COMMAND_OK;
  if (cm) PQclear(cm);
  if (!ok) { PQclear(PQexec(db, "ROLLBACK")); return false; }
  *applied_out = true;
  return true;
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
      PQclear(PQexecParams(
          db,
          "WITH bump AS (UPDATE auth.accounts SET session_epoch = session_epoch + 1 WHERE id = $1) "
          "DELETE FROM auth.sessions WHERE account = $1",
          1, NULL, av, NULL, NULL, 0));
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
      "SELECT coalesce(($1::jsonb)->>'id', ''), coalesce(($1::jsonb)->>'event', '')",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || !PQgetvalue(r, 0, 0)[0] || !PQgetvalue(r, 0, 1)[0]) {
    PQclear(r);
    auth_refuse(q, 400, "Malformed webhook event.");
    return;
  }
  char ev_id[128], ev_type[128];
  snprintf(ev_id, sizeof ev_id, "%s", PQgetvalue(r, 0, 0));
  snprintf(ev_type, sizeof ev_type, "%s", PQgetvalue(r, 0, 1));
  PQclear(r);
  bool applied = false;
  if (!workos_upsert_event(db, ev_id, ev_type, body, &applied)) {
    auth_refuse(q, 400, "Malformed webhook event.");
    return;
  }
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
      bool did = false;
      if (workos_upsert_event(db, eid, etype, eraw, &did) && did) {
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
  else if (!strcmp(sub, "password/login")) route_password_login(q, db, body);
  else if (!strcmp(sub, "password/register")) route_password_register(q, db, body);
  else if (!strcmp(sub, "password/reset")) route_password_reset(q, db, body);
  else if (!strcmp(sub, "password/reset/confirm")) route_password_reset_confirm(q, db, body);
  else if (!strcmp(sub, "org/select")) route_org_select(q, db, body);
  else if (!strcmp(sub, "sso/discover")) route_sso_discover(q, db, body);
  else if (!strcmp(sub, "device/start")) route_device_start(q, db);
  else if (!strcmp(sub, "exchange")) route_exchange(q, db, body);
  else if (!strcmp(sub, "orgs/switch")) route_orgs_switch(q, db, body);
  else if (!strcmp(sub, "webhook")) route_webhook(q, db, body);
  else if (!strcmp(sub, "action")) route_action(q, db, body);
  else { explicit_bzero(body, q->body_len); free(body); return false; }
  explicit_bzero(body, q->body_len);
  free(body);
  return true;
}
