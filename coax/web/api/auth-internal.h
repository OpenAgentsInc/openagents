// What auth.c shares with auth-recover.c (step-up, linking and recovery, #13212, #13213), and
// back. Not for other files: they ask auth_principal_of and auth_gate (api.h).
#pragma once
#include <libpq-fe.h>
#include <stdbool.h>
#include <stddef.h>
#include "api.h"

enum { AUTH_LIMIT_KEY_LOGIN = 4, AUTH_LIMIT_RECOVERY = 8, AUTH_LIMIT_RECOVERY_CODE = 9, AUTH_LIMIT_RECOVERY_CODE_IP = 10 };

// ---- auth.c
bool auth_random(unsigned char *b, size_t n);
bool auth_random_hex(char *out, size_t bytes);
void auth_hex(const unsigned char *b, size_t n, char *out);
bool auth_unhex(const char *s, unsigned char *out, size_t n);
void auth_sha_hex(const char *s, char out[65]);
const char *auth_json_str(const char *body, size_t n, const char *key, char *out, size_t cap);
void auth_json_esc(const char *s, char *out, size_t cap);
bool auth_limit_hit(const request *q, PGconn *db, int which, const char *key);   // false: it answered the 429
void auth_visitor_key(const request *q, char out[65]);
bool auth_lookups(const char *email, char *all, size_t cap, char newest[80]);
bool auth_unseal(const char *ad, const char *sealed, char *out, size_t cap);
bool auth_normal_email(const char *raw, char *out, size_t cap);
void auth_mask_email(const char *email, char *out, size_t cap);
void auth_logged_in(const request *q, PGconn *db, const char *acct, const char *via_kind, const char *via_id, const char *level,
                    bool app, bool made, const char *action, const char *extra_json, const char *extra_lines);

// ---- auth-recover.c
// auth-recover-policy.coax's numbers (AR_*), checked against it at start (auth_recover_init).
enum { AR_NONE = 0, AR_PENDING = 1, AR_READY = 2, AR_CANCELLED = 3, AR_DONE = 4 };
enum { AR_OWNER_NONE = 0, AR_OWNER_THIS = 1, AR_OWNER_OTHER = 2 };
enum { AR_OK = 0, AR_STEP_UP = 1, AR_OTHER = 2, AR_ALREADY = 3, AR_LAST_WAY = 4, AR_NOT_NEEDED = 5,
       AR_NOTHING = 6, AR_WAITING = 7, AR_NOT_EMAIL = 8, AR_NO_REQUEST = 9, AR_ASKED = 10 };
bool auth_recover_init(void);
// Answers a route under /api/auth/ that is step-up, linking or recovery; false when op isn't one.
bool auth_recover_route(const request *q, PGconn *db, const char *op);
// The gate's refusal when a step-up would let it: the level wanted (2 L2, 3 L2-fresh), the
// account's methods, whether this session is limited and its recovery's state.
void auth_step_up_refusal(const request *q, PGconn *db, const auth_principal *p, int need, int status, const char *words);
// A login or step-up at L2: a pending recovery for the account ends, and every channel is told.
void auth_recover_strong_login(PGconn *db, const char *acct, const char *sid);
// An email was linked (auth.c's email_finish): every channel is told.
void auth_recover_linked_email(PGconn *db, const char *acct, const char *sid);
// The step a limited session may not take with email alone: a link of an email on an account
// with a stronger way in. false: it answered the refusal.
bool auth_recover_may_link(const request *q, PGconn *db, const auth_principal *p);
// For auth-life.c: the key that signed this request's proof for purpose (false: it answered the
// refusal), and a notice to every email and key of acct.
bool auth_proof_key(const request *q, PGconn *db, const auth_principal *p, const char *purpose, const char *body, char pk[65]);
void auth_notice(PGconn *db, const char *acct, const char *what, const char *subject, const char *words);
void auth_notice_link(PGconn *db, const char *acct, const char *what, const char *subject, const char *words, const char *lead, const char *url);

// ---- auth-life.c (freeze, merge, export and deletion, #13214)
bool auth_life_init(void);
// Answers /api/auth/{life,unfreeze,delete,delete/cancel,export,merge/*}; false when op isn't one.
bool auth_life_route(const request *q, PGconn *db, const char *op);
// The gate's check of a frozen account's request: true when it answered the refusal.
bool auth_life_frozen_refused(const request *q, const auth_principal *p);
// After a freeze (auth.c): its agents pause and every way in is told.
void auth_life_froze(PGconn *db, const char *acct, bool by_link);
// email_finish for purpose "merge": the account that holds the email proved itself to survivor's ask.
void auth_life_merge_email(const request *q, PGconn *db, const char *survivor, const char *lookup);

// ---- auth.c's small pieces other auth files share
void auth_cookie(const request *q, const char *name, char *out, size_t cap);
void auth_device_name(const request *q, char *out, size_t cap);
// ---- passkeys.c (#13215)
bool passkeys_init(void);
bool passkeys_route(const request *q, PGconn *db, const char *op);
// The account's passkeys appended to an answer (JSON objects, and "passkey ..." lines): the count.
int passkeys_rows(PGconn *db, const char *acct, char *json, size_t jcap, size_t *j, char *lines, size_t lcap, size_t *l);

// ---- auth-device.c (device keys and device-bound refresh, #13216)
bool auth_device_init(void);
// A login's device key, read from its headers before the session is made. bind: bind the new
// session to key. false: it answered the refusal (a key we can't hold is never quietly dropped).
typedef struct { bool bind; unsigned char key[65]; char platform[12], store[12], name[96]; } auth_device_ask;
bool auth_device_login(const request *q, PGconn *db, const char *acct, bool app, auth_device_ask *ask);
// Binds the new session to the key, for good. false: it didn't bind, and the login must not go on.
bool auth_device_bind(PGconn *db, const char *acct, const char *sid, const auth_device_ask *ask);
// What a refresh token is bound to (auth.refresh_look). false: no such token.
typedef struct { bool found, bound, revoked; char family[40], session[32], account[40], device[24], key_hex[131]; long long now_ms; } auth_refresh_facts;
bool auth_device_look(PGconn *db, const char *rt_sha, auth_refresh_facts *f);
// The refresh's OA-Device-Proof against the policy. true: go on, with proved the device's id when
// a proof was checked ("" for an unbound login). false: it answered and audited the refusal.
bool auth_device_refresh(const request *q, PGconn *db, const char *rt_sha, const auth_refresh_facts *f, char proved[24]);
// The database's own refusal of a bound token with no proved device (never reached through the above).
void auth_device_refuse_unproved(const request *q);
// Answers /api/auth/devices and /api/auth/devices/<id>/remove; false when op isn't one.
bool auth_device_route(const request *q, PGconn *db, const char *op);

// ---- wallet.c (the encrypted seed backup and the daily limit, #13217)
bool wallet_init(void);
