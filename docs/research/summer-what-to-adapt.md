# Summer Engine: what to adapt

Status: analysis, 2026-11-13. This compares [summer.md](summer.md) with the
Metaverse Engine as it is on `main` today. It also covers the browser editor
spec ([docs/editor/2026-11-12-browser-editor.md](../editor/2026-11-12-browser-editor.md),
E1-E14 #13410-#13423), the Spawn analysis
([spawn-what-to-adapt.md](spawn-what-to-adapt.md), #13429-#13434), the AAA
program (#13257) and DDD (#13311).

## The short version

Summer is a fork of Godot 4 with an agent control layer on top. Spawn told us
what the engine should *show* an agent (typed diagnostics, the version-matched
reference). Summer shows how to make an engine safe to *operate*. Every action
is a named operation with an effect class, an approval rule and a result that
says what happened. That includes "we don't know whether it ran". Mutations
are bound to one project and one revision. Tests replay operations into a
fresh world and check the live state, the saved file and the behaviour
separately.

We already have more of this than Summer's public code does:

- **One command table.** `REALM_COMMANDS` (37 rows in
  `apps/openagents-cli/commands.coax`) drives both the CLI and the `realm_*`
  MCP tools (`apps/openagents-cli/mcp.coax`).
- **Revision checks on edits.** An edit carries the `rev` it read, and a stale
  edit fails with a conflict.
- **Pictures.** `realm_shot` returns an image.
- **Visiting.** `realm_play`, `realm_look` and `realm_go` sit an agent in a
  realm.

What we lack is the **metadata on each operation** (effect, approval, dry-run
strength), **an "outcome unknown" result**, and **checks that test the saved
world separately from the live one**. Those are cheap to add now, and they
decide how E6 should be built.

Summer's multiplayer movement is client-run, with the server checking the
poses it is sent. Don't adopt it, for the same reason as Spawn's.

## A decision this forces on the editor plan

E6 (#13415) plans "`openagents world` CLI, `world_*` MCP tools". We already
ship `openagents realm` and `realm_*` from one table, so a second `world_*`
family would be exactly the drift Summer's own notes describe: legacy
handlers with hand-written schemas and mirrored CLI dispatch.

**E6 extends `REALM_COMMANDS` instead of adding a second family.** Edit verbs
become rows of the same table. Their MCP tools, CLI commands and `coax drive`
verbs are generated from those rows. The World document from E3 is what the
`realm_*` verbs edit. I've commented this on #13415. S1 to S6 are issues #13435-#13440 on project 22.

## Where we stand on each Summer idea

| Summer idea | Ours today | Verdict |
|---|---|---|
| A named operation vocabulary shared by MCP and the CLI | `REALM_COMMANDS`: one row per verb gives the CLI command and the `realm_*` tool, with bounded arguments checked by tests | **Have it.** Keep one table; E6 adds rows, not a family |
| Operation metadata: effect (read, write, spend, publish, player-facing, destructive), approval, examples | Rows have method, route, arguments and help. Deletion is two-step (`realm_ask_delete`), but no row says what kind of effect it has | **Adapt now** (S1) |
| Search, then call: discover operations instead of loading every schema | 37 realm tools load at once; fine today, but E6-E12 will add many more | **Adapt soon** (S4). It must follow the workspace rule on tool selection: a typed selector or embedding search, not keyword matching (`AGENTS.md`, Semantic Routing). So we adopt the shape, not Summer's lexical ranker |
| Results that say completed, running, needs approval, dry run, failed with retry advice; idempotency keys; outcome unknown | `realm_apply` returns the new rev or a conflict. Errors are words. No idempotency key on writes, and no "may have run" state | **Adapt now** (S2), together with N1's typed diagnostic codes (#13429) |
| Dry runs whose strength is stated | `apply` in E6 plans to return a frame and the refusal | **Adapt now** inside S2: every dry run says which of four levels it reached: schema checked, checked against the current world, the change computed, or run in a throwaway copy |
| Mutations bound to a project identity; a stale edit refused | Edits carry `rev` and conflict when stale. Drive commands name a page, not a realm and revision | **Have it for edits.** E3's fenced edits and `coax drive` should carry the same identity (S3) |
| Observation: scene, scripts, runtime input, screenshots, probes | `coax drive tree/state/screenshot/logs/perf`, `realm_shot`, `realm_look`, plus N1 `why` and N5 `hit` coming | **Have it**, with the Spawn items finishing it |
| Screenshot receipts that record synthetic camera and project mismatch | `realm_shot` returns a PNG and nothing about how it was taken | **Adapt now**, small: a shot returns its realm, rev, build, camera kind and whether the GPU was real or headless (part of S3). Our own rule that headless passes aren't proof needs this |
| An outcome harness: replay operations into a fresh world, check live state, the saved file, runtime errors and probes, and prove broken variants fail | Goldens, mutation testing on rules, `#test` blocks, Splice's 441 checks. No harness replays a recorded agent run against both the live World and the saved document | **Adapt now** (S5), on top of E3. This is the best idea in the report for us |
| Three kinds of context: project memory, skills, project index; stale edits refused after a read | Coder has memory and skills; deterministic git-mined indexes first (#11210); rev conflicts on realms | **Have it** |
| A client pack and a server pack, with authority-only code kept out of what players download | The page and the rooms are built separately (wasm vs native rooms), so room rules never ship in the page | **Have it by construction.** Add a check that no room-only module enters the page build (S6, small) |
| Persistent worlds: revision-based saves, a 16 MiB limit, periodic saves because a crash can't save | Character record and ledger (P4.1, P4.2); checkpoint age is a P5.3 service level (#13297) | **Have it**, more strictly (a ledger, not save blobs) |
| A running world keeps the version it started with when a new version publishes | Pins per published realm (migration 0107); one-release overlap (#13408); dev and live revisions planned in E3 | **Same policy.** Summer shows the cost: many save versions at once. Our record and ledger have one schema with migrations, so keep it that way |
| Asset jobs: estimate, submit with an idempotency key, poll or webhook, `done` only when stored, record the concrete model | E11 imports dropped files; generation goes through our providers (Google first) | **Adapt with E11 and any generation verbs:** estimate first, idempotency key, `done` only when the pack is written, record the model. S2's result shape covers it |
| Runtime AI for players (Router), refusing in the editor so games need a fallback | NPC and boss decisions go through the Decisions API (Clef first) | **Have it.** Copy one rule: every in-game model call has a written fallback, and the fallback is what tests run |
| Multiplayer: client-run movement at 20 Hz, the server checks the poses | Server-authoritative rooms, rows, one prediction module, unreliable state channel, room-loss matrix (P2.3, P2.4, #13164) | **Don't adopt.** Same reason as Spawn |
| Godot project files, GDScript, a native fork | Coax, the World, content tables | **Don't adopt** |
| Eval families kept apart: discovery, execution, runtime, persistence, visuals, planning, honest reporting | Checks exist for most, but not grouped this way | **Adapt soon** as the shape of E14's acceptance report (#13423) |

## Adapt now

- **S1 (#13435). Effect and approval on every command row.**
  - Each `REALM_COMMANDS` row gains an effect kind: read, write, publish,
    player-facing, spend or destructive. It also gains an approval rule:
    none, the owner, or a two-step confirm like today's delete.
  - The MCP tool's description and the CLI's help come from the row.
  - A test asserts that every non-read row has a rule, and that destructive
    rows are two-step.
  - This is a small change to one table, and every verb E6-E12 adds gets it
    for free.
- **S2 (#13436). One result shape for every operation.**
  - The states are: done (with the new rev), running (with a job id),
    needs approval, dry run (with its level), refused (with a code from N1
    #13429 and whether a retry can help), and **outcome unknown**.
  - Every write accepts an idempotency key, and a repeat with the same key
    returns the first result.
  - The shape is a `proto/` shape, so the CLI, MCP and the editor's agent
    strip (E9) read the same thing.
- **S3 (#13437). Every action and every picture names what it touched.**
  - Drive commands, `apply` and `realm_shot` return the realm, rev, build and
    page they ran against.
  - Shots also return the camera kind and whether the GPU was real or
    headless.
  - A command sent to a page showing another realm or rev is refused, never
    applied.
- **S5 (#13438). Outcome replay.**
  - A recorded run of operations replays into a fresh World. Four checks run
    separately:
    1. The live World.
    2. The saved document after a reload.
    3. Runtime errors.
    4. Gameplay probes, such as "the player lands on the platform" (a collider
       query) or "the door opens" (a system ran).
  - Each check also runs against a broken variant, which must fail.
  - It uses the World from E3 and the `why` chain from N1, so a failed probe
    says which link broke.
  - Splice is the first suite, and its recorded edits become regression
    tests.

## Adapt soon

- **S4 (#13439). Search, then call, once the command table passes about 60 rows.**
  - Keep the common verbs loaded and add `realm_find` and `realm_call`.
  - Ranking uses a typed selector over the rows' fields, or an embedding
    search over their help text, as `AGENTS.md` requires. Measure first-call
    tokens for Claude Code and Codex before and after. Summer saw 21% and 0%
    with clients that already defer tools, so don't expect much.
- **S6 (#13440). A page-build check that no room-only module is linked in.** It's
  cheap, and it keeps today's split true as more code is shared.
- **Eval families in E14's report:** discovery, execution, live state, saved
  state, visuals, planning, and honest reporting of what was left undone, each
  scored on its own.
- **Asset generation jobs (with E11).** Estimate first, take an idempotency
  key, report `done` only once stored, and record the model used. They return
  S2's result shape.
- **In-game model calls** declare a fallback, and tests run the fallback.

## Don't adopt

- **Client-run movement with pose checks.** Rooms stay authoritative.
- **A Godot fork or project files.** Our World, content tables and Coax are
  the vocabulary.
- **Summer's lexical operation search.** The shape is good, but keyword
  ranking for tool selection is against the workspace rule.
- **Many simultaneous save versions per world.** One record schema with
  migrations, not version-pinned save blobs.

## Order, given what is in flight

1. **Before E6 starts:** S1 and S2 on `REALM_COMMANDS`, with S2's refusal
   codes coming from N1 (#13429).
2. **S3 along with E3 and E6**, while the identity fields are being defined
   anyway.
3. **S5 once E3 saves and loads a World**, starting with Splice.
4. **S4, S6 and the eval families** with E9-E14.

## What would change our minds

Summer publishes token counts for tool loading and a 168-session interface
test, but no game-quality results and no multiplayer load numbers. If its
whole-game ladder runner ships with results that show agents finishing real
games more often with search-then-call, move S4 to now.
