# P0 PROGRAM: reliable Effect Native Desktop/mobile coding and fleet software

- Issue: #8566
- Status: sole active program parent under Master Revision 31
- Authority: [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md)
- Execution:
  [`../2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md`](../2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md)
- Local coding cutover:
  [`../2026-07-11-openagents-coding-cutover-issue-plan.md`](../2026-07-11-openagents-coding-cutover-issue-plan.md)

## Owner direction

Desktop and mobile are the active product clients. OpenAgents web remains a
supported public/API/operations surface, but web expansion does not preempt the
reliability program. The Sarah product surface is removed. Any future assistant
must consume the same typed actions without becoming a new authority or front
door.

Both clients use one Effect Native application/component/intent grammar and
one Khala Sync identity/state/outcome reality:

```text
OpenAgents mobile <---- Khala Sync ----> OpenAgents Desktop
remote coding + fleet                    full workbench + fleet
```

## Program tracks and bounded work

- #8574 — Desktop track: D0–D6, practical OpenCode parity, Sync, Fleet,
  security, lifecycle, and distribution.
- #8597 — mobile track: R0/R1/R2/R3/R6, full useful Khala Code MVP
  capability fold-in, remote coding, fleet controls, receipts, and iOS/Android.
- #8547/#8636 — bounded P0 remote-workroom execution and target-routing tasks
  required by the mobile coding exit; prove #8547's real target before #8636's
  live hybrid-routing exit. Advanced elastic/provider breadth remains follow-on.
- closed #8638 — existing Fleet substrate consumed by the clients; it is not
  another epic.
- #8640 — bounded accepted simultaneous Codex+Claude runtime proof only.
- closed #8675 — real-Electron acceptance of the landed loss-accounted Codex
  trace workspace and its UX promise.
- #8676 — one real streamed Desktop conversation immediately continued on
  mobile through the same confirmed refs and request processor.
- #8677 — the bounded command/event fault-convergence matrix for that seam.
- reopened #8678 — initial Effect service-topology work landed; #8683/#8684
  own its source-coupling, ambient-authority, replaceability, lifecycle,
  correlation, and full-host residual.
- #8681–#8707 — ordered CUT-01–CUT-27 leaves for the installed local
  Codex/Claude coding cutover with physical mobile continuation/supervision.
- unfiled Revision 31 leaves — portable session authority, owner-managed and
  managed-provider targets, the general broker, and any-host mobile/voice.
  Local live multi-agent supervision is now #8691/#8692. Do not overload
  #8676/#8547/#8636 or CUT-27 with host-movement proof.

The retired web/presentation/privacy-backlog records #8595, #8610, #8634,
#8635, #8642, #8643, #8646, and #8650 are closed `wontfix` / not-planned.
Production defects and real privacy requests get a new bounded incident or task;
these backlog tombstones are not dormant epics.

## Shared implementation laws

- Effect Native owns typed components, state, intents, and command IDs;
  Electron/React Native/DOM/native/canvas are hosts or renderers.
- Khala Sync owns canonical cross-device projections and durable mutation
  outcomes. Device-local state is cache/offline queue, never a second authority.
- Pylon/Worker/Cloud/data/payment services retain their own typed authority.
- Desktop and mobile may expose different capabilities but share identifiers,
  versions, outcome grammar, privacy classes, and receipt refs.
- Every mutation carries owner scope, target ref, idempotency, version/
  commutative semantics, and durable accepted/rejected/failed/unknown outcome.
- Existing auth, promise, receipt, payment, release, and security contracts stay
  green through migration.
- Deprecated mobile and Electrobun clients are extraction sources, not
  conversion targets or parallel shipping products.
- New shared component/host demand goes upstream; app-local state/UI
  architectures do not regrow.
- Process, WorkContext, conversation/run, request/command, and foreign-host/
  view scopes are explicit. Wider scopes never capture narrower authority;
  ambient cwd, `AsyncLocalStorage`, renderer paths, and module singletons do not
  select runtime authority.
- Embedded, local, remote, mobile, and test adapters enter one canonical
  request processor. `ManagedRuntime`, Promise, Electron, provider, and native
  callback bridges remain named perimeter modules.
- Cancellation remains Effect interruption with owned finalization. A broad
  error adapter never converts interruption or a defect into ordinary tool
  success/failure.
- Every human-visible P0 milestone has a checked UX/behavior contract plus the
  real host/device smoke appropriate to its proof rung. Fixture green is not
  owner-visible acceptance.
- Pointer, tap, palette, menu, and conflict-safe hotkeys dispatch the same typed
  supervision intent. Public-safe acceptance evidence reuses the existing QA
  receipt machinery rather than creating a second broad QA program.
- Agent topology is durable session state: children stay attached to the root,
  open independent transcripts, repair the derived current projection from the
  authoritative durable log, and move under graph-wide attachment fencing.

## R0–R7 exit

1. Desktop and mobile are truthfully green from clean state.
2. Both use one authenticated owner/org/device/session contract.
3. Loss-accounted local Codex history remains predictable under its executable
   UX promise, without becoming Sync authority or exposing raw local data.
4. Conversation/project/agent-graph/Fleet/work/attempt/approval/command/outcome/
   receipt state converges through Khala Sync across restart, handoff, and
   graph-wide host movement without an orphaned source child.
5. One real Codex+Claude FleetRun completes through named isolated accounts;
   its later client observation/control remains owned by the client tracks.
6. Offline, lost-ack, out-of-order, stale generation, refetch, restart,
   migration, and rollback behavior fails closed and converges.
7. Desktop completes D0–D6 practical OpenCode parity; mobile completes R6
   compact remote coding plus fleet control through typed workroom capabilities
   without unsafe local filesystem/process/credential authority.
8. Signed/installable iOS, Android, and Desktop releases survive sustained
   owner dogfood with exact receipts.
9. Legacy product/install/release/update paths are deleted or unable to ship
   only after successor and migration proof.

#8566 closes last, after child issues, R7, issue/docs reconciliation, and legacy
retirement agree. Sarah/persona/A/V/presentation work is not an exit criterion.
