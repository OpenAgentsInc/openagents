// Central metered Exa web search, fetch, and first-party Streamable HTTP MCP proxy
// (#14114, #14115, docs/exa/SPEC.md):
//
//   POST /api/v1/search    metered Exa /search (Dynamic Highlights default), NIP-98 or account session
//   POST /api/v1/fetch     metered Exa /contents (SSRF-guarded, max 5 URLs), NIP-98 or account session
//   POST /api/v1/mcp/exa   first-party metered Streamable HTTP MCP server (MCP 2025-03-26, JSON-RPC 2.0)
//                          exposing web_search_exa and web_fetch_exa on the same billing pipeline
//
// Billing: holds a single central EXA_API_KEY on the server, parses Exa's exact costDollars.total
// via exa-policy.coax (exa_cost_dollars_to_micros), applies OpenAgents' retail markup (+25%),
// records every call in inference.calls (kind = 'search'), and debits prepaid inference.credit
// in msat (free for admins and OA_INFERENCE_PUBKEYS).
#include "api.h"
#include "exa-policy.generated.h"

#include <curl/curl.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define EXA_DEFAULT_BASE_URL "https://api.exa.ai"
#define EXA_DYNAMIC_HIGHLIGHTS_BETA "dynamic-highlights-2026-08-28"
#define EXA_MAX_REQ_BYTES (64 * 1024)
#define EXA_MAX_RESP_BYTES (4u << 20)

static const char *exa_env_or(const char *name, const char *fallback) {
  const char *v = getenv(name);
  return (v && *v) ? v : fallback;
}

static long long exa_env_i64(const char *name, long long fallback) {
  const char *v = getenv(name);
  if (!v || !*v) return fallback;
  char *end = NULL;
  long long n = strtoll(v, &end, 10);
  if (!end || *end != '\0' || n < 0) return fallback;
  return n;
}

static coax_string exa_cs(const char *s) {
  return (coax_string){s ? s : "", s ? (int64_t)strlen(s) : 0};
}

typedef struct {
  char *p;
  size_t n;
  size_t cap;
} exa_buf;

static size_t exa_write_cb(char *data, size_t size, size_t nmemb, void *user) {
  exa_buf *b = (exa_buf *)user;
  size_t k = size * nmemb;
  if (k == 0) return 0;
  if (b->n + k + 1 > EXA_MAX_RESP_BYTES) return 0;
  if (b->n + k + 1 > b->cap) {
    size_t cap = b->cap ? b->cap * 2 : 4096;
    while (cap < b->n + k + 1) cap *= 2;
    if (cap > EXA_MAX_RESP_BYTES) cap = EXA_MAX_RESP_BYTES;
    char *p = realloc(b->p, cap);
    if (!p) return 0;
    b->p = p;
    b->cap = cap;
  }
  memcpy(b->p + b->n, data, k);
  b->n += k;
  b->p[b->n] = 0;
  return k;
}

// POST req_json to {OA_EXA_BASE_URL}{path} using central EXA_API_KEY.
// Returns HTTP status (>0), 0 on transport error, or -1 when EXA_API_KEY is not configured.
static long exa_post_upstream(const char *path, const char *req_json, bool dynamic_highlights,
                              long timeout_s, char **body_out) {
  *body_out = NULL;
  const char *api_key = getenv("EXA_API_KEY");
  if (!api_key || !*api_key) return -1;

  const char *base = exa_env_or("OA_EXA_BASE_URL", EXA_DEFAULT_BASE_URL);
  size_t blen = strlen(base);
  while (blen > 1 && base[blen - 1] == '/') blen--;

  char url[1024];
  int un = snprintf(url, sizeof url, "%.*s%s", (int)blen, base, path);
  if (un <= 0 || (size_t)un >= sizeof url) return 0;

  char key_hdr[512];
  int kn = snprintf(key_hdr, sizeof key_hdr, "x-api-key: %s", api_key);
  if (kn <= 0 || (size_t)kn >= sizeof key_hdr) {
    explicit_bzero(key_hdr, sizeof key_hdr);
    return 0;
  }

  CURL *h = curl_easy_init();
  if (!h) {
    explicit_bzero(key_hdr, sizeof key_hdr);
    return 0;
  }

  exa_buf b = {0};
  struct curl_slist *hd = NULL;
  hd = curl_slist_append(hd, "Content-Type: application/json");
  hd = curl_slist_append(hd, "Accept: application/json");
  hd = curl_slist_append(hd, key_hdr);
  hd = curl_slist_append(hd, "x-exa-integration: openagents-api");
  if (dynamic_highlights) {
    hd = curl_slist_append(hd, "Exa-Beta: " EXA_DYNAMIC_HIGHLIGHTS_BETA);
  }

  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  curl_easy_setopt(h, CURLOPT_POSTFIELDS, req_json);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, exa_write_cb);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, timeout_s);

  long status = 0;
  CURLcode rc = curl_easy_perform(h);
  if (rc == CURLE_OK) {
    curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  } else {
    api_log("exa: upstream %s failed (curl %d)", path, (int)rc);
  }

  curl_slist_free_all(hd);
  curl_easy_cleanup(h);
  explicit_bzero(key_hdr, sizeof key_hdr);

  *body_out = b.p;
  return status;
}

typedef struct {
  int status;
  char *json;
  char *markdown;
  char *meta_json;
  char err[256];
  bool credit_refused_sent;
} exa_exec_res;

static void exa_exec_res_free(exa_exec_res *r) {
  free(r->json);
  free(r->markdown);
  free(r->meta_json);
  memset(r, 0, sizeof *r);
}

static bool exa_has_key(void) {
  const char *k = getenv("EXA_API_KEY");
  return k && *k;
}

// Validate a JSON array of strings in Postgres as bare domains using exa_valid_domain().
static bool exa_validate_domains_sql(PGconn *db, const char *arr_json, char *err, size_t errcap) {
  if (!arr_json || !*arr_json || !strcmp(arr_json, "[]")) return true;
  const char *v[1] = {arr_json};
  PGresult *r = PQexecParams(
      db,
      "SELECT value FROM jsonb_array_elements_text($1::jsonb)",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) {
    PQclear(r);
    snprintf(err, errcap, "domains must be an array of strings");
    return false;
  }
  int n = PQntuples(r);
  if (n > 20) {
    PQclear(r);
    snprintf(err, errcap, "at most 20 domains are allowed");
    return false;
  }
  for (int i = 0; i < n; i++) {
    const char *dom = PQgetvalue(r, i, 0);
    if (!exa_valid_domain(exa_cs(dom))) {
      PQclear(r);
      snprintf(err, errcap, "invalid domain in filter");
      return false;
    }
  }
  PQclear(r);
  return true;
}

// Extract Exa's costDollars.total as a decimal string via Postgres JSONB (returns 0 on missing/non-JSON).
static long long exa_extract_cost_micros(PGconn *db, const char *raw_resp) {
  if (!raw_resp || !*raw_resp) return 0;
  const char *v[1] = {raw_resp};
  PGresult *r = PQexecParams(
      db,
      "SELECT coalesce("
      "  CASE WHEN pg_input_is_valid($1, 'jsonb') THEN ($1::jsonb)->'costDollars'->>'total' ELSE NULL END,"
      "  ''"
      ")",
      1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    if (r) PQclear(r);
    return 0;
  }
  long long micros = (long long)exa_cost_dollars_to_micros(exa_cs(PQgetvalue(r, 0, 0)));
  PQclear(r);
  return micros;
}

