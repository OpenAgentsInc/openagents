# Voyager

For cross-project priorities and dependencies, see the [master roadmap](../roadmap.md).

An open-ended agent that lives in a Minecraft world, after the Voyager
paper (arXiv:2305.16291): a curriculum proposes tasks, programs run as
bounded code-as-action, a critic checks what each attempt did, and what
passes banks into a persistent skill library.

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
as an offline player, and runs the curriculum loop: for each task —
survey the spawn area, explore north, gather wood — resolve a program,
run it through the bounded interpreter, check it against the critic, and
bank what passes. The bot narrates in game chat as it goes — `say` is a
first-class action, so watching the world means watching the agent work.

A run leaves everything in `~/.openagents/voyager/runs/<stamp>-<world>/`:

- `server/` — the world data, still loadable by any client on the port
- `server.log` — the server's own log
- `trace.jsonl` — the ATIF trace: every bridge exchange as a `Call`,
  every event the bot reported as a step
- `decisions/` — the decision door's recorded exchanges, when a
  `curriculum.decisions` section points at one
- `program-N.json` — the `act` door's requests and answers, when one is
  wired

`voyager evidence <run-dir>` renders a finished run into
`coverage.json` (the demo's protocol matrix), `metrics.json` (decision
and bridge-call latency, ledger sums), and `evidence.md` (the readable
causal chain).

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
programs are proposed text the interpreter admits into this vocabulary,
never text that runs itself.

## Programs and the interpreter

