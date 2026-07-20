# AFS-06 connect advisory results to IDE action services

Date: 2026-07-20

Status: implementation record for work packet AFS-06. This record is evidence.
It is not release authority. It is not a product promise.

Audience: human and agent.

Authority: the plan
`docs/sol/2026-07-20-apple-fm-router-to-full-agent-system-plan.md` owns the
names, the bounds, and the rules. This record states what AFS-06 delivered.

## 1. What AFS-06 delivered

AFS-06 connects the shared turn kernel to the existing IDE action services. Every
action that starts from an inference result now goes through the current
proposal (IDE-08), task and run (IDE-10), debug (IDE-11), and source-control
(IDE-12) services. Apple FM stays advisory. It adds no action authority.

AFS-06 delivered these items:

1. A widened turn-kernel `ActionBroker` port that carries the host-owned intent
   and turn identity, not only the advisory candidate.
2. A Desktop turn-to-action broker that converts an advisory delivery into a
   typed action REQUEST for the owning IDE service and records the backlink.
3. A narrow `TurnActionSink` port and a default record-only sink. The real
   IDE-08/10/11/12 services plug into the same port.
4. Focused tests that prove an inference result never mints an action, a file
   change goes through an IDE-08 proposal, and the advisory to proposal to
   owner-accept path is typed end to end.

AFS-06 changes no product copy and starts no new run, debug step, or Git action.

## 2. The action invariant, enforced

Inference output is advisory. AFS-06 honors the plan's Action invariant with
structure, not with a runtime promise.

| Rule | Enforcement |
| --- | --- |
| A file mutation must become an IDE-08 proposal. | A proposal candidate carries the IDE-08-minted proposal ref. The broker records the backlink to that proposal. It never writes a file and never re-mints a proposal. |
| A task must use IDE-10. | An explain-failure answer becomes a read-only IDE-10 run-evidence request bound to the intent run ref. |
| Debug work must use IDE-11. | An explain-debug answer becomes a read-only IDE-11 debug-context request bound to the intent debug ref. |
| Source control must use IDE-12. | A commit-message draft becomes an IDE-12 draft field only. |
| Run, debug control, stage, commit, push, and delivery need a separate command. | The request union has no apply, accept, run, stage, commit, push, or deliver variant. |
| Model text must not become a command argument. | Routing is derived from the host-owned intent, never from model output. Model text lands only in an advisory instruction, explanation, or draft field. |
| An inference result cannot redirect the action class. | A candidate whose declared task class disagrees with the host intent is refused to advisory only. |

## 3. Files

| Path | Role |
| --- | --- |
| `packages/agent-turn-runtime/src/ports.ts` | Widens the `ActionBroker` delivery to carry the intent, thread ref, and request ref. |
| `packages/agent-turn-runtime/src/turn-service.ts` | Hands the broker the full delivery at the terminal step. The call stays fail-soft. |
| `apps/openagents-desktop/src/turn/turn-action-broker.ts` | The Desktop broker, the typed request union, the derive function, and the `TurnActionSink` port with its default record-only sink. |
| `apps/openagents-desktop/src/turn/turn-action-broker.test.ts` | The AFS-06 tests. |
| `apps/openagents-desktop/src/turn/desktop-turn-policy.ts` | Removes the AFS-01 no-op broker. |
| `apps/openagents-desktop/src/turn/desktop-turn-main.ts` | Composes the real Desktop broker in the first production composition. |

## 4. Coordination with the IDE-13 program

AFS-06 consumes the IDE-08, IDE-10, IDE-11, and IDE-12 services at their current
revisions. It does not rewrite or fork them. A real file change already goes
through IDE-08 before the candidate reaches the kernel: the cursor and IDE-08
service mint the durable proposal with exact preimages and hash and generation
checks, then return a proposal candidate that references the proposal by its
ref. AFS-06 records the backlink from the candidate to that IDE-08 proposal. The
narrow `TurnActionSink` port is the seam where the real IDE services attach to
submit the proposal, attach read-only run and debug context, or set the
source-control draft field, without any change to the AFS-06 routing logic.

## 5. Verification

AFS-06 passes these checks:

1. `pnpm --dir apps/openagents-desktop run typecheck` passes.
2. `pnpm --dir apps/openagents-desktop run check:ide-boundaries` passes.
3. `pnpm run check:afs-boundaries` passes.
4. The turn-action-broker tests pass.
5. The turn-kernel and Desktop turn composition tests pass.
