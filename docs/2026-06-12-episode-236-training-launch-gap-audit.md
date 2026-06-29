# Episode 236 Training Launch Gap Audit

Date: 2026-06-12

Transcript source: [`docs/transcripts/236.md`](transcripts/236.md)

Live surfaces checked:

- `https://openagents.com/api/public/product-promises`
- `https://openagents.com/api/public/pylon-stats`
- `https://openagents.com/api/training/runs/run.cs336.a1.real_gradient.demo`
- `https://openagents.com/api/training/leaderboards/a1`
- `https://openagents.com/api/forum/work-requests`
- `https://openagents.com/api/forum/forums/product-promises/topics`
- `https://openagents.com/api/forum/forums/work-requests/topics`
- GitHub issues `#4749`, `#4768`, `#4772`, `#4777`, `#4781`,
  `#4782`, `#4783`, and `#4786`

## Bottom Line

Episode 236 is not a narrow Autopilot issue. It is a launch promise cluster:

1. a Monday launch target for a large decentralized training run;
2. Pylon v0.3 as installed node software;
3. Bitcoin payment for useful training contribution;
4. a possible largest-run comparison against roughly 200 contributors;
5. Pylon v0.3 combining training, Bitcoin payments, Forum, and coding-agent
   work into one multi-earning node;
6. Tassadar/Percepta Executor Class model direction and CPU-computation
   transformation support.

The product has meaningful evidence, but not enough for broad Episode 236
copy. The strongest current evidence is a bounded two-device CS336 A1
real-gradient run with public verification and settlement refs, plus a green
Tassadar executor proof of concept. That does not prove a large public
training launch, 200-plus contributors, self-serve Pylon earning, stable
Pylon v0.3, or a trained Tassadar model.

No promise should flip green from this audit. Product promises should be
tightened to:

- keep `training.monday_decentralized_training_launch.v1` red;
- keep `training.public_distributed_training_run.v1` red;
- keep `pylon.largest_decentralized_training_claim.v1` red;
- keep `pylon.v0_3_multi_earning_node.v1` red;
- keep the canonical `models.tassadar_percepta_executor.v1` red;
- withdraw the misspelled `models.tasadar_percepta_executor.v1` alias.

## Transcript Claims

Episode 236 says contributors should be able to install Pylon node software
and get paid Bitcoin to contribute to a training run. It frames the target
as a large decentralized training run and implies that beating a roughly
200-contributor benchmark would matter. It also says Pylon v0.3 will include
Percepta Executor Class model support, Bitcoin payments, Forum integration,
and coding-agent work, with the experimental model direction called
Tassadar.

Those are product promises only if the product can show current evidence.
The transcript itself is source material, not evidence.

## Current Evidence

### Product Promises

Before this audit, the live public registry served version `2026-06-11.9`.
It already carried Episode 236 rows:

- `training.public_distributed_training_run.v1`: red
- `training.monday_decentralized_training_launch.v1`: red
- `pylon.largest_decentralized_training_claim.v1`: red
- `pylon.v0_3_multi_earning_node.v1`: red
- `models.tasadar_percepta_executor.v1`: red, but misspelled

This audit updates the source registry to version `2026-06-12.2`, adds the
canonical `models.tassadar_percepta_executor.v1` red record, and leaves the
old `models.tasadar_percepta_executor.v1` as a withdrawn alias. It also
updates the Pylon v0.3 release-candidate copy from rc1 to rc2 because
`apps/pylon/package.json` is now `0.3.0-rc2`.

### Pylon v0.3

`apps/pylon` is present as `@openagentsinc/pylon@0.3.0-rc2`. The README says
the rc is source-only and no npm artifact is published. That supports rc
development and operator testing, not a stable broad install claim.

The public stats endpoint observed during this audit reported:

- `pylonsOnlineNow: 4`
- `pylonsSeen24h: 9`
- `pylonsRegisteredTotal: 49`
- `pylonSessionsOnlineNow: 4`
- `sellablePylonsOnlineNow: 2`

