# R1–R2 identity and Khala Sync contract — `khala.identity_sync_contract.v1`

- Date: 2026-07-10
- Status: **published senior contract freeze** for the R1–R2 gates
  (MASTER_ROADMAP Revision 24/25, owner decisions 1, 5, 7; execution-order
  step 2). Sections are individually marked **SETTLED** (frozen — build against
  it) or **DRAFT** (bounded open question — do not invent; wait for the named
  follow-up or escalate to Sol).
- Authority: [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md) (R0–R7 gates, "Khala
  Sync laws for this program"),
  [`2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md`](./2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md)
  ("R1–R2 contract freeze: senior decision required"),
  [`2026-07-10-terra-execution-lane.md`](./2026-07-10-terra-execution-lane.md).
- Integration owner: **Sol**. Schema/migration/catalog changes under this
  contract remain serialized through one lane per the claim protocol.
- Consumers: #8574 Desktop (Terra), #8597 mobile, #8566 parent, #8638 Fleet
  substrate; the SYNC-1…SYNC-4 delegation leaves.

## 0. Non-invention rule (SETTLED)

This contract **binds to the substrate that already exists on `main`**. The
schema source of truth is code, not this prose:

| Concern | Canonical package / path |
| --- | --- |
| Wire protocol, scopes, cursors, mutation envelope | `@openagentsinc/khala-sync` — `packages/khala-sync/src/index.ts` (`KHALA_SYNC_PROTOCOL_VERSION = 1`) |
| Fleet entity schemas | `packages/khala-sync/src/fleet.ts` |
| Conversation entity schemas | `packages/khala-sync/src/chat.ts` |
| Client session/store/transport/offline queue | `@openagentsinc/khala-sync-client` — `packages/khala-sync-client/src/` (`session.ts`, `store.ts`, `sqlite-store.ts`, `overlay.ts`, `transport.ts`) |
| Server mutators, CVR service, projections | `@openagentsinc/khala-sync-server` — `packages/khala-sync-server/src/` (`chat-mutators.ts`, `fleet-mutators.ts`, `fleet-projection.ts`, `fleet-run-authority.ts`, `fleet-steering-exchange.ts`, `cvr-service.ts`) |
| Worker/fleet-run/account/worker vocabulary | `@openagentsinc/khala-fleet-intents` — `packages/khala-fleet-intents/src/index.ts` |
| Identity/session resolution (server) | `apps/openagents.com/workers/api/src/auth-cookies.ts`, `apps/openagents.com/workers/api/src/auth/session.ts`, `apps/openagents.com/workers/api/src/auth/mobile-session.ts`, `apps/openagents.com/workers/api/src/auth/bearer-token.ts` |
| TanStack DB collection adapter | `@openagentsinc/khala-sync-db-collection` |

No new sync engine, no app-local fleet schema, no parallel identity store.
Where this document and the code disagree, the code on `main` wins and this
document must be reconciled — a client never "fixes" the disagreement locally.

## R1 — one authenticated identity

### R1.1 Canonical identity model (SETTLED)

- The canonical identity is the **OpenAuth user id** resolved **server-side**
  by the openagents.com API. Every projection row, mutation, and command
  outcome is owner-scoped to that server-derived id (`actor_user_id` /
  `owner_user_id` in durable rows). Clients transmit credentials; they never
  transmit or assert an identity claim of their own.
- Owner scope in Sync is expressed as the personal scope
  `scope.user.<userId>` (`personalScope` in `packages/khala-sync/src/index.ts`;
  `isScopeCompatibleUserId` guards embeddability). Additional scope kinds are
  `scope.team.<id>`, `scope.thread.<id>`, `scope.fleet_run.<id>`,
  `scope.agent_run.<id>`, `scope.public.<channel>`.

### R1.2 Token posture per client class (SETTLED)

| Client class | Credential | Refresh | Source |
| --- | --- | --- | --- |
| Web/browser | HttpOnly OpenAuth cookies `oa_access` + `oa_refresh` (`ACCESS_COOKIE`/`REFRESH_COOKIE`) | Server refreshes and propagates rotated cookies back on responses (`appendSessionCookies`); clear on sign-out (`appendClearSessionCookies`) | `auth-cookies.ts`, `auth/session.ts` (`makeBrowserSessionBoundary`, `VerifiedSession`) |
| Mobile (native) | User **bearer** session against the mobile OpenAuth client (`mobileOpenAuthClientId`), tokens held in platform secure storage only | `makeUserBearerSessionBoundary` / `requireUserBearerSession`; refresh via stored OpenAuth refresh token; revocation via `revokeMobileAccessToken` / `revokeOpenAuthRefreshToken`; deletion receipts via `hasMobileAccountDeletionReceipt` | `auth/mobile-session.ts` |
| Desktop (native/Electron) | Same **user bearer session class as mobile** (a native OpenAgents client, not a browser). Tokens live in the **main process/OS keychain only**; the renderer never sees a token. | Same boundary as mobile | `auth/mobile-session.ts` boundary reused; desktop keychain wiring is new work (§R1.5) |
| Agent/machine | `OPENAGENTS_AGENT_TOKEN` bearer; registered-Pylon bearer for Pylon claim/steering routes | Out of R1 client scope; unchanged | `auth/bearer-token.ts`, Pylon routes |

Provider credentials (Codex/Claude/Grok device auth, GitHub) are **not**
OpenAgents identity. Desktop's existing Codex device-auth Settings flow binds
a provider account to a Pylon home; it neither creates nor substitutes for the
OpenAgents session above.

### R1.3 Fail-closed identity laws (SETTLED)

1. **No device-local identity invention.** A client with no verified session
   renders `unconfigured`/signed-out state. It never mints, guesses, caches
   into validity, or copies an identity from another store.
2. **Identity changes are server events.** Sign-in, refresh rotation,
   revocation, and account deletion are decided by the API. Clients observe
   them: a revoked/expired session produces a typed `denied` session phase and,
   on Sync, `must_refetch` with reason `access_changed` — never a silent
   continue on cached rows.
3. **Revocation purges.** On `denied`/`access_changed`, the device-local cache
   for the revoked owner scope is invalidated (purge or mark-unreadable) before
   any UI renders it again. Fail closed: if purge fails, render nothing.
4. **No credential crosses the renderer/public boundary.** Cookies, bearer
   tokens, refresh tokens, and provider credentials never appear in renderer
   processes, Sync rows, logs, receipts, fixtures, or issue comments.

### R1.4 Session/device catalog (DRAFT — enumerated follow-on)

R1 exit evidence requires "see the same authorized account/session catalog,
revoke either device." The projection for that catalog does not exist yet.
Reserved (do not implement until Sol freezes the entity schema in
`packages/khala-sync`):

- entity type `device_session` in the owner's personal scope: fields
  (sessionRef, clientClass `web | mobile | desktop | agent`, createdAt,
  lastSeenAt, revokedAt?) — refs only, no token material;
- mutation `identity.revokeSession` (rejections: `unauthorized_scope`,
  `session_not_found`), which must chain into the existing revocation
  primitives in `auth/mobile-session.ts`.

### R1.5 What must be built for R1 (SETTLED as scope)

- Desktop OpenAgents sign-in (bearer-session boundary + OS keychain custody in
  the main process; renderer gets a typed session-phase projection only).
- Mobile sign-in against `mobileOpenAuthClientId` with secure-storage recovery
  (port the pattern from frozen `clients/khala-mobile` — pattern only, not the
  component tree).
- The `device_session` projection/mutator pair (after §R1.4 freeze).

## R2 — Khala Sync as the cross-device authority

### R2.1 Authority law (SETTLED — restates roadmap law, binding here)

Server/Pylon authority decides claims, attempts, worker custody, approvals,
command acceptance, and terminal outcomes. Khala Sync distributes typed
projections and mutation results. Device-local stores are **caches and offline
queues only**; they never become a second authority, and no client renders
cache, optimistic, transcript, or fixture state as accepted authority.

### R2.2 Protocol and freshness semantics (SETTLED)

All names below are exported from `@openagentsinc/khala-sync` unless noted.

- **Protocol version:** `KHALA_SYNC_PROTOCOL_VERSION = 1`; schema evolution
  rides `SyncSchemaVersion`. An unsupported version yields `must_refetch`
  reason `schema_version_unsupported` (client stops rendering stale shape and
  re-bootstraps or fails closed).
- **Routes (openagents.com API):** `/api/sync/bootstrap`, `/api/sync/connect`
  (live stream), `/api/sync/cvr-pull` (`CvrPullMode = "reset" | "diff"`),
  `/api/sync/push` (mutations), `/api/sync/log`.
- **Versioning:** per-scope monotonic `SyncVersion` with `SyncVersionWatermark`
  cursors; CVR (`CvrVersion`) for pull-diff reconciliation.
- **Tombstones:** `ChangeOp = "upsert" | "delete"`; deletes are explicit
  tombstoned changes, replayed to caches — a client never infers deletion from
  absence.
- **Freshness proof:** a client may render a row as live only while its session
  phase is live and the row's scope watermark is current. Client session phases
  are the typed set already used by the client package and R0:
  `bootstrapping | catching_up | live | stale | must_refetch | denied |
  unavailable` (plus `reconnecting` at transport level). 30 seconds without
  freshness is `stale`/`reconnecting`, never indefinite live.
- **Gap handling:** reconnect resumes from the exact retained cursor. A cursor
  behind the retained window is `MustRefetchReason =
  "cursor_behind_retained_window"`; access revocation is `"access_changed"`;
  server-side scope invalidation is `"scope_reset"`. All four reasons force a
  bootstrap-quality refetch; none may be papered over with transcript inference
  or last-write-wins.

### R2.3 Mutation-outcome contract (SETTLED)

- Every client mutation carries: owner scope, `MutatorName`, target ref,
  args (schema-decoded), and idempotency identity `ClientGroupId` + `ClientId`
  + monotonically increasing `MutationId` (`LastMutationId` acknowledgement).
- The server mutator is the only writer. The durable result is
  `MutationStatus = "applied" | "rejected" | "duplicate"` with a typed
  rejection reason. `duplicate` is a successful idempotent replay, not an
  error.
- **Conflict is explicit typed state**: expected-version or precondition
  failures surface as `rejected` with the mutator's named rejection (e.g.
  `unauthorized_scope`, `thread_exists`, `thread_not_found`, `message_exists`,
  `confirmation_required`) and the client must reconcile from projections —
  never merge locally on authority-bearing fields.
