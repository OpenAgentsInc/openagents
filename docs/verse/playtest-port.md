# The playtest, inventoried for the port to openagents.com

Task `pt-inventory` (#febd) of the Verse project (https://openagents.com/projects/verse).
Written 2026-10-28 by claude-analytics-1 from the aethyr-cpp repo
(`~/work/aethyr-cpp`, OpenAgentsInc/aethyr-cpp) and the live demo.

**What is live.** playtest.openagents.com serves the tag `demo/kiki-2026-10-28d`
(commit `55a0f3b`, Cloud Run `aethyr-demo` revision `aethyr-demo-00003-z68`,
image `aethyr-demo:55a0f3b366fd…`). The worktree `~/work/aethyr-demo` is still at
`demo/kiki-2026-10-27d` (`fd23217`), a day behind: the analytics parity work
(`4980370`, "Analytics never lose visibility") is only in the newer tags. Every
source path below is at `demo/kiki-2026-10-28d` unless it says otherwise; read
it with `git -C ~/work/aethyr-cpp show demo/kiki-2026-10-28d:PATH`.

The flow people play: the pinned Kiki landing (`frozen/kiki/`, revision 9 in
`frozen/kiki.pin`) at `/`, whose one button, Playtest Now, opens main's
Haunted House, `web/haunt.html`. Analytics follow a person across both.

## 1. Analytics (first: it unblocks `pt-capture` and `pt-dashboard`)

### Sources

| Path | What it is |
| --- | --- |
| `tools/analytics.py` | Storage, cleaning, automation rules, the dashboard's numbers, journeys, health, CSV |
| `tools/analytics-contract.json` | The events each surface must send (`journey`: one run; `live`: production, last day) |
| `tools/analytics_access.py` | The dashboard's emailed-code login for the addresses in `VERSE_ANALYTICS_VIEWERS` |
| `tools/analytics-check.mjs`, `tools/analytics-check.py` | Before and after every demo deploy: plays the flow in headless Chrome and refuses a build that stops sending a contract event |
| `web/analytics.js` | Main's pages (the Haunted House among them): the page side |
| `frozen/kiki/web/app.js` (`Track`, `loadTimes`) | The Kiki landing's page side |
| `web/analytics.html` | The owner's dashboard |
| `web/feedback.html` | The feedback view (survey answers and remarks) |
| `tools/serve.py` (`analytics()`, `/api/events`, `/api/event`, survey and join routes) | The routes |
| `tests/test_analytics.py`, `tests/test_analytics_journeys.py`, `tests/analytics.test.cjs` | The checks |
| `INVARIANTS.md` "Analytics keep words only from pages that say so, and never rewrite", "Analytics never lose visibility" | The rules |
| `docs/owner-decisions.md` 4.2 ("save everything"), `docs/kiki-playtest-2026-10-25.md`, `docs/environments.md` "Kiki flow revisions" | The decisions |

### The event as stored

One JSON object per event, a line each, in append-only files
`<namespace>/events/<UTC time>-<writer>-<n>.jsonl`; namespace `""` is the Kiki
landing's, `"dev"` main's pages' (chosen by the request's Referer: a `/kiki/`
page or `/` is the Kiki flow).

| Field | Meaning | Set by |
| --- | --- | --- |
| `t` | server time, ms | server |
| `sid` | the page session: Kiki `kiki-<player>-<visit>`; main `s<base36 time><random>` kept in `sessionStorage['verse.analytics.sid']` | page |
| `acct` | account id from a verified bearer token, else null | server |
| `ev` | `^[a-z][a-z0-9-]{0,31}$` | page |
| `page` | `kiki`, or main's world name (`haunt`) or file name; ≤ 32 chars | page |
| `ct` | the page's own clock, ms (orders a session's steps) | page |
| `p` | ≤ 16 props, keys `^[A-Za-z][A-Za-z0-9_]{0,23}$`, strings ≤ 80, `text` ≤ 500 kept only with `consent: true` | page, cleaned by server |
| `auto` | the automated tool: header `X-Verse-Automation`, cookie `verse-automation`, or the body's `auto: "webdriver"` | server |

Limits: 50 events a batch, 5,000 per session per server run. A headless or
robot User-Agent (`bot_agent()`, a long regex of crawlers and headless
browsers) is written into the session-start's `p.agent`.