A task's program is Lua — vendored Lua 5.4 through `mlua` — run inside a
bounded engine: the paper's code-as-action claim with the host owning the
vocabulary. Scripts call the same ops the bridge speaks, under
script-facing names:
`say`, `walk` (the bridge's `goto`; `goto` is a Lua keyword),
`explore`, `mine`, `mine_at`, `players`, `state`, `block_at`, `wait`,
and `feedback`, which drains the bot's narration. Every engine bound is
set: operations, call depth, string and data size, and wall seconds, and
every host call is one traced, bound-checked bridge exchange.

A program comes from one of four places, in order:

1. `skill` — a banked skill by `name` or `name@version`, looked up in
   `VOYAGER_SKILL_DIR`, the repository's `skills/`, then
   `~/.openagents/voyager/skills`.
2. `script` — inline source in the task itself.
3. Retrieval — the decision door answers one `choice` question over the
   banked skills' descriptions, with `none` always an option.
4. The `act` door — a `curriculum.act` section names an Open Responses
   door (`POST {url}/v1/responses`) that writes the program from the
   goal and the state, and rewrites it when the interpreter faults. A
   task gets at most four attempts — the paper's self-correction loop —
   and each write lands beside the decisions as `program-N.json`.

A task that can get none of those fails as unwritten rather than
improvising.

## The critic and the skill library

A task's `verify` spec decides success: `moved` and `inventory` read
before/after deltas, `block_at` reads a named position, `ran` records
completion honestly, and `noul` asks the decision door over the
before/after state where no mechanical rule covers the goal. The verdict
and its evidence land in the trace either way.

A passing task marked `bank` writes its program into the skill store: a
digested, versioned record under `~/.openagents/voyager/skills/`
(`VOYAGER_SKILL_DIR` overrides). Rebanking a name versions up; a record
whose bytes moved fails its digest. A skill that proves itself can
promote into the repository's `skills/` directory the way a question set
lives in `questions/` — files before events, digested as a whole.

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
- `curriculum` — the solo episode's task source: `tasks` declares the
  list verbatim (each with `id`, `goal`, an optional `script` or
  `skill`, a `verify` spec, and `bank`), `generate` names the Open
  Responses door that proposes what comes next with a warm-up schedule,
  `act` names the door that writes programs, `decisions` names the
  `POST /v1/systemone` door for retrieval and `noul` verdicts, and
  `max_tasks` bounds the episode's task count. A world with no
  `curriculum` section runs the built-in starter tasks.
- `scenario` — `quest` (the default) runs the economy and coding-quest
  chain with no combat; `war` adds the skirmish the `combat` section
  declares. `--scenario` on the command line overrides the manifest.

## The arena

`worlds/arena.json` enrolls four members in two guilds — `ferro_1`,
`ferro_2`, `lumen_1`, `lumen_2` — on a flat world: a wool camp per
guild, a private iron deposit per guild, a contested diamond deposit,
an emerald deposit across a trench, forges, and a quest board.
`voyager run --world arena` picks the ensemble runner instead of the
solo curriculum: one `mc-bridge` child per member, all in one world.

Two scenarios share the arena. The default `quest` scenario — the coder
scenario — runs the mining economy, the guild channels, the decision
door, and the coding quest; the `combat` section is inert. The `war`
scenario keeps everything `quest` runs and adds the skirmish: enemy
scans between work stretches, model-called engagements, rallies, and
the round-robin patrol the combat section declares. The manifest can
pick either, and `--scenario quest|war` on the command line overrides
the manifest.

Every member joins under its enrolled username, walks to camp, digs its
guild's deposit, and then one member per guild swings at each contested
deposit. The award path is the attribution claim: the host only sends
manifest-registered positions, the bridge refuses a position whose block
is no longer the declared kind, only positions the helper reports `dug`
are proposed to the ledger, and the ledger dedupes `(deposit, pos)` — a
contested block pays once. Gifts and replays never enter the path.

The ledger is `ledger.jsonl` in the run directory: append-only events
(`award`, `reserve`, `settle`, `release`, `xp`), replayed into
per-guild balances of available, reserved, and spent credits plus a
separate per-agent XP fold. A reservation that was never settled stays
reserved across a crash — unknown work keeps its hold rather than
freeing capacity it may still consume.

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

## The coding quest

A world with a `quest` section runs the arena's payoff: bounded compute
buys a verified patch, and the patch buys a world change. The chain
keeps execution, verification, and integration in separate records
under `quest/` in the run directory:

1. **Reserve.** The guild's ledger holds `quest.cost` before anything
   runs. If the guild cannot hold it, the quest reports that instead of
   running.
2. **Execute.** The fixture — a bounded Rust crate under
   `quests/<id>/fixture/` with public tests — is copied into the run
   directory and committed as the base. The solver writes inside the
   copy, bounded `cargo test` answers each attempt, and `git diff`
   seals the result as `patch.diff`. The shipped solver is
   `voyager-quest-builtin/1`, a deterministic planner honestly
   attributed in `execution.json`; the stage is a seam a model-driven
   solver plugs into unchanged. Attempts share the budget: the hold
   settles `cost / attempts` per attempt consumed and returns the rest.
3. **Verify.** The referee applies `patch.diff` to a fresh copy of the
   same base plus the `protected/` cases the solver never saw — a patch
   that only guesses the public case does not pass. The artifact, not
   the working tree, is what gets verified.
4. **Integrate.** Only an accepted artifact runs the manifest-named
   effect from `effects` — the quest names it; it never supplies a
   command. Another guild's member then reads the `verify_blocks` back
   through `block_at` until the world update reaches it. A reconciled
   effect records `quest.xp` in the ledger's separate XP fold and the
   host's relay-management key publishes a NIP-32 `kind:1985` label —
   `openagents.voyager` / `quest-complete` targeting the member's
   pubkey — whose event and verdict land in `quest/label.json`.

`mc-bridge` gained `block_at` for the reconciliation: `{position}` in,
the block kind and loaded status out, no digging.

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

## What is still not done

- **The quest solver is deterministic.** `voyager-quest-builtin/1`
  proves the reserve-execute-verify-integrate chain end to end, but it
  knows the fixture's answer; a `coder`-door solver is the recorded
  upgrade, and execution records name whichever solver ran.
- **Decisions stay local.** `local-http` is the recorded transport; a
  NIP-CJ decision worker would carry the same request bodies over the
  relay, and the coverage matrix marks the CJ rows `absent` until one
  runs.
- **No embeddings.** Retrieval is a `choice` over the banked
  descriptions; the paper's ada-002 index is a measured upgrade, and
  the gym suite's top-5 retrieval accuracy is where it would show.
- **No remote worlds.** Servers are local and supervised; a remote
  world is a different admission story, not a flag.

The issue this closes proposed the shape in
[#9528](https://github.com/OpenAgentsInc/openagents/issues/9528); the
guild demo's evidence bar is
[#9529](https://github.com/OpenAgentsInc/openagents/issues/9529).
