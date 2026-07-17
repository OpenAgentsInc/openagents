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

## Explicit non-authority

This plan grants no deployment, release, paid-provider spend, credential,
settlement, public-promise, cross-tenant sharing, or invariant-bypass
authority. It does not revive deprecated clients or authorize external-source
mutation. Those actions retain their own gates.
