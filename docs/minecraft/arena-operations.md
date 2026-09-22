# Operate the arena episode

An operations runbook for the multi-agent `arena` world: builds,
services, launch discipline, the manifest, debugging, and the traps
that cost time the first time through. Read this before running
`voyager run --world arena`.

[`voyager-runbook.md`](voyager-runbook.md) covers the single-agent
`meadow` episode and basic spectating. This page is the eight-agent,
two-guild combat demo.

## Components

| Piece | What it is | Build |
|---|---|---|
| `voyager` | The runner: boots the server, spawns agents, keeps the ledger, records the trace | `cargo build -p voyager` (workspace toolchain) |
| `mc-bridge` | Nightly Rust helper, one process per agent; speaks JSON lines to voyager and the Minecraft protocol to the server | `./scripts/build-mc-bridge.sh` → `target-mc/release/mc-bridge` |
| `nostr-relay` | Local relay for the guild channels; the episode spawns it | workspace build; needs Postgres running |
| Minecraft server | Vanilla 1.21.11 jar, spawned fresh per episode into the run directory | `./scripts/fetch-mc-server.sh 1.21.11` |

`mc-bridge` needs nightly because azalea uses `portable_simd`. Its
build script keeps a separate target directory (`target-mc/`) so the
nightly build does not contend with the workspace's.

## Services to have running

- **Postgres** — the local relay stores groups and events there.
- **A decision model**, optional but recommended. Two choices:

  | Door | Configure | Cost |
  |---|---|---|
  | Live TypeSafe API (`jev-latest`) | `decision_url: "https://api.typesafe.ai"` in the manifest's `relay` section, and `TYPESAFE_API_KEY` in the environment (this machine keeps it in `~/work/.secrets/typesafe.env`) | ~1s per question |
  | Local `kev-serve` | `decision_url: "http://127.0.0.1:8009"` | ~60s per question on a loaded machine — CPU-only inference; usable, but the queue drains slowly |

  The runner picks `Door::local` for loopback URLs and `Door::live`
  otherwise. No `decision_url` means no door: agents run entirely on
  temperament and aggression.

## Launch discipline

Two rules, both learned the hard way:

1. **Run the binary from the worktree it was built in.** `mc-bridge`
   resolution is baked in at compile time — the binary looks for
   `<its-own-worktree>/target-mc/release/mc-bridge`. A binary built in
   `openagents/` cannot see `openagents-voyager/target-mc/`, and the
   episode fails with `no mc-bridge binary`. Prefer launching the
   binary by absolute path:

   ```sh
   /Users/christopherdavid/work/openagents-voyager/target/debug/voyager run --world arena
   ```

2. **`cd` inside a backgrounded `&&` chain does not stick.** A command
   like `cd dir && nohup prog &` applies `dir` only inside that first
   background job; a second `&` in the same line runs at the shell's
   persistent working directory. Wrap the whole launch in a subshell:

   ```sh
   (cd /path/to/worktree && nohup ./target/debug/voyager run --world arena > /tmp/episode.log 2>&1) &
   ```

   If a run resolves paths against the wrong worktree, this is why.

Before launching, clear the field — stale processes hold port 25565
and double-join bots:

```sh
pkill -f "voyager run"; pkill -f mc-bridge; pkill -f server.jar
```

## The manifest: `worlds/arena.json`

One file shapes the whole episode:

- **`minecraft`** — version, world type, `gamerules`, and
  `setup_commands` (console commands run once the server is ready).
  Arena specifics:
  - `difficulty: normal` and `natural_health_regeneration: false` —
    peaceful regen makes PvP unable to kill.
  - `keep_inventory: true` — bots keep their kits through deaths.
  - `op Smorlox` in `setup_commands` — ops a human at boot, before
    they ever join (offline UUIDs resolve by name).
  - `"sleep 3"` after `forceload add` — forceload queues chunk
    generation asynchronously; `fill` commands issued in the same
    tick race it and silently miss. `sleep` is a voyager-side
    directive that pauses the setup stream.
- **`agents`** — enrolled members: `username`, `pubkey`, `guild`,
  `camp`, optional `temperament` (`berserker` | `hunter` | `worker` |
  `skittish`) and `aggression` (0–1). Unset fields derive
  deterministically from the username, so a member keeps its
  personality across episodes. Aggression scales the effective aggro
  radius and how often the member re-asks the model.
- **`admins`** — usernames switched to creative automatically on
  join, after `force-gamemode` has applied. `Smorlox` is listed.
- **`deposits`** — mining targets: block positions, kind, award per
  block, optional `guild` (absent means contested — first member of
  each guild works them).
- **`combat`** — `aggro_radius`, `rounds`, `ground`, `kill_xp`.
- **`relay`** — `port`, plus `decision_url`/`decision_model` when a
  model is wired.
