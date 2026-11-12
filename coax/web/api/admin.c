// Admin routes for openagents.com/admin/analytics (pt-dashboard). An admin
// is a Nostr public key in OA_ADMIN_PUBKEYS (admins.conf), logged in with a
// session of ours (auth.c: /login, or the signed link below) whose account
// holds that key. From a machine, the admin signs a NIP-98 request (sign.c,
// admin-login.sh) and this file checks it (nostr.c, BIP-340 via libsecp256k1):
//
//   GET /api/admin/login?auth=<base64 event>   the event signs GET <origin>/api/admin/login;
//                                              good once; makes a web session at L2 for the key's
//                                              account (auth_login_key_redirect, the cookie
//                                              __Host-oa_session) and sends the browser to
//                                              /admin/analytics (?next=chat: to /chat);
//                                              and oa_team=1 (Path=/, two years): the team's browser
//   GET /api/admin/logout                      ends that session
//   GET /api/admin/analytics?view=overview|sessions|everglade|session&days=N&space=web|kiki|dev|all&auto=1&team=1&sid=S
//                                              the reports, for admin.coax (our team's own testing,
//                                              auto = 'team', left out unless team=1); a cookie or an
//                                              Authorization: Nostr header for this exact URL
//   GET /api/admin/team-link?host=everglade.world   marks this browser as our team on that host too
//                                              (its cookies are its own, so the oa_team cookie
//                                              set here doesn't reach it): a one-use link, good for
//                                              ten minutes, to that host's /api/analytics/team
//                                              (analytics.c), which sets oa_team there
//   GET /api/admin/rooms                       the rooms' vitals (/admin/rooms): each shard's numbers
//                                              over the last 10 minutes and its seats (#12791)
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
// A root admin: a key in admins.conf (OA_ADMIN_PUBKEYS), changed only by a deploy.
bool admin_is_root(const char *pubkey) {
  for (int i = 0; i < admin_count; i++) if (!strcmp(admins[i], pubkey)) return true;
  return false;
}
// The roots as a Postgres text array literal ({hex,hex}), for the SQL that decides who may change
// the admin list (migrations/0100_admin_keys.sql).
void admin_roots(char *out, size_t cap) {
  size_t n = (size_t)snprintf(out, cap, "{");
  for (int i = 0; i < admin_count && n + 67 < cap; i++) n += (size_t)snprintf(out + n, cap - n, "%s%s", i ? "," : "", admins[i]);
  snprintf(out + n, cap - n, "}");
}
// A site admin: a root, or a key a root added to access.admin_keys (0100). Without the table (the
// migration not applied) only the roots.
static bool is_admin(const char *pubkey) {
  if (admin_is_root(pubkey)) return true;
  size_t n = 0;
  while (n < 65 && isxdigit((unsigned char)pubkey[n])) n++;
  if (n != 64 || pubkey[64]) return false;
  PGconn *db = api_db();
  if (!db) return false;
  const char *v[1] = {pubkey};
  PGresult *r = PQexecParams(db, "SELECT access.is_db_admin($1)", 1, NULL, v, NULL, NULL, 0);
  bool yes = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t");
  PQclear(r);
  return yes;
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
  char auth[4096];
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
  // A session of our own (auth.c): its account's key, if an admin's.
  const auth_principal *p = auth_principal_of(q);
  if (p && p->session[0] && p->pubkey[0]) {
    memcpy(who, p->pubkey, 65);
    return is_admin(who) ? 0 : 403;
  }
  return 401;
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
  // ?next=chat sends the browser to /chat instead (the link is signed without it, as before).
  // The session is ours (auth.c), so it opens /api/drops, /api/chat and every realm's EDIT too.
  char next[16] = "";
  api_query(q, "next", next, sizeof next);
  if (auth_login_key_redirect(q, db, ev.pubkey, !strcmp(next, "chat") ? "/chat" : "/admin/analytics")) api_log("admin logged in: %.12s", ev.pubkey);
}

