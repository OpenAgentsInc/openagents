# Parallelizing the Gym build: the delegation protocol

Date: 2026-08-27. Companion to [`roadmap.md`](roadmap.md). This document is
the operating instruction for turning the open Gym issues into parallel
subagent work — any number of workers, one per independent issue, each in its
own worktree, each landing through the same gates.

## 1. The rule that makes parallelism safe

**The dependency graph, not the issue count, decides parallelism.** Two
issues may run concurrently exactly when neither is blocked by the other and
they touch disjoint files. Everything else waits. The tracker's `deps` edges
(`openagents issue view <n>` shows `blocked_by`) are the machine-readable
form; this document's job is to translate them into worker assignments.

Concurrently safe (disjoint surface):
- #167 (suite commands: new `src/gym/` module + `cli.rs` wiring) and
  #170 (corpus inventory: new engine, touches `trace.rs` consumers) —
  different modules, both green independently.
- #173 (env probe/doctor) and #170 — nothing shared.
- #176 (schema freeze) with anything, as long as it lands first or the
  schema types live in their own module others import — it defines the
  shared contract, so it goes first or goes alone.
- #175 (labels, manual human work) — no code at all; "parallel" with
  everything by definition.

Never concurrent:
- Two issues that both modify `crates/openagents-cli/src/cli.rs` *and* the
  same new module. The `GymArgs`/`GymAction` wiring is the choke point: one
  worker owns `cli.rs` at a time. When two otherwise-disjoint issues both
  need it, sequence them (`cli.rs` wiring is a 20-line diff; serialize the
  wiring, parallelize everything else).
- Anything against its blocker — the graph forbids it automatically.

## 2. Per-worker procedure (every worker, every issue)

1. **Claim.** Read the issue in full (`openagents issue view <n>`), including
   every acceptance criterion. A worker that cannot restate the gate in one
   sentence is not ready to start.
2. **Fresh worktree.**
   ```sh
   git fetch openagents main
   git worktree add --detach ../openagents-gym-<issue-n> openagents/main
   cd ../openagents-gym-<issue-n>
   ```
   One worktree per issue, never shared, never reused for a second issue.
3. **Implement in Rust** per the spec (`gym-cli-spec.md`); new modules under
   `crates/openagents-cli/src/gym/`; the shared `cli.rs` wiring diff kept
   minimal (subcommand enum + dispatch arm only). Stub-server tests follow
   `tests/trace_upload_test.rs`; golden surface tests pin output shapes.
4. **Prove it.** Every acceptance criterion in the issue gets its evidence:
   a test, a command transcript, or a fixture. "Should work" is not evidence
   (best practice V1).
5. **Gate.** `pnpm run check` green, plus `cargo test -p openagents-cli` for
   the touched crate. A worker whose gate is red does not push; it fixes or
   reports back.
6. **Land and close.** Commit with the issue number in the message, push to
   the forge remote, close the issue with a comment naming the evidence:
   commit sha, test names, and the acceptance criterion each satisfies.
7. **Clean up.** Remove the worktree. The main checkout never sees WIP.

## 3. Scaling to N workers

The wave pattern: group the open issues by "blocked only by issues already
closed," then run each wave at full width.

Wave 0 (all independent — run **all five simultaneously**):
- #167 suite list/show/check — new `src/gym/suite.rs` + wiring
- #170 corpus inventory/qualify — new `src/gym/corpus.rs`, touches inventory
  engine only
- #173 env probe/doctor — new `src/gym/env.rs`
- #176 schema freeze — types + fixtures in `src/gym/schemas.rs` (if it lands
  mid-wave, later waves absorb renames; it defines, others follow)
- #175 labels (manual, owner-side; not a coding worker at all)

Wave 1 (after Wave 0 closes):
- #168 run (needs #167; takes over the `cli.rs` wiring from whichever Wave-0
  worker's pattern won review)
- #169 results score/compare (needs #167, #168)
- #171 corpus import (needs #170, and the ingest client patterns from #161's
  Epic A work)
- #174 harbor-runner image + pull (needs #173)
- #172 dataset commands (needs #170's ledger shape)

Wave 2: distiller (D4), first suite recorded (D5), TUI pane (E2),
`run status/list/cancel` (A4 remainder), Box lanes (B5), corpus verify (C4
remainder).

Wave 3: second-domain selection (F1) and its build-out.

**Width cap.** The practical ceiling is not enthusiasm, it is contention:
one worker per issue, and no two live workers on the same module. With the
current first wave that is five coding workers; a second machine adds
nothing until Wave 1 exists because the graph, not the fleet, is the
constraint. Ten idle workers is a cost, not a speedup — right-size the fleet
to the wave's width and spin up more when the wave turns over.

**Heartbeats and stragglers.** A worker that will miss its estimate comments
on its issue with state and the specific blocker before going quiet — a
silent worker is treated as failed at 2× its estimate, its worktree
inspected, and the issue reaped by a fresh worker (the old worktree is
thrown away, not merged piecemeal).

**Merge order inside a wave.** Workers land independently (each pushes its
own branch to the forge), but rebase-on-`main` order is first-come; a worker
whose rebase conflicts (shared `cli.rs` wiring) rebases onto the landed
sibling and re-runs the gate before pushing. The wiring conflict is the only
expected one, and it is deliberately trivial.

## 4. What the orchestrator does

The orchestrating session (this one, or any coder session running the wave):

1. Reads the tracker (`openagents issue list --json`), computes the wave from
   `blocked_by`, and emits the worker roster: issue → worktree path →
   estimate.
2. Spawns one subagent per issue in the wave (`delegate`, count = wave
   width), each prompt self-contained: the issue number, the worktree path,
   the files it owns, the gate commands, and the "comment and close with
   evidence" ending.
3. Watches for completion, enforces the heartbeat rule, and reaps failures.
4. When the wave empties: recompute blocked_by, announce the next wave, and
   repeat — or stop and report if the next wave needs a human call (e.g.
   #175's owner review, #177's domain decision).

The orchestrator never codes. Its outputs are rosters, merges, and decisions
the issues explicitly reserve for a human (labels review, domain selection,
anything touching `bench-results/` rows).
