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
//   GET  /api/villa/m/<slug>                     the old share link: 301 to /memorial/<slug>
//   GET  /memorial/<slug>                        a memorial's own page (memorial.coax), its link preview naming the pet
//   POST /api/villa/pets/<id>/files/<f>/show|unshow  the owner puts a photo or video on the memorial, or takes it off
//   GET  /api/villa/memorial/<slug>/media/<hex>  a shown file's bytes, from the private bucket (Range too)
//                                                (migrations/0034_villa_memorial_media.sql)
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
#include <strings.h>
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

// ---- a shown photo or video, streamed from the bucket

typedef struct { int fd; long status; char range[128]; long long length; bool sent; const char *type; } relay;
static bool send_all(int fd, const char *p, size_t n) {
  while (n) { ssize_t k = write(fd, p, n); if (k <= 0) return false; p += k; n -= (size_t)k; }
  return true;
}
static size_t relay_head(char *data, size_t size, size_t n, void *user) {
  relay *r = user; size_t k = size * n;
  if (k > 5 && !strncmp(data, "HTTP/", 5)) { const char *sp = memchr(data, ' ', k); r->status = sp ? strtol(sp + 1, NULL, 10) : 0; r->range[0] = 0; r->length = -1; }
  else if (k > 14 && !strncasecmp(data, "Content-Range:", 14)) {
    const char *v = data + 14; size_t len = k - 14;
    while (len && *v == ' ') { v++; len--; }
    while (len && (v[len - 1] == '\r' || v[len - 1] == '\n')) len--;
    if (len < sizeof r->range) { memcpy(r->range, v, len); r->range[len] = 0; }
  } else if (k > 15 && !strncasecmp(data, "Content-Length:", 15)) r->length = strtoll(data + 15, NULL, 10);
  return k;
}
static size_t relay_body(char *data, size_t size, size_t n, void *user) {
  relay *r = user; size_t k = size * n;
  if (!r->sent) {   // the first bytes: our head first, from what the bucket said
    if (r->status != 200 && r->status != 206) return 0;
    char head[700], extra[200] = "";
    if (r->status == 206 && r->range[0]) snprintf(extra, sizeof extra, "Content-Range: %s\r\n", r->range);
    int h = snprintf(head, sizeof head, "HTTP/1.1 %ld %s\r\nContent-Type: %s\r\nContent-Length: %lld\r\nAccept-Ranges: bytes\r\n%s"
                     "Cache-Control: private, max-age=300\r\nX-Content-Type-Options: nosniff\r\nContent-Disposition: inline\r\n"
                     "Content-Security-Policy: default-src 'none'; sandbox\r\nConnection: close\r\n\r\n",
                     r->status, r->status == 206 ? "Partial Content" : "OK", r->type, r->length, extra);
    if (r->length < 0 || !send_all(r->fd, head, (size_t)h)) return 0;
    r->sent = true;
  }
  return send_all(r->fd, data, k) ? k : 0;
}
// A response holds at most this much (Cloud Run's HTTP/1 limit is 32 MiB): a video asks by Range, and an
// open or larger range is answered with its first STREAM_PART bytes, which browsers follow with the next.
#define STREAM_PART (8LL << 20)
#define STREAM_WHOLE (24LL << 20)
static void stream_file(const request *q, PGconn *db, answer a) {
  char object[600], type[200], bytes[24];
  field(db, a.all, "{stream,object}", object, sizeof object); field(db, a.all, "{stream,type}", type, sizeof type);
  field(db, a.all, "{stream,bytes}", bytes, sizeof bytes);
  free(a.body); free(a.all);
  long long total = strtoll(bytes, NULL, 10);
  // the range wanted, held to one part
  char want[160], range[96] = "";
  if (api_header(q, "Range", want, sizeof want) && !strncmp(want, "bytes=", 6) && isdigit((unsigned char)want[6])) {
    char *end; long long from = strtoll(want + 6, &end, 10), to = -1;
    if (*end == '-' && isdigit((unsigned char)end[1])) to = strtoll(end + 1, NULL, 10);
    if (to < 0 || to - from + 1 > STREAM_PART) to = from + STREAM_PART - 1;
    if (from >= total) { api_respond(q->fd, 416, "text/plain", NULL, "", 0); return; }
    snprintf(range, sizeof range, "Range: bytes=%lld-%lld", from, to);
  } else if (total > STREAM_WHOLE) snprintf(range, sizeof range, "Range: bytes=0-%lld", STREAM_PART - 1);
  if (!in_child()) return;
  char tok[4096], name[1024], url[2048], auth[4200];
  if (!drops_token(tok, sizeof tok)) { api_json(q->fd, 503, "{\"error\":\"not now\"}"); _exit(0); }
  size_t n = 0;
  for (const char *c = object; *c && n + 4 < sizeof name; c++) {
    unsigned char ch = (unsigned char)*c;
    if (isalnum(ch) || ch == '-' || ch == '_' || ch == '.' || ch == '~') name[n++] = (char)ch;
    else n += (size_t)snprintf(name + n, sizeof name - n, "%%%02X", ch);
  }
  name[n] = 0;
  snprintf(url, sizeof url, "%s/storage/v1/b/%s/o/%s?alt=media", env_or("OA_DROPS_GCS", "https://storage.googleapis.com"),
           env_or("OA_DROPS_BUCKET", "openagentsgemini-project-drops"), name);
  snprintf(auth, sizeof auth, "Authorization: Bearer %s", tok);
  struct curl_slist *hd = curl_slist_append(NULL, auth);
  if (range[0]) hd = curl_slist_append(hd, range);
  relay r = {q->fd, 0, "", -1, false, type};
  CURL *h = curl_easy_init();
  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  curl_easy_setopt(h, CURLOPT_HEADERFUNCTION, relay_head);
  curl_easy_setopt(h, CURLOPT_HEADERDATA, &r);
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, relay_body);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, &r);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, 120L);
  curl_easy_perform(h);
  if (!r.sent) { api_log("villa: the bucket answered %ld for a shown file", r.status); api_json(q->fd, 502, "{\"error\":\"not now\"}"); }
  curl_slist_free_all(hd); curl_easy_cleanup(h);
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

