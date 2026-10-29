// Planetary Villa's pets (Hecate's Cemetery; verse/planetary-villa/README.md).
// The rules live in SQL (migrations/0028_villa_pets.sql, villa.api); this file
// reads who is asking (the playtest player key, X-Verse-Player) and does the
// bucket's part (drops.c's, the same private bucket under <env>/villa/) and the
// avatar agent's call (Gemini on Vertex AI, Google first).
//
//   GET  /api/villa/garden                       every placed pet's tombstone {pets: [{slug, name, born, died, line, plot, avatar, mine}]}
//   GET  /api/villa/memorial/<slug>              a memorial {name, dates, line, story, traits, achievements, avatar, notes, mine}
//   POST /api/villa/memorial/<slug>/notes {text} a testimonial (a player key)
//   GET  /api/villa/pets                         your pets, with their files
//   POST /api/villa/pets {name, species}         a new pet
//   GET/POST /api/villa/pets/<id>                one; POST edits {name, born, died, line, story, traits, achievements, avatar}
//   POST /api/villa/pets/<id>/files {name, bytes}  an upload opened for exactly that many bytes of our type -> {file, upload}
//                                                (the browser PUTs the file to upload, then says it's done)
//   POST /api/villa/pets/<id>/files/<f>/done     the scan: size and first 4 KiB; a refused file leaves the bucket
//   POST /api/villa/pets/<id>/files/<f>/delete   the file leaves the bucket
//   POST /api/villa/pets/<id>/agent              the agent reads the text, photos and videos -> the pet with its avatar and choices
//   POST /api/villa/pets/<id>/place {near}       PLAY: its tombstone takes a plot
//   POST /api/villa/pets/<id>/remove             the pet, its tombstone, notes and files, gone
//   POST /api/villa/pets/<id>/notes/<n>/hide|show
//   GET  /api/villa/vases                        the vases left in the garden and the field; POST one {x, z, yaw, flowers, plot}
//   POST /api/villa/vases/<n>/clean              its maker cleans it up
//   POST /api/villa/pets/<id>/emote {said, error, last}  an emote the agent writes; emote/keep {name, label, text}; emote/<name>/delete
//   POST /api/villa/report {kind, ref, why}      a memorial, a note or a chat line reported (migrations/0031_villa_reports.sql)
//   GET  /api/villa/reports                      a signed-in admin: the open reports; POST reports/<kind>/<ref>/remove|keep
//   GET  /api/villa/m/<slug>                     the share link: a page with the memorial's card, on to the realm
//
// A write comes from this site's pages (Origin). Nothing anyone shared is logged.
//   OA_DROPS_BUCKET, OA_DROPS_PREFIX   the bucket and this environment's folder (drops.c)
//   OA_VILLA_MODEL   the agent's model (gemini-3.8-flash, on the global endpoint)
//   OA_VILLA_URL     its generateContent URL (tests point it at a fake)
#include "api.h"
#include <ctype.h>
#include <curl/curl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define PREFIX "/api/villa/"

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }
static const char *origin(void) { return env_or("OA_API_ORIGIN", "https://openagents.com"); }

typedef struct { char *p; size_t n, cap; } buf;
static size_t take(char *data, size_t size, size_t n, void *user) {
  buf *b = user; size_t k = size * n;
  if (b->n + k + 1 > b->cap) {
    size_t cap = (b->n + k + 1) * 2;
    if (cap > (1u << 22)) return 0;
    char *p = realloc(b->p, cap);
    if (!p) return 0;
    b->p = p; b->cap = cap;
  }
  memcpy(b->p + b->n, data, k); b->n += k; b->p[b->n] = 0;
  return k;
}

typedef struct { int status; char *body; char *all; } answer;
static answer ask(PGconn *db, const char *sql, int n, const char *const *v) {
  answer a = {500, NULL, NULL};
  PGresult *r = PQexecParams(db, sql, n, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    a.status = atoi(PQgetvalue(r, 0, 0)); a.body = strdup(PQgetvalue(r, 0, 1)); a.all = strdup(PQgetvalue(r, 0, 2));
  } else {
    const char *state = PQresultErrorField(r, PG_DIAG_SQLSTATE);
    a.status = state && !strncmp(state, "22", 2) ? 400 : 500;
    api_log("villa: sql %s", state ? state : "?");
  }
  PQclear(r);
  return a;
}
static void reply(int fd, answer a) {
  if (a.body) api_json(fd, a.status, a.body);
  else api_json(fd, a.status, a.status == 400 ? "{\"error\":\"not json\"}" : "{\"error\":\"not kept\"}");
  free(a.body); free(a.all);
}
#define Q(fn) "SELECT (x->>'status')::int, (x->'body')::text, x::text FROM " fn " x"

