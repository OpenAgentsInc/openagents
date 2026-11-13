# What each realm records

Every page sends the common events in `coax/web/analytics.coax`
(session-start, page-view, load-engine, world-enter, alive every 30 s,
hidden/visible, click, key, error, world-leave with time and active time).
On top of that each realm records what people do, with the helpers in the
same file:

| helper | event | properties |
|---|---|---|
| `track_act` / `track_act_n` | the act's own name (`talk`, `quest`, `wanted`, ...) | `id`, `n`, `world` |
| `track_once` / `track_area` | `visit` (a place entered, once a page load) | `id`, `world` |
| `track_walk` | `walk` at 10, 50, 100, 250, 500, 1000, 2500 m | `m`, `s` |
| `track_tally` | `tally` every 30 s, on hide and on leave | `<act>_<id>`: count |
| `track_setting` | `setting` | `id`, `on` |
| `track_chat` | `chat-sent` (its length, never its words) | `via`, `len` |
| `track_fail` | `fail` (a load or request that didn't work) | `id`, `n` (status) |

No words are recorded by these helpers and no personal data; ids are the
game's own names. The ingest (`coax/web/api/analytics.c`, `analytics.ingest`)
keeps at most 16 short properties an event, 50 events a batch and 5000 a
session.

## By page

- **home** (newland-face.c): load-engine, click, key, `card` (1-5), `open` (where PLAYTEST or OPEN PROJECT went).
  The lander at /newlander (the same module, data-page 3) also sends `create-character-click` (`id` newlander) at the press of CREATE YOUR CHARACTER, before the tab goes on to everglade.world/play?via=portal.
- **galaxy** (verse.coax; the native host only since 2026-10-31, when the web's page became a redirect to /realms): planet-click, voice-on, start, join, planet-create; tally `nav_<slug>` for arrow steps.
- **haunt** and **sandbox** (realm.coax, realm-face.c): area, objective, quest, npc, place, edit, survey, voice; `walk`; `fail piece`.
- **everglade** (everglade-track.coax): visit (place, building), walk, wanted, talk, quest, edit, voice, chat-sent, fail (pack, npc); tally cast_/use_<spell>, kill_guard, kill_folk, death, fire, destroy.
- **everglade2, the front door** (everglade.world `/`, realms.ts `frontDoor`, a beacon from the page itself, with or without WebGPU): session-start (a new tab), page-view (`path` /), `play-now-click` (`id` door, `at` hero or foot, `gpu`).
- **everglade2** (everglade/client/track.coax, Everglade II on everglade.world): load-world, portal-shown, create-open, create-pick, create-class, create-level, create-name, create-done, run-end, spawn, room-join, level-up, death, graphics (class, gpu, vendor, arch, mode, start, rung, settled, fps), fail (create-save, room, version, elsewhere); tally cast_<spell>, refused, bow, hit_/hurt_/kill_<wolf|goblin|guard>, xp. The host also sends load-fail, gpu-lost and crash. Admin: EVERGLADE chip (docs/analytics/2026-11-09-everglade-analytics-audit.md).
- **the-grid** (thegrid-track.coax): visit (station), station, answer, suggest, galaxy, walk, others, setting first-person, fail (pack, suggest).
- **dead-gears** (deadgears.coax): wave, end, setting paused, walk; tally shot, hit, kill, hurt, repair, lift, throw, stun, reload, cover, mode.
- **ice-cream-man** (icecream-face.c): round-start, objective, round-end, tool, difficulty, settings (first-person, timer, jumpscares), visit (room), walk; tally hits, melted, hurt, hide, scare.
- **fairy-forest** (fairy-face.c, fairyroom.coax): visit (40 m square), walk, talk, map, maker, quest-step, quest-done, skill, chat, presence; tally flap, swim.
- **hecates-cemetery** (villa-face.c): the villa-* steps, villa-flight, visit (20 m square), walk.
- **realm-builder** (builder.coax): builder-prompt, setup, board, toggle, describe, make, realm-made, `fail builder-plan|builder-make`.
- **realm-made** (myrealm.coax): world-enter, edits, walk.
- **app** (apps/openagents-ios, AppAnalytics.swift): app-launch, app-foreground, app-background (seconds), grid-fallback, login-open, login-approved, wallet-open, voice-on, gyro (once a foreground), world-enter, fail. The app's web pages carry `OpenAgentsApp/<build>` in the User-Agent and count as app traffic.

`/admin/analytics` draws a REALMS table and a card per page (how far visits
got, then the most done, tallies summed), with the team and automation left
out as everywhere else.
