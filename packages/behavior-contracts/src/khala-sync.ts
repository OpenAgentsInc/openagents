import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "./contract"

/**
 * Khala Sync behavior contracts (KS-3.3 #8293; KS-9.2 #8311).
 *
 * Owner-stated sync-engine expectations from docs/khala-sync/SPEC.md that
 * bind synced surfaces. Oracles run in the normal per-package test sweeps:
 * the server-side acceptance rule in the `packages/khala-sync-server`
 * integration suite (real local Postgres via `src/test/local-postgres.ts`),
 * the client-engine contracts in the `packages/khala-sync-client`
 * fake-transport session suite, and the revocation contract additionally in
 * the full-stack e2e suite of the `openagents.com` Worker. Authoring
 * guidance for mutators — including how these rules bind mutator code — is
 * docs/khala-sync/MUTATORS.md. Desktop-surface consumption of the client
 * primitives is contracted separately in
 * clients/khala-code-desktop/src/contracts/ux-contracts.ts
 * (khala_code.fleet.khala_sync_indicator_truthful.v1 /
 * khala_code.fleet.khala_sync_must_refetch_recovers.v1) and referenced
 * here rather than duplicated.
 */
export const khalaSyncContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "This contract binds the Khala Sync push engine's per-mutation acceptance semantics (executePush + mutation ledger) and the push route's rule that business validation is never an HTTP failure. It does not claim client-side rebase correctness, delivery latency, or availability of the sync surface.",
      blockerRefs: [],
      contractId: "khala_sync.push.validation_never_blocks_queue.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8293",
        "https://github.com/OpenAgentsInc/openagents/issues/8291",
        "docs/khala-sync/SPEC.md",
        "docs/khala-sync/MUTATORS.md",
        "packages/khala-sync-server/src/push-engine.test.ts",
        "packages/khala-sync-server/src/mutation-ledger.test.ts",
        "apps/openagents.com/workers/api/src/khala-sync-push-routes.ts",
      ],
      oracles: [
        {
          description:
            "A push batch [valid, invalid, valid] through executePush against real local Postgres yields results [applied, rejected(in-band), applied] with lastMutationId advanced past all three (the rejection is recorded in the mutation ledger, leaves no business/changelog residue) and a subsequent push applies normally — the queue is never blocked.",
          id: "khala_sync.push.queue_never_blocks_batch",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/khala-sync-server/src/push-engine.test.ts",
        },
      ],
      productArea: "khala sync push engine",
      source: {
        channel: "issue",
        statedBy: "owner",
        statedOn: "2026-07-04",
      },
      state: "enforced",
      statement:
        "Acceptance is synchronous with the transaction; validation failures ack the mutation and report the error in-band — they never 4xx/block the queue.",
      surface: "openagents.com-worker",
      verification:
        "KS-3.3 is enforced by the push-engine integration suite in packages/khala-sync-server (executePush against real local Postgres) in that package's normal bun test sweep; the route-level never-4xx mapping is exercised by the openagents.com Worker push route tests.",
    },
    {
      authorityBoundary:
        "This contract binds the client engine's queue semantics (durable pending queue + overlay + session pending exposure). It does not promise delivery latency, server availability, or that a queued mutation will be accepted — a rejected mutation is retracted honestly, not retried forever. Surfaces choose their own pending-badge rendering; the contract only guarantees the primitives cannot lie.",
      blockerRefs: [],
      contractId: "khala_sync.client.offline_pushes_queue_honestly.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8311",
        "docs/khala-sync/SPEC.md",
        "packages/khala-sync-client/src/session.ts",
        "packages/khala-sync-client/src/overlay.ts",
        "packages/khala-sync-client/src/session.test.ts",
      ],
      oracles: [
        {
          description:
            "Drives the client session over the deterministic fake transport with the transport failing: mutations land in the durable pending queue in order, are exposed as pending through the session's UI-facing pending() (never written to the confirmed store), drain in submission order on recovery, and a terminal rejection acks in-band, is surfaced through onRejection, and leaves no confirmed residue; a terminal transport fault parks the queue intact.",
          id: "khala_sync.client.offline_queue_honest_lifecycle",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/khala-sync-client/src/session.test.ts",
        },
      ],
      productArea: "khala sync client engine",
      source: {
        channel: "issue",
        statedBy: "owner",
        statedOn: "2026-07-04",
      },
      state: "enforced",
      statement:
        "Offline pushes queue honestly: mutations made while the transport is failing are durably queued and visibly pending — never shown as confirmed — they drain on recovery in the order they were made, and a terminal rejection is retracted and surfaced honestly instead of being silently dropped or presented as success.",
      surface: "khala-sync-client",
      verification:
        "bun test src/session.test.ts inside packages/khala-sync-client (fake transport, injected time); runs in that package's normal bun test sweep before pushes to main. The session's pending() exposure is the UI-facing pending-vs-confirmed primitive consuming surfaces must use.",
    },
    {
      authorityBoundary:
        "This contract binds the freshness primitives (session phase + lastDeltaAt) and forbids fabricated 'live' defaults in consumers. It does not set a numeric staleness budget, and desktop indicator rendering stays owned by the desktop contract khala_code.fleet.khala_sync_indicator_truthful.v1 — referenced, not duplicated.",
      blockerRefs: [],
      contractId: "khala_sync.client.staleness_never_fabricated.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8311",
        "https://github.com/OpenAgentsInc/openagents/issues/8303",
        "contract:khala_code.fleet.khala_sync_indicator_truthful.v1",
        "docs/khala-sync/SPEC.md",
        "packages/khala-sync-client/src/session.ts",
        "clients/khala-code-desktop/src/ui/fleet-sync-projection.ts",
        "clients/khala-code-desktop/tests/ux-contracts.test.ts",
      ],
      oracles: [
        {
          description:
            "Proves the session exposes real freshness primitives over the fake transport: lastDeltaAt is null before any server-confirmed apply (no fake default), stamps the injected clock on bootstrap/catch-up/live-delta applies, keeps the LAST honest time (never advancing) while the transport is down and the phase has dropped out of live, and is cleared together with the synced state on access denial.",
          id: "khala_sync.client.freshness_primitives_truthful",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/khala-sync-client/src/session.test.ts",
        },
      ],
      productArea: "khala sync client engine",
      source: {
        channel: "issue",
        statedBy: "owner",
        statedOn: "2026-07-04",
      },
      state: "enforced",
      statement:
        "Synced staleness is never fabricated: any surface consuming Khala Sync derives freshness from the session's real phase plus its last-delta time, never from a fake 'live' default.",
      surface: "khala-sync-client",
      verification:
        "bun test src/session.test.ts inside packages/khala-sync-client (fake transport, injected clock) enforces the primitives; the desktop Fleet indicator's consumption of the phase primitive is enforced by the DOM oracle of khala_code.fleet.khala_sync_indicator_truthful.v1 in clients/khala-code-desktop/tests/ux-contracts.test.ts (KS-6.2, #8303). Both run in their packages' normal test sweeps before pushes to main.",
    },
    {
      authorityBoundary:
        "This contract binds SPEC §7 invariant 7's client-visible outcome (revocation retracts synced state; the scope parks terminal instead of retrying). It does not define who may change memberships, and the revocation TRIGGER for already-open sockets remains the operator/Worker access-changed route obligation registered in apps/openagents.com/INVARIANTS.md (Khala Sync invariant 7).",
      blockerRefs: [],
      contractId: "khala_sync.access.revocation_clears_synced_state.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8311",
        "https://github.com/OpenAgentsInc/openagents/issues/8305",
        "docs/khala-sync/SPEC.md",
        "docs/khala-sync/RUNBOOK.md",
        "packages/khala-sync-client/src/session.ts",
        "apps/openagents.com/workers/api/src/khala-sync-access-revocation.e2e.test.ts",
      ],
      oracles: [
        {
          description:
            "Full-stack revocation e2e (KS-7.1, #8305): real local Postgres + real Worker bootstrap/log/connect route handlers + real KhalaSyncHubDO + the real client store/overlay/session — membership removal plus the access-changed trigger broadcasts MustRefetch(access_changed), all reads 403, and the client's denied re-bootstrap clears its scope-local durable state and parks terminal denied.",
          id: "khala_sync.access.revocation_e2e_retracts_state",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/khala-sync-access-revocation.e2e.test.ts",
        },
        {
          description:
            "Client-engine oracles over the fake transport: MustRefetch(access_changed) followed by a denied re-bootstrap CLEARS durable rows + cursor and parks TERMINAL denied with no retry; the same clearing holds for a 403 mid catch-up and for denial on first contact.",
          id: "khala_sync.access.revocation_client_clears_and_parks",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/khala-sync-client/src/session.test.ts",
        },
      ],
      productArea: "khala sync access control",
      source: {
        channel: "issue",
        statedBy: "owner",
        statedOn: "2026-07-04",
      },
      state: "enforced",
      statement:
        "Access revocation clears synced state: once a user's access to a scope is revoked, the client stops receiving that scope, its locally synced copy of the scope is cleared rather than left readable, and the session parks denied instead of silently retrying.",
      surface: "khala-sync-client",
      verification:
        "The full-stack oracle is the KS-7.1 revocation e2e in apps/openagents.com/workers/api (vitest; real Postgres via local-postgres, skips only on machines without initdb/pg_ctl) and the client-side clearing oracles run in the packages/khala-sync-client bun test sweep. Both suites run in their packages' normal test sweeps before pushes to main; SPEC §7 invariant 7 registration with honest limits lives in apps/openagents.com/INVARIANTS.md.",
    },
    {
      authorityBoundary:
        "This SEAM contract (ST-5 #8511) binds the two-sided bearer WebSocket connect boundary: the real client transport's documented ?token= query-param bearer on the WS upgrade (browser and React Native WebSocket clients cannot set an Authorization header) MEETS the Worker connect route's query-param-aware authentication, and an authenticated cookie-less session actually reaches the live phase. It does not bind delivery latency, reconnect backoff policy, or scope membership semantics (owned by the access contracts above). Per the seam convention, its oracle must be an e2e suite driving REAL code from both named sides — the fake-transport session suite can never enforce this contract.",
      blockerRefs: [
        "blocker.khala_sync.depends_on_8507_live_seam_smoke_landing",
      ],
      contractId: "khala_sync.seam.bearer_ws_connect_reaches_live.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8511",
        "https://github.com/OpenAgentsInc/openagents/issues/8507",
        "docs/fable/2026-07-06-seam-testing-audit-qa-swarm-gaps.md",
        "docs/khala-code/2026-07-06-mobile-loading-threads-websocket-auth-audit.md",
        "packages/khala-sync-client/src/transport.ts",
        "apps/openagents.com/workers/api/src/khala-sync-connect-routes.ts",
        "packages/khala-sync-client/src/live-seam-smoke.e2e.test.ts",
      ],
      oracles: [],
      productArea: "khala sync transport",
      seam: {
        client: "packages/khala-sync-client/src/transport.ts",
        server: "apps/openagents.com/workers/api/src/khala-sync-connect-routes.ts",
      },
      source: {
        channel: "issue",
        statedBy: "owner",
        statedOn: "2026-07-06",
      },
      state: "pending",
      statement:
        "A cookie-less bearer client completes a real /api/sync/connect upgrade and reaches live: the client transport's ?token= query-param bearer (the only credential a browser or React Native WebSocket upgrade can carry) is accepted by the real connect route, and the session phase actually reaches live — never an infinite silent retry loop.",
      surface: "khala-sync-client",
      verification:
        "Pending on ST-1 (#8507): the oracle is the live-seam smoke at packages/khala-sync-client/src/live-seam-smoke.e2e.test.ts, which drives the REAL transport (bootstrap -> logPage -> connectLive) with a bearer-only credential against the real Worker route stack. Once that suite lands on main, flip this contract to enforced/test-sweep with that ref as its bun-test oracle — the seam coverage checker will then require the e2e ref and its contractId reference. The one-sided halves that already exist (the connect route's query-param tests and the client's fake-transport session suite) are deliberately NOT acceptable oracles for this contract.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-06.1",
}
