# Fast Follow expansion — accepted plan and work-packet ledger

- Class: accepted plan and implementation admission
- Date: 2026-07-16
- Owner authority: current owner conversation
- FastFollowSpec: `FASTFOLLOW.md` revision 3
- Program: ordered `initial_program`
- Status: active — FULL HARVEST
- Base: `f2c5591e3b5a2c160f436fb62633a6367272c70d`

## Owner direction

> FULL HARVEST. The full Amp port is approved. No more hesitation: implement
> every material capability identified by the Amp Fast Follow gap analysis.

This direction is the separate target authority required by Fast Follow. It
admits the ordered five-day initial program as an active product-expansion
lane and approves the complete target-native port of the material Amp
capabilities identified by the gap analysis. The prior policy block is
superseded. The historical receipt remains truthful for its earlier target
revision; the gap assessment now carries the superseding owner decision so it
cannot be mistaken for current policy.

**Scope decision:** FULL HARVEST means full port. Supersession, revert and
acceptance relations; thread search, share, export and visibility; shared
Queue/Steer/Stop control semantics and receipts; disclosed routing and
specialists; review and thread reading; placement and remote control; and
generated clients and signed plugins are all approved outcomes. Bounded work
packets sequence implementation and verification; they do not make any of
these outcomes optional or defer them to an unspecified future decision.

The repository accepts this plan instead of creating a feature issue because
GitHub issues are reserved for concrete reproducible bugs. Each mutating unit
still requires a bounded packet, one root-coordinated or cross-session claim,
an isolated clean worktree, dependency reconciliation, and target-local proof.

## Program order

1. Day 1 — thread-fabric surfaces.
2. Day 2 — disclosed routing and specialists.
3. Day 3 — review and thread reader.
4. Day 4 — placement and remote control.
5. Day 5 — generated clients and signed plugins.

The order is sequential. Admission of the program avoids repeating an owner
ceremony for every day, but it does not authorize agents to implement an
unbounded day at once or collide on shared contracts.

## Active packet — FF-D1-01

Outcome: establish the provider-neutral Queue / Steer / Stop control algebra
and lower the existing Desktop Queue and Steer composer decisions into it.

Owned implementation paths:

- `packages/agent-runtime-schema/src/thread-control.ts`
- `packages/agent-runtime-schema/src/thread-control.test.ts`
- `packages/agent-runtime-schema/src/index.ts`
- `packages/agent-runtime-schema/README.md`
- `apps/openagents-desktop/src/composer-admission.ts`
- `apps/openagents-desktop/src/composer-admission.test.ts`
- `apps/openagents-desktop/src/renderer/shell.ts`
- `apps/openagents-desktop/src/renderer/shell.test.ts`
- `apps/openagents-desktop/package.json`
- `pnpm-lock.yaml`

Hot contracts: runtime-control schema literals, behavior-contract registry,
FastFollowSpec revision, ProductSpec revision, and Sol manifest.

Required behavior:

- semantic kinds are exactly `turn.queue`, `turn.steer`, and
  `turn.interrupt`;
- an adapter never silently turns steer into queue or queue into a new turn;
- stable intent, idempotency, thread, turn, message, generation, origin,
  ordering, and deadline identity is explicit;
- raw message bodies stay outside the shared control contract;
- admission, delivery, and terminal observation remain separate evidence;
- exact lost-ACK retries dedupe, while conflicting identity reuse fails
  closed; and
- current Desktop behavior remains available while Sync, Pylon, mobile, and
  other adapters await later packets.

Proof: focused schema tests/typecheck, focused Desktop composer/shell and local
runtime tests/typecheck, Fast Follow and behavior-contract checks, Sol document
checks, ProductSpec checks, and the repository-required `pnpm run check`.

Close rule: this packet is an implementation foundation, not Day 1 completion.
A rendered Desktop journey and real compatible-adapter outcomes are required
before claiming the Day 1 surface complete.

## Active packet — FF-D1-02

Outcome: migrate the foreground Desktop Stop path across both current ChatHost
adapters to the provider-neutral `turn.interrupt` envelope and return a typed
runtime-control outcome without changing the visible Stop behavior.

Owned implementation paths:

- `apps/openagents-desktop/src/composer-admission.ts`
- `apps/openagents-desktop/src/composer-admission.test.ts`
- `apps/openagents-desktop/src/renderer/shell.ts`
- `apps/openagents-desktop/src/renderer/shell.test.ts`
- `apps/openagents-desktop/src/renderer/local-harness.ts`
- `apps/openagents-desktop/src/renderer/local-harness.test.ts`
- `apps/openagents-desktop/src/renderer/runtime-conversation.ts`
- `apps/openagents-desktop/src/renderer/runtime-conversation.test.ts`
- `docs/fastfollow/receipts/2026-07-16-ff-d1-02-desktop-stop-control-receipt.md`
- `docs/sol/document-manifest.json`
- this accepted-plan ledger

Hot contracts: `openagents.runtime_control_intent.v2`,
`openagents.runtime_control_outcome.v1`, and the Desktop `ChatHost` interrupt
seam. This packet does not version those shared schema literals.

Required behavior:

- Desktop mints one ref-only `turn.interrupt` intent for the exact active
  thread and turn before either current ChatHost adapter signals Stop;
- the local and durable-conversation adapters validate exact thread/turn
  identity and never reroute interrupt into queue, steer, or a new turn;
- adapter admission/delivery acknowledgement is returned as
  `openagents.runtime_control_outcome.v1`, while terminal interruption remains
  an independently observed runtime event;
- missing, stale, or mismatched active state fails closed without dispatch;
  and
- the existing Stop control and provider-specific transport remain available.

Proof: focused composer, shell, local-harness, and runtime-conversation tests;
Desktop typecheck; Fast Follow, behavior-contract, ProductSpec, and Sol
document checks; and repository-required `pnpm run check`.

Close rule: this packet closes only foreground Desktop Stop lowering and typed
adapter acknowledgement. Durable cross-restart outcome storage, lost-ACK
replay reconciliation, Queue/Steer outcome persistence, Sync/mobile/Pylon
adapters, rendered evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-02-20260717`
- base: `fdc673ad54120ff14ff3483d13a9051a2469b258`
- worktree/branch: `openagents-ff-d1-02` / detached `origin/main`
- scope: foreground Desktop Stop canonical lowering and typed adapter outcome
- paths: the FF-D1-02 owned implementation paths above
- hot files: this accepted-plan ledger, Sol document manifest, and the Desktop ChatHost interrupt seam
- hot contracts: runtime-control schema literals and Desktop ChatHost interrupt return type
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T05:00:27Z`

### CLAIM-STATUS

- implementation: foreground local and durable-conversation ChatHost adapters lower exact active Stop targets into `turn.interrupt` and return typed control outcomes
- focused proof: 239 tests passed with 11 skipped across the packet and existing capability/boundary suites; Desktop typecheck passed
- authority proof: Fast Follow 7/7, behavior contracts 36/36, and ProductSpec 104/104 passed
- repository check: `pnpm run check` passed
- residual: durable outcome persistence/replay, Queue/Steer outcomes, additional adapters, and rendered evidence remain unclaimed later packets

### CLAIM-RELEASE

- landed: `621e900688a869992aaf017ea254e42af010bc13` on `main`
- verification: focused packet/capability/boundary tests 239 passed and 11 skipped; Desktop typecheck; Fast Follow 7/7; behavior contracts 36/36; ProductSpec 104/104; Sol checks 19/19; `pnpm run check`; enforced Desktop gate 1,908 passed and 39 skipped plus build, fixture visual smoke, React smoke, and built Electron smoke
- residual: durable outcome persistence/replay, Queue/Steer outcomes, Sync/mobile/Pylon adapters, thread search/share/export/supersession surfaces, real rendered runtime evidence, and Day 1 completion remain unclaimed

## FF-D1-03 — Foreground Queue/Steer control outcomes

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next ordered Day 1 residual after FF-D1-02. It lowers the
already-minted `turn.queue` and `turn.steer` envelopes through the foreground
Desktop ChatHost adapters and returns provider-neutral adapter acknowledgement.

Owned implementation paths:

- `apps/openagents-desktop/src/composer-admission.ts`
- `apps/openagents-desktop/src/renderer/shell.ts`
- `apps/openagents-desktop/src/renderer/shell.test.ts`
- `apps/openagents-desktop/src/renderer/local-harness.ts`
- `apps/openagents-desktop/src/renderer/local-harness.test.ts`
- `apps/openagents-desktop/src/renderer/runtime-conversation.ts`
- `apps/openagents-desktop/src/renderer/runtime-conversation.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-03-desktop-queue-steer-control-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.runtime_control_intent.v2`,
`openagents.runtime_control_outcome.v1`, and the Desktop `ChatHost` Queue/Steer
seams. This packet does not version those shared schema literals.

Required behavior:

- the shell passes the exact already-minted ref-only Queue or Steer envelope to
  the matching foreground adapter without translating between control kinds;
- local Queue/Steer and durable-conversation Queue validate exact active target
  identity and fail closed without dispatch on a missing or mismatched target;
- adapter admission/delivery acknowledgement is returned as
  `openagents.runtime_control_outcome.v1`; Queue identifies accepted queued
  delivery, while Steer identifies applied, unsupported, or failed delivery;
- lost acknowledgement remains pending for later reconciliation instead of
  being reported as success; and
- existing provider-specific Queue/Steer transport seams remain available.

Proof: focused composer, shell, local-harness, and runtime-conversation tests;
Desktop typecheck; Fast Follow, behavior-contract, ProductSpec, and Sol
document checks; and repository-required `pnpm run check`.

Close rule: this packet closes only foreground Desktop Queue/Steer lowering and
typed adapter acknowledgement. Durable cross-restart outcome storage, lost-ACK
replay reconciliation, Sync/mobile/Pylon adapters, rendered evidence, and Day
1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-03-20260717`
- base: `cf00d8e14f7af3f236412e94fd6105fa43277b88`
- worktree/branch: `openagents-ff-d1-03` / detached `origin/main`
- scope: foreground Desktop Queue/Steer canonical lowering and typed adapter outcomes
- paths: the FF-D1-03 owned implementation paths above
- hot files: this accepted-plan ledger, Sol document manifest, and Desktop ChatHost Queue/Steer seams
- hot contracts: runtime-control schema literals and Desktop ChatHost Queue/Steer return types
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T11:40:53Z`

### CLAIM-STATUS

- implementation: the shell and foreground local/durable adapters lower exact Queue/Steer controls and return typed admission/delivery acknowledgement
- focused proof: 244 tests passed with 11 skipped across packet and capability suites; Desktop typecheck passed
- authority proof: Fast Follow 7/7, behavior contracts 36/36, ProductSpec 104/104, and Sol checks 19/19 passed
- repository check: `pnpm run check` passed
- residual: durable outcome persistence/replay, Sync/mobile/Pylon adapters, rendered evidence, and Day 1 completion remain unclaimed later packets

### CLAIM-RELEASE

- landed: `09e17243209b40f246e7f1a08025ea91dfebf8d9` on `main`
- verification: focused packet/capability tests 244 passed and 11 skipped; Desktop typecheck; Fast Follow 7/7; behavior contracts 36/36; ProductSpec 104/104; Sol checks 19/19; `pnpm run check`; enforced Desktop gate 1,917 passed and 39 skipped plus build, fixture visual smoke, React smoke, and built Electron smoke
- residual: durable outcome persistence/replay, Sync/mobile/Pylon adapters, thread search/share/export/supersession surfaces, real rendered runtime evidence, and Day 1 completion remain unclaimed

## FF-D1-04 — Restart-stable Desktop control-outcome ledger

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next ordered Day 1 residual after FF-D1-03. It persists
provider-neutral Desktop Queue/Steer/Stop acknowledgement in the main process
so accepted, rejected, failed, unsupported, and pending outcomes survive a
renderer reload or Desktop restart.

Owned implementation paths:

- `apps/openagents-desktop/src/runtime-control-outcome-contract.ts`
- `apps/openagents-desktop/src/runtime-control-outcome-store.ts`
- `apps/openagents-desktop/src/runtime-control-outcome-store.test.ts`
- `apps/openagents-desktop/src/main.ts`
- `apps/openagents-desktop/src/preload.cts`
- `apps/openagents-desktop/src/renderer/boot.ts`
- `apps/openagents-desktop/src/renderer/shell.ts`
- `apps/openagents-desktop/src/renderer/shell.test.ts`
- `apps/openagents-desktop/src/renderer/runtime-conversation.ts`
- `apps/openagents-desktop/src/renderer/runtime-conversation.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-04-desktop-control-outcome-ledger-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.runtime_control_outcome.v1`, Desktop preload IPC,
the `ChatHost` control-outcome recorder, and the private Desktop user-data
boundary. This packet does not version the shared runtime-control schemas.

Required behavior:

- every non-null foreground Queue/Steer/Stop acknowledgement is schema-checked
  and recorded through a trusted renderer-to-main IPC boundary before the
  shell consumes its delivery state;
- the private main-process ledger uses bounded atomic persistence under
  Desktop user data and reconstructs identical outcomes after close/reopen;
- exact retries are idempotent, pending axes may advance monotonically, and
  conflicting terminal evidence fails closed instead of overwriting history;
- invalid, corrupt, or cross-identity records are rejected without inventing
  evidence; and
- persistence never stores raw message bodies, provider credentials, or
  terminal runtime events that have not independently been observed.

Proof: focused store, shell, preload-boundary, and runtime-conversation tests;
Desktop typecheck; Fast Follow, behavior-contract, ProductSpec, and Sol
document checks; and repository-required `pnpm run check`.

Close rule: this packet closes only local cross-restart outcome persistence.
Lost-ACK replay/reconciliation, Sync/mobile/Pylon adapters, rendered evidence,
and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-04-20260717`
- base: `c25bb5410d228df56d275c4b0f41023123c8cad2`
- worktree/branch: `openagents-ff-d1-04` / detached `origin/main`
- scope: restart-stable provider-neutral Desktop control-outcome ledger
- paths: the FF-D1-04 owned implementation paths above
- hot files: this accepted-plan ledger, Sol manifest, Desktop main/preload/boot/shell boundaries, and converging ChatHost seam
- hot contracts: runtime-control outcome schema literal, trusted IPC channel, and private outcome-ledger format
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T11:53:37Z`

### CLAIM-STATUS

- implementation: Desktop records schema-checked Queue/Steer/Stop outcomes through preload/main into a bounded private atomic ledger and reconstructs them after reopen
- convergence: exact retry is idempotent; pending evidence advances monotonically; cross-identity and conflicting terminal evidence fail closed
- focused proof: 164 tests passed with 11 skipped across the store, shell, and converging host; Desktop typecheck passed
- authority proof: Fast Follow 7/7, behavior contracts 36/36, ProductSpec 104/104, and repository check passed
- landed: `7a5066e2db` on `main`; enforced Desktop gate passed 1,934 tests with 39 skipped plus production build, compatibility smoke, React smoke, and repeated built Electron smoke
- residual: lost-ACK replay/reconciliation, Sync/mobile/Pylon adapters, rendered evidence, and Day 1 completion remain unclaimed later packets

## FF-D1-05 — Durable Queue/Steer acknowledgement replay

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next ordered Day 1 residual after FF-D1-04. It lets an
exact Queue or Steer retry consult the restart-stable Desktop outcome ledger
before transport dispatch, replaying retained acknowledgement without sending
the control twice.

Owned implementation paths:

- `apps/openagents-desktop/src/runtime-control-outcome-contract.ts`
- `apps/openagents-desktop/src/runtime-control-outcome-store.ts`
- `apps/openagents-desktop/src/runtime-control-outcome-store.test.ts`
- `apps/openagents-desktop/src/main.ts`
- `apps/openagents-desktop/src/preload.cts`
- `apps/openagents-desktop/src/renderer/boot.ts`
- `apps/openagents-desktop/src/renderer/shell.ts`
- `apps/openagents-desktop/src/renderer/shell.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-05-desktop-control-outcome-replay-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.runtime_control_outcome.v1`, the private Desktop
outcome-ledger format, trusted preload IPC, and the Desktop `ChatHost`
Queue/Steer retry seam. Shared runtime-control schemas remain unchanged.

Required behavior:

- an exact Queue/Steer retry looks up `{threadRef, intentRef, idempotencyKey}`
  before calling any adapter transport;
- a retained outcome is replayed into existing shell draft/queue semantics
  without redispatch, including after ledger close/reopen;
- retained pending acknowledgement remains pending and does not authorize a
  duplicate dispatch or a successful UI transition;
- a confirmed missing identity dispatches normally, while corrupt, invalid,
  or conflicting reconciliation fails closed and retains the draft; and
- lookup returns only schema-checked ref/status evidence, never raw message
  bodies, provider credentials, or invented terminal observation.

Proof: focused store, shell, and preload-boundary tests; Desktop typecheck;
Fast Follow, behavior-contract, ProductSpec, and Sol document checks; and
repository-required `pnpm run check`.

Close rule: this packet closes only exact Queue/Steer acknowledgement replay.
Stop retry identity, remote Sync/mobile/Pylon adapters, rendered evidence, and
Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-05-20260717`
- base: `28517a9777d6538b5832561eb1d2b666cba6cc08`
- worktree/branch: `openagents-ff-d1-05` / detached `origin/main`
- scope: exact durable Queue/Steer acknowledgement lookup and replay without redispatch
- paths: the FF-D1-05 owned implementation paths above
- hot files: this accepted-plan ledger, Sol manifest, Desktop outcome store/IPC, boot, and shell retry seams
- hot contracts: outcome identity tuple, lookup result, and no-duplicate-dispatch behavior
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T12:47:16Z`

### CLAIM-STATUS

- implementation: exact Queue/Steer retry identity looks up restart-stable acknowledgement before transport and replays retained outcomes without redispatch
- fail-closed proof: pending, corrupt, unavailable, and conflicting reconciliation retain the draft and cannot authorize duplicate transport or success
- focused proof: 171 tests passed with 11 skipped across store, boundary, shell, and converging-host suites; Desktop typecheck passed
- authority proof: Fast Follow policy/spec 20/20, behavior contracts 36/36, ProductSpec 104/104, and Sol checks 19/19 passed
- repository check: `pnpm run check` passed
- residual: Stop retry identity, Sync/mobile/Pylon adapters, rendered evidence, thread search/share/export/supersession, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- landed: `aa6b9f79eeb47dfe061fd9ecf5f0a347e16b10bf` on `main`
- enforced Desktop proof: 1,940 tests passed with 39 skipped, followed by production build, compatibility Electron smoke, and React Electron smoke
- retry note: the hook's redundant second Desktop run intermittently exceeded the existing 50 ms large-rollout benchmark at 59 ms; that exact benchmark passed separately, and no gate or benchmark was changed
- residual: Stop retry identity, Sync/mobile/Pylon adapters, rendered evidence, thread search/share/export/supersession, and Day 1 completion remain unclaimed

## FF-D1-06 — Durable Stop acknowledgement replay

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next ordered Day 1 residual after FF-D1-05. It gives the
active Desktop Stop target a stable retry identity and consults the
restart-stable control-outcome ledger before adapter transport so a repeated
Stop after a lost acknowledgement cannot signal the same turn twice.

Owned implementation paths:

- `apps/openagents-desktop/src/renderer/shell.ts`
- `apps/openagents-desktop/src/renderer/shell.test.ts`
- `apps/openagents-desktop/src/renderer/local-harness.ts`
- `apps/openagents-desktop/src/renderer/local-harness.test.ts`
- `apps/openagents-desktop/src/renderer/runtime-conversation.ts`
- `apps/openagents-desktop/src/renderer/runtime-conversation.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-06-desktop-stop-outcome-replay-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.runtime_control_intent.v2`,
`openagents.runtime_control_outcome.v1`, the Desktop `ChatHost` Stop identity
seam, and the private Desktop outcome ledger. Shared runtime-control schemas
remain unchanged.

Required behavior:

- a local or durable-conversation Stop target derives one stable intent and
  idempotency identity from the exact active thread/turn;
- before interrupt transport, the shell looks up that exact identity in the
  restart-stable outcome ledger;
- a retained outcome, including pending acknowledgement, suppresses duplicate
  transport and leaves terminal UI state to observed runtime evidence;
- a confirmed missing identity dispatches normally and records the typed
  acknowledgement; corrupt, invalid, conflicting, or unavailable
  reconciliation fails closed without signalling transport; and
- no Stop retry path invents terminal interruption, raw message content, or a
  broader remote-control capability.

Proof: focused shell, local-harness, durable-conversation, and outcome-ledger
tests; Desktop typecheck; Fast Follow, behavior-contract, ProductSpec, Sol,
and repository-required checks.

Close rule: this packet closes only exact local Desktop Stop acknowledgement
replay. Sync/mobile/Pylon adapters, rendered evidence, thread
search/share/export/supersession, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-06-20260717`
- base: `0ccc8bc5a1a2b1b719b4d2dfdaf799c9d5ad72bc`
- worktree/branch: `openagents-ff-d1-06` / detached `origin/main`
- scope: stable exact Stop retry identity and durable acknowledgement replay without redispatch
- paths: the FF-D1-06 owned implementation paths above
- hot files: accepted-plan ledger, Sol manifest, Desktop shell and local/durable Stop adapters
- hot contracts: Stop identity tuple, lookup-before-interrupt ordering, and no-duplicate-signal behavior
- dependencies: FF-D1-04 ledger and FF-D1-05 lookup path landed; no open Fast Follow issue or competing claim found
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T13:10:00Z`

### CLAIM-STATUS

- implementation: local-harness and durable-conversation Stop targets derive stable exact thread/turn identities, and the shell reconciles that identity before interrupt transport
- fail-closed proof: retained pending acknowledgement suppresses redispatch; corrupt, conflicting, invalid, or unavailable ledger lookup cannot authorize transport or terminal UI state
- focused proof: 193 tests passed with 11 skipped across shell, local harness, durable conversation, and outcome-ledger suites; Desktop typecheck passed
- authority proof: Fast Follow policy/spec 20/20, behavior contracts 36/36, ProductSpec 104/104, and Sol checks 19/19 passed
- residual: Sync/mobile/Pylon adapters, rendered evidence, thread search/share/export/supersession, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- landed: `d26025e18596918144cc815ee03c798f7b01fc28` on `main`
- enforced Desktop proof: both repository gate passes completed with 1,942 tests passed and 39 skipped, production builds, compatibility Electron smokes, and the React Electron smoke
- residual: Sync/mobile/Pylon adapters, rendered evidence, thread search/share/export/supersession, and Day 1 completion remain unclaimed

## FF-D1-07 — Accepted-event authority relations

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-06 and the
separately landed mobile/Sync controller work. It establishes a provider-neutral,
ref-only algebra for accepted, superseded, and reverted thread-event authority
without introducing a second transcript store or treating summaries as evidence.

Owned implementation paths:

- `packages/agent-runtime-schema/src/thread-event-authority.ts`
- `packages/agent-runtime-schema/src/thread-event-authority.test.ts`
- `packages/agent-runtime-schema/src/index.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-07-thread-event-authority-receipt.md`
- this accepted-plan ledger, `docs/sol/receipts/README.md`, and `docs/sol/document-manifest.json`