// The old share link (/api/villa/m/<slug>): on to the memorial's own page.
static void share(const request *q, const char *slug) {
  for (const char *c = slug; *c; c++) if (!isalnum((unsigned char)*c)) { api_json(q->fd, 404, "{\"error\":\"unknown\"}"); return; }
  char to[200], head[300];
  snprintf(to, sizeof to, "/memorial/%.40s", slug);
  snprintf(head, sizeof head, "Location: %s\r\nCache-Control: public, max-age=3600\r\n", to);
  api_respond(q->fd, 301, "text/html; charset=utf-8", head, to, strlen(to));
}

// Appends text to a growing page (the memorial's JSON can be long).
static void page_add(buf *b, const char *t) { take((char *)t, 1, strlen(t), b); }

// A memorial's own page, /memorial/<slug> (the load balancer sends /memorial/* here): its
// link preview names the pet (title, dates and line, the garden's card), and the page is
// memorial.coax, whose script tag it takes from the site's shell page
// (/realms/hecates-cemetery/memorial/, content-named files) with the memorial as its
// data-text, the JSON GET /api/villa/memorial/<slug> answers anyone. A memorial that
// isn't there (removed, held, never was) is the same page saying so, 404.
static void memorial_page(const request *q, PGconn *db, const char *slug) {
  bool ok = *slug && strlen(slug) <= 16;
  for (const char *c = slug; *c; c++) if (!isalnum((unsigned char)*c)) ok = false;
  answer a = {404, NULL, NULL};
  if (ok) {
    char parts[64]; snprintf(parts, sizeof parts, "{memorial,%s}", slug);
    const char *v[6] = {"GET", parts, "{}", "", env_or("OA_DROPS_PREFIX", "dev"), env_or("OA_DROPS_BUCKET", "openagentsgemini-project-drops")};
    a = ask(db, Q("villa.api($1, $2::text[], $3::jsonb, $4, $5, $6)"), 6, v);
  }
  bool found = a.status == 200 && a.body;
  char name[200] = "", line[400] = "", born[60] = "", died[60] = "";
  if (found) {
    char raw[400];
    field(db, a.body, "{name}", raw, sizeof raw); html(name, sizeof name, raw);
    field(db, a.body, "{line}", raw, sizeof raw); html(line, sizeof line, raw);
    field(db, a.body, "{born}", raw, sizeof raw); html(born, sizeof born, raw);
    field(db, a.body, "{died}", raw, sizeof raw); html(died, sizeof died, raw);
    if (!*name) found = false;
  }
  char title[300], desc[700], url[200], dates[140] = "";
  if (*born || *died) snprintf(dates, sizeof dates, "%s%s%s. ", born, *died ? " to " : "", died);
  if (found) snprintf(title, sizeof title, "%s, in Hecate's Cemetery", name);
  else snprintf(title, sizeof title, "A memorial in Hecate's Cemetery");
  snprintf(desc, sizeof desc, "%s%s", found ? dates : "", found && *line ? line : "A memorial in the moonlit garden of Planetary Villa, on the Verse.");
  snprintf(url, sizeof url, "%s/memorial/%s", origin(), ok ? slug : "");
  buf b = {NULL, 0, 0};
  char head[4096];
  snprintf(head, sizeof head,
    "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n"
    "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n"
    "<title>%s</title>\n<meta name=\"description\" content=\"%s\">\n%s"
    "<link rel=\"canonical\" href=\"%s\">\n"
    "<meta property=\"og:type\" content=\"website\">\n<meta property=\"og:site_name\" content=\"OpenAgents\">\n"
    "<meta property=\"og:title\" content=\"%s\">\n<meta property=\"og:description\" content=\"%s\">\n"
    "<meta property=\"og:url\" content=\"%s\">\n"
    "<meta property=\"og:image\" content=\"%s/realms/hecates-cemetery/card.jpg\">\n"
    "<meta property=\"og:image:width\" content=\"1200\">\n<meta property=\"og:image:height\" content=\"630\">\n"
    "<meta property=\"og:image:alt\" content=\"Hecate's Cemetery, a moonlit memorial garden\">\n"
    "<meta name=\"twitter:card\" content=\"summary_large_image\">\n"
    "<meta name=\"twitter:title\" content=\"%s\">\n<meta name=\"twitter:description\" content=\"%s\">\n"
    "<meta name=\"twitter:image\" content=\"%s/realms/hecates-cemetery/card.jpg\">\n"
    "<meta name=\"theme-color\" content=\"#000000\">\n<link rel=\"icon\" type=\"image/svg+xml\" href=\"/favicon.svg\">\n"
    "<script>if (\"gpu\" in navigator) document.documentElement.classList.add(\"gpu\")</script>\n"
    "<style>\nhtml,body{margin:0;height:100%%;background:#3c4a6b;color:#ededed}\n"
    "body{overflow:hidden;font-family:ui-sans-serif,system-ui,-apple-system,\"Segoe UI\",Roboto,\"Helvetica Neue\",Arial,sans-serif;font-size:14px;line-height:1.5}\n"
    "canvas{position:fixed;inset:0;width:100%%;height:100%%;touch-action:none;user-select:none;-webkit-user-select:none;-webkit-touch-callout:none}\n"
    "canvas[data-cursor=pointer]{cursor:pointer}\nmain{position:relative;max-width:640px;margin:0 auto;padding:24px 16px}\n"
    "html:not(.gpu) body{overflow:auto}\na{color:#ededed}\n"
    ".gpu main{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%%);white-space:nowrap;padding:0}\n</style>\n</head>\n<body>\n"
    "<canvas id=\"coaxial\" aria-hidden=\"true\"></canvas>\n<main>\n<h1>%s</h1>\n<p><a href=\"/realms/hecates-cemetery\">Hecate's Cemetery</a></p>\n</main>\n"
    "<script type=\"application/json\" id=\"memorial\">",
    title, desc, found ? "" : "<meta name=\"robots\" content=\"noindex\">\n", url, title, desc, url, origin(), title, desc, origin(),
    found ? title : "This memorial isn't here");
  page_add(&b, head);
  if (found) {   // JSON in a script element: every < written as <, so nothing in it closes the element
    for (const char *c = a.body; *c; c++) { if (*c == '<') page_add(&b, "\\u003c"); else take((char *)c, 1, 1, &b); }
  }
  page_add(&b, "</script>\n<script type=\"module\">\n"
    "const t = await (await fetch(\"/realms/hecates-cemetery/memorial/\")).text();\n"
    "const from = new DOMParser().parseFromString(t, \"text/html\").querySelector(\"script[data-app]\");\n"
    "const s = document.createElement(\"script\");\n"
    "for (const a of from.attributes) s.setAttribute(a.name, a.value);\n"
    "const m = document.getElementById(\"memorial\").textContent;\n");
  page_add(&b, found ? "s.dataset.page = \"0\"; s.dataset.text = m;\n" : "s.dataset.page = \"1\";\n");
  page_add(&b, "document.body.append(s);\n</script>\n</body>\n</html>\n");
  free(a.body); free(a.all);
  if (!b.p) { api_json(q->fd, 503, "{\"error\":\"memory\"}"); return; }
  api_respond(q->fd, found ? 200 : 404, "text/html; charset=utf-8", found ? "Cache-Control: public, max-age=60\r\n" : "Cache-Control: no-store\r\n", b.p, b.n);
  free(b.p);
}

