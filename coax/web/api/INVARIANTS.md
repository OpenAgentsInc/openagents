# Invariants — coax/web/api/INVARIANTS.md

> Scope: HTTP API, auth, NIP-98, Postgres schema/migrations, room-session API (2 invariants). Indexed from root [INVARIANTS.md](../../../INVARIANTS.md).

## A NIP-98 caller is its key's account, once per signature

On `/api/playtest/realms` and `GET /api/auth/account`, a request with an
`Authorization: Nostr` header is its signer's account, `auth.account_of`
(the account that holds the key in `auth.nostr_keys`, else `n-<pubkey>`), and
nothing else (a key no account holds is not logged in on `/api/auth/account`): a bad, forged
or expired signature, a signed write without a `payload` tag, and a signature
seen before are 401, never a fallback to the `X-Verse-Player` key. The
decision is `cp_caller` (`coax/web/api/caller-policy.coax`, `#authority`,
mutation-checked); `access.c` only moves the facts in and out. The signer acts
through a player key the database derives for its account
(`auth.caller_key`, `migrations/0098_cli_callers.sql`): kept nowhere, never
sent, so every ownership and admission rule (`verse.owns`, `access.gate`, the
builder's caps) is the same SQL a page's key meets; no rule was changed for
it. The Origin check still runs first. Checks:
`coax/web/api/tests/cli_auth.coax` (owner's signed save 200, another key's
403, replay, changed body, unbound write, wrong URL and forged header 401,
another site's Origin 403, the player key as before; `/api/auth/account`
not logged in before the key logs in, its account after), run by `test.sh`
and by `coax/web/api/cli-auth-test.sh` on staging.

A key joins an account in three ways only, all `auth.c` and its neighbours:
a login with the key (`POST /api/auth/nostr`, which makes the account on
first use), `POST /api/auth/link/nostr` (a logged-in session and the key's own
signature), and device login (`/api/auth/device/*`, `agents.c`): the key's
signed `device/start`, then APPROVE pressed on `/login/device` by a logged-in
session. The old groundwork's `/api/auth/v2/*` (WorkOS, its own device login
and links) and the old phone sign-in's `/api/auth/challenge|approve|poll|key`
were deleted in #13207 and answer 404; their tables (`signin.*`,
`auth.identities`, `auth.v0_sessions`, `auth.workos_states`) are dropped.
Checks: `coax/web/api/tests/agent_checks.coax` (device login);
`coax/web/api/test_realm_edit.py` and `test_access.py` (the old routes are 404);
`apps/openagents-web/private.coax` (page 7 approves only on the press).

## A listed first-party site logs in by email code, may ask the account chat, and does nothing else

