// The openagents.com API service (api.c): what its route files share.
#pragma once
#include <libpq-fe.h>
#include <stdbool.h>
#include <stddef.h>

typedef struct {
  int fd;
  char method[8];
  char target[2048];   // the path and query as sent
  char path[2048];     // without the query
  const char *query;   // after '?', or ""
  const char *head;    // the request line and headers ("\r\n" separated)
  const char *body;    // Content-Length bytes (not terminated)
  size_t body_len;
} request;

double api_now_ms(void);
void api_log(const char *fmt, ...);
void api_respond(int fd, int status, const char *type, const char *extra_headers, const void *body, size_t n);
void api_json(int fd, int status, const char *json);
PGconn *api_db(void);
// A header's value (first one), or NULL with out "".
const char *api_header(const request *q, const char *name, char *out, size_t cap);
// A query parameter, percent-decoded, or NULL with out "".
const char *api_query(const request *q, const char *name, char *out, size_t cap);

// Route files: true when they answered.
bool analytics_route(const request *q);   // analytics.c
void admin_init(void);                    // admin.c
bool admin_route(const request *q);
