// The openagents.com API service (api.c): what its route files share.
#pragma once
#include <libpq-fe.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>

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
bool admin_is(const char *pubkey);        // a site admin: a root (OA_ADMIN_PUBKEYS) or a database admin (0100)
bool admin_cookie_who(const request *q, PGconn *db, char who[65], long long *age_ms);   // the oa_admin cookie's admin, until #13207
bool admin_is_root(const char *pubkey);   // a pubkey in OA_ADMIN_PUBKEYS (admins.conf)
void admin_roots(char *out, size_t cap);  // the roots as a Postgres text array literal
int admin_who(const request *q, PGconn *db, char who[65]);   // 0 an admin (cookie or NIP-98), else 401/403
bool chat_route(const request *q);        // chat.c
void chat_key(const request *q, const char *pubkey, bool member);   // KEY for a signed-in admin, or a project member (private.c)
bool player_route(const request *q);      // player.c
bool rooms_route(const request *q, PGconn *db, const char *rest);   // rooms.c: the Everglade instance directory (#12717)
#define ROOM_SEATS 16   // an Everglade room's seats: coax/web/room/room-relay.coax RELAY_SEATS
bool rooms_pick(PGconn *db, const char *realm, const char *asked, const char *pid, const char *via, const char *build, char *code, size_t cap, int *shard, bool *full);
int rooms_invite(PGconn *db, const char *room, const char *pid, long long *until);
bool rooms_endpoint(PGconn *db, int shard, char *out, size_t cap);   // rooms.c: a live VM shard's wss:// endpoint (#13179)
bool npc_route(const request *q);         // npc.c
bool piece_route(const request *q);       // npc.c: the Sandbox's set pieces
bool voice_edit_route(const request *q);  // npc.c: an admin's spoken edit to a realm (eg-voice-edit)
bool private_route(const request *q);     // private.c
bool room_route(const request *q);        // room.c
bool seats_route(const request *q);       // seats.c: seats in running realms (seat-policy.coax)
void seats_sweep(PGconn *db);             // seats.c: closes the seats seat-policy.coax says are idle
bool drops_route(const request *q);       // drops.c
bool drops_token(char *out, size_t cap);   // the bucket's part, shared with villa.c
bool drops_open_upload(const char *object, const char *type, const char *bytes, const char *from, char *out, size_t cap);
void drops_inspect(const char *object, long long *size, char **head_hex);
void drops_delete_object(const char *object);
bool villa_route(const request *q);       // villa.c
bool verse_packs_route(const request *q); // verse_packs.c: private human packs, a key a session
bool realms_route(const request *q);      // realms.c: build your own realm
bool makers_route(const request *q);      // makers.c: a realm's maker and their claim
bool upgrades_route(const request *q);    // makers.c: listed fairy upgrades, bought with sats or credit
bool mesh_route(const request *q);         // mesh.c: authenticated native SystemOne mesh
bool anime_route(const request *q);        // anime.c: typed Anime Lab directions
bool worldbuilder_route(const request *q); // worldbuilder.c: /create-world's chat, the day's $50 cap and anti-flood
bool site_identity(const request *q, PGconn *db, char who[65], bool *signed_req, bool *payload, bool *unseen);
bool inference_route(const request *q);   // inference.c: POST /api/v1/inference, our models for the Mac app
// Our Spark wallet (inference.c, OA_SPARK_URL): POST path with json (GET with none); its status (0 unreached), its body into *out.
long api_spark(const char *path, const char *json, long timeout_s, char **out);
// The model (npc.c): its generateContent answer into *body (malloc'd, or NULL), the HTTP status (0 unreached).
long npc_ask_model(const char *url_env, const char *model, const char *request, char **body);
long npc_ask_model_twice(const char *url_env, const char *model, const char *request, long timeout_ms, char **body);
long npc_ask_model_twice_n(const char *url_env, const char *model, const char *request, long timeout_ms, char **body, size_t *body_n);
// A page, list or file written into the site's bucket (drops.c, OA_SITE_BUCKET), no-cache.
bool drops_site_write(const char *name, const char *type, const char *data);
bool drops_site_delete(const char *name);   // gone already counts as done
char *drops_site_read(const char *name);   // a text object from the site's bucket, malloc'd, or NULL
bool drops_site_write_bytes(const char *name, const char *type, const unsigned char *data, size_t n);
void realms_refresh_pages(void);   // realms.c: on start, made realms' pages written with an older template
bool jobs_route(const request *q);        // jobs.c: POST /api/jobs/<job>, Cloud Scheduler (OIDC) or an admin
bool email_route(const request *q);       // email.c: POST /api/admin/email, through Resend
// Mail through Resend (email.c): 0 sent, 1 sent before under the same key, else an HTTP status with err.
int email_send(PGconn *db, const char *message, char id[128], char *err, size_t errcap);
bool access_route(const request *q);      // access.c: realm invites, the waitlist, auth v2 (flags off by default)
// realms.c, before a realm is planned or made: true when access.c answered 403 invite only (realm_invites on).
// signer: a NIP-98 caller's pubkey (access_signed), or NULL.
bool access_refused(const request *q, PGconn *db, const char *key, const char *signer);
// A NIP-98 caller (access.c, caller-policy.coax): 0 not signed; 1 good, its pubkey in pk; -1 refused and answered 401.
int access_signed(const request *q, PGconn *db, char pk[65]);
// The player key a signer's account acts with on realm routes (auth.caller_key): false on a database error.
bool access_caller_key(PGconn *db, const char *pk, char key[160]);