static void exa_rollback(PGconn *db) {
  PGresult *rb = PQexec(db, "ROLLBACK");
  if (rb) PQclear(rb);
}

// Run admission check in a plain SQL transaction using #authority exa-policy.coax
// (wallet daily limit, BTC rate refresh, credit > 0, fresh BTC/USD rate, daily caps).
// Returns true if admitted; otherwise populates res->status / res->json / res->err.
static bool exa_admit_call(PGconn *db, const request *q, const char *who, bool free_key,
                           const char *model_name, exa_exec_res *res) {
  if (!free_key) {
    if (wallet_credit_refused(q, db, who)) {
      res->credit_refused_sent = true;
      res->status = 401;
      return false;
    }
    inference_rate_refresh(db);
  }

  long long cap_n = exa_env_i64("OA_EXA_DAILY_REQUESTS", 250);
  long long cap_usd = exa_env_i64("OA_EXA_DAILY_USD_MICROS", 2000000);

  PGresult *tx = PQexec(db, "BEGIN");
  if (!tx || PQresultStatus(tx) != PGRES_COMMAND_OK) {
    api_log("exa: admit begin failed: %s", PQerrorMessage(db));
    if (tx) PQclear(tx);
    res->status = 500;
    snprintf(res->err, sizeof res->err, "search admission failed");
    return false;
  }
  PQclear(tx);

  const char *mv[1] = {model_name};
  PGresult *mr = PQexecParams(
      db,
      "SELECT 1 FROM inference.models WHERE name = $1 AND kind = 'search'",
      1, NULL, mv, NULL, NULL, 0);
  if (!mr || PQresultStatus(mr) != PGRES_TUPLES_OK) {
    api_log("exa: admit model lookup failed: %s", PQerrorMessage(db));
    if (mr) PQclear(mr);
    exa_rollback(db);
    res->status = 500;
    snprintf(res->err, sizeof res->err, "search admission failed");
    return false;
  }
  bool model_found = PQntuples(mr) == 1;
  PQclear(mr);

  long long credit_msat = 0;
  bool has_rate = false;
  long long rate_age_ms = 0;
  if (model_found && !free_key) {
    const char *wv[1] = {who};
    PGresult *cr = PQexecParams(
        db,
        "SELECT msat FROM inference.credit WHERE pubkey = $1",
        1, NULL, wv, NULL, NULL, 0);
    if (!cr || PQresultStatus(cr) != PGRES_TUPLES_OK) {
      api_log("exa: admit credit lookup failed: %s", PQerrorMessage(db));
      if (cr) PQclear(cr);
      exa_rollback(db);
      res->status = 500;
      snprintf(res->err, sizeof res->err, "search admission failed");
      return false;
    }
    if (PQntuples(cr) == 1) credit_msat = atoll(PQgetvalue(cr, 0, 0));
    PQclear(cr);

    PGresult *rr = PQexec(
        db,
        "SELECT ceil(extract(epoch FROM now() - at) * 1000)::bigint FROM inference.btc_usd");
    if (!rr || PQresultStatus(rr) != PGRES_TUPLES_OK) {
      api_log("exa: admit rate lookup failed: %s", PQerrorMessage(db));
      if (rr) PQclear(rr);
      exa_rollback(db);
      res->status = 500;
      snprintf(res->err, sizeof res->err, "search admission failed");
      return false;
    }
    if (PQntuples(rr) == 1) {
      has_rate = true;
      rate_age_ms = atoll(PQgetvalue(rr, 0, 0));
    }
    PQclear(rr);
  }

  int64_t pre = exa_admit_precheck(model_found, free_key, credit_msat, has_rate, rate_age_ms);
  if (pre != 200) {
    exa_rollback(db);
    res->status = (int)pre;
    if (pre == 400) {
      snprintf(res->err, sizeof res->err, "unknown search service");
    } else if (pre == 402) {
      snprintf(res->err, sizeof res->err, "add credit: POST /api/v1/inference/credit");
      char origin[600], cred_s[32];
      snprintf(origin, sizeof origin, "%s/api/v1/inference/credit",
               exa_env_or("OA_API_ORIGIN", "https://openagents.com"));
      snprintf(cred_s, sizeof cred_s, "%lld", credit_msat);
      const char *jv[2] = {origin, cred_s};
      PGresult *jr = PQexecParams(
          db,
          "SELECT jsonb_build_object("
          "  'error', 'add credit: POST /api/v1/inference/credit',"
          "  'credit', $1::text,"
          "  'credit_msat', $2::bigint"
          ")::text",
          2, NULL, jv, NULL, NULL, 0);
      if (jr && PQresultStatus(jr) == PGRES_TUPLES_OK && PQntuples(jr) == 1) {
        res->json = strdup(PQgetvalue(jr, 0, 0));
      }
      if (jr) PQclear(jr);
    } else {
      snprintf(res->err, sizeof res->err, "no bitcoin price to charge at; try again soon");
    }
    return false;
  }

  const char *dv[1] = {who};
  PGresult *dr = PQexecParams(
      db,
      "INSERT INTO inference.account_day AS a (pubkey, day, n) "
      "VALUES ($1, current_date, 1) "
      "ON CONFLICT (pubkey, day) DO UPDATE SET n = a.n + 1 "
      "RETURNING n, usd_micros",
      1, NULL, dv, NULL, NULL, 0);
  if (!dr || PQresultStatus(dr) != PGRES_TUPLES_OK || PQntuples(dr) != 1) {
    api_log("exa: admit quota update failed: %s", PQerrorMessage(db));
    if (dr) PQclear(dr);
    exa_rollback(db);
    res->status = 500;
    snprintf(res->err, sizeof res->err, "search admission failed");
    return false;
  }
  long long day_n = atoll(PQgetvalue(dr, 0, 0));
  long long day_usd = atoll(PQgetvalue(dr, 0, 1));
  PQclear(dr);

  int64_t qdec = exa_admit_quota(day_n, cap_n, day_usd, cap_usd);
  if (qdec != 200) {
    exa_rollback(db);
    res->status = 429;
    if (qdec == 4291) {
      snprintf(res->err, sizeof res->err, "this account's %lld requests for today are used up", cap_n);
    } else {
      snprintf(res->err, sizeof res->err, "this account's $%.2f spend for today is used up",
               (double)cap_usd / 1000000.0);
    }
    return false;
  }

  PGresult *cm = PQexec(db, "COMMIT");
  bool ok = cm && PQresultStatus(cm) == PGRES_COMMAND_OK;
  if (cm) PQclear(cm);
  if (!ok) {
    api_log("exa: admit commit failed: %s", PQerrorMessage(db));
    exa_rollback(db);
    res->status = 500;
    snprintf(res->err, sizeof res->err, "search admission failed");
    return false;
  }
  return true;
}

