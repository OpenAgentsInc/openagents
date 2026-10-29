// Admin routes for openagents.com/admin/analytics (pt-dashboard). Never
// WorkOS: an admin is a Nostr public key in OA_ADMIN_PUBKEYS (admins.conf).
// Until the phone sign-in exists, the admin signs a NIP-98 request on their
// own machine (sign.c, admin-login.sh) and this file checks it (nostr.c,
// BIP-340 via libsecp256k1 for now):
//
//   GET /api/admin/login?auth=<base64 event>   the event signs GET <origin>/api/admin/login;
//                                              good once; sets the oa_admin cookie
//                                              (HttpOnly, Secure, SameSite=Strict, /api/admin)
//                                              and sends the browser to /admin/analytics
//                                              (?next=chat: to /chat; the cookie opens /api/chat too)
//   GET /api/admin/logout                      ends that session
//   GET /api/admin/analytics?view=overview|sessions|session&days=N&space=web|kiki|dev|all&auto=1&sid=S
//                                              the reports, for admin.coax; a cookie or an
//                                              Authorization: Nostr header for this exact URL
//   GET /api/admin/reports, POST .../<kind>/<ref>/hide|keep|restore
//                                              Hecate's Cemetery's review queue (/admin/reports)
//
// Anyone else gets 401 or 403 and nothing more, not even counts. The reports
// are rows for admin.coax to draw: a section starts "#TITLE|FLAGS|COLUMN|...",
// then one "value|value|..." line a row ('|' and line breaks inside a value
// become spaces). FLAGS "s": a row's first value is a session that opens;
// "w": the section wants the full width.
// Every time is Chicago's.
#include "api.h"
#include "nostr.h"
#include <ctype.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#define ADMINS_MAX 32
static char admins[ADMINS_MAX][65];
static int admin_count;
#define SESSION_HOURS 12

void admin_init(void) {
  const char *list = getenv("OA_ADMIN_PUBKEYS");
  admin_count = 0;
  for (const char *p = list ? list : ""; *p && admin_count < ADMINS_MAX;) {
    while (*p && !isxdigit((unsigned char)*p)) p++;
    size_t n = 0;
    while (isxdigit((unsigned char)p[n])) n++;
    if (n == 64) { for (size_t i = 0; i < 64; i++) admins[admin_count][i] = (char)tolower((unsigned char)p[i]); admins[admin_count++][64] = 0; }
    p += n;
  }
  api_log("admins: %d", admin_count);
}
static bool is_admin(const char *pubkey) {
  for (int i = 0; i < admin_count; i++) if (!strcmp(admins[i], pubkey)) return true;
  return false;
}
bool admin_is(const char *pubkey) { return is_admin(pubkey); }
static const char *origin(void) { const char *o = getenv("OA_API_ORIGIN"); return o && *o ? o : "https://openagents.com"; }

// A signed event used once only: false when it was seen before.
static bool first_use(PGconn *db, const nostr_event *ev) {
  char ms[32]; snprintf(ms, sizeof ms, "%lld", (long long)api_now_ms());
  const char *v[3] = {ev->id, ev->pubkey, ms};
  PGresult *r = PQexecParams(db, "INSERT INTO analytics.admin_events (id, pubkey, ms) VALUES ($1, $2, $3::bigint) ON CONFLICT DO NOTHING",
                             3, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK && !strcmp(PQcmdTuples(r), "1");
  PQclear(r);
  return ok;
}

// The admin making this request (their pubkey into who), or why not: 0 an
// admin; 401 nobody signed in; 403 signed, but not an admin.
static int who_is(const request *q, PGconn *db, char who[65]) {
  char auth[4096], cookies[4096];
  who[0] = 0;
  if (api_header(q, "Authorization", auth, sizeof auth) && !strncmp(auth, "Nostr ", 6)) {
    char url[4200]; snprintf(url, sizeof url, "%s%s", origin(), q->target);
    nostr_event ev; char err[160];
    if (!nip98_check(auth, q->method, url, q->body, q->body_len, (long long)time(NULL), &ev, err, sizeof err)) return 401;
    if (!is_admin(ev.pubkey)) return 403;
    if (!first_use(db, &ev)) return 401;
    memcpy(who, ev.pubkey, 65);
    return 0;
  }
  if (!api_header(q, "Cookie", cookies, sizeof cookies)) return 401;
  const char *c = strstr(cookies, "oa_admin=");
  if (!c || (c != cookies && c[-1] != ' ' && c[-1] != ';')) return 401;
  c += 9;
  char token[65]; size_t n = 0;
  while (n < 64 && isxdigit((unsigned char)c[n])) { token[n] = c[n]; n++; }
  token[n] = 0;
  if (n != 64) return 401;
  char ms[32]; snprintf(ms, sizeof ms, "%lld", (long long)api_now_ms());
  const char *v[2] = {token, ms};
  PGresult *r = PQexecParams(db, "SELECT pubkey FROM analytics.admin_sessions WHERE token_sha256 = encode(sha256(convert_to($1, 'UTF8')), 'hex') "
                                 "AND expires_ms > $2::bigint", 2, NULL, v, NULL, NULL, 0);
  int status = 401;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    snprintf(who, 65, "%s", PQgetvalue(r, 0, 0));
    status = is_admin(who) ? 0 : 403;   // taken off the list: the session is no good
  }
  PQclear(r);
  return status;
}

