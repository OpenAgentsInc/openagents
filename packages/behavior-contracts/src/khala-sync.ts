import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "./contract"

/**
 * Khala Sync behavior contracts (KS-3.3, #8293).
 *
 * Server-side sync-engine expectations from docs/khala-sync/SPEC.md that the
 * owner stated as load-bearing acceptance rules. Oracles live in the
 * `packages/khala-sync-server` integration suite (real local Postgres via
 * `src/test/local-postgres.ts`), which runs in that package's normal
 * `bun test` sweep. Authoring guidance for mutators — including how these
 * rules bind mutator code — is docs/khala-sync/MUTATORS.md.
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
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-04.1",
}