// Record and settle a completed Exa search/fetch call in a plain SQL transaction
// using #authority exa-policy.coax for retail markup, upstream cost, and msat ceiling math.
static char *exa_finish_call(PGconn *db, const char *who, const char *model_name,
                             long http_status, long long upstream_micros, int taken_ms,
                             bool free_key) {
  int status_int = (http_status > 0 && http_status <= INT_MAX) ? (int)http_status : 0;
  int ms_clamped = taken_ms > 0 ? taken_ms : 0;

  PGresult *tx = PQexec(db, "BEGIN");
  if (!tx || PQresultStatus(tx) != PGRES_COMMAND_OK) {
    api_log("exa: finish begin (%s) failed: %s", model_name, PQerrorMessage(db));
    if (tx) PQclear(tx);
    goto fallback;
  }
  PQclear(tx);

  long long cost = 0, up_cost = 0, msat_due = 0;
  const char *mv[1] = {model_name};
  PGresult *mr = PQexecParams(
      db,
      "SELECT per_call_micros, markup_bps FROM inference.models WHERE name = $1 AND kind = 'search'",
      1, NULL, mv, NULL, NULL, 0);
  if (!mr || PQresultStatus(mr) != PGRES_TUPLES_OK) {
    api_log("exa: finish model lookup (%s) failed: %s", model_name, PQerrorMessage(db));
    if (mr) PQclear(mr);
    goto fail;
  }
  if (PQntuples(mr) == 1) {
    long long fallback_retail = atoll(PQgetvalue(mr, 0, 0));
    long long markup_bps = atoll(PQgetvalue(mr, 0, 1));
    cost = (long long)exa_retail_micros(upstream_micros, markup_bps, fallback_retail, status_int);
    up_cost = (long long)exa_upstream_micros(upstream_micros, markup_bps, fallback_retail, status_int);
  }
  PQclear(mr);

  if (!free_key && cost > 0) {
    PGresult *rr = PQexec(db, "SELECT usd_per_btc FROM inference.btc_usd");
    if (!rr || PQresultStatus(rr) != PGRES_TUPLES_OK) {
      api_log("exa: finish rate lookup (%s) failed: %s", model_name, PQerrorMessage(db));
      if (rr) PQclear(rr);
      goto fail;
    }
    long long rate = (PQntuples(rr) == 1) ? atoll(PQgetvalue(rr, 0, 0)) : 0;
    PQclear(rr);
    msat_due = (long long)exa_msat_due(cost, rate, free_key);
  }

  char st_s[16], cost_s[32], up_s[32], ms_s[16], msat_s[32];
  snprintf(st_s, sizeof st_s, "%d", status_int);
  snprintf(cost_s, sizeof cost_s, "%lld", cost);
  snprintf(up_s, sizeof up_s, "%lld", up_cost);
  snprintf(ms_s, sizeof ms_s, "%d", ms_clamped);
  snprintf(msat_s, sizeof msat_s, "%lld", msat_due);

  if (cost > 0) {
    const char *dv[2] = {who, cost_s};
    PGresult *dr = PQexecParams(
        db,
        "INSERT INTO inference.account_day AS a (pubkey, day, n, usd_micros) "
        "VALUES ($1, current_date, 0, $2::bigint) "
        "ON CONFLICT (pubkey, day) DO UPDATE SET usd_micros = a.usd_micros + EXCLUDED.usd_micros",
        2, NULL, dv, NULL, NULL, 0);
    if (!dr || PQresultStatus(dr) != PGRES_COMMAND_OK) {
      api_log("exa: finish account_day (%s) failed: %s", model_name, PQerrorMessage(db));
      if (dr) PQclear(dr);
      goto fail;
    }
    PQclear(dr);
  }

  const char *cv[7] = {who, model_name, st_s, cost_s, up_s, ms_s, msat_s};
  PGresult *cr = PQexecParams(
      db,
      "INSERT INTO inference.calls "
      "  (ms, pubkey, model, status, in_tokens, cached_in_tokens, out_tokens, "
      "   usd_micros, upstream_usd_micros, ms_taken, msat, kind) "
      "VALUES "
      "  ((extract(epoch FROM clock_timestamp()) * 1000)::bigint, $1, $2, $3::int, 0, 0, 0, "
      "   $4::bigint, $5::bigint, $6::int, $7::bigint, 'search')",
      7, NULL, cv, NULL, NULL, 0);
  if (!cr || PQresultStatus(cr) != PGRES_COMMAND_OK) {
    api_log("exa: finish insert call (%s) failed: %s", model_name, PQerrorMessage(db));
    if (cr) PQclear(cr);
    goto fail;
  }
  PQclear(cr);

  if (!free_key && cost > 0) {
    const char *uv[2] = {who, msat_s};
    PGresult *ur = PQexecParams(
        db,
        "UPDATE inference.credit SET msat = msat - $2::bigint, updated = now() WHERE pubkey = $1",
        2, NULL, uv, NULL, NULL, 0);
    if (!ur || PQresultStatus(ur) != PGRES_COMMAND_OK) {
      api_log("exa: finish credit debit (%s) failed: %s", model_name, PQerrorMessage(db));
      if (ur) PQclear(ur);
      goto fail;
    }
    PQclear(ur);

    if (msat_due > 0) {
      PGresult *sr = PQexecParams(
          db,
          "INSERT INTO auth.credit_spends AS c (account, hour, msat) "
          "SELECT a.id, date_trunc('hour', now()), $2::bigint "
          "  FROM auth.accounts a "
          "  LEFT JOIN auth.nostr_keys k ON k.account = a.id "
          " WHERE a.kind = 'person' AND (k.pubkey = $1 OR a.id = $1) "
          " GROUP BY a.id "
          "ON CONFLICT (account, hour) DO UPDATE SET msat = c.msat + EXCLUDED.msat",
          2, NULL, uv, NULL, NULL, 0);
      if (!sr || PQresultStatus(sr) != PGRES_COMMAND_OK) {
        api_log("exa: finish credit_spends (%s) failed: %s", model_name, PQerrorMessage(db));
        if (sr) PQclear(sr);
        goto fail;
      }
      PQclear(sr);
    }
  }

  PGresult *cm = PQexec(db, "COMMIT");
  bool ok = cm && PQresultStatus(cm) == PGRES_COMMAND_OK;
  if (cm) PQclear(cm);
  if (!ok) {
    api_log("exa: finish commit (%s) failed: %s", model_name, PQerrorMessage(db));
    goto fail;
  }

  char buf[256];
  snprintf(buf, sizeof buf,
           "{\"model\":\"%s\",\"upstream_usd_micros\":%lld,\"usd_micros\":%lld,\"msat\":%lld,\"free\":%s}",
           model_name, up_cost, cost, msat_due, free_key ? "true" : "false");
  return strdup(buf);

fail:
  exa_rollback(db);
fallback: {
  char buf[256];
  snprintf(buf, sizeof buf,
           "{\"model\":\"%s\",\"upstream_usd_micros\":0,\"usd_micros\":0,\"msat\":0,\"free\":%s}",
           model_name, free_key ? "true" : "false");
  return strdup(buf);
}
}