- **A transport timeout is not an outcome.** Client-side, an unacknowledged
  push is `unknown_pending_reconcile`: the mutation stays in the offline queue,
  is re-pushed with the same `MutationId`, and the UI shows pending —
  never success, never an automatic unsafe re-issue under a new id.
- Optimistic overlay (`packages/khala-sync-client/src/overlay.ts`) is visually
  distinct and reversible. Approval, stop, payment, claim, credential, and
  destructive actions are never displayed committed before authoritative
  acknowledgment.
- Fleet **commands** additionally have durable server outcome rows
  (`fleet_command_outcome`) distinguishing delivery from completion:
  `FleetCommandDeliveryOutcome`, `FleetCommandKind`, and
  `FleetCommandEffectiveOutcome` in `packages/khala-sync/src/fleet.ts`. A
  client renders effective run/approval/steer state only from these rows.

### R2.4 P0 projection catalog (SETTLED — names frozen)

P0 scope set: `scope.user.<userId>` (personal catalog),
`scope.thread.<threadId>` (conversation), `scope.fleet_run.<fleetRunId>`
(fleet supervision).

**Conversations (P0):**

| Entity type | Schema (source of truth) |
| --- | --- |
| `chat_thread` | `ChatThreadEntity` — incl. `ChatThreadRepoBinding` (`chat.bindThreadRepo`) and Codex continuity pin (`ChatThreadCodexContinuityPin`) in `packages/khala-sync/src/chat.ts` |
| `chat_message` | `ChatMessageEntity` (same file) |

