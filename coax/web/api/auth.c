// Our own authentication (docs/auth/, #13200 to #13205): who is asking, whether this route lets
// them, and logging in with an email code or a Nostr key. Accounts, authenticators, sessions, the
// audit log and the rate limits' counts live in our Postgres (migrations 0142 to 0146); no identity
// provider holds a copy. The decisions are auth-policy.coax (#authority); sealing, lookups and
// the token codec are Coax too (auth-seal.coax, coax/lib/crypto/token.coax), compiled to C.
//
// Every request first passes auth_gate: the route's rule from RULES (public, guest, account,
// human, L2, L2-fresh, scope, admin), and a route without a rule is 404. `api --rules` prints the
// table; test.sh checks every route against it.
//
// Who (auth_principal): one credential per request, never two:
//   Authorization: Nostr <NIP-98>     the signer, at L2 (an agent's key is its agent account)
//   Authorization: Bearer oa_at_...   an app's access token (1 hour)
//   Authorization: Bearer oa_key_...  an agent's API key (agents.c; its SHA-256 only is kept)
//   Cookie __Host-oa_session=oas_...  a web session (30 days idle, 180 absolute, new token daily)
//   X-Verse-Player                    a guest's player key: never an account
//
// Routes (JSON; ?as=lines answers the same as "name value" lines for the Coax pages):
//   GET  /api/auth/account                who is logged in: account, kind, level, emails (masked), keys
//   GET  /api/auth/viewer                 who is logged in as every page reads it (coax/web/host.js): K|npub,
//                                         J|slug..., a blank line, H|hex key, M|member..., a blank line, A|1
//                                         an admin; U|account for an account with no key it acts as; nobody
//                                         is a 200 with an empty body. GET /api/auth/me: the same, nobody a 401
//   GET  /api/auth/limits                 every limit (the table /docs/limits is made from)
//   POST /api/auth/email/start {email, client?, link?}   one message with a code and a link; sets
//                                         __Host-oa_pending (an app gets {pending} instead)
//   POST /api/auth/email/verify {code, pending?}         the code, in the browser that asked -> logged in
//   POST /api/auth/email/link {t, pending?}              the link's token: logged in in the browser that
//                                         asked; elsewhere 409 {code} to type where you asked
//   POST /api/auth/nostr/challenge {client?}             a challenge to sign over (5 minutes)
//   POST /api/auth/nostr {challenge}      Authorization: Nostr over this route and body -> logged in
//   POST /api/auth/phone/start            the phone QR's challenge and the page's poll token
//   POST /api/auth/phone/approve {challenge}   the app, NIP-98 signed by its account's key
//   POST /api/auth/phone/poll {poll}      waiting | expired | logged in
//   POST /api/auth/refresh {refresh_token}     an app's next pair; a reused one ends the family; a login
//                                         bound to a device key needs OA-Device-Proof (auth-device.c, #13216)
//   POST /api/auth/logout                 this session (none: the cookie is cleared all the same)
//   POST /api/auth/logout-everywhere      every session of the account (L2)
//   GET  /api/auth/sessions               the devices list; POST /api/auth/sessions/<id>/remove
//   POST /api/auth/freeze {t?}            "This wasn't me": a notice's link token, or the session's own account
//
//   OA_AUTH_SEAL_KEY        "1:<64 hex>[,2:<64 hex>...]": the newest seals, every one opens (Secret Manager oa-auth-seal-key)
//   OA_AUTH_LOOKUP_PEPPER   "1:<64 hex>[,...]": the email lookup's HMAC key (oa-auth-lookup-pepper)
//   OA_API_ORIGIN           this site, for NIP-98 URLs and the email link
//   OA_EMAIL_ALLOW          (email.c) staging's allowlist of addresses mail may go to
//
// An agent (agents.c, agent-policy.coax, #13211) is refused on every human, L2, L2-fresh and admin
// route, and passes no request at all unless its grant is active now, from an address it allows;
// scope routes need the grant's scope. Every admitted agent request is in the audit log.
//
// Nothing secret is ever logged: no token, code, cookie, email or key material. Refusals log the
// route, the rule and the principal's kind.
#include "api.h"
#include "nostr.h"
#include "auth-policy.generated.h"
#include "auth-seal.generated.h"
#include "auth-internal.h"
#include "auth-wire.generated.h"

#include <ctype.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#if defined(__APPLE__)
#include <sys/random.h>
#endif

static const char *env_or(const char *name, const char *fallback) { const char *v = getenv(name); return v && *v ? v : fallback; }
static const char *origin(void) { return env_or("OA_API_ORIGIN", "https://openagents.com"); }

// ---------------------------------------------------------------- the rule table
typedef struct { const char *method, *path; bool prefix; int rule; const char *note; int scope; } auth_rule;
#define R(m, p, pre, rule, note) {m, p, pre, rule, note, 0}
#define S(m, p, pre, scope, note) {m, p, pre, AP_SCOPE, note, scope}   // an agent whose grant has this scope (AG_* bit number)
// First match wins: specific paths before their families. "*" is any method.
static const auth_rule RULES[] = {
  R("GET",  "/api/playtest/health", false, AP_PUBLIC, "a database round trip"),
  R("GET",  "/api/playtest/old-link", false, AP_PUBLIC, "the old playtest's links move"),
  // ---- our own authentication (this file)
  R("GET",  "/api/auth/account", false, AP_PUBLIC, "who is logged in, or nobody"),
  R("GET",  "/api/auth/limits", false, AP_PUBLIC, "every limit"),
  R("GET",  "/api/auth/viewer", false, AP_PUBLIC, "who is logged in, as every page reads it"),
  R("GET",  "/api/auth/me", false, AP_PUBLIC, "the viewer's lines; nobody is a 401"),
  R("POST", "/api/auth/email/start", false, AP_PUBLIC, "log in (or link an email when logged in)"),
  R("POST", "/api/auth/email/verify", false, AP_PUBLIC, "the code, in the browser that asked"),
  R("POST", "/api/auth/email/link", false, AP_PUBLIC, "the link, in the browser that asked"),
  R("POST", "/api/auth/nostr/challenge", false, AP_PUBLIC, "a challenge to sign"),
  R("POST", "/api/auth/nostr", false, AP_PUBLIC, "a signed challenge logs in"),
  R("POST", "/api/auth/phone/start", false, AP_PUBLIC, "the phone QR"),
  R("POST", "/api/auth/phone/approve", false, AP_HUMAN, "the app approves with its account's key"),
  R("POST", "/api/auth/phone/poll", false, AP_PUBLIC, "the page that showed the QR"),
  R("POST", "/api/auth/refresh", false, AP_PUBLIC, "an app's refresh token"),
  R("POST", "/api/auth/logout", false, AP_PUBLIC, "ends this session, if any"),
  R("POST", "/api/auth/logout-everywhere", false, AP_L2, "ends every session of the account"),
  R("GET",  "/api/auth/sessions", false, AP_ACCOUNT, "the devices list"),
  R("POST", "/api/auth/sessions/", true, AP_ACCOUNT, "remove one device's session"),
  R("POST", "/api/auth/freeze", false, AP_PUBLIC, "a notice's link token, or the session's account"),
  // ---- device keys (auth-device.c, auth-device-policy.coax, #13216)
  R("GET",  "/api/auth/devices", false, AP_HUMAN, "the device keys that hold your logins"),
  R("POST", "/api/auth/devices/", true, AP_HUMAN, "remove one device (its key is revoked and its logins end); push {service, token} or push/off: security notices to this login's device"),
  // ---- the wallet's encrypted backup and daily limit (wallet.c, pay/wallet-policy.coax, #13217):
  // reading, storing and deleting the backup and raising the limit are L2-fresh here and in the
  // policy; a payment past the limit is L2-fresh in the policy; the cancel link needs no login
  R("GET",  "/api/wallet", false, AP_HUMAN, "the daily limit, what the last 24 hours counted, a waiting raise, the backup's public facts"),
  R("POST", "/api/wallet/backup/export", false, AP_L2_FRESH, "the encrypted backup itself: ciphertext we can't open (L2-fresh)"),
  R("POST", "/api/wallet/backup", false, AP_L2_FRESH, "store an encrypted backup at the next epoch (L2-fresh)"),
  R("POST", "/api/wallet/backup/delete", false, AP_L2_FRESH, "delete the backup (L2-fresh)"),
  R("POST", "/api/wallet/limit", false, AP_L2_FRESH, "set the daily limit: a raise counts in 24 hours (L2-fresh)"),
  R("POST", "/api/wallet/limit/lower", false, AP_HUMAN, "lower the daily limit: it counts at once"),
  R("POST", "/api/wallet/limit/cancel", false, AP_PUBLIC, "cancel a waiting raise: a notice's link token, or the session's account"),
  R("POST", "/api/wallet/spend", false, AP_HUMAN, "the app asks before it pays: go, or a fresh L2 first"),
  R("POST", "/api/wallet/spend/void", false, AP_HUMAN, "a payment that didn't go no longer counts"),
  // ---- an account's life (auth-life.c, auth-life-policy.coax, #13214): its own proof rules inside
  R("GET",  "/api/auth/life", false, AP_RECOVER, "frozen or waiting to be deleted, until when, and what the account holds"),
  R("POST", "/api/auth/unfreeze", false, AP_RECOVER, "lift a freeze (L2-fresh, or a fresh email code with nothing stronger)"),
  R("POST", "/api/auth/delete/cancel", false, AP_RECOVER, "cancel a deletion from any login, until the purge"),
  R("GET",  "/api/auth/export", false, AP_RECOVER, "everything we hold about you, JSON lines (L2, or any login with nothing stronger)"),
  R("POST", "/api/auth/merge/start", false, AP_RECOVER, "merge another account into this one (L2-fresh, or a fresh email code)"),
  R("POST", "/api/auth/merge/prove", false, AP_RECOVER, "the other account's key signs the merge's challenge"),
  R("POST", "/api/auth/merge/confirm", false, AP_RECOVER, "both accounts proved: they become one; credit moves only when confirmed"),
  // ---- `openagents login` (agents.c): the CLI's new key asks, a person approves it
  R("POST", "/api/auth/device/start", false, AP_PUBLIC, "the CLI's new key, NIP-98 signed, checked there"),
  R("POST", "/api/auth/device/look", false, AP_PUBLIC, "what /login/device shows: the computer's name"),
  R("POST", "/api/auth/device/approve", false, AP_HUMAN, "a person links the CLI's key (a strong login when the account has one)"),
  R("GET",  "/api/auth/device/", true, AP_PUBLIC, "the CLI's key polls its request, NIP-98 signed, checked there"),
  // ---- agents and their grants (agents.c, agent-policy.coax): people manage, agents use
  R("GET",  "/api/agents", false, AP_HUMAN, "your agents and their grants"),
  R("POST", "/api/agents", false, AP_L2, "a new agent and its grant (spending: L2 in the last five minutes)"),
  R("POST", "/api/agents/request", false, AP_PUBLIC, "an agent's new key asks its owner, NIP-98 signed, checked there"),
  R("POST", "/api/agents/request/poll", false, AP_PUBLIC, "that key asks how its request went, NIP-98 signed, checked there"),
  R("GET",  "/api/agents/request/", true, AP_HUMAN, "what an agent asked for, by its code"),
  R("POST", "/api/agents/approve/", true, AP_L2, "approve an agent's request (spending: L2 in the last five minutes)"),
  R("POST", "/api/agents/deny/", true, AP_HUMAN, "deny an agent's request"),
  R("POST", "/api/agents/revoke-all", false, AP_HUMAN, "end every agent's grant now"),
  R("POST", "/api/agents/revoke/", true, AP_HUMAN, "end one agent's grant now"),
  R("POST", "/api/agents/rotate/", true, AP_L2, "a new API key for an agent; its old ones end"),
  R("POST", "/api/agents/grant/", true, AP_L2, "change a grant (more than before: L2 in the last five minutes)"),
  R("POST", "/api/agents/resume/", true, AP_L2, "resume an agent paused by logging out everywhere"),
  R("GET",  "/api/agent-keys/", true, AP_PUBLIC, "an agent key's NIP-OA owner attestation, published"),
  R("GET",  "/api/agent/me", false, AP_ACCOUNT, "an agent's own grant (agents only)"),
  S("GET",  "/api/agent/owner", false, 1, "the owner's name (scope profile:read)"),
  S("POST", "/api/agent/spend", false, 2, "a payment against the spend caps (scope wallet:spend)"),
  // ---- step-up, linking and recovery (auth-recover.c, #13212, #13213)
  R("GET",  "/api/auth/ways", false, AP_ACCOUNT, "the account's ways in, its recovery and what a step-up offers"),
  R("GET",  "/api/auth/notices", false, AP_ACCOUNT, "the security notices the account was sent"),
  R("POST", "/api/auth/proof/challenge", false, AP_RECOVER, "a challenge for a key to sign while logged in"),
  R("POST", "/api/auth/step-up", false, AP_RECOVER, "a key's signature or the recovery code: this session to L2"),
  R("POST", "/api/auth/link/nostr", false, AP_HUMAN, "add a Nostr key (L2, or email alone on an account with nothing stronger)"),
  R("POST", "/api/auth/unlink", false, AP_L2_FRESH, "remove a way in; never the last; its sessions end"),
  R("GET",  "/api/auth/recovery", false, AP_RECOVER, "the newest recovery request: pending, ready, cancelled or done"),
  R("POST", "/api/auth/recovery/start", false, AP_RECOVER, "ask for recovery: 72 hours, every channel told"),
  R("POST", "/api/auth/recovery/cancel", false, AP_PUBLIC, "a notice's cancel link, or any session or key of the account"),
  R("POST", "/api/auth/recovery/finish", false, AP_RECOVER, "after 72 hours: a new key replaces the lost ones"),
  R("POST", "/api/auth/recovery-code", false, AP_L2_FRESH, "a new recovery code, shown once"),
  R("POST", "/api/auth/recovery-code/login", false, AP_PUBLIC, "log in with an email or key and the recovery code"),
  // ---- passkeys (passkeys.c, #13215); removing one is /api/auth/unlink
  R("POST", "/api/auth/passkey/login/options", false, AP_PUBLIC, "a passkey login challenge: 5 minutes, once, this browser or app"),
  R("POST", "/api/auth/passkey/login", false, AP_PUBLIC, "log in with a passkey: a session at L2"),
  R("POST", "/api/auth/passkey/register/options", false, AP_HUMAN, "a challenge to add a passkey to this session's account"),
  R("POST", "/api/auth/passkey/register", false, AP_HUMAN, "add a passkey (L2, or email alone on an account with nothing stronger)"),
  R("POST", "/api/auth/passkey/step-up/options", false, AP_RECOVER, "a challenge for this account's passkeys"),
  R("POST", "/api/auth/passkey/step-up", false, AP_RECOVER, "a passkey: this session to L2"),
  R("GET",  "/api/auth/passkeys", false, AP_ACCOUNT, "the account's passkeys and the one offer after an email login"),
  R("POST", "/api/auth/passkeys/", true, AP_HUMAN, "rename a passkey; mark the offer shown"),
  // ---- reserved: these routes are built in other issues and must keep L2-fresh (#13212)
  R("POST", "/api/auth/delete", false, AP_RECOVER, "delete the account: frozen now, purged in 30 days (L2-fresh, or a fresh email code; auth-life-policy.coax)"),
  R("POST", "/api/lnaddr/destination", false, AP_L2_FRESH, "change a Lightning Address's destination (#13209)"),
  // ---- admins (admin.c, email.c, makers.c' and upgrades' admin pages)
  R("GET",  "/api/admin/login", false, AP_PUBLIC, "an admin's NIP-98 signature logs in"),
  R("*",    "/api/admin/logout", false, AP_PUBLIC, "ends the session"),
  R("POST", "/api/admin/", true, AP_ADMIN_FRESH, "an admin's change: L2 within five minutes"),
  R("*",    "/api/admin/", true, AP_ADMIN, "site admins (admins.conf and database admins)"),
  // ---- /chat for admins, its runner, and the rooms' chat under the same root (chat.c, room.c)
  R("*",    "/api/chat/runner/", true, AP_PUBLIC, "the chat runner's key, checked there"),
  R("*",    "/api/chat/state", false, AP_ADMIN, "/chat"), R("*", "/api/chat/new", false, AP_ADMIN, "/chat"),
  R("*",    "/api/chat/send", false, AP_ADMIN, "/chat"), R("*", "/api/chat/model", false, AP_ADMIN, "/chat"),
  R("*",    "/api/chat/stop", false, AP_ADMIN, "/chat"), R("*", "/api/chat/tab", false, AP_ADMIN, "/chat"),
  R("*",    "/api/chat/record", false, AP_ADMIN, "/chat"), R("*", "/api/chat/key", false, AP_ADMIN, "/chat"),
  R("*",    "/api/chat/key_remove", false, AP_ADMIN, "/chat"), R("*", "/api/chat/complete", false, AP_ADMIN, "/chat"),
  R("*",    "/api/chat/account", false, AP_ADMIN, "/chat"), R("*", "/api/chat/turn", false, AP_ADMIN, "/chat"),
  R("*",    "/api/chat/", true, AP_GUEST, "a room's chat (room.c)"),
  R("*",    "/api/presence/", true, AP_GUEST, "a room's presence (room.c)"),
  // ---- scheduled jobs: Cloud Scheduler's OIDC token or an admin, checked there (jobs.c)
  R("*",    "/api/jobs/", true, AP_PUBLIC, "the scheduler (OIDC) or an admin, checked there"),
  // ---- analytics (analytics.c)
  R("*",    "/api/analytics/events", false, AP_GUEST, "a page's batch"),
  R("*",    "/api/analytics/team", false, AP_PUBLIC, "a team link's token, checked there"),
  R("*",    "/api/analytics/realms", false, AP_PUBLIC, "public realm counts"),
  // ---- the playtest and the Metaverse: player keys, accounts and signers, each checked in its route
  R("*",    "/api/playtest/", true, AP_GUEST, "players, realms, makers, seats, rooms, labs (player.c, realms.c, ...)"),
  R("*",    "/api/change/", true, AP_GUEST, "the demo's old paths (player.c)"),
  R("*",    "/api/log/", true, AP_GUEST, "the demo's old paths (player.c)"),
  R("*",    "/api/award", true, AP_GUEST, "the demo's old paths (player.c)"),
  R("*",    "/api/planets", true, AP_GUEST, "the demo's old paths (player.c)"),
  R("*",    "/api/metaverse/packs/", true, AP_GUEST, "private human packs, a key a session (verse_packs.c)"),
  R("*",    "/api/verse/packs/", true, AP_GUEST, "private human packs, a key a session (verse_packs.c: the first address, kept)"),
  R("*",    "/api/villa/", true, AP_GUEST, "the villa (villa.c)"),
  R("*",    "/memorial", true, AP_PUBLIC, "a memorial's page (villa.c)"),
  // ---- Lightning Addresses (lnaddr.c, #13208): wallets ask without a credential; the holder's routes
  R("GET",  "/.well-known/lnurlp/", true, AP_PUBLIC, "LUD-16 lookups, capped per network address there"),
  R("GET",  "/lnurlp/", true, AP_PUBLIC, "LNURL-pay callbacks and LUD-21 verify, capped there"),
  R("GET",  "/api/lightning/address", false, AP_ACCOUNT, "your address, where it goes, a waiting change"),
  R("POST", "/api/lightning/address", false, AP_HUMAN, "claim a name (people only)"),
  R("POST", "/api/lightning/address/off", false, AP_HUMAN, "stop receiving; the name stays yours"),
  R("POST", "/api/lightning/address/on", false, AP_HUMAN, "receive again"),
  R("POST", "/api/lightning/address/change", false, AP_L2_FRESH, "a new destination: a fresh key login, then a 24-hour wait"),
  R("POST", "/api/lightning/address/cancel", false, AP_HUMAN, "cancel a waiting change"),
  R("GET",  "/api/lightning/address/available", false, AP_ACCOUNT, "whether a name could be claimed, as you type (60 a minute)"),
  R("GET",  "/api/lightning/address/proof", false, AP_HUMAN, "the message your wallet signs to receive at your name"),
  R("GET",  "/api/lightning/address/profile", false, AP_HUMAN, "your Nostr profile with this address in it, for your key to sign"),
  R("POST", "/api/lightning/address/profile", false, AP_HUMAN, "your signed profile, checked and passed on to the relays; we never sign it"),
  R("GET",  "/.well-known/nostr.json", false, AP_PUBLIC, "NIP-05: the key on the account that signed its address's profile"),
  // ---- our models and the mesh: NIP-98 or a session, metered (inference.c, mesh.c)
  R("*",    "/api/v1/", true, AP_GUEST, "NIP-98 signed or logged in, checked there"),
  // ---- private projects and drops: members only, the same 404 for everyone else (private.c, drops.c)
  R("*",    "/api/private/", true, AP_PUBLIC, "members' tools, NIP-98 signed, checked there"),
  R("*",    "/projects/", true, AP_PUBLIC, "a member's page, else the locked one"),
  R("*",    "/api/drops", true, AP_PUBLIC, "who dropped them, checked there"),
  R("*",    "/api/worldbuilder/", true, AP_PUBLIC, "/create-world's chat: anyone, capped per address there (worldbuilder.c)"),
  R("GET",  "/api/worlds", false, AP_PUBLIC, "the worlds the asker may edit, for the site's side: none for someone logged out (worlds.c)"),
  R("GET",  "/api/projects", false, AP_PUBLIC, "the projects the asker is in, for the site's side: none for someone logged out (worlds.c)"),
  R("*",    "/api/worlds/", true, AP_PUBLIC, "a world's document: anyone reads; an admin writes, checked there (worlds.c, world-policy.coax)"),
  R("GET",  "/api/projects/", true, AP_PUBLIC, "a project's attachments: its members and admins, checked there (worlds.c, world-files.coax)"),
};
#undef R