Hot contracts: the new `openagents.thread_event_authority.v1` relation schema,
exact thread/event refs, append-only observed relation order, and conflict-failing
authority projection. Existing runtime-control, portable-session, and provider
event schemas remain unchanged.

Required behavior:

- accepted, superseded, and reverted are distinct typed relation states over
  exact thread/event refs;
- supersession names the accepted event that replaced the original, while a
  revert names both the accepted revert event and the exact restored event;
- self-reference, cross-thread projection, malformed refs/timestamps, and
  duplicate or conflicting relation evidence fail closed;
- projection is deterministic over append-only relations and returns explicit
  missing, resolved, or conflict state without using transcript text; and
- the contract grants no mutation, share, export, visibility, provider,
  acceptance, or release authority by itself.

Proof: focused schema/projection tests and package typecheck; Fast Follow,
behavior-contract, ProductSpec, Sol, and repository-required checks.

Close rule: this packet closes only the shared accepted-event authority
relation algebra. Desktop consumption, share/export visibility, remaining
adapters, rendered runtime evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-07-20260717`
- base: `0cfd9334c3`
- worktree/branch: `openagents-ff-d1-07` / detached `origin/main`
- scope: provider-neutral accepted/superseded/reverted thread-event authority relations and deterministic conflict-failing projection
- paths: the FF-D1-07 owned implementation paths above
- hot files: agent-runtime-schema index and new relation module; accepted-plan ledger and Sol manifest/index
- hot contracts: exact relation refs, no-self-reference, same-thread projection, and fail-closed conflict behavior
- dependencies: FF-D1-01 through FF-D1-06 released; later mobile/Sync landings reconciled; their two unindexed Sol receipts receive additive index rows only
- verification: focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T13:35:00Z`

### CLAIM-STATUS

- implementation: added the ref-only accepted/superseded/reverted relation union and deterministic missing/resolved/conflict projector in the shared agent-runtime schema
- fail-closed proof: malformed, self-referential, cross-thread, duplicate, ambiguously ordered, and invalid-transition evidence cannot resolve authority
- focused proof: 6/6 relation tests and the agent-runtime-schema typecheck passed
- authority proof: Fast Follow policy/spec 20/20, behavior contracts 36/36, ProductSpec 104/104, Sol checks 19/19, and the repository check/fast guard passed
- residual: Desktop consumption, share/export visibility, remaining adapters, rendered runtime evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- landed: `5c80c4098e85c075883d880d5c41c1cc152000b7` on `main`
- remote proof: the fetched remote implementation tree exactly matched the fully checked local tree
- residual: Desktop consumption, share/export visibility, remaining adapters, rendered runtime evidence, and Day 1 completion remain unclaimed

## FF-D1-08 — Thread disclosure and export receipts

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-07. Active
Desktop work currently owns history, shell, runtime-conversation, and rendered
surfaces, so this slice establishes the shared provider-neutral disclosure and
export command/receipt algebra without touching those active seams.

Owned implementation paths:

- `packages/agent-runtime-schema/src/thread-disclosure.ts`
- `packages/agent-runtime-schema/src/thread-disclosure.test.ts`
- `packages/agent-runtime-schema/src/index.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-08-thread-disclosure-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: new `openagents.thread_disclosure_intent.v1` and
`openagents.thread_disclosure_receipt.v1` schemas, exact thread/intent/receipt
identity, explicit audience and administrator-access states, and ref-only
export evidence. Existing event visibility, runtime-control, portable-session,
provider-event, and Desktop contracts remain unchanged.

Required behavior:

- thread visibility changes and thread exports are distinct typed intents with
  stable retry identity and an explicit expected visibility-version state;
- audiences are explicit owner-only, workspace-member, named-group, or
  internet-readable states; there is no `unlisted` state or implication;
- administrator access is an independent explicit axis rather than being
  inferred from audience visibility;
- accepted-pending, rejected, failed, visibility-applied, and export-created
  receipts remain distinct, and an applied receipt must match its intent kind;
- export receipts bind exact artifact ref, SHA-256 digest, format, and artifact
  audience without embedding transcript or exported content; and
- malformed refs/timestamps/digests, self-inconsistent states, raw content,
  exact retries, and conflicting identity reuse are handled deterministically
  and fail closed.

Proof: focused schema/decoder/retry tests and package typecheck; Fast Follow,
behavior-contract, ProductSpec, Sol, and repository-required checks.

Close rule: this packet closes only the shared disclosure/export intent and
receipt algebra. Desktop command consumption and pixels, persistence/transport,
real exported artifacts, remaining adapters, rendered runtime evidence, and
Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-08-20260717`
- base: `36fff3452dc5e533e9dce40b6838dbfe526e9e11`
- worktree/branch: `openagents-ff-d1-08` / detached `origin/main`
- scope: provider-neutral explicit thread visibility/export intents, receipts, and retry classification
- paths: the FF-D1-08 owned implementation paths above
- hot files: agent-runtime-schema index; accepted-plan ledger and Sol manifest
- hot contracts: new disclosure intent/receipt schema literals, audience/admin axes, receipt-kind consistency, and ref-only artifact evidence
- dependencies: FF-D1-07 released; current Sync/mobile attention projection, target resolution, and inbox landings reconciled; active Desktop history/shell/rendering work explicitly excluded
- verification: focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T13:59:34Z`
- rebased_before_implementation: `2026-07-17T14:01:00Z`

### CLAIM-STATUS

- implementation: added distinct ref-only thread visibility/export intents, explicit audience/admin policy, typed receipt outcomes, and retry classification in the shared agent-runtime schema
- fail-closed proof: `unlisted`, malformed/raw evidence, cross-workspace audience/admin policy, wrong-kind applied receipts, invalid digests, and conflicting retry identity cannot produce accepted evidence
- focused proof: 8/8 thread-disclosure tests and the agent-runtime-schema typecheck passed
- authority proof: Fast Follow policy/spec 20/20, behavior contracts 36/36, ProductSpec 104/104, Sol checks 19/19, and the repository check/fast guard passed
- residual: Desktop command consumption and pixels, persistence/transport, real exported artifacts, remaining adapters, rendered runtime evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- landed: `9582538eb287cb77f1c50ad50c69965ef65d839c` on `main`
- remote proof: the fetched remote implementation tree exactly matched the fully checked local tree
- residual: Desktop command consumption and pixels, persistence/transport, real exported artifacts, remaining adapters, rendered runtime evidence, and Day 1 completion remain unclaimed

## FF-D1-09 — Canonical thread event export artifact

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-08. Active
Desktop history, shell, renderer, update, and release work remains excluded.
The packet compiles one real owner-only canonical event bundle from bounded
thread-event payloads and the accepted-event authority relations established by
FF-D1-07, without introducing a second transcript store or granting persistence,
transport, disclosure, or broader audience authority.

Owned implementation paths:

- `packages/agent-runtime-schema/src/thread-export-artifact.ts`
- `packages/agent-runtime-schema/src/thread-export-artifact.test.ts`
- `packages/agent-runtime-schema/src/index.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-09-canonical-thread-export-artifact-receipt.md`
- this accepted-plan ledger, `docs/sol/receipts/README.md`, and `docs/sol/document-manifest.json`

Hot contracts: `openagents.thread_disclosure_intent.v1`,
`openagents.thread_event_authority.v1`, and the new deterministic
`openagents.thread_export_artifact.v1` owner-only canonical event bundle.
Existing runtime-control, provider-event, Sync, and Desktop contracts remain
unchanged.

Required behavior:

- only an exact decoded `thread.export.create` intent with
  `canonical_event_bundle` format and `owner_only` audience can compile;
- every bounded event names the exact intent thread, a unique stable event ref,
  a unique non-negative sequence, and JSON data, then resolves through exact
  accepted-event relation evidence;
- missing, invalid, cross-thread, duplicate, ambiguous, or conflicting authority
  evidence fails closed rather than silently omitting or promoting an event;
- canonical recursive key ordering plus sequence/event-ref ordering produces
  deterministic UTF-8 bytes and a caller-supplied SHA-256 implementation must
  return a valid digest for those exact bytes; and
- the artifact includes explicit accepted/superseded/reverted authority but
  grants no persistence, transport, visibility mutation, audience expansion,
  provider acceptance, or release authority.

Proof: focused schema/compiler tests and package typecheck; Fast Follow,
behavior-contract, ProductSpec, Sol, and repository-required checks.

Close rule: this packet closes only deterministic owner-only canonical event
bundle generation. Persistence/transport, Desktop command consumption and
pixels, broader disclosure adapters, remaining runtime/rendered evidence, and
Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-09-20260717`
- base: `b2649ab74bf101136145db0b0f651bceefb699ca`
- worktree/branch: `openagents-ff-d1-09` / detached `origin/main`
- scope: deterministic owner-only canonical event bundle bytes and digest over exact event-authority evidence
- paths: the FF-D1-09 owned implementation paths above
- hot files: agent-runtime-schema index and new export compiler; accepted-plan ledger and Sol manifest/index
- hot contracts: canonical JSON/event ordering, exact thread binding, authority conflict failure, owner-only audience, and digest-of-exact-bytes behavior
- dependencies: FF-D1-08 released; current mobile/Sync lifecycle landings reconciled; active Desktop history/shell/rendering work explicitly excluded
- verification: focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T14:21:36Z`

### CLAIM-STATUS

- implementation: added a deterministic owner-only canonical event bundle compiler over actual bounded JSON event data and exact accepted/superseded/reverted authority
- fail-closed proof: broader audiences, other formats, non-export intents, malformed/non-JSON data, cross-thread events, duplicate identity/order, unresolved/conflicting authority, oversize artifacts, and invalid digests cannot produce an artifact
- focused proof: all 64 agent-runtime-schema tests passed, including 19 export/disclosure/authority tests; package typecheck passed
- authority proof: Fast Follow policy/spec 20/20, behavior contracts 36/36, ProductSpec 104/104, Sol checks 19/19, and `pnpm run check` passed
- publication correction: the claim commit's truncated Sol manifest transfer was caught by the deterministic gate and repaired on remote `main` at `6cfee8e382decc85529ef81a1b7423fc69ede955` before implementation publication
- residual: artifact persistence/transport, Desktop command consumption and pixels, broader disclosure adapters, remaining runtime/rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- landed: `b310c2dd1e6c43822bbb1a1886f66c8a7a23a37d` on `main`
- remote proof: the fetched remote implementation tree exactly matched the fully checked local tree
- residual: artifact persistence/transport, Desktop command consumption and pixels, broader disclosure adapters, remaining runtime/rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-10 — Private atomic canonical export store

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-09. Active
Desktop history, shell, renderer, main-process integration, update, and release
files remain excluded. The packet adds a new-file-only Desktop main-process
store that persists and reloads FF-D1-09 owner-only canonical event bundles
without granting renderer access, save-dialog transport, broader disclosure,
or command authority.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-artifact-store.ts`
- `apps/openagents-desktop/src/thread-export-artifact-store.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-10-desktop-thread-export-store-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.thread_disclosure_intent.v1`,
`openagents.thread_disclosure_receipt.v1`,
`openagents.thread_export_artifact.v1`, Desktop private `userData` storage,
and exact SHA-256 identity. Existing Desktop IPC, command, shell, renderer,
Sync, provider, and visibility contracts remain unchanged.

Required behavior:

- only an exact decoded owner-only `thread.export.create` intent and matching
  FF-D1-09 compilation can enter the store;
- the store verifies UTF-8 bytes, canonical encoded equality, artifact schema,
  intent/thread/format/audience identity, byte bound, and SHA-256 before write;
- artifacts are addressed only by their SHA-256, written atomically beneath a
  private mode-0700 directory as mode-0600 files, and exact retry is idempotent;
- reopening can load only an exact artifact ref/digest pair and revalidates
  stored bytes, digest, schema, and owner-only audience before returning bytes
  inside the main-process module;
- corruption, mismatched identity, invalid receipt metadata, persistence
  failure, and conflicting existing bytes fail closed without overwrite; and
- successful persistence returns a schema-valid ref-only export-created
  receipt, never a path, transcript projection, broader audience grant,
  renderer capability, provider verdict, or release claim.

Proof: focused store/reopen/corruption/privacy tests; Desktop typecheck; Fast
Follow, behavior-contract, ProductSpec, Sol, and repository-required checks.

Close rule: this packet closes only private Desktop main-process persistence
and verified reload of owner-only canonical export artifacts. IPC/command
wiring, save-dialog or remote transport, Desktop pixels, broader disclosure
adapters, remaining runtime/rendered evidence, and Day 1 completion remain
later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-10-20260717`
- base: `6ea8e81b0b057fad3d4d080b310cfc1756e2bc57`
- worktree/branch: `openagents-ff-d1-10` / detached `origin/main`
- scope: private atomic Desktop persistence and verified reload for exact owner-only canonical export bytes
- paths: the FF-D1-10 owned implementation paths above
- hot files: new Desktop store/test files; accepted-plan ledger and Sol manifest
- hot contracts: exact bytes/digest identity, owner-only artifact audience, atomic private storage, ref-only receipt, and corruption refusal
- dependencies: FF-D1-09 released; current mobile/Sync lifecycle work reconciled; active Desktop integration/rendering/update/release work explicitly excluded
- verification: focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T14:40:11Z`

### CLAIM-STATUS

- implementation: added a new-file-only Desktop main-process store that validates, atomically persists, and verified-reloads exact FF-D1-09 owner-only canonical export bytes
- privacy proof: digest-addressed private mode-0700/mode-0600 storage, no-overwrite corruption refusal, idempotent retry, and a typed ref-only export-created receipt
- focused proof: all 5 store/reopen/corruption/privacy tests passed; the new store passes isolated strict TypeScript compilation
- authority proof: Fast Follow policy/spec 20/20, behavior contracts 36/36, ProductSpec 104/104, Sol checks 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: the current Desktop package typecheck fails only in pre-existing lifecycle-schema consumers under active work; targeted AssuranceSpec is 189/190 with an unrelated environment-digest snapshot drift; neither boundary was changed or weakened
- residual: IPC/command wiring, save-dialog or remote transport, Desktop pixels, broader disclosure adapters, remaining runtime/rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- landed: `8e99d168395efcbba030c2ea132295fd208dc949` on `main`
- remote proof: the fetched remote implementation tree exactly matched the fully checked local tree
- residual: IPC/command wiring, save-dialog or remote transport, Desktop pixels, broader disclosure adapters, remaining runtime/rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-11 — Desktop canonical export command coordinator

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-10. Active
Desktop main-process integration, history, shell, renderer, update, and release
files remain excluded. The packet adds a new-file-only main-process command
coordinator that consumes one exact owner-only export intent, obtains canonical
event payloads and authority relations from a host-owned evidence source,
compiles them through FF-D1-09, and persists them through FF-D1-10.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-command.ts`
- `apps/openagents-desktop/src/thread-export-command.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-11-desktop-thread-export-command-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.thread_disclosure_intent.v1`,
`openagents.thread_event_authority.v1`,
`openagents.thread_export_artifact.v1`, the FF-D1-10 private store, and the
host-owned canonical evidence-source seam. Existing Desktop IPC, main, preload,
shell, renderer, Sync, provider, picker, and transport contracts remain
unchanged.

Required behavior:

- the public command input is only one unknown intent; callers cannot inject
  event payloads, authority relations, receipt refs, timestamps, or digests;
- only a decoded `thread.export.create` intent with
  `canonical_event_bundle` format and `owner_only` audience reaches the
  evidence source;
- the host-owned source is queried for the exact decoded thread and returns a
  bounded evidence snapshot whose thread identity must match;
- compilation consumes actual event data plus explicit
  accepted/superseded/reverted relations and preserves all FF-D1-09 fail-closed
  behavior;
- host-owned receipt identity, clock, and SHA-256 functions feed FF-D1-10, so a
  successful command returns only its typed ref-only stored/unchanged receipt;
  and
- unavailable/throwing evidence, invalid authority, mismatched identity,
  invalid host metadata, or persistence refusal returns a bounded typed reason
  without raw evidence, paths, errors, or partial success.

Proof: focused command/store integration tests; isolated command TypeScript
compile; Fast Follow, behavior-contract, ProductSpec, Sol, and
repository-required checks. The known current-main Desktop lifecycle typecheck
and AssuranceSpec environment-digest snapshot failures remain baseline
collisions unless separately resolved on `main`.

Close rule: this packet closes only host-owned main-process command
coordination from exact export intent through canonical evidence compilation to
private persistence. IPC/preload wiring, save-dialog or remote transport,
Desktop pixels, broader disclosure adapters, remaining runtime/rendered
evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-11-20260717`
- base: `1db8203518c3ec26a6c30770678f0391b4f7117f`
- worktree/branch: `openagents-ff-d1-11` / detached `origin/main`
- scope: host-owned Desktop export command coordination over exact canonical event-authority evidence and private artifact persistence
- paths: the FF-D1-11 owned implementation paths above
- hot files: new Desktop command/test files; accepted-plan ledger and Sol manifest
- hot contracts: intent-only caller boundary, exact evidence thread identity, canonical authority compilation, host-owned receipt identity/clock/digest, and ref-only result
- dependencies: FF-D1-10 released; no relevant open issue or competing claim; active Desktop integration/rendering/update/release work explicitly excluded
- verification: focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T15:01:00Z`

### CLAIM-STATUS

- implementation: added a new-file-only Desktop main-process coordinator that accepts only an export intent, reads exact bounded canonical evidence from a host-owned source, compiles through FF-D1-09, and persists through FF-D1-10
- authority/privacy proof: broader or malformed intents never read evidence; callers cannot inject events, relations, digests, clocks, or receipt refs; results never contain raw evidence, paths, or exception text
- focused proof: all 10 command/store integration tests passed; the new command passes isolated strict TypeScript compilation
- authority proof: Fast Follow policy/spec 20/20, behavior contracts 36/36, ProductSpec 104/104, Sol checks 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: the Desktop package typecheck retains only the pre-existing lifecycle-schema failures recorded by FF-D1-10; the known AssuranceSpec environment-digest snapshot drift remains outside this packet
- residual: IPC/preload wiring, save-dialog or remote transport, Desktop pixels, broader disclosure adapters, remaining runtime/rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- landed: `9453fe057a29a78eb37e42798236321c03ff21bd` on `main`
- remote proof: the fetched remote implementation tree exactly matched the fully checked local tree
- residual: IPC/preload wiring, save-dialog or remote transport, Desktop pixels, broader disclosure adapters, remaining runtime/rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-12 — Owner-selected canonical export file transport

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-11. Active
Desktop main-process integration, history, shell, renderer, update, and release
files remain excluded. The packet adds a new-file-only main-process transport
that consumes an exact FF-D1-11 ref-only export receipt, obtains a destination
only from a host-owned picker seam, verified-loads FF-D1-10 bytes, and writes
the owner-selected JSON file without projecting its path.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-file-transport.ts`
- `apps/openagents-desktop/src/thread-export-file-transport.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-12-desktop-thread-export-file-transport-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.thread_disclosure_receipt.v1`, the FF-D1-10 private
artifact load boundary, owner-selected absolute destination authority, atomic
same-directory file replacement, and ref-only transport outcomes. Existing
Desktop IPC, main, preload, shell, renderer, Sync, provider, and disclosure
audience contracts remain unchanged.

Required behavior:

- only a decoded owner-only `canonical_event_bundle` `export_created` receipt
  can request transport; malformed, broader, other-format, pending, failed, or
  visibility receipts fail before destination selection;
- the transport proposes only a bounded sanitized `.json` filename and the
  host-owned selector returns cancel or an absolute selected path plus explicit
  replace authority;
- the transport loads only the receipt's exact artifact ref/digest pair through
  FF-D1-10 and never accepts bytes or a path from the caller;
- bytes are staged mode-0600 in the selected directory and published atomically;
  no-replace uses an exclusive same-directory link so a race cannot overwrite,
  while replacement occurs only when the selector explicitly confirms it;
- missing/corrupt artifacts, selector failure, invalid destinations, existing
  targets without replace authority, and write failure return bounded typed
  reasons with temporary files removed; and
- success returns only artifact ref/digest and replacement fact, never the path,
  bytes, raw evidence, provider verdict, broader disclosure, or release claim.

Proof: focused transport/store tests; isolated transport TypeScript compile;
Fast Follow, behavior-contract, ProductSpec, Sol, and repository-required
checks. The known current-main Desktop lifecycle typecheck and AssuranceSpec
environment-digest snapshot failures remain baseline collisions unless
separately resolved on `main`.

Close rule: this packet closes only an owner-selected local JSON file transport
after canonical export creation and private persistence. IPC/preload wiring,
Desktop pixels, broader disclosure adapters, remaining runtime/rendered
evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-12-20260717`
- base: `fe8963884b6aba8a7c1a6ebdc1818c7a13302051`
- worktree/branch: `openagents-ff-d1-12` / detached `origin/main`
- scope: owner-selected local atomic JSON transport for exact privately stored canonical export artifacts
- paths: the FF-D1-12 owned implementation paths above
- hot files: new Desktop transport/test files; accepted-plan ledger and Sol manifest
- hot contracts: exact ref/digest load, host-only absolute destination, explicit replacement authority, atomic publication, cleanup, and path-free result
- dependencies: FF-D1-11 released; no relevant open issue or competing claim; active Desktop integration/rendering/update/release work explicitly excluded
- verification: focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T15:10:19Z`

### CLAIM-STATUS

