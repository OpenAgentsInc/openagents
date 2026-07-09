# Coding parallelism now and the Sarah fleet cutover

- Date: 2026-07-09
- Status: operational companion to [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md)
- Original source snapshot: `origin/main` at `5c4ec13dc7`; reconciled with
  master roadmap rev 3 and the 17-issue set on 2026-07-09

## What can be parallelized in this Codex session now

In this current Sol Codex app runtime, the root plus up to three concurrently
active subagents are available—**four active agents total**. This is the
surfaced cap for this session, not a permanent Codex-wide limit. The owner can
ask Sol to fan out bounded issues, and the repository mandate now says to
delegate proactively when lanes are genuinely independent. Extra Codex tabs
are not required for two to four well-partitioned lanes.

All agents in the session share the repository filesystem. Parallel work
therefore uses:

- one clean worktree per implementation lane;
- one issue or explicitly disjoint file set per agent;
- serialized changes to shared schemas, generated catalogs, behavior-contract
  registries, package-script keys, lockfiles, migrations, route tables, and
  other hot files/contracts;
- one coordinating agent to reconcile tests, issue state, rebases, and pushes
  to `main`;
- no stash, reset, checkout, or cleanup of another agent's work.

This is useful parallelism, not four independent repositories or four separate
provider budgets. These subagents are Codex-session agents; they do not
automatically consume the owner's connected Claude or Grok accounts and are
not a substitute for FC-2's real mixed-harness supervisor.

## When multiple Codex tabs are better

Use additional tabs when:

- more than four active lanes are needed;
- work needs separately steerable, long-lived contexts;
- lanes should not share the same conversation lifecycle;
- distinct Codex accounts intentionally supply distinct provider capacity;
- a risky or exploratory lane should be operationally isolated from the main
  coordinator.

Tabs backed by the same Codex account may provide additional sessions, but do
not create a new account quota or rate budget. Tabs or workers backed by
different connected accounts can add real provider capacity. Every tab still
needs an explicit issue, worktree, ownership boundary, and integration owner;
otherwise coordination cost and collision risk erase the extra throughput.
Before mutation it posts the cross-session `CLAIM` defined in
[`CLAIM_PROTOCOL.md`](./CLAIM_PROTOCOL.md), including hot contracts; same-
session subagent claims remain coordinated by the root.
Using Claude or Grok before FC-2 requires their own explicitly operated client
or harness path; opening another Codex tab does not turn it into a Claude or
Grok worker.

For the immediate FC-1/FC-2/FC-3 build, one coordinated session with up to four
agents is enough if work is divided along the existing contract, Pylon
executor, Sarah projection, and verification seams. Additional tabs are
optional capacity, not a prerequisite.

## Current cutover status

The Sarah/Khala fleet workflow is **not yet the honest primary coding path**.
Sarah cannot currently create a durable FleetRun, the live mixed-harness
manager is not yet the standing durable executor, and Sarah does not yet expose
the minimum named-stream progress and control seam. Until those land, this
Codex app—using coordinated subagents or explicitly separated tabs—is the
implementation control plane for building the replacement.

## Cutover gates

### C0 — interim implementation mode: now

Use this Codex app for #8637, #8633, and #8639. Parallelize through child
agents here or additional tabs, with clean worktrees and one integration
owner. Do not describe Sarah as able to run the coding fleet yet.

### C1 — first real Sarah canary: earliest point work can move

The first bounded real issue may be delegated from `/sarah` when all of these
are true on one pinned integrated commit and deployment:

1. **#8637:** an authenticated Sarah tool creates one durable owner-scoped run
   with pinned repository, commit, work units, verifier, idempotency key, and a
   stable `runRef`; decoded operator mode—not model inference—selects concise
   non-commercial posture and fleet-tool policy.
2. **#8633:** one standing Pylon claims the run without a supervising Codex/CLI
   process and executes through named isolated real accounts using the shared
   claim registry.