static const char *rule_name(int rule) {
  static const char *names[] = {"public", "guest", "account", "human", "L2", "L2-fresh", "scope", "admin", "recover", "admin-change"};
  return rule >= 0 && rule <= 9 ? names[rule] : "?";
}

static const auth_rule *rule_for(const char *method, const char *path) {
  for (size_t i = 0; i < sizeof RULES / sizeof RULES[0]; i++) {
    const auth_rule *r = &RULES[i];
    if (strcmp(r->method, "*") && strcmp(r->method, method) && !(!strcmp(r->method, "GET") && !strcmp(method, "HEAD"))) continue;
    size_t n = strlen(r->path);
    if (r->prefix ? !strncmp(path, r->path, n) : !strcmp(path, r->path)) return r;
  }
  return NULL;
}

void auth_print_rules(FILE *out) {
  for (size_t i = 0; i < sizeof RULES / sizeof RULES[0]; i++)
    fprintf(out, "%s\t%s%s\t%s\t%s\n", RULES[i].method, RULES[i].path, RULES[i].prefix ? "*" : "", rule_name(RULES[i].rule), RULES[i].note);
}
// The rule a path would get (api --rule METHOD PATH), for the route check.
const char *auth_rule_of(const char *method, const char *path) {
  const auth_rule *r = rule_for(method, path);
  return r ? rule_name(r->rule) : NULL;
}

// ---------------------------------------------------------------- limits (#13203)
// Every limit we enforce, as people read it. /docs/limits and GET /api/auth/limits are made from
// this table (api --limits), so the page and the server can't drift. The words name the limit;
// the answer adds when it lifts.
typedef struct { const char *name, *what, *per, *words; int max; long long window_ms; } auth_limit;
enum { L_EMAIL_HOUR, L_EMAIL_DAY, L_EMAIL_IP, L_CODE_TRIES, L_KEY_LOGIN, L_REFRESH, L_NEW_ACCOUNT, L_AGENT_ASK, L_RECOVERY, L_RECOVERY_CODE, L_RECOVERY_CODE_IP, L_COUNT };
_Static_assert(L_KEY_LOGIN == AUTH_LIMIT_KEY_LOGIN && L_RECOVERY == AUTH_LIMIT_RECOVERY && L_RECOVERY_CODE == AUTH_LIMIT_RECOVERY_CODE
               && L_RECOVERY_CODE_IP == AUTH_LIMIT_RECOVERY_CODE_IP, "auth-internal.h's limits");
static const auth_limit LIMITS[L_COUNT] = {
  [L_EMAIL_HOUR]  = {"email_hour", "Email codes sent to one address", "an hour", "Too many codes for this address.", 5, 3600000LL},
  [L_EMAIL_DAY]   = {"email_day", "Email codes sent to one address", "a day", "Too many codes for this address today.", 20, 86400000LL},
  [L_EMAIL_IP]    = {"email_ip", "Email codes sent from one network address", "an hour", "Too many codes from your network.", 30, 3600000LL},
  [L_CODE_TRIES]  = {"code_tries", "Tries at one email code", "per code", "Too many tries. Ask for a new code.", 5, 600000LL},
  [L_KEY_LOGIN]   = {"key_login", "Logins with a key or passkey from one network address", "a minute", "Too many logins from your network.", 30, 60000LL},
  [L_REFRESH]     = {"refresh", "App token refreshes for one login", "an hour", "Too many refreshes.", 60, 3600000LL},
  [L_NEW_ACCOUNT] = {"new_account", "New accounts from one network address", "an hour", "Too many new accounts from your network.", 10, 3600000LL},
  [L_AGENT_ASK]   = {"agent_ask", "Agent and computer login requests from one network address", "an hour", "Too many requests from your network.", 30, 3600000LL},
  [L_RECOVERY]    = {"recovery", "Recovery requests for one account", "a week", "Too many recovery requests for this account.", 3, 604800000LL},
  [L_RECOVERY_CODE] = {"recovery_code", "Recovery code tries for one account", "an hour", "Too many recovery code tries for this account.", 5, 3600000LL},
  [L_RECOVERY_CODE_IP] = {"recovery_code_ip", "Recovery code tries from one network address", "an hour", "Too many recovery code tries from your network.", 10, 3600000LL},
};

void auth_print_limits(FILE *out) {   // api --limits: the Markdown /docs/limits is
  fprintf(out, "# Limits\n\nEvery limit we enforce on logging in. When you reach one, we say which and when it lifts.\n\n"
               "| What | Limit |\n| --- | --- |\n");
  for (int i = 0; i < L_COUNT; i++) fprintf(out, "| %s | %d %s |\n", LIMITS[i].what, LIMITS[i].max, LIMITS[i].per);
  fprintf(out, "\nThis page is made from the table the API enforces (`coax/web/api/auth.c`).\n");
}

// ---------------------------------------------------------------- small pieces
static void hex_of(const unsigned char *b, size_t n, char *out) {
  static const char d[] = "0123456789abcdef";
  for (size_t i = 0; i < n; i++) { out[2 * i] = d[b[i] >> 4]; out[2 * i + 1] = d[b[i] & 15]; }
  out[2 * n] = 0;
}
static bool unhex(const char *s, unsigned char *out, size_t n) {
  if (strlen(s) < 2 * n) return false;
  for (size_t i = 0; i < n; i++) {
    int v = 0;
    for (int k = 0; k < 2; k++) {
      char c = s[2 * i + k]; v <<= 4;
      if (c >= '0' && c <= '9') v |= c - '0'; else if (c >= 'a' && c <= 'f') v |= c - 'a' + 10; else return false;
    }
    out[i] = (unsigned char)v;
  }
  return true;
}
static bool random_bytes(unsigned char *b, size_t n) {
  while (n) { size_t k = n > 256 ? 256 : n; if (getentropy(b, k)) return false; b += k; n -= k; }
  return true;
}
static bool random_hex(char *out, size_t bytes) {
  unsigned char b[64];
  if (bytes > sizeof b || !random_bytes(b, bytes)) return false;
  hex_of(b, bytes, out);
  return true;
}
// SHA-256 of a string, hex.
static void sha_hex(const char *s, char out[65]) {
  unsigned char d[32];
  as_sha256((coax_v_u8){(uint8_t *)s, (int64_t)strlen(s)}, (coax_v_u8){d, 32});
  hex_of(d, 32, out);
}
// A new token of one of the four kinds (oas_, oa_at_, oa_rt_): the string and its digest.
static bool new_token(const char *prefix, char *out, size_t cap, char digest[65]) {
  unsigned char r[32];
  if (!random_bytes(r, 32)) return false;
  int64_t n = token_make((coax_string){prefix, (int64_t)strlen(prefix)}, (coax_rv_u8){r, 32}, (coax_v_u8){(uint8_t *)out, (int64_t)cap - 1});
  memset(r, 0, sizeof r);
  if (n <= 0) return false;
  out[n] = 0;
  unsigned char d[32];
  if (!token_digest((coax_string){out, n}, (coax_v_u8){d, 32})) return false;
  hex_of(d, 32, digest);
  return true;
}
// A token as sent: its digest when it is well formed and of this kind, else false. Checked before
// any database lookup.
static bool token_ok(const char *text, int64_t kind, char digest[65]) {
  coax_string s = {text, (int64_t)strlen(text)};
  coax_t_TkCheck c = token_check(s);
  if (c.error || c.kind != kind) return false;
  unsigned char d[32];
  if (!token_digest(s, (coax_v_u8){d, 32})) return false;
  hex_of(d, 32, digest);
  return true;
}
enum { TOKEN_SESSION = 1, TOKEN_ACCESS = 2, TOKEN_REFRESH = 3, TOKEN_KEY = 4 };

// A cookie's value (letters, digits, _ and -), or "".
static void cookie(const request *q, const char *name, char *out, size_t cap);
void auth_cookie(const request *q, const char *name, char *out, size_t cap) { cookie(q, name, out, cap); }
static void cookie(const request *q, const char *name, char *out, size_t cap) {
  char cookies[4096], want[64];
  out[0] = 0;
  if (!api_header(q, "Cookie", cookies, sizeof cookies)) return;
  int w = snprintf(want, sizeof want, "%s=", name);
  for (const char *c = strstr(cookies, want); c; c = strstr(c + 1, want)) {
    if (c != cookies && c[-1] != ' ' && c[-1] != ';') continue;
    c += w;
    size_t n = 0;
    while (n + 1 < cap && (isalnum((unsigned char)c[n]) || c[n] == '_' || c[n] == '-')) { out[n] = c[n]; n++; }
    out[n] = 0;
    return;
  }
}