- implementation: added a new-file-only main-process transport that decodes an exact ref-only export receipt, verified-loads FF-D1-10 bytes, obtains a host-selected destination, and atomically publishes the owner-local JSON file
- privacy/authority proof: callers supply neither bytes nor paths; broader/malformed receipts fail before load/picker; no-replace is exclusive and replace requires explicit selector authority; results contain no path, bytes, or native error
- focused proof: all 16 transport/command/store tests passed; the new transport passes isolated strict TypeScript compilation
- authority proof: Fast Follow policy/spec 20/20, behavior contracts 36/36, ProductSpec 104/104, Sol checks 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: the Desktop package typecheck retains only the pre-existing lifecycle-schema failures recorded by FF-D1-10/11; the known AssuranceSpec environment-digest snapshot drift remains outside this packet
- residual: IPC/preload wiring, Desktop pixels, broader disclosure adapters, remaining runtime/rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- landed: `dc9e62769160d115520bbde0f22af2a148401694` on `main`
- remote proof: the fetched remote implementation tree exactly matched the fully checked local tree
- residual: IPC/preload wiring, Desktop pixels, broader disclosure adapters, remaining runtime/rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-13 — Desktop canonical-export preload boundary

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-12. Active
work still owns Desktop `main.ts`, history, shell, renderer, update, and release
surfaces. The packet therefore adds a fixed typed canonical-export IPC contract
and exposes its ref-only request/result through the sandboxed preload bridge
without registering the colliding main-process handler or rendering pixels.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-bridge-contract.ts`
- `apps/openagents-desktop/src/thread-export-bridge-contract.test.ts`
- `apps/openagents-desktop/src/preload.cts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-13-desktop-thread-export-preload-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.thread_disclosure_receipt.v1`, the fixed Desktop
canonical-export channel, the sandboxed preload allowlist, and the FF-D1-12
path-free transport result. Existing Desktop main-process registration,
renderer, history, shell, Sync, provider, and disclosure-audience contracts
remain unchanged.

Required behavior:

- the bridge accepts only an exact owner-only `canonical_event_bundle`
  `export_created` receipt and rejects malformed, raw-content-bearing, broader,
  other-format, pending, failed, or visibility receipts before IPC invocation;
- the preload exposes one fixed method and channel, never raw `ipcRenderer`, a
  caller-selected channel, destination path, artifact bytes, filesystem,
  process, or provider authority;
- successful, cancelled, and bounded rejected transport outcomes decode
  explicitly, while malformed/native failures collapse to a typed unavailable
  reason without leaking native errors; and
- the renderer-visible result remains ref-only and contains no destination
  path, exported bytes, raw evidence, broader disclosure, or release claim.

Proof: focused contract/preload tests; isolated TypeScript compilation where
the package baseline permits it; Fast Follow, behavior-contract, ProductSpec,
Sol, and repository-required checks. Known current-main Desktop lifecycle
typecheck and AssuranceSpec environment-digest snapshot failures remain
baseline collisions unless separately resolved on `main`.

Close rule: this packet closes only the sandboxed preload boundary for an exact
owner-only canonical export receipt. Main-process handler registration,
renderer command/pixels, broader disclosure adapters, installed/runtime-rendered
evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-13-20260717`
- base: `ffefa153e866b5a0fb1af5b6f2411edb3bb85a27`
- worktree/branch: `openagents-ff-d1-13` / detached `origin/main`
- scope: fixed schema-decoded ref-only canonical-export preload boundary
- paths: the FF-D1-13 owned implementation paths above
- hot files: sandboxed `preload.cts`, new bridge contract/test, accepted-plan ledger, and Sol manifest
- hot contracts: exact owner-only export receipt admission, fixed IPC channel, path-free bounded result, and no raw host authority
- dependencies: FF-D1-12 released; no relevant feature issue or competing claim; active Desktop main/history/shell/renderer/update/release work explicitly excluded
- verification: focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T15:32:21Z`

### CLAIM-STATUS

- implementation: added the fixed canonical-export channel contract and exposed one schema-decoded ref-only `threadExports.write` method through sandboxed preload
- privacy/authority proof: only an exact owner-only canonical-event export receipt invokes the fixed channel; exact result decoding rejects path/error leakage; native failures collapse to a bounded reason
- focused proof: all 22 bridge/command/store tests passed; the bridge passes isolated strict TypeScript compilation; the production Desktop build contains exactly one fixed channel and bridge exposure
- authority proof: Fast Follow policy/spec 20/20, behavior contracts 36/36, focused ProductSpec 104/104, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: the Desktop package typecheck retains only the pre-existing lifecycle-schema failures recorded by FF-D1-10 through FF-D1-12; the known AssuranceSpec environment-digest snapshot drift remains outside this packet
- residual: main-process handler registration, renderer command/pixels, broader disclosure adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- landed: `8fd449fa49e02989d0157fd91e3df6af3ce9d67b` on `main`
- remote proof: the fetched remote implementation tree exactly matched the fully checked local tree
- residual: main-process handler registration, renderer command/pixels, broader disclosure adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-14 — Canonical-export main-process handler seam

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-13. Active
work still owns Desktop `main.ts`, history, shell, renderer, update, and release
surfaces. The packet therefore adds a new-file-only main-process registration
seam that binds FF-D1-13's fixed decoded channel to FF-D1-12's path-free file
transport while leaving actual `main.ts` composition and pixels untouched.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-main-handler.ts`
- `apps/openagents-desktop/src/thread-export-main-handler.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-14-desktop-thread-export-main-handler-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: the fixed `openagents:thread-export:write` channel, exact
FF-D1-13 request/result decoders, trusted Desktop sender authority, handler
registration lifecycle, and FF-D1-12's path-free transport result. Existing
Desktop `main.ts`, preload, renderer, history, shell, Sync, provider, and
disclosure-audience contracts remain unchanged.

Required behavior:

- registration accepts exactly the fixed channel and returns one idempotent
  host-owned cleanup for the registered handler;
- an untrusted sender or malformed/broader/non-canonical request fails before
  transport invocation with the bounded `invalid_request` result;
- a trusted exact request passes only its decoded ref-only receipt to the
  transport, never renderer-supplied bytes, channel, destination, filesystem,
  process, or provider authority;
- valid cancelled, written, and rejected transport outcomes pass through the
  shared decoder, while thrown or malformed outcomes collapse to
  `transport_unavailable`; and
- no result exposes a destination path, artifact bytes, native error, raw
  evidence, broader disclosure, deployment, release, or acceptance claim.

Proof: focused handler/bridge/command/store tests; isolated TypeScript compile;
Fast Follow, behavior-contract, ProductSpec, Sol, and repository-required
checks. Known current-main Desktop lifecycle typecheck and AssuranceSpec
environment-digest snapshot failures remain baseline collisions unless
separately resolved on `main`.

Close rule: this packet closes only the tested main-process handler and
registration lifecycle seam. Actual `main.ts` composition, renderer
command/pixels, broader disclosure adapters, installed/runtime-rendered
evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-14-20260717`
- base: `88c24ec985469ef4a434a96ade59f074bf40bb4e`
- worktree/branch: `openagents-ff-d1-14` / detached `origin/main`
- scope: fixed trusted-sender canonical-export main-process handler registration seam
- paths: the FF-D1-14 owned implementation paths above
- hot files: new Desktop handler/test; accepted-plan ledger and Sol manifest
- hot contracts: fixed channel, exact request/result decoding, trusted sender gate, handler cleanup, and path-free transport delegation
- dependencies: FF-D1-13 released; no relevant feature issue or competing claim; active Desktop main/history/shell/renderer/update/release work explicitly excluded
- verification: focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T16:00:04Z`

### CLAIM-STATUS

- implementation: added a new-file-only fixed-channel handler registration seam with an idempotent cleanup and trusted-sender gate
- privacy/authority proof: exact request decoding occurs before transport; only the decoded ref-only receipt crosses the transport boundary; exact result decoding rejects path/error leakage
- focused proof: all 27 handler/bridge/export tests passed and the handler passes isolated strict TypeScript compilation
- authority proof: Fast Follow policy/spec 20/20, behavior contracts 36/36, focused ProductSpec 104/104, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: the Desktop package typecheck retains only the pre-existing lifecycle-schema failures recorded by FF-D1-10 through FF-D1-13; the known AssuranceSpec environment-digest snapshot drift remains 189/190 and outside this packet
- residual: actual `main.ts` composition, renderer command/pixels, broader disclosure adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- landed: `73b8c9101f9a53f607ec042186b01d5918f00093` on `main`
- remote proof: the fetched remote implementation tree exactly matched the fully checked local tree
- residual: actual `main.ts` composition, renderer command/pixels, broader disclosure adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-15 — Canonical-export creation preload boundary

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-14. Active
work still owns Desktop `main.ts`, history, shell, renderer, update, and release
surfaces. The current sandboxed renderer bridge can write an already-created
export receipt but cannot submit the bounded owner-only export intent that
creates that receipt. This packet therefore adds one fixed creation channel
and exposes its exact intent/result boundary through preload without
registering a colliding main-process handler or rendering pixels.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-create-bridge-contract.ts`
- `apps/openagents-desktop/src/thread-export-create-bridge-contract.test.ts`
- `apps/openagents-desktop/src/preload.cts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-15-desktop-thread-export-create-preload-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.thread_disclosure_intent.v1`,
`openagents.thread_disclosure_receipt.v1`, the fixed Desktop canonical-export
creation channel, the sandboxed preload allowlist, and FF-D1-11's bounded
command result. Existing Desktop main-process registration, renderer, history,
shell, Sync, provider, file transport, and disclosure-audience contracts remain
unchanged.

Required behavior:

- the bridge accepts only an exact owner-only `canonical_event_bundle`
  `thread.export.create` intent and rejects malformed, raw-content-bearing,
  broader, other-format, visibility, or envelope-smuggled requests before IPC;
- the preload exposes one fixed `threadExports.create` method and channel,
  never raw `ipcRenderer`, caller-selected events, authority relations, receipt
  metadata, artifact bytes, destination paths, filesystem, process, or provider
  authority;
- exact stored/unchanged command outcomes return only their decoded canonical
  export receipt, while bounded rejected outcomes retain only their typed
  reason; malformed or native failures collapse to `command_unavailable`;
- result receipts must match the request intent, idempotency key, thread,
  export kind, format, and owner-only audience; and
- no result exposes canonical event payloads, authority relations, paths,
  bytes, native errors, broader disclosure, deployment, release, or acceptance
  claims.

Proof: focused create-bridge/preload plus command/store tests; isolated
TypeScript compilation and production preload build where the package baseline
permits it; Fast Follow, behavior-contract, ProductSpec, Sol, and
repository-required checks. Known current-main Desktop lifecycle typecheck and
AssuranceSpec environment-digest snapshot failures remain baseline collisions
unless separately resolved on `main`.

Close rule: this packet closes only the sandboxed renderer-to-main creation
request/result boundary. Main-process creation-handler registration, actual
`main.ts` composition, renderer create-then-write command/pixels, broader
disclosure adapters, installed/runtime-rendered evidence, and Day 1 completion
remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-15-20260717`
- base: `f64af79791424f1819655cdfa63e40e7297689b1`
- worktree/branch: `openagents-ff-d1-15` / detached `origin/main`
- scope: fixed schema-decoded owner-only canonical-export creation preload boundary
- paths: the FF-D1-15 owned implementation paths above
- hot files: sandboxed `preload.cts`, new creation bridge/test, accepted-plan ledger, and Sol manifest
- hot contracts: exact owner-only export intent admission, fixed creation IPC channel, identity-bound ref-only receipt result, and no raw host authority
- dependencies: FF-D1-14 released; no relevant feature issue or competing claim; active Desktop main/history/shell/renderer/update/release work explicitly excluded
- verification: focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T16:20:53Z`

### CLAIM-STATUS

- implementation: added a fixed canonical-export creation channel and exposed one exact owner-only intent/result method through sandboxed preload
- privacy/authority proof: malformed, broader, raw-content-bearing, and envelope-smuggled requests fail before IPC; exact result decoding binds the ref-only receipt to the requested intent and rejects path/error leakage
- focused proof: all 21 create-bridge/bridge/command tests passed; the production contract passes isolated strict TypeScript compilation; the Desktop production build contains the fixed creation channel and method
- authority proof: Fast Follow policy/spec 20/20, behavior contracts 36/36, focused ProductSpec 104/104, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: the Desktop package typecheck retains only the pre-existing lifecycle-schema failures recorded by FF-D1-10 through FF-D1-14; the known AssuranceSpec environment-digest snapshot drift remains 189/190 and outside this packet
- residual: main-process creation-handler registration, actual `main.ts` composition, renderer create-then-write command/pixels, broader disclosure adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- landed: `5c84a0a69001a76ba62593129dbd2b50c2e33c01` on `main`
- remote proof: after reconciling a disjoint Sync landing, the fetched remote implementation tree exactly matched the fully checked local tree
- residual: main-process creation-handler registration, actual `main.ts` composition, renderer create-then-write command/pixels, broader disclosure adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-16 — Canonical-export creation main-process handler seam

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-15. Active
work still owns Desktop `main.ts`, history, shell, renderer, update, and release
surfaces. The packet therefore adds a new-file-only main-process registration
seam that binds FF-D1-15's fixed decoded creation channel to FF-D1-11's
canonical export command while leaving actual Electron composition and pixels
untouched.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-create-main-handler.ts`
- `apps/openagents-desktop/src/thread-export-create-main-handler.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-16-desktop-thread-export-create-main-handler-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: the fixed `openagents:thread-export:create` channel, exact
FF-D1-15 request/result decoders, trusted Desktop sender authority, handler
registration lifecycle, and FF-D1-11's intent-only command coordinator.
Existing Desktop `main.ts`, preload, renderer, history, shell, Sync, provider,
file transport, and disclosure-audience contracts remain unchanged.

Required behavior:

- registration accepts exactly the fixed creation channel and returns one
  idempotent host-owned cleanup for the registered handler;
- a closed handler, untrusted sender, throwing trust check, or malformed,
  broader, non-canonical request fails before command invocation with the
  bounded `invalid_request` result;
- a trusted exact request passes only its decoded export intent to the command,
  never renderer-supplied events, authority relations, receipt metadata,
  digests, paths, bytes, filesystem, process, or provider authority;
- valid stored, unchanged, and rejected command outcomes pass through the
  identity-bound FF-D1-15 decoder, while thrown or malformed outcomes collapse
  to `command_unavailable`; and
- no result exposes canonical event payloads, authority relations, destination
  paths, artifact bytes, native errors, broader disclosure, deployment,
  release, or acceptance claims.

Proof: focused creation-handler/bridge/command/store tests; isolated TypeScript
compile; Fast Follow, behavior-contract, ProductSpec, Sol, and
repository-required checks. Known current-main Desktop lifecycle typecheck and
AssuranceSpec environment-digest snapshot failures remain baseline collisions
unless separately resolved on `main`.

Close rule: this packet closes only the tested creation handler and registration
lifecycle seam. Actual `main.ts` composition, renderer create-then-write
command/pixels, broader disclosure adapters, installed/runtime-rendered
evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-16-20260717`
- base: `337d01b44b405097b1f2747f845af5d94bc71a61`
- worktree/branch: `openagents-ff-d1-16` / detached `origin/main`
- scope: fixed trusted-sender canonical-export creation main-process handler registration seam
- paths: the FF-D1-16 owned implementation paths above
- hot files: new Desktop creation handler/test; accepted-plan ledger and Sol manifest
- hot contracts: fixed creation channel, exact request/result decoding, trusted sender gate, handler cleanup, and intent-only command delegation
- dependencies: FF-D1-15 released; no relevant feature issue or competing claim; active Desktop main/history/shell/renderer/update/release work explicitly excluded
- verification: focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T16:40:09Z`

### CLAIM-STATUS

- implementation: added a new-file-only fixed creation-channel handler registration seam with an idempotent cleanup and trusted-sender gate
- privacy/authority proof: exact request decoding occurs before command execution; only the decoded owner-only export intent crosses the command boundary; identity-bound result decoding rejects mismatched receipt, path, and native-error leakage
- focused proof: all 25 creation-handler/bridge/command tests passed and the handler passes isolated strict TypeScript compilation
- authority proof: Fast Follow policy/spec 20/20, behavior contracts 36/36, focused ProductSpec 104/104, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: the Desktop package typecheck retains only the pre-existing lifecycle-schema failures recorded by FF-D1-10 through FF-D1-15; the known AssuranceSpec environment-digest snapshot drift remains 189/190 and outside this packet
- residual: actual `main.ts` composition, renderer create-then-write command/pixels, broader disclosure adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `9ca9bedfe7800879791bf9104d23319536372f39`
- implementation: `08ead2f43729debe2a135cc3a4a5dba9c5f7d145`
- disposition: `bounded_packet_landed`
- released_at: `2026-07-17T16:47:00Z`
- residual: actual `main.ts` composition, renderer create-then-write command/pixels, broader disclosure adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-17 — Desktop canonical-export create-then-write workflow

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-16. Active
work still owns Desktop `main.ts` and broad renderer surfaces. The packet
therefore adds a new-file-only, renderer-safe Effect workflow that composes the
already-landed sandboxed create and write methods while leaving Electron
composition and pixels untouched.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-workflow.ts`
- `apps/openagents-desktop/src/thread-export-workflow.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-17-desktop-thread-export-workflow-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-15's exact owner-only creation request/result, FF-D1-12's
bounded write request/result, create-before-write ordering, artifact identity,
and renderer-safe outcome projection. Existing `main.ts`, preload, renderer,
history, shell, Sync, provider, file transport, and disclosure-audience
contracts remain unchanged.

Required behavior:

- malformed or broader creation input fails before either host operation;
- creation rejection stops the workflow without invoking write, and thrown or
  malformed creation outcomes collapse to bounded `command_unavailable`;
- stored or unchanged creation passes only the decoded canonical receipt into
  write, never caller-selected events, paths, bytes, filesystem, process, or
  provider authority;
- cancellation and bounded write rejection remain distinct, while thrown or
  malformed write outcomes collapse to `transport_unavailable`;
- a purported written outcome must match the created receipt's exact artifact
  ref and digest or fail closed; and
- the workflow result exposes no receipt, event payload, authority relation,
  path, bytes, native error, broader disclosure, deployment, release, or
  acceptance claim.

Proof: focused workflow/create/write tests; isolated TypeScript compile; Fast
Follow, behavior-contract, ProductSpec, Sol, and repository-required checks.
Known current-main Desktop lifecycle typecheck and AssuranceSpec
environment-digest snapshot failures remain baseline collisions unless
separately resolved on `main`.

Close rule: this packet closes only the tested renderer-safe create-then-write
workflow. Actual `main.ts` composition, renderer command/pixels, broader
disclosure adapters, installed/runtime-rendered evidence, and Day 1 completion
remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-17-20260717`
- base: `600fcb58e653878b35de1fe505590c5e572c9daf`
- worktree/branch: `openagents-ff-d1-17` / detached `origin/main`
- scope: renderer-safe owner-only canonical-export create-then-write Effect workflow
- paths: the FF-D1-17 owned implementation paths above
- hot files: new Desktop workflow/test; accepted-plan ledger and Sol manifest
- hot contracts: exact create-before-write ordering, receipt-only delegation, artifact identity binding, and bounded renderer-safe results
- dependencies: FF-D1-16 released; no relevant feature issue or competing claim; active Desktop `main.ts` and renderer work explicitly excluded
- verification: focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T17:02:41Z`

### CLAIM-STATUS

- implementation: added a renderer-safe Effect workflow that validates one exact owner-only export request, creates before writing, and exposes only bounded outcome identity
- privacy/authority proof: only the decoded receipt crosses from create to write; thrown, malformed, path-leaking, and native-error outcomes collapse; written artifact identity must match the created receipt
- focused proof: all 37 workflow/export-chain tests passed and the production workflow passes isolated strict TypeScript compilation
- authority proof: Fast Follow policy/spec 20/20, behavior contracts 36/36, and focused ProductSpec 104/104 passed; repository checks are recorded in the packet receipt
- reconciled baseline: disjoint landing `ea99862e52` repaired the prior Desktop lifecycle-schema drift and the rebased package typecheck passes; the known AssuranceSpec environment-digest snapshot drift remains 189/190 and outside this packet
- residual: actual `main.ts` composition, renderer command/pixels, broader disclosure adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `9b3e56b3ba372d036a0d4caa97999d71aad31f07`
- implementation: `9954d78ca6d7685a3340d5e84a06db961f661193`
- disposition: `bounded_packet_landed`
- released_at: `2026-07-17T17:14:00Z`
- residual: actual `main.ts` composition, renderer command/pixels, broader disclosure adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-18 — Desktop canonical-export main composition resource

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-17. Active
work still owns Desktop `main.ts` and broad renderer surfaces. The packet
therefore adds a new-file-only Effect resource that atomically composes the
already-landed create and write handler registrations while leaving actual
Electron boot composition and pixels untouched.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-main-composition.ts`
- `apps/openagents-desktop/src/thread-export-main-composition.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-18-desktop-thread-export-main-composition-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-14's fixed write handler, FF-D1-16's fixed create handler,
trusted-sender reuse, exact channel ownership, atomic acquisition rollback, and
idempotent reverse-order cleanup. Existing `main.ts`, preload, renderer,
history, shell, Sync, provider, file transport, workflow, and
disclosure-audience contracts remain unchanged.

Required behavior:

- composition registers the fixed write and create handlers exactly once using
  one shared trusted-sender decision and the already-bounded operations;
- successful acquisition returns only one idempotent close resource and no raw
  handler, IPC, filesystem, process, provider, path, receipt, or event authority;
- close unregisters both handlers in reverse acquisition order and does not
  repeat either cleanup;
- if write registration fails, acquisition returns a typed bounded `write`
  failure without attempting create;
- if create registration fails, the already-acquired write handler is closed
  exactly once before acquisition returns a typed bounded `create` failure; and
- registration or cleanup failures never project native messages, paths, or
  stack details.

Proof: focused composition and existing handler/bridge tests; isolated and
Desktop TypeScript checks; Fast Follow, behavior-contract, ProductSpec, Sol,
and repository-required checks. The known AssuranceSpec environment-digest
snapshot failure remains a baseline collision unless separately resolved on
`main`.

Close rule: this packet closes only tested atomic composition of the two main
handler resources. The actual `main.ts` call site, renderer command/pixels,
broader disclosure adapters, installed/runtime-rendered evidence, and Day 1
completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-18-20260717`
- base: `dcfac105bbce524f87bdfa194d113fc535b40cce`
- worktree/branch: `openagents-ff-d1-18` / detached `origin/main`
- scope: atomic main-process composition resource for fixed canonical-export create/write handlers
- paths: the FF-D1-18 owned implementation paths above
- hot files: new Desktop main-composition resource/test; accepted-plan ledger and Sol manifest
- hot contracts: exact dual-handler registration, shared trust gate, typed acquisition failure, rollback, and idempotent reverse cleanup
- dependencies: FF-D1-17 released; no relevant feature issue or competing claim; active Desktop `main.ts` and renderer work explicitly excluded
- verification: focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T17:31:05Z`

### CLAIM-STATUS

