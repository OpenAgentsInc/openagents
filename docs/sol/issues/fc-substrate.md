# P0 SUBSTRATE: Fleet Command contracts for Desktop/mobile

- Issue: #8638
- Program parent: #8566
- Status: active P0 shared substrate consumed by the client tracks; not an epic
- Authority: [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md)

## Owner direction

The immediate product priority is reliable software for managing coding fleets
from Desktop and mobile. The landed Sarah Fleet route remains a compatibility
adapter and regression path; it is not the required front door or the new app
information architecture.

## Existing substrate to compose, not rebuild

- `FleetRun`, plan DAG, planner, work units, attempts, and one claim registry;
- `@openagentsinc/khala-fleet-intents` control/approval/steer vocabulary;
- named Codex/Claude worker/account custody and mixed-kind supervision;
- durable command outcomes, verification, usage/economics, and receipts;
- Khala Sync Fleet projections/mutators and reconnect behavior;
- Pylon standing execution, recovery, liveness, and exact closeout;
- the closed #8637/#8633/#8639 contract, executor, and supervision lanes.

Do not create a second Fleet schema, client-local claim universe, or transcript-
inferred outcome.

## Active consumers and bounded proof

1. #8640 — clean real simultaneous Codex+Claude runtime proof, then R3/R7
   Desktop/mobile acceptance. An existing compatibility adapter may initiate
   the runtime proof; that does not select the product front door.
2. #8574 — server-authoritative Desktop Fleet cockpit and typed controls.
3. #8597 — mobile supervision, attention, typed controls, outcomes, receipts,
   compact remote coding, and handoff.
4. #8547 — minimum real brokered Codex Agent Computer/workroom path required
   for mobile coding.
5. #8636 — explicit owner-local/managed-remote target routing under one claim/
   run/workroom contract.

Advanced provider breadth and elastic/cost-aware placement follow R7; the
minimum remote-workroom path does not.

Grok is not a current acceptance item while funded capacity is unavailable; its
adapter and historical canary remain regression evidence.

## Non-negotiable laws

- Named isolated account refs only; never an automatic default provider home.
- Owner-local subscription capacity serves only its owner and is not resold.
- Desktop/mobile present/request; typed services authorize and execute.
- One claim registry prevents duplicates across harnesses and targets.
- Harness/account/target fallback is typed and visible, never silent.
- Raw prompts, shell output, paths, credentials, and private repository content
  stay on the owning private plane.
- Exact usage where measured and explicit `not_measured` otherwise.
- Accepted/rejected/failed/unknown-pending-reconcile are distinct durable
  command states; transport timeout is not success.
- Every cross-session mutation follows
  [`../CLAIM_PROTOCOL.md`](../CLAIM_PROTOCOL.md).

## Substrate exit

1. #8640 produces one accepted real Codex+Claude runtime receipt with zero
   duplicate claims or silent substitution.
2. Desktop and mobile observe the same run/work/attempt/account/approval/
   command/outcome/receipt refs and versions through Khala Sync.
3. Controls from both clients converge to one durable outcome and survive lost
   acknowledgement, reconnect, restart, stale generation, and handoff.
4. R7 dogfood proves the direct software workflow and issue/docs/legacy paths
   reconcile.
5. #8547/#8636 complete the minimum real mobile remote-workroom path before R7;
   any advanced placement/provider residual carries an explicit follow-on
   disposition.

#8638 closes only after these product/runtime exits agree. Sarah/persona/A/V or
presentation quality is not an exit criterion.
