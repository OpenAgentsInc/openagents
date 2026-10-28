#include "speech.h"
#include "wav.h"
#include "ws.h"
#include "VoiceReply.h"
#include <CommonCrypto/CommonDigest.h>
#include <curl/curl.h>
#include <errno.h>
#include <math.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

speech_config g_speech;
static void share_init(void);

static const speech_profile PROFILES[] = {
    {"foreman", 1, "en-US-Chirp3-HD-Charon", "en-US"},
    {"kore", 1, "en-US-Chirp3-HD-Kore", "en-US"},
    {"puck", 1, "en-US-Chirp3-HD-Puck", "en-US"},
};

const speech_profile *speech_find_profile(const char *id) {
  if (!id || !*id) return &PROFILES[0];
  for (size_t i = 0; i < sizeof PROFILES / sizeof PROFILES[0]; i++)
    if (strcmp(PROFILES[i].id, id) == 0) return &PROFILES[i];
  return NULL;
}

static void seterr(char *err, size_t cap, const char *fmt, ...) {
  if (!err || !cap) return;
  va_list ap;
  va_start(ap, fmt);
  vsnprintf(err, cap, fmt, ap);
  va_end(ap);
}

static void mkdirs(const char *path) {
  char tmp[600];
  snprintf(tmp, sizeof tmp, "%s", path);
  for (char *p = tmp + 1; *p; p++)
    if (*p == '/') { *p = 0; mkdir(tmp, 0755); *p = '/'; }
  mkdir(tmp, 0755);
}

void speech_init(void) {
  curl_global_init(CURL_GLOBAL_DEFAULT);
  share_init();
  if (!g_speech.project[0]) snprintf(g_speech.project, sizeof g_speech.project, "openagentsgemini");
  if (!g_speech.model[0]) snprintf(g_speech.model, sizeof g_speech.model, "gemini-2.5-flash");
  if (!g_speech.gcloud_config[0]) {
    const char *e = getenv("OA_VOICE_GCLOUD_CONFIG");
    snprintf(g_speech.gcloud_config, sizeof g_speech.gcloud_config, "%s",
             e && *e ? e : "/Users/christopherdavid/work/.secrets/gcloud-sa-config");
  }
  if (!g_speech.cache_dir[0]) {
    const char *home = getenv("HOME");
    snprintf(g_speech.cache_dir, sizeof g_speech.cache_dir, "%s/.cache/openagents-voice/renders", home ? home : "/tmp");
  }
}

// ---------------------------------------------------------------- access token

static pthread_mutex_t token_lock = PTHREAD_MUTEX_INITIALIZER;
static char token[4096];
static time_t token_at;

// Copies a current token into out. Refreshes when older than max_age seconds
// (Google tokens last 60 minutes) or when `force`. The token is never printed.
static bool get_token_aged(char *out, size_t cap, bool force, long max_age, char *err, size_t errcap) {
  pthread_mutex_lock(&token_lock);
  time_t now = time(NULL);
  if (force || !token[0] || now - token_at > max_age) {
    if (strchr(g_speech.gcloud_config, '\'')) {
      pthread_mutex_unlock(&token_lock);
      seterr(err, errcap, "bad gcloud config path");
      return false;
    }
    char fresh[4096] = "";
    char cmd[800];
    snprintf(cmd, sizeof cmd, "CLOUDSDK_CONFIG='%s' gcloud auth print-access-token 2>/dev/null", g_speech.gcloud_config);
    FILE *p = popen(cmd, "r");
    if (p) {
      size_t got = fread(fresh, 1, sizeof fresh - 1, p);
      fresh[got] = 0;
      pclose(p);
      while (got && (fresh[got - 1] == '\n' || fresh[got - 1] == '\r' || fresh[got - 1] == ' ')) fresh[--got] = 0;
      if (got < 20 || strchr(fresh, ' ')) fresh[0] = 0;
    }
    // Keep a still-valid old token if a refresh fails.
    if (fresh[0] || force || now - token_at > 55 * 60) {
      memcpy(token, fresh, sizeof token);
      token_at = now;
    }
    memset(fresh, 0, sizeof fresh);
  }
  bool ok = token[0] != 0;
  if (ok) snprintf(out, cap, "%s", token);
  pthread_mutex_unlock(&token_lock);
  if (!ok) seterr(err, errcap, "no Google access token (gcloud auth print-access-token failed)");
  return ok;
}
static bool get_token(char *out, size_t cap, bool force, char *err, size_t errcap) {
  return get_token_aged(out, cap, force, 45 * 60, err, errcap);
}

