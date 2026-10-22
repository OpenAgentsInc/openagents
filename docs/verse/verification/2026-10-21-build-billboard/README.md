# The build billboard on the Grid homepage (#12442)

The Grid's plaza has a build billboard west of the spawn, beyond the
Lagrange 2 arch and clear of the Gym. It shows a STAGING lane and a
PRODUCTION lane: the commit and subject, the phase on a five-step strip
(BUILD, DEPLOY, SMOKE, SHIFT, LIVE), the elapsed time, the last result, and
the three deploys before it. The active step pulses, and a lane's border
flashes green when a deploy goes live and red when one fails.

The page reads [NIP-DEPLOY](../../../../nips/openagents/NIP-DEPLOY.md)
events from `wss://relay.openagents.com` on the Grid session's own relay
connection and accepts only events the deploy announcer's key signs.

## Check

On 2026-10-21 a local build of `everglade-web` (`scripts/build-everglade-web.sh
--with-pack`) was served with `python3 -m http.server` and opened at
`/?zone=grid&deploys=test` under `openagents browser run`. The check script
[`check_board.py`](check_board.py) published test events (`--test`) to the
public relay with `openagents deploy event` and captured the board after each
step, with no reload. [`receipt.json`](receipt.json) holds what
`verse_build_board()` reported at each step; no event was refused.

| Capture | What it shows |
| --- | --- |
| [`01-spawn.png`](01-spawn.png) | The board from the spawn, with the events the relay already held: a staging build in progress and a production deploy that went live. |
| [`02-staging-building.png`](02-staging-building.png) | Facing the board: staging BUILDING, the BUILD step pulsing. |
| [`03-staging-smoke.png`](03-staging-smoke.png) | Staging in its smoke test, BUILD and DEPLOY done. The earlier run moved to the history as STOPPED. |
| [`04-staging-live-flash.png`](04-staging-live-flash.png) | Staging LIVE: every step green and the lane's border flashing green. |
| [`05-production-failed-flash.png`](05-production-failed-flash.png) | Production SMOKE FAILED: the SMOKE step red and the border flashing red. |
| [`06-settled.png`](06-settled.png) | Both flashes over. |

The page's start marks: the Grid's content, including the Pylon Bay and the
board's 131 models, was ready 143 ms after the glyph atlas.

## The shared computer's screen

The monitor by the spawn shows a lit screen in the board's letters
(`deploy_board::screen`): OPENAGENTS TERMINAL, the Pylons online when the
page knows them, the last deploy that ended, WALK UP TO USE, and a blinking
prompt. [`07-terminal-screen.png`](07-terminal-screen.png) is up close and
[`08-terminal-from-spawn.png`](08-terminal-from-spawn.png) is from the
spawn; the local server has no Pylons, so that line is absent there.

## The tighter layout

Each lane's title, status, strip, commit, and subject now sit above 6.4 m
of the face, the height the Game Lab and Lagrange 2 arches hide from the
spawn, and its last result and history sit below. A test checks those
sight lines. [`09-tight-layout-from-spawn.png`](09-tight-layout-from-spawn.png)
shows the board from the spawn with test events.

## Real deploys

On 2026-10-21 the coordinator deployed `56b0c0d093` with `scripts/deploy/web.sh`.
The relay received every phase from the deploy announcer: staging
`build_started`, `build_done` (Cloud Build `3f05a41d-…`), `deploying`,
`smoke_running`, `smoke_passed`, and `live`; then production `deploying`,
`promoted`, `smoke_running`, `smoke_passed`, `traffic_shifted`, and `live`.

- [`10-staging-real-deploy.png`](10-staging-real-deploy.png): staging.openagents.com
  with staging LIVE and production in its candidate SMOKE TEST.
- [`11-staging-terminal-screen.png`](11-staging-terminal-screen.png): the
  computer's screen there: PYLONS ONLINE: 11, DEPLOYED 1M AGO.
- [`12-production-live.png`](12-production-live.png): openagents.com with
  both lanes LIVE on `56b0c0d`.
