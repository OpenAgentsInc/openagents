# Everglade analytics audit (2026-11-09)

This audit covers how the analytics system works, which steps of the Everglade
flow it records, and where an admin sees each step. It also lists what this
change added and what is still missing.

## How the system works

1. **Sender.** Each Coax app loads `coax/web/analytics.coax`
   (`Track`). The host (`coax/web/host.js`) passes page facts, hide, show,
   leave and error messages (kinds 600-604) to the app. The app batches events
   and posts them to `POST /api/analytics/events`, with a session id of `c`
   plus the tab id.
2. **Collector.** The C API (`coax/web/api/analytics.c`, Cloud Run `oa-api`)
   checks each batch and calls the SQL function `analytics.ingest`. It keeps at
   most 50 events a batch, 5000 a session, 16 properties an event and 80
   characters a string.
3. **Storage.** Postgres keeps `analytics.events` (session id, event, page,
   count, properties, `auto`, space, host, time) and `analytics.sessions`.
   There is no retention job, so events are kept forever.
4. **Team exclusion.** Migration 0037 sets `auto = 'team'` for any of these:
   the `oa_team` cookie, an admin session or key, a team player from
   `team.conf`, or a team device id. `team_tag` spreads the mark to the rest of
   that device's sessions. Automated sessions (webdriver, the
   `verse-automation` cookie or header, a bot User-Agent) are marked as
   automated.
5. **Admin view.** `/admin/analytics` (`apps/openagents-web/admin.coax`) reads
   `GET /api/admin/analytics?view=...` (`coax/web/api/admin.c`). Each filter
   chip asks for one view. By default the view leaves out team and automated
   sessions. OUR TEAM and AUTOMATED put them back in.

## The Everglade flow, step by step

Every Everglade event uses page `everglade2`. "Admin sees it" means the
**EVERGLADE** chip on `https://openagents.com/admin/analytics`, unless the row
says otherwise. Paths are relative to the repository root.

| Step | Event before | Event now | Where it is sent | Admin sees it |
|---|---|---|---|---|
| openagents.com landing (newland) | `load-engine`, `card`, `click`, `key`, `open` (`newland-face.c:206-262`) | unchanged | newland | HOME and the overview |
| Create-character click on the landing | `click` with no name (`newland.coax:1391`, `newland-portal.coax:69`) | **gap**, see below | newland | not as its own step |
| Hand-over through the portal | none | `portal-shown` (`id` door or lander, `ms`) | `everglade/client/track.coax:67` | HOW FAR VISITS GOT, WAYS IN |
| Arrival on everglade.world (`/`, `/play`, `?via=portal`) | none | `session-start`, `page-view` (path, query) from `Track` | `analytics.coax`, `everglade2-face.c:348` | WAYS IN, SESSIONS |
| Engine loaded | none | `load-engine` (`ms`) | `everglade2-face.c:246` | LOADING |
| World loaded | none | `load-world` (`ms`, `mobile`) | `everglade/client/track.coax:62` | LOADING, HOW FAR VISITS GOT |
| No WebGPU, no GPU adapter, shader failure | none | `load-fail` (`msg`, `webgpu`) | `host.js:3781`, `host.js:3793` | LOADING, WHAT WENT WRONG |
| GPU device lost | none | `gpu-lost` (`msg`) | `host.js:1052` | LOADING, WHAT WENT WRONG |
| Renderer or engine stopped | none | `crash` (`msg`) | `host.js:2868`, `host.js:3204` | LOADING, WHAT WENT WRONG |
| Script errors | `error` (host message 604) | unchanged, now counted for Everglade | `analytics.coax` | LOADING, WHAT WENT WRONG |
| Character Create opened | none | `create-open` (`id` portal or direct) | `everglade/client/track.coax:74` | HOW FAR VISITS GOT |
| Character Create choices | none | `create-pick` (field), `create-class`, `create-level`, once each | `everglade/client/track.coax:78-80` | WHAT THEY DID |
| Name entered | none | `create-name` (`ok`) | `everglade/client/track.coax:84` | HOW FAR VISITS GOT |
| Character not saved | none | `fail` `create-save` | `everglade/client/track.coax:87` | WHAT WENT WRONG |
| Character done | none | `create-done` (class, level, seconds in Create) | `everglade/client/track.coax:91` | HOW FAR VISITS GOT |
| Run out into the glade | none | `run-end` (seconds) | `everglade/client/track.coax:99` | HOW FAR VISITS GOT |
| First spawn | none | `spawn` (`id` create or kept, class, level, `ms`) | `everglade/client/track.coax:105` | HOW FAR VISITS GOT, SESSIONS |
| Joined the shared room | none | `room-join` (`ms`); `fail` room, version or elsewhere | `everglade/client/track.coax:111-117` | HOW FAR VISITS GOT, LOADING |
| The graphics quality, 20 s in | none | `graphics` (`class`, `gpu`, `vendor`, `arch`, `mode`, `start`, `rung`, `settled`, `fps`) | `everglade/client/track.coax` `e2_track` | none yet |
| Combat | none | 30-second `tally`: `cast_<spell>`, `refused`, `bow`, `hit_<foe>`, `hurt_<foe>`, `kill_<foe>` | `everglade/client/track.coax:120-129` | WHAT THEY DID, HOW FAR VISITS GOT |
| Death | none | `death` (`id` the foe, or other) | `everglade/client/track.coax:138` | WHAT THEY DID |
| XP and level-ups | none | `tally` `xp`; `level-up` (`n`) | `everglade/client/track.coax:132-134` | WHAT THEY DID |
| Session length and return visits | `session-start`, hide, show, leave from `Track` | unchanged, now on for Everglade | `analytics.coax` | BY DAY (PEOPLE, CAME BACK), SESSIONS |
| Quests, chat, party | none | none: Everglade II has none of these yet | not applicable | not applicable |
| Admin mode | none | none: it exists only in dev builds | not applicable | not applicable |
| FPS and quality tier | none | left to #13071 | not applicable | not applicable |
| Funnel with drop-off | none | 13 steps, built from the events above | `admin.c:452` | HOW FAR VISITS GOT, BY DAY |