bool speech_google_ready(char *err, size_t errcap) {
  char t[4096];
  bool ok = get_token(t, sizeof t, false, err, errcap);
  memset(t, 0, sizeof t);
  return ok;
}

// ---------------------------------------------------------------- HTTP + JSON

typedef struct { char *p; size_t n, cap; } buf;

static bool buf_put(buf *b, const char *s, size_t n) {
  if (b->n + n + 1 > b->cap) {
    size_t c = b->cap ? b->cap : 4096;
    while (c < b->n + n + 1) c *= 2;
    char *q = realloc(b->p, c);
    if (!q) return false;
    b->p = q;
    b->cap = c;
  }
  memcpy(b->p + b->n, s, n);
  b->n += n;
  b->p[b->n] = 0;
  return true;
}
static bool buf_str(buf *b, const char *s) { return buf_put(b, s, strlen(s)); }

static bool buf_json_string(buf *b, const char *s) {
  bool ok = buf_put(b, "\"", 1);
  for (const unsigned char *p = (const unsigned char *)s; ok && *p; p++) {
    char e[8];
    if (*p == '"' || *p == '\\') { e[0] = '\\'; e[1] = (char)*p; ok = buf_put(b, e, 2); }
    else if (*p == '\n') ok = buf_put(b, "\\n", 2);
    else if (*p < 0x20) { snprintf(e, sizeof e, "\\u%04x", *p); ok = buf_put(b, e, 6); }
    else ok = buf_put(b, (const char *)p, 1);
  }
  return ok && buf_put(b, "\"", 1);
}

static size_t on_body(char *d, size_t sz, size_t nm, void *u) {
  return buf_put((buf *)u, d, sz * nm) ? sz * nm : 0;
}

// Connections: every worker's handles share one connection cache, DNS cache
// and TLS session cache, so a connection opened by a warm-up (or by any
// earlier call) is reused by whichever worker makes the next call. HTTP/2
// over TLS where the server offers it.
static CURLSH *share;
static pthread_mutex_t share_locks[CURL_LOCK_DATA_LAST];
static void share_lock(CURL *h, curl_lock_data d, curl_lock_access a, void *u) { (void)h; (void)a; (void)u; pthread_mutex_lock(&share_locks[d]); }
static void share_unlock(CURL *h, curl_lock_data d, void *u) { (void)h; (void)u; pthread_mutex_unlock(&share_locks[d]); }

static void share_init(void) {
  for (int i = 0; i < CURL_LOCK_DATA_LAST; i++) pthread_mutex_init(&share_locks[i], NULL);
  share = curl_share_init();
  curl_share_setopt(share, CURLSHOPT_LOCKFUNC, share_lock);
  curl_share_setopt(share, CURLSHOPT_UNLOCKFUNC, share_unlock);
  curl_share_setopt(share, CURLSHOPT_SHARE, CURL_LOCK_DATA_CONNECT);
  curl_share_setopt(share, CURLSHOPT_SHARE, CURL_LOCK_DATA_DNS);
  curl_share_setopt(share, CURLSHOPT_SHARE, CURL_LOCK_DATA_SSL_SESSION);
}