**Fleet supervision (P0)** — all in `packages/khala-sync/src/fleet.ts`,
`FLEET_ENTITY_TYPES`:

| Entity type | Schema highlights |
| --- | --- |
| `fleet_run` | `FleetRunStatus`, refs via `FleetPublicRef` |
| `fleet_worker` | `FleetWorkerKind` (`codex\|claude\|grok\|auto`), `FleetWorkerPhase` |
| `fleet_assignment` | assignment edge, account via `FleetAccountRefHash` (never raw account refs) |
| `fleet_work_unit` | `FleetWorkUnitEntity`, `FleetWorkUnitState` |
| `fleet_attempt` | `FleetAttemptEntity`, `FleetAttemptState`, `FleetAttemptVerification`, usage = `FleetAttemptExactUsageEvidence` **or** `FleetAttemptNotMeasuredUsageEvidence` (never inferred) |
| `fleet_account` | `FleetAccountReadiness` |
| `fleet_inbox_flag` | `FleetInboxFlagStatus` (`open\|acknowledged`) |
| `fleet_approval` | `FleetApprovalEntity`, `FleetApprovalStatus` (`pending\|allowed\|denied`); actionable only with exact binding (`fleetApprovalHasExactBinding`) — legacy unbound approvals render visible but non-actionable |
| `fleet_steer` | steer request/delivery projection |
| `fleet_command_outcome` | see §R2.3 |

