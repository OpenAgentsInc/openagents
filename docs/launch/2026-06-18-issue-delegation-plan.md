# Issue Delegation Plan — Crunching the Open Backlog

Date: 2026-06-18
Repo: `OpenAgentsInc/openagents`
Branch basis: `origin/main` @ `e8131c9`
Scope: analysis / planning only. This document does NOT fix issues or change code.

## Purpose

The owner wants to hand the open-issue backlog to one or more agents to crunch
through. This doc answers, concretely:

1. What can/should be delegated and how it groups.
2. If delegated to ONE agent, the exact top-to-bottom order.
3. How parallelizable it is — which clusters run concurrently, which MUST be serial.
4. Execution aids: batch sizes, owner-gated items, stale/closeable, duplicates,
   miscategorized items, and the invariants the executing agents inherit.

## Headline counts

- **Total open: 19.** (`gh issue list`/`gh search issues` return **18**;
  `#5399` is genuinely open per `gh api repos/.../issues/5399`
  (`state: open`, `closed_at: null`) but is missing from the list/search index
  — likely transient index lag since it was the most-recently-created issue.
  Treat the backlog as **19** and include `#5399`.)
- Every open issue is labeled `roadmap` (except `#5399`, which has no labels)
  and was created/updated **2026-06-18** — this is a freshly-decomposed roadmap,
  not an aged backlog. There are effectively **no stale issues**.
- All 19 are in `OpenAgentsInc/openagents`. No milestones set.

By cluster:

| Cluster | Issues | Count |
| --- | --- | --- |
| A. W-* headless coding workflow (deploy gate) | 5376 (EPIC), 5378, 5380, 5381, 5382, 5383 | 6 |
| B. Pylon V1.0 launch readiness | 5392 (EPIC), 5396, 5401, 5403 | 4 |
| C. Hygiene/refactoring lane + settlement | 5335 (EPIC), 5372, 5399 | 3 |
| D. Autopilot Desktop coding surface | 5360 (EPIC), 5364 | 2 |
| E. Replay clip generation | 5346 (EPIC), 5347 | 2 |
| F. Windows support (exploratory) | 5404 | 1 |
| G. Studying/marketplace (gated, deferred) | 5342 | 1 |

By release-relevance (relative to the Pylon V1.0 "Tassadar Run is Live" video):

| Relevance | Issues | Count |
| --- | --- | --- |
| Launch-relevant (touches V1.0 video / its evidence surfaces) | 5392, 5396, 5403, 5404\* | ~3–4 |
| Post-launch / operational-bar (explicitly OUT of V1.0 scope) | 5376, 5378, 5380, 5381, 5382, 5383, 5360, 5364 | 8 |
| Parallel funded lane (independent of launch) | 5335, 5372, 5399 | 3 |
| Net-new capability (no launch dependency) | 5346, 5347 | 2 |
| Gated / deferred (owner says lower priority) | 5342, 5401 | 2 |

\* `#5404` (Windows) is launch-*adjacent* — it widens the contributor pool but
the V1.0 video uses macOS + `npx`, so it is not a hard launch blocker.

Important framing from `#5396`: **V1.0 scope is the contributor earning path
only.** Explicitly OUT of V1.0: the five revenue streams, the module
marketplace, and the W-* headless coding workflow. So most of the backlog
(Clusters A and D) is **post-launch operational-bar work**, not launch-blocking.
The actual remaining V1.0 launch surface is small (Cluster B polish + `#5404`).

## Full categorization table

`Rel`: L = launch-relevant, PL = post-launch, IND = independent lane,
NEW = net-new capability, GATE = gated/deferred.
`Size`: S/M/L rough effort. `Worktree`: code-mutating work needing isolation.

