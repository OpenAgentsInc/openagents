import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts"

/**
 * Sarah cross-prospect isolation behavior contracts (KHS-3, #8602, epic #8599).
 *
 * This registry is the durable home for the owner's stated expectations about
 * what Sarah may and may not do with prospect data. Every entry records the
 * statement verbatim, who stated it and where, and the oracle tests that
 * enforce it in the normal test sweep. The paired coverage test in
 * src/contracts/isolation-contracts.test.ts fails the sweep if an enforced
 * contract loses its oracle, so stated behavior cannot silently drift.
 *
 * The isolation law binds BEFORE any shared learning: KHS-3 landed the
 * oracles first ("Isolation before generalization", epic #8599), and KHS-4
 * (#8603) then shipped the owner-approved collective-learning queue behind
 * them — flipping sarah.collective_learning_owner_gated.v1 to enforced in
 * the same change that added the first shared-knowledge read path.
 *
 * Human rendering: docs/sarah/SARAH_CONTRACTS.md (kept in sync by the same
 * test file).
 */
export const SARAH_CONTRACTS_DOC_PATH = "docs/sarah/SARAH_CONTRACTS.md"

export const sarahIsolationContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "This contract binds Sarah's own read/serve paths (session index, turn store, avatar brain, and the KHS-2 prospect-memory seam from #8601). It does not claim collective learning exists, does not arm any capture sink, and grants no authority over the openagents.com CRM boundary, which remains the system of record. Any NEW prospect-scoped read path added to apps/sarah must gain a bun-test oracle under this contract in the same change.",
      blockerRefs: [],
      contractId: "sarah.cross_prospect_isolation.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/sarah/src/services/session-index.ts",
        "apps/sarah/src/services/turn-store.ts",
        "apps/sarah/src/services/prospect-memory.ts",
        "apps/sarah/src/llm-openai-compat.ts",
        "docs/sarah/SARAH_CONTRACTS.md",
        "docs/fable/2026-07-09-sarah-khala-connection-assessment.md",
        "issue:#8602",
        "issue:#8599",
      ],
      oracles: [
        {
          description:
            "Query-layer scoping: seeds transcript turns, CRM projections, and tool receipts for two different prospect refs (sharing a session id to prove prospect_ref is the filter, not session id), then reads each prospect back through every prospect-scoped read helper in session-index and asserts zero rows, markers, or CRM fields from the other prospect appear.",
          id: "cross_prospect_query_scoping.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/contracts/isolation-contracts.test.ts",
        },
        {
          description:
            "Injection probe on the avatar brain endpoint: seeds a distinctive secret into prospect B's persisted turns, then POSTs /sarah/api/llm/chat/completions (bearer-armed, model deliberately unarmed so only the deterministic layers answer) with a conversation_ref for prospect A and a user message asking what the last customer said. Asserts the reply and prospect A's recorded transcript contain nothing from prospect B.",
          id: "cross_prospect_injection_probe.rpc",
          kind: "bun-test",
          mode: "rpc",
          ref: "apps/sarah/src/contracts/isolation-contracts.test.ts",
        },
        {
          description:
            "KHS-2 memory seam (#8601, merged): getProspectMemoryContext takes exactly one prospectRef; prospectRefAliases only re-encodes that same identity (visitor: refs never alias); an empty ref yields no aliases so no unscoped query is possible; and without a durable store memory fails soft to null instead of falling back to any cross-prospect source. Asserted both in the KHS-2 seam suite and in the isolation contract suite.",
          id: "prospect_memory_isolation.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/services/prospect-memory.test.ts",
        },
      ],
      productArea: "prospect memory + conversation serving",
      source: {
        channel: "sarah-production-conversation",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "enforced",
      statement:
        "safeguards to make sure she's not taking stuff from one person and telling it to another",
      surface: "sarah",
      verification:
        "bun test src/contracts/isolation-contracts.test.ts inside apps/sarah; runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "This contract binds the KHS-4 pipeline (#8603): candidates distilled from cross-prospect transcripts are PII-redacted (redact-or-drop), stored pending, and cross into shared knowledge ONLY via an owner decision on the admin-bearer-guarded operator endpoints, each writing an approval receipt; answer-bank publications carry the receipt ref as approved_by. It authorizes no capture path and makes no public 'learning from conversations' claim while the data.khala_free_tier_trace_capture.v1 promise family is yellow — this is an internal owner-approved store, framed as such. Nothing crosses prospects without an owner-approval receipt.",
      blockerRefs: [],
      contractId: "sarah.collective_learning_owner_gated.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/sarah/src/services/collective-learning.ts",
        "apps/sarah/src/services/semantic-answer-cache.ts",
        "apps/sarah/src/server.ts",
        "docs/sarah/SARAH_CONTRACTS.md",
        "docs/fable/2026-07-09-sarah-khala-connection-assessment.md",
        "issue:#8603",
        "issue:#8599",
      ],
      oracles: [
        {
          description:
            "Shared-knowledge reads come ONLY from the owner-approved store: seeds pending candidates via distillation, approves one with a receipt, and asserts listApprovedLearnings returns exclusively the approved entry (receipt attached) while pending/rejected candidates stay unreachable; the published answer-bank entry's approved_by is the approval receipt ref, so a live answer dereferences back to its receipt and redacted source turns.",
          id: "approved_store_only.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/services/collective-learning.test.ts",
        },
        {
          description:
            "Admin guard is mandatory and fails closed on the HTTP surface: with no admin token configured, GET /sarah/api/operator/learning and the distill/approve/reject POSTs return 503 (an approve without the guard is impossible); with the token armed, a missing or wrong bearer is 401 and no decision or receipt is ever written; only the exact bearer can list, distill, approve, and reject.",
          id: "learning_admin_guard.rpc",
          kind: "bun-test",
          mode: "rpc",
          ref: "apps/sarah/src/services/collective-learning.test.ts",
        },
        {
          description:
            "PII never enters candidates: seeded examples with emails, phone numbers, long digit runs, and URLs come out scrubbed; name introductions and ambiguous residue drop the example entirely (when in doubt, drop); distilled candidates are asserted free of the seeded contact data across summary, canonical question, proposed answer, and examples.",
          id: "learning_pii_redaction.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/services/collective-learning.test.ts",
        },
      ],
      productArea: "collective learning",
      source: {
        channel: "sarah-production-conversation",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "enforced",
      statement:
        "admin approval for determining what she's able to learn generally from everyone else",
      surface: "sarah",
      verification:
        "bun test src/services/collective-learning.test.ts inside apps/sarah; runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "This contract binds the read discipline (exact prospect_ref filter at the query layer), not the storage engine. It does not forbid owner/operator surfaces that intentionally list all prospects (e.g. session receipts for the operator dashboard); those are owner-facing, never prospect-facing, and must never feed a prospect-facing serve path. The KHS-2 prospect-memory reads (#8601) are bound by the seam oracle below.",
      blockerRefs: [],
      contractId: "sarah.memory_query_scoped.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/sarah/src/services/session-index.ts",
        "apps/sarah/src/services/turn-store.ts",
        "apps/sarah/src/services/prospect-memory.ts",
        "docs/sarah/SARAH_CONTRACTS.md",
        "issue:#8601",
        "issue:#8602",
      ],
      oracles: [
        {
          description:
            "Every prospect-scoped read helper that exists today (getSarahSessionTranscript, getSarahProspectCrmProjection, findSarahProspectByContactEmail) takes an exact prospect ref (or resolves to exactly one) and returns only that prospect's rows; an unknown ref returns empty, never a fallback to another prospect's data.",
          id: "memory_query_scoping.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/contracts/isolation-contracts.test.ts",
        },
        {
          description:
            "KHS-2 memory seam (#8601, merged): every SQL read in prospect-memory.ts is bound to prospectRefAliases(prospectRef) (WHERE prospect_ref IN — exact identity re-encodings only, never a pattern or cross-prospect list), and the alias derivation plus the no-store null path are unit-oracled so a prompt-side 'filter' can never substitute for the query-layer one.",
          id: "prospect_memory_query_scoping.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/services/prospect-memory.test.ts",
        },
      ],
      productArea: "prospect memory + conversation serving",
      source: {
        channel: "sarah-production-conversation",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "enforced",
      statement:
        "All prospect-scoped reads are filtered by exact prospect_ref at the query layer, not prompt-side.",
      surface: "sarah",
      verification:
        "bun test src/contracts/isolation-contracts.test.ts inside apps/sarah; runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Deal-rule evaluation and human handoff remain the only pricing-adjacent actions, and the openagents.com API remains the authority for checkout and credits. Retrieval, memory, or any future Khala lookup can inform language, never prices — this guard binds regardless of what memory returns.",
      blockerRefs: [],
      contractId: "sarah.no_improvised_pricing.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/sarah/src/llm-openai-compat.ts",
        "apps/sarah/src/agent-runtime/owned-runtime.ts",
        "apps/sarah/src/services/deal-rules.ts",
        "apps/sarah/INVARIANTS.md",
        "docs/fable/2026-07-07-sarah-sales-agent-spec.md",
      ],
      oracles: [
        {
          description:
            "Text lane: POST /sarah/api/eve/turn with pricing pressure returns modelPath deterministic_guard with the no-improvised-discounts reply — the model path is never reached.",
          id: "pricing_guard_text_lane.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/server.test.ts",
        },
        {
          description:
            "Avatar brain: POST /sarah/api/llm/chat/completions with pricing pressure answers from the deterministic guard before the model, holding the same law on the voice lane.",
          id: "pricing_guard_avatar_brain.rpc",
          kind: "bun-test",
          mode: "rpc",
          ref: "apps/sarah/src/server.test.ts",
        },
      ],
      productArea: "pricing + deal rules",
      source: {
        channel: "sarah-spec",
        statedBy: "owner",
        statedOn: "2026-07-07",
      },
      state: "enforced",
      statement:
        "No improvised discounts; deal-rules code + public packs only; owner-priced params from runtime config.",
      surface: "sarah",
      verification:
        "bun test src/server.test.ts inside apps/sarah (pricing-guard tests on both the text lane and the avatar brain); runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-09.2",
}
