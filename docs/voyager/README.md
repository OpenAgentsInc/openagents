# Voyager

An open-ended agent that lives in a Minecraft world, after the Voyager
paper (arXiv:2305.16291). Phase 1 is the vertical slice: a supervised
local server, a bot that can join it, and an episode that surveys,
explores, and gathers — everything bounded, everything traced.

## What runs where

| Piece | Where | Built by |
| --- | --- | --- |
| `voyager` | `crates/voyager`, stable Rust, in the workspace | `cargo build -p voyager` |
| `mc-bridge` | `mc-bridge`, nightly Rust, its own workspace | `./scripts/build-mc-bridge.sh` |
| Server jar | `~/.openagents/voyager/minecraft/versions/<v>/server.jar` | `./scripts/fetch-mc-server.sh <v>` |
| Worlds | `worlds/*.json` manifests | you |

The bot framework is [azalea](https://github.com/azalea-rs/azalea), which
needs nightly Rust (`simdnbt` uses `portable_simd`) — a toolchain the
workspace's pinned stable cannot provide. The split is the
`swift/lev-bridge` precedent exactly: the product crate orchestrates on
stable, the helper is a supervised child process, and the two speak one
JSON object per line on stdin and stdout.

## Set up

```sh
./scripts/build-mc-bridge.sh          # once, on the pinned nightly
./scripts/fetch-mc-server.sh 1.21.11  # once per Minecraft version
cargo build -p voyager
```

Java must be reachable — `java` on `PATH`, `VOYAGER_JAVA`, or a Homebrew
openjdk install. `voyager paths` shows what the defaults resolve to on
your machine.

## Run a world

```sh
cargo run -p voyager --bin voyager -- run --world meadow
```

The episode boots a local server for `worlds/meadow.json`, joins the bot
as an offline player, and walks its curriculum: survey the spawn area,
explore north, survey again, gather wood, and report. The bot narrates
in game chat as it goes — `say` is a first-class action, so watching the
world means watching the agent work.

A run leaves everything in `~/.openagents/voyager/runs/<stamp>-<world>/`:

- `server/` — the world data, still loadable by any client on the port
- `server.log` — the server's own log
- `trace.jsonl` — the ATIF trace: every bridge exchange as a `Call`,
  every event the bot reported as a step

To watch live, join the server with any Minecraft client at
`127.0.0.1:25565` while the episode runs, or read the trace.

## The bridge protocol

`mc-bridge` reads one request object per line on stdin and writes one
response or event object per line on stdout:

```jsonc
{"id": 1, "op": "join", "args": {"address": "127.0.0.1:25565", "username": "voyager"}}
{"id": 2, "op": "state", "args": {"radius": 16}}
{"id": 3, "op": "say", "args": {"text": "hello"}}
{"id": 4, "op": "goto", "args": {"x": 12, "z": -40, "seconds": 60}}
{"id": 5, "op": "explore", "args": {"direction": "north", "distance": 96, "seconds": 30}}
{"id": 6, "op": "mine", "args": {"names": ["oak_log"], "count": 2, "radius": 32, "seconds": 120}}
{"id": 7, "op": "wait", "args": {"seconds": 2}}
{"id": 8, "op": "disconnect"}
{"id": 9, "op": "shutdown"}
```

Answers:

```jsonc
{"id": 1, "ok": true, "result": {...}}
{"id": 6, "ok": false, "code": "no_blocks", "error": "..."}
{"event": "chat", "text": "<voyager> hello"}
{"event": "feedback", "text": "mining oak_log at (12, 64, -40)"}
{"event": "spawn" | "death" | "disconnect" | "server_exit", ...}
```

The action vocabulary is deliberately typed and bounded. The paper's
agents emit executable code; the host here decides what may run, which is
the seam the workspace's execution model already asks for. Generated
actions — when they come — are proposed text the host admits into this
vocabulary, never text that runs itself.

## World manifests

`worlds/meadow.json` is the example: Minecraft version, seed,
difficulty, gamerules as console commands, and the episode bounds
(`max_actions`, `max_seconds`). A manifest's SHA-256 is the world's
identity, so a custom world or ruleset is a new file — `voyager worlds`
lists what a directory holds with digests.

## What phase 1 does not do

- **No model in the loop.** The curriculum is a fixed program and the
  critic is mechanical: exploration must move the bot, gathering must
  change the inventory. The task list, the action vocabulary, and the
  checks are where `coder::generate` and `POST /v1/systemone` plug in
  later.
- **No skill library.** `seen` block names are remembered inside an
  episode; nothing is banked between episodes yet.
- **No Gym suite.** The trace is the evidence; the suites that score it
  come with the measurement phase.

The proposal for the rest lives in
[issue #9528](https://github.com/OpenAgentsInc/openagents/issues/9528).
