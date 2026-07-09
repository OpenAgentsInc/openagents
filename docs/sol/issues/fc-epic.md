# P0 EPIC: Sarah Fleet Command — parallel Codex, Claude, and Grok work now

## Owner direction

Sarah managing coding fleets is the immediate product priority. She must start,
observe, steer, approve, and close multiple coding streams across the owner's
Codex, Claude, and Grok accounts. Capacity may run on desktop Pylons or managed
Agent Computers, but cloud completion must not block the first useful local
fleet.

Authority: `docs/sol/MASTER_ROADMAP.md`.

## Existing substrate to compose, not rebuild

- `FleetRun`, plan DAGs, work planner, and claim registry.
- `@openagentsinc/khala-fleet-intents` for worker selection, control,
  approvals, and steer messages.
- Codex and Claude Pylon executors plus Grok ACP/worker executor.
- Mixed-kind supervisor fixture and harness conformance suite.
- Khala Sync fleet projections and steering mutators.
- Caller-owned Khala→Pylon assignments, exact token rows, raw/private event
  archives, and closeout proofs.
- Headless `pylon node`, assignment polling, account registry, and coordinator.

## Child lanes

1. #8637 FC-1 — Sarah fleet tool + authenticated durable run contract.
2. #8633 FC-2 — real mixed-harness Pylon supervisor and standing parallel
   executor.
3. #8639 FC-3 — Sarah canvas progress, controls, approvals, receipts, and
   reconnect.
4. #8636 FC-4 — hybrid local-Pylon + Agent Computer routing.
5. #8640 FC-5 — live multi-stream dogfood acceptance.

## Non-negotiable laws

- Named isolated account refs only; never the default `~/.codex` or equivalent
  provider home for automatic work.
- Owner-local subscription capacity serves only its owner and is never resold.
- Sarah proposes and presents; typed services authorize and execute.
- One claim registry prevents duplicate work across every harness and target.
- Harness and target fallback is typed and visible, never silent.
- Raw prompts, shell output, paths, credentials, and private repo contents stay
  on the owning private plane.
- Exact usage where measured, explicit `not_measured` otherwise, and closeout
  evidence remain completion truth; never synthesize usage or infer it from a
  public counter.

## Epic exit

From `/sarah`, the owner starts a pinned plan with at least three concurrent
real work streams. Codex, Claude, and Grok all complete useful units under one
run with zero duplicate claims. Sarah shows live progress, handles at least one
steer or approval, and renders verified closeouts. A local-only pass closes the
immediate unblock milestone; after #8547/#8636 integration, a second receipt
proves at least one owner-local unit and one managed Agent Computer unit both
complete useful verified work concurrently without changing the Sarah
interaction contract.

## Cutover posture

The first real Sarah canary begins only after #8637, #8633, and the minimum
safe #8639 progress/control/reconnect seam pass one fixture on a pinned
integrated deployment. The owner-local workflow becomes the default for new
bounded coding work when #8640 Phase A is clean. Managed cloud, avatar polish,
and full application conversion do not delay that cutover. This Codex app
remains available for break-glass, fleet-control-plane implementation, and
independent evidence review.