- implementation: added an Effect acquisition resource that atomically composes the fixed write/create handler registrations and exposes only one idempotent close
- privacy/authority proof: registration failures are typed only by bounded stage; native details stay contained; create failure rolls back write; cleanup remains reverse-order and non-projecting
- focused proof: all 26 composition/handler tests passed; isolated strict compilation and the full Desktop package typecheck passed
- authority proof: behavior contracts 36/36 and focused ProductSpec 104/104 passed; Fast Follow, Sol, and repository checks are recorded in the packet receipt
- baseline collision: the known AssuranceSpec environment-digest snapshot drift remains 189/190 and outside this packet
- residual: actual `main.ts` call-site composition, renderer command/pixels, broader disclosure adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `a3ff7ef774d9358a8ce6cb345d35eb2c9dd8e081`
- implementation: `a98da6698e3a3fc74fefdb8a4f73fcf40a27425e`
- disposition: `bounded_packet_landed`
- released_at: `2026-07-17T17:37:38Z`
- residual: actual `main.ts` call-site composition, renderer command/pixels, broader disclosure adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-19 — Restart-stable Desktop thread-visibility policy store

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-18. Active
work still owns Desktop `main.ts` and broad renderer surfaces. The packet
therefore adds a new-file-only private main-process store that applies the
already-landed explicit visibility intent with optimistic version checks and
restart-stable receipts, without publishing thread content or rendering pixels.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-visibility-policy-store.ts`
- `apps/openagents-desktop/src/thread-visibility-policy-store.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-19-desktop-thread-visibility-policy-store-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.thread_disclosure_intent.v1`,
`openagents.thread_disclosure_receipt.v1`, explicit audience and administrator
access axes, optimistic visibility versioning, replay identity, and the private
Desktop user-data boundary. Existing shared schemas, `main.ts`, preload,
renderer, export, Sync, provider, publication, and transport contracts remain
unchanged.

Required behavior:

- the store accepts only a decoded ref-only `thread.visibility.set` intent and
  rejects malformed, raw-content-bearing, export, or contradictory policy
  input before persistence;
- first observation accepts either an explicit version-zero expectation or the
  bounded `not_observed` expectation, while subsequent mutation requires the
  exact current version and advances it monotonically by one;
- exact intent/idempotency replay returns the identical stored receipt without
  advancing version, while conflicting identity reuse and stale expectations
  fail closed;
- policy and receipt state persist atomically under a private Desktop directory,
  reconstruct identically after reopen, and reject corrupt or cross-thread
  records without overwriting them; and
- applying policy records intent and receipt evidence only: it grants no content
  publication, membership, administrator, network, provider, deployment,
  release, or acceptance authority.

Proof: focused visibility-store and shared disclosure tests; Desktop TypeScript
check; Fast Follow, behavior-contract, ProductSpec, Sol, AssuranceSpec baseline,
and repository-required checks.

Close rule: this packet closes only private restart-stable visibility policy
application and receipt evidence. Main-process composition, preload/renderer
commands and pixels, actual audience authorization/publication adapters,
installed/runtime-rendered evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-19-20260717`
- base: `a86ffc74992137c8ddafacfa201fa37dd6665dbd`
- worktree/branch: `openagents-ff-d1-19` / detached `origin/main`
- scope: restart-stable private Desktop thread-visibility policy application and receipt evidence
- paths: the FF-D1-19 owned implementation paths above
- hot files: new Desktop visibility store/test; accepted-plan ledger and Sol manifest
- hot contracts: existing disclosure intent/receipt literals, explicit audience/admin axes, optimistic version, replay identity, and private persistence
- dependencies: FF-D1-18 released; no relevant feature issue or competing claim; active Desktop `main.ts` and renderer work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T17:42:11Z`

### CLAIM-STATUS

- implementation: added a private restart-stable Effect visibility-policy store with exact optimistic versioning and ref-only applied receipts
- privacy/authority proof: decoded intent and receipt evidence only; no thread content, membership, administrator, publication, network, provider, path, or renderer authority is granted
- focused proof: all 14 visibility/disclosure tests passed and the full Desktop package typecheck passed
- authority proof: Fast Follow policy/spec 20/20, behavior contracts 36/36, focused ProductSpec 104/104, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collision: the known AssuranceSpec environment-digest snapshot drift remains 189/190 and outside this packet
- residual: `main.ts` composition, preload/renderer command and pixels, actual audience authorization/publication adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `9587300f6299c61a210686e6bc83c2fcfc006b86`
- implementation: `780887ff5fe74dafb857caaf482c8a36f580b9a9`
- disposition: `bounded_packet_landed`
- released_at: `2026-07-17T17:56:47Z`
- residual: `main.ts` composition, preload/renderer command and pixels, actual audience authorization/publication adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-20 — Desktop thread-visibility apply preload boundary

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-19. Active
work still owns Desktop `main.ts` and broad renderer surfaces, while the
sandboxed preload and new visibility bridge paths are unclaimed. The packet
therefore exposes one fixed schema-decoded renderer-to-main method for applying
an explicit visibility intent, without registering the main handler, rendering
pixels, or publishing thread content.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-visibility-bridge-contract.ts`
- `apps/openagents-desktop/src/thread-visibility-bridge-contract.test.ts`
- `apps/openagents-desktop/src/preload.cts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-20-desktop-thread-visibility-preload-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.thread_disclosure_intent.v1`,
`openagents.thread_disclosure_receipt.v1`, FF-D1-19's bounded apply results,
the fixed Desktop visibility channel, and the sandboxed preload allowlist.
Existing shared schemas, visibility persistence, `main.ts`, renderer, export,
Sync, provider, membership, authorization, publication, and transport contracts
remain unchanged.

Required behavior:

- the bridge accepts only an exact ref-only `thread.visibility.set` intent and
  rejects malformed, raw-content-bearing, export, or envelope-smuggled input
  before IPC;
- the preload exposes one fixed `threadVisibility.apply` method and channel,
  never raw `ipcRenderer`, a caller-selected channel, receipt metadata,
  filesystem, process, provider, membership, administrator, or publication
  authority;
- exact stored or unchanged results return only an identity-bound decoded
  visibility-applied receipt whose target exactly matches the request;
- bounded FF-D1-19 rejection reasons remain distinct, while malformed or native
  failures collapse to `command_unavailable` without leaking native details;
  and
- broader audience policy values remain explicit evidence only and do not
  imply that content was authorized, transported, published, or observed.

Proof: focused visibility bridge/preload and store/disclosure tests; production
preload build and Desktop TypeScript check; Fast Follow, behavior-contract,
ProductSpec, Sol, AssuranceSpec baseline, and repository-required checks.

Close rule: this packet closes only the sandboxed visibility apply request and
result boundary. Main-process handler registration and composition,
renderer command/pixels, actual audience authorization/publication adapters,
installed/runtime-rendered evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-20-20260717`
- base: `3d2797a54fadfba9e783e997bee79d0a51d197b9`
- worktree/branch: `openagents-ff-d1-20` / detached `origin/main`
- scope: fixed schema-decoded sandboxed Desktop thread-visibility apply boundary
- paths: the FF-D1-20 owned implementation paths above
- hot files: sandboxed `preload.cts`, new visibility bridge/test, accepted-plan ledger, and Sol manifest
- hot contracts: existing disclosure intent/receipt literals, FF-D1-19 result reasons, exact target binding, fixed IPC channel, and no raw host/publication authority
- dependencies: FF-D1-19 released; no relevant feature issue or competing claim; active Desktop `main.ts` and renderer work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T18:12:38Z`

### CLAIM-STATUS

- implementation: added one fixed schema-decoded visibility apply contract and exposed it through the sandboxed preload allowlist
- privacy/authority proof: exact intent and identity-bound receipt evidence only; no raw IPC, receipt metadata, content, membership, administrator, publication, filesystem, process, or provider authority escapes
- focused proof: all 19 bridge/store/disclosure tests passed; the full Desktop package typecheck and production preload build passed
- authority proof: Fast Follow package 13/13, behavior contracts 36/36, focused ProductSpec 104/104, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: AssuranceSpec environment-digest drift remains 189/190; root Fast Follow teardown coverage awaits the separately owned committed seed update for the new mobile-component teardown
- residual: main-process handler registration/composition, renderer command/pixels, actual audience authorization/publication adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `10878526eb95201820e0ceae88a85351d92cd18c`
- implementation: `d4f4fff6fe54ab6d1fc9b38aad899c7229d0ae3f`
- disposition: `bounded_packet_landed`
- released_at: `2026-07-17T18:19:20Z`
- residual: main-process handler registration/composition, renderer command/pixels, actual audience authorization/publication adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-21 — Desktop thread-visibility main-process handler seam

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-20. Active
work still owns Desktop `main.ts` and broad renderer surfaces. The packet
therefore adds a new-file-only main-process registration seam that binds the
fixed visibility channel to host-owned receipt metadata and the already-landed
visibility policy application, without composing the application entry point,
rendering pixels, or publishing thread content.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-visibility-main-handler.ts`
- `apps/openagents-desktop/src/thread-visibility-main-handler.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-21-desktop-thread-visibility-main-handler-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: the fixed `openagents:thread-visibility:apply` channel,
FF-D1-20 request/result decoding, FF-D1-19 apply input/result semantics,
trusted bundled-renderer sender admission, host-owned receipt references and
observation time, exact target binding, and bounded native-error redaction.
Existing shared schemas, policy persistence, `main.ts`, preload, renderer,
Sync, provider, membership, authorization, publication, and transport contracts
remain unchanged.

Required behavior:

- register exactly the fixed visibility channel, remove it exactly once, and
  reject all calls after close;
- reject untrusted senders, trust-check failures, malformed requests, extra
  envelopes, raw content, export intents, and caller-supplied receipt metadata
  before invoking policy application;
- pass only the decoded visibility intent plus host-supplied receipt reference
  and observation time to policy application, so renderer input cannot choose
  evidence metadata;
- return stored or unchanged results only when FF-D1-20 decoding proves exact
  intent, idempotency, thread, and target binding, while preserving the bounded
  policy rejection reasons; and
- collapse thrown, malformed, mismatched, or native-detail-bearing outcomes to
  `command_unavailable` without leaking paths, errors, content, or authority.

Proof: focused visibility handler, bridge, store, and disclosure tests; Desktop
TypeScript check; Fast Follow, behavior-contract, ProductSpec, Sol,
AssuranceSpec baseline, and repository-required checks.

Close rule: this packet closes only the main-process visibility handler seam.
Actual `main.ts` composition, renderer command/pixels, audience authorization
and publication adapters, installed/runtime-rendered evidence, and Day 1
completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-21-20260717`
- base: `b6852266992055da79f7f00386e3a891ab449105`
- worktree/branch: `openagents-ff-d1-21` / detached `origin/main`
- scope: trusted fixed-channel Desktop thread-visibility main-process handler seam with host-owned receipt metadata
- paths: the FF-D1-21 owned implementation paths above
- hot files: new Desktop visibility handler/test; accepted-plan ledger and Sol manifest
- hot contracts: fixed visibility channel, FF-D1-20 exact request/result boundary, FF-D1-19 apply input/results, trusted sender admission, host-owned receipt metadata, and native-detail redaction
- dependencies: FF-D1-20 released; no relevant feature issue or competing claim; active Desktop `main.ts` and renderer work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T18:33:54Z`

### CLAIM-STATUS

- implementation: added the fixed-channel trusted-sender visibility handler seam with host-owned receipt metadata and exact request/result decoding
- privacy/authority proof: only a decoded ref-only intent plus host-supplied receipt reference and observation time reach policy application; no raw content, path, native error, publication, membership, administrator, transport, process, or provider authority escapes
- focused proof: all 24 handler/bridge/store/disclosure tests passed and the full Desktop package typecheck passed
- authority proof: Fast Follow package 13/13, behavior contracts 36/36, focused ProductSpec 104/104, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: AssuranceSpec environment-digest drift remains 189/190; root Fast Follow teardown coverage still awaits the separately owned committed seed update for the mobile-component teardown
- residual: actual `main.ts` composition, renderer command/pixels, audience authorization/publication adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `f50376c32b457571f09de41c62b2af24bbf6f3f4`
- implementation: `6f186971d996fc7606a22bdda30eb0bab2ce853e`
- disposition: `bounded_packet_landed`
- released_at: `2026-07-17T18:39:29Z`
- residual: actual `main.ts` composition, renderer command/pixels, audience authorization/publication adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-22 — Desktop thread-visibility main composition resource

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-21. Active
work still owns Desktop `main.ts` and broad renderer surfaces. The packet
therefore adds a new-file-only composition resource that binds the trusted
fixed-channel handler to the restart-stable private visibility policy store,
without editing the application entry point, rendering pixels, or authorizing
content publication.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-visibility-main-composition.ts`
- `apps/openagents-desktop/src/thread-visibility-main-composition.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-22-desktop-thread-visibility-main-composition-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-19's private policy-store file and apply semantics,
FF-D1-21's fixed-channel handler dependencies and lifetime, trusted sender
admission, host-owned receipt metadata, Effect execution, restart replay, and
bounded acquisition/cleanup errors. Existing shared schemas, `main.ts`,
preload, renderer, Sync, provider, membership, authorization, publication, and
transport contracts remain unchanged.

Required behavior:

- open exactly one private visibility store for the host-supplied file and
  register exactly one FF-D1-21 fixed-channel handler;
- adapt the store's Effect application into the handler without exposing the
  store, path, Effect runtime, raw failures, or receipt-minting authority;
- preserve stored, unchanged replay, corrupt-store, stale-version, and other
  bounded policy outcomes across the composed boundary;
- close handler ownership exactly once, reject post-close calls, and suppress
  native cleanup details; and
- prove close/reopen against the same private file returns the identical
  persisted receipt for an exact retry without advancing visibility version.

Proof: focused visibility composition, handler, bridge, store, and disclosure
tests; Desktop TypeScript check; Fast Follow, behavior-contract, ProductSpec,
Sol, AssuranceSpec baseline, and repository-required checks.

Close rule: this packet closes only the tested visibility handler/store
composition resource. The actual `main.ts` call site, renderer command/pixels,
audience authorization and publication adapters, installed/runtime-rendered
evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-22-20260717`
- base: `6b8ca94e0b6322762a6243ed9dac0ef9e7d7d8da`
- worktree/branch: `openagents-ff-d1-22` / detached `origin/main`
- scope: restart-stable private Desktop thread-visibility handler/store composition resource
- paths: the FF-D1-22 owned implementation paths above
- hot files: new Desktop visibility composition/test; accepted-plan ledger and Sol manifest
- hot contracts: FF-D1-19 policy apply/store file, FF-D1-21 handler lifetime, trusted sender admission, host-owned receipt metadata, Effect execution, and restart replay
- dependencies: FF-D1-21 released; no relevant feature issue or competing claim; active Desktop `main.ts` and renderer work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T18:52:03Z`

### CLAIM-STATUS

- implementation: added an Effect composition resource that opens the private visibility store and owns exactly one fixed-channel handler lifetime
- privacy/authority proof: the store path and Effect runtime remain inside main-process composition; only decoded intents and bounded results cross the handler, with no content, path, native error, publication, membership, administrator, transport, process, or provider authority exposed
- focused proof: all 29 composition/handler/bridge/store/disclosure tests passed and the full Desktop package typecheck passed
- authority proof: Fast Follow package 13/13, behavior contracts 36/36, focused ProductSpec 104/104, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: AssuranceSpec environment-digest drift remains 189/190; root Fast Follow teardown coverage still awaits the separately owned committed seed update for the mobile-component teardown
- residual: actual `main.ts` call-site composition, renderer command/pixels, audience authorization/publication adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `6340ce8ff9e7719bfb6e83699f430d24c393068f`
- implementation: `4f3151a1666df8ecc2c2d7b8fe14b939f6af4620`
- disposition: `bounded_packet_landed`
- released_at: `2026-07-17T18:58:22Z`
- residual: actual `main.ts` call-site composition, renderer command/pixels, audience authorization/publication adapters, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-23 — Desktop thread-visibility audience authorization adapter

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-22. Active
work still owns Desktop `main.ts` and broad renderer surfaces. The packet
therefore adds a new-file-only pure authorization adapter that evaluates one
applied visibility receipt against bounded host-supplied owner, workspace,
administrator, and group authority facts. It does not look up membership,
transport content, publish a thread, or edit the active host/UI files.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-visibility-audience-authorization.ts`
- `apps/openagents-desktop/src/thread-visibility-audience-authorization.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-23-desktop-thread-visibility-audience-authorization-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.thread_disclosure_receipt.v1`, exact
`visibility_applied` receipt identity, explicit audience and administrator
axes, bounded caller authority facts, fail-closed decoding, and ref-only
authorization decisions. Existing shared schemas, policy persistence,
`main.ts`, preload, renderer, membership stores, Sync, provider, publication,
and transport contracts remain unchanged.

Required behavior:

- accept only an exact visibility-applied receipt plus exact bounded actor,
  owner, workspace-role, and group authority facts; reject raw content,
  duplicate/ambiguous facts, exports, malformed receipts, and extra envelopes;
- authorize the exact owner, internet-readable audience, matching workspace
  members, matching named-group members, or an explicitly configured matching
  workspace administrator, with one bounded basis;
- deny when no supplied fact satisfies the applied target and never infer
  membership, group, administrator, owner, or publication authority;
- bind every decision to the exact receipt, thread, and visibility version and
  expose no content, path, credential, native error, or authority-store detail;
  and
- remain a pure authorization decision only: an authorized result does not
  transport, publish, share, export, or prove observation of thread content.

Proof: focused audience-authorization and shared disclosure tests; Desktop
TypeScript check; Fast Follow, behavior-contract, ProductSpec, Sol,
AssuranceSpec baseline, and repository-required checks.

Close rule: this packet closes only the pure applied-policy audience decision.
Real membership/authorization lookup, publication/transport, the actual
`main.ts` call site, renderer command/pixels, installed/runtime-rendered
evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-23-20260717`
- base: `9de9b278d13c326996c666fb8903f72d7adc3ccf`
- worktree/branch: `openagents-ff-d1-23` / detached `origin/main`
- scope: fail-closed ref-only Desktop visibility audience authorization against an exact applied receipt
- paths: the FF-D1-23 owned implementation paths above
- hot files: new Desktop audience-authorization adapter/test; accepted-plan ledger and Sol manifest
- hot contracts: exact visibility-applied receipt identity, audience/admin axes, bounded authority facts, decision binding, and no inferred publication authority
- dependencies: FF-D1-22 released; no relevant feature issue or competing claim; active Desktop `main.ts` and renderer work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T19:02:45Z`

### CLAIM-STATUS

- implementation: added a pure exact-receipt audience authorization adapter with bounded owner, workspace, administrator, and group facts
- privacy/authority proof: decisions are ref-only and bound to one applied receipt/thread/version; authorization performs no membership lookup, content access, publication, transport, export, network, provider, filesystem, or credential effect
- focused proof: all 15 audience-authorization/disclosure tests passed and the full Desktop package typecheck passed
- authority proof: Fast Follow package 13/13, behavior contracts 36/36, focused ProductSpec 104/104, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: AssuranceSpec environment-digest drift remains 189/190; root Fast Follow teardown coverage still awaits the separately owned committed seed update for the mobile-component teardown
- residual: real authority lookup and publication/transport, actual `main.ts` call-site composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `dfb0569ea260d8d6e77d8241075333daa2b4f8e3`
- implementation: `27d3d216bd04fe57163d302ba293154a3ec8b51e`
- disposition: `bounded_packet_landed`
- released_at: `2026-07-17T19:10:08Z`
- residual: real authority lookup and publication/transport, actual `main.ts` call-site composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-24 — Desktop confirmed Sync visibility-authority lookup

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-23. Active
work still owns Desktop `main.ts` and broad renderer surfaces. The packet
therefore adds a new-file-only Effect adapter that derives one FF-D1-23
workspace authority fact from a live, server-confirmed Khala Sync
`team_membership` entity. It does not edit the active host/UI files, infer
named-group membership, transport content, or publish a thread.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-visibility-sync-authority.ts`
- `apps/openagents-desktop/src/thread-visibility-sync-authority.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-24-desktop-thread-visibility-sync-authority-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-23's exact applied-receipt authorization request and
decision, Khala Sync `scope.team.<teamId>` identity, confirmed
`team_membership` entity schema, live/revoked scope state, and fail-closed
authority lookup. Existing shared schemas, Sync protocol, policy persistence,
`main.ts`, preload, renderer, publication, group, provider, and transport
contracts remain unchanged.

Required behavior:

- preserve FF-D1-23's exact owner and internet-readable decisions without an
  authority-store lookup, and reject every malformed request before lookup;
- for a workspace-bearing target, accept only an exact
  `scope.team.<teamId>` workspace and require the corresponding Sync scope to
  be live with server-confirmed state before and after the membership read;
- decode only the exact confirmed `team_membership` entity for the actor and
  team, map active owner/admin roles to administrator and active
  member/viewer roles to member, and treat confirmed absence or inactive
  membership as a denial;
- fail closed as authority unavailable on stale, denied, refetching,
  unconfirmed, malformed, or failed Sync reads, without exposing entity
  bodies, timestamps, storage errors, credentials, or native details; and
- leave named-group targets denied until a separately authoritative group
  membership source exists; authorization remains ref-only and does not
  transport, publish, share, export, or prove observation of thread content.

Proof: focused Sync-authority, audience-authorization, disclosure, and Khala
Sync contract tests; Desktop TypeScript check; Fast Follow,
behavior-contract, ProductSpec, Sol, AssuranceSpec baseline, and
repository-required checks.

Close rule: this packet closes only live confirmed team-membership lookup for
workspace-member/administrator authorization. Named-group authority,
publication/transport, the actual `main.ts` call site, renderer command/pixels,
installed/runtime-rendered evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-24-20260717`
- base: `483b4521c861389769e177f83a4ed594db1af160`
- worktree/branch: `openagents-ff-d1-24` / detached `origin/main`
- scope: fail-closed Desktop visibility authorization from live server-confirmed Khala Sync team membership
- paths: the FF-D1-24 owned implementation paths above
- hot files: new Desktop Sync-authority adapter/test; accepted-plan ledger and Sol manifest
- hot contracts: FF-D1-23 exact request/decision, team-scope identity, confirmed membership entity, live Sync state, and no inferred group/publication authority
- dependencies: FF-D1-23 released; no relevant feature issue or competing claim; active Desktop `main.ts` and renderer work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T19:25:35Z`

### CLAIM-STATUS

- implementation: added an Effect adapter that derives FF-D1-23 workspace-member or administrator authority only from a live server-confirmed Khala Sync team-membership projection
- privacy/authority proof: owner/public paths remain lookup-free; workspace reads are exact-scope and ref-only; stale, revoked, refetching, unconfirmed, failed, malformed, cross-team, or ambiguous state fails closed without exposing entity bodies or native details; named-group membership is never inferred
- focused proof: 68/68 Sync-authority, audience-authorization, disclosure, and Khala Sync contract tests passed; Desktop package typecheck passed
- authority proof: Fast Follow package 13/13, behavior contracts 36/36, focused ProductSpec 104/104, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: AssuranceSpec environment-digest drift remains 189/190; root Fast Follow teardown coverage remains 6/7 because the separately owned seed update is not committed on `main`
- residual: named-group authority, publication/transport, actual `main.ts` call-site composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `9cccc0627c30682eb8c93880b6888a592e960b2b`
- implementation: `9fafd744f96a7488c36d9511c569289564049aa0`
- disposition: `bounded_packet_landed`
- released_at: `2026-07-17T19:33:12Z`
- residual: named-group authority, publication/transport, actual `main.ts` call-site composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-25 — Desktop public-visibility share publication transport

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-24. No
target-authoritative named-group membership source exists, while active work
still owns Desktop `main.ts` and broad renderer surfaces. The packet therefore
adds a new-file-only Effect transport that binds an exact owner authorization
and applied `internet_readable` visibility receipt to the existing
authenticated `/api/share` projection service. It does not edit the active
host/UI files, invent group authority, upload local transcript bytes, or add a
new server route.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-visibility-publication-transport.ts`
- `apps/openagents-desktop/src/thread-visibility-publication-transport.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-25-desktop-thread-visibility-publication-transport-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-23's exact owner authorization decision,
`openagents.thread_disclosure_receipt.v1`, applied `internet_readable`
visibility identity, existing `/api/share` request/response behavior,
credential custody, single-attempt publication uncertainty, same-origin
canonical share URLs, and ref-only Desktop results. Existing shared schemas,
server routes, policy persistence, `main.ts`, preload, renderer, named-group,
provider, and Sync contracts remain unchanged.

Required behavior:

- accept only one exact applied `internet_readable` visibility receipt, an
  FF-D1-23 `authorized` decision whose basis is the exact owner and whose
  receipt/thread/version binding matches, and a supported server source whose
  id equals that thread ref;
- obtain the existing host-custodied access token only after request
  validation, then send exactly one authenticated `POST /api/share` carrying
  only the source ref and public audience marker—never local transcript,
  export, file, credential, path, provider, or native payload bytes;
- decode only the bounded active share response, require its canonical URL to
  share the configured service origin, and return a ref-only publication
  result bound to the visibility receipt/thread/version;
- fail closed without dispatch for malformed, non-owner, mismatched,
  unsupported-audience, uncredentialed, or expanded requests; and
- never automatically retry a network, timeout, throttle, or server failure:
  because the existing create route has no reviewed idempotency contract,
  ambiguous delivery returns `publication_outcome_unknown` rather than
  success, failure, or a duplicate publication attempt.

Proof: focused publication-transport, audience-authorization, disclosure, and
share-contract tests; Desktop TypeScript check; Fast Follow,
behavior-contract, ProductSpec, Sol, AssuranceSpec baseline, and
repository-required checks.

Close rule: this packet closes only owner-authorized public-visibility
transport into the existing server-side redacted share builder. Named-group
authority, workspace/named-group publication, idempotent publication
reconciliation, the actual `main.ts` call site, renderer command/pixels,
installed/runtime-rendered evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-25-20260717`
- base: `e2a38a225e8df469c0d5a8ff24d9a171a06b6424`
- worktree/branch: `openagents-ff-d1-25` / detached `origin/main`
- scope: owner-authorized ref-only Desktop publication of applied internet-readable visibility through existing `/api/share`
- paths: the FF-D1-25 owned implementation paths above
- hot files: new Desktop publication transport/test; accepted-plan ledger and Sol manifest
- hot contracts: exact applied-public receipt/owner decision binding, existing share route, credential custody, one-attempt uncertainty, same-origin response, and no local content upload
- dependencies: FF-D1-24 released; named-group authority lacks an authoritative source; no relevant feature issue or competing claim; active Desktop `main.ts` and renderer work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T19:49:51Z`

### CLAIM-STATUS

- implementation: added an Effect transport that binds exact applied public visibility and exact owner authorization to one ref-only call into the existing redacted share builder
- privacy/authority proof: request validation precedes host credential access; only source identity and public audience cross the boundary; responses are bounded and same-origin; ambiguous delivery is never retried or mislabeled definitive
- focused proof: 26/26 publication-transport, audience-authorization, disclosure, and share-contract tests passed; Desktop package typecheck passed
- authority proof: Fast Follow package 13/13, behavior contracts 36/36, focused ProductSpec 104/104, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: AssuranceSpec environment-digest drift remains 189/190; root Fast Follow teardown coverage remains 6/7 because the separately owned seed update is not committed on `main`
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-25-desktop-thread-visibility-publication-transport-receipt.md`
- residual: named-group/workspace publication authority, ambiguous-create reconciliation, actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `1fd8c5913573998a33fd7badfda7b8e5b4fca626`
- implementation: `f449d6200f3acfa30f484c612c1469b589d82c5a`
- disposition: `bounded_packet_landed`
- released_at: `2026-07-17T19:59:19Z`
- residual: named-group/workspace publication authority, ambiguous-create reconciliation, actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-26 — Desktop workspace-members share publication transport

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next non-colliding Day 1 residual after FF-D1-25. Canonical
export boot composition still lacks a production accepted-event source,
named-group publication still lacks authoritative group membership, and active
work continues to own Desktop `main.ts` and broad renderer surfaces. The
packet therefore adds a new-file-only Effect transport for an already-applied
`workspace_members` visibility receipt through the existing authenticated
`/api/share` projection service. It does not edit active host/UI files, invent
group authority, upload local transcript bytes, or add a server route.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-visibility-workspace-publication-transport.ts`
- `apps/openagents-desktop/src/thread-visibility-workspace-publication-transport.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-26-desktop-thread-visibility-workspace-publication-transport-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-23's exact authorization decision,
`openagents.thread_disclosure_receipt.v1`, applied `workspace_members`
visibility and `scope.team.<teamId>` identity, existing `/api/share`
`TeamMembers` request behavior, team-thread source identity, credential custody,
single-attempt publication uncertainty, same-origin canonical share URLs, and
ref-only Desktop results. Existing shared schemas, server routes, policy
persistence, `main.ts`, preload, renderer, named-group, provider, and Sync
contracts remain unchanged.