// Execute a metered Exa /search call for REST (/api/v1/search) or MCP (web_search_exa).
static void exa_execute_search(PGconn *db, const request *q, const char *who, bool free_key,
                               const char *args_json, exa_exec_res *res) {
  memset(res, 0, sizeof *res);

  // 1. Parse and normalize input arguments in Postgres JSONB.
  const char *pv[1] = {args_json};
  PGresult *pr = PQexecParams(
      db,
      "WITH raw AS ("
      "  SELECT CASE WHEN pg_input_is_valid($1, 'jsonb') THEN $1::jsonb ELSE NULL END AS b"
      "), parsed AS ("
      "  SELECT b,"
      "    btrim(coalesce(b->>'query', '')) AS raw_query,"
      "    coalesce(b->>'type', 'auto') AS search_type,"
      "    lower(btrim(coalesce(b->>'category', ''))) AS explicit_cat,"
      "    CASE WHEN jsonb_typeof(b->'num_results') = 'number' THEN (b->>'num_results')::numeric::int "
      "         WHEN jsonb_typeof(b->'numResults') = 'number' THEN (b->>'numResults')::numeric::int "
      "         ELSE 0 END AS num_results,"
      "    CASE WHEN jsonb_typeof(b->'max_characters') = 'number' THEN (b->>'max_characters')::numeric::int "
      "         WHEN jsonb_typeof(b->'maxCharacters') = 'number' THEN (b->>'maxCharacters')::numeric::int "
      "         ELSE 0 END AS max_chars,"
      "    CASE WHEN jsonb_typeof(b->'highlights') = 'boolean' THEN (b->'highlights')::bool ELSE true END AS use_highlights,"
      "    coalesce(b->'include_domains', b->'allowed_domains') AS inc_dom,"
      "    coalesce(b->'exclude_domains', b->'blocked_domains') AS exc_dom,"
      "    coalesce(b->>'start_published_date', b->>'startPublishedDate', '') AS start_date,"
      "    coalesce(b->>'end_published_date', b->>'endPublishedDate', '') AS end_date"
      "  FROM raw"
      "), cat AS ("
      "  SELECT *,"
      "    (regexp_match(raw_query, '\\mcategory:(company|research\\s+paper|news|personal\\s+site|financial\\s+report|people|tweet|github|pdf)\\M', 'i'))[1] AS inline_cat"
      "  FROM parsed"
      ")"
      "SELECT "
      "  (b IS NOT NULL AND jsonb_typeof(b) = 'object') AS valid_obj,"
      "  btrim(CASE WHEN explicit_cat = '' AND inline_cat IS NOT NULL "
      "             THEN regexp_replace(raw_query, '\\mcategory:(company|research\\s+paper|news|personal\\s+site|financial\\s+report|people|tweet|github|pdf)\\M', '', 'i') "
      "             ELSE raw_query END) AS clean_query,"
      "  lower(btrim(search_type)) AS search_type,"
      "  CASE WHEN explicit_cat <> '' THEN explicit_cat "
      "       WHEN inline_cat IS NOT NULL THEN lower(regexp_replace(inline_cat, '\\s+', ' ', 'g')) "
      "       ELSE '' END AS category,"
      "  num_results, max_chars, use_highlights,"
      "  CASE WHEN inc_dom IS NULL OR inc_dom = 'null'::jsonb THEN '[]' "
      "       WHEN jsonb_typeof(inc_dom) = 'array' THEN inc_dom::text ELSE 'INVALID' END AS inc_dom,"
      "  CASE WHEN exc_dom IS NULL OR exc_dom = 'null'::jsonb THEN '[]' "
      "       WHEN jsonb_typeof(exc_dom) = 'array' THEN exc_dom::text ELSE 'INVALID' END AS exc_dom,"
      "  start_date, end_date "
      "FROM cat",
      1, NULL, pv, NULL, NULL, 0);
  if (PQresultStatus(pr) != PGRES_TUPLES_OK || PQntuples(pr) != 1) {
    api_log("exa: parse search args failed: %s", PQerrorMessage(db));
    if (pr) PQclear(pr);
    res->status = 400;
    snprintf(res->err, sizeof res->err, "invalid search request");
    return;
  }

  if (strcmp(PQgetvalue(pr, 0, 0), "t") != 0) {
    PQclear(pr);
    res->status = 400;
    snprintf(res->err, sizeof res->err, "request body must be a JSON object");
    return;
  }

  char *query = strdup(PQgetvalue(pr, 0, 1));
  char search_type[32]; snprintf(search_type, sizeof search_type, "%s", PQgetvalue(pr, 0, 2));
  char category[32]; snprintf(category, sizeof category, "%s", PQgetvalue(pr, 0, 3));
  int req_num = atoi(PQgetvalue(pr, 0, 4));
  int req_chars = atoi(PQgetvalue(pr, 0, 5));
  bool use_highlights = !strcmp(PQgetvalue(pr, 0, 6), "t");
  char *inc_dom = strdup(PQgetvalue(pr, 0, 7));
  char *exc_dom = strdup(PQgetvalue(pr, 0, 8));
  char start_date[64]; snprintf(start_date, sizeof start_date, "%s", PQgetvalue(pr, 0, 9));
  char end_date[64]; snprintf(end_date, sizeof end_date, "%s", PQgetvalue(pr, 0, 10));
  PQclear(pr);

  if (!query || !inc_dom || !exc_dom) {
    free(query); free(inc_dom); free(exc_dom);
    res->status = 503;
    snprintf(res->err, sizeof res->err, "memory");
    return;
  }

  // 2. Enforce #authority exa-policy.coax guardrails on the shared key.
  size_t qlen = strlen(query);
  if (qlen == 0 || qlen > 2000) {
    free(query); free(inc_dom); free(exc_dom);
    res->status = 400;
    snprintf(res->err, sizeof res->err, "query is required (1..2000 bytes)");
    return;
  }
  if (!exa_valid_search_type(exa_cs(search_type))) {
    free(query); free(inc_dom); free(exc_dom);
    res->status = 400;
    snprintf(res->err, sizeof res->err, "type must be auto or fast");
    return;
  }
  if (!exa_valid_category(exa_cs(category))) {
    free(query); free(inc_dom); free(exc_dom);
    res->status = 400;
    snprintf(res->err, sizeof res->err, "unsupported search category");
    return;
  }
  if (!strcmp(inc_dom, "INVALID") || !strcmp(exc_dom, "INVALID")) {
    free(query); free(inc_dom); free(exc_dom);
    res->status = 400;
    snprintf(res->err, sizeof res->err, "allowed_domains and blocked_domains must be arrays of domain strings");
    return;
  }
  bool has_inc = strcmp(inc_dom, "[]") != 0;
  bool has_exc = strcmp(exc_dom, "[]") != 0;
  if (has_inc && has_exc) {
    free(query); free(inc_dom); free(exc_dom);
    res->status = 400;
    snprintf(res->err, sizeof res->err, "allowed_domains and blocked_domains cannot both be set");
    return;
  }
  if ((has_inc && !exa_validate_domains_sql(db, inc_dom, res->err, sizeof res->err)) ||
      (has_exc && !exa_validate_domains_sql(db, exc_dom, res->err, sizeof res->err))) {
    free(query); free(inc_dom); free(exc_dom);
    res->status = 400;
    return;
  }

  int num_results = (int)exa_clamp_num_results(req_num);
  int max_chars = (int)exa_clamp_max_chars(req_chars);
  const char *effective_type = search_type[0] ? search_type : "auto";

  // 3. Ensure server key is configured before charging/counting daily quota.
  if (!exa_has_key()) {
    free(query); free(inc_dom); free(exc_dom);
    res->status = 503;
    snprintf(res->err, sizeof res->err, "web search is not configured");
    return;
  }

  // 4. Admission check (credit > 0, fresh BTC/USD rate, daily request & USD caps).
  if (!exa_admit_call(db, q, who, free_key, "exa/search", res)) {
    free(query); free(inc_dom); free(exc_dom);
    return;
  }

  // 5. Build sanitized Exa /search payload via Postgres JSONB.
  char num_s[16], chars_s[16];
  snprintf(num_s, sizeof num_s, "%d", num_results);
  snprintf(chars_s, sizeof chars_s, "%d", max_chars);
  const char *bv[9] = {
      query, effective_type, num_s, category,
      use_highlights ? "true" : "false", chars_s,
      inc_dom, exc_dom, start_date,
  };
  const char *bv2[10] = {
      bv[0], bv[1], bv[2], bv[3], bv[4], bv[5], bv[6], bv[7], bv[8], end_date
  };
  PGresult *br = PQexecParams(
      db,
      "SELECT ("
      "  jsonb_build_object("
      "    'query', $1::text,"
      "    'type', $2::text,"
      "    'numResults', $3::int,"
      "    'contents', CASE WHEN $5::bool "
      "                     THEN '{\"highlights\":{\"dynamic\":true}}'::jsonb "
      "                     ELSE jsonb_build_object('text', jsonb_build_object('maxCharacters', $6::int)) END"
      "  )"
      "  || CASE WHEN $4::text <> '' THEN jsonb_build_object('category', $4::text) ELSE '{}'::jsonb END"
      "  || CASE WHEN $7::jsonb <> '[]'::jsonb THEN jsonb_build_object('includeDomains', $7::jsonb) ELSE '{}'::jsonb END"
      "  || CASE WHEN $8::jsonb <> '[]'::jsonb THEN jsonb_build_object('excludeDomains', $8::jsonb) ELSE '{}'::jsonb END"
      "  || CASE WHEN $9::text <> '' THEN jsonb_build_object('startPublishedDate', $9::text) ELSE '{}'::jsonb END"
      "  || CASE WHEN $10::text <> '' THEN jsonb_build_object('endPublishedDate', $10::text) ELSE '{}'::jsonb END"
      ")::text",
      10, NULL, bv2, NULL, NULL, 0);
  free(query); free(inc_dom); free(exc_dom);
  if (PQresultStatus(br) != PGRES_TUPLES_OK || PQntuples(br) != 1) {
    if (br) PQclear(br);
    res->status = 500;
    snprintf(res->err, sizeof res->err, "failed to encode search payload");
    return;
  }
  char *payload = strdup(PQgetvalue(br, 0, 0));
  PQclear(br);
  if (!payload) {
    res->status = 503;
    snprintf(res->err, sizeof res->err, "memory");
    return;
  }

  // 6. Call Exa /search upstream.
  double t0 = api_now_ms();
  char *raw_resp = NULL;
  long http_status = exa_post_upstream("/search", payload, use_highlights, 25L, &raw_resp);
  int taken_ms = (int)(api_now_ms() - t0);
  free(payload);

  // 7. Parse exact costDollars.total and settle billing in a plain SQL transaction.
  long long upstream_micros = (http_status == 200) ? exa_extract_cost_micros(db, raw_resp) : 0;
  char *billing_json = exa_finish_call(db, who, "exa/search", http_status, upstream_micros, taken_ms, free_key);
  if (!billing_json) {
    free(raw_resp);
    res->status = 503;
    snprintf(res->err, sizeof res->err, "memory");
    return;
  }

  if (http_status != 200 || !raw_resp) {
    api_log("exa: /search upstream answered %ld in %d ms", http_status, taken_ms);
    free(raw_resp);
    free(billing_json);
    res->status = (http_status == 400 || http_status == 429) ? (int)http_status : 502;
    snprintf(res->err, sizeof res->err,
             http_status == 429 ? "search provider rate limited; try again soon"
                                : "search provider did not answer");
    return;
  }

  // 8. Normalize response JSON + formatted markdown + MCP _meta in one SQL pass.
  const char *nv[2] = {raw_resp, billing_json};
  PGresult *nr = PQexecParams(
      db,
      "WITH src AS ("
      "  SELECT CASE WHEN pg_input_is_valid($1, 'jsonb') THEN $1::jsonb ELSE NULL END AS j, $2::jsonb AS b"
      "), items AS ("
      "  SELECT elem, ord "
      "  FROM src, LATERAL jsonb_array_elements(coalesce(src.j->'results', '[]'::jsonb)) WITH ORDINALITY AS t(elem, ord)"
      "), norm AS ("
      "  SELECT ord, jsonb_strip_nulls(jsonb_build_object("
      "    'title', coalesce(elem->>'title', ''),"
      "    'url', coalesce(elem->>'url', ''),"
      "    'publishedDate', elem->>'publishedDate',"
      "    'author', elem->>'author',"
      "    'highlights', CASE WHEN jsonb_typeof(elem->'highlights') = 'array' THEN elem->'highlights' ELSE NULL END,"
      "    'text', elem->>'text'"
      "  )) AS obj,"
      "  concat_ws(E'\\n',"
      "    'Title: ' || coalesce(nullif(elem->>'title', ''), 'N/A'),"
      "    'URL: ' || coalesce(elem->>'url', ''),"
      "    'Published: ' || coalesce(nullif(elem->>'publishedDate', ''), 'N/A'),"
      "    'Author: ' || coalesce(nullif(elem->>'author', ''), 'N/A'),"
      "    CASE WHEN jsonb_typeof(elem->'highlights') = 'array' AND jsonb_array_length(elem->'highlights') > 0 "
      "         THEN 'Highlights:' || E'\\n' || (SELECT string_agg(h, E'\\n') FROM jsonb_array_elements_text(elem->'highlights') h) "
      "         WHEN coalesce(elem->>'text', '') <> '' "
      "         THEN 'Text: ' || (elem->>'text') "
      "         ELSE NULL END"
      "  ) AS md "
      "  FROM items"
      ")"
      "SELECT "
      "  (SELECT j IS NOT NULL FROM src) AS ok_json,"
      "  jsonb_build_object("
      "    'results', coalesce((SELECT jsonb_agg(obj ORDER BY ord) FROM norm), '[]'::jsonb),"
      "    'searchTime', (SELECT j->'searchTime' FROM src),"
      "    'billing', (SELECT b FROM src)"
      "  )::text AS out_json,"
      "  coalesce((SELECT string_agg(md, E'\\n\\n---\\n\\n' ORDER BY ord) FROM norm), 'No search results found. Please try a different query.') AS out_md,"
      "  jsonb_build_object('searchTime', (SELECT j->'searchTime' FROM src), 'billing', (SELECT b FROM src))::text AS out_meta",
      2, NULL, nv, NULL, NULL, 0);
  free(raw_resp);
  free(billing_json);

  if (PQresultStatus(nr) != PGRES_TUPLES_OK || PQntuples(nr) != 1 || strcmp(PQgetvalue(nr, 0, 0), "t") != 0) {
    if (nr) PQclear(nr);
    res->status = 502;
    snprintf(res->err, sizeof res->err, "search provider returned invalid JSON");
    return;
  }

  res->status = 200;
  res->json = strdup(PQgetvalue(nr, 0, 1));
  res->markdown = strdup(PQgetvalue(nr, 0, 2));
  res->meta_json = strdup(PQgetvalue(nr, 0, 3));
  PQclear(nr);
  if (!res->json || !res->markdown || !res->meta_json) {
    exa_exec_res_free(res);
    res->status = 503;
    snprintf(res->err, sizeof res->err, "memory");
  }
}