// One easy handle per thread, reused call after call.
static pthread_key_t handle_key;
static pthread_once_t handle_once = PTHREAD_ONCE_INIT;
static void handle_free(void *h) { curl_easy_cleanup(h); }
static void handle_key_init(void) { pthread_key_create(&handle_key, handle_free); }
static CURL *thread_handle(void) {
  pthread_once(&handle_once, handle_key_init);
  CURL *c = pthread_getspecific(handle_key);
  if (!c) {
    c = curl_easy_init();
    if (!c) return NULL;
    pthread_setspecific(handle_key, c);
  }
  curl_easy_reset(c);
  curl_easy_setopt(c, CURLOPT_SHARE, share);
  curl_easy_setopt(c, CURLOPT_HTTP_VERSION, (long)CURL_HTTP_VERSION_2TLS);
  curl_easy_setopt(c, CURLOPT_TCP_KEEPALIVE, 1L);
  curl_easy_setopt(c, CURLOPT_NOSIGNAL, 1L);
  curl_easy_setopt(c, CURLOPT_CONNECTTIMEOUT, 5L);
  return c;
}

typedef size_t (*write_fn)(char *, size_t, size_t, void *);

// POST JSON with the bearer token. Returns the HTTP status (0 on transport
// failure or when the write callback aborted). Retries once with a fresh
// token on 401. With a custom write function, *resp only collects non-200
// bodies' errors when write is NULL.
static long post_json_to(const char *url, const char *body, write_fn write, void *ctx, buf *resp, long timeout,
                         char *err, size_t errcap) {
  for (int attempt = 0; attempt < 2; attempt++) {
    char t[4096], auth[4200], proj[128];
    if (!get_token(t, sizeof t, attempt > 0, err, errcap)) return 0;
    snprintf(auth, sizeof auth, "Authorization: Bearer %s", t);
    memset(t, 0, sizeof t);
    snprintf(proj, sizeof proj, "x-goog-user-project: %s", g_speech.project);
    CURL *c = thread_handle();
    if (!c) { seterr(err, errcap, "curl init failed"); return 0; }
    struct curl_slist *h = NULL;
    h = curl_slist_append(h, auth);
    h = curl_slist_append(h, proj);
    h = curl_slist_append(h, "Content-Type: application/json");
    memset(auth, 0, sizeof auth);
    resp->n = 0;
    if (resp->p) resp->p[0] = 0;
    curl_easy_setopt(c, CURLOPT_URL, url);
    curl_easy_setopt(c, CURLOPT_HTTPHEADER, h);
    curl_easy_setopt(c, CURLOPT_POSTFIELDS, body);
    curl_easy_setopt(c, CURLOPT_WRITEFUNCTION, write ? write : on_body);
    curl_easy_setopt(c, CURLOPT_WRITEDATA, write ? ctx : resp);
    curl_easy_setopt(c, CURLOPT_TIMEOUT, timeout);
    CURLcode rc = curl_easy_perform(c);
    long status = 0, connects = 0, version = 0;
    curl_easy_getinfo(c, CURLINFO_RESPONSE_CODE, &status);
    curl_easy_getinfo(c, CURLINFO_NUM_CONNECTS, &connects);
    curl_easy_getinfo(c, CURLINFO_HTTP_VERSION, &version);
    if (g_speech.verbose) {
      double tls = 0, total = 0;
      curl_easy_getinfo(c, CURLINFO_APPCONNECT_TIME, &tls);
      curl_easy_getinfo(c, CURLINFO_TOTAL_TIME, &total);
      fprintf(stderr, "voice-host: http %.40s... %s connection, HTTP/%s, tls %.0f ms, total %.0f ms\n", url + 8,
              connects ? "new" : "reused", version == CURL_HTTP_VERSION_2_0 ? "2" : "1.1", tls * 1000, total * 1000);
    }
    curl_easy_setopt(c, CURLOPT_HTTPHEADER, NULL);
    curl_slist_free_all(h);
    if (rc == CURLE_WRITE_ERROR && write) { seterr(err, errcap, "cancelled"); return 0; }
    if (rc != CURLE_OK) { seterr(err, errcap, "HTTP transport: %s", curl_easy_strerror(rc)); return 0; }
    if (status == 401 && attempt == 0) continue;
    if (status != 200) {
      // The body is Google's error JSON; keep a short prefix, never the request.
      seterr(err, errcap, "HTTP %ld from %s: %.160s", status, url, resp->p ? resp->p : "");
    }
    return status;
  }
  return 0;
}
static long post_json(const char *url, const char *body, buf *resp, char *err, size_t errcap) {
  return post_json_to(url, body, NULL, NULL, resp, 20L, err, errcap);
}