| # | Title (short) | Cluster | Rel | Size | Depends on / blocked by | Parallel-safe with | Worktree |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 5376 | EPIC W-* headless coding workflow | A | PL | — | (epic; W-1/W-3 already CLOSED) | tracking only | no |
| 5378 | W-2 sessions reply | A | PL | M | W-1 (#5377 CLOSED ✓) | C, E | yes (pylon) |
| 5380 | W-4 batch fan-out | A | PL | M | W-1 (✓), W-2 (#5378) | C, E | yes (pylon) |
| 5381 | W-5 per-task worktree isolation | A | PL | M | W-1 (✓) | C, E | yes (pylon) |
| 5382 | W-6 task issuance to managed node | A | PL | L | W-1/W-2; ties to CS-B1 (#5364) | E | yes (pylon) |
| 5383 | W-7 DOGFOOD PROOF (acceptance gate) | A | PL | M | W-2,W-4,W-5,W-6 + W-3(✓) | (serial tail) | yes (pylon) |
| 5392 | EPIC Pylon V1.0 launch readiness | B | L | — | (epic; L-1..L-3,L-5,L-6 CLOSED ✓) | tracking only | no |
| 5396 | L-4 define + cut V1.0 | B | L | M | L-1 (#5393 CLOSED ✓) | — (publish = serial) | no (release) |
| 5401 | operator snapshot clean-exit bug | B | PL | S | none | C, E | yes (pylon) |
| 5403 | evidence-surface polish (alias/endpoint/recon) | B | L | S | none | A, C, E | yes (web/api) |
| 5335 | EPIC hygiene lane | C | IND | — | (epic; #5334,#5369,#5340 CLOSED ✓) | runs alongside all | no |
| 5372 | extend settlement to hygiene lane | C | IND | L | #5340(✓),#5369(✓); OWNER-ARMED gate | (gate work = serial) | yes (web/api) |
| 5399 | document hygiene debt-receipt route | C | IND | S | none (settlement-adjacent contract) | A, E | yes (web/api) |
| 5360 | EPIC Autopilot Desktop coding surface | D | PL | — | (epic; CS-A1..A3 CLOSED ✓) | tracking only | no |
| 5364 | CS-B1 packaged node + sign/notarize | D | PL | L | gated on W-1..W-3 + W-7 (#5383) | E | yes (desktop) |
| 5346 | EPIC replay clip generation | E | NEW | — | (epic; R-1 gates rest) | runs alongside all | no |
| 5347 | R-1 one-frame headless render spike | E | NEW | M | none (de-risks R-2..R-5) | A, B, C, D | yes (render) |
| 5404 | Windows support exploratory plan | F | L(adj) | M | none (analysis only) | everything | no (doc) |
| 5342 | SA-5 studying→marketplace/payout (gated) | G | GATE | L | many gates; owner says lower-pri | — | no (planning) |

## 1. What can / should be delegated

**Delegate now (analysis or self-contained build):**
- `#5347` R-1 render spike — bounded, de-risks all of Cluster E. High-value spike.
- `#5404` Windows plan — pure analysis/exploration, no code mutation; safe anywhere.
- `#5401` operator snapshot clean-exit — small, well-scoped bug with a known fix
  pattern (the rc.31–33 `status`/`backup-status` treatment) + a regression test.
- `#5403` evidence-surface polish — three small, mostly-independent fixes; an
  honesty-relevant one (1010→1005 sim-row reconciliation).
- `#5399` OpenAPI doc for the hygiene debt-receipt route — small contract hygiene.
- `#5378`/`#5380`/`#5381` W-2/W-4/W-5 — the W-1/W-3 prerequisites are CLOSED, so
  these are buildable now (they extend the proven composer continuation model).
- `#5335` hygiene lane — this is itself a *delegation vehicle*: it's a funded,
  benchmark-verified lane meant to be handed to contributor agents continuously.

**Delegate with care / sequencing:**
- `#5382` W-6, `#5383` W-7, `#5364` CS-B1 — these have real cross-issue
  dependencies (W-7 is an acceptance gate; CS-B1 is gated on W-1..W-3 + W-7).
- `#5396` L-4 cut V1.0 — owner-gated *publish*; the prep is delegable, the cut is not.
- `#5346`/`#5360`/`#5376`/`#5392` — EPICs. Don't "do" them; track their children.

**Owner input required before/within (do NOT let an agent self-authorize):**
- `#5372` settlement to hygiene lane — **real Bitcoin.** Owner-armed gate scope
  only; the agent builds + tests the path, the main/owner-armed session arms the
  gate scope and executes bounded canaries.
- `#5396` L-4 — owner-gated npm publish.
- `#5342` SA-5 — owner says explicitly lower priority (dogfood first); planning
  only, no premature green.

## 2. Single-agent serial order (top to bottom)

If ONE agent runs the whole list, this is the order, leading with unblockers and
launch-readiness, then high-value capability, then post-launch operational bar,
then deferred:

**Phase 0 — pure analysis, no code, do first to inform everything else**
1. `#5404` Windows exploratory plan — analysis-only; no merge risk; produces a
   roadmap the owner may want before any Windows build is scoped. Cheap to front-load.

**Phase 1 — launch-readiness polish + small bugs (small, independent, honesty-relevant)**
2. `#5403` evidence-surface polish — fix the public surfaces the V1.0 video may
   lean on (settlements `/public/` alias, per-challenge endpoint, **1010→1005**
   sim-row reconciliation). Honesty-relevant; do before the video references stats.
3. `#5401` operator snapshot clean-exit — restores `release:gate` reliability for
   future v1.x cuts; small, known fix pattern.
4. `#5399` document hygiene debt-receipt OpenAPI route — closes a silent,
   settlement-adjacent API surface; small contract hygiene.

**Phase 2 — de-risk the net-new capability (one bounded spike)**
5. `#5347` R-1 headless render spike — single bounded experiment that determines
   whether all of Cluster E (replay clips) is feasible. Do early so a NO answer
   surfaces before anyone scopes R-2..R-5.

**Phase 3 — the W-* headless coding workflow (the real operational bar; the
ordered chain inside EPIC #5376)**
6. `#5378` W-2 sessions reply — first W-* unblocker; many later W-* steps want a
   headless multi-turn loop. (W-1 #5377 + W-3 #5379 already CLOSED.)
7. `#5381` W-5 per-task worktree isolation — must exist before fan-out so
   parallel tasks don't collide.
8. `#5380` W-4 batch fan-out — builds on W-1 + (ideally) W-5 isolation.
9. `#5382` W-6 task issuance to a managed/remote node — ties to CS-B1; harder.
10. `#5383` W-7 DOGFOOD PROOF — the **acceptance gate**: needs W-2/W-4/W-5/W-6.
    Run it only after its prerequisites land.

**Phase 4 — desktop packaging gate (gated on Phase 3)**
11. `#5364` CS-B1 packaged + signed/notarized node — explicitly gated on
    W-1..W-3 + W-7 (`#5376` deploy gate). Do not cut a signed release before W-7 passes.

**Phase 5 — funded hygiene lane + its money rail (owner-armed)**
12. `#5335` hygiene lane — stand up / drive the lane (its first passes #5334 etc.
    are already CLOSED; keep filing + running funded passes).
13. `#5372` settlement to hygiene lane — **real money; owner-armed gate.** Build +
    test the path under the agent; arm the gate scope and run bounded canaries
    only in the owner-armed/main session. Last in the build order because it
    depends on the lane producing merged debt receipts.

**Phase 6 — owner-gated release cut**
14. `#5396` L-4 define + cut V1.0 — lock scope, version-sync, run the runbook;
    **owner-gated publish.** Cut after Phase 1 polish is in and scope is locked.

**Phase 7 — deferred / gated**
15. `#5342` SA-5 studying→marketplace/payout — owner-flagged lower priority
    (dogfood first); planning + gate-definition only, no premature green.

EPICs `#5376`, `#5392`, `#5360`, `#5346`, `#5335` are tracking containers — close
them when their children land; don't schedule them as work units.

## 3. Parallelization map

This backlog parallelizes well because it spans **distinct subsystems** that
rarely touch the same files. The hard constraints are (a) the W-* chain has an
internal dependency order, (b) anything touching the worker `index.ts` /
settlement gate / migrations is a shared-file/authority hot-spot, and (c) the
publish + money-arming steps must be serial and owner-driven.

### Independent concurrent lanes (safe to run as separate worktree-isolated agents)

- **Lane 1 — Windows analysis (`#5404`).** Doc-only. Zero merge risk. Run anytime.
- **Lane 2 — Replay spike (`#5347`).** Net-new render-box code; its own files
  (`proof-replay`, `three-effect` scene mount, a spike entrypoint). Touches no
  worker/pylon/settlement surface. Fully independent.
- **Lane 3 — W-* coding workflow (`#5378`→`#5381`→`#5380`→`#5382`→`#5383`).**
  Concentrated in `apps/pylon` (CLI/control API). **Serial *within* the lane**
  (W-7 needs W-2/W-4/W-5/W-6). Independent of the web/api worker, so it runs
  concurrently with the web lane. One agent owns this lane end-to-end to avoid
  intra-pylon collisions.
- **Lane 4 — web/api small fixes (`#5403` + `#5399`).** Both touch
  `apps/openagents.com/workers/api`. Run them as **one agent** (same files) to
  avoid intra-worker merge conflicts; that single agent can run concurrently with
  Lanes 1–3.
- **Lane 5 — hygiene lane itself (`#5335`).** Designed to fan out to many
  contributor agents in parallel, each on a worktree, each benchmark-gated. This
  is N concurrent sub-agents by construction (its whole point).

### Hard serialization edges (do NOT parallelize across these)

- **Settlement gate / real money (`#5372`).** Must be serial and owner-armed.
  Never run concurrently with another agent that also touches the settlement
  receipt path or `OPENAGENTS_REAL_SETTLEMENT_GATE`. One armed session at a time.
- **W-* internal order.** W-7 (`#5383`) is an acceptance gate; W-4 (`#5380`)
  wants W-5 (`#5381`) isolation first. Keep the chain serial inside Lane 3.
- **CS-B1 (`#5364`)** is gated on the whole W-1..W-3 + W-7 set — it cannot start
  its release-cut portion until W-7 passes. Sequence after Lane 3 completes.
- **L-4 publish (`#5396`)** — single serial owner-gated npm publish. Never
  concurrent with another release/publish action.
- **Shared-file hot-spots — flag for whoever executes:**
  - the big worker `index.ts` / route registry in `apps/openagents.com/workers/api`
    (touched by `#5403`, `#5399`, and indirectly `#5372`) — merge-conflict magnet;
    keep web-lane work single-threaded.
  - the settlement-receipt / gate path (`#5372`) — authority hot-spot; owner-armed only.
  - any D1 migration — serialize; concurrent migrations corrupt the schema chain.
  - `apps/pylon` CLI/control surface (all W-*) — keep within one Lane-3 agent.

### Recommended max concurrency

**5 concurrent agents** at peak:
- Lane 1 (Windows doc), Lane 2 (replay spike), Lane 3 (W-* pylon), Lane 4
  (web/api fixes), Lane 5 (hygiene fan-out — itself multi-agent).

Then **converge to serial** for the tail: CS-B1 (`#5364`) after W-7, the
settlement arming (`#5372`) owner-armed alone, and the L-4 publish (`#5396`)
owner-gated alone. `#5342` (SA-5) is planning-only and can run any time but is
deprioritized by the owner.

Each code-mutating lane gets its **own git worktree off `origin/main`** (the
harness/`pylon` worktree-isolation path, see `#5381`). Doc-only lanes (`#5404`,
`#5342` planning) do not strictly need isolation but should still branch cleanly.

## 4. Execution aids

- **Batch sizes:** Lane 4 = 2 issues/agent (same files). Lane 3 = the full W-*
  chain to ONE agent (5 issues, serial). Lanes 1/2 = 1 issue each. Lane 5 =
  many small benchmark-gated passes (the hygiene model — keep each pass small so
  a green benchmark cheaply proves it changed nothing it shouldn't).
- **Owner-gated (NEEDS-OWNER — spend / policy / publish):**
  - `#5372` — real Bitcoin; owner-armed settlement-gate scope + bounded canaries.
  - `#5396` — owner-gated npm publish.
  - `#5342` — owner-flagged lower priority + multiple product-promise gates; no
    premature green.
- **Stale / closeable:** none. All 19 were created/updated today. (`#5399`'s
  absence from the list index is an index-lag artifact, not a closure — it is
  genuinely open per the API.)
- **Duplicates:** none among the open set. `#5404` *references* prior CLOSED
  Windows issues (`#4468`, `#4655`, `#3429`, `#4568`) as material to mine — those
  stay closed; do not reopen.
- **Miscategorized (per strict-bug-issue policy):** the open set is clean — it is
  a decomposed roadmap (EPICs + tracked children), not a pile of bug reports
  masquerading as features. `#5401` is the only genuine bug and is correctly
  scoped (with a known fix pattern + regression). No Forum/product-promise items
  are mislabeled as bugs here.
- **Already-unblocked (prerequisites CLOSED):** W-1 (`#5377`), W-3 (`#5379`),
  L-1/L-2/L-3/L-5/L-6 (`#5393`/`#5394`/`#5395`/`#5397`/`#5398`), CS-A1/A2/A3
  (`#5361`/`#5362`/`#5363`), and hygiene seeds (`#5334`/`#5369`/`#5340`) are all
  closed. So the open EPICs are mostly in their *late* children — the heavy
  prerequisite work is done.

## Invariants the executing agents inherit (encode these)

These are repo invariants every delegated agent must follow. Whoever executes
this plan inherits them:

- **Never add GitHub Actions / GitHub-hosted CI.** Any build-verify (incl. the
  `#5404` Windows build-verify/sign/test) runs on **our infra** (e.g. a GCP
  Windows Server VM — GCP, not Azure), never GitHub-hosted runners.
- **Deploy / publish only from a clean `origin/main`.** No publishing from a
  feature/worktree branch. Branch work is in-progress evidence, not "done."
- **Receipt-first + honest-scope.** Every green claim must be dereferenceable
  (tests-green / regenerate-and-diff / settled receipt). No green promise flips
  without dereferenceable receipts **and** owner sign-off.
- **Settlement-gate scope is owner-armed only.** `#5372` real-money dispatch
  happens only inside an owner-armed session; agents build + test the path but
  never broaden `OPENAGENTS_REAL_SETTLEMENT_GATE` or arm a new run-ref themselves.
  Reuse the gated, idempotent, fail-closed Tassadar settlement mechanism — do
  NOT build a parallel money rail.
- **Code-mutating parallel work needs worktree isolation.** Each concurrent
  code-mutating lane gets its own git worktree off `origin/main`; clean it up if
  unchanged (the `#5381` model). This is the conflict-avoidance mechanism for the
  parallel lanes above.
- **Close issues on merge to `main`.** An issue is complete only after its
  code/docs are merged + pushed on `main` and any required live proof has run
  from that integrated state — not while it lives on a branch.
- **No green promise flips without the owner.** Especially the V1.0 launch claims
  (`#5392` family) — hold the video / operational claim until the gated proofs
  clear and the owner signs off.