**Likely automated** (decided when read, never stored): `agent`;
`viewport-1050x1893` (no input in that window); `emulated-device` (a phone OS
with no touch screen, or a 2000x2000 screen, no input); `no-input-timeout`
(desktop, no input, ended 29.5–32.5 s in without hidden/visible/resize).
"No input" means only `INPUT_FREE` events: session-start, landing, alive (not
walked), hidden, visible, resize, session-end, error, load-landing, load-engine.

### Every event and its fields

The Kiki landing (namespace `""`, `page: "kiki"`; `frozen/kiki/web/app.js`), as
recorded in the bucket (1,106 events, 104 sessions on 2026-10-28):

| Event | Props |
| --- | --- |
| `session-start` | pid, rev, returning, os, browser, mobile (pointer coarse), sw, sh, vw, vh, dpr, lang, tz, ref (referrer host), entry, gpu; server adds agent |
| `landing`, `map`, `world`, `setup`, `planet`, `edit`, `place`, `publish`, `share`, `invite`, `survey-card`, `survey-sent` | the flow's steps (none) |
| `start` | how (email, guest), to (haunt) |
| `load-landing` | ms, mobile, bg, fcp, dcl |
| `load-engine` | ms, mobile, bg, dataMs, dataKB, cached, wasmMs |
| `alive` (30 s) | ms, active, screen, world, walked |
| `hidden`, `visible` | ms |
| `resize` | vw, vh |
| `session-end` | ms, active, screen, walked |
| `error` | msg, src, line |
| `click` | el, x, y (0–1 of the window), screen, id, card, go, label, name, value, on |
| `key` | key (code), screen, world, editing (walking keys only counted, in alive's walked) |
| `area` | area, world |
| `quest` | step, n |
| `world-enter` | world, id, mine, version |
| `join`, `join-open` | email, how, late, returning |
| `email-ask`, `email-ask-no` | moment |
| `planet-click` | planet, via, open |
| `planet-made` | kind, art, cam, cycle, text + consent |
| `change`, `revert` | world, label, drafts |
| `published`, `publish-failed` | world, id, version, changes, summary / error |
| `prompt` | via, ok, route, score, text + consent |
| `chat` | room, text + consent |
| `voice`, `voice-on` | to, ok, text + consent |
| `survey-show`, `survey-answer`, `survey-later`, `survey-open`, `survey-submit`, `survey-incomplete`, `survey-failed`, `survey-video` | q, n, of, a, answered, saved, href, and the form's answers |

Main's pages (namespace `"dev"`, `web/analytics.js` plus calls in the page
scripts), as recorded (557 events, 29 sessions):

| Event | Props | Source |
| --- | --- | --- |
| `session-start` (once per tab session) | pid (`localStorage['verse.player']`, the landing's), rev `main`, os, browser, mobile, sw, sh, vw, vh, dpr, lang, tz, gpu, ref, entry, landing | analytics.js |
| `page-view` (every load) | pid, from (same-site path or other host), path | analytics.js |
| `world-enter` | world, planet, visit | analytics.js |
| `load-engine` | ms, mobile, page, bg | analytics.js (the `#loading` screen gone) |
| `alive` (30 s), `hidden`, `visible`, `resize` | ms, active, walked / vw, vh | analytics.js |
| `world-leave` / `page-leave` | ms, active, walked | analytics.js (`pagehide`, beacon) |
| `error` | msg, src, line | analytics.js |
| `click` | el, x, y, id, label | analytics.js |
| `key` | key | analytics.js |
| `area`, `objective` | area / text (the HUD's room and goal) | analytics.js |
| `prompt` | via (chat, npc, setup, guide, voice), ok, route, score, text + consent | commands.js, characters.js, planet-setup.js, grid-ui.js |
| `edit-open`, `edit-close`, `edit-pane` | ms / pane | edit-mode.js |
| `place`, `spawn`, `build`, `delete` | item, via | edit-mode.js, haunt.js |
| `publish`, `share`, `change-offer`, `change` | what, kind, art, cam, cycle / changes, price | haunt.js, changes.js |
| `login`, `logout` | method | account.js |
| `survey-show`, `survey-answer`, `survey-skip`, `survey-card`, `survey-sent` | q, moment, world, a, text + consent | survey.js |
| `remark` | kind, via, moment, world, after, editing, score, text + consent | survey.js (feedback said in chat) |
| `chat`, `voice`, `npc`, `guide`, `setup`, `planet` | text + consent where words | chat.js, characters.js, grid-ui.js |

**The contract** (`tools/analytics-contract.json`): landing journey and live
`session-start, landing, load-landing, click, start`; house journey
`session-start, page-view, world-enter, load-engine, click, alive, edit-open,
place|spawn|build, world-leave`, live `session-start, page-view, world-enter,
load-engine, click, alive`.

**Disclosure.** Main's pages say it in Settings › Privacy (`web/hud/hud.js`
line 671): "We save what you do here, including what you say and type, with
an anonymous id that is tied to your account when you log in. It helps the
Verse understand more." The Kiki landing shows none (revision 3: every
session recorded, no opt-out).