static void logout(const request *q, PGconn *db) {
  const auth_principal *p = auth_principal_of(q);   // our own session (auth.c)
  if (p && p->session[0]) { const char *v[1] = {p->session}; PQclear(PQexecParams(db, "SELECT auth.session_end($1)", 1, NULL, v, NULL, NULL, 0)); }
  api_respond(q->fd, 303, "text/plain", "Location: /\r\nSet-Cookie: __Host-oa_session=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n", "", 0);
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
// $2 (or all), k those kept ($3: automated ones too; $4: our team's own
// testing too, auto = 'team', migrations/0037_analytics_team.sql), e their events.
#define KEPT \
  "WITH s AS (SELECT sid, min(ms) AS first_ms, max(ms) AS last_ms, min(space) AS space, count(*) AS n, " \
  "  bool_or((auto IS NOT NULL AND auto <> 'team') OR (ev = 'session-start' AND p ? 'agent')) AS automated, " \
  "  coalesce(bool_or(auto = 'team'), false) AS team " \
  "  FROM analytics.events WHERE ms >= $1::bigint AND ($2 = 'all' OR space = $2) GROUP BY sid), " \
  "k AS (SELECT * FROM s WHERE ($3::boolean OR NOT automated) AND ($4::boolean OR NOT team)), " \
  "e AS (SELECT x.* FROM analytics.events x JOIN k USING (sid) WHERE x.ms >= $1::bigint) "
// The playtesters in the filter (after KEPT): one row a join, so a person
// who came back counts once among the people and again among the visits.
// An automated one (its own mark, or its session's) counts only with $3, the team's only with $4.
#define PLAYTESTERS \
  ", pt AS (SELECT t.email FROM analytics.playtesters t " \
  "  WHERE (CASE WHEN t.at < 1e11 THEN t.at * 1000 ELSE t.at END) >= $1::bigint AND ($2 = 'all' OR t.space = $2) " \
  "  AND ($3::boolean OR ((t.auto IS NULL OR t.auto = 'team') AND NOT EXISTS (SELECT 1 FROM s WHERE s.sid = t.sid AND s.automated)))" \
  "  AND ($4::boolean OR (t.auto IS DISTINCT FROM 'team' AND NOT EXISTS (SELECT 1 FROM s WHERE s.sid = t.sid AND s.team)))) "
#define CHI(ms) "to_char(to_timestamp((" ms ") / 1000.0) AT TIME ZONE 'America/Chicago', 'MM-DD HH24:MI')"
// The flow's steps, as the playtest's funnel counts them.
#define STEP_OF \
  "CASE WHEN ev = 'load-engine' THEN 2 WHEN ev = 'world-enter' THEN 3 WHEN ev IN ('edit', 'edit-open') THEN 4 " \
  "WHEN ev = 'place' THEN 5 WHEN ev IN ('publish', 'published') THEN 6 WHEN ev = 'share' THEN 7 WHEN ev = 'invite' THEN 8 " \
  "WHEN ev IN ('survey-sent', 'survey-answer', 'survey-submit') THEN 9 ELSE 1 END"
#define STEP_NAMES "ARRAY['visited', 'loaded', 'in a world', 'editing', 'placed', 'published', 'shared', 'invited', 'answered']"

// The events every visit sends on its own; the rest are what people did.
#define REALM_NOISE \
  "'session-start', 'page-view', 'load-engine', 'load-landing', 'world-enter', 'world-leave', 'page-leave', 'alive', " \
  "'hidden', 'visible', 'resize', 'click', 'key', 'error', 'fail', 'tally', 'app-launch', 'app-background', 'app-foreground'"
#define REALM_ACT "(ev NOT IN (" REALM_NOISE ") AND ev NOT LIKE 'load-%')"
#define REALM_PAGES \
  "'haunt', 'sandbox', 'the-grid', 'everglade', 'dead-gears', 'ice-cream-man', 'fairy-forest', 'hecates-cemetery', " \
  "'realm-made', 'realm', 'realm-builder', 'galaxy', 'landing', 'home', 'app', 'everglade2'"
static const struct { const char *page, *title; } realm_cards[] = {
  {"home", "HOMEPAGE"}, {"galaxy", "GALAXY"}, {"haunt", "HAUNTED HOUSE"}, {"sandbox", "SANDBOX"}, {"the-grid", "THE GRID"},
  {"everglade", "EVERGLADE"}, {"dead-gears", "DEAD GEARS"}, {"ice-cream-man", "ICE CREAM MAN"}, {"fairy-forest", "FAIRY FOREST"},
  {"hecates-cemetery", "HECATE'S CEMETERY: WHAT THEY DID"}, {"realm-builder", "REALM BUILDER"}, {"realm-made", "MADE REALMS"},
  {"everglade2", "EVERGLADE.WORLD"}, {"app", "IPHONE APP"}, {NULL, NULL}};

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
    "UNION ALL SELECT 'In the iPhone app', count(*)::text FROM k JOIN analytics.sessions x USING (sid) WHERE x.ua LIKE '%OpenAgentsApp/%' OR k.sid LIKE 'a%' "
    "UNION ALL SELECT 'Playtesters', count(DISTINCT lower(email))::text FROM pt "
    "UNION ALL SELECT 'Playtester visits', count(*)::text FROM pt "
    "UNION ALL SELECT 'Automated, ' || CASE WHEN $3::boolean THEN 'counted' ELSE 'left out' END, count(*)::text FROM s WHERE automated "
    "UNION ALL SELECT 'Our team, ' || CASE WHEN $4::boolean THEN 'counted' ELSE 'left out' END, count(*)::text FROM s WHERE team AND NOT automated", 4, f);
  section(o, db, "FUNNEL", "", "STEP|SESSIONS|OF ALL", KEPT
    ", steps(i, label) AS (SELECT i, (" STEP_NAMES ")[i] FROM generate_series(1, 9) i), "
    "reach AS (SELECT sid, max(" STEP_OF ") AS far FROM e GROUP BY sid) "
    "SELECT initcap(label), count(r.sid)::text, CASE WHEN (SELECT count(*) FROM reach) > 0 "
    "  THEN round(100.0 * count(r.sid) / (SELECT count(*) FROM reach))::text || '%' ELSE '-' END "
    "FROM steps LEFT JOIN reach r ON r.far >= steps.i GROUP BY i, label ORDER BY i", 4, f);
  // Planetary Villa's first visit, step by step (villapet.coax EV_NAMES, villa-face.c): the galaxy
  // choice is a click on PLANETARY VILLA, then the realm's own events. Same filter as the rest.
  section(o, db, "HECATE'S CEMETERY", "", "STEP|SESSIONS", KEPT
    ", steps(i, label, evs) AS (VALUES (1, 'Chose it on the galaxy', ARRAY['click']), (2, 'Opened the realm', ARRAY['world-enter']), "
    "  (3, 'Flight', ARRAY['villa-flight']), (4, 'Tips seen', ARRAY['villa-tip']), (5, 'Pet made', ARRAY['villa-pet-made']), "
    "  (6, 'Avatar', ARRAY['villa-avatar']), (7, 'Tombstone', ARRAY['villa-tombstone']), (8, 'Emote', ARRAY['villa-emote']), "
    "  (9, 'Fetch', ARRAY['villa-fetch']), (10, 'Sit, stay or come', ARRAY['villa-obey']), (11, 'Dug', ARRAY['villa-dig']), "
    "  (12, 'Picked a flower', ARRAY['villa-flower']), (13, 'Vase', ARRAY['villa-vase']), (14, 'Memorial', ARRAY['villa-memorial']), "
    "  (15, 'Shared a memorial', ARRAY['villa-share']), (16, 'Chat', ARRAY['villa-chat'])) "
    "SELECT label, count(DISTINCT e.sid)::text FROM steps LEFT JOIN e ON e.ev = ANY (evs) AND "
    "  CASE WHEN i = 1 THEN e.page = 'galaxy' AND e.p->>'label' = 'villa' ELSE e.page = 'hecates-cemetery' END "
    "GROUP BY i, label ORDER BY i", 4, f);
  // Every realm and the app, one card each (migrations/0069_analytics_realm_actions.sql,
  // docs/verse/analytics-events.md): how far visits got, then what people did most.
  // A 'tally' event's properties are counts; every other action counts once.
  // Same filter as the rest: the team and automation are left out.
  section(o, db, "REALMS", "w", "REALM|VISITS|LOADED|IN THE WORLD|DID SOMETHING|MEDIAN MINUTES|IN THE APP", KEPT
    ", v AS (SELECT page, sid, bool_or(ev IN ('load-engine', 'load-landing')) AS loaded, bool_or(ev = 'world-enter') AS entered, "
    "  bool_or(" REALM_ACT ") AS acted, (max(coalesce(ct, ms)) - min(coalesce(ct, ms))) / 60000.0 AS mins "
    "  FROM e WHERE page IN (" REALM_PAGES ") GROUP BY page, sid) "
    "SELECT v.page, count(*)::text, count(*) FILTER (WHERE loaded)::text, count(*) FILTER (WHERE entered)::text, "
    "  count(*) FILTER (WHERE acted)::text, round((percentile_cont(0.5) WITHIN GROUP (ORDER BY mins))::numeric, 1)::text, "
    "  count(*) FILTER (WHERE x.ua LIKE '%OpenAgentsApp/%')::text "
    "FROM v LEFT JOIN analytics.sessions x USING (sid) GROUP BY v.page ORDER BY count(*) DESC", 4, f);
  for (int i = 0; realm_cards[i].page; i++) {
    const char *g[5] = {f[0], f[1], f[2], f[3], realm_cards[i].page};
    section(o, db, realm_cards[i].title, "", "WHAT|SESSIONS|TIMES", KEPT
      ", pe AS (SELECT * FROM e WHERE page = $5), "
      "steps(i, label, n) AS (VALUES "
      "  (1, 'Visited', (SELECT count(DISTINCT sid) FROM pe)), "
      "  (2, 'Loaded', (SELECT count(DISTINCT sid) FROM pe WHERE ev IN ('load-engine', 'load-landing', 'app-launch'))), "
      "  (3, 'In the world', (SELECT count(DISTINCT sid) FROM pe WHERE ev = 'world-enter')), "
      "  (4, 'Did something', (SELECT count(DISTINCT sid) FROM pe WHERE " REALM_ACT ")), "
      "  (5, 'Walked 50 m', (SELECT count(DISTINCT sid) FROM pe WHERE ev = 'walk' AND (p->>'m')::int >= 50)), "
      "  (6, 'Stayed 5 minutes', (SELECT count(*) FROM (SELECT sid FROM pe GROUP BY sid HAVING max(coalesce(ct, ms)) - min(coalesce(ct, ms)) >= 300000) l)), "
      "  (7, 'Failed to load or errored', (SELECT count(DISTINCT sid) FROM pe WHERE ev IN ('error', 'fail')))), "
      "acts AS (SELECT sid, ev || coalesce(' ' || coalesce(p->>'id', p->>'area', p->>'label', p->>'m'), '') AS what, 1::bigint AS n "
      "  FROM pe WHERE " REALM_ACT " "
      "  UNION ALL SELECT pe.sid, j.key, (j.value #>> '{}')::numeric::bigint FROM pe, jsonb_each(pe.p) j "
      "  WHERE pe.ev = 'tally' AND jsonb_typeof(j.value) = 'number'), "
      "top AS (SELECT what, count(DISTINCT sid) AS s, sum(n) AS t FROM acts GROUP BY what ORDER BY sum(n) DESC, what LIMIT 15) "
      "SELECT label, n::text, '' FROM steps "
      "UNION ALL SELECT * FROM (SELECT what, s::text, t::text FROM top ORDER BY t DESC, what) tt", 5, g);
  }
  section(o, db, "WHERE THEY STOPPED", "", "LAST THING DONE|SESSIONS", KEPT
    "SELECT ev, count(*)::text FROM (SELECT DISTINCT ON (sid) sid, ev FROM e "
    "  WHERE ev NOT IN ('alive', 'hidden', 'visible', 'resize', 'world-leave', 'page-leave', 'session-end', 'error') "
    "  ORDER BY sid, coalesce(ct, ms) DESC, id DESC) l GROUP BY ev ORDER BY count(*) DESC LIMIT 15", 4, f);
  section(o, db, "BY DAY", "", "DAY|SESSIONS|VISITORS|EVENTS", KEPT
    "SELECT to_char(to_timestamp(k.first_ms / 1000.0) AT TIME ZONE 'America/Chicago', 'YYYY-MM-DD') AS day, count(*)::text, "
    "  count(DISTINCT st.p->>'pid')::text, sum(k.n)::text "
    "FROM k LEFT JOIN LATERAL (SELECT p FROM e WHERE e.sid = k.sid AND e.ev = 'session-start' LIMIT 1) st ON true "
    "GROUP BY day ORDER BY day DESC LIMIT 60", 4, f);
  section(o, db, "WHERE THEY CAME FROM", "", "FROM|SESSIONS", KEPT
    "SELECT coalesce(nullif(p->>'ref', ''), 'direct'), count(*)::text FROM e WHERE ev = 'session-start' "
    "GROUP BY 1 ORDER BY count(*) DESC LIMIT 15", 4, f);
  // A realm's old addresses (/realm/<slug> before October 29, /realms/<slug> before November 2) count
  // with its new one (/worlds/<slug>).
  section(o, db, "PAGES", "", "PAGE|VIEWS", KEPT
    "SELECT regexp_replace(coalesce(nullif(p->>'path', ''), page), '^/realms?(/|$)', '/worlds\\1'), count(*)::text FROM e WHERE ev IN ('page-view', 'landing') "
    "GROUP BY 1 ORDER BY count(*) DESC LIMIT 15", 4, f);
  section(o, db, "PLACES", "", "COUNTRY|PLACE|SESSIONS", KEPT
    "SELECT coalesce(x.region, '?'), coalesce(initcap(x.city), '') || coalesce(', ' || x.subdivision, ''), count(*)::text "
    "FROM k JOIN analytics.sessions x USING (sid) GROUP BY 1, 2 ORDER BY count(*) DESC LIMIT 15", 4, f);
  section(o, db, "DEVICES", "", "SYSTEM|BROWSER|PHONE|SESSIONS", KEPT
    "SELECT coalesce(p->>'os', '?'), coalesce(regexp_replace(p->>'browser', ' [0-9.]+$', ''), '?'), "
    "  CASE WHEN p->>'mobile' = 'true' THEN 'phone' ELSE '' END, count(*)::text "
    "FROM e WHERE ev = 'session-start' GROUP BY 1, 2, 3 ORDER BY count(*) DESC LIMIT 15", 4, f);
  section(o, db, "ERRORS", "", "MESSAGE|WHERE|TIMES|LAST", KEPT
    "SELECT coalesce(p->>'msg', ''), coalesce(p->>'src', '') || coalesce(':' || (p->>'line'), ''), count(*)::text, " CHI("max(ms)") " "
    "FROM e WHERE ev = 'error' GROUP BY 1, 2 ORDER BY max(ms) DESC LIMIT 20", 4, f);
  section(o, db, "WHAT THEY SAID AND TYPED", "s", "SESSION|WHEN|WHERE|WORDS", KEPT
    "SELECT sid, " CHI("ms") ", ev, p->>'text' FROM e WHERE p ? 'text' ORDER BY ms DESC LIMIT 100", 4, f);
  section(o, db, "SURVEY ANSWERS", "s", "SESSION|WHEN|QUESTION|ANSWER",
    "SELECT coalesce(sid, ''), " CHI("CASE WHEN at < 1e11 THEN at * 1000 ELSE at END") ", q, coalesce(a #>> '{}', '') || CASE WHEN skip THEN ' (skipped)' ELSE '' END "
    "FROM analytics.survey_answers WHERE (CASE WHEN at < 1e11 THEN at * 1000 ELSE at END) >= $1::bigint AND ($2 = 'all' OR space = $2) "
    "AND ($4::boolean OR auto IS DISTINCT FROM 'team') AND $3::boolean IS NOT NULL "
    "ORDER BY at DESC LIMIT 100", 4, f);
}