static size_t discard(char *d, size_t sz, size_t nm, void *u) { (void)d; (void)u; return sz * nm; }

void speech_warm(bool speech, bool agent) {
  char t[4096], err[256];
  // Refresh the token well before expiry, here and not on a turn's path.
  if (!get_token_aged(t, sizeof t, false, 30 * 60, err, sizeof err)) return;
  memset(t, 0, sizeof t);
  const char *hosts[3];
  int n = 0;
  if (speech) {
    hosts[n++] = "https://us-speech.googleapis.com/";
    hosts[n++] = "https://texttospeech.googleapis.com/";
  }
  if (agent) hosts[n++] = "https://us-central1-aiplatform.googleapis.com/";
  for (int i = 0; i < n; i++) {
    // An unauthenticated GET of the root (a quick 404) opens the TLS
    // connection into the shared cache; nothing is sent but the path.
    CURL *c = thread_handle();
    if (!c) return;
    curl_easy_setopt(c, CURLOPT_URL, hosts[i]);
    curl_easy_setopt(c, CURLOPT_WRITEFUNCTION, discard);
    curl_easy_setopt(c, CURLOPT_TIMEOUT, 5L);
    curl_easy_perform(c);
  }
}

// Bounded field extraction from a provider's JSON response: finds the next
// "key": "<string>" at or after *pos and decodes the string into out.
// Returns false if there is none. (The provider's response shape is fixed;
// this reads named fields, it does not route on content.)
static bool json_next_string(const char *json, const char *key, size_t *pos, char *out, size_t cap) {
  char pat[80];
  snprintf(pat, sizeof pat, "\"%s\"", key);
  const char *p = strstr(json + *pos, pat);
  while (p) {
    const char *q = p + strlen(pat);
    while (*q == ' ' || *q == '\n' || *q == '\r' || *q == '\t') q++;
    if (*q == ':') {
      q++;
      while (*q == ' ' || *q == '\n' || *q == '\r' || *q == '\t') q++;
      if (*q == '"') break;
    }
    p = strstr(p + 1, pat);
  }
  if (!p) return false;
  const char *q = strchr(p + strlen(pat), '"') + 1;
  size_t o = 0;
  while (*q && *q != '"') {
    unsigned c = (unsigned char)*q++;
    if (c == '\\') {
      char e = *q++;
      if (e == 'n') c = '\n';
      else if (e == 't') c = '\t';
      else if (e == 'r') c = '\r';
      else if (e == 'b' || e == 'f') c = ' ';
      else if (e == 'u') {
        unsigned v = 0;
        for (int i = 0; i < 4 && *q; i++, q++) {
          char h = *q;
          v = v * 16 + (unsigned)(h >= 'a' ? h - 'a' + 10 : h >= 'A' ? h - 'A' + 10 : h - '0');
        }
        char u[4];
        int k = 0;
        if (v >= 0xD800 && v <= 0xDFFF) v = '?';  // surrogate halves: not worth pairing here
        if (v < 0x80) u[k++] = (char)v;
        else if (v < 0x800) { u[k++] = (char)(0xC0 | v >> 6); u[k++] = (char)(0x80 | (v & 63)); }
        else { u[k++] = (char)(0xE0 | v >> 12); u[k++] = (char)(0x80 | ((v >> 6) & 63)); u[k++] = (char)(0x80 | (v & 63)); }
        for (int i = 0; i < k; i++) if (o + 1 < cap) out[o++] = u[i];
        continue;
      } else if (e) c = (unsigned char)e;
      else break;
    }
    if (o + 1 < cap) out[o++] = (char)c;
  }
  out[o] = 0;
  *pos = (size_t)(q - json) + 1;
  return true;
}