// The visitor's address: the load balancer's next to last (as access.c), and its digest for keys.
static void visitor(const request *q, char ip[128]) {
  char xff[512];
  api_header(q, "X-Forwarded-For", xff, sizeof xff);
  char *parts[16]; int np = 0;
  for (char *t = strtok(xff, ", "); t && np < 16; t = strtok(NULL, ", ")) parts[np++] = t;
  snprintf(ip, 128, "%s", np ? parts[np >= 2 ? np - 2 : 0] : "local");
}
static void visitor_key(const request *q, char out[65]) { char ip[128]; visitor(q, ip); char s[160]; snprintf(s, sizeof s, "ip:%s", ip); sha_hex(s, out); }

// Every body this file writes or reads is auth-wire.coax's: a string for it, a buffer for a body
// (room kept for the 0 after it), and a buffer for a request body's member (its 0 included).
static coax_string cs(const char *s) { return (coax_string){s ? s : "", s ? (int64_t)strlen(s) : 0}; }
#define OUT(b) ((coax_v_u8){(uint8_t *)(b), (int64_t)sizeof (b) - 1})
#define ASK(b) ((coax_v_u8){(uint8_t *)(b), (int64_t)sizeof (b)})
// A string as a JSON string's inside (quotes and controls escaped), bounded.
static void json_esc(const char *s, char *out, size_t cap) {
  size_t k = 0;
  for (; *s && k + 7 < cap; s++) {
    unsigned char c = (unsigned char)*s;
    if (c == '"' || c == '\\') { out[k++] = '\\'; out[k++] = (char)c; }
    else if (c < 0x20) k += (size_t)snprintf(out + k, cap - k, "\\u%04x", c);
    else out[k++] = (char)c;
  }
  out[k] = 0;
}

static bool as_lines(const request *q) { char v[16]; return api_query(q, "as", v, sizeof v) && !strcmp(v, "lines"); }
// An answer: JSON, or (for ?as=lines) the lines. headers: extra response headers, or NULL.
static void answer(const request *q, int status, const char *headers, const char *json, const char *lines) {
  if (as_lines(q) && lines) api_respond(q->fd, status, "text/plain; charset=utf-8", headers, lines, strlen(lines));
  else api_respond(q->fd, status, "application/json", headers, json, strlen(json));
}
static void refuse(const request *q, int status, const char *words);
// The same for bodies auth-wire.coax wrote: their lengths, -1 for one that didn't fit its buffer.
// Only the form that was asked for has to fit.
static void answer_n(const request *q, int status, const char *headers, const char *json, int64_t jn, const char *lines, int64_t ln) {
  bool as = as_lines(q) && lines;
  if (as ? ln < 0 : jn < 0) { refuse(q, 500, "That didn't work. Try again."); return; }
  if (as) api_respond(q->fd, status, "text/plain; charset=utf-8", headers, lines, (size_t)ln);
  else api_respond(q->fd, status, "application/json", headers, json, (size_t)jn);
}
static void refuse(const request *q, int status, const char *words) {
  char json[600], lines[500];
  api_error_text(json, sizeof json, words);
  answer_n(q, status, NULL, json, (int64_t)strlen(json), lines, auw_refusal_lines(cs(words), OUT(lines)));
}
// A one-word answer: linked, approved, waiting, logged-out, removed, frozen.
static void answer_state(const request *q, int status, const char *headers, const char *state) {
  char json[64], lines[64];
  answer_n(q, status, headers, json, auw_state_write(cs(state), OUT(json)), lines, auw_state_lines(cs(state), OUT(lines)));
}

// One audit row (auth.audit). detail: a JSON object of fixed keys, never secrets or PII (NULL: the
// empty object).
static const char *detail_of(char *d, int64_t n) { if (n < 0) return NULL; d[n] = 0; return d; }
static void audit(PGconn *db, const char *acct, const char *actor, const char *action, const char *result, const char *detail) {
  char none[8];
  if (!detail) detail = detail_of(none, auw_none_write(OUT(none)));
  const char *v[5] = {acct && *acct ? acct : NULL, actor && *actor ? actor : NULL, action, result, detail};
  PGresult *r = PQexecParams(db, "SELECT auth.log($1, $2, $3, $4, $5::jsonb)", 5, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK) api_log("auth: audit %s: %s", action, PQerrorMessage(db));
  PQclear(r);
}
// The row whose detail is why it was refused or failed.
static void audit_why(PGconn *db, const char *acct, const char *actor, const char *action, const char *result, const char *why) {
  char d[64];
  audit(db, acct, actor, action, result, detail_of(d, auw_why_write(cs(why), OUT(d))));
}

// One limit's hit, counted. True when allowed; else until_ms is when the window lifts.
static bool limit_hit(PGconn *db, int which, const char *key, long long *until_ms) {
  const auth_limit *l = &LIMITS[which];
  char win[32]; snprintf(win, sizeof win, "%lld", l->window_ms);
  const char *v[3] = {l->name, key, win};
  PGresult *r = PQexecParams(db, "SELECT started_ms, n FROM auth.limit_hit($1, $2, $3::bigint, true)", 3, NULL, v, NULL, NULL, 0);
  bool ok = true;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    long long started = atoll(PQgetvalue(r, 0, 0));
    int n = atoi(PQgetvalue(r, 0, 1));
    ok = ap_limit_ok(n, l->max);
    if (until_ms) *until_ms = started + l->window_ms;
  } else api_log("auth: limit %s: %s", l->name, PQerrorMessage(db));   // a counting failure doesn't lock people out
  PQclear(r);
  return ok;
}
// One hit on a per-minute limit of a route outside this file, for `who` (an address, a key), counted in
// auth.limit_counts (migration 0145) so every worker and instance shares the count (#13300). The key
// kept is a digest. True when allowed (ap_limit_ok, auth-policy.coax); else *left_ms is the ms until
// the window lifts. A count that can't be read refuses nobody.
bool api_limit(PGconn *db, const char *name, const char *who, int per_minute, long long *left_ms) {
  if (left_ms) *left_ms = 0;
  if (!db) return true;
  const char *v[2] = {name, who && *who ? who : "-"};
  PGresult *r = PQexecParams(db, "SELECT started_ms, n, now_ms FROM auth.limit_hit($1, left(encode(sha256(convert_to($2, 'UTF8')), 'hex'), 40), 60000, true)",
                             2, NULL, v, NULL, NULL, 0);
  bool ok = true;
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
    ok = ap_limit_ok(atoll(PQgetvalue(r, 0, 1)), per_minute);
    long long left = atoll(PQgetvalue(r, 0, 0)) + 60000 - atoll(PQgetvalue(r, 0, 2));
    if (!ok && left_ms) *left_ms = left > 0 ? left : 1;
  } else api_log("limit %s: %s", name, PQerrorMessage(db));
  PQclear(r);
  return ok;
}

// The refusal for a limit: its words and when it lifts, both shown.
static void refuse_limit(const request *q, int which, long long until_ms) {
  long long left = until_ms - (long long)api_now_ms();
  if (left < 1000) left = 1000;
  char when[64];
  if (left < 90000) snprintf(when, sizeof when, "in %lld seconds", (left + 999) / 1000);
  else if (left < 5400000) snprintf(when, sizeof when, "in %lld minutes", (left + 59999) / 60000);
  else snprintf(when, sizeof when, "in %lld hours", (left + 3599999) / 3600000);
  char words[300], json[700], lines[500];
  snprintf(words, sizeof words, "%s Try again %s.", LIMITS[which].words, when);
  char retry[64]; snprintf(retry, sizeof retry, "Retry-After: %lld\r\n", (left + 999) / 1000);
  answer_n(q, 429, retry, json, auw_limit_write(cs(words), cs(LIMITS[which].name), LIMITS[which].max, cs(LIMITS[which].per), until_ms, OUT(json)),
           lines, auw_limit_lines(cs(words), cs(LIMITS[which].name), until_ms, OUT(lines)));
}

// ---- for agents.c: the same answers, audit rows, tokens, addresses and limits as this file's
void auth_answer(const request *q, int status, const char *headers, const char *json, const char *lines) { answer(q, status, headers, json, lines); }
void auth_refuse(const request *q, int status, const char *words) { refuse(q, status, words); }
void auth_audit(PGconn *db, const char *acct, const char *actor, const char *action, const char *result, const char *detail) { audit(db, acct, actor, action, result, detail); }
// A new agent API key: oa_key_ + its id (8 base32 characters, shown in lists; not secret) + _ + the
// secret part. id gets the id; the digest is what auth.agent_keys keeps.
bool auth_new_key(char id[9], char *out, size_t cap, char digest[65]) {
  unsigned char r[8];
  if (!random_bytes(r, sizeof r)) return false;
  for (int i = 0; i < 8; i++) id[i] = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"[r[i] & 31];
  id[8] = 0;
  char prefix[20]; snprintf(prefix, sizeof prefix, "oa_key_%s_", id);
  return new_token(prefix, out, cap, digest);
}
void auth_visitor(const request *q, char ip[128]) { visitor(q, ip); }
bool auth_limited(const request *q, PGconn *db) {   // true when it refused: agent and device requests from this address
  long long until = 0; char ipk[65]; visitor_key(q, ipk);
  if (limit_hit(db, L_AGENT_ASK, ipk, &until)) return false;
  refuse_limit(q, L_AGENT_ASK, until);
  return true;
}
bool auth_same_site(const request *q) { char from[256]; if (!api_header(q, "Origin", from, sizeof from)) return true; return !strcmp(from, origin()); }
const char *auth_origin(void) { return origin(); }

// ---------------------------------------------------------------- keys for sealing and lookup
typedef struct { int version; unsigned char key[32]; } versioned_key;
// "1:<hex>,2:<hex>" into keys (newest last); how many.
static int keys_from(const char *env, versioned_key *keys, int cap) {
  const char *s = getenv(env);
  int n = 0;
  while (s && *s && n < cap) {
    int v = atoi(s);
    const char *colon = strchr(s, ':');
    if (v <= 0 || v > 255 || !colon || !unhex(colon + 1, keys[n].key, 32)) break;
    keys[n].version = v; n++;
    s = strchr(colon, ',');
    if (s) s++;
  }
  return n;
}
static bool sealing_ready(void) {
  versioned_key k[4];
  return keys_from("OA_AUTH_SEAL_KEY", k, 4) > 0 && keys_from("OA_AUTH_LOOKUP_PEPPER", k, 4) > 0;
}
// The lookup of a normalized address under every pepper: "{1:hex,2:hex}" (a Postgres array) and
// the newest one alone.
static bool lookups(const char *email, char *all, size_t cap, char newest[80]) {
  versioned_key k[4];
  int n = keys_from("OA_AUTH_LOOKUP_PEPPER", k, 4);
  if (!n) return false;
  size_t at = (size_t)snprintf(all, cap, "{");
  for (int i = 0; i < n; i++) {
    unsigned char d[32]; char h[65];
    if (!as_lookup((coax_v_u8){k[i].key, 32}, (coax_v_u8){(uint8_t *)email, (int64_t)strlen(email)}, (coax_v_u8){d, 32})) return false;
    hex_of(d, 32, h);
    at += (size_t)snprintf(all + at, cap - at, "%s%d:%s", i ? "," : "", k[i].version, h);
    if (i == n - 1) snprintf(newest, 80, "%d:%s", k[i].version, h);
  }
  snprintf(all + at, cap - at, "}");
  return true;
}
// Seals text with associated data ad under the newest key: hex of version, nonce, box.
static bool seal(const char *ad, const char *text, char *out, size_t cap) {
  versioned_key k[4];
  int n = keys_from("OA_AUTH_SEAL_KEY", k, 4);
  size_t len = strlen(text);
  if (!n || len > 512 || cap < 2 * (1 + 24 + len + 16) + 1) return false;
  unsigned char buf[1 + 24 + 512 + 16];
  buf[0] = (unsigned char)k[n - 1].version;
  if (!random_bytes(buf + 1, 24)) return false;
  if (!as_seal((coax_v_u8){k[n - 1].key, 32}, (coax_v_u8){buf + 1, 24}, (coax_v_u8){(uint8_t *)ad, (int64_t)strlen(ad)},
               (coax_v_u8){(uint8_t *)text, (int64_t)len}, (coax_v_u8){buf + 25, (int64_t)len + 16})) return false;
  hex_of(buf, 1 + 24 + len + 16, out);
  return true;
}
// Opens a sealed value with associated data ad; false when no key opens it.
static bool unseal(const char *ad, const char *sealed, char *out, size_t cap) {
  size_t h = strlen(sealed);
  if (h % 2 || h < 2 * (1 + 24 + 16) || h > 2 * (1 + 24 + 512 + 16)) return false;
  unsigned char buf[1 + 24 + 512 + 16];
  if (!unhex(sealed, buf, h / 2)) return false;
  size_t len = h / 2 - 1 - 24 - 16;
  if (cap < len + 1) return false;
  versioned_key k[4];
  int n = keys_from("OA_AUTH_SEAL_KEY", k, 4);
  for (int i = 0; i < n; i++) {
    if (k[i].version != buf[0]) continue;
    if (as_open((coax_v_u8){k[i].key, 32}, (coax_v_u8){buf + 1, 24}, (coax_v_u8){(uint8_t *)ad, (int64_t)strlen(ad)},
                (coax_v_u8){buf + 25, (int64_t)len + 16}, (coax_v_u8){(uint8_t *)out, (int64_t)len})) { out[len] = 0; return true; }
  }
  return false;
}
// a•••@example.com
static void mask_email(const char *email, char *out, size_t cap) {
  const char *at = strchr(email, '@');
  if (!at || at == email) { snprintf(out, cap, "•••"); return; }
  snprintf(out, cap, "%c•••%s", email[0], at);
}

// ---------------------------------------------------------------- the principal
static auth_principal current;
static bool resolved;
static const request *resolved_for;
static char set_cookie[700];   // a rotated or cleared session cookie, sent with this request's answer

#define SESSION_COOKIE "__Host-oa_session"
#define PENDING_COOKIE "__Host-oa_pending"
#define SESSION_MAX_AGE 2592000
#define LOCAL_OUT_COOKIE "oa_local_out"   // the dev server only (OA_LOCAL_SERVE): this browser logged out   // 30 days: the idle limit; every rotation sends it again

static void cookie_header(char *out, size_t cap, const char *name, const char *value, int max_age) {
  snprintf(out, cap, "Set-Cookie: %s=%s; Path=/; Max-Age=%d; HttpOnly; Secure; SameSite=Lax\r\n", name, value, max_age);
}