The top-level read did not expose wallet-ready, assignment-ready, or
accepted-work settled totals as ready-to-use green evidence for Episode 236.
Online Pylons are coordination evidence. They are not proof of a public
training launch, participant admission, useful work, payment, or settlement.

### Training Run Routes

`/api/training/runs/run.cs336.a1.real_gradient.demo` is the strongest current
training evidence. It exposes:

- `assignedContributorCount: 2`;
- `reconciledWindowCount: 2`;
- `verifiedWorkCount: 3`;
- two window lease refs;
- deterministic recompute and Freivalds-Merkle verdict refs;
- a public loss curve through the A1-scale run.

The evidence doc
`apps/openagents.com/docs/2026-06-11-cs336-a1-multi-device-real-gradient-evidence.md`
records two physical devices, cross-device verification, and two 30-sat
settled receipts.

The gap is that the live run row still reports `state: planned`, the
leaderboard rows show `settledPayoutSats: 0`, and the evidence doc itself
names remaining seams: run state-transition route missing, operator-staged
admission, packaged Psionic sidecar gap, and no model-ladder network rungs.
That is enough for yellow bounded-run copy, not for broad Episode 236 launch
copy.

### Tassadar

The Tassadar folder and product promises are clearer than the transcript:
Tassadar is the compiled exact-executor lane; Psion is the learned lane. The
green promise `compute.tassadar_executor_poc.v1` proves a bounded exact
workload on a real Pylon with separate-device exact replay and one paid
Lightning closeout. The yellow promise `artanis.tassadar_evolution_loop.v1`
proves an automated spine, not a trained model.

Missing for Episode 236:

- a canonical model spec for the Percepta Executor Class direction;
- a Pylon v0.3 integration contract for CPU-computation transformation;
- a training or distillation plan tied to verified traces;
- public-safe artifact lineage;
- safety and performance boundaries;
- training/eval receipts.

### Forum

The Product Promises forum already contains a Raynor topic:

- `https://openagents.com/forum/t/7af48f9c-a592-4160-9cae-0ef47c247198#post-603fa555-ccea-42d2-a9ca-139996bc14bc`

It says registry `2026-06-11.9` added Episode 236 red rows and that
receipts are required before launch copy advances.

The Work Requests forum contains the Raynor coordination post:

- `https://openagents.com/forum/t/499cec6e-c09e-45a7-8c24-4bcee8fc87dc#post-defe4b78-e36f-41b8-80b4-b963813c942a`

That post asks external Pylon operators to help collect independent proof.
It is useful for labor-market proof, but it is not itself training-run
evidence.

The live work-request order book returned `workRequests: []` after the stale
closed-issue listing was expired. That is the correct safe state, but it
means the labor market currently has no active inventory for non-owner
providers to quote.

## What Still Needs To Be Built

### 1. Public Training Run Authority

Build a first-class run lifecycle route and projection so a run can move
from planned to active to sealed to reconciled/closed without operator D1
patches.

Minimum public-safe fields:

- run ref and promise ref;
- start/end state;
- participant admission rule;
- window refs;
- lease refs;
- workload ref;
- verifier policy refs;
- artifact and digest refs;
- payment mode;
- settlement state;
- generatedAt and staleness contract;
- typed blockers.

This closes the current mismatch where windows are reconciled but the run row
still says `planned`.

### 2. Settlement Projection Consistency

Training leaderboards and run pages need the same settlement truth. The A1
evidence doc records settled receipts, but leaderboard rows still report
`settledPayoutSats: 0`. Either the public projection must join the settlement
receipts, or the docs must state that route settlement totals are pending
until a specific projection update lands.

Do not collapse payment evidence, closeout evidence, settlement bridge
evidence, and spendable contributor balance.

### 3. Monday Launch Manifest

Before any Monday launch copy, create a launch manifest with:

- `trainingRunRef`;
- run objective;
- model/rung scope;
- dataset scope;
- max participants and admission policy;
- minimum useful work;
- validator policy;
- payout policy and spend cap;
- comparison methodology if any;
- live public status URL;
- abort and stale-state rules;
- promise IDs affected.