// A text field of a JSON object (as Postgres prints it) into out.
static bool field(PGconn *db, const char *json, const char *path, char *out, size_t cap) {
  out[0] = 0;
  const char *v[2] = {json, path};
  PGresult *r = PQexecParams(db, "SELECT $1::jsonb #>> $2::text[]", 2, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0);
  if (ok) snprintf(out, cap, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  return ok;
}

static bool in_child(void) {
  signal(SIGCHLD, SIG_IGN);
  pid_t pid = fork();
  return pid == 0;
}

// The objects a result says to delete, each out of the bucket (in the caller's child).
static void deletes(PGconn *db, const char *all) {
  const char *v[1] = {all};
  PGresult *r = PQexecParams(db, "SELECT jsonb_array_elements_text(coalesce($1::jsonb->'delete', '[]'))", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) == PGRES_TUPLES_OK) for (int i = 0; i < PQntuples(r); i++) drops_delete_object(PQgetvalue(r, i, 0));
  PQclear(r);
}

static void open_file(const request *q, PGconn *db, answer a) {
  char object[600], type[200], file[40], bytes[24], from[256];
  field(db, a.all, "{open,object}", object, sizeof object); field(db, a.all, "{open,type}", type, sizeof type);
  field(db, a.all, "{open,file}", file, sizeof file); field(db, a.all, "{open,bytes}", bytes, sizeof bytes);
  if (!api_header(q, "Origin", from, sizeof from) || strcmp(from, origin())) snprintf(from, sizeof from, "%s", origin());
  free(a.body); free(a.all);
  if (!in_child()) return;
  char session[2048];
  bool opened = drops_open_upload(object, type, bytes, from, session, sizeof session);
  char out[2400];
  if (opened) {
    snprintf(out, sizeof out, "{\"file\":\"%s\",\"upload\":\"%s\"}", file, session);
    api_json(q->fd, 200, out);
  } else api_json(q->fd, 503, "{\"error\":\"we could not open the upload: try again\"}");
  _exit(0);
}

static void scan_file(const request *q, PGconn *db, answer a) {
  char object[600], file[40];
  field(db, a.all, "{scan,object}", object, sizeof object); field(db, a.all, "{scan,file}", file, sizeof file);
  free(a.body); free(a.all);
  if (!in_child()) return;
  long long size; char *head = NULL, sz[24];
  drops_inspect(object, &size, &head);
  snprintf(sz, sizeof sz, "%lld", size);
  PGconn *c = api_db_new();
  if (!c) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); free(head); _exit(0); }
  const char *w[3] = {file, sz, head ? head : ""};
  answer s = ask(c, Q("villa.scanned($1, $2::bigint, decode($3, 'hex'))"), 3, w);
  if (s.all) deletes(c, s.all);
  reply(q->fd, s);
  PQfinish(c); free(head);
  _exit(0);
}