A browser write to `/api/auth/` needs this site's `Origin` (`OA_API_ORIGIN`);
apps and tools send none. `OA_API_SITE_ORIGINS` (unset by default; production
lists `https://gamebuilder.org`, `coax/web/api/deploy.sh`) names other
first-party sites whose load balancers send six of their own paths to this
API (and the command line's two, below, which carry no `Origin`). An `Origin` is a listed site only when it equals one entry byte for
byte, and an entry counts only as `https://host[:port]` in lower case (or the
dev server's `http://127.0.0.1[:port]`, `http://localhost[:port]`): no
wildcard, suffix, prefix, path, slash, space or capital is forgiven. Unset,
every answer is as it was.

A request with a listed site's `Origin` reaches `GET /api/auth/account`,
`POST /api/auth/email/start`, `POST /api/auth/email/verify`,
`POST /api/auth/logout`, `POST /api/auth/cli/code` (a command line's code,
made by the logged-in browser: next section), `POST /api/chat/complete/v1`, a
GameBuilder build agent's three routes (`POST /api/gamebuilder/agents/v1`,
`GET /api/gamebuilder/agent/v1`, `GET /api/gamebuilder/agent/events/v1`), and
The Whispering Spring's six shared-grove guest/room routes
(`POST /api/playtest/player`, `POST /api/playtest/player/name`,
`GET`/`POST /api/playtest/chat/whispering-spring`,
`GET`/`POST /api/playtest/chat/whispering-spring-ribbons`,
`GET`/`POST /api/playtest/chat/whispering-spring-mandala`,
`POST /api/playtest/presence/whispering-spring`), and is
`403 Not from this site.` on every other route of the API, at the gate, before
the route runs. The account chat keeps its own rules there as on this site
(`account-chat.c`: a person's session or 401, `act_read`'s shape and sizes or
400, `inference.c`'s quotas), but for one: who is funded. The site's chat is
for every account (owner, 2026-11-17, GameBuilder issue 15): asked with a
listed site's `Origin`, `POST /api/chat/complete/v1` answers any person's own
account, on the team beta or not (`act_funded_from`,
`coax/web/api/account-chat-policy.coax`, `#authority`, mutation-checked).
Everywhere else the funding is the team's as before (`act_funded` or 403):
this site's own page, an app or tool with no `Origin`, the world, delegated
and project turns and the durable chat turns (a GameBuilder build agent has
its own rule, the section "A GameBuilder build agent is any person's own
account's"). What
bounds an account there is what bounds the team: the free key (no credit,
sats or wallet is read or charged), the account's own day
(`OA_INFERENCE_DAILY_REQUESTS`, 500 requests, and
`OA_INFERENCE_DAILY_USD_MICROS`, $2.00; `inference.request`, whose 429 says
the limit with its number since migration 0213), the answer's 2,048 tokens
(`act_completion`) and the thread's shape and sizes (`act_read`). No other chat route is
reached, not the world, delegated or project turns, the saved threads, nor
any GET. Its email code is a
plain browser login: `link`, `merge` and `client: "app"` are refused at the
start and when the code is typed, and the pending login is the browser's
cookie, never a value in the body (a page of the listed site that posts to
this site's host carries no cookie, so it logs nobody in there). Passkeys,
keys, recovery, logout everywhere, freezing, devices, settings, the wallet
and agents stay on this site. No answer names the caller's origin: no CORS
header is added, cookies keep no `Domain`, and the email's link is
`OA_API_ORIGIN`'s. The audit rows `email.code` and `login.email` say which
site (`"site"`). Every other route file keeps `auth_same_site`: this site's
`Origin` or none.

The decisions are `ap_site`, `ap_site_route`, `ap_site_post`, `ap_site_login`
and `ap_site_pending` (`coax/web/api/auth-policy.coax`, `#authority`,
mutation-checked); `auth.c` moves the header and the two variables in. Checks:
that file's `#test`s (exact match, wrong scheme, suffix, prefix, port, case,
slash, whitespace, wildcard, empty list, several entries);
`coax/web/api/tests/auth_checks.coax` `ac_site_checks` (no list: 403; listed:
the code is asked for, typed and logged out from the site; the link, merge,
app, a pending value in the body, the chat's neighbours, every other write, a read with its `Origin`
and every other site are refused; the account chat with no session is 401;
a person off the team is asked of the model from the site's page, the ask
counts against that account's day, a bad shape is 400 and the day's limit is
429 in words with its number, while the same session from this site's own
page, with no `Origin`, and on the world and delegated turns keeps the
team's 403 and counts nothing; no CORS header, no `Domain`, the mail's
link is ours, the audit rows name the site); `account-chat-policy.coax`'s
`#test` (only the plain completion, only a listed site, only a person's own
account; the team's funding unchanged). Not covered: an `Origin` is what the
caller sends, so a tool that sends the listed site's `Origin` with a person's
own login is answered as the site's page is; the bound is that account's own
day, as it is on the page. A new account is an email address, so the total is
bounded per account, not across accounts (the email login's own rate limits
apply). Not covered: a GET the other site's load balancer forwards carries
no `Origin`, so it is answered as this site's own; that balancer must send
only those paths ([sessions and tokens](docs/auth/sessions-and-tokens.md#a-second-first-party-site)).