3. **Minimum safe #8639 seam:** Sarah shows each named work unit and current
   state, can stop the run and steer or answer an approval for an unambiguous
   unit, survives browser reconnect, and renders verification/closeout without
   exposing private worker data.
4. The Sarah→durable run→Pylon claim→worker→verification→closeout fixture passes
   with owner isolation, duplicate-claim, refusal, reconnect, and redaction
   checks.
5. Acknowledgment/run ref, first capacity, first progress/blocker, and heartbeat
   freshness meet the 5s/15s/30s/15s budgets or visibly enter typed delayed/
   stalled/reconnecting states; media failure cannot remove text/fleet control.

At C1, move one low-risk, pinned, public-repository task through Sarah as a
canary. Keep this Codex app available as observer and break-glass debugger. C1
does not close #8639 or make Sarah the default; it proves the first real handoff
is safe enough to attempt.

### C2 — default owner-local cutover: #8640 Phase A

Sarah/Khala becomes the **default entry point for new bounded owner coding
work** immediately after #8640 Phase A produces a clean receipt proving:

- at least three simultaneous useful work units under one FleetRun;
- one real Codex, Claude, and Grok completion on owner-local capacity;
- Sarah started and managed the run;
- one named steer or approval round trip and one reconnect;
- zero duplicate claims, default provider homes, silent substitutions, or
  manually launched per-assignment shells;
- honest usage truth, verification, and closeout visible through Sarah;
- the measured latency distribution and one-minute coding-closeout receipt meet
  the FC-1/FC-3 contract, with every acceptance item labeled by its six-rung
  proof state.

After C2, do not open routine pinned backlog work directly in this Codex app by
default. Use Sarah/Khala/Pylon. Retain the Codex app for:

- repairing or extending the fleet control plane itself;
- break-glass recovery when Sarah, Khala, Sync, or Pylon is degraded;
- high-risk authority/migration work not yet expressible as a bounded Sarah
  plan;
- independent review of the fleet's evidence when separation is useful.

Every fallback should be recorded in the #8640 friction ledger so repeated
fallback classes become typed Sarah-fleet backlog rather than permanent manual
procedure.

For this gate, **clean** means every required item comes from the same pinned
integrated deployment: owner/auth scope, named isolated accounts, fresh
advertised capacity, claim uniqueness, typed fallback, reconnect,
verification, closeout, and honest usage evidence all pass without manual
per-assignment shells or silent substitution.

### C3 — default hybrid cutover: #8640 Phase B

After #8547 and #8636 are integrated, #8640 Phase B must prove at least one
owner-local unit and one managed Agent Computer unit both complete useful,
verified work concurrently under the same claim registry, with typed visible
target selection and separate compute/model usage truth. Sarah may then route
bounded work through `owner_local | managed_cloud | auto`. This expands
capacity and availability; it is not a prerequisite for C2.

### C4 — broader production use

Use beyond owner dogfood additionally requires the production inference,
security, capacity, support, and promise gates appropriate to that audience,
including #8600 where Sarah's production brain path is involved. C4 does not
move the owner-local cutover later.

## Explicit non-blockers for the local switch

C1 and C2 do not wait for:

- #8610 avatar or presentation perfection;
- #8547/#8636 managed cloud;
- completion of the three-app Effect Native conversion;
- retirement of old public web pages;
- every planned pause/drain/cockpit affordance beyond the minimum safe canary
  controls.

Text-first Sarah fleet control, authenticated authority, durable state, named
work-unit identity, stop/steer/approval safety, reconnect, verification, and
receipts are blockers. Presentation polish and additional hosts are parallel.

## The practical decision

- **Need two to four lanes today:** ask Sol to fan them out in this session.
- **Need more or separately owned contexts today:** open more Codex tabs with
  explicit worktree and issue ownership; use distinct accounts only when real
  additional provider capacity is intended.
- **First task through Sarah:** C1 fixture is green; run one real canary.
- **Stop using this Codex app as the routine front door:** #8640 Phase A is
  clean (C2).
- **Let Sarah choose local or cloud:** #8640 Phase B is clean (C3).