// The same check for other routes (chat.c): 0 an admin (pubkey in who), else 401 or 403.
int admin_who(const request *q, PGconn *db, char who[65]) { return who_is(q, db, who); }

static void refuse(int fd, int status) {
  api_json(fd, status, status == 403 ? "{\"error\":\"admins only\"}" : "{\"error\":\"log in as an admin\"}");
}

static void login(const request *q, PGconn *db) {
  char b64[4096], auth[4200], url[1200];
  if (!api_query(q, "auth", b64, sizeof b64) || !b64[0]) { refuse(q->fd, 401); return; }
  snprintf(auth, sizeof auth, "Nostr %s", b64);
  snprintf(url, sizeof url, "%s/api/admin/login", origin());
  nostr_event ev; char err[160];
  if (!nip98_check(auth, "GET", url, NULL, 0, (long long)time(NULL), &ev, err, sizeof err)) {
    api_log("admin login refused: %s", err);
    refuse(q->fd, 401); return;
  }
  if (!is_admin(ev.pubkey)) { api_log("admin login: not an admin"); refuse(q->fd, 403); return; }
  if (!first_use(db, &ev)) { refuse(q->fd, 401); return; }
  unsigned char raw[32]; char token[65];
  int fd = open("/dev/urandom", O_RDONLY);
  if (fd < 0 || read(fd, raw, 32) != 32) { if (fd >= 0) close(fd); api_json(q->fd, 503, "{\"error\":\"no randomness\"}"); return; }
  close(fd);
  for (int i = 0; i < 32; i++) snprintf(token + 2 * i, 3, "%02x", raw[i]);
  char now[32], until[32];
  long long t = (long long)api_now_ms();
  snprintf(now, sizeof now, "%lld", t); snprintf(until, sizeof until, "%lld", t + SESSION_HOURS * 3600000LL);
  const char *v[4] = {token, ev.pubkey, now, until};
  PGresult *r = PQexecParams(db, "INSERT INTO analytics.admin_sessions (token_sha256, pubkey, created_ms, expires_ms) "
                                 "VALUES (encode(sha256(convert_to($1, 'UTF8')), 'hex'), $2, $3::bigint, $4::bigint)", 4, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK;
  PQclear(r);
  if (!ok) { api_json(q->fd, 503, "{\"error\":\"no session\"}"); return; }
  char headers[900], next[16];
  // ?next=chat sends the browser to /chat instead (the link is signed without it, as before).
  api_query(q, "next", next, sizeof next);
  // The same session opens /api/drops (drops.c), so an admin can drop files for a new project,
  // and /api/chat (chat.c), the admins' chat with a Claude agent.
  snprintf(headers, sizeof headers, "Location: %s\r\nSet-Cookie: oa_admin=%s; Path=/api/admin; Max-Age=%d; HttpOnly; Secure; SameSite=Strict\r\n"
           "Set-Cookie: oa_admin=%s; Path=/api/drops; Max-Age=%d; HttpOnly; Secure; SameSite=Strict\r\n"
           "Set-Cookie: oa_admin=%s; Path=/api/chat; Max-Age=%d; HttpOnly; Secure; SameSite=Strict\r\n",
           !strcmp(next, "chat") ? "/chat" : "/admin/analytics",
           token, SESSION_HOURS * 3600, token, SESSION_HOURS * 3600, token, SESSION_HOURS * 3600);
  api_log("admin signed in: %.12s", ev.pubkey);
  api_respond(q->fd, 303, "text/plain", headers, "", 0);
}

static void logout(const request *q, PGconn *db) {
  char cookies[4096];
  if (api_header(q, "Cookie", cookies, sizeof cookies)) {
    const char *c = strstr(cookies, "oa_admin=");
    if (c) {
      char token[65]; size_t n = 0; c += 9;
      while (n < 64 && isxdigit((unsigned char)c[n])) { token[n] = c[n]; n++; }
      token[n] = 0;
      const char *v[1] = {token};
      PQclear(PQexecParams(db, "DELETE FROM analytics.admin_sessions WHERE token_sha256 = encode(sha256(convert_to($1, 'UTF8')), 'hex')",
                           1, NULL, v, NULL, NULL, 0));
    }
  }
  api_respond(q->fd, 303, "text/plain", "Location: /\r\nSet-Cookie: oa_admin=; Path=/api/admin; Max-Age=0; HttpOnly; Secure; SameSite=Strict\r\n"
              "Set-Cookie: oa_admin=; Path=/api/drops; Max-Age=0; HttpOnly; Secure; SameSite=Strict\r\n"
              "Set-Cookie: oa_admin=; Path=/api/chat; Max-Age=0; HttpOnly; Secure; SameSite=Strict\r\n", "", 0);
}

// ---- the reports

typedef struct { char *p; size_t n, cap; bool full; } out_buf;
#define OUT_MAX (900 * 1024)   // admin.coax's inbox holds a megabyte

static void put(out_buf *o, const char *s, size_t n) {
  if (o->full) return;
  if (o->n + n + 1 > OUT_MAX) { o->full = true; return; }
  if (o->n + n + 1 > o->cap) {
    size_t cap = o->cap ? o->cap : 65536;
    while (cap < o->n + n + 1) cap *= 2;
    char *p = realloc(o->p, cap);
    if (!p) { o->full = true; return; }
    o->p = p; o->cap = cap;
  }
  memcpy(o->p + o->n, s, n); o->n += n; o->p[o->n] = 0;
}
static void puts_(out_buf *o, const char *s) { put(o, s, strlen(s)); }
// A value: '|' and line breaks would split it, so they become spaces.
static void value(out_buf *o, const char *s) {
  char buf[512]; size_t n = 0;
  for (; *s; s++) {
    buf[n++] = *s == '|' || *s == '\n' || *s == '\r' || *s == '\t' ? ' ' : *s;
    if (n == sizeof buf) { put(o, buf, n); n = 0; }
  }
  put(o, buf, n);
}

// One section: its title and columns, then the query's rows.
static void section(out_buf *o, PGconn *db, const char *title, const char *flags, const char *columns, const char *sql,
                    int nparams, const char *const *params) {
  PGresult *r = PQexecParams(db, sql, nparams, NULL, params, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) {
    api_log("report %s: %s", title, PQerrorMessage(db));
    PQclear(r);
    return;
  }
  puts_(o, "#"); puts_(o, title); puts_(o, "|"); puts_(o, flags); puts_(o, "|"); puts_(o, columns); puts_(o, "\n");
  int rows = PQntuples(r), cols = PQnfields(r);
  for (int i = 0; i < rows && !o->full; i++) {
    for (int c = 0; c < cols; c++) { if (c) puts_(o, "|"); value(o, PQgetvalue(r, i, c)); }
    puts_(o, "\n");
  }
  PQclear(r);
}

// The sessions in the filter: s every session with events since $1 in space
// $2 (or all), k those kept ($3: automated ones too), e their events.
#define KEPT \
  "WITH s AS (SELECT sid, min(ms) AS first_ms, max(ms) AS last_ms, min(space) AS space, count(*) AS n, " \
  "  bool_or(auto IS NOT NULL OR (ev = 'session-start' AND p ? 'agent')) AS automated " \
  "  FROM analytics.events WHERE ms >= $1::bigint AND ($2 = 'all' OR space = $2) GROUP BY sid), " \
  "k AS (SELECT * FROM s WHERE $3::boolean OR NOT automated), " \
  "e AS (SELECT x.* FROM analytics.events x JOIN k USING (sid) WHERE x.ms >= $1::bigint) "
// The playtesters in the filter (after KEPT): one row a join, so a person
// who came back counts once among the people and again among the visits.
// An automated one (its own mark, or its session's) counts only with $3.
#define PLAYTESTERS \
  ", pt AS (SELECT t.email FROM analytics.playtesters t " \
  "  WHERE (CASE WHEN t.at < 1e11 THEN t.at * 1000 ELSE t.at END) >= $1::bigint AND ($2 = 'all' OR t.space = $2) " \
  "  AND ($3::boolean OR (t.auto IS NULL AND NOT EXISTS (SELECT 1 FROM s WHERE s.sid = t.sid AND s.automated)))) "
#define CHI(ms) "to_char(to_timestamp((" ms ") / 1000.0) AT TIME ZONE 'America/Chicago', 'MM-DD HH24:MI')"
// The flow's steps, as the playtest's funnel counts them.
#define STEP_OF \
  "CASE WHEN ev = 'load-engine' THEN 2 WHEN ev = 'world-enter' THEN 3 WHEN ev IN ('edit', 'edit-open') THEN 4 " \
  "WHEN ev = 'place' THEN 5 WHEN ev IN ('publish', 'published') THEN 6 WHEN ev = 'share' THEN 7 WHEN ev = 'invite' THEN 8 " \
  "WHEN ev IN ('survey-sent', 'survey-answer', 'survey-submit') THEN 9 ELSE 1 END"
#define STEP_NAMES "ARRAY['visited', 'loaded', 'in a world', 'editing', 'placed', 'published', 'shared', 'invited', 'answered']"

static void overview(out_buf *o, PGconn *db, const char *const *f) {
  section(o, db, "HEALTH", "", "SURFACE|LAST EVENT|LAST HOUR|LAST DAY|CONTRACT EVENTS MISSING",
    "SELECT surface, coalesce(" CHI("last_ms") " || ' ' || last_ev, 'never'), hour::text, day::text, "
    "  CASE WHEN cardinality(missing) = 0 THEN 'none' ELSE 'MISSING ' || array_to_string(missing, ', ') END "
    "FROM analytics.health(floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint) "
    "ORDER BY cardinality(missing) = 0, surface", 0, NULL);
  section(o, db, "SUMMARY", "", "WHAT|COUNT", KEPT PLAYTESTERS
    "SELECT 'Sessions', count(*)::text FROM k "
    "UNION ALL SELECT 'Visitors', count(DISTINCT p->>'pid')::text FROM e WHERE ev = 'session-start' "
    "UNION ALL SELECT 'Events', count(*)::text FROM e "
    "UNION ALL SELECT 'Median session', coalesce(round((percentile_cont(0.5) WITHIN GROUP (ORDER BY last_ms - first_ms) / 60000.0)::numeric, 1)::text || ' min', '-') FROM k "
    "UNION ALL SELECT 'Entered a world', count(DISTINCT sid)::text FROM e WHERE ev = 'world-enter' "
    "UNION ALL SELECT 'Said or typed something', count(DISTINCT sid)::text FROM e WHERE p ? 'text' "
    "UNION ALL SELECT 'Errors', count(*)::text FROM e WHERE ev = 'error' "
    "UNION ALL SELECT 'Playtesters', count(DISTINCT lower(email))::text FROM pt "
    "UNION ALL SELECT 'Playtester visits', count(*)::text FROM pt "
    "UNION ALL SELECT 'Automated, ' || CASE WHEN $3::boolean THEN 'counted' ELSE 'left out' END, count(*)::text FROM s WHERE automated", 3, f);
  section(o, db, "FUNNEL", "", "STEP|SESSIONS|OF ALL", KEPT
    ", steps(i, label) AS (SELECT i, (" STEP_NAMES ")[i] FROM generate_series(1, 9) i), "
    "reach AS (SELECT sid, max(" STEP_OF ") AS far FROM e GROUP BY sid) "
    "SELECT initcap(label), count(r.sid)::text, CASE WHEN (SELECT count(*) FROM reach) > 0 "
    "  THEN round(100.0 * count(r.sid) / (SELECT count(*) FROM reach))::text || '%' ELSE '-' END "
    "FROM steps LEFT JOIN reach r ON r.far >= steps.i GROUP BY i, label ORDER BY i", 3, f);
  section(o, db, "WHERE THEY STOPPED", "", "LAST THING DONE|SESSIONS", KEPT
    "SELECT ev, count(*)::text FROM (SELECT DISTINCT ON (sid) sid, ev FROM e "
    "  WHERE ev NOT IN ('alive', 'hidden', 'visible', 'resize', 'world-leave', 'page-leave', 'session-end', 'error') "
    "  ORDER BY sid, coalesce(ct, ms) DESC, id DESC) l GROUP BY ev ORDER BY count(*) DESC LIMIT 15", 3, f);
  section(o, db, "BY DAY", "", "DAY|SESSIONS|VISITORS|EVENTS", KEPT
    "SELECT to_char(to_timestamp(k.first_ms / 1000.0) AT TIME ZONE 'America/Chicago', 'YYYY-MM-DD') AS day, count(*)::text, "
    "  count(DISTINCT st.p->>'pid')::text, sum(k.n)::text "
    "FROM k LEFT JOIN LATERAL (SELECT p FROM e WHERE e.sid = k.sid AND e.ev = 'session-start' LIMIT 1) st ON true "
    "GROUP BY day ORDER BY day DESC LIMIT 60", 3, f);
  section(o, db, "WHERE THEY CAME FROM", "", "FROM|SESSIONS", KEPT
    "SELECT coalesce(nullif(p->>'ref', ''), 'direct'), count(*)::text FROM e WHERE ev = 'session-start' "
    "GROUP BY 1 ORDER BY count(*) DESC LIMIT 15", 3, f);
  section(o, db, "PAGES", "", "PAGE|VIEWS", KEPT
    "SELECT coalesce(nullif(p->>'path', ''), page), count(*)::text FROM e WHERE ev IN ('page-view', 'landing') "
    "GROUP BY 1 ORDER BY count(*) DESC LIMIT 15", 3, f);
  section(o, db, "PLACES", "", "COUNTRY|PLACE|SESSIONS", KEPT
    "SELECT coalesce(x.region, '?'), coalesce(initcap(x.city), '') || coalesce(', ' || x.subdivision, ''), count(*)::text "
    "FROM k JOIN analytics.sessions x USING (sid) GROUP BY 1, 2 ORDER BY count(*) DESC LIMIT 15", 3, f);
  section(o, db, "DEVICES", "", "SYSTEM|BROWSER|PHONE|SESSIONS", KEPT
    "SELECT coalesce(p->>'os', '?'), coalesce(regexp_replace(p->>'browser', ' [0-9.]+$', ''), '?'), "
    "  CASE WHEN p->>'mobile' = 'true' THEN 'phone' ELSE '' END, count(*)::text "
    "FROM e WHERE ev = 'session-start' GROUP BY 1, 2, 3 ORDER BY count(*) DESC LIMIT 15", 3, f);
  section(o, db, "ERRORS", "", "MESSAGE|WHERE|TIMES|LAST", KEPT
    "SELECT coalesce(p->>'msg', ''), coalesce(p->>'src', '') || coalesce(':' || (p->>'line'), ''), count(*)::text, " CHI("max(ms)") " "
    "FROM e WHERE ev = 'error' GROUP BY 1, 2 ORDER BY max(ms) DESC LIMIT 20", 3, f);
  section(o, db, "WHAT THEY SAID AND TYPED", "s", "SESSION|WHEN|WHERE|WORDS", KEPT
    "SELECT sid, " CHI("ms") ", ev, p->>'text' FROM e WHERE p ? 'text' ORDER BY ms DESC LIMIT 100", 3, f);
  section(o, db, "SURVEY ANSWERS", "s", "SESSION|WHEN|QUESTION|ANSWER",
    "SELECT coalesce(sid, ''), " CHI("CASE WHEN at < 1e11 THEN at * 1000 ELSE at END") ", q, coalesce(a #>> '{}', '') || CASE WHEN skip THEN ' (skipped)' ELSE '' END "
    "FROM analytics.survey_answers WHERE (CASE WHEN at < 1e11 THEN at * 1000 ELSE at END) >= $1::bigint AND ($2 = 'all' OR space = $2) "
    "ORDER BY at DESC LIMIT 100", 2, f);
}

static void sessions(out_buf *o, PGconn *db, const char *const *f) {
  section(o, db, "SESSIONS", "s", "SESSION|STARTED|SPACE|FIRST PAGE|EVENTS|MINUTES|FURTHEST|PLACE|DEVICE|AUTOMATED", KEPT
    "SELECT k.sid, " CHI("k.first_ms") ", k.space, coalesce(nullif(st.p->>'entry', ''), fp.page, ''), k.n::text, "
    "  round((k.last_ms - k.first_ms) / 60000.0, 1)::text, (" STEP_NAMES ")[far.far], "
    "  coalesce(x.region, '') || coalesce(' ' || initcap(x.city), ''), "
    "  coalesce(st.p->>'os', '') || coalesce(' ' || regexp_replace(st.p->>'browser', ' [0-9.]+$', ''), '') || CASE WHEN st.p->>'mobile' = 'true' THEN ' phone' ELSE '' END, "
    "  CASE WHEN k.automated THEN coalesce(st.p->>'agent', 'yes') ELSE '' END "
    "FROM k LEFT JOIN analytics.sessions x USING (sid) "
    "LEFT JOIN LATERAL (SELECT p FROM e WHERE e.sid = k.sid AND e.ev = 'session-start' LIMIT 1) st ON true "
    "LEFT JOIN LATERAL (SELECT page FROM e WHERE e.sid = k.sid ORDER BY coalesce(ct, ms), id LIMIT 1) fp ON true "
    "LEFT JOIN LATERAL (SELECT max(" STEP_OF ") AS far FROM e WHERE e.sid = k.sid) far ON true "
    "ORDER BY k.first_ms DESC LIMIT 300", 3, f);
}

static void one_session(out_buf *o, PGconn *db, const char *sid) {
  const char *v[1] = {sid};
  section(o, db, "SESSION", "", "WHAT|VALUE",
    "WITH x AS (SELECT * FROM analytics.sessions WHERE sid = $1), "
    "ev AS (SELECT min(ms) AS a, max(ms) AS b, count(*) AS n, min(space) AS space, min(host) AS host FROM analytics.events WHERE sid = $1) "
    "SELECT 'Session', $1 UNION ALL SELECT 'Started', " CHI("a") " FROM ev UNION ALL SELECT 'Minutes', round((b - a) / 60000.0, 1)::text FROM ev "
    "UNION ALL SELECT 'Events', n::text FROM ev UNION ALL SELECT 'Space', space FROM ev UNION ALL SELECT 'Host', coalesce(host, '') FROM ev "
    "UNION ALL SELECT 'Place', coalesce(region, '') || coalesce(' ' || subdivision, '') || coalesce(' ' || initcap(city), '') || coalesce(' (' || latlong || ')', '') FROM x "
    "UNION ALL SELECT 'Address', coalesce(ip, '') FROM x UNION ALL SELECT 'Browser', coalesce(ua, '') FROM x "
    "UNION ALL SELECT 'Robot', coalesce(agent, '') FROM x UNION ALL SELECT 'Round trip', coalesce(rtt_ms::text || ' ms', '') FROM x "
    "UNION ALL SELECT 'Nostr key', coalesce(npub, '') FROM x", 1, v);
  section(o, db, "TIMELINE", "w", "AT|PAGE|EVENT|DETAILS",
    "WITH t AS (SELECT *, min(coalesce(ct, ms)) OVER () AS t0 FROM analytics.events WHERE sid = $1) "
    "SELECT to_char(((coalesce(ct, ms) - t0) / 1000.0) * interval '1 second', 'HH24:MI:SS'), page, ev, "
    "  coalesce((SELECT string_agg(key || ' ' || CASE WHEN jsonb_typeof(value) = 'string' THEN value #>> '{}' ELSE value::text END, '   ' ORDER BY key) "
    "    FROM jsonb_each(p) WHERE key <> 'consent'), '') || CASE WHEN auto IS NOT NULL THEN '   (automated: ' || auto || ')' ELSE '' END "
    "FROM t ORDER BY coalesce(ct, ms), id LIMIT 3000", 1, v);
}

static void reports(const request *q, PGconn *db) {
  char view[24], days[12], space[12], autos[4], sid[96];
  api_query(q, "view", view, sizeof view);
  api_query(q, "days", days, sizeof days);
  api_query(q, "space", space, sizeof space);
  api_query(q, "auto", autos, sizeof autos);
  api_query(q, "sid", sid, sizeof sid);
  long d = days[0] ? strtol(days, NULL, 10) : 7;
  if (d < 0 || d > 3650) d = 7;
  char from[32];
  snprintf(from, sizeof from, "%lld", d == 0 ? 0LL : (long long)api_now_ms() - d * 86400000LL);
  if (strcmp(space, "web") && strcmp(space, "kiki") && strcmp(space, "dev")) snprintf(space, sizeof space, "all");
  const char *f[3] = {from, space, !strcmp(autos, "1") ? "true" : "false"};
  out_buf o = {0};
  if (!strcmp(view, "session")) {
    for (const char *c = sid; *c; c++)
      if (!isalnum((unsigned char)*c) && *c != '_' && *c != ':' && *c != '-') { api_json(q->fd, 400, "{\"error\":\"bad session\"}"); return; }
    if (!sid[0]) { api_json(q->fd, 400, "{\"error\":\"which session?\"}"); return; }
    one_session(&o, db, sid);
  } else if (!strcmp(view, "sessions")) sessions(&o, db, f);
  else overview(&o, db, f);
  api_respond(q->fd, 200, "text/plain; charset=utf-8", NULL, o.p ? o.p : "", o.n);
  free(o.p);
}

// Hecate's Cemetery's review queue (/admin/reports, admin.coax): the reports as rows
// (migrations/0033_villa_review.sql, villa.reports_rows), and an admin's decision on
// one, through villa.reports_api, the same rules /api/villa/reports keeps:
//   GET  /api/admin/reports                                 the rows
//   POST /api/admin/reports/<kind>/<ref>/hide|keep|restore  {"decided": ...}
static void review(const request *q, PGconn *db, const char *who) {
  const char *rest = q->path + 18;
  if (!*rest || !strcmp(rest, "/")) {
    if (strcmp(q->method, "GET")) { api_json(q->fd, 405, "{\"error\":\"GET\"}"); return; }
    const char *v[1] = {who};
    PGresult *r = PQexecParams(db, "SELECT villa.reports_rows($1)", 1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0)) {
      const char *t = PQgetvalue(r, 0, 0);
      api_respond(q->fd, 200, "text/plain; charset=utf-8", "Cache-Control: no-store\r\n", t, strlen(t));
    } else { api_log("review: %s", PQerrorMessage(db)); api_json(q->fd, 500, "{\"error\":\"not now\"}"); }
    PQclear(r);
    return;
  }
  if (strcmp(q->method, "POST")) { api_json(q->fd, 405, "{\"error\":\"POST\"}"); return; }
  char from[256];   // a decision comes from this site's page
  if (api_header(q, "Origin", from, sizeof from) && strcmp(from, origin())) { api_json(q->fd, 403, "{\"error\":\"cross-site\"}"); return; }
  // "/<kind>/<ref>/<act>" as a text[] of [A-Za-z0-9] segments, behind "reports"
  char parts[200] = "{reports"; size_t n = strlen(parts); int segs = 0;
  for (const char *p = rest; *p; p++) {
    if (*p == '/') { if (!p[1]) break; if (n + 2 >= sizeof parts) break; parts[n++] = ','; segs++; continue; }
    if (!isalnum((unsigned char)*p) || n + 2 >= sizeof parts) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return; }
    parts[n++] = *p;
  }
  parts[n++] = '}'; parts[n] = 0;
  if (segs != 3) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return; }
  const char *v[2] = {parts, who};
  PGresult *r = PQexecParams(db, "SELECT (x->>'status')::int, (x->'body')::text FROM villa.reports_api('POST', $1::text[], '{}', NULL, $2) x",
                             2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) api_json(q->fd, atoi(PQgetvalue(r, 0, 0)), PQgetvalue(r, 0, 1));
  else { api_log("review: %s", PQerrorMessage(db)); api_json(q->fd, 500, "{\"error\":\"not kept\"}"); }
  PQclear(r);
}

bool admin_route(const request *q) {
  if (strncmp(q->path, "/api/admin/", 11)) return false;
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  if (!strcmp(q->path, "/api/admin/login")) { login(q, db); return true; }
  if (!strcmp(q->path, "/api/admin/logout")) { logout(q, db); return true; }
  bool queue = !strncmp(q->path, "/api/admin/reports", 18) && (q->path[18] == 0 || q->path[18] == '/');
  if (strcmp(q->path, "/api/admin/analytics") && !queue) { api_json(q->fd, 404, "{\"error\":\"not found\"}"); return true; }
  char who[65];
  int status = who_is(q, db, who);
  if (status) { refuse(q->fd, status); return true; }
  if (queue) review(q, db, who);
  else reports(q, db);
  return true;
}