**Enumerated follow-ons (names reserved, NOT P0):** `agent_run`,
`agent_run_event` (`packages/khala-sync/src/agent-run.ts`), `credit_balance`,
`activity_timeline_snapshot`, `gym_run_progress`; and **DRAFT — new entity
schemas required later by R3–R7, to be frozen by Sol before implementation:**
`project`/`session` (workbench identity beyond `chat_thread`),
`workroom` (remote-workroom lifecycle, #8547/#8636 M3 freeze), `preview`,
`artifact`, `writeback`, `receipt`, `device_session` (§R1.4).

**Field-visibility classes (SETTLED):** projection rows are owner-private,
refs-only. Public-safe refs use `FleetPublicRef`/hashed account refs. Never
projected: raw prompts, shell output, local paths, credentials, provider
payloads, private repo content. Host-only (never in renderer or Sync): tokens,
keychain material, raw SDK event streams.

### R2.5 P0 mutation catalog (SETTLED — names frozen)

Registered in `@openagentsinc/khala-sync-server`:

| Mutator | Rejections |
| --- | --- |
| `chat.createThread` | `unauthorized_scope`, `thread_exists` |
| `chat.appendMessage` | `unauthorized_scope`, `thread_not_found`, `message_exists` |
| `chat.renameThread` | `unauthorized_scope`, `thread_not_found` |
| `chat.bindThreadRepo` | `unauthorized_scope`, `thread_not_found` |
| `chat.pinCodexContinuity` | `unauthorized_scope`, `thread_not_found` |
| `fleet.setDesiredSlots` | `unauthorized_scope` |
| `fleet.pauseRun` / `fleet.resumeRun` | `unauthorized_scope` |
| `fleet.pauseWorker` / `fleet.resumeWorker` | `unauthorized_scope` |
| `fleet.stopRun` | `unauthorized_scope`, `confirmation_required` |
| `fleet.acknowledgeInboxFlag` | `unauthorized_scope` |

Fleet mutators record **durable operator intents** consumed by the Pylon
steering exchange (`fleet-intents.ts`, `fleet-steering-exchange.ts`); they are
requests, not effects — effect truth arrives as `fleet_command_outcome` rows
(§R2.3). Approval decisions and exact steer flow through the landed
steering/approval authority (#8639 substrate); their client entry points reuse
this same envelope. **DRAFT:** whether R3 adds a first-class
`fleet.approve`/`fleet.steer` mutator pair or keeps the landed exchange routes
is an R3 freeze item, not a client decision.

### R2.6 Device-local persistence (SETTLED decision, one DRAFT detail)

- Both clients persist through the existing `khala-sync-client` store
  semantics: `store-core.ts` + `sqlite-store.ts` (SQLite) with the overlay for
  optimistic state and the offline mutation queue.
- Desktop: SQLite in the Electron **main process**; the renderer receives typed
  projections over the fixed schema-decoded IPC boundary only.
- Mobile: same store semantics over the app-side SQLite driver
  (**DRAFT:** exact Expo SQLite driver binding is an M1 leaf; semantics and
  test-kit `store-semantics.testkit.ts` conformance are frozen).
- Caches carry explicit staleness/conflict markers (§R2.2 phases). A restarted
  client may render cached rows **only** labeled as `stale`/`catching_up`
  until the cursor reconciles; authority-bearing actions stay disabled until
  `live`.

### R2.7 Migration/compatibility (SETTLED posture)

- Additive schema changes bump `SyncSchemaVersion`; the server supports the
  current and immediately previous version during the R-gate window.
- Breaking changes require `scope_reset`/`schema_version_unsupported`
  refetch, a migration note in this contract, and serialized landing by the
  integration owner. Client rollback across one version must not corrupt the
  local store (R4 test 6).

## Acceptance oracles (six-rung vocabulary)

**R1 oracle:** an integration fixture (extend
`packages/khala-sync-client/src/cross-app-compose-turn.test.ts` pattern per
SYNC-4) proving: sign-in on two client classes resolves the same server-derived
owner id; both receive the same `scope.user.<id>` catalog; revoking one
session yields `denied` + `access_changed` purge on that device while the
other stays `live`; no token material appears in any projection row, renderer
payload, or log. Rungs: fixture-proven at SYNC-4 merge → deployed with the
Worker → live-proven by one real two-device sign-in/revoke receipt →
owner-accepted at R1 gate review.

**R2 oracle:** the same fixture family proving: `chat.createThread` +
`chat.appendMessage` on device A appear on device B with matching refs and
`SyncVersion`s; both restart and reconstruct identical state with no duplicate
objects; a forced retention-window gap yields `must_refetch
(cursor_behind_retained_window)` and a clean re-bootstrap; a `fleet.stopRun`
without confirmation is `rejected (confirmation_required)`; a replayed
`MutationId` is `duplicate` with no double effect; a dropped push
acknowledgement stays `unknown_pending_reconcile` then reconciles to exactly
one applied mutation; `fleet_*` rows for a real run render only from
projections and `fleet_command_outcome`, never from optimistic overlay. Rungs
reported per item; no rung implies the next.

## Delta ledger — binds-to vs must-build

| Already exists (bind, do not rebuild) | Must be built |
| --- | --- |
| Protocol/envelope/cursors/tombstones/`must_refetch` (`khala-sync`) | Desktop OpenAgents sign-in + keychain custody (R1.5) |
| Chat + fleet entity schemas and mutators (`khala-sync`/`-server`) | Mobile bearer sign-in + secure-storage recovery in `apps/openagents-mobile` |
| Client session/store/overlay/offline/reconnect + fault tests (`khala-sync-client`) | `device_session` projection + `identity.revokeSession` (after §R1.4 freeze) |
| Server session boundaries, refresh propagation, revocation (`workers/api/src/auth*`) | Desktop/mobile Sync adapters (SYNC-2/SYNC-3) over the frozen catalog |
| Fleet run/attempt/approval/command authority + steering exchange (#8637/#8633/#8639 substrate) | SYNC-4 cross-client continuity fixture; mobile SQLite driver binding |
| `khala-sync-db-collection` TanStack adapter | R3+ entities (`project/session`, `workroom`, `receipt`, …) — DRAFT, later freeze |

## Open DRAFT register

1. §R1.4 `device_session` entity + revoke mutator schema.
2. §R2.5 R3 approve/steer mutator surface vs landed exchange routes.
3. §R2.6 exact mobile SQLite driver binding.
4. R3–R7 entity schemas listed as follow-ons (workroom/preview/artifact/
   writeback/receipt, project/session).

Everything not listed in this register is **frozen**. SYNC-1/2/3/4, M1, and
Desktop D1 may proceed against the SETTLED sections immediately.