// ---------------------------------------------------------------- recognize

bool speech_recognize(speech_provider p, const int16_t *pcm, size_t n, char *out, size_t cap, char *err, size_t errcap) {
  if (p == PROVIDER_LOCAL) {
    // Deterministic: names the turn's duration, so tests can assert on it.
    snprintf(out, cap, "I heard %u milliseconds of speech. Thank you.", (unsigned)(n * 1000 / 16000));
    return true;
  }
  char *b64 = malloc(4 * ((n * 2 + 2) / 3) + 1);
  if (!b64) { seterr(err, errcap, "out of memory"); return false; }
  ws_base64((const uint8_t *)pcm, n * 2, b64);
  buf body = {0}, resp = {0};
  bool ok = buf_str(&body,
                    "{\"config\":{\"explicitDecodingConfig\":{\"encoding\":\"LINEAR16\",\"sampleRateHertz\":16000,"
                    "\"audioChannelCount\":1},\"languageCodes\":[\"en-US\"],\"model\":\"chirp_3\"},\"content\":\"") &&
            buf_str(&body, b64) && buf_str(&body, "\"}");
  free(b64);
  if (!ok) { free(body.p); seterr(err, errcap, "out of memory"); return false; }
  char url[256];
  snprintf(url, sizeof url, "https://us-speech.googleapis.com/v2/projects/%s/locations/us/recognizers/_:recognize",
           g_speech.project);
  long st = post_json(url, body.p, &resp, err, errcap);
  free(body.p);
  if (st != 200) { free(resp.p); return false; }
  out[0] = 0;
  size_t pos = 0, o = 0;
  char t[700];
  while (json_next_string(resp.p, "transcript", &pos, t, sizeof t)) {
    if (strcmp(t, "[BACKGROUND]") == 0) continue;  // chirp_3's "no clear speech" marker
    o += (size_t)snprintf(out + o, o < cap ? cap - o : 0, "%s%s", o ? " " : "", t);
    if (o >= cap) { out[cap - 1] = 0; break; }
  }
  free(resp.p);
  return true;
}

// ---------------------------------------------------------------- synthesize

void speech_render_key(const char *profile, uint32_t revision, const char *text, uint8_t key[32]) {
  // SHA-256 over "<profile>\0<revision as 4 bytes LE><text>": unambiguous framing.
  CC_SHA256_CTX c;
  CC_SHA256_Init(&c);
  CC_SHA256_Update(&c, profile, (CC_LONG)strlen(profile) + 1);
  uint8_t r[4] = {(uint8_t)revision, (uint8_t)(revision >> 8), (uint8_t)(revision >> 16), (uint8_t)(revision >> 24)};
  CC_SHA256_Update(&c, r, 4);
  CC_SHA256_Update(&c, text, (CC_LONG)strlen(text));
  CC_SHA256_Final(key, &c);
}

static int16_t *local_tone(const char *text, uint32_t rate, size_t *n) {
  // A rising chirp, 50 ms per byte of text plus 200 ms, at most 12 s.
  size_t ms = 200 + 50 * strlen(text);
  if (ms > 12000) ms = 12000;
  size_t count = (size_t)rate * ms / 1000;
  int16_t *pcm = malloc(count * 2 + 2);
  if (!pcm) return NULL;
  double phase = 0;
  for (size_t i = 0; i < count; i++) {
    double t = (double)i / (double)count;
    double f = 220.0 + 220.0 * t;
    phase += 2 * M_PI * f / rate;
    double env = fmin(1.0, fmin((double)i, (double)(count - i)) / (rate * 0.01));
    pcm[i] = (int16_t)lrint(9000.0 * env * sin(phase));
  }
  *n = count;
  return pcm;
}

