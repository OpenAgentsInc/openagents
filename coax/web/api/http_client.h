// Coax Web unified bounded outbound HTTP client with test fakes (Issue #14078).
#pragma once
#include <stdbool.h>
#include <stddef.h>

typedef struct {
  const char *method;       // "GET" or "POST"
  const char *url;
  const char *body;         // optional (POST)
  size_t body_len;
  const char *const *headers; // NULL-terminated array of "Key: Value" strings (optional)
  long connect_timeout_ms;  // >= 100 (default 5000)
  long total_timeout_ms;    // >= connect_timeout_ms (default 20000)
  size_t max_response_bytes; // default 65536
  bool allow_http_local;
} http_client_req;

typedef struct {
  long status;              // HTTP status code, or 0 on transport/policy refusal
  char *body;               // malloc'd NUL-terminated body (caller frees with http_client_res_free)
  size_t body_len;
  bool blocked_stray;       // true when prevent_stray_requests blocked an unfaked URL
} http_client_res;

void http_client_send(const http_client_req *req, http_client_res *res);
void http_client_res_free(http_client_res *res);

// Test-only in-process fakes (mirrors Laravel Http::fake() & Http::preventStrayRequests())
void http_fake_reset(void);
void http_fake_register(const char *url_prefix, long status, const char *body);
void http_prevent_stray_requests(bool enable);
