# Switching playtest.openagents.com to openagents.com

Task `pt-cutover-plan` (#2086) of the Verse project
(https://openagents.com/projects/verse). Written 2026-10-28 by
claude-verse-18. The switch was the owner's decision after the parity review
(`pt-parity-review`).

**Switched 2026-10-29, 02:33 CDT** (owner's go: once openagents.com/verse
showed the old landing, which it does since the release with the playtest's
orbs, field and glass button, and PLAYTEST NOW opening the Haunted House
directly). T−15 import: every `held` equal to its `source` (events 1806,
sessions 145, playtesters 31; 132 events added since the run before). The
moved map was in after 21 s, the first clean pass came at 138 s and it
settled (five clean passes) at 153 s. A headless visit to
playtest.openagents.com landed on /verse, recorded its page view with
`q: from=playtest`, and PLAYTEST NOW opened the house. Imports at T+1 h and
T+24 h follow as in section 4.

**In one line:** the switch changes only the URL map of the playtest's own
load balancer, so every old address answers 301 to its place on
openagents.com; DNS, the certificate, the address, the backend and the Cloud
Run service `aethyr-demo` stay exactly as they are, so the rollback is one
command that puts the old map back.

```
coax/web/playtest-switch.sh switch      # owner step
coax/web/playtest-switch.sh rollback    # anyone, any time
```

## 1. What serves playtest.openagents.com today

Read with `gcloud … describe` and `url-maps export` on 2026-10-28 (19:20 CDT),
project `openagentsgemini`, the automation account:

| Piece | Today |
| --- | --- |
| DNS | playtest.openagents.com A 34.110.190.11 (Cloudflare) |
| Forwarding rules | `aethyr-demo-https` (443) and `aethyr-demo-http` (80) on 34.110.190.11, `EXTERNAL_MANAGED` |
| HTTPS proxy | `aethyr-demo-https` → URL map `aethyr-demo-lb`, certificate `aethyr-demo-domain-cert` (Google-managed, playtest.openagents.com, ACTIVE) |
| HTTP proxy | `aethyr-demo-http` → URL map `aethyr-demo-http-redirect` (301 to https) |
| URL map `aethyr-demo-lb` | two lines: `name: aethyr-demo-lb`, `defaultService: …/backendServices/aethyr-demo-web` (no host or path rules) |
| Backend | `aethyr-demo-web` → serverless NEG `aethyr-demo-neg` → Cloud Run `aethyr-demo` (us-central1, revision `aethyr-demo-00003-z68`, 100%, min 1 / max 1), request logging off |
| Data | bucket `aethyr-demo-data` mounted at `/data`, versioning on; 1,674 events in 135 sessions, 28 playtesters, 4 published worlds on 2026-10-28 |

openagents.com is a different load balancer (`coaxial-openagents-web-staging`,
136.69.24.60, certificate map with openagents.com and staging.openagents.com).
`/verse`, `/verse/galaxy`, `/realms/*`, `/admin/analytics` go to the site's
bucket and `/api/*` to Cloud Run `oa-api` there; the old `/realm/*` answers 301 to `/realms/*`, path, query and hash kept.

## 2. Why the URL map and not DNS

Two ways were weighed:

| | A. Change `aethyr-demo-lb` (chosen) | B. Move DNS to the Coaxial load balancer |
| --- | --- | --- |
| What changes | one URL map | a certificate-map entry, a DNS-authorization CNAME at `_acme-challenge.playtest`, a new certificate, the A record |
| Certificate | the one that is ACTIVE today | a new one, minutes to an hour to issue |
| Rollback | import the old map (measured below) | DNS back, bounded by resolvers' caches (TTL), not by us |
| Cost | as today (the demo's two forwarding rules and min-1 instance stay) | saves the demo's forwarding rules later |

A is faster, has nothing to issue, and its rollback doesn't wait on anyone's
DNS cache. B can follow later, once the owner retires the old demo, as its
own decision.

## 3. The address mapping (old → new)

Every redirect is a permanent 301 to https://openagents.com and carries
`?from=playtest`, which the page records (the `url` fact of every page view),
so the dashboard tells arrivals from the old host apart.

| Old address on playtest.openagents.com | Answer | New address |
| --- | --- | --- |
| `/`, `/index.html`, `/kiki/…` (the Kiki landing), `/home.html` | 301 (URL map), query dropped | `/verse?from=playtest` (the landing; its galaxy is `/verse/galaxy`, reached from it) |
| `/haunt.html` (the Haunted House; the Share link) | 301 from `oa-api` (`/api/playtest/old-link`) | `/realms/haunted-house?from=playtest` |
| `/haunt.html?visit=ID&planet=…` (a shared or invited world) | 301 from `oa-api` | `/realms/haunted-house?from=playtest#ID` (that world; the realm page's hash names it). IDs are `[a-z0-9-]{1,64}`; anything else opens the house |
| `/analytics.html`, `/feedback.html` | 301 (URL map) | `/admin/analytics` |
| `/api/*` | still the old service | unchanged, so tabs already open keep saving and recording until the last import (section 5) |
| everything else (`/grid.html`, `/town.html`, other labs) | 301 (URL map) | `/verse?from=playtest` (not ported) |

The URL map can't turn a query into a hash, so `/haunt.html` goes to the API
service rewritten to `/api/playtest/old-link`, which answers the 301 (`api.c`,
`old_link`, checks in `coax/web/api/test_old_link.py`; deployed to staging and
production, commit `ea40a2d`). It is inert until the switch: nothing else
routes there. The four worlds the demo published (`haunted-house`,
`planet-06258408`, `planet-1a9793977cb79fa3`, `planet-mekiki65hgyv6087`) are
already imported and answer 200 at `/api/playtest/world/ID` on openagents.com.

With a pinned demo (`coax/web/pin.sh playtest REF`, an owner step), switch
with `--to /demo/playtest/verse` so the old host lands on the frozen build.

## 4. The switch, step by step

Times are measured in the rehearsal (section 7). Chicago time.

| When | Step | Who | Takes |
| --- | --- | --- | --- |
| Before | Parity review done and a yes (NEEDS_OWNER.md) | owner | |
| T−15 min | `coax/web/api/import_demo.sh --production`: adds only what the demo recorded since the last run; its output lists source and held counts per kind, and every `held` must equal `source` | agent | 10 s |
| T−10 min | `bun coax/web/goldens.ts https://openagents.com --walk` (landing, galaxy, house, analytics contract) and `curl -sI 'https://openagents.com/api/playtest/old-link?visit=planet-06258408'` (301 to the world) | agent | 52 s |
| T−5 min | `coax/web/playtest-switch.sh show` prints the two-line map of section 1; anything else, stop | agent | 2 s |
| T | `coax/web/playtest-switch.sh switch` (add `--to /demo/playtest/verse` for the pin): validates and imports the moved map, then smokes the real host until every address answers as in section 3 five passes in a row; if it never settles in 8 minutes it puts the old map back by itself | owner says go; agent or owner runs | 3–4 min (180 s and 221 s) |
| T+5 min | On a phone: open playtest.openagents.com; it should land on ENTER THE VERSE at openagents.com/verse. Open an old Share link: the Haunted House | owner | 1 min |
| T+1 h | `import_demo.sh --production` again (what open tabs saved since) | agent | 10 s |
| T+24 h | Again, the last one. `/api/*` keeps reaching the old service; it does no harm | agent | 10 s |

The whole switch, from the last import to a settled smoke, takes about
5 minutes; the first visitors reach the new site within a minute of the map
going in, everyone within about 2.5 minutes.

**Data freeze.** None is needed: the import is incremental and idempotent
(re-running it added 0 of 1,674 events), the old service stays up and keeps
its bucket, and the imports at T+1 h and T+24 h catch anything written after
the switch. The demo's bucket is only ever read.

## 5. Analytics continuity

- **Everything before the switch** is in Cloud SQL from the imports, under the
  space `playtest.openagents.com`, and shows in `/admin/analytics` with the
  new site's own sessions.
- **Tabs open at the switch** keep posting to `/api/events` on the old host,
  which still reaches the old service; the T+1 h and T+24 h imports bring
  them over.
- **Arrivals through the old host** carry `?from=playtest` in the page's
  recorded `url`; the browser keeps the original referrer across a 301, so
  `ref` still says where the click came from.
- **The contract check** (`pt-analytics-contract`) keeps walking the flow at
  every openagents.com release, so the new pages can't stop sending events
  unnoticed.
- **What does not carry over:** an anonymous player's id lives in
  `localStorage` on playtest.openagents.com, which openagents.com can't read.
  Players who gave an email are imported and are matched again when they
  enter it; guests start fresh. Their past sessions stay in the dashboard.
- **Counting redirects:** the old load balancer has request logging off and
  redirects never reach a backend, so the count of hits on the old host is
  the forwarding rule's request count in Cloud Monitoring
  (`loadbalancing.googleapis.com/https/request_count`), and, for people,
  the `?from=playtest` page views.

## 6. Rollback

```
coax/web/playtest-switch.sh rollback
```

It imports the old two-line map (default service `aethyr-demo-web`) and
smokes the host until `/`, `/haunt.html`, `/kiki/index.html` and
`/api/config` answer 200 from the old demo five passes in a row. Nothing
else was changed, so nothing else has to be put back. Measured twice: 226 s
and 242 s from the command to a settled smoke, so about 4 minutes, inside the
5-minute target; visitors start reaching the old demo again about 3 minutes
in (section 7). The new site keeps whatever was recorded on it.

## 7. The rehearsal (2026-10-28, 19:26–20:10 CDT)

A throwaway copy of the playtest's load balancer, built the same way and
removed afterwards (`pt-rehearsal-*`, about 45 minutes in all): serverless NEG `pt-rehearsal-neg` → Cloud Run
`aethyr-demo` (read only: GETs of `/`, `/haunt.html`, `/kiki/index.html`,
`/api/config`), backend `pt-rehearsal-web`, URL map `pt-rehearsal-lb`, a
self-signed certificate for the made-up host `pt-rehearsal.openagents.com`
(never in DNS: `curl --resolve` to the address), HTTPS proxy and forwarding
rule `pt-rehearsal-https` on 136.68.133.32. The script ran exactly as it will
for the real host, with `--map pt-rehearsal-lb --old pt-rehearsal-web --host
pt-rehearsal.openagents.com --ip 136.68.133.32 --insecure`.

Building it: NEG 3 s, backend 55 s, backend attach 91 s, URL map 5 s,
certificate 2 s, proxy 4 s, forwarding rule about 65 s; first answer through
it 83 s later.

| Step | Map imported | First clean pass | Settled (5 clean passes) | Total |
| --- | --- | --- | --- | --- |
| Baseline smoke (old) | | 1 s | 15 s | 15 s |
| Data import, staging (incremental, nothing new) | | | | 10 s (and 9 s, 10 s on re-runs: 0 added) |
| Production walk (goldens `--walk`) | | | | 52 s, passed |
| Round 1 switch | 48 s | 117 s | 131 s | 180 s |
| Round 1 rollback | 18 s | 197 s | 208 s | 226 s |
| Round 2 switch | 18 s | 189 s | 202 s | 221 s |
| Round 2 rollback | 18 s | 213 s | 224 s | 242 s |

A probe asking `/` once a second during round 2 shows how the edge spreads a
map. Switch (map in at 19:54:38): the first 301 at 34 s, then mixed, and only
301 from 2 min 28 s. Rollback (map in at 19:58:19): the first 200 from the old
demo at 2 min 47 s, and only 200 from 3 min 29 s.

Repointing the HTTPS proxy at a second, ready-made map instead of editing the
map (tried once, by hand): proxy updated in 23 s, the first 301 at 89 s, 20 in
a row at 142 s; back in 22 s, the first 200 at 152 s, 20 in a row at 178 s
(200 s in all). The same order as editing the map, so the script keeps the
single map; keeping `aethyr-demo-lb` untouched and swapping proxies is the
alternative if the owner prefers that the old map is never edited.

Every check passed in the settled passes: `/`, `/kiki/index.html` and
`/home.html?x=1` → `/verse?from=playtest`; `/haunt.html` →
`/realms/haunted-house?from=playtest`; `/haunt.html?visit=planet-06258408…` →
`/realms/haunted-house?from=playtest#planet-06258408`; `/analytics.html` →
`/admin/analytics`; `/api/config` 200 from the old service. Both targets draw
in headless Chrome (the landing at 390 px, the shared world's house at
1280 px).

**What the rehearsal taught.** Google's edge takes up a new URL map front end
by front end: for two to three minutes after an import some requests get the
old map and some the new. An earlier version of the script gave up after 3
minutes and rolled back by itself mid-spread (twice, during the first
rehearsal attempt): the smoke now waits up to 8 minutes for five clean passes
in a row before calling it failed. That self-rollback also worked, which
proved the rollback path a third and fourth time. The edge sometimes writes
the default port into its own redirects (`https://openagents.com:443/verse`);
browsers drop it, and the smoke treats the two as the same.

**Nothing about playtest.openagents.com changed**: `aethyr-demo-lb` exported
before and after the rehearsal is the same two lines with the same
fingerprint (`GMDdx6n4-Ms=`), and the rehearsal's resources were deleted
(section 8).

## 8. Clean-up of the rehearsal

Deleted at 20:09–20:10 CDT, in order: forwarding rule (19 s), HTTPS proxy
(4 s), certificate (1 s), the two URL maps (5 s), backend service (58 s), NEG
(2 s). `gcloud compute … list --filter="name~pt-rehearsal"` returns nothing
for all six kinds afterwards. Then, read again: `aethyr-demo-https` still
points at `aethyr-demo-lb` with `aethyr-demo-domain-cert`, the map is the
same two lines (fingerprint `GMDdx6n4-Ms=`), Cloud Run `aethyr-demo` is still
on `aethyr-demo-00003-z68`, and https://playtest.openagents.com/ answers 200.

## 9. What the owner checks before saying yes

1. The parity review (`pt-parity-review`): each demo screen next to its new
   one, and the known differences.
2. On openagents.com/admin/analytics: the playtest's sessions are there, and
   the counts in the last import's output match (every `held` equals its
   `source`).
3. On a phone and a desktop: openagents.com/verse → ENTER (guest) → the
   galaxy → the Haunted House, and an email entry.
4. An old shared world opens: https://openagents.com/realms/haunted-house#planet-06258408.
5. That guests' progress on the old host doesn't follow them (section 5) is
   acceptable.
6. Which target: main's `/verse`, or a pinned demo (`/demo/playtest/verse`).

Then the yes, and a time. The switch itself is one command and about
5 minutes, and the rollback one command and about 4.