int16_t *speech_synthesize(speech_provider p, const speech_profile *prof, const char *text, uint32_t rate,
                           size_t *n, bool *cached, char *err, size_t errcap) {
  *cached = false;
  if (p == PROVIDER_LOCAL) {
    int16_t *pcm = local_tone(text, rate, n);
    if (!pcm) seterr(err, errcap, "out of memory");
    return pcm;
  }
  uint8_t key[32];
  speech_render_key(prof->id, prof->revision, text, key);
  char hex[65], path[700];
  for (int i = 0; i < 32; i++) snprintf(hex + 2 * i, 3, "%02x", key[i]);
  snprintf(path, sizeof path, "%s/%s-%u.wav", g_speech.cache_dir, hex, rate);
  uint32_t got_rate = 0;
  int16_t *pcm = wav_read(path, &got_rate, n);
  if (pcm && got_rate == rate) { *cached = true; return pcm; }
  free(pcm);

  buf body = {0}, resp = {0};
  char tail[256];
  snprintf(tail, sizeof tail,
           "},\"voice\":{\"languageCode\":\"%s\",\"name\":\"%s\"},\"audioConfig\":{\"audioEncoding\":\"LINEAR16\","
           "\"sampleRateHertz\":%u}}", prof->language, prof->provider_voice, rate);
  bool ok = buf_str(&body, "{\"input\":{\"text\":") && buf_json_string(&body, text) && buf_str(&body, tail);
  if (!ok) { free(body.p); seterr(err, errcap, "out of memory"); return NULL; }
  long st = post_json("https://texttospeech.googleapis.com/v1/text:synthesize", body.p, &resp, err, errcap);
  free(body.p);
  if (st != 200) { free(resp.p); return NULL; }
  const char *a = strstr(resp.p, "\"audioContent\"");
  a = a ? strchr(a + 14, '"') : NULL;
  const char *e = a ? strchr(a + 1, '"') : NULL;
  if (!a || !e) { free(resp.p); seterr(err, errcap, "TTS response has no audioContent"); return NULL; }
  size_t cap = (size_t)(e - a) * 3 / 4 + 4;
  uint8_t *wav = malloc(cap);
  size_t wn = wav ? ws_unbase64(a + 1, (size_t)(e - a - 1), wav, cap) : 0;
  free(resp.p);
  uint32_t r = 0;
  size_t samples = 0;
  const int16_t *d = wn ? wav_parse(wav, wn, &r, &samples) : NULL;
  if (!d || r != rate) { free(wav); seterr(err, errcap, "TTS returned unexpected audio"); return NULL; }
  pcm = malloc(samples * 2 + 2);
  if (pcm) memcpy(pcm, d, samples * 2);
  free(wav);
  if (!pcm) { seterr(err, errcap, "out of memory"); return NULL; }
  *n = samples;
  // Write through a unique temp name and rename, so concurrent sessions never
  // read a half-written render.
  mkdirs(g_speech.cache_dir);
  char tmp[760];
  snprintf(tmp, sizeof tmp, "%s.%d.%lx.tmp", path, getpid(), (unsigned long)(uintptr_t)pthread_self());
  if (wav_write(tmp, pcm, samples, rate)) rename(tmp, path);
  else unlink(tmp);
  return pcm;
}

// ---------------------------------------------------------------- agents