Required behavior:

- accept only one exact applied `workspace_members` visibility receipt, an
  FF-D1-23 authorized decision whose basis and receipt/thread/version binding
  match, a bounded team display name, and a supported server source whose id
  equals the thread ref;
- derive the team id only from an exact `scope.team.<teamId>` target, require a
  team-thread source to carry that same team id, and never accept a caller-
  supplied alternate audience or authority scope;
- obtain the existing host-custodied access token only after request
  validation, then send exactly one authenticated `POST /api/share` carrying
  only the source ref and exact `TeamMembers` audience—never local transcript,
  export, file, credential, path, provider, native payload, or group bytes;
- decode only the bounded active share response, require its canonical URL to
  share the configured service origin, and return a ref-only publication
  result bound to the visibility receipt/thread/version/team; and
- fail closed without dispatch for malformed, mismatched, unsupported,
  uncredentialed, or expanded requests, and never automatically retry an
  ambiguous create while the existing route lacks a reviewed idempotency
  contract.

Proof: focused workspace-publication, audience-authorization, disclosure, Sync
authority, and share-contract tests; Desktop TypeScript check; Fast Follow,
behavior-contract, ProductSpec, Sol, AssuranceSpec baseline, and repository-
required checks.

Close rule: this packet closes only authorized workspace-members publication
into the existing server-side redacted share builder. Named-group authority and
publication, ambiguous-create reconciliation, canonical-export evidence
authority, actual `main.ts` composition, renderer command/pixels,
installed/runtime-rendered evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-26-20260717`
- base: `8342ad394292473564fb2f15429a65c2fc675562`
- worktree/branch: `openagents-ff-d1-26` / detached `origin/main`
- scope: ref-only Desktop publication of applied workspace-members visibility through existing `/api/share`
- paths: the FF-D1-26 owned implementation paths above
- hot files: new Desktop workspace-publication transport/test; accepted-plan ledger and Sol manifest
- hot contracts: exact applied-workspace receipt/authorization binding, exact team scope/source, existing share route, credential custody, one-attempt uncertainty, same-origin response, and no local content upload
- dependencies: FF-D1-25 released; canonical export source and named-group authority remain unavailable; no relevant feature issue or competing claim; active Desktop `main.ts` and renderer work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T20:07:00Z`

### CLAIM-STATUS

- implementation: added an Effect transport that binds exact applied workspace-members visibility and bounded authorization to one ref-only call into the existing redacted share builder
- privacy/authority proof: exact team scope is derived from the receipt; team-thread sources must match it; request validation precedes credential access; only source identity and TeamMembers audience cross the boundary; ambiguous delivery is never retried or mislabeled definitive
- focused proof: 58/58 workspace-publication, audience-authorization, Sync-authority, disclosure, and share-contract tests passed; Desktop package typecheck passed
- authority proof: Fast Follow package 13/13, behavior contracts 36/36, focused ProductSpec 104/104, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: AssuranceSpec environment-digest drift remains 189/190; root Fast Follow teardown coverage remains 6/7 because the separately owned seed update is not committed on `main`
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-26-desktop-thread-visibility-workspace-publication-transport-receipt.md`
- residual: named-group authority/publication, ambiguous-create reconciliation, canonical-export evidence authority, actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `f22f0ae89d2156ccf3509ed1d885fba368143c17`
- implementation: `6a9bc3be27cfbed1ef3b5c3aac96ba102feddd80`
- disposition: `bounded_packet_landed`
- released_at: `2026-07-17T20:19:32Z`
- residual: named-group authority/publication, ambiguous-create reconciliation, canonical-export evidence authority, actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-27 — Idempotent share-create reconciliation

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-26. Active work
continues to own Desktop `main.ts` and broad renderer surfaces, canonical
export boot composition still lacks an authoritative accepted-event source,
and named-group publication still lacks authoritative membership. The prior
public and workspace publication transports also cannot safely retry an
ambiguous `POST /api/share`, because the share-create route has no reviewed
idempotency identity. This packet adds that server-side contract without
touching active Desktop host/UI work.

Owned implementation paths:

- `apps/openagents.com/workers/api/src/share-create-idempotency.ts`
- `apps/openagents.com/workers/api/src/share-create-idempotency.test.ts`
- `apps/openagents.com/workers/api/src/share-routes.ts`
- `apps/openagents.com/workers/api/src/share-routes.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-27-idempotent-share-create-reconciliation-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: authenticated actor identity, bounded `Idempotency-Key`, exact
share-create semantics, existing redacted source loading and audience
authorization, deterministic owner-scoped share identity, semantic replay,
conflicting-key refusal, canonical same-origin URL, and no credential or
content widening. No schema, migration, Desktop `main.ts`, preload, renderer,
provider, or Sync contract changes are admitted.

Required behavior:

- accept an optional bounded ASCII `Idempotency-Key` only on authenticated
  share creation and derive a deterministic UUID-shaped share identity from
  the authenticated owner plus that exact key;
- preserve existing random identities when the header is absent;
- after normal authentication, source loading, and audience authorization,
  return an existing active record only when its owner, source, audience,
  title, redaction policy, expiry, and canonical URL exactly match the request;
- refuse a reused key whose existing record differs, is revoked, expired, or
  malformed, without overwriting or publishing another share; and
- keep the response bounded and distinguish a replay from a first creation
  without exposing private source content.

Proof: focused idempotency and share-route tests; API typecheck; Fast Follow,
behavior-contract, ProductSpec, Sol, AssuranceSpec baseline, and repository-
required checks.

Close rule: this packet closes only server-side ambiguous-create
reconciliation. Client transport retry/reconciliation, named-group authority
and publication, canonical-export evidence authority, actual `main.ts`
composition, renderer command/pixels, installed/runtime-rendered evidence, and
Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-27-20260717`
- base: `eb15ce99c54af497874a998192b1afbb2fa8268b`
- worktree/branch: `openagents-ff-d1-27` / detached `origin/main`
- scope: authenticated owner-scoped idempotency and exact semantic replay for existing `/api/share` creation
- paths: the FF-D1-27 owned implementation paths above
- hot files: existing share route/test; new idempotency helper/test; accepted-plan ledger and Sol manifest
- hot contracts: authenticated owner binding, bounded key syntax, deterministic UUID identity, exact semantic replay, conflict refusal, and unchanged no-header behavior
- dependencies: FF-D1-26 released; no relevant open bug issue or competing active claim; July 4 share-route worktrees audited stale with no owning process; active Desktop host/UI paths explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T20:45:03Z`

### CLAIM-STATUS

- implementation: added authenticated owner-scoped UUIDv5 identity, exact semantic replay, conflicting-key refusal, and concurrent-create reread reconciliation to existing `/api/share` creation
- privacy/authority proof: ordinary source loading and audience authorization still run; replay matches only exact active record semantics; no credential, content, migration, schema, audience, Desktop host, or renderer authority widened
- focused proof: 32/32 idempotency/share route/projection tests passed; API package typecheck passed with two pre-existing Effect advisories
- authority proof: Fast Follow package 13/13, behavior contracts 36/36, focused ProductSpec 104/104, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: AssuranceSpec is 188/190 from the known environment digest drift plus an unrelated offline distribution timeout reproduced twice; root Fast Follow remains 6/7 from the separately owned teardown seed; shared `core.bare=true` was not mutated and `check:fast` passed under a task-local work-tree override
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-27-idempotent-share-create-reconciliation-receipt.md`
- residual: client publication retry/reconciliation, named-group authority/publication, canonical-export evidence authority, actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `46b95cc49ab7a2202f8b2470394936a495df2238`
- implementation: `4ef8dc7858aad3e07c81d4c2707257ecb28c5076`
- disposition: `bounded_packet_landed`
- released_at: `2026-07-17T21:01:36Z`
- residual: client publication retry/reconciliation, named-group authority/publication, canonical-export evidence authority, actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-28 — Desktop public-share idempotent retry transport

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-27. The server
now owns a reviewed authenticated `Idempotency-Key` reconciliation contract,
while the existing Desktop public-visibility transport still sends only one
attempt and reports every ambiguous delivery as unknown. Active work continues
to own Desktop `main.ts` and renderer surfaces; named-group authority and
canonical-export evidence remain unavailable. This packet therefore wires one
bounded idempotent retry only into the already-authorized public-share adapter.
Workspace-members retry remains a separate later packet.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-visibility-publication-transport.ts`
- `apps/openagents-desktop/src/thread-visibility-publication-transport.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-28-desktop-public-share-idempotent-retry-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: the exact applied public-visibility receipt, FF-D1-23 owner
authorization, FF-D1-27 authenticated owner-scoped `Idempotency-Key`, same-key
semantic replay, `201` first creation, `200` replay, bounded response decoding,
credential custody, and ref-only request/response privacy. Server routes,
schemas, migrations, workspace/named-group adapters, `main.ts`, preload,
renderer, provider, and Sync contracts remain unchanged.

Required behavior:

- derive one bounded visible-ASCII publication key from the exact disclosure
  receipt identity without exposing raw content, credentials, paths, or
  provider data;
- validate the complete request before reading the host-custodied token, then
  send at most two identical authenticated ref-only create requests carrying
  the same key and body;
- retry exactly once only for transport failure, retryable/ambiguous HTTP
  status, unreadable body, or malformed/unsafe success evidence;
- accept only `201` with `Idempotency-Replayed: false` or `200` with
  `Idempotency-Replayed: true`, plus the existing bounded active same-origin
  share response; and
- never retry definitive authentication, authorization, malformed-request, or
  idempotency-conflict refusal, and keep exhausted ambiguity typed unknown.

Proof: focused public-publication, disclosure, authorization, and FF-D1-27
server-contract tests; Desktop typecheck; Fast Follow, behavior-contract,
ProductSpec, Sol, AssuranceSpec baseline, and repository-required checks.

Close rule: this packet closes only bounded public-share client retry and
reconciliation. Workspace-members retry, named-group authority/publication,
canonical-export evidence authority, actual `main.ts` composition, renderer
command/pixels, installed/runtime-rendered evidence, and Day 1 completion
remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-28-20260717`
- base: `20f352f33213e4aa9c908f468391787cca496c74`
- worktree/branch: `openagents-ff-d1-28` / detached `origin/main`
- scope: one bounded same-key retry and exact replay acceptance for Desktop public visibility publication
- paths: the FF-D1-28 owned implementation paths above
- hot files: existing public-publication transport/test; accepted-plan ledger and Sol manifest
- hot contracts: exact receipt-derived key, at-most-two identical attempts, FF-D1-27 201/200 replay distinction, definitive refusal, exhausted unknown, and unchanged credential/content boundary
- dependencies: FF-D1-27 released; no relevant open bug issue or competing claim; all audited worktrees leave these two Desktop transport paths unmodified; active Desktop host/UI, teardown, T3, and Full Auto files explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T21:15:00Z`

### CLAIM-STATUS

- implementation: added one typed Effect retry for public visibility publication using one bounded exact receipt-derived key, byte-identical request replay, and FF-D1-27's exact first-create/replay evidence contract
- privacy/authority proof: full request validation still precedes one credential read; only source/audience refs cross the boundary; retry is capped at two total attempts; definitive refusal never retries; same-origin bounded decoding remains mandatory
- focused proof: 35/35 public-publication, disclosure, authorization, and server idempotency-contract tests passed; Desktop package typecheck passed
- authority proof: Fast Follow package 13/13, root Fast Follow 7/7, behavior contracts 36/36, focused ProductSpec 104/104, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: AssuranceSpec remains 189/190 from only the known environment digest drift after its Git-fixture inventory reran 2/2 without the task-local work-tree override; shared `core.bare=true` was not mutated
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-28-desktop-public-share-idempotent-retry-receipt.md`
- residual: workspace-members retry, named-group authority/publication, canonical-export evidence authority, actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `4c4510bf6c564e354f1df7f633c3dd184a7b7fcb`
- implementation: `f2d88980e7ada9b732802468904cde0d8ba60d48`
- disposition: bounded FF-D1-28 public-share client retry landed, verified, and receipted; release only this packet's claim
- released_at: `2026-07-17T21:21:31Z`
- residual: workspace-members retry, named-group authority/publication, canonical-export evidence authority, actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-29 — Desktop workspace-share idempotent retry transport

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-28. The existing
workspace-members publication transport still sends only one attempt even
though FF-D1-27 now supplies authenticated owner-scoped idempotency and exact
first-create/replay evidence. Active work continues to own Desktop `main.ts`
and renderer surfaces, while named-group publication still lacks authoritative
membership. This packet therefore adds one bounded idempotent retry only to
the existing already-authorized workspace-members adapter.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-visibility-workspace-publication-transport.ts`
- `apps/openagents-desktop/src/thread-visibility-workspace-publication-transport.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-29-desktop-workspace-share-idempotent-retry-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: the exact applied workspace-members visibility receipt,
FF-D1-23 owner/workspace-member authorization, exact team scope and source,
FF-D1-27 authenticated owner-scoped `Idempotency-Key`, same-key semantic
replay, `201` first creation, `200` replay, exact bounded audience response,
credential custody, and ref-only request/response privacy. Server routes,
schemas, migrations, public/named-group adapters, `main.ts`, preload, renderer,
provider, and Sync contracts remain unchanged.

Required behavior:

- derive one bounded visible-ASCII publication key from the exact disclosure
  receipt identity without exposing raw content, credentials, paths, team
  names, or provider data;
- validate the complete receipt, authorization, team scope/source, and bounded
  team name before reading the host-custodied token, then send at most two
  identical authenticated ref-only create requests with the same key and body;
- retry exactly once only for transport failure, retryable or ambiguous HTTP
  status, unreadable body, or malformed, unsafe, or wrong-audience success
  evidence;
- accept only `201` with `Idempotency-Replayed: false` or `200` with
  `Idempotency-Replayed: true`, plus the existing bounded active same-origin
  response carrying the exact expected TeamMembers audience label; and
- never retry definitive authentication, authorization, malformed-request, or
  idempotency-conflict refusal, and keep exhausted ambiguity typed unknown.

Proof: focused workspace-publication, disclosure, authorization, Sync-authority,
and FF-D1-27 server-contract tests; Desktop typecheck; Fast Follow,
behavior-contract, ProductSpec, Sol, AssuranceSpec baseline, and repository-
required checks.

Close rule: this packet closes only bounded workspace-members client retry and
reconciliation. Named-group authority/publication, canonical-export evidence
authority, actual `main.ts` composition, renderer command/pixels,
installed/runtime-rendered evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-29-20260717`
- base: `6631bcba080ec4005030f9ad1d5bcfee4d890a18`
- worktree/branch: `openagents-ff-d1-29` / detached `origin/main`
- scope: one bounded same-key retry and exact replay acceptance for Desktop workspace-members visibility publication
- paths: the FF-D1-29 owned implementation paths above
- hot files: existing workspace-publication transport/test; accepted-plan ledger and Sol manifest
- hot contracts: exact receipt-derived key, exact team audience, at-most-two identical attempts, FF-D1-27 201/200 replay distinction, definitive refusal, exhausted unknown, and unchanged credential/content boundary
- dependencies: FF-D1-28 released; no relevant open bug issue or competing claim; all audited worktrees leave these two Desktop transport paths unmodified; active Desktop host/UI, teardown, T3, and Full Auto files explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T21:25:23Z`

### CLAIM-STATUS

