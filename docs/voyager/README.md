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
[docs/minecraft/voyager-runbook.md](../minecraft/voyager-runbook.md) is
the full runbook, spectator mode included.

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
{"id": 6, "op": "mine", "args": {"positions": [[-24, 0, -1]], "names": ["iron_ore"], "count": 1}}
{"id": 7, "op": "wait", "args": {"seconds": 2}}
{"id": 8, "op": "disconnect"}
{"id": 9, "op": "shutdown"}
```

Answers:

```jsonc
{"id": 1, "ok": true, "result": {...}}
{"id": 6, "ok": true, "result": {"mined": 5, "attempted": 6, "dug": [[-24, 0, -1], ...]}}
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

`worlds/meadow.json` is the solo example: Minecraft version, seed,
difficulty, gamerules as console commands, and the episode bounds
(`max_actions`, `max_seconds`). A manifest's SHA-256 is the world's
identity, so a custom world or ruleset is a new file — `voyager worlds`
lists what a directory holds with digests.

A manifest can go further and enroll a roster. The optional sections:

- `agents` — the members of an ensemble episode: an offline username, a
  guild, the Nostr pubkey the member signs under, and an optional camp
  position. The loader refuses duplicate usernames.
- `deposits` — registered ore: an id, an optional owning guild (absent
  means contested), the block kind, the per-block award, and the exact
  block positions. A position may sit in only one deposit.
- `economy` — starting credits per guild and the largest hold one quest
  may place.
- `relay` — guild communication and, optionally, a decision door:
  `port` for the supervised `nostr-relay`, `decision_url` for a
  `POST /v1/systemone` endpoint, and `decision_model`.
- `effects` — named console commands a verified quest may run, such as
  `open_bridge`. A quest names an effect; it never supplies commands.
- `minecraft.generator_settings` — flat or custom world generation, as
  the JSON string `server.properties` reads.
- `minecraft.setup_commands` — console commands run once after the
  gamerules: world edits, `forceload`, `setworldspawn`. Gamerule names
  are the 1.21.11 snake_case forms (`advance_time`, `spawn_mobs`), and
  edits outside the spawn chunks need `forceload` first.

## The arena

`worlds/arena.json` enrolls four members in two guilds — `ferro_1`,
`ferro_2`, `lumen_1`, `lumen_2` — on a flat world: a wool camp per
guild, a private iron deposit per guild, a contested diamond deposit,
an emerald deposit across a trench, forges, and a quest board.
`voyager run --world arena` picks the ensemble runner instead of the
solo curriculum: one `mc-bridge` child per member, all in one world.

Every member joins under its enrolled username, walks to camp, digs its
guild's deposit, and then one member per guild swings at each contested
deposit. The award path is the attribution claim: the host only sends
manifest-registered positions, the bridge refuses a position whose block
is no longer the declared kind, only positions the helper reports `dug`
are proposed to the ledger, and the ledger dedupes `(deposit, pos)` — a
contested block pays once. Gifts and replays never enter the path.

The ledger is `ledger.jsonl` in the run directory: append-only events
(`award`, `reserve`, `settle`, `release`), replayed into per-guild
balances of available, reserved, and spent credits. A reservation that
was never settled stays reserved across a crash — unknown work keeps
its hold rather than freeing capacity it may still consume.

Ore dug without a pickaxe drops nothing, so arena agents earn credits
rather than items — the ledger, not the inventory, is the award.

## Guild channels

A world with a `relay` section runs the episode's Nostr relay too: a
supervised `nostr-relay` child against a local Postgres database
(`VOYAGER_RELAY_DATABASE_URL`, default `postgres://127.0.0.1:5432/
voyager_relay`; the binary is `VOYAGER_RELAY_BIN` or the workspace's
debug build). Before the first agent joins, the runner creates one
closed NIP-29 group per guild through the NIP-86 management endpoint
and enrolls each member's manifest pubkey.

Each member then holds a websocket as its own derived key, answers the
relay's NIP-42 challenge, and speaks in its guild with C7 `kind:9`
chat events carrying the `h` tag. The episode demonstrates the
boundary rather than asserting it: a member's write to another guild's
channel is refused `restricted:` and recorded as a passing task, and
an unaffiliated observer reads both guilds' histories afterward —
public read, restricted write. The relay's own key and the management
key are derived like the agents' (`voyager-relay-key:` roles); this is
a demo relay whose groups live for one episode, so the runner deletes
any stale group before creating its own.

## The decision door

A `relay.decision_url` adds one `POST /v1/systemone` call per guild:
when a member faces more than one contested deposit, a typed `choice`
question asks which to work first, and the answer orders the mining
pass. The model picks among admitted work only — the deposit set, the
positions, and the awards all stay manifest-bounded.

Every call writes `decisions/decision-N.json` under the run directory:
the exact state, the typed questions, the model identity, the raw
response body, and the transport (`local-http`; the NIP-CJ relay
family carries the same request body once a decision worker is
deployed). NIP-CJ normalizes `confidence` to the picked option's
probability, but the SDK accepts any in-range value, so the runner
recomputes confidence from the returned distribution rather than
trusting the field.

A local door is `kev-serve` — for example
`./target/debug/kev-serve --adapter-dir ~/work/kev-artifacts/kev-0.6b
--base-dir ~/work/kev-artifacts/qwen3-0.6b --port 8009`, which serves
the model as `kev-latest`. Inference on CPU runs tens of seconds, so
the door call carries a 120-second timeout and a patient retry policy:
a `busy` answer means a forward is computing, not that the door is
down.

## Agent keys

An enrolled member signs as the username it joined under. The secret is
`sha256("voyager-agent-key:" + username)`, re-derived at run time, so no
private key sits in a manifest, a run directory, or this repository.
`voyager keys <username>...` prints the pubkeys a manifest binds:

```sh
cargo run -p voyager --bin voyager -- keys ferro_1 ferro_2
```

Deterministic derivation is the enrollment: two runs name the same
identities, and a trace can verify a signature without a keystore. This
is the arena's key story, not a production one — outside operators need
real key custody, a later NIP-CAP question.

## What phase 1 does not do

- **The model orders work; it does not choose it.** Where a decision
  door answers, a `choice` question picks among already-admitted
  deposits; the task list, the action vocabulary, and the mechanical
  critic stay the host's. `coder::generate`-driven task proposals come
  with the curriculum phase.
- **No skill library.** `seen` block names are remembered inside an
  episode; nothing is banked between episodes yet.
- **No Gym suite.** The trace is the evidence; the suites that score it
  come with the measurement phase.

The proposal for the rest lives in
[issue #9528](https://github.com/OpenAgentsInc/openagents/issues/9528).