// Who each route is: the one table of per-route agent instructions. A route
// is the Hello's binding (an agent or NPC id), never read from speech. A new
// NPC voice is a new row here.
static const struct { const char *route; const char *instruction; } ROUTES[] = {
    {"foreman",
     "You are the Foreman, an agent standing by the start of the Grid in OpenAgents' Verse, a Metareal "
     "world built in Coax. You are talking out loud with a visitor. Speak briefly: at most 30 words, in "
     "plain spoken English, with no lists, markdown, emoji or code. You have no list of projects, tasks "
     "or plans and no access to anyone's work, so never invent projects, products, people, numbers or "
     "facts. When you don't know something, say so plainly and ask what they'd like to do. Use what was "
     "said earlier in this conversation."},
};

bool speech_route_known(const char *route) {
  for (size_t i = 0; i < sizeof ROUTES / sizeof ROUTES[0]; i++)
    if (strcmp(ROUTES[i].route, route) == 0) return true;
  return false;
}

static const char *route_instruction(const char *route) {
  for (size_t i = 0; i < sizeof ROUTES / sizeof ROUTES[0]; i++)
    if (strcmp(ROUTES[i].route, route) == 0) return ROUTES[i].instruction;
  return NULL;
}

// Cuts a growing reply into sentences with voice-reply.coax's rule and hands
// each one to the caller as soon as it is complete.
typedef struct {
  speech_sentence_fn on_sentence;
  void *ctx;
  const volatile int *cancel;
  char reply[MAX_REPLY + 1];  // everything so far
  size_t n;
  size_t from;                // start of the sentence being built
  size_t scan;                // next byte to look at
  int sentences;
  buf raw;                    // SSE bytes not yet parsed
} splitter;

static void emit(splitter *sp, size_t end) {
  size_t a = sp->from;
  while (a < end && (sp->reply[a] == ' ' || sp->reply[a] == '\n' || sp->reply[a] == '\t' || sp->reply[a] == '\r')) a++;
  size_t b = end;
  while (b > a && (sp->reply[b - 1] == ' ' || sp->reply[b - 1] == '\n' || sp->reply[b - 1] == '\r')) b--;
  sp->from = end;
  if (b <= a) return;
  char sentence[MAX_REPLY + 1];
  memcpy(sentence, sp->reply + a, b - a);
  sentence[b - a] = 0;
  sp->sentences++;
  sp->on_sentence(sp->ctx, sentence);
}

static void split_more(splitter *sp, bool done) {
  while (sp->scan < sp->n) {
    size_t i = sp->scan;
    int next = i + 1 < sp->n ? (unsigned char)sp->reply[i + 1] : (done ? -1 : -2);
    if (next == -2) return;  // decide this byte when the next one is known
    size_t start = sp->from;
    while (start < i && (sp->reply[start] == ' ' || sp->reply[start] == '\n')) start++;
    if (metareal_voice_reply_sentence_end((unsigned char)sp->reply[i], next, (int64_t)(i - start + 1), sp->sentences == 0)) emit(sp, i + 1);
    sp->scan = i + 1;
  }
  if (done && sp->from < sp->n) emit(sp, sp->n);
}

static void split_append(splitter *sp, const char *t) {
  size_t k = strlen(t);
  if (sp->n + k > MAX_REPLY) k = MAX_REPLY - sp->n;
  // Never split a UTF-8 sequence at the cap.
  while (k && sp->n + k == MAX_REPLY && ((unsigned char)t[k] & 0xC0) == 0x80) k--;
  memcpy(sp->reply + sp->n, t, k);
  sp->n += k;
  sp->reply[sp->n] = 0;
  split_more(sp, false);
}

// Server-sent events from streamGenerateContent?alt=sse: each "data: {...}"
// line is one GenerateContentResponse chunk.
static void sse_event(splitter *sp, const char *json) {
  const char *usage = strstr(json, "\"usageMetadata\"");
  const char *cands = strstr(json, "\"candidates\"");
  if (!cands) return;
  size_t pos = (size_t)(cands - json);
  char t[MAX_REPLY + 1];
  while (json_next_string(json, "text", &pos, t, sizeof t)) {
    if (usage && json + pos > usage) break;
    split_append(sp, t);
  }
}