- implementation: added one typed Effect retry for workspace-members visibility publication using one bounded exact receipt-derived key, byte-identical request replay, and FF-D1-27's exact first-create/replay evidence contract
- privacy/authority proof: exact receipt, authorization, team scope/source, team name, and origin validation still precede one credential read; only source and TeamMembers audience refs cross the boundary; retry is capped at two total attempts; definitive refusal never retries; exact-audience same-origin decoding remains mandatory
- focused proof: 43/43 workspace-publication, disclosure, authorization, Sync-authority, and server idempotency-contract tests passed; Desktop package typecheck passed
- authority proof: Fast Follow package 13/13, root Fast Follow 7/7, behavior contracts 36/36, focused ProductSpec 104/104, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: AssuranceSpec remains 189/190 from only the known environment digest drift after its Git-fixture inventory reran 2/2 without the task-local work-tree override; shared `core.bare=true` was not mutated
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-29-desktop-workspace-share-idempotent-retry-receipt.md`
- residual: named-group authority/publication, canonical-export evidence authority, actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `4398a7765be0e338fb9426e4c1b51c38bec340fd`
- implementation: `9c492766148371f4dc27000bc2695e11f028a726`
- disposition: bounded FF-D1-29 workspace-share client retry landed, verified, and receipted; release only this packet's claim
- released_at: `2026-07-17T21:33:55Z`
- residual: named-group authority/publication, canonical-export evidence authority, actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-30 — Desktop confirmed-timeline canonical-export evidence adapter

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-29. Named-group
publication still lacks authoritative membership, and active work continues to
own Desktop `main.ts` and renderer surfaces. The target-owned Khala Sync client
now exposes a bounded server-confirmed agent timeline for an exact thread, but
the canonical-export command still has only an abstract evidence seam. This
packet adds a new-file-only Effect adapter from that confirmed timeline to the
existing FF-D1-11 evidence snapshot without treating provider history,
optimistic state, or a projection default as accepted-event authority.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-confirmed-timeline-evidence.ts`
- `apps/openagents-desktop/src/thread-export-confirmed-timeline-evidence.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-30-desktop-confirmed-timeline-export-evidence-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: the target-owned server-confirmed agent timeline, exact thread
lookup, live settled Sync state, bounded current run/events, strict canonical
export event identity, deterministic accepted-relation identity, and the
existing FF-D1-11 `{ status, threadRef, events, relations }` evidence seam.
Provider history, event-authority schemas, export compiler/store/IPC, `main.ts`,
preload, renderer, named-group, server, and Sync contracts remain unchanged.

Required behavior:

- read only one exact thread through the injected confirmed-timeline
  `snapshotForThread` authority and return unavailable when the source throws,
  is absent, non-live, cursorless, has pending optimistic mutations, or lacks a
  current confirmed run;
- accept at most 500 exact server-confirmed events, require every event to bind
  the current run, and fail closed on malformed, duplicate, non-canonical, or
  cross-run identity/sequence/timestamp data;
- project each confirmed event into one bounded canonical export event whose
  data is the exact decoded target-owned confirmed event projection and one
  deterministic ref-only `accepted` relation bound to its thread/event/version;
- never synthesize superseded or reverted relations from a source that does not
  carry those facts, never consume provider-native history, and never expose
  credentials, paths, native payloads, or host authority; and
- produce only the existing bounded FF-D1-11 evidence snapshot or the exact
  `{ status: "unavailable" }` refusal.

Proof: focused confirmed-timeline adapter, export command/compiler,
event-authority, and Khala Sync confirmed-timeline tests; Desktop typecheck;
Fast Follow, behavior-contract, ProductSpec, Sol, AssuranceSpec baseline, and
repository-required checks.

Close rule: this packet closes only current accepted-event evidence for owner-
only export from the target-owned confirmed timeline. Supersession/reversion
evidence remains unavailable until an authoritative source carries it;
named-group authority/publication, actual `main.ts` composition, renderer
command/pixels, installed/runtime-rendered evidence, and Day 1 completion
remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-30-20260717`
- base: `eae0c55d660812bdb630017bae5599c08a09ce0d`
- worktree/branch: `openagents-ff-d1-30` / detached `origin/main`
- scope: fail-closed canonical-export evidence from one exact settled server-confirmed thread timeline
- paths: the FF-D1-30 owned implementation paths above
- hot files: two new Desktop evidence adapter/test files; accepted-plan ledger and Sol manifest
- hot contracts: confirmed-only target authority, exact thread/run/event binding, deterministic accepted relations, no inferred supersession/reversion, and the unchanged FF-D1-11 evidence seam
- dependencies: FF-D1-29 released; confirmed Sync timeline authority exists; no relevant open bug issue or competing claim; all audited worktrees leave the new paths unclaimed; active Desktop host/UI, T3, Full Auto, teardown, and release files explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T21:46:50Z`

### CLAIM-STATUS

- implementation: added one new-file-only Effect adapter from the exact settled target-owned confirmed timeline to FF-D1-11's existing evidence snapshot, with bounded current events and deterministic accepted relations
- privacy/authority proof: exact thread/run/event binding and strict decoding precede projection; optimistic, non-live, malformed, duplicate, cross-run, or throwing sources fail closed; no provider history, credentials, paths, native payloads, inferred supersession, or inferred reversion enters the export seam
- focused proof: 26/26 confirmed-timeline adapter, export command/compiler, event-authority, and Sync timeline tests passed; Desktop package typecheck passed
- authority proof: Fast Follow package 13/13, root Fast Follow 7/7, behavior contracts 36/36, focused ProductSpec 104/104, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: AssuranceSpec remains 189/190 from only the known environment digest drift; shared Git configuration was not mutated
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-30-desktop-confirmed-timeline-export-evidence-receipt.md`
- residual: authoritative supersession/reversion export evidence, named-group authority/publication, actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `3b319f0465919a49a081504a5207878b9d43b22f`
- implementation: `de2f39ee26954e40108170d70217783e118c3897`
- disposition: bounded FF-D1-30 confirmed-timeline export-evidence adapter landed, verified, and receipted; release only this packet's claim
- released_at: `2026-07-17T21:54:17Z`
- residual: authoritative supersession/reversion export evidence, named-group authority/publication, actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-31 — Desktop confirmed-timeline export-command composition

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-30. The
target-owned confirmed-timeline adapter and canonical export command exist but
remain uncomposed, while authoritative supersession/reversion and named-group
membership are unavailable and active work owns Desktop `main.ts`, renderer,
and installed-runtime surfaces. This packet therefore adds one new-file-only
Effect composition that binds the confirmed evidence reader to the existing
owner-only export command without touching those active surfaces.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-confirmed-timeline-command.ts`
- `apps/openagents-desktop/src/thread-export-confirmed-timeline-command.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-31-desktop-confirmed-timeline-export-command-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-30's exact settled confirmed-timeline reader, FF-D1-11's
owner-only canonical export command, the existing evidence-unavailable and
invalid-evidence outcomes, exact persistence/digest/receipt dependencies, and
one eventual host call site. Export schemas, persistence, IPC, preload,
`main.ts`, renderer, provider, named-group, server, and Sync contracts remain
unchanged.

Required behavior:

- accept the explicit confirmed-timeline snapshot authority plus the existing
  command persistence, receipt, observation, and digest dependencies, without
  hidden defaults or new authority;
- bind `readEvidence` only to the FF-D1-30 reader and preserve its exact
  fail-closed unavailable result through the existing command outcome;
- defer the confirmed-timeline lookup until a valid owner-only canonical export
  intent reaches command execution, and read only the intent's exact thread;
- preserve existing deterministic compilation, persistence, idempotency,
  receipt, and rejection behavior without exposing source exceptions, local
  paths, credentials, provider payloads, or host authority; and
- return only the existing command surface so a later `main.ts` packet has one
  bounded composition call rather than reconstructing the evidence seam.

Proof: focused confirmed-timeline command composition, adapter, export command,
compiler, event-authority, and Sync timeline tests; Desktop typecheck; Fast
Follow, behavior-contract, ProductSpec, Sol, AssuranceSpec baseline, and
repository-required checks.

Close rule: this packet closes only confirmed-timeline-to-command composition.
Authoritative supersession/reversion evidence, named-group authority/publication,
actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered
evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-31-20260717`
- base: `c478f1449c298716c18993ba0733a48d7a9767d2`
- worktree/branch: `openagents-ff-d1-31` / detached `origin/main`
- scope: compose one exact settled confirmed-timeline evidence authority into the existing owner-only canonical export command
- paths: the FF-D1-31 owned implementation paths above
- hot files: two new Desktop command-composition/test files; accepted-plan ledger and Sol manifest
- hot contracts: deferred exact-thread lookup, FF-D1-30 fail-closed evidence, FF-D1-11 command outcomes, unchanged persistence and receipt authority, and no host/UI ownership
- dependencies: FF-D1-30 released; no relevant open bug issue or competing claim; all audited worktrees leave the new paths unclaimed; active Desktop host/UI, T3, Full Auto, teardown, and release files explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T22:06:37Z`

### CLAIM-STATUS

- implementation: added one new-file-only Effect composition binding FF-D1-30's exact settled confirmed-timeline evidence reader to FF-D1-11's existing owner-only export command while keeping every host dependency explicit
- privacy/authority proof: invalid or unsupported intents do not read Sync; valid execution reads only the exact intent thread; unavailable source authority stays unavailable; command results expose no content, source exception, path, credential, provider payload, or host authority
- focused proof: 30/30 composition, confirmed-timeline adapter, export command/compiler, event-authority, and Sync timeline tests passed; Desktop package typecheck passed
- authority proof: Fast Follow package 13/13, behavior contracts 36/36, focused ProductSpec 107/107, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline collisions: root Fast Follow coverage is 6/7 because a separately landed teardown synthesis awaits its actively owned catalog update; AssuranceSpec remains 189/190 from only the known environment digest drift; neither baseline nor shared Git configuration was mutated
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-31-desktop-confirmed-timeline-export-command-receipt.md`
- residual: authoritative supersession/reversion export evidence, named-group authority/publication, actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim: `bde5519d5a6ce713912d6faad98bab7f9eb8ccc7`
- implementation: `efe0477078f5fea85ab11585922df4d4c7d9c1f4`
- disposition: bounded FF-D1-31 confirmed-timeline export-command composition landed, verified, and receipted; release only this packet's claim
- released_at: `2026-07-17T22:11:59Z`
- residual: authoritative supersession/reversion export evidence, named-group authority/publication, actual `main.ts` composition, renderer command/pixels, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-32 — Desktop canonical-export host runtime composition

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-31. The private
artifact store, settled confirmed-timeline command, local file transport, and
atomic two-handler composition all exist, but the host still has to reconstruct
their shared authority graph. Authoritative supersession/reversion and named-
group membership remain unavailable, while active work owns `main.ts`, preload,
renderer, installed-runtime, mobile, and teardown surfaces. This packet adds
one new-file-only Effect resource composing the existing main-process pieces
behind a single bounded lifetime.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-host-runtime.ts`
- `apps/openagents-desktop/src/thread-export-host-runtime.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-32-desktop-thread-export-host-runtime-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: one private artifact-store directory, FF-D1-31's confirmed-
timeline command, FF-D1-12's local destination transport, FF-D1-18's atomic
handler lifetime, exact trusted-sender and fixed-channel registration, and
path-free renderer results. Export schemas, store/transport/handler internals,
IPC/preload, `main.ts`, renderer, provider, named-group, server, and Sync
contracts remain unchanged.

Required behavior:

- construct one private artifact store and share its exact `persist`/`load`
  authority only with the existing command and file transport respectively;
- bind the confirmed timeline, receipt identity, observation time, digest,
  destination selection, trusted-sender, and fixed-channel registration as
  explicit host dependencies without defaults or new authority;
- acquire both existing handlers through the atomic main composition so create
  registration failure rolls back write registration and close remains reverse-
  ordered and idempotent;
- carry one valid exact-thread create through confirmed evidence, deterministic
  private persistence, receipt-only handoff, selected local write, and path-free
  result while untrusted or malformed calls fail before host effects; and
- expose only the existing close-only lifetime resource so a later `main.ts`
  packet performs one bounded acquisition and cannot reach store bytes or paths.

Proof: focused host-runtime, command composition, store, transport, handler,
compiler, authority, and Sync timeline tests; Desktop typecheck; Fast Follow,
behavior-contract, ProductSpec, Sol, AssuranceSpec baseline, and repository-
required checks.

Close rule: this packet closes only the main-process canonical-export resource
graph behind one host acquisition. Authoritative supersession/reversion
evidence, named-group authority/publication, the actual `main.ts` call site,
renderer command/pixels, installed/runtime-rendered evidence, and Day 1
completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-32-20260717`
- base: `7a71022edb76cb18201c10b061bb02c96e7ff03f`
- worktree/branch: `openagents-ff-d1-32-impl` / detached `origin/main`
- scope: compose the existing private store, confirmed export command, file transport, and atomic handlers into one close-only host resource
- paths: the FF-D1-32 owned implementation paths above
- hot files: two new Desktop host-runtime/test files; accepted-plan ledger and Sol manifest
- hot contracts: one shared private store, exact confirmed authority, receipt-only handoff, fixed channels, atomic rollback/cleanup, and no host-path projection
- dependencies: FF-D1-31 released; no relevant open bug issue or competing claim; all audited worktrees leave the new paths unclaimed; active Desktop host/UI, mobile, Full Auto, teardown, and release files explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T22:15:37Z`

### CLAIM-STATUS

- status: released after exact implementation tree landed on `origin/main`
- implementation_revision: `184871d03577707013718f823ec2d0bdd0b873de`
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-32-desktop-thread-export-host-runtime-receipt.md`
- observed_at: `2026-07-17T22:21:45Z`

### CLAIM-RELEASE

- claim_revision: `7e5140fe05bf3abfe500c895bf702fe1563b8ac3`
- implementation_revision: `184871d03577707013718f823ec2d0bdd0b873de`
- status: released; FF-D1-32 implementation is landed and the bounded claim is closed
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-32-desktop-thread-export-host-runtime-receipt.md`
- released_at: `2026-07-17T22:23:20Z`

## FF-D1-33 — Desktop canonical-export Electron host adapter

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-32. The complete
canonical-export resource graph exists, but the actual Electron host still has
to bind its fixed IPC lifetime, owner-selected save destination, private
`userData` store, confirmed timeline, receipt identity, observation time, and
digest production. Active work owns Desktop `main.ts`, preload, renderer,
installed-runtime, mobile, and teardown surfaces; authoritative named-group
membership and supersession/reversion facts remain unavailable. This packet
therefore adds only a new-file Electron-shaped adapter and its tests, leaving
the collided call site and rendered surface for later packets.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-electron-host.ts`
- `apps/openagents-desktop/src/thread-export-electron-host.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-33-desktop-thread-export-electron-host-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-32's close-only host runtime, fixed create/write IPC
channels, the existing trusted-renderer predicate, target-owned confirmed
timeline snapshots, native save-dialog selection, private `userData` custody,
and path-free renderer results. Electron imports, `main.ts`, preload, renderer,
export schemas, store/transport/handler internals, Sync contracts, named-group
authority, and server publication remain unchanged.

Required behavior:

- derive exactly one private artifact directory beneath the supplied Desktop
  `userData` root and never expose it through the returned lifetime or handler
  results;
- bind fixed-channel handler installation/removal through the supplied Electron
  IPC seam, preserving FF-D1-32 atomic rollback plus reverse-ordered,
  idempotent close;
- map one native save-dialog result into cancelled or explicitly selected JSON
  destination authority, treating a selected existing file as replacement
  authority only after the dialog returns it and rejecting malformed results;
- bind confirmed timeline reads, trusted-renderer checks, receipt UUIDs, UTC
  observation time, and SHA-256 using explicit host primitives without
  provider, credential, path, shell, or renderer authority; and
- prove one exact confirmed create-then-write journey through the adapter plus
  fail-closed untrusted, malformed-dialog, and registration-failure paths.

Proof: focused Electron-host, host-runtime, command, store, transport, handler,
compiler, authority, and Sync timeline tests; Desktop typecheck; Fast Follow,
behavior-contract, ProductSpec, Sol, AssuranceSpec baseline, and repository-
required checks.

Close rule: this packet closes only the Electron-shaped dependency adapter for
one later host acquisition. Authoritative supersession/reversion evidence,
named-group authority/publication, the actual `main.ts` call site, renderer
command/pixels, installed/runtime-rendered evidence, and Day 1 completion
remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-33-20260717`
- base: `5f921616751fbc2bf8663027f9eb38315a113cb0`
- worktree/branch: `openagents-ff-d1-33` / detached `origin/main`
- scope: bind FF-D1-32's canonical-export resource graph to explicit Electron IPC, save-dialog, private-storage, timeline, identity, time, and digest primitives
- paths: the FF-D1-33 owned implementation paths above
- hot files: two new Desktop Electron-host/test files; accepted-plan ledger and Sol manifest
- hot contracts: fixed-channel registration lifetime, owner-selected JSON replacement authority, private `userData` custody, confirmed timeline input, and path-free results
- dependencies: FF-D1-32 released; no relevant open bug issue or competing claim; active `main.ts`, preload, renderer, installed-runtime, mobile, teardown, and Full Auto work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T22:37:55Z`

### CLAIM-STATUS

- status: released after exact implementation tree landed on `origin/main`
- implementation_revision: `930cbb19980bac4e8ce5c606ebeaba98b0d54bb4`
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-33-desktop-thread-export-electron-host-receipt.md`
- observed_at: `2026-07-17T22:42:51Z`

### CLAIM-RELEASE

- claim_revision: `e94624a60a162254fda551eb4172c1f1dae3b696`
- implementation_revision: `930cbb19980bac4e8ce5c606ebeaba98b0d54bb4`
- status: released; FF-D1-33 implementation is landed and the bounded claim is closed
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-33-desktop-thread-export-electron-host-receipt.md`
- released_at: `2026-07-17T22:47:51Z`

## FF-D1-34 — Canonical accepted-event search projection

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-33. The shared
kernel already carries canonical owner-only event bundles and explicit
accepted/superseded/reverted authority, while the existing Desktop history
search ranks provider-history cache rows without canonical event authority.
Active work owns Desktop `main.ts`, renderer, installed-runtime, mobile, T3,
Full Auto, and teardown surfaces; named-group and authoritative
supersession/reversion producers remain unavailable. This packet therefore
adds only a pure shared search projection over already-authoritative canonical
bundles, leaving persistence, acquisition, transport, and pixels unchanged.

Owned implementation paths:

- `packages/agent-runtime-schema/src/thread-event-search.ts`
- `packages/agent-runtime-schema/src/thread-event-search.test.ts`
- `packages/agent-runtime-schema/src/index.ts`
- `packages/agent-runtime-schema/README.md`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-34-canonical-accepted-event-search-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-09 owner-only canonical event bundles, FF-D1-07 exact
event-authority state, bounded rebuildable local indexing, deterministic text
filtering after the search route is selected, and exact original event
navigation. No transcript, acceptance, persistence, transport, visibility,
provider, host, or renderer authority is added.

Required behavior:

- decode only owner-only canonical event bundles and project each exact
  thread/event/sequence with its existing accepted, superseded, or reverted
  state intact;
- search bounded string leaves deterministically and return the original exact
  event ref plus a bounded snippet, never a synthesized replacement event;
- preserve superseded and reverted originals in results with their exact
  replacement/revert/restored refs so navigation cannot erase history;
- reject malformed, duplicate-thread, duplicate-event, oversized, or
  conflicting projection input instead of tie-breaking or inventing authority;
- report bounded indexing truncation explicitly and grant no persistence,
  transport, disclosure, mutation, or rendering authority.

Proof: focused search, canonical-artifact, and event-authority tests; shared
package typecheck; Fast Follow, behavior-contract, ProductSpec, Sol,
AssuranceSpec baseline, and repository-required checks.

Close rule: this packet closes only the shared canonical accepted-event search
projection. Real historical acquisition/index persistence, Desktop consumption
and pixels, authoritative supersession/reversion producers, named-group
authority/publication, actual `main.ts` acquisition, installed/runtime-rendered
evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-34-20260717`
- base: `7f1dd9912fa23eed6c2c5b1015a864c041508716`
- worktree/branch: `openagents-ff-d1-34` / detached `origin/main`
- scope: bounded rebuildable search projection over exact canonical accepted events and their existing authority state
- paths: the FF-D1-34 owned implementation paths above
- hot files: two new shared schema files, the package index/README, accepted-plan ledger, Sol manifest, and packet receipt
- hot contracts: owner-only canonical bundles, exact original event identity, preserved authority state, explicit truncation, and no new authority
- dependencies: FF-D1-07 and FF-D1-09 released; FF-D1-33 released; no relevant open bug issue or competing claim; active host/UI, mobile, T3, Full Auto, teardown, and installed-runtime work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T22:59:10Z`

### CLAIM-STATUS

- implementation: added one pure shared bounded search projection over exact owner-only canonical event bundles, preserving each original accepted event's accepted, superseded, or reverted authority state
- fail-closed proof: malformed artifacts, duplicate threads/events, empty or duplicate relation identity, self-supersession, invalid revert identity, invalid query/limit bounds, and oversized artifact sets cannot produce results
- bounded proof: string-leaf and character budgets report `indexTruncated`; result limits report `resultsTruncated`; blank queries do not scan event data; returned rows contain only exact refs, sequence, authority, bounded snippet, and score
- focused proof: canonical search/artifact/authority tests 17/17 passed; agent-runtime-schema typecheck passed
- authority proof: root Fast Follow 7/7, Fast Follow package 13/13 plus typecheck/distribution, behavior contracts 36/36, ProductSpec 107/107, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline: AssuranceSpec reproduced only the recorded environment-profile digest snapshot mismatch, 189/190; no baseline, invariant, or Git configuration was mutated
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-34-canonical-accepted-event-search-receipt.md`
- residual: real historical acquisition/index persistence, Desktop consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, actual `main.ts` acquisition, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim_revision: `f41e349278de06e64b7127f0272bd9efddf1487a`
- implementation_revision: `ba7ac6e82d3b465815e9ff1957426ee0c2c89429`
- status: released; FF-D1-34 implementation is landed and the bounded claim is closed
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-34-canonical-accepted-event-search-receipt.md`
- released_at: `2026-07-17T23:11:12Z`
- residual: real historical acquisition/index persistence, Desktop consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, actual `main.ts` acquisition, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-35 — Desktop persisted canonical-event search acquisition

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-34. The shared
projection can search exact canonical events, and the private Desktop export
store already durably verifies those bundles, but no adapter acquires persisted
bundles from their ref-only export receipts for search. Active work owns
Desktop `main.ts`, renderer, installed-runtime, mobile, T3, Full Auto, and
teardown surfaces. This packet adds only a new-file acquisition adapter over
the existing private store seam, leaving host composition and pixels unchanged.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-event-search-artifact-source.ts`
- `apps/openagents-desktop/src/thread-event-search-artifact-source.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-35-desktop-canonical-event-search-artifact-source-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-08 ref-only export receipts, FF-D1-10 verified private
artifact loads, FF-D1-09 exact owner-only bundle identity, and FF-D1-34's
bounded search projection. No new persistence, transcript, acceptance,
visibility, transport, provider, host, preload, renderer, or release authority
is added.

Required behavior:

- accept only bounded canonical owner-only `export_created` receipts and
  require each artifact ref to bind its exact SHA-256 digest;
- load each unique artifact through the injected private-store seam, verify
  byte bounds, digest, UTF-8/JSON/schema, and receipt-to-artifact intent/thread/
  format/audience identity before search;
- dedupe exact receipt replays without duplicate loads, while conflicting
  receipt, artifact, or thread identity fails closed;
- feed only verified bundles into FF-D1-34 and return only its bounded
  projection or one redacted unavailable reason, never bytes, paths, receipt
  bodies, or store authority; and
- avoid all private artifact reads for a blank query.

Proof: focused acquisition, search, artifact-store, compiler, disclosure, and
authority tests; Desktop and shared-package typechecks; Fast Follow,
behavior-contract, ProductSpec, Sol, AssuranceSpec baseline, and repository-
required checks.

Close rule: this packet closes only verified acquisition from already-
persisted canonical export receipts into the shared search projection. Receipt
catalog persistence, broader historical-session ingestion, Desktop host/UI
consumption and pixels, authoritative supersession/reversion producers,
named-group authority/publication, actual `main.ts` acquisition, installed/
runtime-rendered evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-35-20260717`
- base: `13393b62306325dd1576c31ea8e29731461f6ab6`
- worktree/branch: `openagents-ff-d1-35` / detached `origin/main`
- scope: verify and acquire persisted canonical bundles through exact export receipts for FF-D1-34 search
- paths: the FF-D1-35 owned implementation paths above
- hot files: two new Desktop adapter/test files, accepted-plan ledger, Sol manifest, and packet receipt
- hot contracts: exact ref/digest receipt identity, private verified load, owner-only artifact identity, bounded search, and no byte/path projection
- dependencies: FF-D1-08, FF-D1-09, FF-D1-10, and FF-D1-34 released; no relevant open bug issue or competing claim; active host/UI, mobile, T3, Full Auto, teardown, and installed-runtime work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T23:18:04Z`

### CLAIM-STATUS

- implementation: added one Desktop acquisition adapter that accepts only exact canonical owner-only export receipts, loads each unique artifact through the injected private-store seam, re-verifies its bounded bytes/digest/schema/identity, and delegates only verified bundles to FF-D1-34 search
- fail-closed proof: malformed or oversized receipt sets, conflicting receipt-ref reuse, non-canonical or non-owner receipts, invalid ref/digest binding, unavailable/corrupt artifacts, receipt-to-artifact identity mismatch, and duplicate-thread projections cannot produce results
- bounded proof: receipt count and artifact bytes are capped; exact receipt/artifact replay dedupes before load; blank queries perform no receipt validation or private reads; returned values contain only the bounded FF-D1-34 projection or one redacted unavailable reason
- focused proof: acquisition/search/artifact-store/compiler/disclosure/authority tests 29/29 passed; Desktop and agent-runtime-schema typechecks passed
- authority proof: root Fast Follow 7/7, Fast Follow package 13/13 plus typecheck/distribution, behavior contracts 36/36, ProductSpec 107/107, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline: AssuranceSpec reproduced only the recorded environment-profile digest snapshot mismatch, 189/190; no baseline, invariant, or Git configuration was mutated
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-35-desktop-canonical-event-search-artifact-source-receipt.md`
- residual: receipt catalog persistence, broader historical-session ingestion, Desktop host/UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, actual `main.ts` acquisition, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim_revision: `8067dda00f7ed42f5cea27f3b9160ea3e3bcf292`
- implementation_revision: `dc1f35a9692ad0b7b46b221b6336c085c8162ca2`
- status: released; FF-D1-35 implementation is landed and the bounded claim is closed
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-35-desktop-canonical-event-search-artifact-source-receipt.md`
- released_at: `2026-07-17T23:26:56Z`
- residual: receipt catalog persistence, broader historical-session ingestion, Desktop host/UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, actual `main.ts` acquisition, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-36 — Desktop canonical-export receipt catalog

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-35. The persisted
artifact acquisition adapter can verify and search a bounded receipt set, but
Desktop has no durable private source for those ref-only receipts across
restart. Active work still owns Desktop `main.ts`, renderer, installed-runtime,
mobile, T3, Full Auto, and teardown surfaces. This packet therefore adds only
a new-file owner-private receipt catalog and proves its output through FF-D1-35,
leaving host composition and pixels unchanged.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-event-search-receipt-catalog.ts`
- `apps/openagents-desktop/src/thread-event-search-receipt-catalog.test.ts`
- `docs/fastfollow/receipts/2026-07-17-ff-d1-36-desktop-canonical-export-receipt-catalog-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-08 ref-only export receipts, FF-D1-10 owner-private
artifact persistence, and FF-D1-35 verified artifact acquisition. No artifact
bytes, transcript data, directory discovery, host, preload, renderer, Sync,
provider, or release authority is added.

Required behavior:

- admit only exact canonical owner-only `export_created` receipts whose
  artifact ref binds the receipt's SHA-256 digest;
- persist at most 1,000 decoded ref-only receipts in one owner-private,
  bounded, schema-versioned, atomically replaced catalog;
- make exact replay unchanged while conflicting receipt, intent, idempotency,
  or artifact identity fails closed without changing the prior catalog;
- reopen only a fully valid catalog, rejecting corruption, oversized state,
  forbidden raw fields, or partial/unknown schema rather than salvaging or
  inferring authority; and
- return only decoded ref-only receipts and bounded status metadata, never a
  path, artifact bytes, transcript content, or renderer-facing authority.

Proof: focused catalog, acquisition, artifact-store, compiler, disclosure, and
authority tests; Desktop and shared-package typechecks; Fast Follow,
behavior-contract, ProductSpec, Sol, AssuranceSpec baseline, and repository-
required checks.

Close rule: this packet closes only owner-private persistence of the exact
canonical export receipts consumed by FF-D1-35. Broader historical-session
ingestion, Desktop host/UI consumption and pixels, authoritative
supersession/reversion producers, named-group authority/publication, actual
`main.ts` acquisition, installed/runtime-rendered evidence, and Day 1
completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-36-20260717`
- base: `50486fabbdc2e9d6d5fb30a5c824ecebe1674ddc`
- worktree/branch: `openagents-ff-d1-36` / detached `origin/main`
- scope: persist the exact ref-only canonical export receipts consumed by FF-D1-35 across Desktop restart
- paths: the FF-D1-36 owned implementation paths above
- hot files: two new Desktop catalog/test files, accepted-plan ledger, Sol manifest, and packet receipt
- hot contracts: owner-private atomic persistence, exact receipt/ref/digest identity, bounded fail-closed reopen, and no artifact-byte or renderer projection
- dependencies: FF-D1-08, FF-D1-10, and FF-D1-35 released; no relevant open bug issue or competing claim; active host/UI, mobile, T3, Full Auto, teardown, and installed-runtime work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T23:37:07Z`

### CLAIM-STATUS

- implementation: added one owner-private Desktop catalog that schema-decodes and atomically persists only exact canonical owner-only export receipts, then reopens their bounded ref-only set for FF-D1-35 acquisition
- fail-closed proof: malformed/non-canonical receipts, ref/digest mismatch, conflicting receipt/intent/idempotency/artifact identity, duplicate or corrupt persisted state, unknown/extra schema fields, forbidden raw fields, invalid UTF-8/JSON, oversize, capacity overflow, and invalid storage roots cannot change or expose catalog authority
- bounded proof: the catalog holds at most 1,000 receipts and 1 MiB, exact replay does not rewrite, directory/file modes are owner-private where supported, atomic replacement leaves no path or bytes in results, and a real reopen feeds FF-D1-35 search without copying artifact content into the catalog
- focused proof: catalog/acquisition/artifact-store/compiler/disclosure/authority tests 42/42 passed; Desktop and agent-runtime-schema typechecks passed
- authority proof: root Fast Follow 7/7, Fast Follow package 13/13 plus typecheck/distribution, behavior contracts 36/36, ProductSpec 107/107, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline: AssuranceSpec reproduced only the recorded environment-profile digest snapshot mismatch, 189/190; no baseline, invariant, or Git configuration was mutated
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-36-desktop-canonical-export-receipt-catalog-receipt.md`
- residual: broader historical-session ingestion, Desktop host/UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, actual `main.ts` acquisition, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim_revision: `774a5b3908e3aac6b5626c9a0cd6e3b93356dd50`
- implementation_revision: `7bf96755257705bf404ee27c829551965f002646`
- status: released; FF-D1-36 implementation is landed and the bounded claim is closed
- receipt: `docs/fastfollow/receipts/2026-07-17-ff-d1-36-desktop-canonical-export-receipt-catalog-receipt.md`
- released_at: `2026-07-17T23:42:52Z`
- residual: broader historical-session ingestion, Desktop host/UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, actual `main.ts` acquisition, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-37 — Desktop canonical-export search-catalog ingestion

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-36. Desktop now
has a verified canonical export artifact store and a private ref-only search
receipt catalog, but successful host-runtime exports do not admit their receipt
into that catalog. Active work owns Desktop `main.ts`, renderer, installed-
runtime, Full Auto, mobile, T3, and teardown surfaces. This packet therefore
composes only the existing export host/Electron adapters with the existing
catalog, leaving the actual call site and pixels unchanged.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-host-runtime.ts`
- `apps/openagents-desktop/src/thread-export-host-runtime.test.ts`
- `apps/openagents-desktop/src/thread-export-electron-host.ts`
- `apps/openagents-desktop/src/thread-export-electron-host.test.ts`
- `docs/fastfollow/receipts/2026-07-18-ff-d1-37-desktop-canonical-export-search-catalog-ingestion-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-32 export host resource graph, FF-D1-33 Electron
`userData` boundary, FF-D1-35 verified persisted search acquisition, and
FF-D1-36 ref-only receipt catalog. No transcript, acceptance, visibility,
preload, renderer, Sync, provider, or release authority is added.