// Why a frozen account is frozen: 1 a freeze, 2 a deletion waiting (auth-life-policy.coax AL_WHY_*).
static int frozen_why(PGconn *db, const char *acct) {
  const char *v[1] = {acct};
  PGresult *r = PQexecParams(db, "SELECT frozen_why FROM auth.accounts WHERE id = $1", 1, NULL, v, NULL, NULL, 0);
  int why = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "delete") ? 2 : 1;
  PQclear(r);
  return why;
}
// Looks up a session by its token's digest and fills p. false when there is none or it has
// ended (an ended web session's row is deleted, and its cookie cleared).
static bool from_session(PGconn *db, const char *sha, bool web, const char *token_for_rotation, auth_principal *p) {
  const char *v[1] = {sha};
  PGresult *r = PQexecParams(db, "SELECT id, account, kind, verse_account, level, via_kind, client, created_ms, last_seen_ms, rotated_ms, "
                                 "coalesce(l2_ms, 0), now_ms, current, good_epoch, frozen, team, coalesce(pubkey, ''), strong FROM auth.session_look($1)",
                             1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); return false; }
  long long now = atoll(PQgetvalue(r, 0, 11));
  bool app = !strcmp(PQgetvalue(r, 0, 6), "app");
  int state = (int)ap_session(app, now, atoll(PQgetvalue(r, 0, 7)), atoll(PQgetvalue(r, 0, 8)), atoll(PQgetvalue(r, 0, 9)));
  bool good_epoch = !strcmp(PQgetvalue(r, 0, 13), "t"), frozen = !strcmp(PQgetvalue(r, 0, 14), "t");
  char sid[32]; snprintf(sid, sizeof sid, "%s", PQgetvalue(r, 0, 0));
  if (state == AP_SESSION_ENDED || !good_epoch) {
    PQclear(r);
    const char *e[1] = {sid};
    PQclear(PQexecParams(db, "SELECT auth.session_end($1)", 1, NULL, e, NULL, NULL, 0));
    if (web) cookie_header(set_cookie, sizeof set_cookie, SESSION_COOKIE, "", 0);
    return false;
  }
  memset(p, 0, sizeof *p);
  const char *kind = PQgetvalue(r, 0, 2);
  p->kind = !strcmp(kind, "agent") ? AP_AGENT : !strcmp(kind, "service") ? AP_SERVICE : AP_PERSON;
  snprintf(p->account, sizeof p->account, "%s", PQgetvalue(r, 0, 1));
  snprintf(p->verse_account, sizeof p->verse_account, "%s", PQgetvalue(r, 0, 3));
  snprintf(p->session, sizeof p->session, "%s", sid);
  p->level = !strcmp(PQgetvalue(r, 0, 4), "L2") ? 2 : 1;
  long long l2 = atoll(PQgetvalue(r, 0, 10));
  p->l2_age_ms = l2 > 0 ? now - l2 : -1;
  p->team = !strcmp(PQgetvalue(r, 0, 15), "t");
  snprintf(p->pubkey, sizeof p->pubkey, "%s", PQgetvalue(r, 0, 16));
  snprintf(p->via, sizeof p->via, "%s", PQgetvalue(r, 0, 5));
  p->strong = atoi(PQgetvalue(r, 0, 17));
  // Email alone on an account with a stronger way in (#13212): it reads and plays; it never acts as
  // the account's key (private projects, admin pages, credit) until it steps up or recovery ends.
  p->limited = ap_limited(p->level, p->strong);
  if (p->limited) p->pubkey[0] = 0;
  // A frozen account's session (#13214): the gate lets it only look, unfreeze, delete, cancel and
  // log out (auth-life-policy.coax), and it never acts as the account's key.
  p->l1_age_ms = !strcmp(p->via, "email") ? now - atoll(PQgetvalue(r, 0, 7)) : -1;
  if (frozen) { p->frozen = frozen_why(db, p->account); p->pubkey[0] = 0; }
  p->admin = p->pubkey[0] && admin_is(p->pubkey);
  p->credential = web ? AUTH_COOKIE : AUTH_BEARER;
  bool is_current = !strcmp(PQgetvalue(r, 0, 12), "t");
  PQclear(r);
  const char *s[1] = {sid};
  PQclear(PQexecParams(db, "SELECT auth.session_seen($1)", 1, NULL, s, NULL, NULL, 0));
  // A new web token every 24 hours of use; the old one works 60 seconds more (parallel tabs).
  if (web && is_current && state == AP_SESSION_ROTATE && token_for_rotation) {
    char tok[160], dig[65];
    if (new_token("oas_", tok, sizeof tok, dig)) {
      const char *w[3] = {sid, sha, dig};
      PGresult *x = PQexecParams(db, "SELECT auth.session_rotate($1, $2, $3)", 3, NULL, w, NULL, NULL, 0);
      if (PQresultStatus(x) == PGRES_TUPLES_OK && PQntuples(x) == 1 && !strcmp(PQgetvalue(x, 0, 0), "t"))
        cookie_header(set_cookie, sizeof set_cookie, SESSION_COOKIE, tok, SESSION_MAX_AGE);
      PQclear(x);
    }
  }
  return true;
}

#ifdef OA_LOCAL_SERVE
static bool local_admin(const request *q, PGconn *db, auth_principal *p);
static void local_owner_init(PGconn *db);
static bool local_owner_account(PGconn *db, const char *acct);
#endif
// Resolves the request's one credential. false with *status (400 two credentials, 401 a bad
// credential) when it answered nothing yet but must be refused.
static bool resolve(const request *q, PGconn *db, auth_principal *p, int *status) {
  memset(p, 0, sizeof *p);
  *status = 0;
  char auth[4096], tok[256];
  bool has_auth = api_header(q, "Authorization", auth, sizeof auth) != NULL;
  bool nostr = has_auth && !strncmp(auth, "Nostr ", 6);
  bool bearer = has_auth && (!strncmp(auth, "Bearer oa_at_", 13) || !strncmp(auth, "Bearer oa_key_", 14));
  cookie(q, SESSION_COOKIE, tok, sizeof tok);
  bool has_cookie = tok[0] != 0;
  int verdict = (int)ap_credentials(nostr, bearer, has_cookie);
  if (verdict != AP_OK) { *status = (int)ap_status(verdict); return false; }
  api_header(q, "X-Verse-Player", p->player, sizeof p->player);
  if (nostr) {
    char url[4300], err[160];
    snprintf(url, sizeof url, "%s%s", origin(), q->target);
    nostr_event ev;
    if (!nip98_check(auth, q->method, url, q->body, q->body_len, (long long)time(NULL), &ev, err, sizeof err)) { *status = 401; return false; }
    memcpy(p->pubkey, ev.pubkey, 65);
    memcpy(p->event_id, ev.id, 65);
    p->payload = ev.payload[0] != 0;
    p->credential = AUTH_NOSTR;
    p->level = 2; p->l2_age_ms = 0;
    p->kind = AP_PERSON;   // a key no account holds signs as itself; an agent's key is its agent
    const char *v[1] = {ev.pubkey};
    PGresult *r = PQexecParams(db, "SELECT a.id, a.kind, a.verse_account, a.team, a.frozen_at IS NOT NULL FROM auth.nostr_keys k "
                                   "JOIN auth.accounts a ON a.id = k.account WHERE k.pubkey = $1 AND a.deleted_at IS NULL",
                               1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) {
      bool frozen = !strcmp(PQgetvalue(r, 0, 4), "t");
      snprintf(p->account, sizeof p->account, "%s", PQgetvalue(r, 0, 0));
      if (frozen) p->frozen = frozen_why(db, p->account);   // the gate holds it to auth-life-policy.coax's few routes
      const char *k = PQgetvalue(r, 0, 1);
      p->kind = !strcmp(k, "agent") ? AP_AGENT : !strcmp(k, "service") ? AP_SERVICE : AP_PERSON;
      snprintf(p->verse_account, sizeof p->verse_account, "%s", PQgetvalue(r, 0, 2));
      p->team = !strcmp(PQgetvalue(r, 0, 3), "t");
    } else snprintf(p->verse_account, sizeof p->verse_account, "n-%s", ev.pubkey);
    PQclear(r);
    p->admin = admin_is(p->pubkey);
    snprintf(p->via, sizeof p->via, "nostr");
    return true;
  }
  if (bearer && !strncmp(auth, "Bearer oa_key_", 14)) {   // an agent's API key
    char t[256], dig[65];
    snprintf(t, sizeof t, "%s", auth + 7);
    if (!token_ok(t, TOKEN_KEY, dig) || !agents_key(db, dig, p)) { *status = 401; return false; }
    api_header(q, "X-Verse-Player", p->player, sizeof p->player);
    return true;
  }
  if (bearer) {
    char t[256], dig[65];
    snprintf(t, sizeof t, "%s", auth + 7);
    if (!token_ok(t, TOKEN_ACCESS, dig)) { *status = 401; return false; }
    const char *v[1] = {dig};
    PGresult *r = PQexecParams(db, "SELECT auth.access_session($1)", 1, NULL, v, NULL, NULL, 0);
    char ssha[65] = "";
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0)) snprintf(ssha, sizeof ssha, "%s", PQgetvalue(r, 0, 0));
    PQclear(r);
    if (!ssha[0] || !from_session(db, ssha, false, NULL, p)) { *status = 401; return false; }
    api_header(q, "X-Verse-Player", p->player, sizeof p->player);
    return true;
  }
  if (has_cookie) {
    char dig[65];
    if (token_ok(tok, TOKEN_SESSION, dig) && from_session(db, dig, true, tok, p)) {
      api_header(q, "X-Verse-Player", p->player, sizeof p->player);
      return true;
    }
#ifdef OA_LOCAL_SERVE
    // On the local serve mode an unknown cookie is another local API's login on this host (cookies don't
    // keep to a port): it is left alone, and the request is the local admin's as with no cookie.
    if (local_admin(q, db, p)) return true;
#endif
    // A stale or unknown cookie is nobody, and is cleared.
    memset(p, 0, sizeof *p);
    api_header(q, "X-Verse-Player", p->player, sizeof p->player);
    if (!set_cookie[0]) cookie_header(set_cookie, sizeof set_cookie, SESSION_COOKIE, "", 0);
  }
#ifdef OA_LOCAL_SERVE
  if (!has_cookie && !has_auth && local_admin(q, db, p)) return true;
#endif
  p->kind = p->player[0] ? AP_KIND_GUEST : AP_NOBODY;
  return true;
}

// The principal for this request (resolved once, on first ask). NULL when its credential was
// refused (two credentials, a bad signature or token).
const auth_principal *auth_principal_of(const request *q) {
  if (resolved && resolved_for == q) return current.refused ? NULL : &current;
  PGconn *db = api_db();
  resolved = true; resolved_for = q;
#ifdef OA_LOCAL_SERVE
  static bool owner_ready;   // the dev server: the owner's account and email, before the first answer
  if (!owner_ready && db) { owner_ready = true; local_owner_init(db); }
#endif
  int status = 0;
  if (!db || !resolve(q, db, &current, &status)) { current.refused = status ? status : 503; return NULL; }
  return &current;
}
// The numbers api.h shares with auth-policy.coax: a mismatch stops the service at start.
void auth_init(void) {
  bool ok = ap_admit(AP_ADMIN, AP_PERSON, 2, 0, true, false) == AP_OK && ap_admit(AP_ADMIN, AP_PERSON, 2, 0, false, false) == AP_NOT_ADMIN
         && ap_admit(AP_ACCOUNT, AP_NOBODY, 0, -1, false, false) == AP_LOG_IN && ap_admit(AP_HUMAN, AP_AGENT, 2, 0, false, false) == AP_HUMAN_ONLY
         && ap_admit(AP_L2, AP_PERSON, 1, -1, false, false) == AP_STEP_UP && ap_admit(AP_GUEST, AP_KIND_GUEST, 0, -1, false, false) == AP_OK
         && ap_credentials(true, false, true) == AP_TWO_CREDENTIALS && ap_status(AP_LOG_IN) == 401
         && ap_session(false, 200LL * 86400000LL, 0, 0, 0) == AP_SESSION_ENDED && agents_init()
         && ap_admit(AP_ADMIN_FRESH, AP_PERSON, 2, 400000, true, false) == AP_STEP_UP && ap_admit(AP_RECOVER, AP_PERSON, 1, -1, false, false) == AP_OK
         && ap_limited_admit(AP_L2, false, true) == AP_LIMITED && ap_status(AP_LIMITED) == 401;
  if (!ok || !auth_recover_init() || !auth_life_init() || !passkeys_init() || !auth_device_init() || !wallet_init()) { fprintf(stderr, "api: auth.c and auth-policy.coax disagree on their numbers\n"); exit(2); }
}

// Start of each request (api.c): nothing carries over from the last.
void auth_reset(void) { resolved = false; resolved_for = NULL; set_cookie[0] = 0; memset(&current, 0, sizeof current); }
const char *auth_extra_headers(void) { return set_cookie; }

// The gate: true when it answered (a refusal), false to let the route run.
bool auth_gate(const request *q) {
  const auth_rule *rule = rule_for(q->method, q->path);
  if (!rule) {
    api_log("auth: no rule for %s %s", q->method, q->path);
    api_error(q->fd, 404, "not found");
    return true;
  }
  // A request with an Authorization header is resolved on every route, public ones too, so an
  // agent whose grant ended is refused everywhere, not only where a rule asks who it is.
  char authz[16] = {0};
  bool signed_in = api_header(q, "Authorization", authz, sizeof authz) != NULL;
  if ((rule->rule == AP_PUBLIC || rule->rule == AP_GUEST) && !signed_in) return false;
  const auth_principal *p = auth_principal_of(q);
  // A frozen account (#13214): only looking, unfreezing, deleting, cancelling and logging out.
  if (auth_life_frozen_refused(q, p)) return true;
  // A bearer token of ours that is revoked, rotated or unknown is refused on public routes too.
  if (!p && (rule->rule == AP_PUBLIC || rule->rule == AP_GUEST) && !strncmp(authz, "Bearer oa_", 10)) {
    api_log("auth: refused %s %s: a refused bearer token", q->method, q->path);
    refuse(q, 401, "That key isn't good any more. Ask its owner for a new one.");
    return true;
  }
  if (!p && (rule->rule == AP_PUBLIC || rule->rule == AP_GUEST)) return false;   // the route reads its own signature
  // An agent on a human, L2, L2-fresh or admin route: 403 whatever its grant says.
  if (p && p->kind == AP_AGENT && ap_admit(rule->rule, p->kind, p->level, p->l2_age_ms, p->admin, true) == AP_HUMAN_ONLY) {
    api_log("auth: refused %s %s: rule %s, agent", q->method, q->path, rule_name(rule->rule));
    coax_string w = ap_words(AP_HUMAN_ONLY);
    char hw[200]; snprintf(hw, sizeof hw, "%.*s", (int)w.count, w.data);
    refuse(q, (int)ap_status(AP_HUMAN_ONLY), hw);
    return true;
  }
  if (p && p->kind == AP_AGENT) {
    char words[200];
    int status = agents_admit(q, api_db(), &current, words);
    if (status) { api_log("auth: refused agent %s %s: its grant", q->method, q->path); refuse(q, status, words); return true; }
  }
  if (rule->rule == AP_PUBLIC || rule->rule == AP_GUEST) { if (p && p->kind == AP_AGENT) agents_used(q, api_db(), p); return false; }
  int verdict;
  if (!p) verdict = current.refused == 400 ? AP_TWO_CREDENTIALS : AP_LOG_IN;
  else verdict = (int)ap_admit(rule->rule, p->kind, p->level, p->l2_age_ms, p->admin, p->kind == AP_AGENT && agents_scoped(p, rule->scope));
  bool write = strcmp(q->method, "GET") && strcmp(q->method, "HEAD");
  if (verdict == AP_OK && p) verdict = (int)ap_limited_admit(rule->rule, write, p->limited);
  if (verdict == AP_OK) { if (p->kind == AP_AGENT) agents_used(q, api_db(), p); return false; }
  api_log("auth: refused %s %s: rule %s, %s", q->method, q->path, rule_name(rule->rule),
          !p ? "a refused credential" : p->kind == AP_PERSON ? "person" : p->kind == AP_AGENT ? "agent" : p->kind == AP_SERVICE ? "service" : p->kind == AP_KIND_GUEST ? "guest" : "nobody");
  coax_string w = ap_words(verdict);
  char words[200]; snprintf(words, sizeof words, "%.*s", (int)w.count, w.data);
  // A step-up the page can act on: the level wanted and the account's methods (auth-recover.c).
  if ((verdict == AP_STEP_UP || verdict == AP_LIMITED) && p && p->kind == AP_PERSON) {
    auth_step_up_refusal(q, api_db(), p, (int)ap_need(rule->rule), (int)ap_status(verdict), words);
    return true;
  }
  refuse(q, (int)ap_status(verdict), words);
  return true;
}