static size_t on_sse(char *d, size_t sz, size_t nm, void *u) {
  splitter *sp = u;
  if (sp->cancel && *sp->cancel) return 0;  // aborts the transfer
  size_t n = sz * nm;
  if (!buf_put(&sp->raw, d, n)) return 0;
  for (;;) {
    char *nl = memchr(sp->raw.p, '\n', sp->raw.n);
    if (!nl) break;
    size_t len = (size_t)(nl - sp->raw.p);
    *nl = 0;
    if (len && sp->raw.p[len - 1] == '\r') sp->raw.p[len - 1] = 0;
    if (strncmp(sp->raw.p, "data:", 5) == 0) sse_event(sp, sp->raw.p + 5);
    memmove(sp->raw.p, nl + 1, sp->raw.n - len - 1);
    sp->raw.n -= len + 1;
    sp->raw.p[sp->raw.n] = 0;
  }
  return n;
}

bool speech_agent_stream(speech_agent a, const char *route, const speech_exchange *history, size_t nhistory,
                         const char *transcript, speech_sentence_fn on_sentence, void *ctx, const volatile int *cancel,
                         char *out, size_t cap, char *err, size_t errcap) {
  splitter *sp = calloc(1, sizeof *sp);
  if (!sp) { seterr(err, errcap, "out of memory"); return false; }
  sp->on_sentence = on_sentence;
  sp->ctx = ctx;
  sp->cancel = cancel;
  bool ok = true;
  if (a == AGENT_ECHO) {
    char r[MAX_REPLY + 1];
    snprintf(r, sizeof r, "You said: %s", transcript);
    split_append(sp, r);
  } else {
    buf body = {0}, resp = {0};
    const char *instruction = route_instruction(route);
    if (!instruction) { seterr(err, errcap, "no agent instruction for this route"); free(sp); return false; }
    ok = buf_str(&body, "{\"systemInstruction\":{\"parts\":[{\"text\":") && buf_json_string(&body, instruction) &&
         buf_str(&body, "}]},\"contents\":[");
    for (size_t i = 0; ok && i < nhistory; i++) {
      ok = buf_str(&body, "{\"role\":\"user\",\"parts\":[{\"text\":") && buf_json_string(&body, history[i].user) &&
           buf_str(&body, "}]},{\"role\":\"model\",\"parts\":[{\"text\":") && buf_json_string(&body, history[i].agent) &&
           buf_str(&body, "}]},");
    }
    ok = ok && buf_str(&body, "{\"role\":\"user\",\"parts\":[{\"text\":") && buf_json_string(&body, transcript) &&
         buf_str(&body, "}]}],\"generationConfig\":{\"maxOutputTokens\":120,\"temperature\":0.7,"
                        "\"thinkingConfig\":{\"thinkingBudget\":0}}}");
    if (!ok) seterr(err, errcap, "out of memory");
    if (ok) {
      char url[320];
      snprintf(url, sizeof url,
               "https://us-central1-aiplatform.googleapis.com/v1/projects/%s/locations/us-central1/publishers/google/"
               "models/%s:streamGenerateContent?alt=sse", g_speech.project, g_speech.model);
      long st = post_json_to(url, body.p, on_sse, sp, &resp, 30L, err, errcap);
      if (st != 200) {
        // A non-200 body went to the SSE parser; show what Google said.
        if (st) seterr(err, errcap, "HTTP %ld from Vertex AI: %.160s", st, sp->raw.p ? sp->raw.p : "");
        ok = false;
      }
    }
    free(body.p);
    free(resp.p);
  }
  if (ok) split_more(sp, true);
  snprintf(out, cap, "%s", sp->reply);
  if (ok && !sp->sentences) { seterr(err, errcap, "the agent returned no text"); ok = false; }
  free(sp->raw.p);
  free(sp);
  return ok;
}