- **`episode`** — `max_actions`, `max_seconds`. With eight agents,
  combat burns actions roughly eight times faster than the original
  four-agent budget assumed; 480 covers a full episode.

## What an episode does

1. Boots a fresh server in the run directory, waits for `Done`, runs
   `setup_commands`.
2. Starts the local relay; deletes any stale guild groups, creates
   `ferro` and `lumen` channels, enrolls every member's pubkey.
3. Builds the decision door if `decision_url` is set, and moves it
   onto its own thread — legs post asks over a channel and collect
   answers later, so model latency never idles a body.
4. Spawns one `mc-bridge` per agent on its own thread. Each agent
   gets a kit once per episode (diamond pickaxe, diamond sword, bow,
   64 arrows) via the join-event hook; admins get creative the same
   way.
5. Every agent runs the roaming loop: dig deposits a few blocks at a
   time, scan for enemies inside the aggro radius between stretches,
   and post sightings to guild intel as rally calls. The model
   weighs engage/retreat/keep-working, target, weapon, contested
   order, and whether to answer a rally — sampled from its
   probabilities, announced in chat under the model's name.
6. When work runs out, agents patrol an arena-wide waypoint ring —
   seeded per member so the eight spread out — until the episode's
   bound ends the leg.
7. Closing: guild balances spoken in chat, ledger and trace sealed,
   bridges and the server shut down.

## Watching and recording

Join the server from a 1.21.11 client: Multiplayer → Direct Connect →
`127.0.0.1`. Admins land in creative automatically; `force-gamemode`
resets to survival only at join.

Screen-record the world for evidence:

```sh
ffmpeg -y -f avfoundation -framerate 30 -capture_cursor 1 -i "4:none" \
  -pix_fmt yuv420p -c:v libx264 -preset veryfast \
  ~/Desktop/voyager-arena-$(date -u +%Y%m%dT%H%M%SZ).mov
```

## Debugging

Each run writes `~/.openagents/voyager/runs/<stamp>-<world>-<hash>/`:

- `trace.jsonl` — the ATIF trace: every bridge exchange as a `Call`
  with `at` (stamp at call **end**) and `milliseconds`, every event
  and note as steps.
- `decisions/decision-N.json` — each model exchange verbatim:
  request state, options, answers, probabilities.
- `ledger.jsonl` — credits, holds, XP.
- `server/logs/latest.log` — the Minecraft server's own log.
- `relay.log` — the local relay's log.

**Measuring idle.** Attribute each call by `purpose` (`"<user>: …"`),
then compute between-call time as `next.at - (prev.at + prev.ms) -
next.ms` — `at` is stamped at call end, so subtract the next call's
own duration too, or you overcount. A large **tail gap** (last call
ended long ago, nothing since) means a leg exited early; a series of
mid-stream gaps means blocking work between calls.

## Traps, and their signatures

- **`unknown op "players"` / `unknown op "attack"`** — the episode
  spawned a stale `mc-bridge`. Rebuild via `build-mc-bridge.sh` and
  confirm which `target-mc/` the launched binary searches.
- **Bots idle after work finishes** — patrol `goto`s returning
  `"from" == "to"` in 0 ms: the waypoint is inside the goal radius,
  so the call completes instantly and the loop burns out. Patrol
  radius is 1.2 and waypoints must span the map.
- **Everyone frozen during model calls** — the synchronous-door
  failure mode: a leg that waits on inference also stops draining
  its bridge's event stream, so join events, kits, and admin
  commands queue up behind it. The door runs on its own thread now;
  nothing in the work path may wait on the model. Keep it that way.
- **Ore missing at dig time, `holds minecraft:air`** — either a
  player broke it first (the registry correctly refuses to credit
  air) or a `fill` raced `forceload` (use `sleep` in
  `setup_commands`). The diagnostic names what the position holds.
- **Kit or creative arrives ~45 s late** — join events drain while
  the agent's current bridge call runs. Instant-kit comes from the
  event hook; if it is late, something upstream is blocking.
- **`chat:` lines repeat** — every enrolled client sees the same
  world message, and the mirror prints each view. Cosmetic.
- **One agent "rejoining" repeatedly** — the server replays recent
  messages on reconnect; check `server/logs/latest.log` for actual
  logins before assuming a disconnect loop.
- **A leg ends silently** — a `?` on `channel_chat` used to kill
  legs on relay hiccups; guild posts in the work loop are
  best-effort now. Keep fatal `?` only where a broken binding truly
  must stop the episode.

## Verify before pushing

`./scripts/verify-rust.sh` under the pinned toolchain; read
`docs/verification.md` for scope. Use a separate Cargo target
directory per worktree, and keep `mc-bridge/target/` out of commits
(it is `.gitignore`d).