// ---------------------------------------------------------------- logging in
// The browser's name for the devices list, from its User-Agent: "Chrome on Mac" and the like.
static void device_name(const request *q, char *out, size_t cap) {
  char ua[512]; api_header(q, "User-Agent", ua, sizeof ua);
  const char *b = strstr(ua, "Edg/") ? "Edge" : strstr(ua, "Firefox/") ? "Firefox" : strstr(ua, "Chrome/") ? "Chrome"
                : strstr(ua, "Safari/") ? "Safari" : strstr(ua, "OpenAgents") ? "OpenAgents app" : "Browser";
  const char *o = strstr(ua, "iPhone") || strstr(ua, "iPad") ? "iPhone" : strstr(ua, "Android") ? "Android" : strstr(ua, "Mac OS") ? "Mac"
                : strstr(ua, "Windows") ? "Windows" : strstr(ua, "Linux") ? "Linux" : "";
  if (*o) snprintf(out, cap, "%s on %s", b, o); else snprintf(out, cap, "%s", b);
}
void auth_device_name(const request *q, char *out, size_t cap) { device_name(q, out, cap); }

// A logged-in answer: a session (cookie for the web, a token pair for an app), the guest's player
// attached, the audit row. made: the account is new. Answers the request.
void auth_logged_in(const request *q, PGconn *db, const char *acct, const char *via_kind, const char *via_id, const char *level,
                    bool app, bool made, const char *action, const char *extra_json, const char *extra_lines) {
  char tok[160], dig[65], name[96], country[8] = "";
  // An app's device key (#13216), checked before there is a session: one we can't hold refuses the login.
  auth_device_ask device;
  if (!auth_device_login(q, db, acct, app, &device)) return;
  device_name(q, name, sizeof name);
  api_header(q, "X-Client-Region", country, sizeof country);
  if (app) { if (!random_hex(tok, 32)) { refuse(q, 503, "No randomness here. Try again."); return; } sha_hex(tok, dig); }
  else if (!new_token("oas_", tok, sizeof tok, dig)) { refuse(q, 503, "No randomness here. Try again."); return; }
  const char *v[8] = {dig, acct, via_kind, via_id, level, app ? "app" : "web", name, country};
  PGresult *r = PQexecParams(db, "SELECT auth.session_new($1, $2, $3, $4::bigint, $5, $6, $7, $8), a.team, a.verse_account "
                                 "FROM auth.accounts a WHERE a.id = $2", 8, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || PQgetisnull(r, 0, 0)) {
    api_log("auth: session for %s: %s", action, PQerrorMessage(db));
    PQclear(r);
    audit_why(db, acct, NULL, action, "refused", "deleted");
    refuse(q, 410, "This account was deleted.");
    return;
  }
  char sid[32], vacct[160]; bool team = !strcmp(PQgetvalue(r, 0, 1), "t");
  snprintf(sid, sizeof sid, "%s", PQgetvalue(r, 0, 0)); snprintf(vacct, sizeof vacct, "%s", PQgetvalue(r, 0, 2));
  PQclear(r);
  // The guest's player joins the account once.
  char player[160], pjson[600] = "null";
  api_header(q, "X-Verse-Player", player, sizeof player);
  if (player[0]) {
    const char *g[2] = {vacct, player};
    PGresult *x = PQexecParams(db, "SELECT auth.attach_guest($1, $2)::text", 2, NULL, g, NULL, NULL, 0);
    if (PQresultStatus(x) == PGRES_TUPLES_OK && PQntuples(x) == 1 && !PQgetisnull(x, 0, 0)) snprintf(pjson, sizeof pjson, "%s", PQgetvalue(x, 0, 0));
    PQclear(x);
  }
  // The session is bound to its device key before any token is made, and never unbound.
  if (device.bind && !auth_device_bind(db, acct, sid, &device)) {
    const char *e[1] = {sid};
    PQclear(PQexecParams(db, "SELECT auth.session_end($1)", 1, NULL, e, NULL, NULL, 0));
    audit_why(db, acct, NULL, action, "failed", "device");
    refuse(q, 500, "That didn't work. Try again.");
    return;
  }
  char detail[200];
  audit(db, acct, sid, action, "ok", detail_of(detail, auw_login_detail_write(cs(via_kind), cs(app ? "app" : "web"), made, device.bind, OUT(detail))));
  if (!strcmp(level, "L2")) auth_recover_strong_login(db, acct, sid);   // a pending recovery ends (#13213)
  if (!extra_json) extra_json = "";
  if (!extra_lines) extra_lines = "";
  char json[1600], lines[600], headers[900];
  if (app) {
    char rt[160], rtd[65], at[160], atd[65], fam[33];
    if (!new_token("oa_rt_", rt, sizeof rt, rtd) || !new_token("oa_at_", at, sizeof at, atd) || !random_hex(fam, 16)) { refuse(q, 503, "No randomness here. Try again."); return; }
    const char *w[4] = {sid, fam, rtd, atd};
    PQclear(PQexecParams(db, "SELECT auth.app_pair($1, $2, $3, $4)", 4, NULL, w, NULL, NULL, 0));
    answer_n(q, 200, NULL, json, auw_login_app_write(cs(acct), made, cs(at), cs(rt), device.bind, cs(pjson), cs(extra_json), OUT(json)),
             lines, auw_login_app_lines(cs(acct), made, cs(at), cs(rt), device.bind, cs(extra_lines), OUT(lines)));
    return;
  }
  set_cookie[0] = 0;   // this answer's own cookie replaces a cleared or rotated one
  snprintf(headers, sizeof headers, "Set-Cookie: %s=%s; Path=/; Max-Age=%d; HttpOnly; Secure; SameSite=Lax\r\n"
           "Set-Cookie: %s=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n%s",
           SESSION_COOKIE, tok, SESSION_MAX_AGE, PENDING_COOKIE, team ? TEAM_COOKIE : "");
#ifdef OA_LOCAL_SERVE
  strncat(headers, "Set-Cookie: " LOCAL_OUT_COOKIE "=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax\r\n", sizeof headers - strlen(headers) - 1);
#endif
  answer_n(q, 200, headers, json, auw_login_web_write(cs(acct), made, cs(pjson), cs(extra_json), OUT(json)),
           lines, auw_login_web_lines(cs(acct), made, cs(extra_lines), OUT(lines)));
}

// Our Origin on a cookie write (as every route here); apps and tools send no Origin.
static bool same_site(const request *q) {
  char from[256];
  if (!api_header(q, "Origin", from, sizeof from)) return true;
  return !strcmp(from, origin());
}

static char *body_text(const request *q) {
  char *b = malloc(q->body_len + 1);
  if (!b) return NULL;
  memcpy(b, q->body, q->body_len); b[q->body_len] = 0;
  return b;
}

// ---- email
// The 6-digit code a link token stands for: the same message carries both, so either one shown is
// no more than the email already said.
static void code_of(const char *link_token, char code[7]) {
  char s[200], h[65]; snprintf(s, sizeof s, "code:%s", link_token); sha_hex(s, h);
  unsigned long v = strtoul((char[9]){h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 0}, NULL, 16);
  snprintf(code, 7, "%06lu", v % 1000000UL);
}