## What this change added

- **Everglade events.** `everglade/client/track.coax` turns
  Everglade's own state into events, so the game logic itself needs no
  analytics calls. `everglade/client/net.coax` counts your seat's hits, harm, kills,
  XP and level from the room's wire lines. `everglade2-face.c` starts `Track`
  and sends its batches. Two `#test`s cover the flow and the per-seat XP.
- **Load failures and crashes.** `host.js` sends a `sendBeacon` for
  `load-fail`, `gpu-lost` and `crash`. These can happen before the app, or
  after it stops, so the app can't send them itself.
- **The admin report.** The new `view=everglade` in `admin.c` and the
  EVERGLADE chip in `admin.coax` show these sections:
  - HOW FAR VISITS GOT: each step, how many sessions got that far, and how many
    stopped there.
  - WAYS IN: front door, `/play`, portal, or direct.
  - BY DAY: the funnel by day, with people and people who came back.
  - LOADING: median and slowest-tenth load times, and the failure rate.
  - WHAT WENT WRONG: grouped failures and their messages.
  - WHAT THEY DID: Character Create choices, combat, deaths and XP.
  - SESSIONS: recent sessions. Selecting a row opens that session's timeline,
    and Back returns to EVERGLADE.

  The overview also has an EVERGLADE.WORLD card.
- **Team play on everglade.world.** The `oa_team` cookie belongs to
  openagents.com, so it never reached everglade.world. An admin can now open
  `https://openagents.com/api/admin/team-link?host=everglade.world`. That link
  goes to a one-time, 10-minute link on everglade.world, which sets the team
  cookie there. Only the token's hash is stored (migration
  `0137_analytics_team_links.sql`), and only listed hosts are accepted.
- **Tests.** `coax/web/api/test_everglade.py` sends fixture sessions and checks
  every section, the team link and the team filter.

## What remains

- **Done since: the front door and the lander click.** The front door (`/`, realms.ts) now sends its
  own session-start, page-view and `play-now-click`, with or without WebGPU. The lander at
  `openagents.com/newlander` sends `create-character-click` at the press, then the tab goes on to
  `everglade.world/play?via=portal`; WAYS IN names those arrivals `openagents.com/newlander portal`
  (a `/play?via=portal` page view that came from openagents.com).
- **A visitor isn't linked across the two domains.** The ids are per origin, so
  the report can't follow a visitor from the openagents.com click to
  everglade.world. WAYS IN shows the `?via=portal` arrivals instead.
- **There is no retention policy.** Events and sessions are kept forever.
- **Host beacons on pages without `data-realm`** use the app name as the page.
- **Quests, chat and party** need events when they ship in Everglade II.

## How it was checked

- The Coax checks pass: `everglade/client/client.coax` (the flow and the per-seat XP checks)
  and `admin.coax` (48 of 48).
- The API checks pass: `test_capture.py`, `test_dashboard.py`, `test_team.py`,
  `test_everglade.py` and `test_health.py`.
- `everglade2.wasm` is 818,675 bytes with #13071's GFX control merged in.
  Its budget goes from 815,000 to 842,000. `admin.wasm` is 54,136 bytes,
  under its 60,000 budget.
- An end-to-end run used a throwaway Postgres, the API and `coax/web/serve.sh`
  with `--api`. It took two headless shots (`coax/web/tool.sh shot`) of
  `/realms/everglade2/domain/play/`, with and without `?via=portal`. Both
  sessions sent these events: `session-start`, `page-view`, `load-engine`,
  `load-world`, `world-enter` and `create-open`. They showed up in
  `view=everglade` with AUTOMATED on: both sessions reached Character Create,
  the world showed in a median of 2.7 seconds, the failure rate was 0%, and
  each way in had one visit. Headless Chrome is marked as automated, so the
  default view leaves these sessions out.

## Deploy

- The API (the report, team link and migration 0137) is the Cloud Run service
  `oa-api`. Deploy it with `coax/web/api/deploy.sh --staging`, then
  `coax/web/api/deploy.sh`.
- The web wasm files and `host.js` ship with the next `coax/web/release.sh`.
- After the API deploy, each team browser opens the team link once.