// Everglade (everglade.world, Everglade II and Character Create: page 'everglade2', its events in
// everglade/client/track.coax and coax/web/host.js lifeBeacon;
// docs/analytics/2026-11-09-everglade-analytics-audit.md): how far each visit got, step by step, where
// they came in, each day, what went wrong and what they did, and the sessions. Same filter as the rest:
// the team and automation are left out unless asked for.
#define EG_E ", ee AS (SELECT * FROM e WHERE page = 'everglade2') "
// The furthest step an event says a visit reached (the steps' names below, in order).
#define EG_STEP \
  "CASE WHEN ev = 'load-engine' THEN 2 WHEN ev = 'load-world' THEN 3 WHEN ev = 'portal-shown' THEN 4 WHEN ev = 'create-open' THEN 5 " \
  "WHEN ev IN ('create-pick', 'create-class', 'create-level') THEN 6 WHEN ev = 'create-name' THEN 7 WHEN ev = 'create-done' THEN 8 " \
  "WHEN ev = 'run-end' THEN 9 WHEN ev = 'spawn' THEN 10 WHEN ev = 'room-join' THEN 11 " \
  "WHEN ev = 'tally' AND EXISTS (SELECT 1 FROM jsonb_object_keys(p) j WHERE left(j, 5) = 'kill_') THEN 13 " \
  "WHEN ev = 'tally' AND EXISTS (SELECT 1 FROM jsonb_object_keys(p) j WHERE left(j, 5) IN ('cast_', 'hurt_') OR left(j, 4) = 'hit_' OR j = 'bow') THEN 12 " \
  "WHEN ev IN ('death', 'level-up') THEN 12 ELSE 1 END"