static void email_start(const request *q, PGconn *db, const char *body) {
  char raw[400], email[300], client[16] = "web";
  auw_ask_client(cs(body), ASK(client));
  bool app = !strcmp(client, "app");
  bool link = auw_ask_link(cs(body));
  bool merge = auw_ask_merge(cs(body));   // the other account's email proves it to a merge (auth-life.c)
  if (auw_ask_email(cs(body), ASK(raw)) < 0) { refuse(q, 400, "Type your email address."); return; }
  int64_t n = as_email_normalize((coax_v_u8){(uint8_t *)raw, (int64_t)strlen(raw)}, (coax_v_u8){(uint8_t *)email, (int64_t)sizeof email - 1});
  if (n < 0) { refuse(q, 400, "That isn't an email address."); return; }
  email[n] = 0;
  if (!sealing_ready()) { refuse(q, 503, "Email login isn't set up here yet."); return; }
  char all[400], lookup[80];
  if (!lookups(email, all, sizeof all, lookup)) { refuse(q, 503, "Email login isn't set up here yet."); return; }
  // Linking needs a logged-in account; logging in needs none.
  const auth_principal *p = auth_principal_of(q);
  if (link && (!p || !p->account[0])) { refuse(q, 401, "Log in first."); return; }
  if (link && p->frozen) { refuse(q, 423, "This account is frozen. Unfreeze it at openagents.com/settings/data first."); return; }   // a login, yes; a new way in, no (#13214)
  if (link && !auth_recover_may_link(q, db, p)) return;
  if (merge && (!p || !p->account[0])) { refuse(q, 401, "Log in to the account that stays first."); return; }
  if (merge) link = false;
  // Limits: per address (hour and day) and per network address; a new address is a new account.
  long long until = 0; char ipk[65]; visitor_key(q, ipk);
  if (!limit_hit(db, L_EMAIL_IP, ipk, &until)) { refuse_limit(q, L_EMAIL_IP, until); return; }
  if (!limit_hit(db, L_EMAIL_HOUR, lookup, &until)) { refuse_limit(q, L_EMAIL_HOUR, until); return; }
  if (!limit_hit(db, L_EMAIL_DAY, lookup, &until)) { refuse_limit(q, L_EMAIL_DAY, until); return; }
  const char *lv[1] = {all};
  PGresult *r = PQexecParams(db, "SELECT EXISTS (SELECT 1 FROM auth.emails WHERE lookup = ANY($1::text[]))", 1, NULL, lv, NULL, NULL, 0);
  bool known = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t");
  PQclear(r);
  if (merge && !known) { refuse(q, 404, "No account has that email."); return; }
  if (!known && !link && !merge && !limit_hit(db, L_NEW_ACCOUNT, ipk, &until)) { refuse_limit(q, L_NEW_ACCOUNT, until); return; }
  // The pending login: the code and link only as digests; the address sealed to this row.
  char id[33], link_token[65], code[7], pending[65], code_sha[65], link_sha[65], pend_sha[65], sealed[1200], ad[96], tmp[120];
  if (!random_hex(id, 16) || !random_hex(link_token, 32) || !random_hex(pending, 32)) { refuse(q, 503, "No randomness here. Try again."); return; }
  code_of(link_token, code);
  snprintf(tmp, sizeof tmp, "%s%s", code, id); sha_hex(tmp, code_sha);
  sha_hex(link_token, link_sha); sha_hex(pending, pend_sha);
  snprintf(ad, sizeof ad, "auth.pending|target_sealed|%s", id);
  if (!seal(ad, email, sealed, sizeof sealed)) { refuse(q, 503, "Email login isn't set up here yet."); return; }
  const char *v[9] = {id, lookup, sealed, code_sha, link_sha, pend_sha, app ? "app" : "web", merge ? "merge" : link ? "link" : "login", link || merge ? p->account : NULL};
  r = PQexecParams(db, "SELECT auth.email_start($1, $2, $3, $4, $5, $6, $7, $8, $9)", 9, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK;
  if (!ok) api_log("auth: email start: %s", PQerrorMessage(db));
  PQclear(r);
  if (!ok) { refuse(q, 500, "That didn't work. Try again."); return; }
  // One message: the code, and the link for the same browser. Short, plain. The code is six digits
  // unbroken, so a double click or a tap selects all of it (owner, 2026-11-13).
  char message[2400], url[400];
  snprintf(url, sizeof url, "%s/login/email#t=%s", origin(), link_token);
  int64_t mn = auw_mail_write(cs(email), cs(code), cs(id), cs(url), OUT(message));
  if (mn < 0) { memset(message, 0, sizeof message); memset(code, 0, sizeof code); memset(link_token, 0, sizeof link_token); refuse(q, 500, "That didn't work. Try again."); return; }
  message[mn] = 0;
  char sent_id[128], err[300];
#ifdef OA_LOCAL_SERVE
  // The dev server sends no mail: the code goes to its own log, which serve.sh shows in its terminal.
  int s = 0; (void)sent_id; (void)err;
  api_log("auth: dev server login code for %s: %.6s (it works 10 minutes)", email, code);
#else
  int s = email_send(db, message, sent_id, err, sizeof err);
#endif
  memset(message, 0, sizeof message); memset(code, 0, sizeof code); memset(link_token, 0, sizeof link_token);
  if (s > 1) {
    audit_why(db, link ? p->account : NULL, NULL, "email.code", "failed", "not sent");
    refuse(q, s == 400 ? 400 : 502, s == 400 ? "We can't send email to that address." : "The email didn't send. Try again.");
    return;
  }
  char purpose[64];
  audit(db, link || merge ? p->account : NULL, link || merge ? p->session : NULL, "email.code", "ok",
        detail_of(purpose, auw_purpose_write(cs(merge ? "merge" : link ? "link" : "login"), OUT(purpose))));
  char headers[300] = "", json[400], lines[300];
  // An app gets the pending value in the answer; a browser gets it as a cookie.
  if (!app) snprintf(headers, sizeof headers, "Set-Cookie: %s=%s; Path=/; Max-Age=600; HttpOnly; Secure; SameSite=Lax\r\n", PENDING_COOKIE, pending);
  answer_n(q, 200, headers, json, auw_sent_write(cs(app ? pending : ""), OUT(json)), lines, auw_sent_lines(cs(app ? pending : ""), OUT(lines)));
}

// The pending value: the cookie in a browser, the body's "pending" from an app.
static bool pending_sha(const request *q, const char *body, char out[65]) {
  char v[80];
  cookie(q, PENDING_COOKIE, v, sizeof v);
  if (!v[0]) auw_ask_pending(cs(body), ASK(v));
  if (strlen(v) != 64) return false;
  sha_hex(v, out);
  return true;
}

// Finishes a proved email (code or link): the account it logs in to (a new one for a new address)
// or the account it links to. Answers.
static void email_finish(const request *q, PGconn *db, PGresult *r) {
  // columns: state, id, sealed, lookup, purpose, account, client
  char id[40], sealed_p[1200], lookup[80], purpose[8], link_acct[40], client[8], email[300], ad[96];
  snprintf(id, sizeof id, "%s", PQgetvalue(r, 0, 1)); snprintf(sealed_p, sizeof sealed_p, "%s", PQgetvalue(r, 0, 2));
  snprintf(lookup, sizeof lookup, "%s", PQgetvalue(r, 0, 3)); snprintf(purpose, sizeof purpose, "%s", PQgetvalue(r, 0, 4));
  snprintf(link_acct, sizeof link_acct, "%s", PQgetvalue(r, 0, 5)); snprintf(client, sizeof client, "%s", PQgetvalue(r, 0, 6));
  if (!strcmp(purpose, "merge")) { auth_life_merge_email(q, db, link_acct, lookup); return; }   // #13214
  snprintf(ad, sizeof ad, "auth.pending|target_sealed|%s", id);
  if (!unseal(ad, sealed_p, email, sizeof email)) { refuse(q, 500, "That didn't work. Ask for a new code."); return; }
  char lower[300], team_sha[65];   // team.conf names team emails by the SHA-256 of the lower-cased address
  size_t k = 0; for (; email[k] && k + 1 < sizeof lower; k++) lower[k] = (char)tolower((unsigned char)email[k]);
  lower[k] = 0;
  sha_hex(lower, team_sha); memset(lower, 0, sizeof lower);
  const char *tv[1] = {team_sha};
  PGresult *t = PQexecParams(db, "SELECT EXISTS (SELECT 1 FROM analytics.team WHERE kind = 'email_sha256' AND value = $1)", 1, NULL, tv, NULL, NULL, 0);
  bool team = PQresultStatus(t) == PGRES_TUPLES_OK && PQntuples(t) == 1 && !strcmp(PQgetvalue(t, 0, 0), "t");
  PQclear(t);
  PQclear(PQexec(db, "BEGIN"));
  char acct[40], email_id[24]; bool made = false;
  if (!strcmp(purpose, "link")) {
    const char *v[2] = {link_acct, lookup};
    PGresult *x = PQexecParams(db, "SELECT state, email_id FROM auth.email_attach($1, $2)", 2, NULL, v, NULL, NULL, 0);
    bool ok = PQresultStatus(x) == PGRES_TUPLES_OK && PQntuples(x) == 1;
    char state[16] = ""; if (ok) { snprintf(state, sizeof state, "%s", PQgetvalue(x, 0, 0)); snprintf(email_id, sizeof email_id, "%s", PQgetvalue(x, 0, 1)); }
    PQclear(x);
    if (!ok || strcmp(state, "ok")) {
      PQclear(PQexec(db, "ROLLBACK"));
      audit_why(db, link_acct, NULL, "email.link", "refused", "another account's");
      if (!strcmp(state, "already")) { answer_state(q, 200, NULL, "linked"); return; }
      refuse(q, 409, "This email already has an account. Log in to it to merge.");
      return;
    }
    snprintf(acct, sizeof acct, "%s", link_acct);
  } else {
    const char *v[2] = {lookup, team ? "t" : "f"};
    PGresult *x = PQexecParams(db, "SELECT acct, email_id, made FROM auth.email_account($1, $2::boolean)", 2, NULL, v, NULL, NULL, 0);
    bool ok = PQresultStatus(x) == PGRES_TUPLES_OK && PQntuples(x) == 1;
    if (ok) { snprintf(acct, sizeof acct, "%s", PQgetvalue(x, 0, 0)); snprintf(email_id, sizeof email_id, "%s", PQgetvalue(x, 0, 1)); made = !strcmp(PQgetvalue(x, 0, 2), "t"); }
    else api_log("auth: email account: %s", PQerrorMessage(db));
    PQclear(x);
    if (!ok) { PQclear(PQexec(db, "ROLLBACK")); refuse(q, 500, "That didn't work. Try again."); return; }
  }
  // Sealed to its account (auth.emails|sealed|<account>), in the same transaction as the row.
  if (made || !strcmp(purpose, "link")) {
    char sealed[1200], ad2[96];
    snprintf(ad2, sizeof ad2, "auth.emails|sealed|%s", acct);
    bool ok = seal(ad2, email, sealed, sizeof sealed);
    if (ok) { const char *v[2] = {email_id, sealed}; PGresult *x = PQexecParams(db, "SELECT auth.email_seal($1::bigint, $2)", 2, NULL, v, NULL, NULL, 0); ok = PQresultStatus(x) == PGRES_TUPLES_OK; PQclear(x); }
    if (!ok) { PQclear(PQexec(db, "ROLLBACK")); refuse(q, 500, "That didn't work. Try again."); return; }
  }
  PQclear(PQexec(db, "COMMIT"));
  memset(email, 0, sizeof email);
  if (!strcmp(purpose, "link")) {
    audit(db, acct, NULL, "email.link", "ok", NULL);
    auth_recover_linked_email(db, acct, NULL);
    answer_state(q, 200, "Set-Cookie: " PENDING_COOKIE "=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax\r\n", "linked");
    return;
  }
  const char *level = "L1";
#ifdef OA_LOCAL_SERVE
  // The dev server: the local admin's own account by its email's code is L2, as this computer's requests are
  // with no login at all (local_admin), so the owner who logged out and back in is its admin again.
  if (local_owner_account(db, acct)) level = "L2";
#endif
  auth_logged_in(q, db, acct, "email", email_id, level, !strcmp(client, "app"), made, "login.email", NULL, NULL);
}

static void email_verify(const request *q, PGconn *db, const char *body) {
  char code[16], psha[65];
  auw_ask_code(cs(body), ASK(code));
  // "482 913" as typed: the space goes.
  char c[8]; size_t k = 0;
  for (const char *s = code; *s && k < 7; s++) if (*s != ' ') c[k++] = *s;
  c[k] = 0;
  if (!ap_code_ok((coax_string){c, (int64_t)k})) { refuse(q, 400, "Type the 6 digits from the email."); return; }
  if (!pending_sha(q, body, psha)) { refuse(q, 409, "Ask for a code in this browser first."); return; }
  const char *v[2] = {psha, c};
  PGresult *r = PQexecParams(db, "SELECT state, id, sealed, lookup, purpose, account, client, tries_left FROM auth.email_code($1, $2)", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { api_log("auth: email code: %s", PQerrorMessage(db)); PQclear(r); refuse(q, 500, "That didn't work. Try again."); return; }
  const char *state = PQgetvalue(r, 0, 0);
  if (!strcmp(state, "ok")) { email_finish(q, db, r); PQclear(r); return; }
  if (!strcmp(state, "wrong")) {
    int left = atoi(PQgetvalue(r, 0, 7)); PQclear(r);
    audit_why(db, NULL, NULL, "login.email", "refused", "wrong code");
    char words[120]; snprintf(words, sizeof words, "That code isn't right. %d %s left.", left, left == 1 ? "try" : "tries");
    refuse(q, 401, words);
    return;
  }
  bool dead = !strcmp(state, "dead");
  PQclear(r);
  audit_why(db, NULL, NULL, "login.email", "refused", dead ? "code spent" : "no code");
  refuse(q, dead ? 410 : 409, dead ? "That code has ended: 5 tries or 10 minutes. Ask for a new one." : "Ask for a code in this browser first.");
}

static void email_link(const request *q, PGconn *db, const char *body) {
  char t[80], tsha[65], psha[65] = "";
  if (auw_ask_t(cs(body), ASK(t)) < 0 || strlen(t) != 64) { refuse(q, 400, "That link isn't whole. Type the code instead."); return; }
  sha_hex(t, tsha);
  pending_sha(q, body, psha);
  const char *v[2] = {tsha, psha[0] ? psha : NULL};
  PGresult *r = PQexecParams(db, "SELECT state, id, sealed, lookup, purpose, account, client FROM auth.email_link($1, $2)", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); refuse(q, 500, "That didn't work. Try again."); return; }
  const char *state = PQgetvalue(r, 0, 0);
  if (!strcmp(state, "ok")) { email_finish(q, db, r); PQclear(r); return; }
  if (!strcmp(state, "elsewhere")) {
    PQclear(r);
    char code[7], json[200], lines[200];
    code_of(t, code);
    answer_n(q, 409, NULL, json, auw_elsewhere_write(cs(code), OUT(json)), lines, auw_elsewhere_lines(cs(code), OUT(lines)));
    return;
  }
  bool dead = !strcmp(state, "dead");
  PQclear(r);
  refuse(q, 410, dead ? "That link has ended. Ask for a new code." : "That link was used or has ended. Ask for a new code.");
}

// ---- Nostr
static void nostr_challenge(const request *q, PGconn *db, const char *body) {
  char client[16] = "web", id[33], ch[65], chsha[65];
  auw_ask_client(cs(body), ASK(client));
  if (!random_hex(id, 16) || !random_hex(ch, 32)) { refuse(q, 503, "No randomness here. Try again."); return; }
  sha_hex(ch, chsha);
  const char *v[3] = {id, chsha, !strcmp(client, "app") ? "app" : "web"};
  PGresult *r = PQexecParams(db, "SELECT auth.nostr_challenge($1, $2, $3)", 3, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK; PQclear(r);
  if (!ok) { refuse(q, 500, "That didn't work. Try again."); return; }
  char json[400], lines[300];
  answer_n(q, 200, NULL, json, auw_challenge_write(cs(ch), cs(origin()), OUT(json)), lines, auw_challenge_lines(cs(ch), cs(origin()), OUT(lines)));
}

// The account a key logs in to (made on first login, D8) and its key row's id. A second statement
// for the id: the function's insert isn't visible to the statement that called it.
static bool key_account(PGconn *db, const char *pk, const char *label, char acct[40], char kid[24], bool *made) {
  const char *v[2] = {pk, label};
  PGresult *r = PQexecParams(db, "SELECT acct, made FROM auth.account_for_key($1, $2)", 2, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  if (ok) { snprintf(acct, 40, "%s", PQgetvalue(r, 0, 0)); *made = !strcmp(PQgetvalue(r, 0, 1), "t"); }
  else api_log("auth: key account: %s", PQerrorMessage(db));
  PQclear(r);
  if (!ok) return false;
  r = PQexecParams(db, "SELECT id FROM auth.nostr_keys WHERE pubkey = $1", 1, NULL, v, NULL, NULL, 0);
  ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  if (ok) snprintf(kid, 24, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  return ok;
}

// The account a key logs in to (made on first login, D8), then logged in at L2.
static void key_login(const request *q, PGconn *db, const char *pk, const char *label, bool app, const char *action) {
  const char *kv[1] = {pk};
  PGresult *r = PQexecParams(db, "SELECT EXISTS (SELECT 1 FROM auth.nostr_keys WHERE pubkey = $1)", 1, NULL, kv, NULL, NULL, 0);
  bool known = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t");
  PQclear(r);
  long long until = 0; char ipk[65]; visitor_key(q, ipk);
  if (!known && !limit_hit(db, L_NEW_ACCOUNT, ipk, &until)) { refuse_limit(q, L_NEW_ACCOUNT, until); return; }
  char acct[40], kid[24]; bool made = false;
  if (!key_account(db, pk, label, acct, kid, &made)) { refuse(q, 500, "That didn't work. Try again."); return; }
  auth_logged_in(q, db, acct, "nostr", kid, "L2", app, made, action, NULL, NULL);
}

static void nostr_login(const request *q, PGconn *db, const char *body) {
  char ch[80], chsha[65];
  long long until = 0; char ipk[65]; visitor_key(q, ipk);
  if (!limit_hit(db, L_KEY_LOGIN, ipk, &until)) { refuse_limit(q, L_KEY_LOGIN, until); return; }
  const auth_principal *p = auth_principal_of(q);
  if (!p) { refuse(q, current.refused == 400 ? 400 : 401, current.refused == 400 ? "Log out first, or send only the signature." : "That signature isn't good for this request."); return; }
  if (p->credential != AUTH_NOSTR) { refuse(q, 401, "Sign the challenge with your key (NIP-98)."); return; }
  if (!p->payload) { refuse(q, 401, "Sign the body too (a payload tag)."); return; }
  if (p->kind == AP_AGENT) { refuse(q, 403, "An agent signs each request with its key; it doesn't log in."); return; }
  if (auw_ask_challenge(cs(body), ASK(ch)) < 0 || strlen(ch) != 64) { refuse(q, 400, "Ask for a challenge first."); return; }
  sha_hex(ch, chsha);
  const char *v[2] = {chsha, p->event_id};
  PGresult *r = PQexecParams(db, "SELECT state, client FROM auth.nostr_spend($1, $2)", 2, NULL, v, NULL, NULL, 0);
  char state[16] = "", client[8] = "web";
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) { snprintf(state, sizeof state, "%s", PQgetvalue(r, 0, 0)); snprintf(client, sizeof client, "%s", PQgetvalue(r, 0, 1)); }
  PQclear(r);
  if (strcmp(state, "ok")) {
    audit_why(db, p->account, NULL, "login.nostr", "refused", !strcmp(state, "replay") ? "replay" : "no challenge");
    refuse(q, 401, !strcmp(state, "replay") ? "That signature was used already." : "That challenge has ended. Ask for a new one.");
    return;
  }
  char label[32] = "";
  auw_ask_label(cs(body), ASK(label));
  if (strcmp(label, "extension") && strcmp(label, "bunker") && strcmp(label, "phone") && strcmp(label, "app")) label[0] = 0;
  char pk[65]; memcpy(pk, p->pubkey, 65);
  key_login(q, db, pk, label, !strcmp(client, "app"), "login.nostr");
}

// ---- the phone approves the web
static void phone_start(const request *q, PGconn *db) {
  char id[33], ch[65], poll[65], pend[65], chsha[65], pollsha[65], pendsha[65];
  if (!random_hex(id, 16) || !random_hex(ch, 32) || !random_hex(poll, 32) || !random_hex(pend, 32)) { refuse(q, 503, "No randomness here. Try again."); return; }
  sha_hex(ch, chsha); sha_hex(poll, pollsha); sha_hex(pend, pendsha);
  const char *v[4] = {id, chsha, pollsha, pendsha};
  PGresult *r = PQexecParams(db, "SELECT auth.phone_start($1, $2, $3, $4)", 4, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK; PQclear(r);
  if (!ok) { refuse(q, 500, "That didn't work. Try again."); return; }
  char headers[300], json[600], lines[500];
  snprintf(headers, sizeof headers, "Set-Cookie: %s=%s; Path=/; Max-Age=300; HttpOnly; Secure; SameSite=Lax\r\n", PENDING_COOKIE, pend);
  answer_n(q, 200, headers, json, auw_phone_write(cs(ch), cs(poll), cs(origin()), OUT(json)), lines, auw_phone_lines(cs(ch), cs(poll), cs(origin()), OUT(lines)));
}
static void phone_approve(const request *q, PGconn *db, const char *body) {
  const auth_principal *p = auth_principal_of(q);
  if (!p || p->credential != AUTH_NOSTR || !p->payload) { refuse(q, 401, "Sign the approval with the app's key (NIP-98, with the body)."); return; }
  const char *e[1] = {p->event_id};
  PGresult *r = PQexecParams(db, "SELECT auth.nip98_once($1)", 1, NULL, e, NULL, NULL, 0);
  bool once = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "t");
  PQclear(r);
  if (!once) { refuse(q, 401, "That signature was used already."); return; }
  char ch[80], chsha[65];
  if (auw_ask_challenge(cs(body), ASK(ch)) < 0 || strlen(ch) != 64) { refuse(q, 400, "Scan the code on the page again."); return; }
  sha_hex(ch, chsha);
  const char *v[2] = {chsha, p->pubkey};
  r = PQexecParams(db, "SELECT auth.phone_approve($1, $2)", 2, NULL, v, NULL, NULL, 0);
  char state[16] = ""; if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) snprintf(state, sizeof state, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  if (!strcmp(state, "ok")) { audit(db, p->account, NULL, "login.phone_approve", "ok", NULL); answer_state(q, 200, NULL, "approved"); return; }
  refuse(q, !strcmp(state, "already") ? 409 : 410, !strcmp(state, "already") ? "That code was approved already." : "That code has ended. Show a new one on the page.");
}
static void phone_poll(const request *q, PGconn *db, const char *body) {
  char poll[80], pollsha[65], psha[65];
  if (auw_ask_poll(cs(body), ASK(poll)) < 0 || strlen(poll) != 64) { refuse(q, 400, "Show a code first."); return; }
  if (!pending_sha(q, body, psha)) { refuse(q, 409, "Show a code in this browser first."); return; }
  sha_hex(poll, pollsha);
  const char *v[2] = {pollsha, psha};
  PGresult *r = PQexecParams(db, "SELECT state, coalesce(pubkey, '') FROM auth.phone_poll($1, $2)", 2, NULL, v, NULL, NULL, 0);
  char state[16] = "", pk[65] = "";
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) { snprintf(state, sizeof state, "%s", PQgetvalue(r, 0, 0)); snprintf(pk, sizeof pk, "%s", PQgetvalue(r, 0, 1)); }
  PQclear(r);
  if (!strcmp(state, "waiting")) { answer_state(q, 200, NULL, "waiting"); return; }
  if (!strcmp(state, "ok")) { key_login(q, db, pk, "phone", false, "login.phone"); return; }
  refuse(q, 410, "That code has ended. Show a new one.");
}

// ---- app tokens
static void refresh(const request *q, PGconn *db, const char *body) {
  char rt[200], dig[65];
  if (auw_ask_refresh_token(cs(body), ASK(rt)) < 0 || !token_ok(rt, TOKEN_REFRESH, dig)) { refuse(q, 401, "Log in again."); return; }
  long long until = 0;
  // Limit per family: the family is found from the token; a digest stands in until then.
  char nrt[160], nrtd[65], nat[160], natd[65];
  if (!new_token("oa_rt_", nrt, sizeof nrt, nrtd) || !new_token("oa_at_", nat, sizeof nat, natd)) { refuse(q, 503, "No randomness here. Try again."); return; }
  auth_refresh_facts facts;
  char proved[24] = "";
  if (auth_device_look(db, dig, &facts)) {
    if (!limit_hit(db, L_REFRESH, facts.family, &until)) { refuse_limit(q, L_REFRESH, until); return; }
    // A login bound to a device key (#13216): its own key's fresh proof, or nothing moves and
    // the token stays good for the real device. auth-device-policy.coax decides.
    if (!auth_device_refresh(q, db, dig, &facts, proved)) return;
  }
  const char *v[4] = {dig, nrtd, natd, proved[0] ? proved : NULL};
  PGresult *r = PQexecParams(db, "SELECT state, coalesce(account, '') FROM auth.app_refresh($1, $2, $3, $4::bigint)", 4, NULL, v, NULL, NULL, 0);
  char state[16] = "", acct[40] = "";
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) { snprintf(state, sizeof state, "%s", PQgetvalue(r, 0, 0)); snprintf(acct, sizeof acct, "%s", PQgetvalue(r, 0, 1)); }
  PQclear(r);
  if (!strcmp(state, "ok")) {
    char json[600], lines[500];
    answer_n(q, 200, NULL, json, auw_pair_write(cs(nat), cs(nrt), OUT(json)), lines, auw_pair_lines(cs(nat), cs(nrt), OUT(lines)));
    return;
  }
  if (!strcmp(state, "proof")) { auth_device_refuse_unproved(q); return; }   // the database's own refusal: a bound token, no proved device
  if (!strcmp(state, "reused")) {
    // Someone copied a refresh token: the whole login ended. The notice to the account's other
    // ways in comes with linking notices (#13213); the audit row is here now.
    char ended[64];
    audit(db, acct, NULL, "refresh.reused", "refused", detail_of(ended, auw_ended_write(cs("family"), OUT(ended))));
    refuse(q, 401, "This login was ended because its token was used twice. Log in again.");
    return;
  }
  refuse(q, 401, "Log in again.");
}

// ---- sessions
static void logout(const request *q, PGconn *db) {
  const auth_principal *p = auth_principal_of(q);
  char headers[400]; cookie_header(headers, sizeof headers, SESSION_COOKIE, "", 0);
#ifdef OA_LOCAL_SERVE
  // The dev server: this browser stays logged out (local_admin reads the mark) until it logs in again.
  strncat(headers, "Set-Cookie: " LOCAL_OUT_COOKIE "=1; Path=/; Max-Age=31536000; HttpOnly; SameSite=Lax\r\n", sizeof headers - strlen(headers) - 1);
#endif
  if (p && p->session[0]) {
    const char *v[1] = {p->session};
    PQclear(PQexecParams(db, "SELECT auth.session_end($1)", 1, NULL, v, NULL, NULL, 0));
    audit(db, p->account, p->session, "logout", "ok", NULL);
  }
  set_cookie[0] = 0;
  answer_state(q, 200, headers, "logged-out");
}
static void logout_everywhere(const request *q, PGconn *db, const char *body) {
  const auth_principal *p = auth_principal_of(q);
  const char *v[1] = {p->account};
  PGresult *r = PQexecParams(db, "SELECT auth.sessions_end_all($1)", 1, NULL, v, NULL, NULL, 0);
  int n = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 ? atoi(PQgetvalue(r, 0, 0)) : 0;
  PQclear(r);
  // {"agents": true}: every agent of the account is paused too, until resumed (agents.c).
  int paused = auw_ask_agents(cs(body)) ? agents_pause_all(db, p->account) : 0;
  char d[64];
  audit(db, p->account, p->session, "logout.everywhere", "ok", detail_of(d, auw_ended_all_write(n, paused, OUT(d))));
  char headers[200], json[80], lines[80]; cookie_header(headers, sizeof headers, SESSION_COOKIE, "", 0);
  set_cookie[0] = 0;
  answer_n(q, 200, headers, json, auw_logout_all_write(n, OUT(json)), lines, auw_logout_all_lines(n, OUT(lines)));
}
static void sessions(const request *q, PGconn *db) {
  const auth_principal *p = auth_principal_of(q);
  const char *v[2] = {p->account, p->session};
  PGresult *r = PQexecParams(db, "SELECT auth.sessions_of($1, $2)::text", 2, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); refuse(q, 500, "That didn't work. Try again."); return; }
  size_t n = PQgetlength(r, 0, 0) + 32;
  char *json = malloc(n);
  if (json) { answer_n(q, 200, NULL, json, auw_sessions_write(cs(PQgetvalue(r, 0, 0)), (coax_v_u8){(uint8_t *)json, (int64_t)n - 1}), NULL, -1); free(json); }
  PQclear(r);
}
static void session_remove(const request *q, PGconn *db) {
  const auth_principal *p = auth_principal_of(q);
  char sid[40];
  if (sscanf(q->path, "/api/auth/sessions/%39[a-z0-9_]/remove", sid) != 1) { refuse(q, 404, "Not found."); return; }
  const char *v[2] = {sid, p->account};
  PGresult *r = PQexecParams(db, "DELETE FROM auth.sessions WHERE id = $1 AND account = $2", 2, NULL, v, NULL, NULL, 0);
  bool gone = PQresultStatus(r) == PGRES_COMMAND_OK && !strcmp(PQcmdTuples(r), "1");
  PQclear(r);
  if (!gone) { refuse(q, 404, "No such session."); return; }
  audit(db, p->account, p->session, "session.remove", "ok", NULL);
  answer_state(q, 200, NULL, "removed");
}
static void freeze(const request *q, PGconn *db, const char *body) {
  char t[80] = "", tsha[65], acct[40] = "";
  if (auw_ask_t(cs(body), ASK(t)) >= 0 && strlen(t) == 64) {
    sha_hex(t, tsha);
    const char *v[1] = {tsha};
    PGresult *r = PQexecParams(db, "SELECT auth.freeze_by_link($1)", 1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0)) snprintf(acct, sizeof acct, "%s", PQgetvalue(r, 0, 0));
    PQclear(r);
    if (!acct[0]) { refuse(q, 410, "That link was used or has ended. Log in to freeze your account."); return; }
  } else {
    const auth_principal *p = auth_principal_of(q);
    if (!p || !p->account[0]) { refuse(q, 401, "Log in first, or use the link in the email."); return; }
    if (p->kind != AP_PERSON) { refuse(q, 403, "Only a person can do this, not an agent."); return; }
    snprintf(acct, sizeof acct, "%s", p->account);
    const char *v[1] = {acct};
    PQclear(PQexecParams(db, "SELECT auth.freeze($1)", 1, NULL, v, NULL, NULL, 0));
  }
  audit(db, acct, NULL, "freeze", "ok", NULL);
  auth_life_froze(db, acct, t[0] != 0 && strlen(t) == 64);   // its agents pause; every way in is told (#13214)
  char headers[200]; cookie_header(headers, sizeof headers, SESSION_COOKIE, "", 0);
  set_cookie[0] = 0;
  answer_state(q, 200, headers, "frozen");
}
// "This wasn't me": a new link for acct (for notices). The token goes into the notice only.
bool auth_freeze_link(PGconn *db, const char *acct, char token[65]) {
  char sha[65];
  if (!random_hex(token, 32)) return false;
  sha_hex(token, sha);
  const char *v[2] = {acct, sha};
  PGresult *r = PQexecParams(db, "SELECT auth.freeze_link($1, $2)", 2, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK;
  PQclear(r);
  return ok;
}

static void me(const request *q, PGconn *db) {
  const auth_principal *p = auth_principal_of(q);
  // A bad, forged or expired NIP-98 signature is refused, never read as nobody (INVARIANTS.md).
  char authz[8] = {0};
  if (!p && api_header(q, "Authorization", authz, sizeof authz) && !strncmp(authz, "Nostr ", 6)) { refuse(q, 401, "That signature isn't good. Sign this request again."); return; }
  if (!p || !p->account[0]) {
    // A key that signs without an account, or nobody: not logged in (a 200, so pages ask without a console error).
    char json[32], lines[32];
    answer_n(q, 200, NULL, json, auw_nobody_write(OUT(json)), lines, auw_nobody_lines(OUT(lines)));
    return;
  }
  const char *v[1] = {p->account};
  PGresult *r = PQexecParams(db, "SELECT x->>'kind', x->'emails', x->'keys', (x->>'team')::boolean FROM auth.me_of($1) x", 1, NULL, v, NULL, NULL, 0);
  if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1) { PQclear(r); refuse(q, 500, "That didn't work. Try again."); return; }
  char *json = malloc(16384), *lines = malloc(8192);
  if (!json || !lines) { free(json); free(lines); PQclear(r); refuse(q, 500, "Out of memory."); return; }
  coax_v_u8 jv = {(uint8_t *)json, 16383}, lv = {(uint8_t *)lines, 8191};
  int64_t j = auw_me_head_write(cs(p->account), cs(PQgetvalue(r, 0, 0)), p->level, cs(p->via), cs(p->session), p->admin, jv);
  int64_t l = auw_me_head_lines(cs(p->account), cs(PQgetvalue(r, 0, 0)), p->level, cs(p->via), p->admin, lv);
  // Emails, opened and masked: a•••@example.com. The list never carries an address whole; the account's
  // primary email (else its first) comes whole once, as "primary", for the account's own name on the
  // page (the side's foot shows it, owner 2026-11-13): the caller is that account's own login.
  const char *emails = PQgetvalue(r, 0, 1);
  int count = 0;
  char primary[300] = ""; bool primary_marked = false;
  for (int64_t at = auw_me_sealed_at(cs(emails), 0); at >= 0; at = auw_me_sealed_at(cs(emails), at + 1)) {
    int64_t len = auw_me_sealed_len(cs(emails), at); if (len < 0) break;
    const char *e = emails + at + len;   // the sealed value's closing quote
    char sealed[1200], email[300], masked[320], ad[96];
    snprintf(sealed, sizeof sealed, "%.*s", (int)len, emails + at);
    snprintf(ad, sizeof ad, "auth.emails|sealed|%s", p->account);
    if (!unseal(ad, sealed, email, sizeof email)) continue;
    const char *close = strchr(e, '}');
    const char *mark = strstr(e, "\"primary\": true");
    bool is_primary = mark && close && mark < close;
    if (!primary[0] || (is_primary && !primary_marked)) { snprintf(primary, sizeof primary, "%s", email); primary_marked = is_primary; }
    mask_email(email, masked, sizeof masked);
    memset(email, 0, sizeof email);
    j = auw_me_email_write(cs(masked), count++ == 0, jv, j);
    l = auw_me_email_lines(cs(masked), lv, l);
  }
  const char *keys = PQgetvalue(r, 0, 2);
  if (primary[0]) l = auw_me_primary_lines(cs(primary), lv, l);
  j = auw_me_tail_write(cs(primary), cs(keys), jv, j);
  memset(primary, 0, sizeof primary);
  for (int64_t at = auw_me_pubkey_at(cs(keys), 0); at >= 0 && l >= 0 && l < 8000; at = auw_me_pubkey_at(cs(keys), at + 1))
    l = auw_me_key_lines((coax_string){keys + at, (int64_t)strnlen(keys + at, 64)}, lv, l);
  answer_n(q, 200, NULL, json, j, lines, l);
  PQclear(r);
  free(json); free(lines);
}

