import { BehaviorContractSchemaVersion, type BehaviorContractRegistryDocument } from "./contract";

/**
 * Behaviour contracts for the local swap session store (openagents#9320,
 * SWAP-5). The issue requires contracts for exactly two laws: the
 * export/import round trip and the resume guarantee. The oracles live in
 * `@openagentsinc/mkt-swp-session-store`.
 */
export const marketSwapSessionStoreContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "This contract covers the local persistence dataset only. It grants no spend authority: the export contains signed public records, exit packages, and effect results — never keys, preimages, or other secret material, which belong to the SWAP-4 secret store (#9319). Possession of an export does not move funds.",
      blockerRefs: [],
      contractId: "openagents_web.swap_history.export_import_round_trip.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-session-store/src/export.test.ts",
        "github:OpenAgentsInc/openagents#9320",
        "docs/teardowns/2026-08-04-boltz-web-app-ux-parity-teardown.md",
      ],
      oracles: [
        {
          description:
            "Exporting a profile's history and importing it into a clean profile reproduces every session's actionable state — the resume plan, the effect ledger (idempotency dataset), and the exit packages needed for a unilateral exit — and import refuses foreign, tampered, future-version, or secret-carrying documents with a typed error, all-or-nothing (teardown §4.4: Boltz's export is write-only; ours is not).",
          id: "openagents_web.swap_history.export_import_round_trip.store",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-session-store/src/export.test.ts",
        },
      ],
      productArea: "Swap history and self-custody escape hatch",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "If we emit an export, we ingest it: export then import into a clean profile reproduces every session's actionable state, including the ability to execute a unilateral exit, and a foreign or corrupt document is refused with a typed error rather than partially applied.",
      surface: "openagents.com/swap",
      verification:
        "packages/mkt-swp-session-store tests export a seeded store (the custody tripwire runs per session on the export path itself), import into a clean store with default arguments (the shipped migration chain, so an older build's document ingests), and assert session-set equality, resume-plan equality, exit-package presence, and prior-effect-result queryability; refusal tests cover not_an_export, unsupported_version, digest_mismatch, session_invalid, secret_material, and conflicting_session, each asserting zero writes, plus storage_failure, asserting a mid-apply driver failure rolls this import's writes back before refusing.",
    },
    {
      authorityBoundary:
        "This contract covers resume planning and effect-ledger idempotency in the local store. It does not authorise funding or claim execution: the engine's verify-before-fund gate and the host wallet remain the authority for any external effect the resumed session drives.",
      blockerRefs: [],
      contractId: "openagents_web.swap_history.resume_after_reload.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/mkt-swp-session-store/src/resume.test.ts",
        "packages/mkt-swp-session-store/src/store.test.ts",
        "github:OpenAgentsInc/openagents#9320",
      ],
      oracles: [
        {
          description:
            "After a reload, a new store instance over the same storage re-plans every non-terminal session plus terminal-but-unclaimed ones with a snapshot-before-live catch-up spec, and a persisted effect result suppresses the external callback — the same effect id with a different request fails closed — so no external effect is duplicated.",
          id: "openagents_web.swap_history.resume_after_reload.store",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/mkt-swp-session-store/src/resume.test.ts",
        },
      ],
      productArea: "Swap history and self-custody escape hatch",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-08-04",
      },
      state: "enforced",
      statement:
        "A session interrupted by a reload resumes without user action and without duplicating an external effect; resume is app-wide, re-attaching every non-terminal stored session plus terminal-but-unclaimed ones, with a catch-up read before the live fold is trusted.",
      surface: "openagents.com/swap",
      verification:
        "packages/mkt-swp-session-store tests persist a mid-flight session with a recorded effect request and result, reopen the store as a fresh instance, and assert the resume plan includes the session, the prior result is returned for the exact persisted request (suppressing the callback), a mismatched request fails closed, an unresulted request survives for the crash-window replay, and a definitively failed effect (recordEffectFailure) releases the reload guard while priorEffectResult still permits the retry.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-08-07.1",
};