// The avatar agent: the request to Gemini, its answer kept.
static void agent(const request *q, PGconn *db, answer a) {
  char pet[40], done[16] = "";
  field(db, a.all, "{pet}", pet, sizeof pet);
  field(db, a.all, "{done}", done, sizeof done);   // "emote": a motion (0032_villa_emotes.sql), else the avatar
  const char *v[1] = {a.all};
  PGresult *r = PQexecParams(db, "SELECT ($1::jsonb->'request')::text", 1, NULL, v, NULL, NULL, 0);
  char *request = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 ? strdup(PQgetvalue(r, 0, 0)) : NULL;
  PQclear(r); free(a.body); free(a.all);
  if (!request) { api_json(q->fd, 500, "{\"error\":\"not kept\"}"); return; }
  if (!in_child()) { free(request); return; }
  const char *model = env_or("OA_VILLA_MODEL", "gemini-3.8-flash");
  char url[512], tok[4096], auth[4200], http[16], took[16];
  const char *given = getenv("OA_VILLA_URL");
  if (given && *given) snprintf(url, sizeof url, "%s", given);
  else snprintf(url, sizeof url, "https://aiplatform.googleapis.com/v1/projects/openagentsgemini/locations/global/publishers/google/models/%s:generateContent", model);
  long status = 0; buf b = {0};
  double t0 = api_now_ms();
  if (drops_token(tok, sizeof tok)) {
    CURL *h = curl_easy_init();
    snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
    struct curl_slist *hd = curl_slist_append(curl_slist_append(NULL, auth), "Content-Type: application/json");
    curl_easy_setopt(h, CURLOPT_URL, url);
    curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
    curl_easy_setopt(h, CURLOPT_POSTFIELDS, request);
    curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
    curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
    curl_easy_setopt(h, CURLOPT_TIMEOUT, 170L);
    if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
    curl_slist_free_all(hd); curl_easy_cleanup(h);
  } else api_log("villa: no token for the agent");
  if (status != 200) api_log("villa: the agent answered %ld", status);
  snprintf(http, sizeof http, "%ld", status);
  snprintf(took, sizeof took, "%d", (int)(api_now_ms() - t0));
  PGconn *c = api_db_new();
  if (!c) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); _exit(0); }
  const char *w[4] = {pet, http, b.p ? b.p : "", took};
  reply(q->fd, !strcmp(done, "emote") ? ask(c, Q("villa.emote_done($1, $2::int, $3, $4::int)"), 4, w)
                                      : ask(c, Q("villa.agent_done($1, $2::int, $3, $4::int)"), 4, w));
  PQfinish(c); free(b.p); free(request);
  _exit(0);
}

// HTML-escaped text into out.
static void html(char *out, size_t cap, const char *s) {
  size_t n = 0;
  for (; *s && n + 8 < cap; s++) {
    const char *e = *s == '<' ? "&lt;" : *s == '>' ? "&gt;" : *s == '&' ? "&amp;" : *s == '"' ? "&quot;" : NULL;
    if (e) { size_t k = strlen(e); memcpy(out + n, e, k); n += k; } else out[n++] = *s;
  }
  out[n] = 0;
}

// The share link: a small page whose card names the pet, on to the memorial in the realm.
static void share(const request *q, PGconn *db, const char *slug) {
  for (const char *c = slug; *c; c++) if (!isalnum((unsigned char)*c)) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return; }
  const char *v[1] = {slug};
  PGresult *r = PQexecParams(db, "SELECT name, line, born, died FROM villa.pets WHERE slug = $1 AND status = 'placed'", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) {
    PQclear(r);
    const char *gone = "<!doctype html><meta charset=utf-8><title>Hecate's Cemetery</title><main><h1>Hecate's Cemetery</h1><a href=\"/realm/hecates-cemetery/\">Hecate's Cemetery</a></main>";
    api_respond(q->fd, 404, "text/html; charset=utf-8", NULL, gone, strlen(gone));
    return;
  }
  char name[200], line[400], title[300], desc[600], page[4096], to[200];
  html(name, sizeof name, PQgetvalue(r, 0, 0)); html(line, sizeof line, PQgetvalue(r, 0, 1));
  const char *born = PQgetvalue(r, 0, 2), *died = PQgetvalue(r, 0, 3);
  char dates[120] = "";
  if (*born || *died) snprintf(dates, sizeof dates, "%.40s%s%.40s. ", born, *died ? " to " : "", died);
  PQclear(r);
  snprintf(title, sizeof title, "%s, in Hecate's Cemetery", name);
  char dd[300]; html(dd, sizeof dd, dates);
  snprintf(desc, sizeof desc, "%s%s", dd, *line ? line : "A memorial in the moonlit garden of Planetary Villa.");
  snprintf(to, sizeof to, "/realm/hecates-cemetery/#m/%s", slug);
  snprintf(page, sizeof page,
           "<!doctype html><html lang=en><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\">"
           "<title>%s</title><meta name=description content=\"%s\">"
           "<meta property=og:title content=\"%s\"><meta property=og:description content=\"%s\">"
           "<meta property=og:image content=\"%s/realm/hecates-cemetery/card.jpg\"><meta property=og:url content=\"%s/api/villa/m/%s\">"
           "<meta name=twitter:card content=summary_large_image><meta http-equiv=refresh content=\"0;url=%s\"></head>"
           "<body style=\"background:#05070d;color:#cfd8ee\"><main><h1>%s</h1><a href=\"%s\">%s</a></main></body></html>",
           title, desc, title, desc, origin(), origin(), slug, to, name, to, title);
  api_respond(q->fd, 200, "text/html; charset=utf-8", "Cache-Control: public, max-age=300\r\n", page, strlen(page));
}