#define EG_STEPS "ARRAY['Opened the game', 'The game loaded', 'The world showed', 'Past the front door or the portal', " \
  "'Character Create showed', 'Changed their look or class', 'Named their character', 'Pressed USE IN EVERGLADE', " \
  "'Ran out of the glade', 'In Everglade', 'Joined the other players', 'Fought', 'Felled a wolf, goblin or the guard']"
// Where a visit came in: its first page view (the front door's own, realms.ts; /play?via=portal from
// openagents.com is the lander's portal, newland-portal.coax).
#define EG_WAY \
  ", way AS (SELECT DISTINCT ON (sid) sid, CASE " \
  "  WHEN p->>'path' IN ('/', '/index.html') OR p->>'path' LIKE '%/everglade2/domain/' OR p->>'path' LIKE '%/everglade2/domain/index.html' THEN 'everglade.world front door' " \
  "  WHEN p->>'path' IN ('/play', '/play/') OR p->>'path' LIKE '%/domain/play%' THEN " \
  "    CASE WHEN coalesce(p->>'q', '') NOT LIKE '%via=portal%' THEN 'everglade.world/play' " \
  "      WHEN p->>'from' = 'openagents.com' THEN 'openagents.com/newlander portal' ELSE 'everglade.world front door' END " \
  "  WHEN p->>'path' LIKE '%character-creator%' THEN " \
  "    CASE WHEN coalesce(p->>'q', '') LIKE '%via=portal%' THEN 'openagents.com homepage portal' ELSE 'openagents.com Character Create' END " \
  "  WHEN p->>'path' LIKE '%everglade2%' THEN 'openagents.com/worlds/everglade2' ELSE 'other' END AS way, host " \
  "  FROM ee WHERE ev = 'page-view' ORDER BY sid, coalesce(ct, ms), id) "
#define EG_PROBLEM "ev IN ('load-fail', 'gpu-lost', 'crash', 'error', 'fail')"

