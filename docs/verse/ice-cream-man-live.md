# The Ice Cream Man, live: a demo of game updates in seconds

Status: a local demo, 2026-10-21 (#12431). It is the first hosted game on
[livehost](../../crates/livehost/README.md), the prototype of
[web deploys in seconds](../audits/2026-10-21-web-deploys-in-seconds.md),
and covers that design's "Hosted sites and games" for one game.
[The Ice Cream Man](games/the-ice-cream-man.md) runs as a livehost tenant
with a portal on the Grid. You change its numbers, its code, or its art,
deploy, and players already in the house get the change between two steps
of the game, without a reload, still holding their keys.

Everything runs on this Mac: a local relay, a local host, and a browser.
Nothing touches staging or production.

## How it fits together

| Piece | What it is | Code |
| --- | --- | --- |
| The tenant | `icecream` on livehost: its own key, manifest history, and `live` channel. A deploy uploads only the objects whose hash changed and signs a pointer (kind 30440) to the new manifest | [`crates/icecream-live`](../../crates/icecream-live/) |
| The game's rules | `icecream-rules` built as a plain WebAssembly module with no imports and five exports. The page keeps the game state and passes it through the module each step (30 a second), so the page can drop the module for another between two steps | [`crates/icecream-logic`](../../crates/icecream-logic/), [`icecream_rules::live`](../../crates/icecream-rules/src/live.rs) |
| The art | The game's own meshes, which it makes in code, baked into content-addressed chunks: one per room of the house (49), one for the Ice Cream Man, one for the kid. 22 bytes a vertex. The game has no texture or sound files (it makes its sounds in the page), so there are no texture or audio chunks | [`icecream_zone::chunk`](../../crates/icecream-zone/src/chunk.rs), [`bake.rs`](../../crates/icecream-live/src/bake.rs) |
| The zone index | `zone.json`: the house's seed, where each room chunk sits, the shell, and the first-play budget. It plans what loads first, what streams next, and what an update must fetch | [`crates/icecream-zone`](../../crates/icecream-zone/) |
| The shell | `icecream-web` in live mode: it draws, reads input, follows `/t/icecream/_live`, fetches only new hashes, and applies a version at the next safe point. Its page has `data-livehost-self`, so livehost's injected client doesn't reload it | [`live.rs`](../../crates/icecream-web/src/live.rs), [`app.rs`](../../crates/icecream-web/src/app.rs) |
| The portal | An arch on the Grid lettered ICE CREAM MAN. Within 24 m of it the Grid prefetches the zone's first play; walking through opens the zone over the Grid; **Back to the Grid** returns you in front of the arch | [`zones/gate.rs`](../../crates/verse/src/zones/gate.rs), [`everglade-web/src/hosted.rs`](../../crates/everglade-web/src/hosted.rs) |
| Game Lab | Its on-save hook (`GAME_LAB_ON_SAVE`) runs `icecream-live lab` after each saved edit, so a chat's "make him 20% slower" reaches the zone | [`crates/game-lab/src/lab.rs`](../../crates/game-lab/src/lab.rs) |

### What a player sees when a version lands

The page fetches what changed while the game keeps running, then applies it
at the top of a frame, before any step:

- **New numbers** (`tuning.json`): the run retunes. Speeds apply at once.
  Two more hiding spots are added to the same house, and only the rooms that
  gained one are fetched again. A change to the house's size or shape waits
  for the next run, and the player is told so.
- **New code** (the logic module): the next step runs on the new module,
  from the same state. A module that needs a newer state than the page has
  is refused, and the run carries on with the old one.
- **New art** (a chunk): the mesh is replaced in the next frame. A room
  that hasn't streamed in yet arrives with its new hash later.
- **A new shell** (the page or the drawing module): the page stores the run,
  reloads, and carries on from it.
- **A broken build**: if the logic module fails a step, the run stops where
  it is and the player sees that it's paused. A rollback brings the old
  module back, and the run continues from the same step.

The house is the zone's (`zone/level.json`), the same for every player, so
the chunks are baked once per version and any number of players share them.
Each player's run is their own and local; no state is shared between players.

### Loading

- **First play:** the page, its drawing module, the logic, the tuning, both
  characters, and the rooms within 16 m of the spawn (4 of 49). The budget
  is 3.5 MB uncompressed and 1.5 s; a deploy over it is refused on `live`
  and warns on `staging` (livehost's pre-publish check,
  `icecream_zone::budget::check`).
- **Streaming:** the other 45 rooms come in by distance from the player,
  four at a time, while they play.
- **Prefetch:** on the Grid, within 24 m of the arch, the page fetches the
  first play into the browser's cache. Everything after the page itself is
  named by hash, so the cache keeps it for good.
- **Updates:** an object whose hash a player has seen is never fetched
  again. Publishing a version a player already had costs only its manifest.

## Run the demo

You need the pinned toolchain's `wasm32-unknown-unknown` target, the
`wasm-bindgen` CLI at the version `Cargo.lock` pins, Homebrew's `llvm` (for
the Grid's build) and `postgresql@16`, and Google Chrome.

1. **Set up the build environment.** The demo's WebAssembly builds use
   their own target folder, so a deploy never waits on another build's lock
   (the same reason livehost's guest builds have `LIVEHOST_GUEST_TARGET`):

   ```sh
   cd ~/code/openagents   # or your worktree
   export CARGO_TARGET_DIR=~/work/openagents-target-1-icecream-wasm
   . scripts/dev/cargo-build-env.sh
   cargo build -p icecream-live -p nostr-relay --release
   B=$CARGO_TARGET_DIR/release/icecream-live
   ```

2. **Start a relay** in its own terminal, on ports of its own:

   ```sh
   PATH=/opt/homebrew/opt/postgresql@16/bin:$PATH \
   LIVEHOST_RELAY_PORT=7458 LIVEHOST_RELAY_PG_PORT=55458 \
     crates/livehost/scripts/relay.sh ~/.openagents/scratch/icl-relay
   ```

3. **Start the host** in another terminal. It makes the two tenants' keys
   and serves both on `127.0.0.1:8790`:

   ```sh
   $B serve --relay ws://127.0.0.1:7458
   ```

4. **Build and deploy.** `shell` builds the drawing module (about two
   minutes the first time); `deploy` builds the logic module, bakes the
   chunks, checks the budget, and publishes. Then build the Grid and deploy
   it as the `grid` tenant with a page that loads it:

   ```sh
   $B shell
   $B deploy --relay ws://127.0.0.1:7458
   EVERGLADE_PROFILE=release scripts/build-everglade-web.sh ~/.openagents/icecream-live/grid-build
   cp crates/icecream-live/zone/grid.html ~/.openagents/icecream-live/grid-build/index.html
   $B grid ~/.openagents/icecream-live/grid-build --relay ws://127.0.0.1:7458
   ```

5. **Open the Grid** in Chrome:
   <http://127.0.0.1:8790/t/grid/?zone=grid&offline&hosted=icecream>.
   The ICE CREAM MAN arch stands to the right of the Lagrange 2 arch. Walk
   toward it (the zone is prefetched before you reach it) and through it.
   To watch a player who never stops, open
   <http://127.0.0.1:8790/t/icecream/#bot> in a second window: the game's
   bot plays it.

6. **Change the numbers.** Make the Ice Cream Man 20% slower and add two
   hiding spots in `crates/icecream-live/zone/tuning.json` (Game Lab's
   names):

   ```json
   { "man_wander": 1.76, "man_investigate": 2.72, "man_search": 2.56, "man_chase": 4.48, "extra_closets": 4 }
   ```

   then deploy and wait for the page to say it has the change:

   ```sh
   $B deploy --relay ws://127.0.0.1:7458 --wait 30
   ```

   The line it prints has `edit_to_live_ms` and the page's ack: the tick it
   applied at, its keys and health, and the bytes it fetched.

7. **Change the code.** In `crates/icecream-rules/src/tuning.rs`, set
   `VOLLEY_PELLETS` to 7 (seven sprinkles a volley instead of five) and
   deploy the same way. Only the logic module rebuilds.

8. **Change the art.** Make his forehead mint in
   `crates/icecream-live/zone/palette.json`:

   ```json
   { "man": { "FFE4D0": "8FD3B4" } }
   ```

   and deploy. One chunk changes.

9. **Roll back.** `$B rollback --relay ws://127.0.0.1:7458 --wait 30` points
   `live` at the manifest before the current one. To show a rollback saving
   a broken build, make the logic module fail (for example, a `panic!` in
   its `step` operation in `crates/icecream-logic/src/lib.rs`), deploy, see
   the run stop, and roll back.

10. **From a chat.** Start Game Lab with the hook, on a folder of its own:

    ```sh
    GAME_LAB_ON_SAVE="$B lab --relay ws://127.0.0.1:7458" \
      cargo run -p game-lab -- serve --port 8794 --dir ~/.openagents/scratch/gamelab
    ```

    A `game_edit` on it, whether a chat makes it or you do with
    `curl -X POST 127.0.0.1:8794/api/tool/game_edit -d '{...}'`, saves the
    version, runs the bot's 24 checking runs, publishes the version's
    numbers to the zone, and answers with the published manifest.
    `scripts/dev/full-local.sh start --lab` points the web chat at a Game
    Lab; start that Game Lab with the hook to talk to the zone.

Put the sources back with `git checkout crates/icecream-live/zone
crates/icecream-rules crates/icecream-logic` and deploy once more.

### Measure it

```sh
openagents browser run --timeout 60 -- $B measure --relay ws://127.0.0.1:7458 --runs 3 --lab http://127.0.0.1:8794
openagents browser run --timeout 60 -- $B grid-check
```

`measure` opens a page where the bot plays and makes each change in turn:
steps 6 to 9 and a Game Lab edit. It times each one from the moment the edit
is saved to the moment that page acks it, and puts the sources back at the
end. `grid-check` walks the Grid path: the arch, the prefetch, the walk
through, the zone's first play inside the Grid, and the way back.

## Measurements

Measured on 2026-10-21 on the owner's Mac (18 cores) while other agents
were building on it (load average 15 to 18 during the runs, 30 over 15
minutes), with a headless Chrome, all on localhost. `measure --runs 3`; the
raw lines are in `~/.openagents/icecream-live/measure-final.json`.

**Edit to live**, from saving the edit to the page that was already playing
applying it, median of three (range):

| Change | Edit to live | Build | Bake | Publish | Fetched by the page | Of a fresh page's first play | Same run |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Numbers: 20% slower, two more hiding spots | **191 ms** (188–274) | 1 ms (none) | 110 ms | 51 ms | 44.7 KB | 9% | yes |
| Code: seven sprinkles a volley | **1.43 s** (1.43–1.53) | 1.18 s | 108 ms | 119 ms | 141 KB | 28% | yes |
| Art: a mint forehead (one chunk) | **181 ms** | 1 ms (none) | 106 ms | 46 ms | 45.3 KB | 9% | yes |
| A broken build | 1.10 s (0.95–1.36) | 0.83 s | 116 ms | 132 ms | 141 KB | 28% | yes, paused |
| Rollback | **32 ms** (31–32) | none | none | 4 ms | 0 KB (cached) | 0% | yes, resumed |
| A Game Lab edit (the chat's `game_edit`) | 6.1 s (3.9–15.9) | none | 169 ms | 180 ms | 20 KB | 4% | yes |

- "Same run" is the page's own report: it applied each version at a step
  boundary and kept going in the same run. One bot run went through all 18
  changes without restarting: its tick counted on from 159 to 1678, and the
  keys it picked up stayed with it (none, then one, then two). Each broken
  build stopped the run at one tick (326, 716, 1464); the rollback resumed
  it from that same tick.
- The page fetched only objects new to it. The first time each version
  is seen it fetches the new manifest (about 19 KB, most of a numbers
  change), the zone index, and the changed objects: the tuning plus the two
  rooms that gained a closet, the 141 KB logic module (brotli), or the one
  45 KB character chunk. When a version republishes hashes it already
  had, which is what runs two and three did, it fetches only the manifest
  (19.3 KB); a rollback fetches nothing at all.
- A Game Lab edit's time is almost all Game Lab's own check, where the bot
  plays 24 seeded houses with the new numbers before it saves (3.8 to
  15.8 s under this load). Publishing and applying took about 0.3 s of it.
- The logic module builds in about a second with the `live-wasm` profile
  (release without LTO, incremental, 415 KB); under heavier load an earlier
  pass measured 2 to 7 s.

**First play.** A fresh browser profile opening `/t/icecream/` reached its
first frame, with the game running, in **61 ms**, moving **497 KB** over the
network in 14 files (1.8 MB uncompressed in 11 zone files; the budget is
3.4 MB and 1.5 s). That's the page, the 815 KB drawing module (brotli), the
415 KB logic module, the tuning, both characters, and 4 of 49 rooms. The
other 45 rooms (6.6 MB uncompressed in all) streamed in as the bot played.

**The Grid path** (`grid-check`, headless):

| Step | Measured |
| --- | --- |
| The Grid's first frame with the arch | 0.56 s |
| At the spawn, 27.6 m from the arch | not near, nothing fetched |
| In front of the arch: prefetch of the first play | 22 ms, 11 files, 1.9 MB decoded |
| Holding W to walk through | 0.55 s to cross |
| The zone's first play inside the Grid, after the prefetch | **72 ms**, **4 KB** over the network (the page; the rest from the cache) |
| **Back to the Grid** | back in front of the arch |

Without the prefetch, the same first play moves 497 KB.

## Limits

- **Local only.** One host, one relay, one browser; nothing was deployed to
  staging or production.
- **Players don't share a world.** Each player's run is local. The zone
  shares a house and its version, not a simulation; a shared one needs the
  authority and recovery protocol in [zone rules](zone-rules.md).
- **The browser only.** The logic module is a core WebAssembly module the
  page instantiates, not a component: browsers can't run components without
  a JavaScript translation. The desktop and phone apps don't follow the
  zone's pointer yet, and the native Grid has no hosted arch; its zones stay
  closed, host-supported code ([zones](zones.md#build-and-register-a-new-zone)).
- **The shell carries a copy of the rules' types.** It reads the state to
  draw it. A logic change that keeps `STATE_VERSION` swaps in place; one
  that changes the state's shape must raise it, and the page then refuses
  the new module until its shell is redeployed.
- **The Ice Cream Man has no texture or audio files**, so the asset chunks
  are meshes; a recolour (`palette.json`) stands in for a texture change.
- **A hidden window waits.** The page applies a version from its frame
  loop, and Chrome stops drawing a tab or window that is hidden behind
  others, so it fetches and applies the version when it is shown again.
- **Game Lab** checks an edit with 24 bot runs before saving it, which is
  most of a chat edit's time to live.