// Our own authentication (auth.c, docs/auth/): the gate every request passes, and who is asking.
// The rule and verdict numbers are auth-policy.coax's (AP_*), checked against it at start.
enum { AP_PUBLIC = 0, AP_GUEST = 1, AP_ACCOUNT = 2, AP_HUMAN = 3, AP_L2 = 4, AP_L2_FRESH = 5, AP_SCOPE = 6, AP_ADMIN = 7, AP_RECOVER = 8, AP_ADMIN_FRESH = 9 };
enum { AP_NOBODY = 0, AP_KIND_GUEST = 1, AP_PERSON = 2, AP_AGENT = 3, AP_SERVICE = 4 };
enum { AP_OK = 0, AP_LOG_IN = 1, AP_HUMAN_ONLY = 2, AP_STEP_UP = 3, AP_NOT_ADMIN = 4, AP_NO_SCOPE = 5, AP_BAD_RULE = 6, AP_TWO_CREDENTIALS = 7, AP_LIMITED = 8 };
enum { AP_SESSION_GOOD = 0, AP_SESSION_ROTATE = 1, AP_SESSION_ENDED = 2 };
enum { AUTH_NONE = 0, AUTH_NOSTR = 1, AUTH_BEARER = 2, AUTH_COOKIE = 3 };
typedef struct {
  int kind;               // AP_NOBODY .. AP_SERVICE
  int credential;         // AUTH_*
  int level;              // 1 or 2 (0 nobody)
  long long l2_age_ms;    // since the session last proved L2, or -1
  bool admin, team, payload;
  bool limited;           // email alone on an account with a stronger way in: reads and plays only (#13212)
  int strong;             // the account's stronger ways in: keys, passkeys, devices, an unspent recovery code
  int frozen;             // 0, 1 frozen by a freeze, 2 waiting to be deleted (auth-life-policy.coax AL_WHY_*, #13214)
  long long l1_age_ms;    // since this session's email login, or -1 (an email-only account's fresh proof)
  int refused;            // the credential's refusal (400 two, 401 bad), when auth_principal_of gave NULL
  char account[40];       // acct_..., or "" (a key no account holds, a guest, nobody)
  char verse_account[160];   // the game account it plays as (n-<pk>, g-..., e-...)
  char session[32];       // ses_..., or ""
  char pubkey[65];        // the signer, or the session's Nostr key, or ""
  char event_id[65];      // a NIP-98 event's id
  char player[160];       // X-Verse-Player
  char via[16];           // nostr, email, phone, key (an agent's oa_key_), ...
  // An agent's grant (agents.c), filled by agents_admit: what agent-policy.coax decides on.
  char grant[40], owner[40], key_id[16];
  long long scopes, per_day, per_payment;
} auth_principal;
void auth_init(void);                     // checks the AP_* numbers against auth-policy.coax
void auth_reset(void);                    // start of each request
bool auth_gate(const request *q);         // true when it answered a refusal (no rule: 404)
const auth_principal *auth_principal_of(const request *q);   // NULL: the credential was refused
const char *auth_extra_headers(void);     // a rotated or cleared session cookie, for this answer
bool auth_api_route(const request *q);    // /api/auth/{me,limits,email/*,nostr*,phone/*,refresh,logout*,sessions*,freeze}
void auth_print_rules(FILE *out);         // api --rules
void auth_print_limits(FILE *out);        // api --limits
const char *auth_rule_of(const char *method, const char *path);   // api --rule METHOD PATH
bool auth_login_key_redirect(const request *q, PGconn *db, const char *pk, const char *location);
bool auth_freeze_link(PGconn *db, const char *acct, char token[65]);
// For agents.c: auth.c's answers, audit rows, tokens, addresses and limits.
void auth_answer(const request *q, int status, const char *headers, const char *json, const char *lines);
void auth_refuse(const request *q, int status, const char *words);
void auth_audit(PGconn *db, const char *acct, const char *actor, const char *action, const char *result, const char *detail);
bool auth_new_key(char id[9], char *out, size_t cap, char digest[65]);   // a new oa_key_, its id and digest
void auth_visitor(const request *q, char ip[128]);
bool auth_limited(const request *q, PGconn *db);             // true when it refused (agent_ask)
bool auth_same_site(const request *q);
const char *auth_origin(void);

// Agents, their grants and keys, and `openagents login` (agents.c, agent-policy.coax, #13211).
bool agents_init(void);                                      // its numbers against agent-policy.coax
bool agents_key(PGconn *db, const char *digest, auth_principal *p);   // an oa_key_'s agent, or false
int agents_admit(const request *q, PGconn *db, auth_principal *p, char words[200]);   // 0, or the refusal's status
bool agents_scoped(const auth_principal *p, int scope);     // the grant holds this scope
void agents_used(const request *q, PGconn *db, const auth_principal *p);   // the audit row of an admitted request
int agents_pause_all(PGconn *db, const char *owner);        // logout everywhere with agents: how many paused
char *agents_export_b64(PGconn *db, const char *owner);    // the owner's agents as a base64 AgentList (malloc'd), or NULL
bool agents_route(const request *q);                        // /api/agents*, /api/agent/*, /api/agent-keys/*, /api/auth/device/*

int auth_notice(PGconn *db, const char *acct, const char *key, const char *subject, const char *text);   // emails every verified address
bool lnaddr_route(const request *q);       // /.well-known/lnurlp/*, /lnurlp/*, /api/lightning/address* (lnaddr.c)
void lnaddr_print_limits(FILE *out);      // api --limits: the Lightning Address section of /docs/limits
void lnaddr_init(void);                   // checks lnaddr.c's numbers against the Coax
