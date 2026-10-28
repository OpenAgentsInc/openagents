# Private projects: threat model

What protects a private project's contents (`docs/private-projects.md`),
what it is checked by, and what is left.

## Assets

The project document (goals, milestones, checklist, links), its history,
agent activity, messages, the member list, and the fact that a given slug
exists. Sessions, sign-in links and keys that grant access to them.

## Who we defend against

- Anyone on the internet, signed out or with their own Nostr key.
- A member of one project reaching another project.
- A registered agent trying to do more than an agent may.
- A hostile site the member visits (CSRF), and anything that sees URLs:
  proxy and load-balancer logs, referrers, browser history, link previews.
- Caches between oa-api and the browser.

Out of scope for now: whoever runs oa-api or Cloud SQL (contents are plain
text there; end-to-end encryption with NIP-44 is the next step), a member's
compromised machine or key, and a member sharing contents on purpose.

## Controls

| Threat | Control | Check |
| --- | --- | --- |
| Reading without access | Every read and write goes through `projects.api`, which answers anyone who is not a member or agent of that slug with the same 404 `{"error": "not found"}` | `test_private.py`: stranger key, made-up cookie, other project's session, nonexistent slug |
| Learning that a slug exists | Signed out, wrong key and nonexistent slug get byte-identical answers from `/api/private/*`; the page answers 404 with a generic title ("Private project") and no name; unknown `/projects/*` paths get the same locked page from the bucket (`lb.sh private-pages`); timing gap between a real and a made-up slug checked under 3 ms | `test_private.py` (identical bytes, timing), `smoke.sh` |
| Caches keeping private answers | `Cache-Control: no-store, private` and `Vary: Cookie, Authorization` on every oa-api answer; Cloud CDN is off on the `oa-api` and `oa-api-staging` backend services (checked with `gcloud compute backend-services list`); the bucket's CDN serves only the generic locked page | `test_private.py`, production check with a second request from another session |
| Stolen session cookie | `HttpOnly; Secure; SameSite=Lax; Path=/projects/<slug>`; 30-day expiry; only its SHA-256 is stored; sign out deletes it | `test_private.py` (flags, hash-only storage, expiry, per-project) |
| Sign-in link leaking | The token rides in the URL fragment (`#invite=`), which browsers never send to a server, so it never reaches the load balancer's logs or a referrer; the page clears the hash before trading it; single use, 24 hours, bound to one member's pubkey and one project; only its hash is stored | `test_private.py` (single use, expiry, bound pubkey), `private.coax` tests |
| CSRF | Every state-changing request is a POST; a cookie-authenticated POST must carry an `Origin` equal to the site's; NIP-98 requests are signed over method, URL and body hash | `test_private.py` (no Origin, evil Origin) |
| Replay of a signed request | NIP-98 events within 60 s of now, each event id accepted once | `test_private.py` |
| Guessing tokens | 256-bit random tokens; redeem limited to 10 a minute per address; every POST to 60 a minute per address and writes to 30 a minute per key (address from the load balancer's own `X-Forwarded-For` entry, not the client's) | `test_private.py` (redeem limit, spoofed header) |
| An agent overreaching | Agents' pushes may change only milestone and item status/notes and add proposed goals; changing or removing agreed goals, members or the title is 403; agents cannot mint links or register agents; an agreed goal names the members who agreed | `test_private.py` |
| The project's agent overreaching | Its key (role `runner`) writes only inside a running turn of its own project; its pushes may not change the title, the members or an agreed goal (`projects.kept`); it can't mint links, register agents, ask, undo or agree; its routes exist only for projects in the server's approved list, and answer the same 404 elsewhere; each edit is recorded under the turn with the member's message, and UNDO TURN restores the version before it | `test_private_agent.py` |
| The agent's spend | A day's cost per project from the CLI's reports, refused at the cap (`OA_AGENT_DAILY_USD`, 10) with a reason on the page; one running turn per project; a turn not ended in 20 minutes fails | `test_private_agent.py` |
| Losing history | Every push keeps the previous version; a push based on a stale version is 409 | `test_private.py` |
| Contents in logs | oa-api logs only the operation, status and the first 8 hex digits of the caller's pubkey; SQL errors log only their SQLSTATE; no tokens, cookies or contents | `test_private.py` reads the test server's log |
| Contents in public places | The page's files are generic and public; contents come only from the API with a session; nothing goes to the Nostr relay or the site bucket | review; `smoke.sh` |
| Framing and referrers | `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer` | `test_private.py` |

### Why SameSite=Lax, not Strict

Sign-in links are opened from messaging and mail apps, a cross-site top-level
navigation. A Strict cookie is not sent on that first load, so a member
following any link to their project would see the locked page. Lax sends the
cookie only on top-level GET navigations from other sites; every write is a
POST that also needs a same-site `Origin`, so Lax allows nothing Strict would
refuse.

## Known residual risks

- **Unknown slugs get the site's 404 page, not the locked page.** The load
  balancer's custom error policy (`lb.sh private-pages`) is in the URL map,
  but the bucket's own not-found page still answers unknown `/projects/*`
  paths. Both are 404 and name nothing, but a visitor can tell "routed and
  locked" from "no such slug" by the page they see. The data endpoints are
  identical for both. Fix: serve every unrouted `/projects/<slug>` from
  oa-api (one prefix rule, the shell answering locked for unknown slugs).
- **Routed slugs differ at the header level.** `lb.sh private-route SLUG`
  sends a real slug's page to oa-api, while an unknown slug is answered by
  the bucket. The bodies say the same thing, but raw response headers (for
  example `x-goog-*` from the bucket, `Vary` from oa-api) can tell a routed
  slug from an unrouted one. The data endpoints (`/api/private/<slug>/*`) do
  not have this difference. Fix later by routing all of `/projects/*` that
  the bucket doesn't hold through oa-api.
- **Plain text at rest.** Cloud SQL and the private repo hold contents in the
  clear; whoever administers them can read them. NIP-44 end-to-end
  encryption is the planned fix.
- **Rate limits are per instance.** oa-api keeps the counters in memory, so
  several instances multiply the allowance. The token space makes guessing
  infeasible regardless.
- **A sign-in link is a bearer secret for 24 hours.** Whoever has it first
  becomes the member until the session is signed out. Links are minted only
  by members and should be sent over a private channel.