static void everglade(out_buf *o, PGconn *db, const char *const *f) {
  section(o, db, "EVERGLADE: HOW FAR VISITS GOT", "w", "STEP|GOT THIS FAR|OF ALL WHO OPENED IT|STOPPED HERE", KEPT EG_E
    ", reach AS (SELECT sid, max(" EG_STEP ") AS far FROM ee GROUP BY sid), "
    "steps(i, label) AS (SELECT i, (" EG_STEPS ")[i] FROM generate_series(1, 13) i) "
    "SELECT label, count(r.sid)::text, CASE WHEN (SELECT count(*) FROM reach) > 0 "
    "  THEN round(100.0 * count(r.sid) / (SELECT count(*) FROM reach))::text || '%' ELSE '-' END, "
    "  (SELECT count(*) FROM reach x WHERE x.far = steps.i)::text "
    "FROM steps LEFT JOIN reach r ON r.far >= steps.i GROUP BY i, label ORDER BY i", 4, f);
  section(o, db, "EVERGLADE: WAYS IN", "", "WAY IN|VISITS|GOT INTO EVERGLADE|FAILED TO LOAD", KEPT EG_E EG_WAY
    "SELECT way.way, count(*)::text, "
    "  count(*) FILTER (WHERE EXISTS (SELECT 1 FROM ee WHERE ee.sid = way.sid AND ee.ev = 'spawn'))::text, "
    "  count(*) FILTER (WHERE EXISTS (SELECT 1 FROM ee WHERE ee.sid = way.sid AND ee.ev = 'load-fail'))::text "
    "FROM way GROUP BY way.way ORDER BY count(*) DESC", 4, f);
  section(o, db, "EVERGLADE: BY DAY", "w",
    "DAY|VISITS|PEOPLE|CAME BACK|LOADED|MADE A CHARACTER|IN EVERGLADE|FOUGHT|FAILED TO LOAD|MEDIAN MINUTES", KEPT EG_E
    ", per AS (SELECT sid, min(coalesce(ct, ms)) AS a, max(coalesce(ct, ms)) AS b, max(" EG_STEP ") AS far, "
    "  bool_or(ev = 'load-fail') AS failed, bool_or(ev = 'create-done') AS made, "
    "  min(p->>'pid') FILTER (WHERE ev IN ('session-start', 'page-view') AND p->>'pid' <> '') AS pid FROM ee GROUP BY sid), "
    "d AS (SELECT *, to_char(to_timestamp(a / 1000.0) AT TIME ZONE 'America/Chicago', 'YYYY-MM-DD') AS day FROM per), "
    "first AS (SELECT pid, min(day) AS day FROM d WHERE pid IS NOT NULL GROUP BY pid) "
    "SELECT d.day, count(*)::text, count(DISTINCT d.pid)::text, "
    "  count(DISTINCT d.pid) FILTER (WHERE first.day < d.day)::text, "
    "  count(*) FILTER (WHERE far >= 2)::text, count(*) FILTER (WHERE made)::text, count(*) FILTER (WHERE far >= 10)::text, "
    "  count(*) FILTER (WHERE far >= 12)::text, count(*) FILTER (WHERE failed)::text, "
    "  round((percentile_cont(0.5) WITHIN GROUP (ORDER BY (b - a) / 60000.0))::numeric, 1)::text "
    "FROM d LEFT JOIN first USING (pid) GROUP BY d.day ORDER BY d.day DESC LIMIT 60", 4, f);
  section(o, db, "EVERGLADE: LOADING", "", "WHAT|VALUE", KEPT EG_E
    ", v AS (SELECT count(DISTINCT sid) AS n FROM ee), "
    "le AS (SELECT (p->>'ms')::numeric AS ms FROM ee WHERE ev = 'load-engine' AND p ? 'ms'), "
    "lw AS (SELECT (p->>'ms')::numeric AS ms FROM ee WHERE ev = 'load-world' AND p ? 'ms') "
    "SELECT 'Visits', n::text FROM v "
    "UNION ALL SELECT 'Game loaded, median seconds', coalesce(round((percentile_cont(0.5) WITHIN GROUP (ORDER BY ms) / 1000)::numeric, 1)::text, '-') FROM le "
    "UNION ALL SELECT 'Game loaded, slowest tenth from (seconds)', coalesce(round((percentile_cont(0.9) WITHIN GROUP (ORDER BY ms) / 1000)::numeric, 1)::text, '-') FROM le "
    "UNION ALL SELECT 'World showed, median seconds', coalesce(round((percentile_cont(0.5) WITHIN GROUP (ORDER BY ms) / 1000)::numeric, 1)::text, '-') FROM lw "
    "UNION ALL SELECT 'World showed, slowest tenth from (seconds)', coalesce(round((percentile_cont(0.9) WITHIN GROUP (ORDER BY ms) / 1000)::numeric, 1)::text, '-') FROM lw "
    "UNION ALL SELECT 'Browser without WebGPU', count(DISTINCT sid)::text FROM ee WHERE (ev = 'session-start' AND p->>'gpu' = 'false') OR (ev = 'load-fail' AND p->>'webgpu' = 'false') "
    "UNION ALL SELECT 'Failed to load', count(DISTINCT sid)::text FROM ee WHERE ev = 'load-fail' "
    "UNION ALL SELECT 'Left before the world showed', count(*)::text FROM (SELECT sid FROM ee GROUP BY sid "
    "  HAVING NOT bool_or(ev IN ('load-world', 'load-fail'))) l "
    "UNION ALL SELECT 'Lost the GPU', count(DISTINCT sid)::text FROM ee WHERE ev = 'gpu-lost' "
    "UNION ALL SELECT 'The game stopped', count(DISTINCT sid)::text FROM ee WHERE ev = 'crash' "
    "UNION ALL SELECT 'Page errors', count(DISTINCT sid)::text FROM ee WHERE ev = 'error' "
    "UNION ALL SELECT 'Lost the other players', count(DISTINCT sid)::text FROM ee WHERE ev = 'fail' AND p->>'id' = 'room' "
    "UNION ALL SELECT 'Failure rate', CASE WHEN (SELECT n FROM v) > 0 THEN round(100.0 * count(DISTINCT sid) / (SELECT n FROM v))::text || '%' ELSE '-' END "
    "  FROM ee WHERE ev IN ('load-fail', 'gpu-lost', 'crash')", 4, f);
  section(o, db, "EVERGLADE: WHAT WENT WRONG", "w", "WHAT|MESSAGE|TIMES|VISITS|LAST", KEPT EG_E
    "SELECT CASE ev WHEN 'load-fail' THEN 'Failed to load' WHEN 'gpu-lost' THEN 'Lost the GPU' WHEN 'crash' THEN 'The game stopped' "
    "  WHEN 'error' THEN 'Page error' ELSE 'Didn''t work' END, "
    "  coalesce(p->>'msg', p->>'id', '') || coalesce(' (' || (p->>'src') || coalesce(':' || (p->>'line'), '') || ')', ''), "
    "  count(*)::text, count(DISTINCT sid)::text, " CHI("max(ms)") " "
    "FROM ee WHERE " EG_PROBLEM " GROUP BY 1, 2 ORDER BY max(ms) DESC LIMIT 25", 4, f);
  section(o, db, "EVERGLADE: WHAT THEY DID", "", "WHAT|VISITS|TIMES", KEPT EG_E
    ", acts AS (SELECT sid, ev || coalesce(' ' || coalesce(p->>'id', p->>'n'), '') AS what, 1::bigint AS n FROM ee "
    "  WHERE ev IN ('create-class', 'create-done', 'death', 'level-up', 'spawn') "
    "  UNION ALL SELECT ee.sid, j.key, (j.value #>> '{}')::numeric::bigint FROM ee, jsonb_each(ee.p) j "
    "  WHERE ee.ev = 'tally' AND jsonb_typeof(j.value) = 'number') "
    "SELECT what, count(DISTINCT sid)::text, sum(n)::text FROM acts GROUP BY what ORDER BY sum(n) DESC, what LIMIT 30", 4, f);
  section(o, db, "EVERGLADE: SESSIONS", "s", "SESSION|STARTED|WAY IN|FURTHEST|MINUTES|CLASS|FELLED|DIED|PLACE|DEVICE", KEPT EG_E EG_WAY
    ", per AS (SELECT sid, min(coalesce(ct, ms)) AS a, max(coalesce(ct, ms)) AS b, max(" EG_STEP ") AS far, "
    "  (array_agg(coalesce(p->>'class', p->>'id') ORDER BY coalesce(ct, ms) DESC) FILTER (WHERE ev IN ('spawn', 'create-done')))[1] AS klass, "
    "  coalesce(sum((SELECT sum((j.value #>> '{}')::numeric) FROM jsonb_each(p) j WHERE left(j.key, 5) = 'kill_' AND jsonb_typeof(j.value) = 'number')) FILTER (WHERE ev = 'tally'), 0) AS kills, "
    "  count(*) FILTER (WHERE ev = 'death') AS deaths FROM ee GROUP BY sid) "
    "SELECT per.sid, " CHI("per.a") ", coalesce(way.way, ''), (" EG_STEPS ")[per.far], round((per.b - per.a) / 60000.0, 1)::text, "
    "  coalesce(per.klass, ''), per.kills::text, per.deaths::text, "
    "  coalesce(x.region, '') || coalesce(' ' || initcap(x.city), ''), "
    "  coalesce(st.p->>'os', '') || coalesce(' ' || regexp_replace(st.p->>'browser', ' [0-9.]+$', ''), '') || CASE WHEN st.p->>'mobile' = 'true' THEN ' phone' ELSE '' END "
    "FROM per LEFT JOIN way USING (sid) LEFT JOIN analytics.sessions x USING (sid) "
    "LEFT JOIN LATERAL (SELECT p FROM e WHERE e.sid = per.sid AND e.ev = 'session-start' LIMIT 1) st ON true "
    "ORDER BY per.a DESC LIMIT 200", 4, f);
}

