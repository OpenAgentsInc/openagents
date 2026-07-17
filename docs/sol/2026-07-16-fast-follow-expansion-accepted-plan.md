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

## Explicit non-authority

This plan grants no deployment, release, paid-provider spend, credential,
settlement, public-promise, cross-tenant sharing, or invariant-bypass
authority. It does not revive deprecated clients or authorize external-source
mutation. Those actions retain their own gates.