bool villa_route(const request *q) {
  if (!strncmp(q->path, "/memorial/", 10) || !strcmp(q->path, "/memorial")) {
    if (strcmp(q->method, "GET") && strcmp(q->method, "HEAD")) { api_json(q->fd, 405, "{\"error\":\"GET\"}"); return true; }
    const char *slug = q->path[9] ? q->path + 10 : "";
    char want[40]; snprintf(want, sizeof want, "%.32s", slug);
    size_t wl = strlen(want); if (wl && want[wl - 1] == '/') want[wl - 1] = 0;   // /memorial/<slug>/ too
    PGconn *db = api_db();
    if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
    memorial_page(q, db, want);
    return true;
  }
  if (strncmp(q->path, PREFIX, strlen(PREFIX))) return false;
  const char *rest = q->path + strlen(PREFIX);
  bool post = !strcmp(q->method, "POST");
  if (!post && strcmp(q->method, "GET")) { api_json(q->fd, 405, "{\"error\":\"GET or POST\"}"); return true; }
  PGconn *db = api_db();
  if (!db) { api_json(q->fd, 503, "{\"error\":\"no database\"}"); return true; }
  if (!strncmp(rest, "m/", 2) && !post) { share(q, rest + 2); return true; }
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
  // a shown file (memorial/<slug>/media/<hex>), or its owner showing one (pets/<id>/files/<f>/show|unshow)
  size_t rl = strlen(rest);
  bool media = (!strncmp(rest, "memorial/", 9) && strstr(rest, "/media/"))
            || (!strncmp(rest, "pets/", 5) && strstr(rest, "/files/") && ((rl > 5 && !strcmp(rest + rl - 5, "/show")) || (rl > 7 && !strcmp(rest + rl - 7, "/unshow"))));
  bool reports = !strncmp(rest, "report", 6) && (rest[6] == 0 || rest[6] == '/' || (rest[6] == 's' && (rest[7] == 0 || rest[7] == '/')));
  char admin[65] = "";
  if (reports && rest[6] == 's' && admin_who(q, db, admin)) admin[0] = 0;   // the review queue: a signed-in admin only
  const char *rv[5] = {q->method, parts, body, key, admin};
  const char *mv[3] = {q->method, parts, key};
  answer a = media ? ask(db, Q("villa.media_api($1, $2::text[], $3)"), 3, mv)
           : emote ? ask(db, Q("villa.emote_api($1, $2::text[], $3::jsonb, $4)"), 4, v)
           : reports ? ask(db, Q("villa.reports_api($1, $2::text[], $3::jsonb, $4, $5)"), 5, rv)
           : vases ? ask(db, Q("villa.vases_api($1, $2::text[], $3::jsonb, $4)"), 4, v)
                   : ask(db, Q("villa.api($1, $2::text[], $3::jsonb, $4, $5, $6)"), 6, v);
  free(body);
  if (a.status == 200 && a.all) {
    if (media && strstr(a.all, "\"stream\": {")) { stream_file(q, db, a); return true; }
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