bool villa_route(const request *q) {
  if (strncmp(q->path, PREFIX, strlen(PREFIX))) return false;
  const char *rest = q->path + strlen(PREFIX);
  bool post = !strcmp(q->method, "POST");
  if (!post && strcmp(q->method, "GET")) { api_json(q->fd, 405, "{\"error\":\"GET or POST\"}"); return true; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  if (!strncmp(rest, "m/", 2) && !post) { share(q, db, rest + 2); return true; }
  if (post) {   // CSRF: a write comes from this site's pages
    char from[256];
    if (api_header(q, "Origin", from, sizeof from) && strcmp(from, origin())) { api_json(q->fd, 403, "{\"error\":\"cross-site\"}"); return true; }
  }
  // The path as a Postgres text[] literal: segments of [A-Za-z0-9_-] only.
  char parts[300]; size_t pn = 0; int segs = 0;
  parts[pn++] = '{';
  for (const char *p = rest; *p; p++) {
    char ch = *p;
    if (ch == '/') { if (p[1] == 0) break; parts[pn++] = ','; continue; }
    bool ok = isalnum((unsigned char)ch) || ch == '_' || ch == '-';
    if (!ok || pn + 3 >= sizeof parts) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return true; }
    if (p == rest || p[-1] == '/') segs++;
    parts[pn++] = ch;
  }
  parts[pn++] = '}'; parts[pn] = 0;
  if (!segs || segs > 5) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return true; }
  char key[160];
  api_header(q, "X-Verse-Player", key, sizeof key);
  char *body = malloc(q->body_len + 3);
  if (!body) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return true; }
  if (q->body_len) { memcpy(body, q->body, q->body_len); body[q->body_len] = 0; } else strcpy(body, "{}");
  const char *v[6] = {q->method, parts, body, key, env_or("OA_DROPS_PREFIX", "dev"), env_or("OA_DROPS_BUCKET", "openagentsgemini-project-drops")};
  bool vases = !strncmp(rest, "vases", 5) && (rest[5] == 0 || rest[5] == '/');
  bool emote = !strncmp(rest, "pets/", 5) && strstr(rest, "/emote") && (strstr(rest, "/emote")[6] == 0 || strstr(rest, "/emote")[6] == '/');
  bool reports = !strncmp(rest, "report", 6) && (rest[6] == 0 || rest[6] == '/' || (rest[6] == 's' && (rest[7] == 0 || rest[7] == '/')));
  char admin[65] = "";
  if (reports && rest[6] == 's' && admin_who(q, db, admin)) admin[0] = 0;   // the review queue: a signed-in admin only
  const char *rv[5] = {q->method, parts, body, key, admin};
  answer a = emote ? ask(db, Q("villa.emote_api($1, $2::text[], $3::jsonb, $4)"), 4, v)
           : reports ? ask(db, Q("villa.reports_api($1, $2::text[], $3::jsonb, $4, $5)"), 5, rv)
           : vases ? ask(db, Q("villa.vases_api($1, $2::text[], $3::jsonb, $4)"), 4, v)
                   : ask(db, Q("villa.api($1, $2::text[], $3::jsonb, $4, $5, $6)"), 6, v);
  free(body);
  if (a.status == 200 && a.all) {
    if (strstr(a.all, "\"open\": {")) { open_file(q, db, a); return true; }
    if (strstr(a.all, "\"scan\": {")) { scan_file(q, db, a); return true; }
    if (strstr(a.all, "\"request\": {")) { agent(q, db, a); return true; }
    if (strstr(a.all, "\"delete\": [\"")) {
      if (in_child()) { PGconn *c = api_db_new(); if (c) { deletes(c, a.all); PQfinish(c); } reply(q->fd, a); _exit(0); }
      free(a.body); free(a.all);
      return true;
    }
  }
  reply(q->fd, a);
  return true;
}