// Execute a metered Exa /contents call for REST (/api/v1/fetch) or MCP (web_fetch_exa).
static void exa_execute_fetch(PGconn *db, const request *q, const char *who, bool free_key,
                              const char *args_json, exa_exec_res *res) {
  memset(res, 0, sizeof *res);

  const char *pv[1] = {args_json};
  PGresult *pr = PQexecParams(
      db,
      "WITH raw AS ("
      "  SELECT CASE WHEN pg_input_is_valid($1, 'jsonb') THEN $1::jsonb ELSE NULL END AS b"
      ")"
      "SELECT "
      "  (b IS NOT NULL AND jsonb_typeof(b) = 'object') AS valid_obj,"
      "  CASE "
      "    WHEN jsonb_typeof(b->'urls') = 'array' THEN (b->'urls')::text "
      "    WHEN jsonb_typeof(b->'urls') = 'string' AND pg_input_is_valid(b->>'urls', 'jsonb') "
      "         AND jsonb_typeof((b->>'urls')::jsonb) = 'array' THEN (b->>'urls')::jsonb::text "
      "    WHEN jsonb_typeof(b->'urls') = 'string' AND btrim(b->>'urls') <> '' THEN jsonb_build_array(btrim(b->>'urls'))::text "
      "    WHEN jsonb_typeof(b->'url') = 'string' AND btrim(b->>'url') <> '' THEN jsonb_build_array(btrim(b->>'url'))::text "
      "    ELSE 'INVALID' END AS urls_json,"
      "  CASE WHEN jsonb_typeof(b->'max_characters') = 'number' THEN (b->>'max_characters')::numeric::int "
      "       WHEN jsonb_typeof(b->'maxCharacters') = 'number' THEN (b->>'maxCharacters')::numeric::int "
      "       ELSE 0 END AS max_chars, "
      "  btrim(coalesce(b->>'query', '')) AS hl_query "
      "FROM raw",
      1, NULL, pv, NULL, NULL, 0);
  if (PQresultStatus(pr) != PGRES_TUPLES_OK || PQntuples(pr) != 1) {
    if (pr) PQclear(pr);
    res->status = 400;
    snprintf(res->err, sizeof res->err, "invalid fetch request");
    return;
  }
  if (strcmp(PQgetvalue(pr, 0, 0), "t") != 0 || !strcmp(PQgetvalue(pr, 0, 1), "INVALID")) {
    PQclear(pr);
    res->status = 400;
    snprintf(res->err, sizeof res->err, "urls (1..5 public http(s) URLs) is required");
    return;
  }
  char *urls_json = strdup(PQgetvalue(pr, 0, 1));
  int req_chars = atoi(PQgetvalue(pr, 0, 2));
  char *hl_query = strdup(PQgetvalue(pr, 0, 3));
  PQclear(pr);
  if (!urls_json || !hl_query) {
    free(urls_json);
    free(hl_query);
    res->status = 503;
    snprintf(res->err, sizeof res->err, "memory");
    return;
  }
  if (strlen(hl_query) > 2000) {
    free(urls_json);
    free(hl_query);
    res->status = 400;
    snprintf(res->err, sizeof res->err, "query exceeds 2000 bytes");
    return;
  }

  // Validate URL count and SSRF safety for every URL via exa-policy.coax.
  const char *uv[1] = {urls_json};
  PGresult *ur = PQexecParams(
      db,
      "SELECT elem #>> '{}', jsonb_typeof(elem) FROM jsonb_array_elements($1::jsonb) AS t(elem)",
      1, NULL, uv, NULL, NULL, 0);
  if (PQresultStatus(ur) != PGRES_TUPLES_OK) {
    if (ur) PQclear(ur);
    free(urls_json);
    free(hl_query);
    res->status = 400;
    snprintf(res->err, sizeof res->err, "urls must be an array of URL strings");
    return;
  }
  int n_urls = PQntuples(ur);
  if (!exa_valid_url_count(n_urls)) {
    PQclear(ur);
    free(urls_json);
    free(hl_query);
    res->status = 400;
    snprintf(res->err, sizeof res->err, "fetch accepts 1 to 5 URLs");
    return;
  }
  for (int i = 0; i < n_urls; i++) {
    if (strcmp(PQgetvalue(ur, i, 1), "string") != 0 || !exa_url_safe(exa_cs(PQgetvalue(ur, i, 0)))) {
      PQclear(ur);
      free(urls_json);
      free(hl_query);
      res->status = 400;
      snprintf(res->err, sizeof res->err, "blocked or invalid URL (public http(s) URLs only)");
      return;
    }
  }
  PQclear(ur);

  int max_chars = (int)exa_clamp_max_chars(req_chars);

  if (!exa_has_key()) {
    free(urls_json);
    free(hl_query);
    res->status = 503;
    snprintf(res->err, sizeof res->err, "web fetch is not configured");
    return;
  }

  if (!exa_admit_call(db, q, who, free_key, "exa/contents", res)) {
    free(urls_json);
    free(hl_query);
    return;
  }

  char chars_s[16];
  snprintf(chars_s, sizeof chars_s, "%d", max_chars);
  const char *bv[3] = {urls_json, chars_s, hl_query};
  PGresult *br = PQexecParams(
      db,
      "SELECT CASE WHEN $3::text <> '' "
      "  THEN jsonb_build_object('urls', $1::jsonb, 'highlights', jsonb_build_object('query', $3::text, 'maxCharacters', $2::int)) "
      "  ELSE jsonb_build_object('urls', $1::jsonb, 'text', jsonb_build_object('maxCharacters', $2::int)) "
      "END::text",
      3, NULL, bv, NULL, NULL, 0);
  free(urls_json);
  free(hl_query);
  if (PQresultStatus(br) != PGRES_TUPLES_OK || PQntuples(br) != 1) {
    if (br) PQclear(br);
    res->status = 500;
    snprintf(res->err, sizeof res->err, "failed to encode fetch payload");
    return;
  }
  char *payload = strdup(PQgetvalue(br, 0, 0));
  PQclear(br);
  if (!payload) {
    res->status = 503;
    snprintf(res->err, sizeof res->err, "memory");
    return;
  }

  double t0 = api_now_ms();
  char *raw_resp = NULL;
  long http_status = exa_post_upstream("/contents", payload, false, 30L, &raw_resp);
  int taken_ms = (int)(api_now_ms() - t0);
  free(payload);

  long long upstream_micros = (http_status == 200) ? exa_extract_cost_micros(db, raw_resp) : 0;
  char *billing_json = exa_finish_call(db, who, "exa/contents", http_status, upstream_micros, taken_ms, free_key);
  if (!billing_json) {
    free(raw_resp);
    res->status = 503;
    snprintf(res->err, sizeof res->err, "memory");
    return;
  }

  if (http_status != 200 || !raw_resp) {
    api_log("exa: /contents upstream answered %ld in %d ms", http_status, taken_ms);
    free(raw_resp);
    free(billing_json);
    res->status = (http_status == 400 || http_status == 429) ? (int)http_status : 502;
    snprintf(res->err, sizeof res->err,
             http_status == 429 ? "fetch provider rate limited; try again soon"
                                : "fetch provider did not answer");
    return;
  }

  const char *nv[2] = {raw_resp, billing_json};
  PGresult *nr = PQexecParams(
      db,
      "WITH src AS ("
      "  SELECT CASE WHEN pg_input_is_valid($1, 'jsonb') THEN $1::jsonb ELSE NULL END AS j, $2::jsonb AS b"
      "), items AS ("
      "  SELECT elem, ord "
      "  FROM src, LATERAL jsonb_array_elements(coalesce(src.j->'results', '[]'::jsonb)) WITH ORDINALITY AS t(elem, ord)"
      "), norm AS ("
      "  SELECT ord, jsonb_strip_nulls(jsonb_build_object("
      "    'title', coalesce(elem->>'title', ''),"
      "    'url', coalesce(elem->>'url', ''),"
      "    'publishedDate', elem->>'publishedDate',"
      "    'author', elem->>'author',"
      "    'highlights', elem->'highlights',"
      "    'text', elem->>'text'"
      "  )) AS obj,"
      "  concat_ws(E'\\n',"
      "    '# ' || coalesce(nullif(elem->>'title', ''), '(no title)'),"
      "    'URL: ' || coalesce(elem->>'url', ''),"
      "    CASE WHEN coalesce(elem->>'publishedDate', '') <> '' THEN 'Published: ' || split_part(elem->>'publishedDate', 'T', 1) ELSE NULL END,"
      "    CASE WHEN coalesce(elem->>'author', '') <> '' THEN 'Author: ' || (elem->>'author') ELSE NULL END,"
      "    '',"
      "    coalesce("
      "      (SELECT string_agg(h, E'\\n\\n') FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(elem->'highlights') = 'array' THEN elem->'highlights' ELSE '[]'::jsonb END) h),"
      "      elem->>'text',"
      "      ''"
      "    )"
      "  ) AS md "
      "  FROM items"
      ")"
      "SELECT "
      "  (SELECT j IS NOT NULL FROM src) AS ok_json,"
      "  jsonb_build_object("
      "    'results', coalesce((SELECT jsonb_agg(obj ORDER BY ord) FROM norm), '[]'::jsonb),"
      "    'statuses', coalesce((SELECT j->'statuses' FROM src), '[]'::jsonb),"
      "    'searchTime', (SELECT j->'searchTime' FROM src),"
      "    'billing', (SELECT b FROM src)"
      "  )::text AS out_json,"
      "  coalesce((SELECT string_agg(md, E'\\n\\n---\\n\\n' ORDER BY ord) FROM norm), 'No content found for the provided URL(s).') AS out_md,"
      "  jsonb_build_object('searchTime', (SELECT j->'searchTime' FROM src), 'billing', (SELECT b FROM src))::text AS out_meta",
      2, NULL, nv, NULL, NULL, 0);
  free(raw_resp);
  free(billing_json);

  if (PQresultStatus(nr) != PGRES_TUPLES_OK || PQntuples(nr) != 1 || strcmp(PQgetvalue(nr, 0, 0), "t") != 0) {
    if (nr) PQclear(nr);
    res->status = 502;
    snprintf(res->err, sizeof res->err, "fetch provider returned invalid JSON");
    return;
  }

  res->status = 200;
  res->json = strdup(PQgetvalue(nr, 0, 1));
  res->markdown = strdup(PQgetvalue(nr, 0, 2));
  res->meta_json = strdup(PQgetvalue(nr, 0, 3));
  PQclear(nr);
  if (!res->json || !res->markdown || !res->meta_json) {
    exa_exec_res_free(res);
    res->status = 503;
    snprintf(res->err, sizeof res->err, "memory");
  }
}

