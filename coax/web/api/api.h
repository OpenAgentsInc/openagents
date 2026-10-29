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
PGconn *api_db_new(void);   // a connection of its own (a child process's)
// A header's value (first one), or NULL with out "".
const char *api_header(const request *q, const char *name, char *out, size_t cap);
// A query parameter, percent-decoded, or NULL with out "".
const char *api_query(const request *q, const char *name, char *out, size_t cap);

// Route files: true when they answered.
bool analytics_route(const request *q);   // analytics.c
void analytics_team_init(PGconn *db);     // the team (OA_TEAM, OA_ADMIN_PUBKEYS) into analytics.team, and its backfill
#define TEAM_COOKIE "Set-Cookie: oa_team=1; Path=/; Max-Age=63072000; HttpOnly; Secure; SameSite=Lax\r\n"   // this browser is the team's
void admin_init(void);                    // admin.c
bool admin_route(const request *q);
bool admin_is(const char *pubkey);        // a pubkey in OA_ADMIN_PUBKEYS
int admin_who(const request *q, PGconn *db, char who[65]);   // 0 an admin (cookie or NIP-98), else 401/403
bool chat_route(const request *q);        // chat.c
void chat_key(const request *q, const char *pubkey, bool member);   // KEY for a signed-in admin, or a project member (private.c)
bool player_route(const request *q);      // player.c
bool npc_route(const request *q);         // npc.c
bool piece_route(const request *q);       // npc.c: the Sandbox's set pieces
bool voice_edit_route(const request *q);  // npc.c: an admin's spoken edit to a realm (eg-voice-edit)
bool private_route(const request *q);     // private.c
bool room_route(const request *q);        // room.c
bool drops_route(const request *q);       // drops.c
bool drops_token(char *out, size_t cap);   // the bucket's part, shared with villa.c
bool drops_open_upload(const char *object, const char *type, const char *bytes, const char *from, char *out, size_t cap);
void drops_inspect(const char *object, long long *size, char **head_hex);
void drops_delete_object(const char *object);
bool villa_route(const request *q);       // villa.c
bool realms_route(const request *q);      // realms.c: build your own realm
// The model (npc.c): its generateContent answer into *body (malloc'd, or NULL), the HTTP status (0 unreached).
long npc_ask_model(const char *url_env, const char *model, const char *request, char **body);
// A page, list or file written into the site's bucket (drops.c, OA_SITE_BUCKET), no-cache.
bool drops_site_write(const char *name, const char *type, const char *data);
