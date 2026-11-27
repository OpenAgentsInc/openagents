// Referral & Promo Credit HTTP API + promo spend / activation / kickback helpers.
// Policy decisions: pay/referral-policy.coax (#authority, 0 surviving mutants).
// Wire serialization/parsing: coax/web/api/referral-wire.coax (0 surviving mutants).
// Pure parameterized SQL + pg_advisory_xact_lock (ZERO PL/pgSQL functions).
#include "api.h"
#include "auth-internal.h"
#include "http_client.h"
#include "workos.h"
#include "referral-policy.generated.h"
#include "referral-wire.generated.h"

#include <ctype.h>
#include <openssl/rand.h>
#include <openssl/sha.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static coax_string rf_cs(const char *s) {
  return (coax_string){s ? s : "", (int64_t)(s ? strlen(s) : 0)};
}

static const char *rf_origin(void) {
  const char *v = getenv("OA_API_ORIGIN");
  return (v && *v) ? v : "https://openagents.com";
}

static const char *rf_github_api_base(void) {
  const char *v = getenv("OA_GITHUB_API_BASE");
  return (v && *v) ? v : "https://api.github.com";
}

static bool rf_github_allow_local(void) {
  const char *b = rf_github_api_base();
  return !strncmp(b, "http://127.0.0.1:", 17) || !strncmp(b, "http://localhost:", 17);
}

static void rf_sha256_hex(const char *data, size_t len, char out[65]) {
  unsigned char dig[32];
  SHA256((const unsigned char *)data, len, dig);
  static const char hex[] = "0123456789abcdef";
  for (int i = 0; i < 32; i++) {
    out[i * 2] = hex[(dig[i] >> 4) & 0xf];
    out[i * 2 + 1] = hex[dig[i] & 0xf];
  }
  out[64] = 0;
}

static void rf_code_hash(const char *code, char out[65]) {
  char low[96];
  size_t n = strlen(code);
  if (n >= sizeof low) n = sizeof low - 1;
  for (size_t i = 0; i < n; i++) low[i] = (char)tolower((unsigned char)code[i]);
  low[n] = 0;
  rf_sha256_hex(low, n, out);
}

// Coarse network prefix hash (IPv4 /24 or IPv6 first 3 groups) so raw IPs are never stored.
static void rf_ip_prefix_hash(const request *q, char out[65]) {
  char ip[128] = "";
  auth_visitor(q, ip);
  if (!ip[0]) { out[0] = 0; return; }
  char prefix[128] = "";
  const char *dot1 = strchr(ip, '.');
  if (dot1) {
    const char *dot2 = strchr(dot1 + 1, '.');
    const char *dot3 = dot2 ? strchr(dot2 + 1, '.') : NULL;
    size_t k = dot3 ? (size_t)(dot3 - ip) : strlen(ip);
    if (k >= sizeof prefix) k = sizeof prefix - 1;
    memcpy(prefix, ip, k); prefix[k] = 0;
  } else {
    // IPv6: keep first 3 colon-separated groups
    int colons = 0;
    size_t k = 0;
    while (ip[k] && k + 1 < sizeof prefix) {
      if (ip[k] == ':' && ++colons >= 3) break;
      prefix[k] = ip[k];
      k++;
    }
    prefix[k] = 0;
  }
  rf_sha256_hex(prefix, strlen(prefix), out);
}

static void rf_header_hash(const request *q, const char *hdr_name, char out[65]) {
  char val[256] = "";
  if (!api_header(q, hdr_name, val, sizeof val) || !val[0]) { out[0] = 0; return; }
  rf_sha256_hex(val, strlen(val), out);
}

static void rf_respond_json(int fd, int status, const void *data, size_t n) {
  api_respond(fd, status, "application/json", "Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n", data, n);
}

static void rf_respond_lines(int fd, int status, const void *data, size_t n) {
  api_respond(fd, status, "text/plain; charset=utf-8", "Cache-Control: private, no-store\r\nVary: Cookie, Authorization\r\n", data, n);
}

// ---- config & account resolution

typedef struct {
  long long github_connect_micros;
  long long invitee_micros;
  long long inviter_micros;
  int kickback_bps;
  int gh_min_age_days;
  int gh_min_repos;
  int gh_min_followers;
  int inviter_cap_30d;
  long long promo_daily_cap_micros;
  int promo_expiry_days;
  int kickback_inactivity_days;
  int redeem_window_days;
  int activation_days_needed;
  int activation_window_days;
  long long daily_budget_micros;
  long long total_budget_micros;
  char promo_models[512];
  bool flag_redeem;
  bool flag_grants;
  bool flag_kickback;
  bool flag_github_connect;
  int terms_version;
} rf_config;

static bool rf_load_config(PGconn *db, bool for_update, rf_config *c) {
  memset(c, 0, sizeof *c);
  const char *sql = for_update
    ? "SELECT github_connect_micros, invitee_micros, inviter_micros, kickback_bps, "
      "gh_min_age_days, gh_min_repos, gh_min_followers, inviter_cap_30d, "
      "promo_daily_cap_micros, promo_expiry_days, kickback_inactivity_days, "
      "redeem_window_days, activation_days_needed, activation_window_days, "
      "daily_budget_micros, total_budget_micros, promo_models, "
      "flag_redeem, flag_grants, flag_kickback, flag_github_connect, terms_version "
      "FROM referral.config WHERE id = 1 FOR UPDATE"
    : "SELECT github_connect_micros, invitee_micros, inviter_micros, kickback_bps, "
      "gh_min_age_days, gh_min_repos, gh_min_followers, inviter_cap_30d, "
      "promo_daily_cap_micros, promo_expiry_days, kickback_inactivity_days, "
      "redeem_window_days, activation_days_needed, activation_window_days, "
      "daily_budget_micros, total_budget_micros, promo_models, "
      "flag_redeem, flag_grants, flag_kickback, flag_github_connect, terms_version "
      "FROM referral.config WHERE id = 1";
  PGresult *r = PQexec(db, sql);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    if (r) PQclear(r);
    return false;
  }
  c->github_connect_micros = atoll(PQgetvalue(r, 0, 0));
  c->invitee_micros = atoll(PQgetvalue(r, 0, 1));
  c->inviter_micros = atoll(PQgetvalue(r, 0, 2));
  c->kickback_bps = atoi(PQgetvalue(r, 0, 3));
  c->gh_min_age_days = atoi(PQgetvalue(r, 0, 4));
  c->gh_min_repos = atoi(PQgetvalue(r, 0, 5));
  c->gh_min_followers = atoi(PQgetvalue(r, 0, 6));
  c->inviter_cap_30d = atoi(PQgetvalue(r, 0, 7));
  c->promo_daily_cap_micros = atoll(PQgetvalue(r, 0, 8));
  c->promo_expiry_days = atoi(PQgetvalue(r, 0, 9));
  c->kickback_inactivity_days = atoi(PQgetvalue(r, 0, 10));
  c->redeem_window_days = atoi(PQgetvalue(r, 0, 11));
  c->activation_days_needed = atoi(PQgetvalue(r, 0, 12));
  c->activation_window_days = atoi(PQgetvalue(r, 0, 13));
  c->daily_budget_micros = atoll(PQgetvalue(r, 0, 14));
  c->total_budget_micros = atoll(PQgetvalue(r, 0, 15));
  snprintf(c->promo_models, sizeof c->promo_models, "%s", PQgetvalue(r, 0, 16));
  c->flag_redeem = !strcmp(PQgetvalue(r, 0, 17), "t");
  c->flag_grants = !strcmp(PQgetvalue(r, 0, 18), "t");
  c->flag_kickback = !strcmp(PQgetvalue(r, 0, 19), "t");
  c->flag_github_connect = !strcmp(PQgetvalue(r, 0, 20), "t");
  c->terms_version = atoi(PQgetvalue(r, 0, 21));
  PQclear(r);
  return true;
}