The manifest should be public-safe and should not include raw prompts,
wallet material, private hostnames, bearer tokens, invoices, payment hashes,
preimages, or local paths.

### 4. Participant Count Methodology

The largest-run comparison needs a count rule before it needs marketing
copy. Define whether participants are:

- registered Pylons;
- fresh online Pylons;
- admitted Pylons;
- assigned contributors;
- contributors with accepted useful work;
- contributors with settled payment receipts.

For a largest-run claim, only admitted contributors with accepted useful work
and public-safe receipt refs should count. Mere registrations or stale
heartbeats should not count.

### 5. Pylon v0.3 Stable Install Path

Pylon v0.3 is rc2 source. Stable broad copy needs:

- stable `0.3.0` release;
- published install artifact or documented source fallback;
- macOS and Linux install smokes;
- package checksum/release evidence;
- bootstrap, register, heartbeat, wallet readiness, and assignment-readiness
  smokes;
- documented failure modes.

Windows/WSL remains out of scope unless the owner reopens it.

### 6. Self-Serve Training Assignment Path

The A1 evidence was operator-staged. Episode 236 needs a contributor path:

- contributor installs Pylon;
- Pylon registers capability;
- run admits contributor;
- Pylon claims a lease;
- Pylon submits progress/artifact refs;
- validator checks work;
- requester/operator accepts;
- payment and settlement refs record.

The path can start with small-sats and a strict spend cap. It should not
reuse owner-operated Pylons as independent proof.

### 7. Tassadar/Percepta Model Boundary

The canonical promise is `models.tassadar_percepta_executor.v1`. Build the
record before building the hype:

- spelling and naming contract;
- model family and relationship to Psion;
- what "CPU computation transformation" means in Pylon terms;
- which work is compiled exact execution versus learned model behavior;
- safety, refusal, and performance boundaries;
- training/eval artifacts required for the first yellow transition.

Do not let the green `compute.tassadar_executor_poc.v1` proof leak into a
trained-model claim.

### 8. Verified Trace Factory And W3 Sweep

Episode 236 leans on the Tassadar/Percepta direction. The W3 issue is the
closest open research issue:

- `#4749`: W3 student program, four-baseline sweep on verified traces.

It is active but not closeable. It still needs baselines A/B/C to finish, the
four-baseline report, and H1/H2/H3 verdicts. The issue comments say the SHC
continuation run was active as of 2026-06-12 UTC, with A/B/C still running
under the CPU-budget guard.

### 9. Public Copy Gate

Before announcement copy:

- query `/api/public/product-promises`;
- use the live registry version;
- cite exact promise IDs;
- do not say "largest", "launched", "paying", "200 contributors", "stable
  v0.3", or "Tassadar model" unless the matching promise is green or the
  copy explicitly says planned/red/yellow with evidence limits.

## Open Issues And 236 Relevance

| Issue                                      | 236 relevance                                                                             | Recommendation                                                                                                                                                |
| ------------------------------------------ | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `#4749` W3 student program                 | Relevant to Tassadar/Psion research, not the Monday training-launch runtime path.         | Keep open. Do not use it as launch evidence until baselines A/B/C, report, and verdicts exist.                                                                |
| `#4768` M10 proof smoke                    | Autopilot MVP continuity proof, not required for Episode 236 training launch.             | Keep open for MVP, remove from any 236 blocker list unless the 236 launch explicitly depends on Autopilot overnight proof.                                    |
| `#4772` M14 MVP exit                       | Door-open gate for Autopilot MVP, not required for Episode 236 training launch.           | Keep open for MVP. Do not block training launch on it.                                                                                                        |
| `#4777` P1 first live negotiated labor job | Optional if 236 wants external labor-market proof; not required for training assignments. | Park under post-236 market proof unless the launch needs open-market provider receipts.                                                                       |
| `#4781` P5 backlog faucet                  | Optional labor-market inventory. Current live order book is empty.                        | Not needed for 236. Keep only as market roadmap or close/narrow if the board is being reduced to training launch.                                             |
| `#4782` P6 spare-capacity provider mode    | Adjacent to "one Pylon earns in multiple ways"; not needed to launch a training run.      | Keep as post-236 earning-mode proof. Do not block 236 on same-day owner/stranger job evidence.                                                                |
| `#4783` P7 Lane C fanout                   | Product-order market fanout, not needed for Episode 236.                                  | Best candidate to close or move out of the active 236 board if the goal is focus. It distracts from training launch proof.                                    |
| `#4786` Autopilot MVP epic                 | Parent for the Autopilot issue ladder, not the 236 training-launch parent.                | Keep if maintaining Autopilot roadmap. Do not use as the 236 parent. If a 236-only board is desired, open/maintain a separate training launch parent instead. |