Required behavior:

- preflight the private receipt catalog before a canonical artifact write so
  corrupt catalog state cannot produce a falsely successful export;
- after a successful or unchanged artifact persist, durably record its exact
  canonical owner-only receipt before returning success;
- reconcile exact intent/idempotency retry to the original cataloged receipt
  without minting a second receipt or rewriting the catalog, while conflicting
  artifact identity fails closed;
- map catalog refusal to one path-free persistence failure and never project
  catalog paths, artifact bytes, transcript content, or partial success; and
- derive the receipt-catalog directory only beneath the validated Electron
  `userData/thread-exports` root.

Proof: focused host-runtime, Electron-host, catalog, acquisition, artifact-
store, command, compiler, disclosure, and authority tests; Desktop and shared-
package typechecks; Fast Follow, behavior-contract, ProductSpec, Sol,
AssuranceSpec baseline, and repository-required checks.

Close rule: this packet closes only automatic catalog admission for newly
created canonical exports through the already-composed host runtime. Broader
historical-session backfill, Desktop `main.ts`/UI consumption and pixels,
authoritative supersession/reversion producers, named-group authority/
publication, installed/runtime-rendered evidence, and Day 1 completion remain
later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-37-20260718`
- base: `c3dd17313e36af8e7c347d5cb56d5d4082143556`
- worktree/branch: `openagents-ff-d1-37` / detached `origin/main`
- scope: admit each successful canonical host export's exact ref-only receipt into FF-D1-36 and reconcile exact retry
- paths: the FF-D1-37 owned implementation paths above
- hot files: export host runtime/Electron adapter and their focused tests, accepted-plan ledger, Sol manifest, and packet receipt
- hot contracts: preflighted private catalog, artifact-then-receipt success boundary, original retry receipt, validated userData custody, and no partial-success projection
- dependencies: FF-D1-32, FF-D1-33, FF-D1-35, and FF-D1-36 released; no relevant open bug issue or competing claim; active `main.ts`/UI, Full Auto, mobile, T3, teardown, and installed-runtime work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-18T01:32:07Z`

### CLAIM-STATUS

- implementation: composed the canonical export host runtime with FF-D1-36 so every successful new export records its exact ref-only receipt, exact retries return the original cataloged receipt, and the Electron adapter derives both private stores beneath validated `userData`
- fail-closed proof: corrupt catalog preflight prevents artifact persistence; catalog refusal returns one path-free persistence failure; retry with conflicting artifact identity returns the existing artifact conflict; untrusted and registration-failure paths still perform no catalog/store effects
- bounded proof: the existing 1,000-receipt/1-MiB catalog remains the only receipt authority; no second index, artifact bytes, paths, transcript content, or partial-success result crosses the host boundary
- focused proof: host-runtime/Electron/catalog/acquisition/store/command/compiler/disclosure/authority tests 59/59 passed; agent-runtime-schema typecheck passed
- authority proof: root Fast Follow 7/7, Fast Follow package 13/13 plus typecheck/distribution, behavior contracts 36/36, ProductSpec 107/107, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline: Desktop typecheck reproduced only three unrelated Full Auto run-report fixture typing failures already present on the claimed `origin/main`; AssuranceSpec reproduced only the recorded environment-profile digest snapshot mismatch, 189/190; no baseline, invariant, or Git configuration was mutated
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-37-desktop-canonical-export-search-catalog-ingestion-receipt.md`
- residual: broader historical-session backfill, Desktop `main.ts`/UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim_revision: `fc39cc8fdbe901507869d5795eede1b294fb7922`
- implementation_revision: `d48e9c29529f8024df7fd498bc35bd47b5b62b5a`
- status: released; FF-D1-37 implementation is landed and the bounded claim is closed
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-37-desktop-canonical-export-search-catalog-ingestion-receipt.md`
- released_at: `2026-07-18T01:38:52Z`
- residual: broader historical-session backfill, Desktop `main.ts`/UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-38 — Desktop canonical accepted-event search bridge contract

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-37. Desktop has
a bounded accepted-event projection and verified owner-private acquisition,
but no typed renderer-to-host contract can request that projection. Historical
backfill cannot be reconstructed honestly from artifact files that lack their
original receipt identity, while active work still owns Desktop `main.ts`,
renderer, installed-runtime, Full Auto, mobile, T3, and teardown surfaces.
This packet therefore adds only a new bridge contract and focused proof;
preload exposure, handlers, host composition, UI, and pixels remain unchanged.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-event-search-bridge-contract.ts`
- `apps/openagents-desktop/src/thread-event-search-bridge-contract.test.ts`
- `docs/fastfollow/receipts/2026-07-18-ff-d1-38-desktop-canonical-event-search-bridge-contract-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-34 bounded accepted-event search projection and FF-D1-35
verified persisted artifact acquisition. No event body, artifact byte, receipt,
path, preload, handler, host, renderer, Sync, provider, or release authority is
added.

Required behavior:

- admit only an exact bounded query request with a normalized search string and
  optional integer result limit;
- invoke one fixed Desktop IPC channel and reject malformed input before any
  invocation;
- decode only the exact FF-D1-34 bounded projection or a closed set of redacted
  unavailable reasons;
- reject extra response fields so receipts, artifact bytes, paths, transcript
  content, event bodies, native errors, and partial authority cannot cross the
  bridge; and
- collapse thrown transport failures and malformed replies to one path-free
  `transport_unavailable` result.

Proof: focused bridge and accepted-event projection tests; Desktop and shared-
package typechecks; Fast Follow, behavior-contract, ProductSpec, Sol,
AssuranceSpec baseline, and repository-required checks.

Close rule: this packet closes only the typed IPC contract for requesting the
existing bounded search projection. Historical-session backfill, preload and
main-process registration, Desktop `main.ts`/UI consumption and pixels,
authoritative supersession/reversion producers, named-group authority/
publication, installed/runtime-rendered evidence, and Day 1 completion remain
later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-38-20260718`
- base: `341164e633bcfeb0098c16e80ad7effbae948fc6`
- worktree/branch: `openagents-ff-d1-38` / detached `origin/main`
- scope: define the exact typed Desktop IPC contract for FF-D1-34's bounded canonical accepted-event search projection
- paths: the FF-D1-38 owned implementation paths above
- hot files: two new Desktop bridge/test files, accepted-plan ledger, Sol manifest, and packet receipt
- hot contracts: exact bounded request, fixed channel, exact path-free projection/rejections, malformed transport collapse, and no receipt/artifact/event-body authority
- dependencies: FF-D1-34 and FF-D1-35 released; no relevant open bug issue or competing claim; active `main.ts`/UI, Full Auto, mobile, T3, teardown, and installed-runtime work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-18T01:52:49Z`

### CLAIM-STATUS

- implementation: added one typed Desktop IPC contract that normalizes an exact bounded search request, invokes one fixed channel, and schema-decodes only FF-D1-34's bounded accepted-event projection or a closed redacted unavailable result
- fail-closed proof: malformed or extra request fields prevent invocation; extra nested projection fields, receipts, artifact bytes, paths, event bodies, native errors, inconsistent counts, duplicate identity, self-supersession, invalid reversion, malformed replies, and thrown transport errors cannot cross the bridge
- bounded proof: queries are at most 200 characters, limits and result arrays at most 100, counts at most 10,000, snippets at most 240 characters, authority relations at most 1,000, and exact output keys expose no storage or disclosure authority
- focused proof: bridge and accepted-event projection tests 12/12 passed; Desktop and agent-runtime-schema typechecks passed
- authority proof: root Fast Follow 7/7, Fast Follow package 13/13 plus typecheck/distribution, behavior contracts 36/36, ProductSpec 107/107, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline: AssuranceSpec reproduced only the recorded environment-profile digest snapshot mismatch, 189/190; no baseline, invariant, or Git configuration was mutated
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-38-desktop-canonical-event-search-bridge-contract-receipt.md`
- residual: historical-session backfill, preload and main-process registration, Desktop `main.ts`/UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim_revision: `73035dd3f885703999eeeb8425edcedc12b35fdc`
- implementation_revision: `4f62498da69376ca7eb372fc7c455cc7347c73d1`
- status: released; FF-D1-38 implementation is landed and the bounded claim is closed
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-38-desktop-canonical-event-search-bridge-contract-receipt.md`
- released_at: `2026-07-18T02:00:36Z`
- residual: historical-session backfill, preload and main-process registration, Desktop `main.ts`/UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-39 — Desktop canonical accepted-event search preload exposure

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-38. The bounded
accepted-event search IPC contract is released, but the sandboxed renderer has
no method that can invoke it. Active work still owns Desktop `main.ts`,
renderer, installed-runtime, Full Auto, mobile, T3, and teardown surfaces.
`preload.cts` and the focused bridge test are unclaimed, so this packet exposes
only the existing decoded invoker through the narrow host bridge. It does not
register a handler, compose acquisition, or add UI.

Owned implementation paths:

- `apps/openagents-desktop/src/preload.cts`
- `apps/openagents-desktop/src/thread-event-search-bridge-contract.test.ts`
- `docs/fastfollow/receipts/2026-07-18-ff-d1-39-desktop-canonical-event-search-preload-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-38 fixed-channel search bridge and the sandboxed preload
capability surface. No receipt, artifact byte, path, event body, handler, host
composition, renderer, Sync, provider, or release authority is added.

Required behavior:

- expose one `threadSearch.query(value)` method through `openagentsDesktop`;
- delegate only through FF-D1-38's validated invoker and its fixed IPC channel;
- never expose raw `ipcRenderer`, channel selection, subscriptions, receipts,
  artifact bytes, filesystem paths, event bodies, or native errors;
- preserve FF-D1-38's invalid-request and transport-failure collapse; and
- build the actual sandboxed preload artifact successfully with the new import
  and method wired.

Proof: focused bridge/preload and accepted-event projection tests; Desktop
typecheck and build; shared-package typecheck; Fast Follow, behavior-contract,
ProductSpec, Sol, AssuranceSpec baseline, and repository-required checks.

Close rule: this packet closes only sandboxed preload exposure of the existing
bounded search contract. Historical-session backfill, main-process handler and
host composition, Desktop `main.ts`/UI consumption and pixels, authoritative
supersession/reversion producers, named-group authority/publication,
installed/runtime-rendered evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-39-20260718`
- base: `afe51fd0e5396a5f13bcbd83f3cabe47b48928a5`
- worktree/branch: `openagents-ff-d1-39` / detached `origin/main`
- scope: expose FF-D1-38's exact decoded canonical accepted-event search invoker through sandboxed Desktop preload
- paths: the FF-D1-39 owned implementation paths above
- hot files: sandboxed preload, focused bridge test, accepted-plan ledger, Sol manifest, and packet receipt
- hot contracts: fixed-channel decoded invocation, no raw Electron authority, and no receipt/artifact/event-body projection
- dependencies: FF-D1-38 released; no relevant open bug issue or competing claim; active `main.ts`/UI, Full Auto, mobile, T3, teardown, and installed-runtime work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-18T02:12:12Z`

### CLAIM-STATUS

- implementation: imported FF-D1-38's decoded invoker into sandboxed preload and exposed exactly one `openagentsDesktop.threadSearch.query(value)` method
- fail-closed proof: malformed input still prevents invocation; malformed replies and thrown native errors still collapse through FF-D1-38; the preload exposes no raw `ipcRenderer`, selectable channel, subscription, receipt, artifact byte, path, or event body
- built proof: the production Desktop build emitted `dist/preload.cjs` with the fixed search channel and `threadSearch.query` delegation present
- focused proof: bridge/preload and accepted-event projection tests 13/13 passed; Desktop and agent-runtime-schema typechecks passed; Desktop production build passed
- authority proof: root Fast Follow 7/7, Fast Follow package 13/13 plus typecheck/distribution, behavior contracts 36/36, ProductSpec 107/107, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline: AssuranceSpec reproduced only the recorded environment-profile digest snapshot mismatch, 189/190; no baseline, invariant, or Git configuration was mutated
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-39-desktop-canonical-event-search-preload-receipt.md`
- residual: historical-session backfill, main-process handler and host composition, Desktop `main.ts`/UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim_revision: `d91251159ce248d6e13b65cd802c4a01d0b3f1c3`
- implementation_revision: `3a614a85492959be4869e97e795d08607ad440d2`
- status: released; FF-D1-39 implementation is landed and the bounded claim is closed
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-39-desktop-canonical-event-search-preload-receipt.md`
- released_at: `2026-07-18T02:17:48Z`
- residual: historical-session backfill, main-process handler and host composition, Desktop `main.ts`/UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-40 — Desktop canonical accepted-event search main-process handler seam

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-39. Sandboxed
preload can invoke the bounded search contract, but Desktop has no trusted-
sender main-process registration seam for that fixed channel. Active work still
owns Desktop `main.ts`, renderer, installed-runtime, Full Auto, mobile, T3, and
teardown surfaces. This packet therefore adds only two new handler/test files;
actual Electron registration, acquisition composition, UI, and pixels remain
unchanged.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-event-search-main-handler.ts`
- `apps/openagents-desktop/src/thread-event-search-main-handler.test.ts`
- `docs/fastfollow/receipts/2026-07-18-ff-d1-40-desktop-canonical-event-search-main-handler-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-38 exact search request/result boundary and FF-D1-39
sandboxed preload exposure. No receipt, artifact byte, path, event body,
Electron registration, host composition, renderer, Sync, provider, or release
authority is added.

Required behavior:

- register exactly FF-D1-38's fixed search channel and remove it once on close;
- reject closed, untrusted, throwing-trust, malformed, or broader requests
  before search execution;
- pass only FF-D1-38's decoded normalized query and optional bounded limit to
  the search dependency;
- decode only exact bounded results and require an available projection's query
  to match the normalized request; and
- preserve bounded unavailable results while collapsing thrown, malformed,
  query-mismatched, or detail-leaking outcomes to `transport_unavailable`.

Proof: focused handler, bridge/preload, acquisition, and accepted-event
projection tests; Desktop and shared-package typechecks; Fast Follow,
behavior-contract, ProductSpec, Sol, AssuranceSpec baseline, and repository-
required checks.

Close rule: this packet closes only the trusted-sender main-process handler
seam. Historical-session backfill, Electron registration and acquisition host
composition, Desktop `main.ts`/UI consumption and pixels, authoritative
supersession/reversion producers, named-group authority/publication,
installed/runtime-rendered evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-40-20260718`
- base: `75b116befe78dda9bdcf1a4d378da7896f8cf793`
- worktree/branch: `openagents-ff-d1-40` / detached `origin/main`
- scope: add the trusted-sender main-process registration seam for FF-D1-38's fixed canonical accepted-event search channel
- paths: the FF-D1-40 owned implementation paths above
- hot files: two new Desktop handler/test files, accepted-plan ledger, Sol manifest, and packet receipt
- hot contracts: fixed-channel registration, trusted sender, exact normalized request/result decoding, request-bound projection query, and no receipt/artifact/event-body projection
- dependencies: FF-D1-38 and FF-D1-39 released; no relevant open bug issue or competing claim; active `main.ts`/UI, Full Auto, mobile, T3, teardown, and installed-runtime work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-18T02:31:34Z`

### CLAIM-STATUS

- implementation: added a new trusted-sender main-process handler resource that registers FF-D1-38's fixed channel, decodes its exact normalized request/result contract, and binds available projection query identity to the request
- fail-closed proof: closed, untrusted, throwing-trust, malformed, and broader requests cannot execute search; thrown, malformed, query-mismatched, receipt/path-leaking, and native-detail results collapse to path-free `transport_unavailable`
- lifecycle proof: the handler registers the fixed channel once, unregisters exactly once, and refuses post-close execution without invoking its dependency
- focused proof: handler/bridge/preload/acquisition/projection tests 25/25 passed; Desktop and agent-runtime-schema typechecks passed
- authority proof: root Fast Follow 7/7, Fast Follow package 13/13 plus typecheck/distribution, behavior contracts 36/36, ProductSpec 107/107, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline: AssuranceSpec reproduced only the recorded environment-profile digest snapshot mismatch, 189/190; no baseline, invariant, or Git configuration was mutated
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-40-desktop-canonical-event-search-main-handler-receipt.md`
- residual: historical-session backfill, Electron registration and acquisition host composition, Desktop `main.ts`/UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim_revision: `014a7aa5f0697a3a8c8a4ff1c0144f6705b30c5e`
- implementation_revision: `b9ef4b13c63d325ea9a8b1b06d500884d2903d33`
- status: released
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-40-desktop-canonical-event-search-main-handler-receipt.md`
- released_at: `2026-07-18T02:38:28Z`
- residual: historical-session backfill, Electron registration and acquisition host composition, Desktop `main.ts`/UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-41 — Desktop persisted canonical-event search host runtime composition

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-40. The fixed
trusted-sender search handler exists, but no main-process resource composes it
with the released owner-private canonical-export receipt catalog and verified
artifact loader. Active work still owns Desktop `main.ts`, renderer,
installed-runtime, Full Auto, mobile, T3, and teardown surfaces. This packet
therefore adds only a new host-runtime composition and focused proof; actual
Electron acquisition, `main.ts`, UI, pixels, and historical backfill remain
unchanged.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-event-search-host-runtime.ts`
- `apps/openagents-desktop/src/thread-event-search-host-runtime.test.ts`
- `docs/fastfollow/receipts/2026-07-18-ff-d1-41-desktop-canonical-event-search-host-runtime-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-35 verified artifact acquisition, FF-D1-36/37 canonical
receipt catalog, FF-D1-38 exact search result, and FF-D1-40 trusted handler.
No receipt, artifact byte, path, event body, Electron acquisition, renderer,
Sync, provider, or release authority is added.

Required behavior:

- compose one private artifact store and one private receipt catalog with the
  fixed FF-D1-40 handler under a close-only Effect resource;
- on each trusted bounded request, list current canonical receipts and search
  only artifacts verified by exact ref, digest, receipt, thread, and intent
  identity through FF-D1-35;
- fail catalog corruption and registration exceptions closed through bounded,
  path-free outcomes without projecting native or persistence detail;
- expose only FF-D1-38's bounded projection or closed unavailable reasons,
  never receipts, artifact bytes, filesystem paths, or native errors; and
- unregister exactly once and reject post-close calls before catalog or
  artifact access.

Proof: focused host-runtime, handler, bridge/preload, acquisition, catalog,
and accepted-event projection tests; Desktop and shared-package typechecks;
Fast Follow, behavior-contract, ProductSpec, Sol, AssuranceSpec baseline, and
repository-required checks.

Close rule: this packet closes only main-process composition of persisted
canonical-event search behind the existing handler. Historical-session
backfill, Electron registration/acquisition and `main.ts` composition, Desktop
UI consumption and pixels, authoritative supersession/reversion producers,
named-group authority/publication, installed/runtime-rendered evidence, and
Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-41-20260718`
- base: `331b86568907413f4f091a5a0ab1802991297812`
- worktree/branch: `openagents-ff-d1-41` / detached `origin/main`
- scope: compose persisted canonical-event search acquisition behind FF-D1-40's fixed trusted main handler
- paths: the FF-D1-41 owned implementation paths above
- hot files: two new Desktop host-runtime/test files, accepted-plan ledger, Sol manifest, and packet receipt
- hot contracts: exact canonical receipt listing, verified artifact acquisition, fixed handler registration, bounded path-free outcomes, and close-only lifetime
- dependencies: FF-D1-35 through FF-D1-40 released; no relevant open bug issue or competing claim; active `main.ts`/UI, Full Auto, mobile, T3, teardown, and installed-runtime work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-18T02:42:08Z`

### CLAIM-STATUS

- implementation: composed the private canonical-export store and receipt catalog behind FF-D1-40's fixed trusted handler and FF-D1-35's verified artifact search
- fail-closed proof: corrupt catalog state and native registration errors project only bounded path-free outcomes; untrusted and post-close requests cannot reach private acquisition
- lifecycle proof: the fixed handler registers once, unregisters exactly once, and the returned resource projects no receipt, artifact byte, store path, or native authority
- focused proof: host-runtime/handler/bridge/acquisition/catalog tests 28/28 passed; Desktop and agent-runtime-schema typechecks passed
- authority proof: root Fast Follow 7/7, Fast Follow package 13/13, behavior contracts 36/36, ProductSpec 107/107, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline: AssuranceSpec reproduced only the recorded environment-profile digest snapshot mismatch; no baseline, invariant, or Git configuration was mutated
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-41-desktop-canonical-event-search-host-runtime-receipt.md`
- residual: historical-session backfill, Electron registration/acquisition and `main.ts` composition, Desktop UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim_revision: `7c96378c0c049969890c023d9b7239085ac19701`
- implementation_revision: `def57a54b497b4820bfc68295afc5d00e4ac36e4`
- status: released
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-41-desktop-canonical-event-search-host-runtime-receipt.md`
- released_at: `2026-07-18T02:49:09Z`
- residual: historical-session backfill, Electron registration/acquisition and `main.ts` composition, Desktop UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-42 — Desktop canonical-event search Electron host acquisition

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-41. Persisted
canonical-event search is composed behind a close-only main-process resource,
but no Electron host boundary validates Desktop user-data placement or binds
that resource to fixed handle/remove seams. Active work still owns Desktop
`main.ts`, renderer, installed-runtime, Full Auto, mobile, T3, and teardown
surfaces. This packet therefore adds only a new Electron-host adapter and
focused proof; the actual `main.ts` call site, UI, pixels, and historical
backfill remain unchanged.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-event-search-electron-host.ts`
- `apps/openagents-desktop/src/thread-event-search-electron-host.test.ts`
- `docs/fastfollow/receipts/2026-07-18-ff-d1-42-desktop-canonical-event-search-electron-host-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-41 search runtime, Desktop's owner-private
`thread-exports/{artifacts,search-receipts}` layout, and fixed Electron
handle/remove lifecycle. No receipt, artifact byte, path, event body, main
call-site, renderer, Sync, provider, or release authority is added.

Required behavior:

- accept only an absolute, non-root, NUL-free Desktop user-data directory and
  derive the existing private artifact and search-receipt directories beneath
  it;
- bind FF-D1-41 to exactly its fixed search channel through injected Electron
  handle/remove seams and the existing trusted-sender predicate;
- reject unsafe user-data before registration or private acquisition;
- map registration failure through FF-D1-41's typed, path-free failure and
  never project native detail; and
- remove the fixed handler exactly once, with post-close calls rejected before
  receipt or artifact access.

Proof: focused Electron-host, host-runtime, handler, bridge/preload,
acquisition, catalog, and accepted-event projection tests; Desktop and shared-
package typechecks; Fast Follow, behavior-contract, ProductSpec, Sol,
AssuranceSpec baseline, and repository-required checks.

Close rule: this packet closes only safe Electron acquisition and registration
of the composed canonical-event search resource. Historical-session backfill,
the actual Desktop `main.ts` call site, Desktop UI consumption and pixels,
authoritative supersession/reversion producers, named-group authority/
publication, installed/runtime-rendered evidence, and Day 1 completion remain
later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-42-20260718`
- base: `d42c98c066862abefe73a44d5270d2168e17ad41`
- worktree/branch: `openagents-ff-d1-42` / detached `origin/main`
- scope: bind FF-D1-41 persisted canonical-event search to validated Desktop user-data and fixed Electron handle/remove seams
- paths: the FF-D1-42 owned implementation paths above
- hot files: two new Desktop Electron-host/test files, accepted-plan ledger, Sol manifest, and packet receipt
- hot contracts: private thread-export directory derivation, fixed-channel Electron registration, trusted sender, typed registration failure, and close-only lifetime
- dependencies: FF-D1-35 through FF-D1-41 released; no relevant open bug issue or competing claim; active `main.ts`/UI, Full Auto, mobile, T3, teardown, and installed-runtime work explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-18T03:01:26Z`

### CLAIM-STATUS

- implementation: bound FF-D1-41's persisted canonical-event search resource to validated Desktop user-data placement and fixed Electron handle/remove seams
- fail-closed proof: relative, root, and NUL-bearing user-data are rejected before registration; native registration detail remains inside D41's typed path-free failure
- lifecycle proof: the fixed handler registers once, removes exactly once, and untrusted or post-close calls cannot reach private receipt or artifact acquisition
- focused proof: Electron-host/runtime/handler/bridge/acquisition/catalog tests 32/32 passed; Desktop and agent-runtime-schema typechecks passed
- authority proof: root Fast Follow 7/7, Fast Follow package 13/13, behavior contracts 36/36, ProductSpec 107/107, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline: AssuranceSpec compiler reproduced only the recorded environment-profile digest snapshot mismatch, 5/6; no baseline, invariant, or Git configuration was mutated
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-42-desktop-canonical-event-search-electron-host-receipt.md`
- residual: historical-session backfill, the actual Desktop `main.ts` call site, Desktop UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim_revision: `3b8268ed91d1cbcf6a44422da73c8ac7bcdfad64`
- implementation_revision: `fe7b4aebf2ae1da4e77052a2850f28eb4d114d6c`
- status: released
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-42-desktop-canonical-event-search-electron-host-receipt.md`
- released_at: `2026-07-18T03:05:39Z`
- residual: historical-session backfill, the actual Desktop `main.ts` call site, Desktop UI consumption and pixels, authoritative supersession/reversion producers, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-43 — Desktop terminal thread-event authority relation ledger

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-42. Canonical
search can preserve superseded and reverted authority already present in an
export artifact, but Desktop has no private durable boundary for terminal
authority relations observed by a later real producer. The actual `main.ts`
and renderer surfaces remain actively owned, while historical exports without
their original receipt identity cannot be backfilled honestly. This packet
therefore adds only a new owner-private terminal-relation ledger and focused
proof. It does not create, infer, authorize, or compose an authority producer.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-event-authority-relation-ledger.ts`
- `apps/openagents-desktop/src/thread-event-authority-relation-ledger.test.ts`
- `docs/fastfollow/receipts/2026-07-18-ff-d1-43-desktop-thread-event-authority-relation-ledger-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.thread_event_authority.v1`, the accepted-then-one-
terminal relation history, owner-private atomic persistence, and no invented
authority. No accepted fact, event body, receipt, artifact byte, path,
producer, host, renderer, Sync, provider, or release authority is added.

Required behavior:

- admit only exact ref-only `superseded` or `reverted` relations decoded by
  the shared authority schema; accepted facts remain owned by the confirmed
  timeline source;
- persist at most one terminal relation for an exact thread/event identity in
  a bounded owner-private atomic catalog;
- make exact replay idempotent while rejecting conflicting relation refs,
  terminal transitions, malformed input, extra fields, capacity overflow, and
  corrupt persisted state;
- list only exact decoded terminal relations for one validated thread ref,
  with deterministic order and no path, body, summary, prompt, provider, or
  native-error projection; and
- reopen identical state after restart with owner-private directory and file
  permissions where the platform supports them.

Proof: focused relation-ledger and shared authority/search/export tests;
Desktop and shared-package typechecks; Fast Follow, behavior-contract,
ProductSpec, Sol, AssuranceSpec baseline, and repository-required checks.

Close rule: this packet closes only private persistence for already observed
terminal authority relations. Real supersession/reversion observation and
producer composition, merging those facts with confirmed accepted timelines,
historical-session backfill, the actual Desktop `main.ts` call site, Desktop
UI consumption and pixels, named-group authority/publication,
installed/runtime-rendered evidence, and Day 1 completion remain later
packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-43-20260718`
- base: `de18ab353fe45aa726080deb2216c67a2e521fab`
- worktree/branch: `openagents-ff-d1-43` / detached `origin/main`
- scope: persist exact already-observed superseded/reverted thread-event authority relations without inventing producer authority
- paths: the FF-D1-43 owned implementation paths above
- hot files: two new Desktop ledger/test files, accepted-plan ledger, Sol manifest, and packet receipt
- hot contracts: shared authority v1 exact decoding, accepted-then-one-terminal history, deterministic private persistence, and no inferred authority
- dependencies: FF-D1-23/31 authority schema/projection and FF-D1-42 search host released; no relevant open bug issue or competing claim; active `main.ts`/UI and unavailable historical receipt identity explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-18T03:27:35Z`

### CLAIM-STATUS

- implementation: added a bounded owner-private atomic ledger for exact already-observed superseded/reverted authority-v1 relations
- fail-closed proof: accepted, malformed, extra-field, self-referential, conflicting, over-capacity, and corrupt inputs are refused without rewriting valid state or projecting native detail
- lifecycle proof: exact replay is unchanged; state reopens identically with deterministic per-thread order and private permissions where supported
- authority boundary: the ledger validates and persists supplied terminal facts but does not observe, infer, authorize, or produce them; confirmed accepted facts remain source-owned
- focused proof: ledger/shared-authority/export/search tests 36/36 passed; Desktop and agent-runtime-schema typechecks passed
- authority proof: root Fast Follow 7/7, Fast Follow package 13/13, behavior contracts 36/36, ProductSpec 107/107, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline: AssuranceSpec compiler reproduced only the recorded environment-profile digest snapshot mismatch, 5/6; no baseline, invariant, or Git configuration was mutated
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-43-desktop-thread-event-authority-relation-ledger-receipt.md`
- residual: real supersession/reversion observation and producer composition, merging those facts with confirmed accepted timelines, historical-session backfill, the actual Desktop `main.ts` call site, Desktop UI consumption and pixels, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim_revision: `c0f52d55c6a5b7f573fabb120f3a3ba8eedf36cc`
- implementation_revision: `8588604915502e5a1689de71dba2f6c889a7cb5c`
- status: released
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-43-desktop-thread-event-authority-relation-ledger-receipt.md`
- released_at: `2026-07-18T03:36:06Z`
- residual: real supersession/reversion observation and producer composition, merging those facts with confirmed accepted timelines, historical-session backfill, the actual Desktop `main.ts` call site, Desktop UI consumption and pixels, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-44 — Desktop confirmed-timeline terminal-authority overlay

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-43. Desktop now
has separate confirmed accepted evidence and a private ledger for exact
terminal facts observed elsewhere, but canonical export cannot yet consume the
two sources as one validated history. The actual producer, `main.ts`, and
renderer surfaces remain outside this packet. This packet therefore adds only
a read-side overlay and focused proof. It does not observe, create, infer, or
authorize any authority fact.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-terminal-authority-overlay.ts`
- `apps/openagents-desktop/src/thread-export-terminal-authority-overlay.test.ts`
- `docs/fastfollow/receipts/2026-07-18-ff-d1-44-desktop-confirmed-timeline-terminal-authority-overlay-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: `openagents.thread_event_authority.v1`, the accepted-then-one-
terminal history, confirmed-timeline evidence, and no invented authority. No
schema, producer, host composition, renderer, Sync, event body, receipt,
artifact byte, path, provider, or release authority is added.

Required behavior:

- start only from an available target-owned confirmed timeline and its exact
  accepted relations;
- read only exact terminal relations already retained by the FF-D1-43 private
  ledger for the same validated thread;
- require every terminal relation event and every referenced successor,
  reversion, or restoration event to exist in the confirmed timeline;
- validate each merged accepted-then-terminal history through the shared
  authority projection and fail closed on incomplete, conflicting, corrupt,
  or invalid evidence; and
- return deterministic ref-only canonical-export evidence without projecting
  paths, native errors, bodies, prompts, providers, or producer claims.

Proof: focused overlay, ledger, shared authority, export, and search tests;
Desktop and shared-package typechecks; Fast Follow, behavior-contract,
ProductSpec, Sol, AssuranceSpec baseline, and repository-required checks.

Close rule: this packet closes only read-side merging of already-observed
terminal facts with confirmed accepted evidence. Real supersession/reversion
observation and producer composition, historical-session backfill, the actual
Desktop `main.ts` call site, Desktop UI consumption and pixels, named-group
authority/publication, installed/runtime-rendered evidence, and Day 1
completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-44-20260718`
- base: `a675d42f781fc7f54d209daf458a76a9dc198f98`
- worktree/branch: `openagents-ff-d1-44` / detached `origin/main`
- scope: merge exact already-observed terminal authority relations into confirmed accepted export evidence without inventing producer authority
- paths: the FF-D1-44 owned implementation paths above
- hot files: two new Desktop overlay/test files, accepted-plan ledger, Sol manifest, and packet receipt
- hot contracts: shared authority v1 accepted-then-one-terminal projection, confirmed-timeline evidence, deterministic ref closure, and no inferred authority
- dependencies: FF-D1-31 projection, FF-D1-33 confirmed timeline evidence, and FF-D1-43 terminal ledger released; no relevant open bug issue or competing claim; active `main.ts`/UI and unavailable historical receipt identity explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-18T03:43:33Z`

### CLAIM-STATUS

- implementation: added a read-only overlay that joins exact already-observed terminal authority relations to target-owned confirmed accepted evidence
- fail-closed proof: missing event refs, missing successor/reversion/restoration refs, corrupt ledger state, unsafe thread refs, and terminal-before-accepted histories remain unavailable
- compiler proof: complete ref-closed histories drive the existing canonical compiler to exact superseded and reverted states without body, path, prompt, provider, or native-error projection
- authority boundary: the overlay consumes but does not observe, infer, authorize, create, persist, or publish terminal facts; real producer composition remains absent
- focused proof: overlay/ledger/shared-authority/export/search tests 34/34 passed; Desktop and agent-runtime-schema typechecks passed
- authority proof: root Fast Follow 7/7, Fast Follow package 13/13, behavior contracts 36/36, ProductSpec 107/107, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline: AssuranceSpec compiler reproduced only the recorded environment-profile digest snapshot mismatch, 5/6; no baseline, invariant, or Git configuration was mutated
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-44-desktop-confirmed-timeline-terminal-authority-overlay-receipt.md`
- residual: real supersession/reversion observation and producer composition, historical-session backfill, the actual Desktop `main.ts` call site, Desktop UI consumption and pixels, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

### CLAIM-RELEASE

- claim_revision: `76b94796b2e76b1b414c3383958871b6a73c2b18`
- implementation_revision: `adcdd9c348fe158ad2a717882f08a60bcef1637e`
- status: released
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-44-desktop-confirmed-timeline-terminal-authority-overlay-receipt.md`
- released_at: `2026-07-18T03:49:34Z`
- residual: real supersession/reversion observation and producer composition, historical-session backfill, the actual Desktop `main.ts` call site, Desktop UI consumption and pixels, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## FF-D1-45 — Desktop terminal-authority export resource composition

Status: claimed implementation packet; not a Day 1 completion claim.

This packet is the next unblocked Day 1 residual after FF-D1-44. The validated
terminal-authority overlay exists, but the production canonical-export command
and Electron host resource graph still call the earlier accepted-only reader,
so already-observed terminal facts cannot reach an actual export. This packet
composes the existing overlay and private ledger directory into that resource
graph. It does not add, infer, or claim a terminal-fact producer.

Owned implementation paths:

- `apps/openagents-desktop/src/thread-export-confirmed-timeline-command.ts`
- `apps/openagents-desktop/src/thread-export-confirmed-timeline-command.test.ts`
- `apps/openagents-desktop/src/thread-export-host-runtime.ts`
- `apps/openagents-desktop/src/thread-export-host-runtime.test.ts`
- `apps/openagents-desktop/src/thread-export-electron-host.ts`
- `apps/openagents-desktop/src/thread-export-electron-host.test.ts`
- `docs/fastfollow/receipts/2026-07-18-ff-d1-45-desktop-terminal-authority-export-composition-receipt.md`
- this accepted-plan ledger and `docs/sol/document-manifest.json`

Hot contracts: FF-D1-31 confirmed-timeline command composition, FF-D1-32/33
private host resource graph, FF-D1-43 terminal ledger, FF-D1-44 overlay, and
no invented authority. No schema, producer, `main.ts`, renderer, Sync,
disclosure audience, artifact format, provider, or release authority is added.

Required behavior:

- make the canonical-export command read evidence through FF-D1-44 rather
  than bypassing it through the accepted-only adapter;
- require an explicit private terminal-authority ledger directory throughout
  command and host composition;
- derive that directory only beneath validated Electron
  `userData/thread-exports` placement beside the existing artifact and receipt
  stores;
- preserve accepted-only output when no terminal facts exist and produce exact
  terminal authority only for complete already-observed histories; and
- fail closed on corrupt or invalid terminal evidence without writing an
  artifact, catalog receipt, or destination and without projecting a path or
  native error.

Proof: focused command, overlay, host, Electron, ledger, artifact, and search
tests; Desktop and shared-package typechecks; Fast Follow,
behavior-contract, ProductSpec, Sol, AssuranceSpec baseline, and repository-
required checks.

Close rule: this packet closes only production resource composition for
consuming already-observed terminal facts during canonical export. Real
supersession/reversion observation and producer composition, historical-
session backfill, the actual Desktop `main.ts` call site, Desktop UI
consumption and pixels, named-group authority/publication, installed/runtime-
rendered evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-45-20260718`
- base: `73c19ea3ee9db967deb65dbdf94d93098fa94318`
- worktree/branch: `openagents-ff-d1-45` / detached `origin/main`
- scope: compose the FF-D1-44 terminal-authority overlay and FF-D1-43 private ledger into the actual canonical-export command/host/Electron resource graph
- paths: the FF-D1-45 owned implementation paths above
- hot files: six existing Desktop command/host/Electron files and focused tests, accepted-plan ledger, Sol manifest, and packet receipt
- hot contracts: accepted-then-terminal evidence, private ledger placement, export resource lifetime, fail-closed persistence, and no inferred producer authority
- dependencies: FF-D1-31 through FF-D1-33 and FF-D1-43/44 released; no relevant open bug issue or competing claim; active `main.ts`/renderer work and unavailable historical receipt identity explicitly excluded
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-18T04:03:56Z`

### CLAIM-STATUS

- implementation: composed FF-D1-44 into the actual canonical-export command and carried its private authority-ledger directory through host and Electron resource acquisition
- end-to-end proof: a ledger-retained supersession reaches the owner-selected canonical artifact through fixed IPC while an empty ledger preserves accepted-only output
- fail-closed proof: corrupt terminal state returns evidence unavailable before artifact, receipt-catalog, or destination effects and projects no path or native error
- authority boundary: the resource graph consumes already-observed facts but does not observe, infer, authorize, create, or publish them; real producer composition remains absent
- focused proof: command/overlay/host/Electron/ledger/artifact/search tests 42/42 passed; Desktop and agent-runtime-schema typechecks passed
- authority proof: root Fast Follow 7/7, Fast Follow package 13/13, behavior contracts 36/36, ProductSpec 107/107, Sol 19/19, `pnpm run check`, and `pnpm run check:fast` passed
- baseline: AssuranceSpec compiler reproduced only the recorded environment-profile digest snapshot mismatch, 5/6; no baseline, invariant, or Git configuration was mutated
- receipt: `docs/fastfollow/receipts/2026-07-18-ff-d1-45-desktop-terminal-authority-export-composition-receipt.md`
- residual: real supersession/reversion observation and producer composition, historical-session backfill, the actual Desktop `main.ts` call site, Desktop UI consumption and pixels, named-group authority/publication, installed/runtime-rendered evidence, and Day 1 completion remain unclaimed

## Explicit non-authority

This plan grants no deployment, release, paid-provider spend, credential,
settlement, public-promise, cross-tenant sharing, or invariant-bypass
authority. It does not revive deprecated clients or authorize external-source
mutation. Those actions retain their own gates.