**Not recorded by the playtest:** IP address (only hashed with a per-run salt
for guests of `/api/event`), geography, full User-Agent string, field
keystrokes, passwords and tokens.

### The dashboard (`web/analytics.html`, `GET /api/analytics`)

Controls: flow (`playtest` joined journey, `kiki`, `dev`), sessions (people,
automated, both), from/to; exports CSV, JSON, emails CSV, survey CSV. Cards:
tiles (sessions, players, emails, logged in, events, survey answers, finished
survey, median session, prompts understood, automated left out), funnel with
median time per step, where sessions stopped, prompts by via, not understood
(with words), events by count, understood as (routes), load time (landing and
engine, phone and desktop, median and p75, data download), emails (where they
came from), playtesters, sessions (started, email, player, length, furthest
step, device, clicks, keys, lines said, events, automated), survey answers,
Health (per surface: last event, last hour and day, missing live events).
Funnels: `kiki`, `dev`, `playtest` (`FUNNELS`, `STEP_ALIASES` in analytics.py).
Journeys: a main-page session joins the landing visit by player id, else by
time within 120 s of Playtest Now (`journeys()`).

Access: the owner key (secret `aethyr-demo-owner-token`, bearer) or an emailed
code for the addresses in `VERSE_ANALYTICS_VIEWERS` on the service.

### Stored files (bucket `aethyr-demo-data`, 592 KB on 2026-10-28)

| Path | What |
| --- | --- |
| `events/*.jsonl` (252 files) | the Kiki landing's events |
| `dev/events/*.jsonl` (234 files) | main's pages' events |
| `playtesters/*.json` (30) | joins: id, at, email, player, sid, how, auto |
| `survey-answers/*.json` (8) | one answer each (revision 7): id, at, v, rid, q, a, skip, n, of, player, sid, user, joinedAs |
| `surveys/*.json` (8), `survey.json` | the form sent once (before revision 7) |
| `funnel.json` | step counts from revisions 1–2 |
| `players.json` | player keys and the worlds they own |
| `awards.json` | each user's planet |
| `world-<id>.json`, `log-<id>.json` | published worlds and their edit logs (haunted-house, three planets) |
| `chat-<room>.json` | room chat |
| `grid-suggestions.json` | the Grid's suggestions |
| `dev/…` | main's pages' own copies of the above |

Bucket versioning is on; the service mounts it at `/data` (gcsfuse) and is
its only writer (max 1 instance).

## 2. Pages

The Kiki flow (`frozen/kiki/web/`, served under `/kiki/`, its landing at `/`):

| Page | What it does |
| --- | --- |
| `shell.html` + `app.js` (`/`, `/kiki/index.html`) | The landing: title, faint orbs, Playtest Now (to `/haunt.html`); also the pinned map (galaxy), world, planet setup and survey screens of revisions 3–8; the later email ask; Track analytics |
| `survey-steps.js` | The survey, one question at a time |
| `grid.html`, `town.html`, `coding.html` | The pinned Grid, Medieval Town and coding pages |

Main's pages (`web/`), the ones the playtest reaches first:

| Page | What it does |
| --- | --- |
| `haunt.html` (+ haunt.js, haunt-world.js, haunt-edit.js, edit-mode.js, hotbar.js, quest.js, characters.js, chat.js, survey.js, planet-setup.js, changes.js, hud/hud.js) | The Haunted House: walk, haunts, characters, edit mode, publish, share, planets, survey, chat, voice |
| `home.html` | The orb home (the Verse) |
| `analytics.html`, `feedback.html` | The owner's dashboard and feedback view |
| `grid.html`, `town.html`, `icecream.html`, `dead-gears.html`, `coding.html`, `wallet.html`, `payments-lab.html`, `payment-network.html`, `agent-economy.html`, `sandbox.html`, `hello.html`, `crowd.html`, `ecs-bench.html`, `lighting-test.html`, `icecream-legacy.html` | Other worlds, labs and benchmarks, not in the playtest flow |

## 3. Endpoints (`tools/serve.py`)

Bodies are JSON. "Owner" is `Authorization: Bearer $VERSE_OWNER_TOKEN`.

| Route | Request → response |
| --- | --- |
| `GET /api/config` | → `{googleClientId, github, email, publicUrl}` |
| `POST /api/auth/google` | `{credential}` → account + token |
| `GET /api/auth/github/start?next`, `/callback?code&state` | redirect, then `next#login=<token>` |
| `GET /api/session/<token>` | → the account behind a one-time token |
| `POST /api/auth/email/start`, `/verify` | `{email}` / `{email, code}` → account + 90-day token (`tools/email_login.py`, Resend) |
| `GET /api/me`, `POST /api/auth/logout`, `POST /api/account/claim {guest}` | session account |
| `POST /api/player`, `GET /api/player` | mint / read a player key (`X-Verse-Player`) → `{key, id, account, worlds}` |
| `POST /api/worlds` | → `{id}` a new world owned by the player |
| `GET/POST /api/world/<id>` | the published document / publish `{doc, summary}` (owner only, else 403 with `offer`) |
| `POST /api/world/<id>/open`, `GET/POST /api/world/<id>/settings`, `POST /api/world/<id>/reset`, `GET /api/world/<id>/owner` | open publishing, view settings, reset, ownership |
| `GET/POST /api/progress/<world>/<name>`, `…/erase` | a player's save `{bytes, at}`, 409 on a later save |
| `GET /api/log/<id>` | the edit log |
| `POST /api/change/<id>`, `…/<cid>/accept`, `…/decline` | offered changes `{summary, price, delta}` |
| `/api/agent-wallet/…`, `/api/settlement/…`, `GET /api/settlements`, `GET /api/wallet` | agent wallets and simulated settlement (403 from the Kiki pages) |
| `GET /api/chat/<room>?after=N`, `POST /api/chat/<room>` | room chat `{name, text, sid}` |
| `POST /api/presence/<room>` | `{id, name, x, z, yaw, seed, speed, look}` → everyone seen in 6 s |
| `POST /api/award`, `POST/GET /api/planets`, `GET /api/planets/<id>`, `GET/POST /api/planets/mine` | planets |
| `POST /api/event`, `GET /api/funnel` | revisions 1–2 step counter |
| `POST /api/playtest/join` | `{email, player, sid, how}` → `{joined}`; a file in `playtesters/` |
| `POST /api/survey`, `POST /api/survey/answer` | the form / one answer → `surveys/`, `survey-answers/` |
| `POST /api/npc/ask` | `{npc, world, history, question}` → a character's answer (Claude via `tools/npc_chat.py`), 503 without a key |
| `POST /api/events` | `{sid, events, auto?}` → `{kept}` |
| `GET /api/analytics?flow&from&to&automated`, `/me`, `/health`, `/events.json`, `/events.csv`, `/feedback.json`, `/feedback.csv`, `/survey.csv`, `/emails.csv` | the dashboard (owner or viewer) |
| `POST /api/analytics/login/start`, `/verify` | the dashboard's emailed code |
| `/api/coding/…` (`tools/coding_cloud.py`), grid services (`tools/grid_services.py`) | the coding page and the Grid, not in the playtest flow |

## 4. Hosting

