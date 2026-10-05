# The Grid in the browser (#10587)

Recorded on 2026-10-05 on a macOS (Apple silicon) contributor machine, with
headless Google Chrome, a scratch relay, and `openagents verse walkers`.

## What runs

- The browser Grid (`everglade-web`, `?zone=grid` or the `/grid` page) joins
  `verse-bare` with the presence session the phone's bare Grid runs
  (`verse::session::Session::start_presence`): the phone's cadence
  (`PublishIntervals::mobile`), its crowd delay and live-only rule, spawn
  recovery, solid avatars, and name tags over heads.
- `verse-net`'s browser `Link` is the browser's WebSocket, driven from its
  callbacks. It shares the native link's protocol state (`net/wire.rs`):
  subscriptions restored after a reconnect or an accepted NIP-42 answer,
  durable events held while offline, one-shot subscriptions closed after
  their stored events, and the native backoff. `Instant` comes from
  `web-time` on the session's path, so the session has a clock in a
  browser.
- The page keeps the player's key, display name, and block and mute lists
  in local storage. A click on a remote player's tag opens the phone's card
  (Block, Mute or Unmute, Close). A blocked player is never drawn.
- The connection line at the top left says when the relay's population cap
  (`NOSTR_RELAY_WORLD_POPULATION_CAP`, 20) refuses this player; the session
  slows down and tries again, and the page keeps drawing the others.
- `openagents-web` serves the page at `/grid`, with the build's policy plus
  `connect-src wss://relay.openagents.com`.
- Once a second the page logs `Grid frames {...}`: frame times, the players
  drawn, the session's status, and its refusals.

## Test mechanism

```sh
cargo test -p verse-net        # wire state, JSON lists, stored key
cargo test -p everglade-web    # query options and the card's buttons
cargo test -p openagents-web --lib grid
./scripts/build-everglade-web.sh /tmp/grid-web
cp crates/everglade-web/index.html /tmp/grid-web/
python3 -m http.server -d /tmp/grid-web 8765
```

A scratch relay ran as `scripts/verse-relay.sh` runs it, on port 7457, with
`NOSTR_RELAY_MAX_CONNECTIONS_PER_IP=64` so 20 walkers and a browser from one
address fit (the default is 20 connections an address). The walkers ran
with a scratch `HOME`:

```sh
openagents verse walkers 20 --relay ws://127.0.0.1:7457 --wait 400
```

Then open
`http://localhost:8765/?zone=grid&relay=ws%3A%2F%2F127.0.0.1%3A7457&name=browser`
(add `&gl` to force WebGL2). Walk with `W`, zoom out with the wheel, and
click a walker's name tag to block it. Headless Chrome was driven over the
DevTools protocol: a wheel zoom at 3 s, then 40 s of real-time frames.

## Results

| Run | Relay cap | What the browser shows | Frames ([console](crowd-gl.console.log)) |
| --- | --- | --- | --- |
| WebGL2 (ANGLE Metal) | 21 | 20 walkers with names, `ONLINE · 21 HERE` ([walkers-20-webgl2.png](walkers-20-webgl2.png)) | 39 s with 20 drawn: 59–60 frames a second, p50 16.7 ms, p95 16.8 ms, no refusals |
| WebGPU | 21 | The same 20 walkers ([walkers-20-webgpu.png](walkers-20-webgpu.png)) | 37 s with 20 drawn: 60 frames a second, p95 16.8 ms ([console](crowd-gpu.console.log)) |
| WebGL2, close | 21 | Names over heads, the browser's own `browser-gl` ([names-close-webgl2.png](names-close-webgl2.png)) | — |
| WebGL2, `walker-0` blocked in storage | 21 | 19 walkers; `walker-0` is never drawn ([blocked-walker-0-webgl2.png](blocked-walker-0-webgl2.png)) | 19 drawn, `live` 19 ([console](block-gl.console.log)) |
| WebGL2, the 21st player | 20 (default) | `THE GRID IS FULL; WAITING FOR ROOM`, still drawing all 20 ([world-full-webgl2.png](world-full-webgl2.png)) | 5 refusals, `rate-limited: world is full` ([console](cap-gl.console.log)) |
| `/grid` on `openagents-web`, `?offline` | — | The Grid on the engine renderer ([site-grid-offline.png](site-grid-offline.png)) | — |

The other side: with the browser joined as `Browser Player` and walking,
`openagents verse who --world verse-bare` on the same relay listed it live
with its name ([verse-who.txt](verse-who.txt)). The walkers' own log is in
[walkers-20.txt](walkers-20.txt).

Frame times are the browser's `requestAnimationFrame` intervals on a 60 Hz
headless display, so 16.7 ms is the ceiling; `render_ms` (under 1.2 ms) is
the renderer's CPU time for a frame.

## Not covered here

- The phone's player seen from the browser, and the browser seen from a
  phone: the phone runs the same session in the same world, but no phone
  ran here. The step is in `NEEDS_OWNER.md`.
- `/grid` on openagents.com: the site is not redeployed by this change. The
  deploy step is in `NEEDS_OWNER.md`.
- The arches lead nowhere in the browser yet; walking through one stops the
  page with a message, as before.
- The chamber transport hook (#10552) is not wired.