static void exa_reply_exec(const request *q, const exa_exec_res *res) {
  if (res->credit_refused_sent) return;
  if (res->status == 200 && res->json) {
    api_json(q->fd, 200, res->json);
  } else if (res->status == 402 && res->json) {
    api_json(q->fd, 402, res->json);
  } else {
    api_error(q->fd, res->status ? res->status : 500, res->err[0] ? res->err : "search error");
  }
}

// Helper to emit a JSON-RPC 2.0 success or error response built via Postgres JSONB.
static void exa_mcp_send_error(PGconn *db, int fd, int http_status, const char *id_json,
                               int rpc_code, const char *msg, const char *data_json) {
  char code_s[24];
  snprintf(code_s, sizeof code_s, "%d", rpc_code);
  const char *v[4] = {
      (id_json && *id_json) ? id_json : "null",
      code_s,
      msg ? msg : "error",
      (data_json && *data_json) ? data_json : "null",
  };
  PGresult *r = PQexecParams(
      db,
      "SELECT jsonb_build_object("
      "  'jsonrpc', '2.0',"
      "  'id', $1::jsonb,"
      "  'error', jsonb_strip_nulls(jsonb_build_object('code', $2::int, 'message', $3::text, 'data', CASE WHEN $4::jsonb = 'null'::jsonb THEN NULL ELSE $4::jsonb END))"
      ")::text",
      4, NULL, v, NULL, NULL, 0);
  if (r && PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    api_json(fd, http_status, PQgetvalue(r, 0, 0));
  } else {
    api_error(fd, http_status, msg ? msg : "mcp error");
  }
  if (r) PQclear(r);
}