static void sessions(out_buf *o, PGconn *db, const char *const *f) {
  section(o, db, "SESSIONS", "s", "SESSION|STARTED|SPACE|FIRST PAGE|EVENTS|MINUTES|FURTHEST|PLACE|DEVICE|AUTOMATED", KEPT
    "SELECT k.sid, " CHI("k.first_ms") ", k.space, coalesce(nullif(st.p->>'entry', ''), fp.page, ''), k.n::text, "
    "  round((k.last_ms - k.first_ms) / 60000.0, 1)::text, (" STEP_NAMES ")[far.far], "
    "  coalesce(x.region, '') || coalesce(' ' || initcap(x.city), ''), "
    "  coalesce(st.p->>'os', '') || coalesce(' ' || regexp_replace(st.p->>'browser', ' [0-9.]+$', ''), '') || CASE WHEN st.p->>'mobile' = 'true' THEN ' phone' ELSE '' END, "
    "  CASE WHEN k.automated THEN coalesce(st.p->>'agent', 'yes') WHEN k.team THEN 'our team' ELSE '' END "
    "FROM k LEFT JOIN analytics.sessions x USING (sid) "
    "LEFT JOIN LATERAL (SELECT p FROM e WHERE e.sid = k.sid AND e.ev = 'session-start' LIMIT 1) st ON true "
    "LEFT JOIN LATERAL (SELECT page FROM e WHERE e.sid = k.sid ORDER BY coalesce(ct, ms), id LIMIT 1) fp ON true "
    "LEFT JOIN LATERAL (SELECT max(" STEP_OF ") AS far FROM e WHERE e.sid = k.sid) far ON true "
    "ORDER BY k.first_ms DESC LIMIT 300", 4, f);
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
  char view[24], days[12], space[12], autos[4], team[4], sid[96];
  api_query(q, "view", view, sizeof view);
  api_query(q, "days", days, sizeof days);
  api_query(q, "space", space, sizeof space);
  api_query(q, "auto", autos, sizeof autos);
  api_query(q, "team", team, sizeof team);   // our own testing: left out unless team=1
  api_query(q, "sid", sid, sizeof sid);
  long d = days[0] ? strtol(days, NULL, 10) : 7;
  if (d < 0 || d > 3650) d = 7;
  char from[32];
  snprintf(from, sizeof from, "%lld", d == 0 ? 0LL : (long long)api_now_ms() - d * 86400000LL);
  if (strcmp(space, "web") && strcmp(space, "kiki") && strcmp(space, "dev")) snprintf(space, sizeof space, "all");
  const char *f[4] = {from, space, !strcmp(autos, "1") ? "true" : "false", !strcmp(team, "1") ? "true" : "false"};
  out_buf o = {0};
  if (!strcmp(view, "session")) {
    for (const char *c = sid; *c; c++)
      if (!isalnum((unsigned char)*c) && *c != '_' && *c != ':' && *c != '-') { api_json(q->fd, 400, "{\"error\":\"bad session\"}"); return; }
    if (!sid[0]) { api_json(q->fd, 400, "{\"error\":\"which session?\"}"); return; }
    one_session(&o, db, sid);
  } else if (!strcmp(view, "sessions")) sessions(&o, db, f);
  else if (!strcmp(view, "everglade")) everglade(&o, db, f);
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

// The rooms' vitals (/admin/rooms, admin.coax; #12791): each shard that reported in the last 10
// minutes (migrations/0123_room_stats.sql), its numbers now and over the 10 minutes (flag "k": the
// last value is the series, drawn as a sparkline), in three groups, then its seats from its last report.
#define ROOM_LINES \
  "WITH p AS (SELECT at, body FROM verse.room_stats WHERE shard = $1::int AND at > now() - interval '10 minutes'), " \
  "l AS (SELECT p.at, x.line, x.ord FROM p, regexp_split_to_table(p.body, E'\\n') WITH ORDINALITY AS x(line, ord) WHERE x.line LIKE '%=%'), " \
  "m AS (SELECT at, ord, split_part(line, '=', 1) AS name, split_part(line, '=', 2) AS v FROM l), " \
  "g AS (SELECT name, min(ord) AS ord, (array_agg(v ORDER BY at DESC))[1] AS now, string_agg(v, ',' ORDER BY at) AS series FROM m GROUP BY name) " \
  "SELECT name, now, series FROM g WHERE name ~ $2 ORDER BY ord"
static void rooms(const request *q, PGconn *db) {
  if (strcmp(q->method, "GET")) { api_json(q->fd, 405, "{\"error\":\"GET\"}"); return; }
  out_buf o = {0};
  // admission (#12796, migrations/0126_room_admission.sql): refusals by reason since the start, and who waits
  section(&o, db, "ADMISSION", "", "REASON|COUNT",
    "SELECT reason, n::text FROM verse.room_admission "
    "UNION ALL SELECT 'waiting now', count(*)::text FROM verse.room_waiting WHERE last_ms > verse.room_now() - 60000 "
    "UNION ALL SELECT 'last wait s', round(wait_ms / 1000.0)::text FROM verse.room_wait_last", 0, NULL);
  section(&o,db,"PLACEMENT CAPACITY","w","SHARD|CONFIGURED WORLDS|DIRECTORY AGE MS|GENERATION|DRAINING|LIVE WORLDS|REPORTED SEATS|PLAYER HOLDS|INVITE HOLDS|REPORTED READY|STATS AGE MS|BUILD",
    "WITH t AS (SELECT verse.room_now() AS ms) "
    "SELECT s.shard::text,s.rooms_cap::text,CASE WHEN s.at IS NULL THEN '-1' ELSE greatest(0,t.ms-s.at)::text END,coalesce(s.gen,0)::text,"
    "verse.room_draining(s.shard,t.ms)::text,i.worlds::text,coalesce(i.seats,0)::text,h.players::text,h.invites::text,"
    "coalesce(substring(b.body FROM E'\\ncapacity startup ready=([01])\\n'),'unknown'),"
    "CASE WHEN b.at IS NULL THEN '-1' ELSE greatest(0,t.ms-(extract(epoch FROM b.at)*1000)::bigint)::text END,coalesce(s.build,'unknown') "
    "FROM verse.room_shards s CROSS JOIN t "
    "LEFT JOIN LATERAL (SELECT count(*) AS worlds,sum(x.players) AS seats FROM verse.room_instances x WHERE x.shard=s.shard AND verse.room_live(x,t.ms)) i ON true "
    "LEFT JOIN LATERAL (SELECT count(*) FILTER(WHERE h.pid NOT LIKE 'i-%') AS players,count(*) FILTER(WHERE h.pid LIKE 'i-%') AS invites "
    "FROM verse.room_holds h JOIN verse.room_instances x ON x.code=h.code WHERE x.shard=s.shard AND h.until_ms>t.ms AND NOT(h.pid=ANY(x.pids))) h ON true "
    "LEFT JOIN LATERAL (SELECT b.at,b.body FROM verse.room_stats b WHERE b.shard=s.shard AND b.at>now()-interval '30 seconds' "
    "AND substring(b.body FROM E'\\nprocess generation=([0-9]{1,15})\\n')::bigint=s.gen ORDER BY b.at DESC LIMIT 1) b ON true ORDER BY s.shard",0,NULL);
  PGresult *r = PQexec(db, "SELECT DISTINCT shard FROM verse.room_stats WHERE at > now() - interval '10 minutes' ORDER BY shard");
  int n = PQresultStatus(r) == PGRES_TUPLES_OK ? PQntuples(r) : 0;
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("rooms: %s", PQerrorMessage(db));
  if (n == 0) puts_(&o, "#ROOMS||NUMBER|NOW\n");
  static const struct { const char *group, *match; } groups[] = {
    {"TICK", "^(ticks a second|tick |players p|raiders |effects |folk |writing |sending |measuring|report |observation measured|process generation)"},
    {"PLAYERS", "^(players$|queue |relevant |input to ack|server queued ack|join|rejoins |refused )"},
    {"BYTES", "^(out|in) .*a second$|^transport "},
    {"REPLICATION", "^(interest (seats|whole |health only |blocked |hidden |aggregation)|rows |row text |row wait |removal |removals |player info |join pieces |cue |cues |outcome |outcomes |snapshots |towns |observation counter)"},
    {"SAVES", "^(checkpoint |legacy |owner )"},
    {"CAPACITY", "^capacity "},
  };
  for (int i = 0; i < n && i < 8; i++) {
    const char *shard = PQgetvalue(r, i, 0);
    char title[48];
    for (size_t g = 0; g < sizeof groups / sizeof groups[0]; g++) {
      snprintf(title, sizeof title, "SHARD %s %s", shard, groups[g].group);
      const char *v[2] = {shard, groups[g].match};
      section(&o, db, title, "k", "NUMBER|NOW|LAST 10 MINUTES", ROOM_LINES, 2, v);
    }
    snprintf(title, sizeof title, "SHARD %s SEATS", shard);
    const char *v[1] = {shard};
    section(&o, db, title, "w", "ROOM|SEAT|QUEUE BYTES|QUEUE MS|RELEVANT|ACK P50 MS|ACK P95 MS|ROUND TRIP MS",
      "WITH b AS (SELECT body FROM verse.room_stats WHERE shard = $1::int ORDER BY at DESC LIMIT 1) "
      "SELECT split_part(x, '|', 2), split_part(x, '|', 3), split_part(x, '|', 4), split_part(x, '|', 5), split_part(x, '|', 6), "
      "  split_part(x, '|', 7), split_part(x, '|', 8), split_part(x, '|', 9) "
      "FROM b, regexp_split_to_table(b.body, E'\\n') AS x WHERE x LIKE 'seat|%'", 1, v);
    snprintf(title,sizeof title,"SHARD %s QUEUED ACK",shard);
    section(&o,db,title,"w","ROOM|SEAT|P50 MS|P95 MS",
      "WITH b AS (SELECT body FROM verse.room_stats WHERE shard=$1::int ORDER BY at DESC LIMIT 1) "
      "SELECT split_part(x,'|',2),split_part(x,'|',3),split_part(x,'|',4),split_part(x,'|',5) "
      "FROM b,regexp_split_to_table(b.body,E'\\n') AS x WHERE x LIKE 'queued ack|%'",1,v);
    snprintf(title,sizeof title,"SHARD %s ACCEPTED SAVES",shard);
    section(&o,db,title,"w","ROOM|SEAT|PROFILE|OWNER|REVISION|CAPTURE AGE MS",
      "WITH b AS (SELECT body FROM verse.room_stats WHERE shard=$1::int ORDER BY at DESC LIMIT 1) "
      "SELECT split_part(x,'|',2),split_part(x,'|',3),split_part(x,'|',4),split_part(x,'|',5),split_part(x,'|',7),split_part(x,'|',8) "
      "FROM b,regexp_split_to_table(b.body,E'\\n') AS x WHERE x LIKE 'saved|%'",1,v);
    snprintf(title,sizeof title,"SHARD %s SAVE DELIVERY",shard);
    section(&o,db,title,"w","ROOM|SEAT|PROCESS|RECEIPT AGE MS|API ACCEPTED MS|LATEST REVISION|PENDING|PAUSED",
      "WITH b AS (SELECT body FROM verse.room_stats WHERE shard=$1::int ORDER BY at DESC LIMIT 1) "
      "SELECT split_part(x,'|',2),split_part(x,'|',3),split_part(x,'|',6),split_part(x,'|',9),split_part(x,'|',10),split_part(x,'|',11),split_part(x,'|',12),split_part(x,'|',13) "
      "FROM b,regexp_split_to_table(b.body,E'\\n') AS x WHERE x LIKE 'saved|%'",1,v);
    snprintf(title,sizeof title,"SHARD %s MEASUREMENT SCOPE",shard);
    section(&o,db,title,"w","MEASUREMENT|SCOPE",
      "WITH b AS (SELECT body FROM verse.room_stats WHERE shard=$1::int ORDER BY at DESC LIMIT 1) "
      "SELECT split_part(x,'=',1),split_part(x,'=',2) FROM b,regexp_split_to_table(b.body,E'\\n') AS x WHERE x LIKE '% scope=%'",1,v);
    snprintf(title,sizeof title,"SHARD %s BUSIEST REPLICATION",shard);
    section(&o,db,title,"w","ROOM|SEAT|SERIALIZED|DEFERRED",
      "WITH b AS (SELECT body FROM verse.room_stats WHERE shard=$1::int ORDER BY at DESC LIMIT 1) "
      "SELECT split_part(x,'|',2),split_part(x,'|',3),split_part(x,'|',4),split_part(x,'|',5) FROM b,regexp_split_to_table(b.body,E'\\n') AS x WHERE x LIKE 'replication worst|%'",1,v);
    snprintf(title,sizeof title,"SHARD %s BUSIEST INTEREST",shard);
    section(&o,db,title,"w","ROOM|SEAT|WHOLE|HEALTH|BLOCKED|HIDDEN",
      "WITH b AS (SELECT body FROM verse.room_stats WHERE shard=$1::int ORDER BY at DESC LIMIT 1) "
      "SELECT split_part(x,'|',2),split_part(x,'|',3),split_part(x,'|',4),split_part(x,'|',5),split_part(x,'|',6),split_part(x,'|',7) "
      "FROM b,regexp_split_to_table(b.body,E'\\n') AS x WHERE x LIKE 'interest worst|%'",1,v);
  }
  PQclear(r);
  api_respond(q->fd, 200, "text/plain; charset=utf-8", "Cache-Control: no-store\r\n", o.p ? o.p : "", o.n);
  free(o.p);
}

// The hosts this site also serves (docs/verse/everglade-domain.md), whose browsers a team link may mark.
static bool team_host(const char *h) {
  static const char *hosts[] = {"everglade.world", "www.everglade.world", "staging.everglade.world", NULL};
  for (int i = 0; hosts[i]; i++) if (!strcmp(h, hosts[i])) return true;
  const char *extra = getenv("OA_TEAM_HOSTS");   // a check's local host (test.sh)
  return extra && *extra && !strcmp(h, extra);
}
static void team_link(const request *q, PGconn *db, const char *who) {
  char host[96];
  if (!api_query(q, "host", host, sizeof host) || !team_host(host)) { api_json(q->fd, 400, "{\"error\":\"which host?\"}"); return; }
  unsigned char raw[24]; char token[49];
  int fd = open("/dev/urandom", O_RDONLY);
  if (fd < 0 || read(fd, raw, sizeof raw) != (ssize_t)sizeof raw) { if (fd >= 0) close(fd); api_json(q->fd, 503, "{\"error\":\"no randomness\"}"); return; }
  close(fd);
  for (size_t i = 0; i < sizeof raw; i++) snprintf(token + 2 * i, 3, "%02x", raw[i]);
  char until[32]; snprintf(until, sizeof until, "%lld", (long long)api_now_ms() + 600000LL);
  const char *v[4] = {token, who, host, until};
  PGresult *r = PQexecParams(db, "INSERT INTO analytics.team_links (token_sha256, pubkey, host, expires_ms) "
                                 "VALUES (encode(sha256(convert_to($1, 'UTF8')), 'hex'), $2, $3, $4::bigint)", 4, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_COMMAND_OK;
  if (!ok) api_log("team link: %s", PQerrorMessage(db));
  PQclear(r);
  if (!ok) { api_json(q->fd, 503, "{\"error\":\"not now\"}"); return; }
  char headers[400];
  snprintf(headers, sizeof headers, "Location: https://%s/api/analytics/team?t=%s\r\nCache-Control: no-store\r\n", host, token);
  api_respond(q->fd, 303, "text/plain", headers, "", 0);
}

bool admin_route(const request *q) {
  if (strncmp(q->path, "/api/admin/", 11)) return false;
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  if (!strcmp(q->path, "/api/admin/login")) { login(q, db); return true; }
  if (!strcmp(q->path, "/api/admin/logout")) { logout(q, db); return true; }
  bool queue = !strncmp(q->path, "/api/admin/reports", 18) && (q->path[18] == 0 || q->path[18] == '/');
  bool room_view = !strcmp(q->path, "/api/admin/rooms");
  bool link = !strcmp(q->path, "/api/admin/team-link");
  if (strcmp(q->path, "/api/admin/analytics") && !queue && !room_view && !link) { api_json(q->fd, 404, "{\"error\":\"not found\"}"); return true; }
  char who[65];
  int status = who_is(q, db, who);
  if (status) { refuse(q->fd, status); return true; }
  if (link) team_link(q, db, who);
  else if (room_view) rooms(q, db);
  else if (queue) review(q, db, who);
  else reports(q, db);
  return true;
}
