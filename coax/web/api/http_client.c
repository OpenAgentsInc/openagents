// Coax Web unified bounded outbound HTTP client with test fakes (Issue #14078).
#include "http_client.h"
#include "http-client-policy.generated.h"

#include <curl/curl.h>
#include <stdlib.h>
#include <string.h>

#define HTTP_FAKES_MAX 32

typedef struct {
  char prefix[256];
  long status;
  char *body;
} http_fake_entry;

static http_fake_entry g_fakes[HTTP_FAKES_MAX];
static int g_fake_count;
static bool g_prevent_stray;

void http_fake_reset(void) {
  for (int i = 0; i < g_fake_count; i++) {
    free(g_fakes[i].body);
    g_fakes[i].body = NULL;
  }
  g_fake_count = 0;
  g_prevent_stray = false;
}

void http_fake_register(const char *url_prefix, long status, const char *body) {
  if (!url_prefix || g_fake_count >= HTTP_FAKES_MAX) return;
  http_fake_entry *e = &g_fakes[g_fake_count++];
  snprintf(e->prefix, sizeof e->prefix, "%s", url_prefix);
  e->status = status;
  e->body = strdup(body ? body : "");
}

void http_prevent_stray_requests(bool enable) {
  g_prevent_stray = enable;
}

typedef struct {
  char *data;
  size_t n;
  size_t max_bytes;
} http_buf;

static size_t http_write_cb(char *p, size_t size, size_t count, void *u) {
  http_buf *b = (http_buf *)u;
  size_t k = size * count;
  if (b->n + k > b->max_bytes) return 0;
  char *g = (char *)realloc(b->data, b->n + k + 1);
  if (!g) return 0;
  b->data = g;
  memcpy(b->data + b->n, p, k);
  b->n += k;
  b->data[b->n] = 0;
  return k;
}

void http_client_send(const http_client_req *req, http_client_res *res) {
  memset(res, 0, sizeof *res);
  if (!req || !req->url || !*req->url) return;

  // Check in-process fakes first
  for (int i = 0; i < g_fake_count; i++) {
    size_t plen = strlen(g_fakes[i].prefix);
    if (!strncmp(req->url, g_fakes[i].prefix, plen)) {
      res->status = g_fakes[i].status;
      res->body = strdup(g_fakes[i].body ? g_fakes[i].body : "");
      res->body_len = res->body ? strlen(res->body) : 0;
      return;
    }
  }
  if (g_prevent_stray) {
    res->blocked_stray = true;
    return;
  }

  long connect_ms = req->connect_timeout_ms > 0 ? req->connect_timeout_ms : 5000;
  long total_ms = req->total_timeout_ms > 0 ? req->total_timeout_ms : 20000;
  size_t max_bytes = req->max_response_bytes > 0 ? req->max_response_bytes : 65536;

  coax_string url_cs = {req->url, (int64_t)strlen(req->url)};
  if (!hcp_url_allowed(url_cs, req->allow_http_local)) return;
  if (!hcp_timeout_valid(connect_ms, total_ms, (int64_t)max_bytes)) return;

  CURL *h = curl_easy_init();
  if (!h) return;

  struct curl_slist *hdrs = NULL;
  if (req->headers) {
    for (size_t i = 0; req->headers[i]; i++) hdrs = curl_slist_append(hdrs, req->headers[i]);
  }
  http_buf b = {NULL, 0, max_bytes};
  curl_easy_setopt(h, CURLOPT_URL, req->url);
  curl_easy_setopt(h, CURLOPT_FOLLOWLOCATION, 0L);
  if (hdrs) curl_easy_setopt(h, CURLOPT_HTTPHEADER, hdrs);
  if (req->method && !strcmp(req->method, "POST")) {
    curl_easy_setopt(h, CURLOPT_POST, 1L);
    curl_easy_setopt(h, CURLOPT_POSTFIELDS, req->body ? req->body : "");
    curl_easy_setopt(h, CURLOPT_POSTFIELDSIZE, (long)(req->body ? (req->body_len ? req->body_len : strlen(req->body)) : 0));
  } else if (req->method && !strcmp(req->method, "PUT")) {
    curl_easy_setopt(h, CURLOPT_CUSTOMREQUEST, "PUT");
    curl_easy_setopt(h, CURLOPT_POSTFIELDS, req->body ? req->body : "");
    curl_easy_setopt(h, CURLOPT_POSTFIELDSIZE, (long)(req->body ? (req->body_len ? req->body_len : strlen(req->body)) : 0));
  } else if (req->method && !strcmp(req->method, "DELETE")) {
    curl_easy_setopt(h, CURLOPT_CUSTOMREQUEST, "DELETE");
  }
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, http_write_cb);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, &b);
  curl_easy_setopt(h, CURLOPT_CONNECTTIMEOUT_MS, connect_ms);
  curl_easy_setopt(h, CURLOPT_TIMEOUT_MS, total_ms);
  curl_easy_setopt(h, CURLOPT_NOSIGNAL, 1L);

  long status = 0;
  if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  curl_slist_free_all(hdrs);
  curl_easy_cleanup(h);

  res->status = status;
  res->body = b.data;
  res->body_len = b.n;
}

void http_client_res_free(http_client_res *res) {
  if (!res) return;
  free(res->body);
  res->body = NULL;
  res->body_len = 0;
}