static void exa_mcp_send_result(PGconn *db, int fd, const char *id_json, const char *result_json) {
  const char *v[2] = {(id_json && *id_json) ? id_json : "null", result_json};
  PGresult *r = PQexecParams(
      db,
      "SELECT jsonb_build_object('jsonrpc', '2.0', 'id', $1::jsonb, 'result', $2::jsonb)::text",
      2, NULL, v, NULL, NULL, 0);
  if (r && PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    api_json(fd, 200, PQgetvalue(r, 0, 0));
  } else {
    api_error(fd, 500, "failed to encode MCP response");
  }
  if (r) PQclear(r);
}

static const char EXA_MCP_TOOLS_LIST[] =
    "{"
    "  \"tools\": ["
    "    {"
    "      \"name\": \"web_search_exa\","
    "      \"title\": \"Web Search\","
    "      \"description\": \"Search the web using OpenAgents metered Exa search and get clean, ready-to-use highlights. Billed against your OpenAgents credit balance.\","
    "      \"inputSchema\": {"
    "        \"type\": \"object\","
    "        \"properties\": {"
    "          \"query\": {\"type\": \"string\", \"description\": \"Natural language search query\"},"
    "          \"numResults\": {\"type\": \"integer\", \"description\": \"Number of results (1-10, default 8)\"},"
    "          \"allowed_domains\": {\"type\": \"array\", \"items\": {\"type\": \"string\"}, \"description\": \"Only include results from these domains\"},"
    "          \"blocked_domains\": {\"type\": \"array\", \"items\": {\"type\": \"string\"}, \"description\": \"Exclude results from these domains\"},"
    "          \"category\": {\"type\": \"string\", \"description\": \"Optional category filter (company, research paper, news, personal site, financial report, people, tweet, github, pdf)\"}"
    "        },"
    "        \"required\": [\"query\"]"
    "      },"
    "      \"annotations\": {\"readOnlyHint\": true, \"destructiveHint\": false, \"openWorldHint\": true, \"idempotentHint\": true}"
    "    },"
    "    {"
    "      \"name\": \"web_fetch_exa\","
    "      \"title\": \"Fetch Web Pages\","
    "      \"description\": \"Read webpage content as clean text via OpenAgents metered Exa contents API (1-5 public URLs). Billed against your OpenAgents credit balance.\","
    "      \"inputSchema\": {"
    "        \"type\": \"object\","
    "        \"properties\": {"
    "          \"urls\": {\"type\": \"array\", \"items\": {\"type\": \"string\"}, \"description\": \"Public http(s) URLs to fetch (1-5)\"},"
    "          \"maxCharacters\": {\"type\": \"integer\", \"description\": \"Max characters per page (200-24000, default 3000)\"}"
    "        },"
    "        \"required\": [\"urls\"]"
    "      },"
    "      \"annotations\": {\"readOnlyHint\": true, \"destructiveHint\": false, \"openWorldHint\": true, \"idempotentHint\": true}"
    "    }"
    "  ]"
    "}";

