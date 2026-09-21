# Watch a Voyager episode live

A runbook for seeing the `crates/voyager` agent work in a real Minecraft
world: join the server it boots, spectate the bot, and read the run's
trace afterward. Set up once, then every episode is watchable.

The other documents in this directory specify a proposed application.
This page describes the shipped slice; [docs/voyager](../voyager/README.md)
covers the crate, the helper, and the bridge protocol.

## Prerequisites

One-time setup from the repository root:

```sh
./scripts/build-mc-bridge.sh          # nightly helper, azalea bot
./scripts/fetch-mc-server.sh 1.21.11  # vanilla server jar, sha1-verified
cargo build -p voyager
```

You also need:

- **Java.** `voyager paths` shows what the defaults resolve. If nothing
  resolves, install a JDK (`brew install openjdk` works) or point
  `VOYAGER_JAVA` at a `java` binary. A path that exists but cannot run —
  like the `/usr/bin/java` stub on macOS without a JDK — is refused.
- **A Minecraft 1.21.11 client**, only if you want to watch in the world.
  Everything else works without one.

## Run an episode

```sh
cargo run -p voyager -- run --world meadow
```

The runner boots a fresh server for `worlds/meadow.json`, waits for the
vanilla `Done` line, joins the bot as an offline player named `voyager`,
and works a fixed curriculum: survey the spawn area, explore, gather
wood, report. Episodes are bounded by the manifest (`max_actions`,
`max_seconds`); this one finishes in a few minutes.

The terminal narrates as it goes — `bot says:` lines are the agent's own
chat, `bot:` lines are helper feedback, `chat:` lines are what the
server saw.

`cargo run -p voyager -- run --world arena` runs the multi-agent episode
instead: four enrolled bots in two guilds on a flat world, mining
registered deposits for ledger credits. The run directory gains a
`ledger.jsonl` alongside the trace, and the spectator advice below
works the same — there is just more to watch.

## Watch in the world

Start the episode, then in the Minecraft client:

1. Multiplayer → Add Server → address `127.0.0.1` (port 25565).
2. Join. The server runs `online-mode=false`, so your account joins as an
   unverified player — fine for watching.
3. You spawn at world spawn, near where the bot joined. It announces
   what it is doing and where in chat; walk over and watch it work.

Join early: the survey and first explore leg keep the bot near spawn for
the first minute or two.

### Spectate instead

Joining puts you in survival — the server forces the manifest's
gamemode. To fly alongside the bot, op yourself through the manifest's
console channel. Edit `worlds/meadow.json`:

```json
"setup_commands": ["time set day", "weather clear", "op YourName"]
```

`setup_commands` run as console commands once the server reports ready,
before the bot joins. In offline mode a name resolves to its
name-derived UUID, so `op` works before you have ever joined. Then join
the episode and run `/gamemode spectator` — free flight, no collision,
you can follow the bot into a ravine.

If `op` ever fails because the server cannot resolve the name yet, join
once in survival so your player record exists, then run again.

## Read the trace

Every run leaves a directory under `~/.openagents/voyager/runs/`:

- `server/` — the world data and `server.properties`
- `server.log` — the server's own log
- `trace.jsonl` — the ATIF trace: every bridge exchange as a `Call`,
  every event the bot reported as a step

Watch a run live without a client:

```sh
tail -f ~/.openagents/voyager/runs/<stamp>-meadow/trace.jsonl
```

## Inspect the aftermath

The world persists after the episode. To walk it yourself, copy
`server/world/` into your client's `saves/` directory
(`~/Library/Application Support/minecraft/saves/` on macOS) and open it
in singleplayer — the mined trunks and the bot's path are still there.

## Troubleshooting

- **`voyager paths` shows no Java** — install a JDK or set `VOYAGER_JAVA`.
- **`run` fails naming a missing prerequisite** — build the helper or
  fetch the jar it names; the runner refuses rather than substituting.
- **Port 25565 in use** — another server or a previous run still holds
  it. Stale runs stop on their own; `voyager` also stops its server when
  the episode ends.
- **The bot joined but the episode failed a task** — read the task list
  it prints. A failed task is an honest report, not a crash: the trace
  shows which check fell short.
- **Nothing in chat** — the bot trims messages to the protocol limit;
  the full narration is in the terminal output and the trace.