| Piece | Setup |
| --- | --- |
| Cloud Run | `aethyr-demo`, us-central1, gen2, min 1 / max 1 instance, startup CPU boost; env `VERSE_DATA=/data`, `VERSE_ANALYTICS_VIEWERS`, `GITHUB_REDIRECT_URI`; secrets for the owner token, Resend, Anthropic; deployed by `tools/deploy-demo.sh <tag>` |
| Data | bucket `aethyr-demo-data` mounted at `/data` (gcsfuse), versioning on |
| Load balancer | forwarding rules `aethyr-demo-https` and `aethyr-demo-http` on 34.110.190.11; backend service `aethyr-demo-web` → serverless NEG `aethyr-demo-neg` |
| Certificate | Google-managed `aethyr-demo-domain-cert` for playtest.openagents.com, ACTIVE |
| DNS | playtest.openagents.com A 34.110.190.11 (Cloudflare) |
| Local | launchd `com.openagents.aethyr-demo` runs `tools/serve.py 8100` from `~/work/aethyr-demo`, data `~/work/aethyr-demo-data` |

openagents.com: one load balancer, `coaxial-openagents-web-staging` (136.69.24.60).
Host rule openagents.com → path matcher `production`: `/`, `/index.html`,
`/favicon.svg`, `/projects`, `/build/*`, `/coaxial/*`, `/realm/*`,
`/projects/*` → backend bucket `coaxial-openagents-web`; `/project*` → 301 to
`/projects*`; everything else → backend service `coaxial-coder` (the old
Cloud Run `coder`). staging.openagents.com → matcher `staging`, default the
staging bucket. On production, `coaxial-coder` answers 404 under `/api/`
and 303 at `/admin`, so `/api/` and `/admin/analytics` are free for new rules.

## 5. What openagents.com already covers

`/realm/haunted-house` (`apps/openagents-web/realm.coax`, `realm-face.c`,
`metareal/haunt*.coax`, `player.coax`, `grid.coax`): walking (keys, mouse
look, two thumbs), jump, run, zoom, doors, the nine haunts and HAUNTS FOUND,
talking to characters (scripted lines, a hint), the map, views, sounds, the
room title. Nothing else of the playtest yet: no landing, galaxy, edit mode,
publishing, planets, saves, chat, voice, survey, accounts or server, and no
analytics until `pt-capture`.

| Playtest piece | Queue task |
| --- | --- |
| Analytics events and storage | `pt-capture` |
| Dashboard and access | `pt-dashboard` |
| Moving the bucket's data | `pt-import-demo-data` |
| Server, database | `pt-server-skeleton` |
| Landing, email, guest | `pt-landing` |
| Galaxy | `pt-galaxy` |
| Quest, tutorial | `pt-quest` |
| Edit mode, place, publish | `pt-edit-mode`, `pt-player-endpoints` |
| Character chat | `pt-character-chat`, `pt-chat-endpoint` |
| Saves | `pt-saves` |
| Survey, remarks, feedback view | `pt-feedback` |
| Share card | `pt-share-card` |
| Pinned demo, goldens | `pt-pinned-releases`, `pt-goldens` |
| Switch-over | `pt-cutover-plan`, `pt-parity-review` |

## 6. Gaps the queue misses (proposed tasks, for the taskmaster)

1. **pt-analytics-contract**: the analytics contract and journey check in
   `coax/web/release.sh`: play the realm in headless Chrome on staging and
   refuse to promote a build that stops sending a contract event; a Health
   card on the dashboard. (INVARIANTS.md "Analytics never lose visibility".)
2. **pt-presence-chat**: room chat and presence (`/api/chat/<room>`,
   `/api/presence/<room>`, `web/chat.js`): other players in the house and
   the room's chat.
3. **pt-planets**: planet setup and the player's planet (`/api/award`,
   `/api/planets*`, `web/planet-setup.js`), which the galaxy task only shows.
4. **pt-change-offers**: offered changes to someone's world and the edit log
   (`/api/change/*`, `/api/log/<id>`, `web/changes.js`), without settlement.
5. **pt-voice**: the voice bot and voice prompts (`voice`, `voice-on`), after
   the first switch as `coax/web.md` decides.
6. **pt-accounts**: tying a session to a Nostr identity once the Coax
   milestone "Sign in to the website with your phone" lands (the playtest
   used Google, GitHub and email sign-in; openagents.com uses Nostr, never
   WorkOS). The analytics schema already has the column.