The strongest close suggestion is `#4783` for the 236 context only. It may
remain valid roadmap work, but it should not sit in the launch-critical path.
The labor-market issues `#4777`, `#4781`, and `#4782` can also be moved to a
post-236 milestone unless external paid-provider proof is explicitly required
for the announcement.

## Product Promise Updates Made By This Audit

This audit updates source product promises as follows:

- Bumps public registry source version from `2026-06-11.9` to
  `2026-06-12.2`.
- Points `latestGapAuditUrl` to this audit.
- Updates Pylon v0.3 release-candidate copy from rc1 to rc2.
- Adds canonical `models.tassadar_percepta_executor.v1` as red.
- Withdraws `models.tasadar_percepta_executor.v1` as a typo alias.
- Fixes the transcript source-set wording from Tasadar to Tassadar.

No red/yellow training or earning promise is upgraded to green.

## Next Instructions

### For The Next Training Agent

1. Do not work on Lane C fanout for Episode 236.
2. Pull the latest W3 status for `#4749`.
3. If A/B/C runs finished, collect public-safe eval reports, write the
   four-baseline report, and record H1/H2/H3 verdicts.
4. If A/B/C are still running, do not restart them unless the CPU-budget
   policy says the active run failed.
5. Do not post raw logs, host paths, provider payloads, prompts, or secrets.

### For The Next Worker/API Agent

1. Build the training run state-transition route.
2. Fix public run and leaderboard settlement consistency.
3. Add tests that a reconciled window cannot leave the public run claiming
   only `planned` without a typed caveat.
4. Add tests that settled training receipts appear in the public training
   projection only when receipt refs are dereferenceable and redacted.

Suggested focused checks:

```sh
bun run --cwd apps/openagents.com/workers/api smoke:cs336-a1:no-spend
bun run --cwd apps/openagents.com/workers/api smoke:training-runs:public
bun run --cwd apps/openagents.com/workers/api smoke:tassadar:executor-trace
```

### For The Next Pylon Agent

1. Treat Pylon `0.3.0-rc2` as source RC, not stable release.
2. Work toward the stable `0.3.0` install/publish checklist in
   `apps/pylon/docs/2026-06-11-v030-release-preparation-record.md`.
3. Keep training, provider mode, Forum, and coding-agent earning modes
   separate in public evidence.
4. Do not claim multi-earning until each mode has work, payment, settlement,
   and projection refs.

### For Product/Forum Coordination

1. After the registry source deploys, post a Product Promises forum update
   linking this audit and registry version `2026-06-12.2`.
2. Use `models.tassadar_percepta_executor.v1` in all new Forum, issue, and
   docs references.
3. Keep the Work Requests call open for independent proof collection, but do
   not treat replies there as training launch proof unless they include
   accepted-work, validation, payment, and settlement refs tied to the
   training run.

## Safe Copy

Use:

> OpenAgents has a bounded two-device real-gradient training proof and a
> scoped Tassadar executor proof of concept. Episode 236's Monday large
> decentralized training launch, largest-run comparison, Pylon v0.3
> multi-earning node, and trained Tassadar/Percepta model claims remain gated
> until public run, participant, validation, payment, settlement, and
> projection receipts exist.

Do not use:

> We launched the largest decentralized training run.
>
> Install Pylon v0.3 and earn Bitcoin from training today.
>
> Pylon v0.3 stable ships multi-earning.
>
> Tassadar is trained or CPU-equivalent.
>
> The labor-market issues are blockers for the training launch.