// GET /api/auth/viewer and /api/auth/me: who is logged in, as the pages read it (coax/web/host.js
// viewer(); auth.viewer_lines, migration 0158). The key is the one the principal acts as: a session's
// account key (none while limited or frozen) or a NIP-98 signer. An account with no such key is
// U|account, so a page knows someone is logged in. Nobody: viewer is a 200 with an empty body (every
// page asks it), me a 401.
static void viewer(const request *q, PGconn *db, bool quiet) {
  const auth_principal *p = auth_principal_of(q);
  const char *head = "Cache-Control: no-store\r\nVary: Cookie, Authorization\r\n";
  char *out = NULL; int64_t on = 0;
  if (p && p->pubkey[0]) {
    const char *v[1] = {p->pubkey};
    PGresult *r = PQexecParams(db, "SELECT auth.viewer_lines($1)", 1, NULL, v, NULL, NULL, 0);
    if (PQresultStatus(r) != PGRES_TUPLES_OK || PQntuples(r) != 1 || PQgetisnull(r, 0, 0)) {
      api_log("auth: viewer: %s", PQerrorMessage(db)); PQclear(r); refuse(q, 500, "That didn't work. Try again."); return;
    }
    size_t n = strlen(PQgetvalue(r, 0, 0));
    out = malloc(n + 8);
    if (out) on = auw_viewer_lines(cs(PQgetvalue(r, 0, 0)), p->admin, (coax_v_u8){(uint8_t *)out, (int64_t)n + 7});
    PQclear(r);
    if (!out) { refuse(q, 500, "Out of memory."); return; }
  } else if (p && p->account[0]) {
    out = malloc(64);
    if (!out) { refuse(q, 500, "Out of memory."); return; }
    on = auw_viewer_account_lines(cs(p->account), (coax_v_u8){(uint8_t *)out, 63});
  }
  if (out && on < 0) { free(out); refuse(q, 500, "That didn't work. Try again."); return; }
  if (!out && !quiet) {
    char nobody[32]; int64_t nn = auw_viewer_nobody_lines(OUT(nobody));
    api_respond(q->fd, 401, "text/plain; charset=utf-8", head, nobody, nn < 0 ? 0 : (size_t)nn);
    return;
  }
  api_respond(q->fd, 200, "text/plain; charset=utf-8", head, out ? out : "", out ? (size_t)on : 0);
  free(out);
}