static bool rf_advisory_lock(PGconn *db, const char *key) {
  const char *v[1] = {key};
  PGresult *r = PQexecParams(db, "SELECT pg_advisory_xact_lock(hashtext($1))", 1, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK;
  PQclear(r);
  return ok;
}

// Resolve billing identity (acct_... or 64-hex pubkey) to auth.accounts.id.
static bool rf_resolve_account(PGconn *db, const char *who, char acct_out[48]) {
  acct_out[0] = 0;
  if (!who || !*who) return false;
  const char *v[1] = {who};
  PGresult *r = PQexecParams(
      db,
      "SELECT id FROM auth.accounts WHERE id = $1 "
      "UNION ALL "
      "SELECT account FROM auth.nostr_keys WHERE pubkey = $1 "
      "LIMIT 1",
      1, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  if (ok) snprintf(acct_out, 48, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  return ok;
}

static bool rf_account_frozen(PGconn *db, const char *acct) {
  if (!acct || !*acct) return false;
  const char *v[1] = {acct};
  PGresult *r = PQexecParams(db, "SELECT frozen_at IS NOT NULL OR deleted_at IS NOT NULL FROM auth.accounts WHERE id = $1", 1, NULL, v, NULL, NULL, 0);
  bool fr = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t");
  PQclear(r);
  return fr;
}

// Lazily ensure an account has its permanent 8-char Crockford base32 personal referral code.
static bool rf_ensure_personal_code(PGconn *db, const char *acct, char code_out[16]) {
  code_out[0] = 0;
  const char *v[1] = {acct};
  PGresult *r = PQexecParams(db, "SELECT code_text FROM referral.codes WHERE owner_account_id = $1 AND kind = 'personal'", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    snprintf(code_out, 16, "%s", PQgetvalue(r, 0, 0));
    PQclear(r);
    return true;
  }
  PQclear(r);

  for (int attempt = 0; attempt < 32; attempt++) {
    unsigned char rnd[5];
    if (RAND_bytes(rnd, sizeof rnd) != 1) return false;
    uint8_t cb[9] = {0};
    int64_t cn = rp_code_from_bytes((coax_v_u8){rnd, 5}, (coax_v_u8){cb, 8});
    if (cn != 8) continue;
    cb[8] = 0;
    char chash[65];
    rf_code_hash((const char *)cb, chash);
    char hint[8];
    snprintf(hint, sizeof hint, "%.4s", (const char *)cb);
    const char *ins[4] = {chash, (const char *)cb, hint, acct};
    PGresult *ir = PQexecParams(
        db,
        "INSERT INTO referral.codes (code_hash, code_text, code_hint, kind, owner_account_id) "
        "VALUES ($1, $2, $3, 'personal', $4) "
        "ON CONFLICT DO NOTHING RETURNING code_text",
        4, NULL, ins, NULL, NULL, 0);
    if (PQresultStatus(ir) == PGRES_TUPLES_OK && PQntuples(ir) == 1) {
      snprintf(code_out, 16, "%s", PQgetvalue(ir, 0, 0));
      PQclear(ir);
      return true;
    }
    PQclear(ir);
    // Check if another concurrent request created the account's row
    r = PQexecParams(db, "SELECT code_text FROM referral.codes WHERE owner_account_id = $1 AND kind = 'personal'", 1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
      snprintf(code_out, 16, "%s", PQgetvalue(r, 0, 0));
      PQclear(r);
      return true;
    }
    PQclear(r);
  }
  return false;
}

// Public helper: total unexpired promo credit balance and nearest expiry ISO-8601 UTC.
// Accepts either an auth.accounts.id (acct_...) or a 64-hex Nostr pubkey.
void referral_promo_balance(PGconn *db, const char *account_or_key, long long *out_micros, char out_expires[40]) {
  if (out_micros) *out_micros = 0;
  if (out_expires) out_expires[0] = 0;
  if (!db || !account_or_key || !*account_or_key) return;
  char resolved[48];
  const char *acct = account_or_key;
  if (strncmp(account_or_key, "acct_", 5) != 0) {
    if (!rf_resolve_account(db, account_or_key, resolved)) return;
    acct = resolved;
  }
  const char *v[1] = {acct};
  PGresult *r = PQexecParams(
      db,
      "SELECT coalesce(sum(remaining_micros), 0)::bigint, "
      "coalesce(to_char(min(expires_at) AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'), '') "
      "FROM inference.promo_credit "
      "WHERE account_id = $1 AND remaining_micros > 0 AND (expires_at IS NULL OR expires_at > now())",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    if (out_micros) *out_micros = atoll(PQgetvalue(r, 0, 0));
    if (out_expires) snprintf(out_expires, 40, "%s", PQgetvalue(r, 0, 1));
  }
  if (r) PQclear(r);
}

static long long rf_promo_spent_today(PGconn *db, const char *acct) {
  const char *v[1] = {acct};
  PGresult *r = PQexecParams(
      db,
      "SELECT coalesce(spent_micros, 0) FROM inference.promo_spend_day "
      "WHERE account_id = $1 AND utc_day = (now() AT TIME ZONE 'UTC')::date",
      1, NULL, v, NULL, NULL, 0);
  long long s = (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) ? atoll(PQgetvalue(r, 0, 0)) : 0;
  PQclear(r);
  return s;
}

// ---- GitHub eligibility lookup & grant evaluation (inside a transaction)

typedef struct {
  bool has_row;
  bool eligible;
  bool denied;
  char github_id[64];
  char login[64];
  char avatar[256];
  int age_days;
  int repos;
  int followers;
  int orgs;
  int verdict;
  char reason[160];
} rf_gh_state;

static void rf_format_gh_reason(int verdict, int age_days, const rf_config *cfg, char out[160]) {
  out[0] = 0;
  if (verdict == 0) return; // RP_ELIG_OK
  if (verdict == 1) {
    snprintf(out, 160, "Your GitHub account is %d days old; rewards unlock at %d.", age_days, cfg->gh_min_age_days);
  } else if (verdict == 2) {
    snprintf(out, 160, "GitHub needs at least %d non-fork public repos, %d followers, or a public org.",
             cfg->gh_min_repos, cfg->gh_min_followers);
  } else if (verdict == 3) {
    snprintf(out, 160, "This GitHub identity has already claimed rewards on another account.");
  } else if (verdict == 4) {
    snprintf(out, 160, "This GitHub identity is not eligible for promo rewards.");
  }
}

static void rf_load_gh_state(PGconn *db, const char *acct, const rf_config *cfg, rf_gh_state *st) {
  memset(st, 0, sizeof *st);
  st->verdict = 1; // default ineligible until proven
  const char *v[1] = {acct};
  PGresult *r = PQexecParams(
      db,
      "SELECT github_id_key, github_login, github_avatar, "
      "coalesce(floor(extract(epoch FROM (now() - github_created_at)) / 86400)::int, 0), "
      "repos_count, followers_count, orgs_count, eligible, denied "
      "FROM referral.github_claims WHERE account_id = $1 ORDER BY eligible DESC, claimed_at ASC LIMIT 1",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    st->has_row = true;
    snprintf(st->github_id, sizeof st->github_id, "%s", PQgetvalue(r, 0, 0));
    snprintf(st->login, sizeof st->login, "%s", PQgetvalue(r, 0, 1));
    snprintf(st->avatar, sizeof st->avatar, "%s", PQgetvalue(r, 0, 2));
    st->age_days = atoi(PQgetvalue(r, 0, 3));
    st->repos = atoi(PQgetvalue(r, 0, 4));
    st->followers = atoi(PQgetvalue(r, 0, 5));
    st->orgs = atoi(PQgetvalue(r, 0, 6));
    st->denied = !strcmp(PQgetvalue(r, 0, 8), "t");
    st->verdict = (int)rp_eligible(st->age_days, cfg->gh_min_age_days, st->repos, cfg->gh_min_repos,
                                   st->followers, cfg->gh_min_followers, st->orgs, false, st->denied);
    st->eligible = (st->verdict == 0);
    rf_format_gh_reason(st->verdict, st->age_days, cfg, st->reason);
  }
  PQclear(r);
}

static int rf_kind_code(const char *kind) {
  if (!strcmp(kind, "github_connect")) return 1;
  if (!strcmp(kind, "referral_invitee")) return 2;
  if (!strcmp(kind, "referral_inviter")) return 3;
  if (!strcmp(kind, "kickback")) return 4;
  if (!strcmp(kind, "milestone")) return 5;
  if (!strcmp(kind, "campaign")) return 6;
  return 0;
}

static bool rf_flag_for_kind(int k, const rf_config *cfg) {
  if (k == 1) return cfg->flag_github_connect;
  if (k == 4) return cfg->flag_kickback;
  return cfg->flag_grants;
}

static const char *rf_reason_for_action(int act, const rf_gh_state *gh) {
  switch (act) {
    case 1: return "ok";
    case 2: return "flag_off";
    case 3: return "budget_ceiling";
    case 4: return "held";
    case 5: return gh && gh->verdict == 1 ? "github_too_new" : gh && gh->verdict == 2 ? "github_low_activity" : "github_required";
    case 6: return "awaiting_activation";
    case 10: return "account_frozen";
    case 11: return "github_claimed_or_denied";
    case 12: return "window_expired";
    case 13: return "cap";
    default: return "pending";
  }
}

// Evaluate all pending grants for `acct` inside an active transaction.
// Locks referral.config row for flat budget draws so ceilings are strictly conserved.
static long long rf_eval_pending_grants_tx(PGconn *db, const char *acct, int gh_override_verdict) {
  rf_config cfg;
  if (!rf_load_config(db, true, &cfg)) return 0;

  bool acct_frozen = rf_account_frozen(db, acct);
  rf_gh_state gh;
  rf_load_gh_state(db, acct, &cfg, &gh);
  int gh_verdict = (gh_override_verdict >= 0) ? gh_override_verdict : (gh.has_row ? gh.verdict : 1);

  // Compute current daily & total granted flat budget (excludes kickbacks per §4.5)
  long long day_budget = 0, total_budget = 0;
  PGresult *br = PQexec(
      db,
      "SELECT "
      "coalesce(sum(amount_micros) FILTER (WHERE granted_at >= date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'), 0)::bigint, "
      "coalesce(sum(amount_micros), 0)::bigint "
      "FROM referral.grants WHERE status = 'granted' AND kind <> 'kickback'");
  if (PQresultStatus(br) == PGRES_TUPLES_OK && PQntuples(br) == 1) {
    day_budget = atoll(PQgetvalue(br, 0, 0));
    total_budget = atoll(PQgetvalue(br, 0, 1));
  }
  PQclear(br);

  // Rolling 30-day granted referral_inviter count & per-account cap override
  int inv_30d = 0, inv_cap = cfg.inviter_cap_30d;
  const char *av[1] = {acct};
  PGresult *ir = PQexecParams(
      db,
      "SELECT "
      "(SELECT count(*)::int FROM referral.grants WHERE account_id = $1 AND kind = 'referral_inviter' AND status = 'granted' AND granted_at >= now() - interval '30 days'), "
      "(SELECT inviter_cap_override FROM referral.codes WHERE owner_account_id = $1 AND kind = 'personal' LIMIT 1)",
      1, NULL, av, NULL, NULL, 0);
  if (PQresultStatus(ir) == PGRES_TUPLES_OK && PQntuples(ir) == 1) {
    inv_30d = atoi(PQgetvalue(ir, 0, 0));
    if (!PQgetisnull(ir, 0, 1)) inv_cap = atoi(PQgetvalue(ir, 0, 1));
  }
  PQclear(ir);

  PGresult *gr = PQexecParams(
      db,
      "SELECT g.id, g.kind, g.amount_micros, coalesce(g.attribution_id, ''), "
      "coalesce(floor(extract(epoch FROM (now() - g.created_at)))::bigint, 0), "
      "coalesce(a.held, false), coalesce(a.active_days_count, 0), "
      "coalesce(a.inviter_account_id, ''), coalesce(a.invitee_account_id, '') "
      "FROM referral.grants g "
      "LEFT JOIN referral.attributions a ON a.invitee_account_id = g.attribution_id "
      "WHERE g.account_id = $1 AND g.status = 'pending' "
      "ORDER BY g.created_at ASC, g.id ASC FOR UPDATE OF g",
      1, NULL, av, NULL, NULL, 0);
  if (PQresultStatus(gr) != PGRES_TUPLES_OK) { PQclear(gr); return 0; }

  long long newly_granted = 0;
  int rows = PQntuples(gr);
  for (int i = 0; i < rows; i++) {
    const char *gid_s = PQgetvalue(gr, i, 0);
    const char *kind_s = PQgetvalue(gr, i, 1);
    long long amt = atoll(PQgetvalue(gr, i, 2));
    long long elapsed_s = atoll(PQgetvalue(gr, i, 4));
    bool held = !strcmp(PQgetvalue(gr, i, 5), "t");
    int active_days = atoi(PQgetvalue(gr, i, 6));
    const char *inviter_id = PQgetvalue(gr, i, 7);
    const char *invitee_id = PQgetvalue(gr, i, 8);
    int kcode = rf_kind_code(kind_s);

    bool other_frozen = false;
    if (kcode == 3 && invitee_id[0]) other_frozen = rf_account_frozen(db, invitee_id);
    else if (kcode == 4 && invitee_id[0]) other_frozen = rf_account_frozen(db, invitee_id);
    else if (kcode == 2 && inviter_id[0]) other_frozen = rf_account_frozen(db, inviter_id);

    long long max_win_s = 0;
    if (kcode == 2 || kcode == 3 || kcode == 6) max_win_s = (long long)cfg.activation_window_days * 86400LL;
    else if (kcode == 4) max_win_s = 90LL * 86400LL;

    int act = (int)rp_grant(
        kcode, rf_flag_for_kind(kcode, &cfg), acct_frozen, other_frozen, held,
        gh_verdict, amt,
        day_budget, cfg.daily_budget_micros,
        total_budget, cfg.total_budget_micros,
        inv_30d, inv_cap,
        active_days, cfg.activation_days_needed,
        elapsed_s, max_win_s);

    const char *rsn = rf_reason_for_action(act, &gh);
    if (act == 1) { // RP_ACT_GRANT
      int exp_days = (kcode == 4) ? cfg.kickback_inactivity_days : cfg.promo_expiry_days;
      char exp_s[16], amt_s[32];
      snprintf(exp_s, sizeof exp_s, "%d", exp_days);
      snprintf(amt_s, sizeof amt_s, "%lld", amt);
      const char *uv[3] = {gid_s, rsn, exp_s};
      PGresult *ur = PQexecParams(
          db,
          "UPDATE referral.grants SET status = 'granted', reason = $2, granted_at = now(), "
          "expires_at = now() + make_interval(days => $3::int) WHERE id = $1::bigint",
          3, NULL, uv, NULL, NULL, 0);
      bool uok = PQresultStatus(ur) == PGRES_COMMAND_OK;
      PQclear(ur);
      if (uok) {
        const char *pv[5] = {gid_s, acct, amt_s, kind_s, exp_s};
        PGresult *pr = PQexecParams(
            db,
            "INSERT INTO inference.promo_credit (grant_id, account_id, remaining_micros, original_micros, kind, expires_at) "
            "VALUES ($1::bigint, $2, $3::bigint, $3::bigint, $4, now() + make_interval(days => $5::int)) "
            "ON CONFLICT (grant_id) DO NOTHING",
            5, NULL, pv, NULL, NULL, 0);
        PQclear(pr);
        newly_granted += amt;
        if (kcode != 4) { day_budget += amt; total_budget += amt; }
        if (kcode == 3) inv_30d += 1;
      }
    } else if (act >= 10) { // VOID
      const char *vv[2] = {gid_s, rsn};
      PGresult *vr = PQexecParams(db, "UPDATE referral.grants SET status = 'void', reason = $2, voided_at = now() WHERE id = $1::bigint", 2, NULL, vv, NULL, NULL, 0);
      PQclear(vr);
    } else { // Stay pending with clear reason
      const char *pv[2] = {gid_s, rsn};
      PGresult *pr = PQexecParams(db, "UPDATE referral.grants SET reason = $2 WHERE id = $1::bigint", 2, NULL, pv, NULL, NULL, 0);
      PQclear(pr);
    }
  }
  PQclear(gr);
  return newly_granted;
}

// Check & award one-time milestones (5 -> $10, 25 -> $50, 100 -> $250) for `inviter`.
static void rf_check_milestones_tx(PGconn *db, const char *inviter, const rf_config *cfg) {
  if (!inviter || !*inviter) return;
  const char *v[1] = {inviter};
  PGresult *r = PQexecParams(
      db,
      "SELECT "
      "(SELECT count(*)::int FROM referral.attributions WHERE inviter_account_id = $1 AND activated_at IS NOT NULL AND NOT held), "
      "coalesce((SELECT sum(CASE tier WHEN 5 THEN 1 WHEN 25 THEN 2 WHEN 100 THEN 4 ELSE 0 END)::int FROM referral.milestones WHERE account_id = $1), 0)",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); return; }
  int act_cnt = atoi(PQgetvalue(r, 0, 0));
  int mask = atoi(PQgetvalue(r, 0, 1));
  PQclear(r);

  for (int iter = 0; iter < 3; iter++) {
    int tier = (int)rp_milestone_tier(act_cnt, mask);
    if (tier == 0) break;
    long long amt = (long long)rp_milestone_micros(tier);
    char sref[24], amt_s[32], tv_s[16], tier_s[16];
    snprintf(sref, sizeof sref, "tier_%d", tier);
    snprintf(amt_s, sizeof amt_s, "%lld", amt);
    snprintf(tv_s, sizeof tv_s, "%d", cfg->terms_version);
    snprintf(tier_s, sizeof tier_s, "%d", tier);
    const char *gv[5] = {inviter, sref, amt_s, tv_s, tier_s};
    PGresult *gr = PQexecParams(
        db,
        "INSERT INTO referral.grants (account_id, kind, source_ref, amount_micros, status, reason, terms_version, milestone_tier) "
        "VALUES ($1, 'milestone', $2, $3::bigint, 'pending', 'milestone_reached', $4::int, $5::int) "
        "ON CONFLICT (kind, account_id, source_ref) DO NOTHING RETURNING id",
        5, NULL, gv, NULL, NULL, 0);
    if (PQresultStatus(gr) == PGRES_TUPLES_OK && PQntuples(gr) == 1) {
      const char *gid = PQgetvalue(gr, 0, 0);
      const char *mv[3] = {inviter, tier_s, gid};
      PGresult *mr = PQexecParams(
          db,
          "INSERT INTO referral.milestones (account_id, tier, grant_id) VALUES ($1, $2::int, $3::bigint) ON CONFLICT DO NOTHING",
          3, NULL, mv, NULL, NULL, 0);
      PQclear(mr);
    }
    PQclear(gr);
    mask |= (tier == 5 ? 1 : tier == 25 ? 2 : 4);
  }
}

// Record a UTC active day for an account after a completed 200 inference call.
static void rf_record_active_day(PGconn *db, const char *acct) {
  if (!db || !acct || !*acct) return;
  PGresult *tx = PQexec(db, "BEGIN");
  if (PQresultStatus(tx) != PGRES_COMMAND_OK) { PQclear(tx); return; }
  PQclear(tx);

  char lk[80];
  snprintf(lk, sizeof lk, "ref_act:%s", acct);
  if (!rf_advisory_lock(db, lk)) { PQclear(PQexec(db, "ROLLBACK")); return; }

  const char *v[1] = {acct};
  PGresult *r = PQexecParams(
      db,
      "SELECT coalesce(inviter_account_id, ''), active_days_count, "
      "coalesce(first_active_day::text, ''), coalesce(second_active_day::text, ''), "
      "(now() AT TIME ZONE 'UTC')::date::text, "
      "coalesce(floor(extract(epoch FROM (now() - bound_at)))::bigint, 0), "
      "activated_at IS NOT NULL "
      "FROM referral.attributions WHERE invitee_account_id = $1 FOR UPDATE",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    PQclear(r);
    PQclear(PQexec(db, "COMMIT"));
    return;
  }
  char inviter[48];
  snprintf(inviter, sizeof inviter, "%s", PQgetvalue(r, 0, 0));
  int cnt = atoi(PQgetvalue(r, 0, 1));
  char d1[16], d2[16], today[16];
  snprintf(d1, sizeof d1, "%s", PQgetvalue(r, 0, 2));
  snprintf(d2, sizeof d2, "%s", PQgetvalue(r, 0, 3));
  snprintf(today, sizeof today, "%s", PQgetvalue(r, 0, 4));
  long long bound_age_s = atoll(PQgetvalue(r, 0, 5));
  bool already_act = !strcmp(PQgetvalue(r, 0, 6), "t");
  PQclear(r);

  rf_config cfg;
  if (!rf_load_config(db, false, &cfg)) { PQclear(PQexec(db, "ROLLBACK")); return; }

  bool newly_activated = false;
  if (!d1[0]) {
    const char *u[2] = {acct, today};
    PGresult *ur = PQexecParams(db, "UPDATE referral.attributions SET first_active_day = $2::date, active_days_count = 1 WHERE invitee_account_id = $1", 2, NULL, u, NULL, NULL, 0);
    PQclear(ur);
    cnt = 1;
  } else if (strcmp(d1, today) != 0 && !d2[0]) {
    cnt += 1;
    bool in_win = bound_age_s <= (long long)cfg.activation_window_days * 86400LL;
    bool now_act = !already_act && in_win && cnt >= cfg.activation_days_needed;
    const char *u[2] = {acct, today};
    PGresult *ur = PQexecParams(
        db,
        now_act
          ? "UPDATE referral.attributions SET second_active_day = $2::date, active_days_count = active_days_count + 1, activated_at = now() WHERE invitee_account_id = $1"
          : "UPDATE referral.attributions SET second_active_day = $2::date, active_days_count = active_days_count + 1 WHERE invitee_account_id = $1",
        2, NULL, u, NULL, NULL, 0);
    PQclear(ur);
    newly_activated = now_act;
  }

  if (inviter[0] && (newly_activated || already_act) && cnt >= cfg.activation_days_needed) {
    char ilk[80];
    snprintf(ilk, sizeof ilk, "ref_acct:%s", inviter);
    rf_advisory_lock(db, ilk);

    // Ensure pending referral_inviter grant exists (idempotent via UNIQUE(kind, account_id, source_ref))
    char amt_s[32], tv_s[16];
    snprintf(amt_s, sizeof amt_s, "%lld", cfg.inviter_micros);
    snprintf(tv_s, sizeof tv_s, "%d", cfg.terms_version);
    const char *gv[4] = {inviter, acct, amt_s, tv_s};
    PGresult *gr = PQexecParams(
        db,
        "INSERT INTO referral.grants (account_id, attribution_id, kind, source_ref, amount_micros, status, reason, terms_version) "
        "VALUES ($1, $2, 'referral_inviter', $2, $3::bigint, 'pending', 'activated', $4::int) "
        "ON CONFLICT (kind, account_id, source_ref) DO NOTHING",
        4, NULL, gv, NULL, NULL, 0);
    PQclear(gr);

    rf_check_milestones_tx(db, inviter, &cfg);
    rf_eval_pending_grants_tx(db, inviter, -1);
  }
  PQclear(PQexec(db, "COMMIT"));
}

// ---- Promo Credit spend order + Inference hooks

// Extract "model":"..." from a Chat Completions JSON body.
static bool rf_extract_model(const char *body, char out[128]) {
  out[0] = 0;
  if (!body) return false;
  const char *p = strstr(body, "\"model\"");
  if (!p) return false;
  p += 7;
  while (*p == ' ' || *p == '\t' || *p == ':' || *p == '\r' || *p == '\n') p++;
  if (*p != '"') return false;
  p++;
  size_t i = 0;
  while (p[i] && p[i] != '"' && p[i] != '\\' && i + 1 < 128) { out[i] = p[i]; i++; }
  out[i] = 0;
  return i > 0;
}

// Called by inference.c before inference.request: true if promo credit can fund this model call
// even when paid inference.credit == 0.
bool referral_promo_can_admit(PGconn *db, const char *who, const char *req_body) {
  if (!db || !who || !*who || !req_body) return false;
  char model[128];
  if (!rf_extract_model(req_body, model)) return false;
  char acct[48];
  if (!rf_resolve_account(db, who, acct)) return false;
  if (rf_account_frozen(db, acct)) return false;

  rf_config cfg;
  if (!rf_load_config(db, false, &cfg)) return false;
  if (!rp_model_promo_eligible(rf_cs(model), rf_cs(cfg.promo_models))) return false;

  long long bal = 0;
  char exp_s[40];
  referral_promo_balance(db, acct, &bal, exp_s);
  long long spent_today = rf_promo_spent_today(db, acct);
  int64_t promo_u = 0, paid_u = 0;
  if (!rp_spend_order(bal, 0, (int64_t)time(NULL), spent_today, cfg.promo_daily_cap_micros,
                      true, false, 1, (coax_v_s64){&promo_u, 1}, (coax_v_s64){&paid_u, 1})) {
    return false;
  }
  return promo_u > 0;
}

// Called by inference.c after inference.finish_call recorded cost_micros on inference.calls.
// Debits promo credit first (oldest expiry first, up to $5/UTC day on promo-eligible models),
// debits paid inference.credit for any remaining cost, records inference.promo_calls,
// and records UTC active day for inviter activation on HTTP 200 turns.
void referral_settle_inference(PGconn *db, const char *who, const char *model, int http_status,
                               long long call_id, long long cost_micros, bool free_key) {
  if (!db || !who || !*who) return;
  char acct[48];
  bool has_acct = rf_resolve_account(db, who, acct);

  if (!free_key && cost_micros > 0) {
    PGresult *tx = PQexec(db, "BEGIN");
    if (PQresultStatus(tx) != PGRES_COMMAND_OK) { PQclear(tx); goto active_step; }
    PQclear(tx);

    char lk[80];
    snprintf(lk, sizeof lk, "ref_promo:%s", has_acct ? acct : who);
    rf_advisory_lock(db, lk);

    rf_config cfg;
    bool have_cfg = rf_load_config(db, false, &cfg);
    bool model_elig = have_cfg && model && rp_model_promo_eligible(rf_cs(model), rf_cs(cfg.promo_models));
    bool frozen = has_acct && rf_account_frozen(db, acct);
    long long promo_used_total = 0;
    long long remaining_cost = cost_micros;

    if (has_acct && have_cfg && model_elig && !frozen) {
      long long day_spent = rf_promo_spent_today(db, acct);
      const char *av[1] = {acct};
      PGresult *lr = PQexecParams(
          db,
          "SELECT grant_id, remaining_micros, kind, "
          "coalesce(floor(extract(epoch FROM expires_at))::bigint, 0) "
          "FROM inference.promo_credit "
          "WHERE account_id = $1 AND remaining_micros > 0 AND (expires_at IS NULL OR expires_at > now()) "
          "ORDER BY expires_at ASC NULLS LAST, created_at ASC FOR UPDATE",
          1, NULL, av, NULL, NULL, 0);
      int64_t now_s = (int64_t)time(NULL);
      int nrows = (PQresultStatus(lr) == PGRES_TUPLES_OK) ? PQntuples(lr) : 0;
      for (int i = 0; i < nrows && remaining_cost > 0; i++) {
        const char *gid = PQgetvalue(lr, i, 0);
        long long lot_rem = atoll(PQgetvalue(lr, i, 1));
        const char *kind = PQgetvalue(lr, i, 2);
        long long exp_s = atoll(PQgetvalue(lr, i, 3));
        int64_t p_use = 0, p_paid = 0;
        if (!rp_spend_order(lot_rem, exp_s, now_s, day_spent, cfg.promo_daily_cap_micros,
                            true, false, remaining_cost, (coax_v_s64){&p_use, 1}, (coax_v_s64){&p_paid, 1})) {
          break;
        }
        if (p_use <= 0) continue;
        char use_s[32], kd_s[16];
        snprintf(use_s, sizeof use_s, "%lld", (long long)p_use);
        snprintf(kd_s, sizeof kd_s, "%d", cfg.kickback_inactivity_days);
        bool is_kb = !strcmp(kind, "kickback");
        const char *uv[3] = {gid, use_s, kd_s};
        PGresult *ur = PQexecParams(
            db,
            is_kb
              ? "UPDATE inference.promo_credit SET remaining_micros = remaining_micros - $2::bigint, "
                "last_activity_at = now(), expires_at = now() + make_interval(days => $3::int) WHERE grant_id = $1::bigint"
              : "UPDATE inference.promo_credit SET remaining_micros = remaining_micros - $2::bigint, "
                "last_activity_at = now() WHERE grant_id = $1::bigint",
            is_kb ? 3 : 2, NULL, uv, NULL, NULL, 0);
        PQclear(ur);
        promo_used_total += p_use;
        day_spent += p_use;
        remaining_cost -= p_use;
      }
      PQclear(lr);

      if (promo_used_total > 0) {
        char ps[32];
        snprintf(ps, sizeof ps, "%lld", promo_used_total);
        const char *dv[2] = {acct, ps};
        PGresult *dr = PQexecParams(
            db,
            "INSERT INTO inference.promo_spend_day (account_id, utc_day, spent_micros) "
            "VALUES ($1, (now() AT TIME ZONE 'UTC')::date, $2::bigint) "
            "ON CONFLICT (account_id, utc_day) DO UPDATE SET spent_micros = inference.promo_spend_day.spent_micros + EXCLUDED.spent_micros",
            2, NULL, dv, NULL, NULL, 0);
        PQclear(dr);
      }
    }

    long long msat_due = 0;
    if (remaining_cost > 0) {
      PGresult *rr = PQexec(db, "SELECT usd_per_btc FROM inference.btc_usd LIMIT 1");
      long long rate = (PQresultStatus(rr) == PGRES_TUPLES_OK && PQntuples(rr) == 1) ? atoll(PQgetvalue(rr, 0, 0)) : 0;
      PQclear(rr);
      if (rate > 0) msat_due = (remaining_cost * 100000LL + rate - 1) / rate;
      if (msat_due > 0) {
        char ms[32], cid_s[32];
        snprintf(ms, sizeof ms, "%lld", msat_due);
        snprintf(cid_s, sizeof cid_s, "%lld", call_id);
        const char *cv[2] = {who, ms};
        PGresult *cr = PQexecParams(db, "UPDATE inference.credit SET msat = msat - $2::bigint, updated = now() WHERE pubkey = $1", 2, NULL, cv, NULL, NULL, 0);
        PQclear(cr);
        if (call_id > 0) {
          const char *uv[2] = {cid_s, ms};
          PGresult *ur = PQexecParams(db, "UPDATE inference.calls SET msat = $2::bigint WHERE id = $1::bigint", 2, NULL, uv, NULL, NULL, 0);
          PQclear(ur);
        }
        const char *sv[2] = {who, ms};
        PGresult *sr = PQexecParams(
            db,
            "INSERT INTO auth.credit_spends AS c (account, hour, msat) "
            "SELECT a.id, date_trunc('hour', now()), $2::bigint "
            "FROM auth.accounts a LEFT JOIN auth.nostr_keys k ON k.account = a.id "
            "WHERE a.kind = 'person' AND (k.pubkey = $1 OR a.id = $1) GROUP BY a.id "
            "ON CONFLICT (account, hour) DO UPDATE SET msat = c.msat + EXCLUDED.msat",
            2, NULL, sv, NULL, NULL, 0);
        PQclear(sr);
      }
    }

    if (call_id > 0 && (promo_used_total > 0 || msat_due > 0)) {
      char cid_s[32], ps[32], ms[32];
      snprintf(cid_s, sizeof cid_s, "%lld", call_id);
      snprintf(ps, sizeof ps, "%lld", promo_used_total);
      snprintf(ms, sizeof ms, "%lld", msat_due);
      const char *pcv[4] = {cid_s, has_acct ? acct : who, ps, ms};
      PGresult *pcr = PQexecParams(
          db,
          "INSERT INTO inference.promo_calls (call_id, account_id, promo_micros, paid_msat) "
          "VALUES ($1::bigint, $2, $3::bigint, $4::bigint) ON CONFLICT (call_id) DO NOTHING",
          4, NULL, pcv, NULL, NULL, 0);
      PQclear(pcr);
    }

    PQclear(PQexec(db, "COMMIT"));
  }

active_step:
  if (http_status == 200 && has_acct) {
    rf_record_active_day(db, acct);
  }
}

// Called by inference.c inside the SAME transaction as inference.topup_settle when status == 200.
// Grants 1% lifetime kickback atomically to the inviter (idempotent on source_ref = payment_hash).
bool referral_on_topup_settled_tx(PGconn *db, const char *payment_hash, const char *pubkey_or_acct, long long msat) {
  if (!db || !payment_hash || !*payment_hash || !pubkey_or_acct || msat <= 0) return true;
  char invitee[48];
  if (!rf_resolve_account(db, pubkey_or_acct, invitee)) return true;

  const char *v[1] = {invitee};
  PGresult *ar = PQexecParams(
      db,
      "SELECT coalesce(inviter_account_id, '') FROM referral.attributions WHERE invitee_account_id = $1",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(ar) != PGRES_TUPLES_OK || PQntuples(ar) != 1) {
    PQclear(ar);
    return true;
  }
  char inviter[48];
  snprintf(inviter, sizeof inviter, "%s", PQgetvalue(ar, 0, 0));
  PQclear(ar);
  if (!inviter[0]) return true;

  rf_config cfg;
  if (!rf_load_config(db, false, &cfg)) return true;

  PGresult *rr = PQexec(db, "SELECT usd_per_btc FROM inference.btc_usd LIMIT 1");
  long long usd_per_btc = (PQresultStatus(rr) == PGRES_TUPLES_OK && PQntuples(rr) == 1) ? atoll(PQgetvalue(rr, 0, 0)) : 0;
  PQclear(rr);
  if (usd_per_btc <= 0) return true;

  long long topup_micros = (long long)rp_msat_to_micros(msat, usd_per_btc);
  long long kb_micros = (long long)rp_kickback(topup_micros, cfg.kickback_bps, cfg.flag_kickback, true);
  if (kb_micros <= 0) return true;

  char lk[80];
  snprintf(lk, sizeof lk, "ref_acct:%s", inviter);
  if (!rf_advisory_lock(db, lk)) return false;

  char amt_s[32], tv_s[16];
  snprintf(amt_s, sizeof amt_s, "%lld", kb_micros);
  snprintf(tv_s, sizeof tv_s, "%d", cfg.terms_version);
  const char *gv[5] = {inviter, invitee, payment_hash, amt_s, tv_s};
  PGresult *gr = PQexecParams(
      db,
      "INSERT INTO referral.grants (account_id, attribution_id, kind, source_ref, amount_micros, status, reason, terms_version, source_topup_id) "
      "VALUES ($1, $2, 'kickback', $3, $4::bigint, 'pending', 'topup_settled', $5::int, $3) "
      "ON CONFLICT (kind, account_id, source_ref) DO NOTHING",
      5, NULL, gv, NULL, NULL, 0);
  bool gok = PQresultStatus(gr) == PGRES_COMMAND_OK;
  PQclear(gr);
  if (!gok) return false;

  rf_eval_pending_grants_tx(db, inviter, -1);
  return true;
}

// ---- GitHub API profile fetch (using transient WorkOS Pipes token, wiped via explicit_bzero)

static bool rf_parse_iso8601_utc(const char *s, time_t *out) {
  if (!s || strlen(s) < 19) return false;
  int Y = 0, M = 0, D = 0, h = 0, m = 0, sec = 0;
  if (sscanf(s, "%d-%d-%dT%d:%d:%d", &Y, &M, &D, &h, &m, &sec) != 6) return false;
  struct tm tm;
  memset(&tm, 0, sizeof tm);
  tm.tm_year = Y - 1900;
  tm.tm_mon = M - 1;
  tm.tm_mday = D;
  tm.tm_hour = h;
  tm.tm_min = m;
  tm.tm_sec = sec;
  *out = timegm(&tm);
  return *out > 0;
}

static bool rf_json_extract_str(const char *json, size_t len, const char *key, char *out, size_t cap) {
  out[0] = 0;
  if (!json || !len) return false;
  char pat[96];
  snprintf(pat, sizeof pat, "\"%s\"", key);
  const char *p = strstr(json, pat);
  if (!p) return false;
  p += strlen(pat);
  while (*p == ' ' || *p == '\t' || *p == ':') p++;
  if (*p != '"') return false;
  p++;
  size_t i = 0;
  while (*p && *p != '"' && i + 1 < cap) {
    if (*p == '\\' && p[1]) p++;
    out[i++] = *p++;
  }
  out[i] = 0;
  return true;
}

static long long rf_json_extract_int(const char *json, const char *key, long long fallback) {
  if (!json) return fallback;
  char pat[96];
  snprintf(pat, sizeof pat, "\"%s\"", key);
  const char *p = strstr(json, pat);
  if (!p) return fallback;
  p += strlen(pat);
  while (*p == ' ' || *p == '\t' || *p == ':') p++;
  if (!(*p == '-' || (*p >= '0' && *p <= '9'))) return fallback;
  return strtoll(p, NULL, 10);
}

// Count non-fork public repositories in GitHub /user/repos JSON array.
static int rf_count_non_fork_public_repos(const char *json) {
  if (!json) return 0;
  int count = 0;
  const char *p = json;
  while ((p = strchr(p, '{')) != NULL) {
    const char *end = strchr(p + 1, '}');
    if (!end) break;
    size_t slen = (size_t)(end - p + 1);
    char *obj = malloc(slen + 1);
    if (!obj) break;
    memcpy(obj, p, slen); obj[slen] = 0;
    bool is_fork = strstr(obj, "\"fork\":true") != NULL || strstr(obj, "\"fork\": true") != NULL;
    bool is_priv = strstr(obj, "\"private\":true") != NULL || strstr(obj, "\"private\": true") != NULL;
    bool has_repo = strstr(obj, "\"full_name\"") != NULL || strstr(obj, "\"name\"") != NULL;
    if (has_repo && !is_fork && !is_priv) count++;
    free(obj);
    p = end + 1;
  }
  return count;
}

// Count org objects in GitHub /user/orgs JSON array.
static int rf_count_orgs(const char *json) {
  if (!json) return 0;
  int count = 0;
  const char *p = json;
  while ((p = strstr(p, "\"login\"")) != NULL) {
    count++;
    p += 7;
  }
  return count;
}

static bool rf_gh_get(const char *path, const char *auth_hdr, http_client_res *res) {
  char url[512];
  snprintf(url, sizeof url, "%s%s", rf_github_api_base(), path);
  const char *hdrs[] = {
    auth_hdr,
    "Accept: application/vnd.github+json",
    "X-GitHub-Api-Version: 2022-11-28",
    "User-Agent: openagents-api",
    NULL
  };
  http_client_req req = {
    .method = "GET",
    .url = url,
    .body = NULL,
    .body_len = 0,
    .headers = hdrs,
    .connect_timeout_ms = 5000,
    .total_timeout_ms = 12000,
    .max_response_bytes = 524288,
    .allow_http_local = rf_github_allow_local(),
  };
  http_client_send(&req, res);
  return res->status == 200 && res->body && res->body_len > 0;
}

typedef struct {
  bool connected;
  bool eligible;
  char login[64];
  char reason[160];
  long long newly_granted;
  long long promo_micros;
  char promo_expires[40];
} rf_recheck_out;

static bool rf_run_github_check(PGconn *db, const char *acct, rf_recheck_out *out) {
  memset(out, 0, sizeof *out);
  char token[512] = {0}, err[128] = {0};
  if (!workos_pipes_github_token(db, acct, token, sizeof token, err, sizeof err)) {
    explicit_bzero(token, sizeof token);
    out->connected = false;
    snprintf(out->reason, sizeof out->reason, "Connect GitHub to unlock promo credit.");
    referral_promo_balance(db, acct, &out->promo_micros, out->promo_expires);
    return true;
  }

  char auth_hdr[600];
  snprintf(auth_hdr, sizeof auth_hdr, "Authorization: Bearer %s", token);
  explicit_bzero(token, sizeof token);

  http_client_res ures = {0}, rres = {0}, ores = {0};
  bool uok = rf_gh_get("/user", auth_hdr, &ures);
  if (uok) {
    rf_gh_get("/user/repos?type=owner&per_page=100", auth_hdr, &rres);
    rf_gh_get("/user/orgs?per_page=10", auth_hdr, &ores);
  }
  explicit_bzero(auth_hdr, sizeof auth_hdr);

  if (!uok) {
    http_client_res_free(&ures); http_client_res_free(&rres); http_client_res_free(&ores);
    out->connected = false;
    snprintf(out->reason, sizeof out->reason, "Could not read GitHub profile. Try again.");
    referral_promo_balance(db, acct, &out->promo_micros, out->promo_expires);
    return false;
  }

  long long gh_id = rf_json_extract_int(ures.body, "id", 0);
  char login[64] = "", avatar[256] = "", created_s[48] = "";
  rf_json_extract_str(ures.body, ures.body_len, "login", login, sizeof login);
  rf_json_extract_str(ures.body, ures.body_len, "avatar_url", avatar, sizeof avatar);
  rf_json_extract_str(ures.body, ures.body_len, "created_at", created_s, sizeof created_s);
  int followers = (int)rf_json_extract_int(ures.body, "followers", 0);
  int non_fork_repos = (rres.status == 200 && rres.body) ? rf_count_non_fork_public_repos(rres.body) : 0;
  int orgs = (ores.status == 200 && ores.body) ? rf_count_orgs(ores.body) : 0;

  http_client_res_free(&ures); http_client_res_free(&rres); http_client_res_free(&ores);

  if (gh_id <= 0 || !login[0]) {
    out->connected = true;
    snprintf(out->reason, sizeof out->reason, "GitHub profile is incomplete.");
    referral_promo_balance(db, acct, &out->promo_micros, out->promo_expires);
    return false;
  }

  time_t created_t = 0;
  int age_days = 0;
  if (rf_parse_iso8601_utc(created_s, &created_t)) {
    time_t now_t = time(NULL);
    if (now_t > created_t) age_days = (int)((now_t - created_t) / 86400);
  }

  char gh_id_s[64];
  snprintf(gh_id_s, sizeof gh_id_s, "%lld", gh_id);

  PGresult *tx = PQexec(db, "BEGIN");
  if (PQresultStatus(tx) != PGRES_COMMAND_OK) { PQclear(tx); return false; }
  PQclear(tx);

  char lk[80];
  snprintf(lk, sizeof lk, "ref_acct:%s", acct);
  rf_advisory_lock(db, lk);
  char glk[96];
  snprintf(glk, sizeof glk, "ref_gh:%s", gh_id_s);
  rf_advisory_lock(db, glk);

  rf_config cfg;
  if (!rf_load_config(db, false, &cfg)) { PQclear(PQexec(db, "ROLLBACK")); return false; }

  // Check if this GitHub numeric ID is already claimed by another account or denied
  bool already_claimed = false, denied = false;
  const char *gv[1] = {gh_id_s};
  PGresult *cr = PQexecParams(db, "SELECT account_id, denied FROM referral.github_claims WHERE github_id_key = $1 FOR UPDATE", 1, NULL, gv, NULL, NULL, 0);
  if (PQresultStatus(cr) == PGRES_TUPLES_OK && PQntuples(cr) == 1) {
    const char *owner = PQgetvalue(cr, 0, 0);
    denied = !strcmp(PQgetvalue(cr, 0, 1), "t");
    if (strcmp(owner, acct) != 0) already_claimed = true;
  }
  PQclear(cr);

  int verdict = (int)rp_eligible(age_days, cfg.gh_min_age_days, non_fork_repos, cfg.gh_min_repos,
                                 followers, cfg.gh_min_followers, orgs, already_claimed, denied);

  if (!already_claimed) {
    char rc_s[16], fc_s[16], oc_s[16];
    snprintf(rc_s, sizeof rc_s, "%d", non_fork_repos);
    snprintf(fc_s, sizeof fc_s, "%d", followers);
    snprintf(oc_s, sizeof oc_s, "%d", orgs);
    const char *uv[9] = {gh_id_s, acct, login, avatar, created_s[0] ? created_s : NULL, rc_s, fc_s, oc_s, verdict == 0 ? "true" : "false"};
    PGresult *ur = PQexecParams(
        db,
        "INSERT INTO referral.github_claims "
        "(github_id_key, account_id, github_login, github_avatar, github_created_at, repos_count, followers_count, orgs_count, eligible, updated_at) "
        "VALUES ($1, $2, $3, $4, $5::timestamptz, $6::int, $7::int, $8::int, $9::bool, now()) "
        "ON CONFLICT (github_id_key) DO UPDATE SET "
        "github_login = EXCLUDED.github_login, github_avatar = EXCLUDED.github_avatar, "
        "github_created_at = coalesce(EXCLUDED.github_created_at, referral.github_claims.github_created_at), "
        "repos_count = EXCLUDED.repos_count, followers_count = EXCLUDED.followers_count, "
        "orgs_count = EXCLUDED.orgs_count, eligible = EXCLUDED.eligible, updated_at = now()",
        9, NULL, uv, NULL, NULL, 0);
    PQclear(ur);

    // Same-hour GitHub account creation cluster signal (§4.4)
    if (created_s[0]) {
      const char *clv[2] = {acct, created_s};
      PGresult *clr = PQexecParams(
          db,
          "SELECT count(*)::int FROM referral.attributions a1 "
          "JOIN referral.attributions a2 ON a2.code_id = a1.code_id AND a2.invitee_account_id <> a1.invitee_account_id "
          "JOIN referral.github_claims g2 ON g2.account_id = a2.invitee_account_id "
          "WHERE a1.invitee_account_id = $1 "
          "AND g2.github_created_at BETWEEN $2::timestamptz - interval '1 hour' AND $2::timestamptz + interval '1 hour'",
          2, NULL, clv, NULL, NULL, 0);
      int cluster = (PQresultStatus(clr) == PGRES_TUPLES_OK && PQntuples(clr) == 1) ? atoi(PQgetvalue(clr, 0, 0)) : 0;
      PQclear(clr);
      if (rp_hold_signal(0, cluster > 0, 0) != 0) {
        const char *hv[1] = {acct};
        PGresult *hr = PQexecParams(db, "UPDATE referral.attributions SET held = true, hold_reason = 'gh_cluster' WHERE invitee_account_id = $1 AND NOT held", 1, NULL, hv, NULL, NULL, 0);
        PQclear(hr);
      }
    }

    // Store verified GitHub handle on personal code row if valid (reclaiming stale old handle if needed per §3.1)
    char pcode[16];
    if (rf_ensure_personal_code(db, acct, pcode) && rp_vanity_valid(rf_cs(login))) {
      const char *cv[2] = {login, acct};
      PGresult *clr = PQexecParams(db, "UPDATE referral.codes SET vanity_handle = NULL, vanity_enabled = false WHERE lower(vanity_handle) = lower($1) AND owner_account_id <> $2", 2, NULL, cv, NULL, NULL, 0);
      PQclear(clr);
      PGresult *svr = PQexecParams(db, "UPDATE referral.codes SET vanity_handle = $1 WHERE owner_account_id = $2 AND kind = 'personal'", 2, NULL, cv, NULL, NULL, 0);
      PQclear(svr);
    }
  }

  // Ensure github_connect ($5) grant exists (idempotent via UNIQUE(kind, account_id, source_ref))
  char amt_s[32], tv_s[16];
  snprintf(amt_s, sizeof amt_s, "%lld", cfg.github_connect_micros);
  snprintf(tv_s, sizeof tv_s, "%d", cfg.terms_version);
  const char *gcv[3] = {acct, amt_s, tv_s};
  PGresult *gcr = PQexecParams(
      db,
      "INSERT INTO referral.grants (account_id, kind, source_ref, amount_micros, status, reason, terms_version) "
      "VALUES ($1, 'github_connect', '', $2::bigint, 'pending', 'checking', $3::int) "
      "ON CONFLICT (kind, account_id, source_ref) DO NOTHING",
      3, NULL, gcv, NULL, NULL, 0);
  PQclear(gcr);

  long long newly = rf_eval_pending_grants_tx(db, acct, verdict);
  PQclear(PQexec(db, "COMMIT"));

  out->connected = true;
  out->eligible = (verdict == 0);
  snprintf(out->login, sizeof out->login, "%s", login);
  rf_format_gh_reason(verdict, age_days, &cfg, out->reason);
  out->newly_granted = newly;
  referral_promo_balance(db, acct, &out->promo_micros, out->promo_expires);
  return true;
}

// Best-effort hook called from integrations.c / workos.c after GitHub connects or OAuth login completes.
// Short-circuits if the account already has an eligible GitHub claim and no pending grants.
void referral_on_github_connected(PGconn *db, const char *account_id) {
  if (!db || !account_id || !*account_id) return;
  const char *v[1] = {account_id};
  PGresult *r = PQexecParams(
      db,
      "SELECT EXISTS (SELECT 1 FROM referral.github_claims WHERE account_id = $1 AND eligible), "
      "EXISTS (SELECT 1 FROM referral.grants WHERE account_id = $1 AND status = 'pending')",
      1, NULL, v, NULL, NULL, 0);
  bool already_done = (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 &&
                       !strcmp(PQgetvalue(r, 0, 0), "t") && strcmp(PQgetvalue(r, 0, 1), "t") != 0);
  if (r) PQclear(r);
  if (already_done) return;
  rf_recheck_out out;
  rf_run_github_check(db, account_id, &out);
}

// ---- HTTP Route Handlers

static void rf_handle_lookup(const request *q, PGconn *db, bool as_lines) {
  char raw_code[96] = "";
  api_query(q, "code", raw_code, sizeof raw_code);

  rf_config cfg;
  if (!rf_load_config(db, false, &cfg)) { api_error(q->fd, 503, "Referral config unavailable."); return; }

  bool valid = false;
  char kind[16] = "", code_disp[64] = "", inv_handle[64] = "", inv_avatar[256] = "";
  long long bonus = 0;

  if (raw_code[0] && rp_lookup_token_ok(rf_cs(raw_code))) {
    char chash[65];
    rf_code_hash(raw_code, chash);
    const char *v[2] = {chash, raw_code};
    PGresult *r = PQexecParams(
        db,
        "SELECT c.id, c.kind, coalesce(c.code_text, c.code_hint), c.bonus_micros, "
        "c.max_uses, c.uses_count, c.expires_at IS NOT NULL AND c.expires_at <= now(), "
        "c.revoked_at IS NOT NULL, c.show_name, "
        "coalesce(g.github_login, ''), coalesce(g.github_avatar, '') "
        "FROM referral.codes c "
        "LEFT JOIN referral.github_claims g ON g.account_id = c.owner_account_id "
        "WHERE c.code_hash = $1 OR (c.vanity_enabled AND lower(c.vanity_handle) = lower($2)) "
        "ORDER BY g.eligible DESC NULLS LAST LIMIT 1",
        2, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
      const char *cid_s = PQgetvalue(r, 0, 0);
      bool expired = !strcmp(PQgetvalue(r, 0, 6), "t");
      bool revoked = !strcmp(PQgetvalue(r, 0, 7), "t");
      int max_uses = PQgetisnull(r, 0, 4) ? -1 : atoi(PQgetvalue(r, 0, 4));
      int uses = atoi(PQgetvalue(r, 0, 5));
      bool exhausted = (max_uses >= 0 && uses >= max_uses);
      bool show_name = !strcmp(PQgetvalue(r, 0, 8), "t");

      // Always record click for valid-target lookups
      char iph[65], uah[65];
      rf_ip_prefix_hash(q, iph);
      rf_header_hash(q, "User-Agent", uah);
      const char *cv[4] = {cid_s, raw_code, iph, uah};
      PGresult *cr = PQexecParams(db, "INSERT INTO referral.clicks (code_id, code_text, ip_hash, ua_hash) VALUES ($1::bigint, $2, $3, $4)", 4, NULL, cv, NULL, NULL, 0);
      PQclear(cr);

      if (!expired && !revoked && !exhausted) {
        valid = true;
        snprintf(kind, sizeof kind, "%s", PQgetvalue(r, 0, 1));
        snprintf(code_disp, sizeof code_disp, "%s", !strcmp(kind, "personal") ? PQgetvalue(r, 0, 2) : raw_code);
        bonus = PQgetisnull(r, 0, 3) ? cfg.invitee_micros : atoll(PQgetvalue(r, 0, 3));
        if (show_name && !strcmp(kind, "personal")) {
          snprintf(inv_handle, sizeof inv_handle, "%s", PQgetvalue(r, 0, 9));
          snprintf(inv_avatar, sizeof inv_avatar, "%s", PQgetvalue(r, 0, 10));
        }
      }
    }
    PQclear(r);
  }

  uint8_t out[1024];
  int64_t n = as_lines
    ? rfw_lookup_lines(valid, rf_cs(kind), rf_cs(code_disp), rf_cs(inv_handle), rf_cs(inv_avatar),
                       bonus, cfg.github_connect_micros, cfg.terms_version, (coax_v_u8){out, sizeof out})
    : rfw_lookup_write(valid, rf_cs(kind), rf_cs(code_disp), rf_cs(inv_handle), rf_cs(inv_avatar),
                       bonus, cfg.github_connect_micros, cfg.terms_version, (coax_v_u8){out, sizeof out});
  if (n < 1) { api_error(q->fd, 500, "Could not encode lookup."); return; }
  if (as_lines) rf_respond_lines(q->fd, 200, out, (size_t)n);
  else rf_respond_json(q->fd, 200, out, (size_t)n);
}

static void rf_handle_summary(const request *q, PGconn *db, const auth_principal *p, bool as_lines) {
  rf_config cfg;
  if (!rf_load_config(db, false, &cfg)) { api_error(q->fd, 503, "Referral config unavailable."); return; }

  char code[16] = "";
  if (!rf_ensure_personal_code(db, p->account, code)) {
    api_error(q->fd, 500, "Could not provision referral code.");
    return;
  }

  const char *av[1] = {p->account};
  char vanity[64] = "";
  bool vanity_enabled = false, show_name = true, commit_footer = false;
  long long clicks = 0;
  PGresult *cr = PQexecParams(
      db,
      "SELECT c.id, coalesce(c.vanity_handle, ''), c.vanity_enabled, c.show_name, c.commit_footer_opt_in, "
      "(SELECT count(*)::bigint FROM referral.clicks cl WHERE cl.code_id = c.id) "
      "FROM referral.codes c WHERE c.owner_account_id = $1 AND c.kind = 'personal'",
      1, NULL, av, NULL, NULL, 0);
  if (PQresultStatus(cr) == PGRES_TUPLES_OK && PQntuples(cr) == 1) {
    snprintf(vanity, sizeof vanity, "%s", PQgetvalue(cr, 0, 1));
    vanity_enabled = !strcmp(PQgetvalue(cr, 0, 2), "t");
    show_name = !strcmp(PQgetvalue(cr, 0, 3), "t");
    commit_footer = !strcmp(PQgetvalue(cr, 0, 4), "t");
    clicks = atoll(PQgetvalue(cr, 0, 5));
  }
  PQclear(cr);

  long long signups = 0, pending_inv = 0, activated_inv = 0;
  long long bonus_earned = 0, kb_earned = 0, kb_today = 0;
  int milestones_mask = 0;
  bool attributed = false, can_redeem = false;

  PGresult *sr = PQexecParams(
      db,
      "SELECT "
      "(SELECT count(*)::bigint FROM referral.attributions WHERE inviter_account_id = $1), "
      "(SELECT count(*)::bigint FROM referral.attributions WHERE inviter_account_id = $1 AND activated_at IS NULL), "
      "(SELECT count(*)::bigint FROM referral.attributions WHERE inviter_account_id = $1 AND activated_at IS NOT NULL), "
      "coalesce((SELECT sum(amount_micros)::bigint FROM referral.grants WHERE account_id = $1 AND status = 'granted' AND kind <> 'kickback'), 0), "
      "coalesce((SELECT sum(amount_micros)::bigint FROM referral.grants WHERE account_id = $1 AND status = 'granted' AND kind = 'kickback'), 0), "
      "coalesce((SELECT sum(amount_micros)::bigint FROM referral.grants WHERE account_id = $1 AND status = 'granted' AND kind = 'kickback' AND granted_at >= date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'), 0), "
      "coalesce((SELECT sum(CASE tier WHEN 5 THEN 1 WHEN 25 THEN 2 WHEN 100 THEN 4 ELSE 0 END)::int FROM referral.milestones WHERE account_id = $1), 0), "
      "EXISTS(SELECT 1 FROM referral.attributions WHERE invitee_account_id = $1), "
      "coalesce((SELECT extract(epoch FROM (now() - greatest(a.created_at, cfg.launch_at)))::bigint <= cfg.redeem_window_days * 86400 FROM auth.accounts a, referral.config cfg WHERE a.id = $1 AND cfg.id = 1), false)",
      1, NULL, av, NULL, NULL, 0);
  if (PQresultStatus(sr) == PGRES_TUPLES_OK && PQntuples(sr) == 1) {
    signups = atoll(PQgetvalue(sr, 0, 0));
    pending_inv = atoll(PQgetvalue(sr, 0, 1));
    activated_inv = atoll(PQgetvalue(sr, 0, 2));
    bonus_earned = atoll(PQgetvalue(sr, 0, 3));
    kb_earned = atoll(PQgetvalue(sr, 0, 4));
    kb_today = atoll(PQgetvalue(sr, 0, 5));
    milestones_mask = atoi(PQgetvalue(sr, 0, 6));
    attributed = !strcmp(PQgetvalue(sr, 0, 7), "t");
    can_redeem = !attributed && cfg.flag_redeem && !strcmp(PQgetvalue(sr, 0, 8), "t");
  }
  PQclear(sr);

  long long promo_micros = 0;
  char promo_exp[40] = "";
  referral_promo_balance(db, p->account, &promo_micros, promo_exp);
  long long promo_day = rf_promo_spent_today(db, p->account);

  rf_gh_state gh;
  rf_load_gh_state(db, p->account, &cfg, &gh);

  char link[256], vlink[256] = "";
  snprintf(link, sizeof link, "%s/r/%s", rf_origin(), code);
  if (vanity[0] && vanity_enabled) snprintf(vlink, sizeof vlink, "%s/r/%s", rf_origin(), vanity);

  // Build grants JSON array via referral-wire step writer
  char *grants_buf = calloc(1, 16384);
  char *inv_buf = calloc(1, 8192);
  uint8_t *out = calloc(1, 32768);
  if (!grants_buf || !inv_buf || !out) {
    free(grants_buf); free(inv_buf); free(out);
    api_error(q->fd, 500, "Out of memory.");
    return;
  }

  int64_t gn = 0;
  PGresult *gr = PQexecParams(
      db,
      "SELECT id, kind, amount_micros, status, reason, "
      "to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'), "
      "coalesce(to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'), '') "
      "FROM referral.grants WHERE account_id = $1 ORDER BY created_at DESC, id DESC LIMIT 40",
      1, NULL, av, NULL, NULL, 0);
  if (PQresultStatus(gr) == PGRES_TUPLES_OK) {
    for (int i = 0; i < PQntuples(gr); i++) {
      int64_t nxt = rfw_grant_item_write(
          i == 0, atoll(PQgetvalue(gr, i, 0)), rf_cs(PQgetvalue(gr, i, 1)),
          atoll(PQgetvalue(gr, i, 2)), rf_cs(PQgetvalue(gr, i, 3)), rf_cs(PQgetvalue(gr, i, 4)),
          rf_cs(PQgetvalue(gr, i, 5)), rf_cs(PQgetvalue(gr, i, 6)),
          (coax_v_u8){(uint8_t *)grants_buf, 16383}, gn);
      if (nxt < 0) break;
      gn = nxt;
    }
  }
  PQclear(gr);
  grants_buf[gn] = 0;

  // Build privacy-safe invitees list (status + handle only when invitee opted in; never email or top-ups)
  int64_t in_n = 0;
  PGresult *ir = PQexecParams(
      db,
      "SELECT CASE WHEN a.activated_at IS NOT NULL THEN 'activated' ELSE 'pending' END, "
      "CASE WHEN a.show_to_inviter THEN coalesce(g.github_login, '') ELSE '' END, "
      "a.active_days_count, "
      "to_char(a.bound_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') "
      "FROM referral.attributions a "
      "LEFT JOIN referral.github_claims g ON g.account_id = a.invitee_account_id "
      "WHERE a.inviter_account_id = $1 ORDER BY a.bound_at DESC LIMIT 40",
      1, NULL, av, NULL, NULL, 0);
  if (PQresultStatus(ir) == PGRES_TUPLES_OK) {
    for (int i = 0; i < PQntuples(ir); i++) {
      int64_t nxt = rfw_invitee_item_write(
          i == 0, rf_cs(PQgetvalue(ir, i, 0)), rf_cs(PQgetvalue(ir, i, 1)),
          atoi(PQgetvalue(ir, i, 2)), rf_cs(PQgetvalue(ir, i, 3)),
          (coax_v_u8){(uint8_t *)inv_buf, 8191}, in_n);
      if (nxt < 0) break;
      in_n = nxt;
    }
  }
  PQclear(ir);
  inv_buf[in_n] = 0;

  int64_t n = as_lines
    ? rfw_summary_lines(rf_cs(code), rf_cs(vanity), vanity_enabled, rf_cs(link), rf_cs(vlink),
                        show_name, commit_footer, clicks, signups, pending_inv, activated_inv,
                        promo_micros, rf_cs(promo_exp), promo_day, cfg.promo_daily_cap_micros,
                        bonus_earned, kb_earned, kb_today, milestones_mask,
                        gh.has_row, gh.eligible, rf_cs(gh.login), rf_cs(gh.reason),
                        attributed, can_redeem, (coax_v_u8){out, 32768})
    : rfw_summary_write(rf_cs(code), rf_cs(vanity), vanity_enabled, rf_cs(link), rf_cs(vlink),
                        show_name, commit_footer, clicks, signups, pending_inv, activated_inv,
                        promo_micros, rf_cs(promo_exp), promo_day, cfg.promo_daily_cap_micros,
                        bonus_earned, kb_earned, kb_today, milestones_mask,
                        gh.has_row, gh.eligible, rf_cs(gh.login), rf_cs(gh.reason),
                        attributed, can_redeem,
                        cfg.flag_redeem, cfg.flag_grants, cfg.flag_kickback, cfg.flag_github_connect,
                        rf_cs(grants_buf), rf_cs(inv_buf), (coax_v_u8){out, 32768});
  if (n < 1) api_error(q->fd, 500, "Could not encode referral summary.");
  else if (as_lines) rf_respond_lines(q->fd, 200, out, (size_t)n);
  else rf_respond_json(q->fd, 200, out, (size_t)n);

  free(grants_buf); free(inv_buf); free(out);
}

static const char *rf_bind_error_msg(int v, int *http_status) {
  *http_status = 400;
  switch (v) {
    case 1: *http_status = 503; return "Referral redemptions are paused.";
    case 2: *http_status = 423; return "Frozen accounts cannot redeem referral codes.";
    case 3: *http_status = 409; return "This account already has a referral code.";
    case 4: *http_status = 404; return "Referral code not found.";
    case 5: *http_status = 410; return "That referral code was revoked.";
    case 6: *http_status = 410; return "That referral code has expired.";
    case 7: *http_status = 410; return "That campaign code has reached its use limit.";
    case 8: *http_status = 400; return "You cannot redeem your own referral code.";
    case 9: *http_status = 403; return "That code belongs to an account sharing your GitHub identity.";
    case 10: *http_status = 400; return "Reciprocal referral codes are not allowed.";
    case 11: *http_status = 400; return "Referral codes must be redeemed within 7 days of signup.";
    default: return "Could not redeem referral code.";
  }
}

static int rf_redeem_inner(const request *q, PGconn *db, const char *acct, const char *code_in, const char *src_s,
                           bool show_inv, char disp_code_out[64], char kind_out[16], long long *bonus_out,
                           bool *held_out, long long *granted_now_out, int *http_status_out, const char **err_msg_out) {
  if (disp_code_out) disp_code_out[0] = 0;
  if (kind_out) kind_out[0] = 0;
  if (bonus_out) *bonus_out = 0;
  if (held_out) *held_out = false;
  if (granted_now_out) *granted_now_out = 0;
  if (http_status_out) *http_status_out = 400;
  if (err_msg_out) *err_msg_out = "Could not redeem referral code.";

  size_t cn = code_in ? strlen(code_in) : 0;
  if (cn < 1 || cn > 64 || !rp_lookup_token_ok((coax_string){code_in, (int64_t)cn})) {
    if (err_msg_out) *err_msg_out = "Invalid referral code format.";
    return -1;
  }

  PGresult *tx = PQexec(db, "BEGIN");
  if (PQresultStatus(tx) != PGRES_COMMAND_OK) { PQclear(tx); if (http_status_out) *http_status_out = 500; if (err_msg_out) *err_msg_out = "Database busy."; return -1; }
  PQclear(tx);

  char lk[80];
  snprintf(lk, sizeof lk, "ref_acct:%s", acct);
  rf_advisory_lock(db, lk);

  rf_config cfg;
  if (!rf_load_config(db, false, &cfg)) {
    PQclear(PQexec(db, "ROLLBACK"));
    if (http_status_out) *http_status_out = 503;
    if (err_msg_out) *err_msg_out = "Referral config unavailable.";
    return -1;
  }

  // Account state & age (measured from GREATEST(created_at, launch_at) per §3.3)
  bool frozen = false, already_attr = false;
  long long acct_age_s = 0;
  const char *av[1] = {acct};
  PGresult *ar = PQexecParams(
      db,
      "SELECT (a.frozen_at IS NOT NULL OR a.deleted_at IS NOT NULL), "
      "EXISTS(SELECT 1 FROM referral.attributions WHERE invitee_account_id = a.id), "
      "coalesce(floor(extract(epoch FROM (now() - greatest(a.created_at, cfg.launch_at))))::bigint, 0) "
      "FROM auth.accounts a, referral.config cfg WHERE a.id = $1 AND cfg.id = 1",
      1, NULL, av, NULL, NULL, 0);
  if (PQresultStatus(ar) == PGRES_TUPLES_OK && PQntuples(ar) == 1) {
    frozen = !strcmp(PQgetvalue(ar, 0, 0), "t");
    already_attr = !strcmp(PQgetvalue(ar, 0, 1), "t");
    acct_age_s = atoll(PQgetvalue(ar, 0, 2));
  }
  PQclear(ar);

  // Lookup code row FOR UPDATE
  char chash[65];
  rf_code_hash(code_in, chash);
  const char *cv[2] = {chash, code_in};
  PGresult *cr = PQexecParams(
      db,
      "SELECT id, kind, coalesce(owner_account_id, ''), coalesce(code_text, ''), bonus_micros, "
      "max_uses, uses_count, (expires_at IS NOT NULL AND expires_at <= now()), (revoked_at IS NOT NULL) "
      "FROM referral.codes "
      "WHERE code_hash = $1 OR (vanity_enabled AND lower(vanity_handle) = lower($2)) "
      "LIMIT 1 FOR UPDATE",
      2, NULL, cv, NULL, NULL, 0);
  bool code_exists = (PQresultStatus(cr) == PGRES_TUPLES_OK && PQntuples(cr) == 1);
  char cid_s[32] = "", kind_s[16] = "", owner_id[48] = "", code_text[64] = "";
  long long bonus = cfg.invitee_micros;
  bool code_expired = false, code_revoked = false, code_exhausted = false;
  if (code_exists) {
    snprintf(cid_s, sizeof cid_s, "%s", PQgetvalue(cr, 0, 0));
    snprintf(kind_s, sizeof kind_s, "%s", PQgetvalue(cr, 0, 1));
    snprintf(owner_id, sizeof owner_id, "%s", PQgetvalue(cr, 0, 2));
    snprintf(code_text, sizeof code_text, "%s", PQgetvalue(cr, 0, 3));
    if (!PQgetisnull(cr, 0, 4)) bonus = atoll(PQgetvalue(cr, 0, 4));
    int max_uses = PQgetisnull(cr, 0, 5) ? -1 : atoi(PQgetvalue(cr, 0, 5));
    int uses = atoi(PQgetvalue(cr, 0, 6));
    code_expired = !strcmp(PQgetvalue(cr, 0, 7), "t");
    code_revoked = !strcmp(PQgetvalue(cr, 0, 8), "t");
    code_exhausted = (max_uses >= 0 && uses >= max_uses);
  }
  PQclear(cr);

  bool self_ref = code_exists && owner_id[0] && !strcmp(owner_id, acct);
  bool shared_gh = false, reciprocal = false;
  if (code_exists && owner_id[0] && !self_ref) {
    const char *pv[2] = {acct, owner_id};
    PGresult *pr = PQexecParams(
        db,
        "SELECT "
        "EXISTS(SELECT 1 FROM referral.attributions WHERE invitee_account_id = $2 AND inviter_account_id = $1), "
        "EXISTS(SELECT 1 FROM referral.github_claims g1 JOIN referral.github_claims g2 "
        "       ON g1.github_id_key = g2.github_id_key OR (g1.github_login <> '' AND lower(g1.github_login) = lower(g2.github_login)) "
        "       WHERE g1.account_id = $1 AND g2.account_id = $2)",
        2, NULL, pv, NULL, NULL, 0);
    if (PQresultStatus(pr) == PGRES_TUPLES_OK && PQntuples(pr) == 1) {
      reciprocal = !strcmp(PQgetvalue(pr, 0, 0), "t");
      shared_gh = !strcmp(PQgetvalue(pr, 0, 1), "t");
    }
    PQclear(pr);
  }

  int bver = (int)rp_bind(cfg.flag_redeem, frozen, already_attr, self_ref, shared_gh, reciprocal,
                          acct_age_s, (long long)cfg.redeem_window_days * 86400LL,
                          code_exists, code_revoked, code_expired, code_exhausted);
  if (bver != 0) {
    PQclear(PQexec(db, "ROLLBACK"));
    int st = 400;
    const char *msg = rf_bind_error_msg(bver, &st);
    if (http_status_out) *http_status_out = st;
    if (err_msg_out) *err_msg_out = msg;
    return bver;
  }

  // Velocity & prefix auto-hold signals (§4.3, §4.4)
  char iph[65], uah[65], devh[65];
  rf_ip_prefix_hash(q, iph);
  rf_header_hash(q, "User-Agent", uah);
  rf_header_hash(q, "X-OA-Device", devh);

  int same_prefix = 0, uses_hour = 0;
  const char *sv[2] = {cid_s, iph};
  PGresult *sr = PQexecParams(
      db,
      "SELECT "
      "CASE WHEN $2 = '' THEN 0 ELSE (SELECT count(*)::int FROM referral.attributions WHERE code_id = $1::bigint AND ip_hash = $2 AND bound_at >= now() - interval '24 hours') END, "
      "(SELECT count(*)::int FROM referral.attributions WHERE code_id = $1::bigint AND bound_at >= now() - interval '1 hour')",
      2, NULL, sv, NULL, NULL, 0);
  if (PQresultStatus(sr) == PGRES_TUPLES_OK && PQntuples(sr) == 1) {
    same_prefix = atoi(PQgetvalue(sr, 0, 0));
    uses_hour = atoi(PQgetvalue(sr, 0, 1));
  }
  PQclear(sr);

  int hold_sig = (int)rp_hold_signal(same_prefix, false, uses_hour);
  bool held = (hold_sig != 0);
  const char *hold_reason = hold_sig == 1 ? "prefix_velocity" : hold_sig == 3 ? "hourly_velocity" : "";

  char tv_s[16];
  snprintf(tv_s, sizeof tv_s, "%d", cfg.terms_version);
  const char *iv[11] = {
    acct, owner_id[0] ? owner_id : NULL, cid_s,
    code_text[0] ? code_text : code_in,
    src_s, show_inv ? "true" : "false", tv_s, iph, uah, devh,
    held ? "true" : "false"
  };
  PGresult *ir = PQexecParams(
      db,
      "INSERT INTO referral.attributions "
      "(invitee_account_id, inviter_account_id, code_id, code_text, source, show_to_inviter, terms_version, ip_hash, ua_hash, device_hash, held, hold_reason) "
      "VALUES ($1, $2, $3::bigint, $4, $5, $6::bool, $7::int, $8, $9, $10, $11::bool, '')",
      11, NULL, iv, NULL, NULL, 0);
  if (PQresultStatus(ir) != PGRES_COMMAND_OK) {
    PQclear(ir);
    PQclear(PQexec(db, "ROLLBACK"));
    if (http_status_out) *http_status_out = 409;
    if (err_msg_out) *err_msg_out = "Could not bind referral code.";
    return -1;
  }
  PQclear(ir);

  if (held && hold_reason[0]) {
    const char *hv[2] = {acct, hold_reason};
    PGresult *hr = PQexecParams(db, "UPDATE referral.attributions SET hold_reason = $2 WHERE invitee_account_id = $1", 2, NULL, hv, NULL, NULL, 0);
    PQclear(hr);
  }

  const char *ucv[1] = {cid_s};
  PGresult *ucr = PQexecParams(db, "UPDATE referral.codes SET uses_count = uses_count + 1 WHERE id = $1::bigint", 1, NULL, ucv, NULL, NULL, 0);
  PQclear(ucr);

  // Create pending invitee / campaign grant
  const char *gkind = !strcmp(kind_s, "campaign") ? "campaign" : "referral_invitee";
  char amt_s[32];
  snprintf(amt_s, sizeof amt_s, "%lld", bonus);
  const char *gv[5] = {acct, gkind, amt_s, tv_s, held ? hold_reason : "github_required"};
  PGresult *gr = PQexecParams(
      db,
      "INSERT INTO referral.grants (account_id, attribution_id, kind, source_ref, amount_micros, status, reason, terms_version) "
      "VALUES ($1, $1, $2, '', $3::bigint, 'pending', $5, $4::int) "
      "ON CONFLICT (kind, account_id, source_ref) DO NOTHING",
      5, NULL, gv, NULL, NULL, 0);
  PQclear(gr);

  // If GitHub is already eligible on this account, evaluate immediately!
  long long granted_now = rf_eval_pending_grants_tx(db, acct, -1);
  PQclear(PQexec(db, "COMMIT"));

  if (disp_code_out) snprintf(disp_code_out, 64, "%s", code_text[0] ? code_text : code_in);
  if (kind_out) snprintf(kind_out, 16, "%s", kind_s);
  if (bonus_out) *bonus_out = bonus;
  if (held_out) *held_out = held;
  if (granted_now_out) *granted_now_out = granted_now;
  return 0;
}

// Best-effort server-side auto-redeem when a browser logs in carrying the first-party oa_ref cookie.
void referral_maybe_redeem_cookie(const request *q, PGconn *db, const char *account_id) {
  if (!q || !db || !account_id || !*account_id) return;
  char ref[80] = "";
  auth_cookie(q, "oa_ref", ref, sizeof ref);
  if (!ref[0]) return;
  (void)rf_redeem_inner(q, db, account_id, ref, "link", true, NULL, NULL, NULL, NULL, NULL, NULL, NULL);
}

static void rf_handle_redeem(const request *q, PGconn *db, const auth_principal *p, bool as_lines) {
  uint8_t code_b[65] = {0}, src_b[17] = {0};
  int64_t show_inv = 1;
  int64_t cn = rfw_redeem_req_read((coax_string){q->body, (int64_t)q->body_len},
                                   (coax_v_u8){code_b, 64}, (coax_v_u8){src_b, 16}, (coax_v_s64){&show_inv, 1});
  if (cn < 1) { api_error(q->fd, 400, "Send {\"code\":\"...\"}."); return; }
  code_b[cn] = 0;
  const char *src_s = (!strcmp((const char *)src_b, "link") || !strcmp((const char *)src_b, "cli")) ? (const char *)src_b : "typed";

  char disp_code[64] = "", kind_s[16] = "";
  long long bonus = 0, granted_now = 0;
  bool held = false;
  int st = 400;
  const char *err_msg = "Could not redeem referral code.";
  if (rf_redeem_inner(q, db, p->account, (const char *)code_b, src_s, show_inv != 0,
                      disp_code, kind_s, &bonus, &held, &granted_now, &st, &err_msg) != 0) {
    api_error(q->fd, st, err_msg);
    return;
  }

  const char *status_s = (granted_now > 0) ? "granted" : "pending";
  const char *msg_s = (granted_now > 0)
    ? "Referral code redeemed and promo credit granted."
    : (held ? "Referral code redeemed; reward is pending review."
            : "Referral code redeemed. Connect an eligible GitHub account to claim your credit.");

  uint8_t out[1024];
  int64_t n = as_lines
    ? rfw_redeem_lines(true, rf_cs(disp_code), rf_cs(kind_s), bonus, rf_cs(status_s), rf_cs(msg_s), (coax_v_u8){out, sizeof out})
    : rfw_redeem_write(true, rf_cs(disp_code), rf_cs(kind_s), bonus, rf_cs(status_s), rf_cs(msg_s), (coax_v_u8){out, sizeof out});
  if (n < 1) { api_error(q->fd, 500, "Could not encode redeem response."); return; }
  if (as_lines) rf_respond_lines(q->fd, 200, out, (size_t)n);
  else rf_respond_json(q->fd, 200, out, (size_t)n);
}

static void rf_handle_vanity(const request *q, PGconn *db, const auth_principal *p, bool as_lines) {
  int64_t vals[3] = {-1, -1, -1};
  if (!rfw_vanity_req_read((coax_string){q->body, (int64_t)q->body_len}, (coax_v_s64){vals, 3})) {
    api_error(q->fd, 400, "Send boolean fields: on/vanity_enabled, show_name, or commit_footer_opt_in.");
    return;
  }

  char code[16];
  if (!rf_ensure_personal_code(db, p->account, code)) {
    api_error(q->fd, 500, "Could not load referral code.");
    return;
  }

  rf_config cfg;
  if (!rf_load_config(db, false, &cfg)) { api_error(q->fd, 503, "Referral config unavailable."); return; }
  rf_gh_state gh;
  rf_load_gh_state(db, p->account, &cfg, &gh);

  if (vals[0] == 1) {
    if (!gh.has_row || !gh.eligible || !gh.login[0]) {
      api_error(q->fd, 409, "Connect an eligible GitHub account before enabling a vanity link.");
      return;
    }
    if (!rp_vanity_valid(rf_cs(gh.login))) {
      api_error(q->fd, 400, "Your GitHub handle cannot be used as a vanity link.");
      return;
    }
    const char *cv[2] = {gh.login, p->account};
    PGresult *clr = PQexecParams(db, "UPDATE referral.codes SET vanity_handle = NULL, vanity_enabled = false WHERE lower(vanity_handle) = lower($1) AND owner_account_id <> $2", 2, NULL, cv, NULL, NULL, 0);
    PQclear(clr);
    PGresult *ur = PQexecParams(db, "UPDATE referral.codes SET vanity_handle = $1, vanity_enabled = true WHERE owner_account_id = $2 AND kind = 'personal'", 2, NULL, cv, NULL, NULL, 0);
    PQclear(ur);
  } else if (vals[0] == 0) {
    const char *v[1] = {p->account};
    PGresult *ur = PQexecParams(db, "UPDATE referral.codes SET vanity_enabled = false WHERE owner_account_id = $1 AND kind = 'personal'", 1, NULL, v, NULL, NULL, 0);
    PQclear(ur);
  }

  if (vals[1] != -1) {
    const char *v[2] = {p->account, vals[1] ? "true" : "false"};
    PGresult *ur = PQexecParams(db, "UPDATE referral.codes SET show_name = $2::bool WHERE owner_account_id = $1 AND kind = 'personal'", 2, NULL, v, NULL, NULL, 0);
    PQclear(ur);
  }
  if (vals[2] != -1) {
    const char *v[2] = {p->account, vals[2] ? "true" : "false"};
    PGresult *ur = PQexecParams(db, "UPDATE referral.codes SET commit_footer_opt_in = $2::bool WHERE owner_account_id = $1 AND kind = 'personal'", 2, NULL, v, NULL, NULL, 0);
    PQclear(ur);
  }

  const char *av[1] = {p->account};
  PGresult *r = PQexecParams(db, "SELECT vanity_enabled, coalesce(vanity_handle, ''), show_name, commit_footer_opt_in FROM referral.codes WHERE owner_account_id = $1 AND kind = 'personal'", 1, NULL, av, NULL, NULL, 0);
  bool ven = false, sname = true, cfoot = false;
  char van[64] = "", vlink[256] = "";
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    ven = !strcmp(PQgetvalue(r, 0, 0), "t");
    snprintf(van, sizeof van, "%s", PQgetvalue(r, 0, 1));
    sname = !strcmp(PQgetvalue(r, 0, 2), "t");
    cfoot = !strcmp(PQgetvalue(r, 0, 3), "t");
  }
  PQclear(r);
  if (ven && van[0]) snprintf(vlink, sizeof vlink, "%s/r/%s", rf_origin(), van);

  uint8_t out[1024];
  int64_t n = as_lines
    ? rfw_vanity_lines(ven, rf_cs(van), rf_cs(vlink), sname, cfoot, (coax_v_u8){out, sizeof out})
    : rfw_vanity_write(ven, rf_cs(van), rf_cs(vlink), sname, cfoot, (coax_v_u8){out, sizeof out});
  if (n < 1) { api_error(q->fd, 500, "Could not encode vanity response."); return; }
  if (as_lines) rf_respond_lines(q->fd, 200, out, (size_t)n);
  else rf_respond_json(q->fd, 200, out, (size_t)n);
}

static void rf_handle_recheck(const request *q, PGconn *db, const auth_principal *p, bool as_lines) {
  rf_recheck_out res;
  rf_run_github_check(db, p->account, &res);

  uint8_t out[1024];
  int64_t n = as_lines
    ? rfw_recheck_lines(res.connected, res.eligible, rf_cs(res.login), rf_cs(res.reason),
                        res.newly_granted, res.promo_micros, rf_cs(res.promo_expires), (coax_v_u8){out, sizeof out})
    : rfw_recheck_write(res.connected, res.eligible, rf_cs(res.login), rf_cs(res.reason),
                        res.newly_granted, res.promo_micros, rf_cs(res.promo_expires), (coax_v_u8){out, sizeof out});
  if (n < 1) { api_error(q->fd, 500, "Could not encode recheck response."); return; }
  if (as_lines) rf_respond_lines(q->fd, 200, out, (size_t)n);
  else rf_respond_json(q->fd, 200, out, (size_t)n);
}

// ---- Admin routes (/api/v1/referral/admin/*)

static void rf_admin_audit(PGconn *db, const char *admin_id, const char *action, const char *target, const char *reason) {
  const char *v[4] = {admin_id, action, target ? target : "", reason ? reason : ""};
  PGresult *r = PQexecParams(
      db,
      "INSERT INTO referral.admin_audit (admin_account_id, action, target_id, reason) VALUES ($1, $2, $3, $4)",
      4, NULL, v, NULL, NULL, 0);
  PQclear(r);
}

static bool rf_handle_admin(const request *q, PGconn *db, const char *sub) {
  char admin_id[65] = "";
  int ast = admin_who(q, db, admin_id);
  if (ast != 0) {
    api_error(q->fd, ast, ast == 401 ? "Admin login required." : "Admin access required.");
    return true;
  }
  const auth_principal *p = auth_principal_of(q);
  if (p && p->account[0]) snprintf(admin_id, sizeof admin_id, "%s", p->account);

  bool is_get = !strcmp(q->method, "GET");
  bool is_post = !strcmp(q->method, "POST");
  if (is_post && p && p->credential == AUTH_COOKIE && !auth_same_site(q)) {
    api_error(q->fd, 403, "Cross-site write refused.");
    return true;
  }

  // GET /api/v1/referral/admin/campaigns
  if (!strcmp(sub, "campaigns") && is_get) {
    PGresult *r = PQexec(
        db,
        "SELECT coalesce(string_agg(jsonb_build_object("
        "'id', id, 'code_hint', code_hint, 'max_uses', max_uses, 'uses_count', uses_count, "
        "'bonus_micros', bonus_micros, 'expires_at', coalesce(to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'), ''), "
        "'revoked', revoked_at IS NOT NULL, 'note', note)::text, ',' ORDER BY id DESC), '') "
        "FROM referral.codes WHERE kind = 'campaign'");
    const char *items = (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) ? PQgetvalue(r, 0, 0) : "";
    uint8_t *out = calloc(1, 32768);
    int64_t n = out ? rfw_admin_list_write(rf_cs("campaigns"), rf_cs(items), (coax_v_u8){out, 32768}) : -1;
    PQclear(r);
    if (n < 1) api_error(q->fd, 500, "Could not encode campaigns.");
    else rf_respond_json(q->fd, 200, out, (size_t)n);
    free(out);
    return true;
  }

  // POST /api/v1/referral/admin/campaigns (mint campaign code — hash only stored, code shown once)
  if (!strcmp(sub, "campaigns") && is_post) {
    uint8_t note_b[200] = {0};
    int64_t nums[3] = {100, 30, -1};
    int64_t nn = rfw_admin_campaign_req((coax_string){q->body, (int64_t)q->body_len},
                                        (coax_v_u8){note_b, 199}, (coax_v_s64){nums, 3});
    if (nn < 0 || nums[0] < 1 || nums[0] > 1000000 || nums[1] < 1 || nums[1] > 3650) {
      api_error(q->fd, 400, "Invalid campaign parameters (uses 1..1000000, days 1..3650).");
      return true;
    }
    note_b[nn] = 0;

    rf_config cfg;
    if (!rf_load_config(db, false, &cfg)) { api_error(q->fd, 503, "Referral config unavailable."); return true; }
    long long bonus = (nums[2] >= 0) ? (long long)nums[2] : cfg.invitee_micros;

    // Mint 16-char Crockford code (2 x 8-char chunks), store ONLY SHA-256 hash + 4-char hint
    char code[20] = "";
    for (int tries = 0; tries < 16; tries++) {
      unsigned char rnd[10];
      if (RAND_bytes(rnd, sizeof rnd) != 1) { api_error(q->fd, 503, "No randomness."); return true; }
      uint8_t c1[9] = {0}, c2[9] = {0};
      if (rp_code_from_bytes((coax_v_u8){rnd, 5}, (coax_v_u8){c1, 8}) == 8 &&
          rp_code_from_bytes((coax_v_u8){rnd + 5, 5}, (coax_v_u8){c2, 8}) == 8) {
        snprintf(code, sizeof code, "%.8s%.8s", (const char *)c1, (const char *)c2);
        break;
      }
    }
    if (!code[0]) { api_error(q->fd, 500, "Could not generate campaign code."); return true; }

    char chash[65], hint[8], uses_s[24], days_s[24], bonus_s[32];
    rf_code_hash(code, chash);
    snprintf(hint, sizeof hint, "%.4s", code);
    snprintf(uses_s, sizeof uses_s, "%lld", (long long)nums[0]);
    snprintf(days_s, sizeof days_s, "%lld", (long long)nums[1]);
    snprintf(bonus_s, sizeof bonus_s, "%lld", bonus);
    const char *v[6] = {chash, hint, bonus_s, uses_s, days_s, (const char *)note_b};
    PGresult *r = PQexecParams(
        db,
        "INSERT INTO referral.codes (code_hash, code_text, code_hint, kind, bonus_micros, max_uses, expires_at, note) "
        "VALUES ($1, NULL, $2, 'campaign', $3::bigint, $4::int, now() + make_interval(days => $5::int), $6) "
        "RETURNING id, to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
        6, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
      PQclear(r);
      api_error(q->fd, 500, "Could not save campaign code.");
      return true;
    }
    long long id = atoll(PQgetvalue(r, 0, 0));
    char exp_s[40];
    snprintf(exp_s, sizeof exp_s, "%s", PQgetvalue(r, 0, 1));
    PQclear(r);

    char id_s[32]; snprintf(id_s, sizeof id_s, "%lld", id);
    rf_admin_audit(db, admin_id, "campaign_new", id_s, (const char *)note_b);

    char link[256];
    snprintf(link, sizeof link, "%s/r/%s", rf_origin(), code);
    uint8_t out[1024];
    int64_t n = rfw_admin_campaign_write(id, rf_cs(code), rf_cs(link), nums[0], bonus, rf_cs(exp_s), rf_cs((const char *)note_b), (coax_v_u8){out, sizeof out});
    if (n < 1) api_error(q->fd, 500, "Could not encode campaign.");
    else rf_respond_json(q->fd, 200, out, (size_t)n);
    return true;
  }

  // POST /api/v1/referral/admin/campaigns/revoke
  if (!strcmp(sub, "campaigns/revoke") && is_post) {
    uint8_t tgt[65] = {0}, rsn[160] = {0};
    int64_t vals[4] = {0};
    if (!rfw_admin_target_req((coax_string){q->body, (int64_t)q->body_len}, (coax_v_u8){tgt, 64}, (coax_v_u8){rsn, 159}, (coax_v_s64){vals, 4})) {
      api_error(q->fd, 400, "Send {\"id\": N} or {\"code\": \"...\"}.");
      return true;
    }
    tgt[vals[0]] = 0; rsn[vals[1]] = 0;
    char id_s[32] = "";
    if (vals[2] > 0) snprintf(id_s, sizeof id_s, "%lld", (long long)vals[2]);
    else if (tgt[0]) snprintf(id_s, sizeof id_s, "%s", (const char *)tgt);
    if (!id_s[0]) { api_error(q->fd, 400, "Missing campaign id or code."); return true; }

    PGresult *r = NULL;
    bool all_digits = true;
    for (const char *c = id_s; *c; c++) if (*c < '0' || *c > '9') all_digits = false;
    if (all_digits) {
      const char *v[1] = {id_s};
      r = PQexecParams(db, "UPDATE referral.codes SET revoked_at = now() WHERE id = $1::bigint AND kind = 'campaign' RETURNING id", 1, NULL, v, NULL, NULL, 0);
    } else {
      char chash[65]; rf_code_hash(id_s, chash);
      const char *v[1] = {chash};
      r = PQexecParams(db, "UPDATE referral.codes SET revoked_at = now() WHERE code_hash = $1 AND kind = 'campaign' RETURNING id", 1, NULL, v, NULL, NULL, 0);
    }
    bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
    PQclear(r);
    if (!ok) { api_error(q->fd, 404, "Campaign code not found."); return true; }
    rf_admin_audit(db, admin_id, "campaign_revoke", id_s, (const char *)rsn);
    uint8_t out[512];
    int64_t n = rfw_admin_action_write(rf_cs("campaign_revoke"), true, rf_cs(id_s), 0, rf_cs("revoked"), (coax_v_u8){out, sizeof out});
    rf_respond_json(q->fd, 200, out, (size_t)n);
    return true;
  }

  // POST /api/v1/referral/admin/grants/hold | void | clawback
  if ((!strcmp(sub, "grants/hold") || !strcmp(sub, "grants/void") || !strcmp(sub, "grants/clawback")) && is_post) {
    uint8_t tgt[65] = {0}, rsn[160] = {0};
    int64_t vals[4] = {0};
    if (!rfw_admin_target_req((coax_string){q->body, (int64_t)q->body_len}, (coax_v_u8){tgt, 64}, (coax_v_u8){rsn, 159}, (coax_v_s64){vals, 4})) {
      api_error(q->fd, 400, "Send {\"account\":\"acct_...\",\"reason\":\"...\"}.");
      return true;
    }
    tgt[vals[0]] = 0; rsn[vals[1]] = 0;
    if (!tgt[0]) { api_error(q->fd, 400, "Target account required."); return true; }
    const char *reason_s = rsn[0] ? (const char *)rsn : "admin_action";
    const char *act = !strcmp(sub, "grants/hold") ? "hold" : !strcmp(sub, "grants/void") ? "void" : "clawback";

    PGresult *tx = PQexec(db, "BEGIN");
    if (PQresultStatus(tx) != PGRES_COMMAND_OK) { PQclear(tx); api_error(q->fd, 500, "Database busy."); return true; }
    PQclear(tx);

    char lk[96];
    snprintf(lk, sizeof lk, "ref_acct:%s", (const char *)tgt);
    rf_advisory_lock(db, lk);

    long long affected_micros = 0;
    if (!strcmp(act, "hold")) {
      const char *v[2] = {(const char *)tgt, reason_s};
      PGresult *r1 = PQexecParams(db, "UPDATE referral.attributions SET held = true, hold_reason = $2 WHERE invitee_account_id = $1", 2, NULL, v, NULL, NULL, 0);
      PQclear(r1);
      PGresult *r2 = PQexecParams(db, "UPDATE referral.grants SET reason = $2 WHERE account_id = $1 AND status = 'pending'", 2, NULL, v, NULL, NULL, 0);
      PQclear(r2);
    } else if (!strcmp(act, "void")) {
      const char *v[2] = {(const char *)tgt, reason_s};
      PGresult *r = PQexecParams(
          db,
          "UPDATE referral.grants SET status = 'void', reason = $2, voided_at = now() "
          "WHERE account_id = $1 AND status = 'pending' RETURNING amount_micros",
          2, NULL, v, NULL, NULL, 0);
      if (PQresultStatus(r) == PGRES_TUPLES_OK) {
        for (int i = 0; i < PQntuples(r); i++) affected_micros += atoll(PQgetvalue(r, i, 0));
      }
      PQclear(r);
    } else { // clawback: void pending + zero remaining unspent promo_credit; NEVER touch paid inference.credit!
      const char *v[2] = {(const char *)tgt, reason_s};
      PGresult *r1 = PQexecParams(
          db,
          "UPDATE referral.grants SET status = 'void', reason = $2, voided_at = now() "
          "WHERE account_id = $1 AND status = 'pending'",
          2, NULL, v, NULL, NULL, 0);
      PQclear(r1);

      PGresult *sr = PQexecParams(
          db,
          "SELECT coalesce(sum(remaining_micros), 0)::bigint FROM inference.promo_credit "
          "WHERE account_id = $1 AND remaining_micros > 0",
          1, NULL, v, NULL, NULL, 0);
      if (PQresultStatus(sr) == PGRES_TUPLES_OK && PQntuples(sr) == 1) affected_micros = atoll(PQgetvalue(sr, 0, 0));
      PQclear(sr);

      PGresult *zr = PQexecParams(
          db,
          "UPDATE inference.promo_credit SET remaining_micros = 0 WHERE account_id = $1 AND remaining_micros > 0",
          1, NULL, v, NULL, NULL, 0);
      PQclear(zr);

      PGresult *cr = PQexecParams(
          db,
          "UPDATE referral.grants SET status = 'clawed_back', reason = $2, voided_at = now() "
          "WHERE account_id = $1 AND status = 'granted'",
          2, NULL, v, NULL, NULL, 0);
      PQclear(cr);
    }

    rf_admin_audit(db, admin_id, act, (const char *)tgt, reason_s);
    PQclear(PQexec(db, "COMMIT"));

    uint8_t out[512];
    int64_t n = rfw_admin_action_write(rf_cs(act), true, rf_cs((const char *)tgt), affected_micros, rf_cs(reason_s), (coax_v_u8){out, sizeof out});
    rf_respond_json(q->fd, 200, out, (size_t)n);
    return true;
  }

  // POST /api/v1/referral/admin/cap
  if (!strcmp(sub, "cap") && is_post) {
    uint8_t tgt[65] = {0}, rsn[160] = {0};
    int64_t vals[4] = {0};
    if (!rfw_admin_target_req((coax_string){q->body, (int64_t)q->body_len}, (coax_v_u8){tgt, 64}, (coax_v_u8){rsn, 159}, (coax_v_s64){vals, 4}) ||
        !tgt[0] || vals[2] < 0) {
      api_error(q->fd, 400, "Send {\"account\":\"acct_...\",\"cap\":N}.");
      return true;
    }
    tgt[vals[0]] = 0; rsn[vals[1]] = 0;
    char pcode[16];
    if (!rf_ensure_personal_code(db, (const char *)tgt, pcode)) {
      api_error(q->fd, 500, "Could not provision personal code for cap override.");
      return true;
    }
    char cap_s[24];
    snprintf(cap_s, sizeof cap_s, "%lld", (long long)vals[2]);
    const char *v[2] = {(const char *)tgt, cap_s};
    PGresult *r = PQexecParams(db, "UPDATE referral.codes SET inviter_cap_override = $2::int WHERE owner_account_id = $1 AND kind = 'personal'", 2, NULL, v, NULL, NULL, 0);
    PQclear(r);
    rf_admin_audit(db, admin_id, "cap", (const char *)tgt, rsn[0] ? (const char *)rsn : cap_s);
    uint8_t out[512];
    int64_t n = rfw_admin_action_write(rf_cs("cap"), true, rf_cs((const char *)tgt), vals[2], rf_cs("cap_updated"), (coax_v_u8){out, sizeof out});
    rf_respond_json(q->fd, 200, out, (size_t)n);
    return true;
  }

  // POST /api/v1/referral/admin/flag
  if (!strcmp(sub, "flag") && is_post) {
    uint8_t tgt[65] = {0}, rsn[160] = {0};
    int64_t vals[4] = {0};
    if (!rfw_admin_target_req((coax_string){q->body, (int64_t)q->body_len}, (coax_v_u8){tgt, 64}, (coax_v_u8){rsn, 159}, (coax_v_s64){vals, 4}) ||
        !tgt[0] || vals[3] == -1) {
      api_error(q->fd, 400, "Send {\"name\":\"referral_redeem|referral_grants|referral_kickback|github_connect_grant\",\"on\":true|false}.");
      return true;
    }
    tgt[vals[0]] = 0; rsn[vals[1]] = 0;
    const char *col = NULL;
    if (!strcmp((const char *)tgt, "referral_redeem") || !strcmp((const char *)tgt, "flag_redeem")) col = "flag_redeem";
    else if (!strcmp((const char *)tgt, "referral_grants") || !strcmp((const char *)tgt, "flag_grants")) col = "flag_grants";
    else if (!strcmp((const char *)tgt, "referral_kickback") || !strcmp((const char *)tgt, "flag_kickback")) col = "flag_kickback";
    else if (!strcmp((const char *)tgt, "github_connect_grant") || !strcmp((const char *)tgt, "flag_github_connect")) col = "flag_github_connect";
    if (!col) { api_error(q->fd, 400, "Unknown referral flag name."); return true; }

    char sql[160];
    snprintf(sql, sizeof sql, "UPDATE referral.config SET %s = $1::bool, updated_at = now() WHERE id = 1", col);
    const char *v[1] = {vals[3] ? "true" : "false"};
    PGresult *r = PQexecParams(db, sql, 1, NULL, v, NULL, NULL, 0);
    PQclear(r);
    rf_admin_audit(db, admin_id, "flag", (const char *)tgt, vals[3] ? "on" : "off");
    uint8_t out[512];
    int64_t n = rfw_admin_action_write(rf_cs("flag"), true, rf_cs((const char *)tgt), vals[3], rf_cs(vals[3] ? "on" : "off"), (coax_v_u8){out, sizeof out});
    rf_respond_json(q->fd, 200, out, (size_t)n);
    return true;
  }

  // GET / POST /api/v1/referral/admin/config
  if (!strcmp(sub, "config") && (is_get || is_post)) {
    if (is_post) {
      uint8_t key_b[65] = {0}, val_b[513] = {0}, rsn_b[160] = {0};
      int64_t lens[3] = {0};
      if (!rfw_admin_config_req((coax_string){q->body, (int64_t)q->body_len},
                                (coax_v_u8){key_b, 64}, (coax_v_u8){val_b, 512}, (coax_v_u8){rsn_b, 159}, (coax_v_s64){lens, 3})) {
        api_error(q->fd, 400, "Send {\"key\":\"...\",\"value\":\"...\"}.");
        return true;
      }
      key_b[lens[0]] = 0; val_b[lens[1]] = 0; rsn_b[lens[2]] = 0;
      const char *k = (const char *)key_b;
      const char *sql = NULL;
      bool bump_terms = false;
      if (!strcmp(k, "github_connect_micros")) { sql = "UPDATE referral.config SET github_connect_micros = $1::bigint, terms_version = terms_version + 1, updated_at = now() WHERE id = 1"; bump_terms = true; }
      else if (!strcmp(k, "invitee_micros")) { sql = "UPDATE referral.config SET invitee_micros = $1::bigint, terms_version = terms_version + 1, updated_at = now() WHERE id = 1"; bump_terms = true; }
      else if (!strcmp(k, "inviter_micros")) { sql = "UPDATE referral.config SET inviter_micros = $1::bigint, terms_version = terms_version + 1, updated_at = now() WHERE id = 1"; bump_terms = true; }
      else if (!strcmp(k, "kickback_bps")) { sql = "UPDATE referral.config SET kickback_bps = $1::int, terms_version = terms_version + 1, updated_at = now() WHERE id = 1"; bump_terms = true; }
      else if (!strcmp(k, "gh_min_age_days")) sql = "UPDATE referral.config SET gh_min_age_days = $1::int, updated_at = now() WHERE id = 1";
      else if (!strcmp(k, "gh_min_repos")) sql = "UPDATE referral.config SET gh_min_repos = $1::int, updated_at = now() WHERE id = 1";
      else if (!strcmp(k, "gh_min_followers")) sql = "UPDATE referral.config SET gh_min_followers = $1::int, updated_at = now() WHERE id = 1";
      else if (!strcmp(k, "inviter_cap_30d")) sql = "UPDATE referral.config SET inviter_cap_30d = $1::int, updated_at = now() WHERE id = 1";
      else if (!strcmp(k, "promo_daily_cap_micros")) sql = "UPDATE referral.config SET promo_daily_cap_micros = $1::bigint, updated_at = now() WHERE id = 1";
      else if (!strcmp(k, "promo_expiry_days")) sql = "UPDATE referral.config SET promo_expiry_days = $1::int, updated_at = now() WHERE id = 1";
      else if (!strcmp(k, "kickback_inactivity_days")) sql = "UPDATE referral.config SET kickback_inactivity_days = $1::int, updated_at = now() WHERE id = 1";
      else if (!strcmp(k, "redeem_window_days")) sql = "UPDATE referral.config SET redeem_window_days = $1::int, updated_at = now() WHERE id = 1";
      else if (!strcmp(k, "activation_days_needed")) sql = "UPDATE referral.config SET activation_days_needed = $1::int, updated_at = now() WHERE id = 1";
      else if (!strcmp(k, "activation_window_days")) sql = "UPDATE referral.config SET activation_window_days = $1::int, updated_at = now() WHERE id = 1";
      else if (!strcmp(k, "daily_budget_micros")) sql = "UPDATE referral.config SET daily_budget_micros = $1::bigint, updated_at = now() WHERE id = 1";
      else if (!strcmp(k, "total_budget_micros")) sql = "UPDATE referral.config SET total_budget_micros = $1::bigint, updated_at = now() WHERE id = 1";
      else if (!strcmp(k, "promo_models")) sql = "UPDATE referral.config SET promo_models = $1, updated_at = now() WHERE id = 1";
      (void)bump_terms;
      if (!sql) { api_error(q->fd, 400, "Unknown config key."); return true; }
      const char *v[1] = {(const char *)val_b};
      PGresult *ur = PQexecParams(db, sql, 1, NULL, v, NULL, NULL, 0);
      bool ok = PQresultStatus(ur) == PGRES_COMMAND_OK;
      PQclear(ur);
      if (!ok) { api_error(q->fd, 400, "Invalid config value."); return true; }
      rf_admin_audit(db, admin_id, "config", k, rsn_b[0] ? (const char *)rsn_b : (const char *)val_b);
    }

    PGresult *r = PQexec(db, "SELECT row_to_json(c)::text FROM referral.config c WHERE id = 1");
    const char *cfg_json = (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) ? PQgetvalue(r, 0, 0) : "{}";
    uint8_t out[4096];
    int64_t n = rfw_admin_config_write(rf_cs(cfg_json), (coax_v_u8){out, sizeof out});
    PQclear(r);
    if (n < 1) api_error(q->fd, 500, "Could not encode config.");
    else rf_respond_json(q->fd, 200, out, (size_t)n);
    return true;
  }

  return false;
}

// ---- Main route dispatcher

bool referral_route(const request *q) {
  if (strcmp(q->path, "/api/v1/referral") != 0 && strncmp(q->path, "/api/v1/referral/", 17) != 0) return false;
  PGconn *db = api_db();
  if (!db) { api_error(q->fd, 503, "Database unavailable."); return true; }

  char as_q[16] = {0};
  api_query(q, "as", as_q, sizeof as_q);
  bool as_lines = !strcmp(as_q, "lines");

  // Public lookup route
  if (!strcmp(q->path, "/api/v1/referral/lookup")) {
    if (strcmp(q->method, "GET") != 0) { api_error(q->fd, 405, "GET this."); return true; }
    rf_handle_lookup(q, db, as_lines);
    return true;
  }

  // Admin routes
  if (!strncmp(q->path, "/api/v1/referral/admin/", 23)) {
    return rf_handle_admin(q, db, q->path + 23);
  }

  // Authenticated person routes
  const auth_principal *p = auth_principal_of(q);
  if (!p || p->kind != AP_PERSON || !p->account[0]) {
    api_error(q->fd, 401, "Log in first.");
    return true;
  }
  bool is_get = !strcmp(q->method, "GET");
  bool is_post = !strcmp(q->method, "POST");
  if (is_post && p->credential == AUTH_COOKIE && !auth_same_site(q)) {
    api_error(q->fd, 403, "Cross-site write refused.");
    return true;
  }
  if (!api_limit(db, "referral", p->account, 60, NULL)) {
    api_error(q->fd, 429, "Try again shortly.");
    return true;
  }

  if (!strcmp(q->path, "/api/v1/referral")) {
    if (!is_get) { api_error(q->fd, 405, "GET this."); return true; }
    rf_handle_summary(q, db, p, as_lines);
    return true;
  }
  if (!strcmp(q->path, "/api/v1/referral/redeem")) {
    if (!is_post) { api_error(q->fd, 405, "POST this."); return true; }
    rf_handle_redeem(q, db, p, as_lines);
    return true;
  }
  if (!strcmp(q->path, "/api/v1/referral/vanity")) {
    if (!is_post) { api_error(q->fd, 405, "POST this."); return true; }
    rf_handle_vanity(q, db, p, as_lines);
    return true;
  }
  if (!strcmp(q->path, "/api/v1/referral/recheck")) {
    if (!is_post) { api_error(q->fd, 405, "POST this."); return true; }
    rf_handle_recheck(q, db, p, as_lines);
    return true;
  }
  return false;
}