static void exa_mcp_handle(const request *q, PGconn *db, const char *who, bool free_key, const char *body) {
  const char *pv[1] = {body};
  PGresult *pr = PQexecParams(
      db,
      "WITH raw AS ("
      "  SELECT CASE WHEN pg_input_is_valid($1, 'jsonb') THEN $1::jsonb ELSE NULL END AS j"
      ")"
      "SELECT "
      "  (j IS NOT NULL AND jsonb_typeof(j) = 'object') AS ok,"
      "  coalesce(j->>'jsonrpc', '') AS ver,"
      "  coalesce((j->'id')::text, 'null') AS id_json,"
      "  (j ? 'id') AS has_id,"
      "  coalesce(j->>'method', '') AS method,"
      "  coalesce(j->'params'->>'name', '') AS tool_name,"
      "  coalesce((j->'params'->'arguments')::text, '{}') AS tool_args "
      "FROM raw",
      1, NULL, pv, NULL, NULL, 0);
  if (PQresultStatus(pr) != PGRES_TUPLES_OK || PQntuples(pr) != 1) {
    if (pr) PQclear(pr);
    exa_mcp_send_error(db, q->fd, 400, "null", -32700, "Parse error", NULL);
    return;
  }
  bool ok = !strcmp(PQgetvalue(pr, 0, 0), "t");
  char ver[16]; snprintf(ver, sizeof ver, "%s", PQgetvalue(pr, 0, 1));
  char id_json[128]; snprintf(id_json, sizeof id_json, "%s", PQgetvalue(pr, 0, 2));
  bool has_id = !strcmp(PQgetvalue(pr, 0, 3), "t");
  char method[128]; snprintf(method, sizeof method, "%s", PQgetvalue(pr, 0, 4));
  char tool_name[64]; snprintf(tool_name, sizeof tool_name, "%s", PQgetvalue(pr, 0, 5));
  char *tool_args = strdup(PQgetvalue(pr, 0, 6));
  PQclear(pr);

  if (!ok || strcmp(ver, "2.0") != 0 || !method[0]) {
    free(tool_args);
    exa_mcp_send_error(db, q->fd, 400, id_json, -32600, "Invalid JSON-RPC 2.0 request", NULL);
    return;
  }

  // Notifications (no id) require no JSON-RPC response body.
  if (!strncmp(method, "notifications/", 14) && !has_id) {
    free(tool_args);
    api_respond(q->fd, 204, "application/json", NULL, "", 0);
    return;
  }

  if (!strcmp(method, "initialize")) {
    free(tool_args);
    exa_mcp_send_result(
        db, q->fd, id_json,
        "{\"protocolVersion\":\"2025-03-26\",\"capabilities\":{\"tools\":{}},"
        "\"serverInfo\":{\"name\":\"openagents-exa\",\"version\":\"1.0.0\"}}");
    return;
  }
  if (!strcmp(method, "ping")) {
    free(tool_args);
    exa_mcp_send_result(db, q->fd, id_json, "{}");
    return;
  }
  if (!strcmp(method, "tools/list")) {
    free(tool_args);
    exa_mcp_send_result(db, q->fd, id_json, EXA_MCP_TOOLS_LIST);
    return;
  }
  if (strcmp(method, "tools/call") != 0) {
    free(tool_args);
    exa_mcp_send_error(db, q->fd, 200, id_json, -32601, "Method not found", NULL);
    return;
  }

  exa_exec_res res = {0};
  if (!strcmp(tool_name, "web_search_exa")) {
    exa_execute_search(db, q, who, free_key, tool_args ? tool_args : "{}", &res);
  } else if (!strcmp(tool_name, "web_fetch_exa")) {
    exa_execute_fetch(db, q, who, free_key, tool_args ? tool_args : "{}", &res);
  } else {
    free(tool_args);
    exa_mcp_send_error(db, q->fd, 200, id_json, -32602, "Unknown tool", NULL);
    return;
  }
  free(tool_args);

  if (res.credit_refused_sent) {
    exa_exec_res_free(&res);
    return;
  }
  if (res.status == 402) {
    exa_mcp_send_error(db, q->fd, 402, id_json, -32002,
                       res.err[0] ? res.err : "add credit: POST /api/v1/inference/credit",
                       res.json);
    exa_exec_res_free(&res);
    return;
  }
  if (res.status == 429) {
    exa_mcp_send_error(db, q->fd, 429, id_json, -32029,
                       res.err[0] ? res.err : "daily limit reached", NULL);
    exa_exec_res_free(&res);
    return;
  }
  if (res.status == 400) {
    exa_mcp_send_error(db, q->fd, 200, id_json, -32602,
                       res.err[0] ? res.err : "invalid arguments", NULL);
    exa_exec_res_free(&res);
    return;
  }
  if (res.status != 200) {
    // Upstream / service error returned as MCP tool error result so MCP clients surface it cleanly.
    const char *ev[1] = {res.err[0] ? res.err : "search provider error"};
    PGresult *er = PQexecParams(
        db,
        "SELECT jsonb_build_object("
        "  'content', jsonb_build_array(jsonb_build_object('type', 'text', 'text', $1::text)),"
        "  'isError', true"
        ")::text",
        1, NULL, ev, NULL, NULL, 0);
    if (er && PQresultStatus(er) == PGRES_TUPLES_OK && PQntuples(er) == 1) {
      exa_mcp_send_result(db, q->fd, id_json, PQgetvalue(er, 0, 0));
    } else {
      exa_mcp_send_error(db, q->fd, 502, id_json, -32000, ev[0], NULL);
    }
    if (er) PQclear(er);
    exa_exec_res_free(&res);
    return;
  }

  // Success: return MCP CallToolResult with clean markdown text and _meta { searchTime, billing }.
  const char *rv[2] = {res.markdown ? res.markdown : "", res.meta_json ? res.meta_json : "{}"};
  PGresult *rr = PQexecParams(
      db,
      "SELECT jsonb_build_object("
      "  'content', jsonb_build_array(jsonb_build_object('type', 'text', 'text', $1::text)),"
      "  'isError', false,"
      "  '_meta', $2::jsonb"
      ")::text",
      2, NULL, rv, NULL, NULL, 0);
  if (rr && PQresultStatus(rr) == PGRES_TUPLES_OK && PQntuples(rr) == 1) {
    exa_mcp_send_result(db, q->fd, id_json, PQgetvalue(rr, 0, 0));
  } else {
    exa_mcp_send_error(db, q->fd, 500, id_json, -32603, "failed to format MCP tool result", NULL);
  }
  if (rr) PQclear(rr);
  exa_exec_res_free(&res);
}

bool exa_route(const request *q) {
  int kind = 0;
  if (!strcmp(q->path, "/api/v1/search")) kind = 1;
  else if (!strcmp(q->path, "/api/v1/fetch")) kind = 2;
  else if (!strcmp(q->path, "/api/v1/mcp/exa")) kind = 3;
  else return false;

  if (strcmp(q->method, "POST") != 0) {
    api_error(q->fd, 405, kind == 1 ? "POST a search request"
                        : kind == 2 ? "POST a fetch request"
                                    : "POST a JSON-RPC 2.0 MCP request");
    return true;
  }
  if (q->body_len > EXA_MAX_REQ_BYTES) {
    api_error(q->fd, 413, "request too large");
    return true;
  }

  char who[65];
  bool free_key = false;
  if (!inference_auth_caller(q, who, &free_key)) return true;

  PGconn *db = api_db();
  if (!db) {
    api_error(q->fd, 503, "no database");
    return true;
  }

  char *body = malloc(q->body_len + 1);
  if (!body) {
    api_error(q->fd, 503, "memory");
    return true;
  }
  memcpy(body, q->body, q->body_len);
  body[q->body_len] = 0;

  if (kind == 1) {
    exa_exec_res res = {0};
    exa_execute_search(db, q, who, free_key, body, &res);
    exa_reply_exec(q, &res);
    exa_exec_res_free(&res);
  } else if (kind == 2) {
    exa_exec_res res = {0};
    exa_execute_fetch(db, q, who, free_key, body, &res);
    exa_reply_exec(q, &res);
    exa_exec_res_free(&res);
  } else {
    exa_mcp_handle(q, db, who, free_key, body);
  }

  free(body);
  return true;
}