static void limits(const request *q) {
  char json[4096], lines[3000];
  int64_t j = auw_limits_open_write(OUT(json)), l = 0;
  for (int i = 0; i < L_COUNT; i++) {
    j = auw_limits_row_write(i == 0, cs(LIMITS[i].name), cs(LIMITS[i].what), LIMITS[i].max, cs(LIMITS[i].per), LIMITS[i].window_ms, OUT(json), j);
    l = auw_limits_row_lines(cs(LIMITS[i].name), LIMITS[i].max, cs(LIMITS[i].per), cs(LIMITS[i].what), OUT(lines), l);
  }
  j = auw_limits_close_write(OUT(json), j);
  answer_n(q, 200, "Cache-Control: public, max-age=300\r\n", json, j, lines, l);
}

// A key's login for another file's route (the admin page's signed GET, admin.c): a web session
// at L2, answered with its cookie and Location. false when it answered a refusal itself.
bool auth_login_key_redirect(const request *q, PGconn *db, const char *pk, const char *location) {
  char acct[40], kid[24]; bool made = false;
  if (!key_account(db, pk, "admin", acct, kid, &made)) { api_error(q->fd, 500, "no session"); return false; }
  PGresult *r;
  char tok[160], dig[65], name[96];
  device_name(q, name, sizeof name);
  if (!new_token("oas_", tok, sizeof tok, dig)) { api_error(q->fd, 503, "no randomness"); return false; }
  const char *s[8] = {dig, acct, "nostr", kid, "L2", "web", name, NULL};
  r = PQexecParams(db, "SELECT auth.session_new($1, $2, $3, $4::bigint, $5, $6, $7, $8)", 8, NULL, s, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0);
  char sid[32] = ""; if (ok) snprintf(sid, sizeof sid, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  if (!ok) { api_error(q->fd, 410, "this account was deleted"); return false; }
  audit(db, acct, sid, "admin.login", "ok", NULL);
  set_cookie[0] = 0;
  char headers[900];
  snprintf(headers, sizeof headers, "Location: %s\r\nSet-Cookie: %s=%s; Path=/; Max-Age=%d; HttpOnly; Secure; SameSite=Lax\r\n" TEAM_COOKIE,
           location, SESSION_COOKIE, tok, SESSION_MAX_AGE);
  api_respond(q->fd, 303, "text/plain", headers, "", 0);
  return true;
}

#ifdef OA_LOCAL_SERVE
// ---------------------------------------------------------------- the local admin (#13463)
// Only in the local serve mode: test.sh --serve builds with -DOA_LOCAL_SERVE and sets OA_LOCAL_ADMIN to
// its throwaway admin's key; the image (Dockerfile, deploy.sh) never defines it, and the checks' build
// refuses a request with no credential though OA_LOCAL_ADMIN is set (test_zchat.py). A request with no
// credential is that admin's when it comes to this machine from this machine: the peer is loopback,
// the Host is a loopback name, no proxy put an X-Forwarded-For on it, and the browser didn't say it
// came from another site. It runs on one L2 web session of its own, made on first use and again when
// it ends, so every route's own rules (and Origin checks) hold as for a person logged in.
#include <arpa/inet.h>
#include <netinet/in.h>
#include <sys/socket.h>
static char local_digest[65];
static bool loopback_peer(int fd) {
  struct sockaddr_storage a; socklen_t n = sizeof a;
  if (getpeername(fd, (struct sockaddr *)&a, &n)) return false;
  if (a.ss_family == AF_INET) return (ntohl(((struct sockaddr_in *)&a)->sin_addr.s_addr) >> 24) == 127;
  if (a.ss_family == AF_INET6) return IN6_IS_ADDR_LOOPBACK(&((struct sockaddr_in6 *)&a)->sin6_addr) != 0;
  return false;
}
static bool loopback_host(const request *q) {
  char h[256];
  if (!api_header(q, "Host", h, sizeof h)) return false;
  static const char *const names[] = {"127.0.0.1", "localhost", "[::1]", NULL};
  for (const char *const *n = names; *n; n++) {
    size_t k = strlen(*n);
    if (!strncmp(h, *n, k) && (h[k] == 0 || h[k] == ':')) return true;
  }
  return false;
}
// OA_LOCAL_EMAIL (test.sh --serve: this computer's owner's email): the local admin's account carries it, so
// the page names the account by it and a login by that email's code is the same account as the key. Once a
// run; an address another account here already has stays that account's, and the log says so.
static void local_owner_email(PGconn *db, const char *acct) {
  static bool done;
  const char *raw = getenv("OA_LOCAL_EMAIL");
  if (done || !raw || !*raw) return;
  done = true;
  char email[300], all[400], lookup[80];
  int64_t n = as_email_normalize((coax_v_u8){(uint8_t *)raw, (int64_t)strlen(raw)}, (coax_v_u8){(uint8_t *)email, (int64_t)sizeof email - 1});
  if (n < 0 || !sealing_ready()) { api_log("auth: OA_LOCAL_EMAIL isn't an email address, or sealing isn't set up"); return; }
  email[n] = 0;
  if (!lookups(email, all, sizeof all, lookup)) return;
  const char *lv[1] = {all};
  PGresult *r = PQexecParams(db, "SELECT account FROM auth.emails WHERE lookup = ANY($1::text[]) LIMIT 1", 1, NULL, lv, NULL, NULL, 0);
  char other[40] = "";
  if (PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1) snprintf(other, sizeof other, "%s", PQgetvalue(r, 0, 0));
  PQclear(r);
  if (other[0]) {
    if (strcmp(other, acct)) api_log("auth: %s is account %s's here, not the local admin's (%s): it stays there", email, other, acct);
    return;
  }
  PQclear(PQexec(db, "BEGIN"));
  const char *v[2] = {acct, lookup};
  r = PQexecParams(db, "SELECT state, email_id FROM auth.email_attach($1, $2)", 2, NULL, v, NULL, NULL, 0);
  bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !strcmp(PQgetvalue(r, 0, 0), "ok");
  char email_id[24] = ""; if (ok) snprintf(email_id, sizeof email_id, "%s", PQgetvalue(r, 0, 1));
  PQclear(r);
  char sealed[1200], ad[96];
  snprintf(ad, sizeof ad, "auth.emails|sealed|%s", acct);
  if (ok) ok = seal(ad, email, sealed, sizeof sealed);
  if (ok) {
    const char *w[2] = {email_id, sealed};
    r = PQexecParams(db, "SELECT auth.email_seal($1::bigint, $2)", 2, NULL, w, NULL, NULL, 0);
    ok = PQresultStatus(r) == PGRES_TUPLES_OK; PQclear(r);
  }
  if (ok) {   // the owner's email is the account's primary: the name the page shows
    const char *w[2] = {acct, email_id};
    r = PQexecParams(db, "UPDATE auth.emails SET is_primary = (id = $2::bigint) WHERE account = $1", 2, NULL, w, NULL, NULL, 0);
    ok = PQresultStatus(r) == PGRES_COMMAND_OK; PQclear(r);
  }
  PQclear(PQexec(db, ok ? "COMMIT" : "ROLLBACK"));
  api_log(ok ? "auth: the local admin's account %s now carries %s" : "auth: %s: %s couldn't be added", acct, email);
}
// Whether acct is the local admin's account (its key's).
static bool local_owner_account(PGconn *db, const char *acct) {
  const char *pk = getenv("OA_LOCAL_ADMIN");
  if (!pk || strlen(pk) != 64 || !admin_is(pk)) return false;
  const char *v[2] = {pk, acct};
  PGresult *r = PQexecParams(db, "SELECT 1 FROM auth.nostr_keys WHERE pubkey = $1 AND account = $2", 2, NULL, v, NULL, NULL, 0);
  bool yes = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1;
  PQclear(r);
  return yes;
}
// Before anything is asked: the local admin's account and the owner's email on it.
static void local_owner_init(PGconn *db) {
  const char *pk = getenv("OA_LOCAL_ADMIN");
  char acct[40], kid[24]; bool made = false;
  if (!db || !pk || strlen(pk) != 64 || !admin_is(pk)) return;
  if (key_account(db, pk, "admin", acct, kid, &made)) local_owner_email(db, acct);
}
static bool local_admin(const request *q, PGconn *db, auth_principal *p) {
  const char *pk = getenv("OA_LOCAL_ADMIN");
  char xff[8], site[32], out[8];
  if (!pk || strlen(pk) != 64 || !admin_is(pk)) return false;
  cookie(q, LOCAL_OUT_COOKIE, out, sizeof out);
  if (out[0]) return false;   // this browser logged out: it logs in again (email code, or Log in as this computer's owner)
  if (api_header(q, "X-Forwarded-For", xff, sizeof xff) || !loopback_peer(q->fd) || !loopback_host(q)) return false;
  if (api_header(q, "Sec-Fetch-Site", site, sizeof site) && strcmp(site, "same-origin") && strcmp(site, "none")) return false;
  char keep[sizeof set_cookie]; memcpy(keep, set_cookie, sizeof keep);
  for (int round = 0; round < 2; round++) {
    if (local_digest[0] && from_session(db, local_digest, true, NULL, p)) { memcpy(set_cookie, keep, sizeof keep); return true; }
    memcpy(set_cookie, keep, sizeof keep);   // an ended local session clears no cookie of the browser's
    char acct[40], kid[24], tok[160], dig[65]; bool made = false;
    if (!key_account(db, pk, "admin", acct, kid, &made) || !new_token("oas_", tok, sizeof tok, dig)) return false;
    const char *s[8] = {dig, acct, "nostr", kid, "L2", "web", "This computer (the local API)", NULL};
    PGresult *r = PQexecParams(db, "SELECT auth.session_new($1, $2, $3, $4::bigint, $5, $6, $7, $8)", 8, NULL, s, NULL, NULL, 0);
    bool ok = PQresultStatus(r) == PGRES_TUPLES_OK && PQntuples(r) == 1 && !PQgetisnull(r, 0, 0);
    PQclear(r);
    memset(tok, 0, sizeof tok);
    if (!ok) return false;
    memcpy(local_digest, dig, sizeof local_digest);
    api_log("auth: the local admin's session made");
  }
  return false;
}
#endif

// ---------------------------------------------------------------- the routes
bool auth_api_route(const request *q) {
  if (strncmp(q->path, "/api/auth/", 10)) return false;
  const char *op = q->path + 10;
  api_route_at("auth_recover_route");
  if (auth_recover_route(q, api_db(), op)) return true;
  api_route_at("auth_life_route");
  if (auth_life_route(q, api_db(), op)) return true;
  api_route_at("passkeys_route");
  if (passkeys_route(q, api_db(), op)) return true;
  api_route_at("auth_device_route");
  if (auth_device_route(q, api_db(), op)) return true;
  api_route_at("auth_api_route");
  bool get = !strcmp(q->method, "GET");
  static const char *const posts[] = {"email/start", "email/verify", "email/link", "nostr/challenge", "nostr", "phone/start", "phone/approve",
                                      "phone/poll", "refresh", "logout", "logout-everywhere", "freeze", NULL};
  bool known_post = false;
  for (const char *const *o = posts; *o; o++) if (!strcmp(op, *o)) known_post = true;
  bool remove = !strncmp(op, "sessions/", 9) && strstr(op, "/remove");
  if (!known_post && !remove && strcmp(op, "account") && strcmp(op, "limits") && strcmp(op, "sessions") && strcmp(op, "viewer") && strcmp(op, "me")) return false;   // the gate has no rule for it: 404
  if (!strcmp(op, "limits")) { if (!get) { refuse(q, 405, "GET this."); return true; } limits(q); return true; }
  PGconn *db = api_db();
  if (!db) { refuse(q, 503, "The database isn't answering. Try again."); return true; }
  if (!strcmp(op, "account")) { if (!get) { refuse(q, 405, "GET this."); return true; } me(q, db); return true; }
  if (!strcmp(op, "viewer") || !strcmp(op, "me")) { if (!get) { refuse(q, 405, "GET this."); return true; } viewer(q, db, op[0] == 'v'); return true; }
  if (!strcmp(op, "sessions")) { if (!get) { refuse(q, 405, "GET this."); return true; } sessions(q, db); return true; }
  if (get || strcmp(q->method, "POST")) { refuse(q, 405, "POST this."); return true; }
  if (!same_site(q)) { refuse(q, 403, "Not from this site."); return true; }
  if (remove) { session_remove(q, db); return true; }
  char *body = body_text(q);
  if (!body) { refuse(q, 500, "Out of memory."); return true; }
  if (!strcmp(op, "email/start")) email_start(q, db, body);
  else if (!strcmp(op, "email/verify")) email_verify(q, db, body);
  else if (!strcmp(op, "email/link")) email_link(q, db, body);
  else if (!strcmp(op, "nostr/challenge")) nostr_challenge(q, db, body);
  else if (!strcmp(op, "nostr")) nostr_login(q, db, body);
  else if (!strcmp(op, "phone/start")) phone_start(q, db);
  else if (!strcmp(op, "phone/approve")) phone_approve(q, db, body);
  else if (!strcmp(op, "phone/poll")) phone_poll(q, db, body);
  else if (!strcmp(op, "refresh")) refresh(q, db, body);
  else if (!strcmp(op, "logout")) logout(q, db);
  else if (!strcmp(op, "logout-everywhere")) logout_everywhere(q, db, body);
  else if (!strcmp(op, "freeze")) freeze(q, db, body);
  free(body);
  return true;
}

// ---------------------------------------------------------------- for auth-recover.c (auth-internal.h)
bool auth_random(unsigned char *b, size_t n) { return random_bytes(b, n); }
bool auth_random_hex(char *out, size_t bytes) { return random_hex(out, bytes); }
void auth_hex(const unsigned char *b, size_t n, char *out) { hex_of(b, n, out); }
bool auth_unhex(const char *s, unsigned char *out, size_t n) { return unhex(s, out, n); }
void auth_sha_hex(const char *s, char out[65]) { sha_hex(s, out); }
// A JSON string's value for a key, unescaped for \" and \\, or NULL: auth-wire.coax's scan, with the
// member named by the caller (the other auth files' reads).
const char *auth_json_str(const char *body, size_t n, const char *key, char *out, size_t cap) {
  return auw_body_str((coax_string){body, (int64_t)strnlen(body, n)}, cs(key), (coax_v_u8){(uint8_t *)out, (int64_t)cap}) >= 0 ? out : NULL;
}
void auth_json_esc(const char *s, char *out, size_t cap) { json_esc(s, out, cap); }
bool auth_limit_hit(const request *q, PGconn *db, int which, const char *key) {
  long long until = 0;
  if (limit_hit(db, which, key, &until)) return true;
  refuse_limit(q, which, until);
  return false;
}
void auth_visitor_key(const request *q, char out[65]) { visitor_key(q, out); }
bool auth_lookups(const char *email, char *all, size_t cap, char newest[80]) { return sealing_ready() && lookups(email, all, cap, newest); }
bool auth_unseal(const char *ad, const char *sealed, char *out, size_t cap) { return unseal(ad, sealed, out, cap); }
bool auth_normal_email(const char *raw, char *out, size_t cap) {
  int64_t n = as_email_normalize((coax_v_u8){(uint8_t *)raw, (int64_t)strlen(raw)}, (coax_v_u8){(uint8_t *)out, (int64_t)cap - 1});
  if (n < 0) return false;
  out[n] = 0;
  return true;
}
void auth_mask_email(const char *email, char *out, size_t cap) { mask_email(email, out, cap); }
