import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "./contract"

/**
 * RETIRED Sarah behavior contracts (owner direction 2026-07-10, epic #8610).
 *
 * The Sarah surface — the web page at openagents.com/sarah, every
 * /sarah/api/* route, and the whole apps/sarah package — was removed at
 * owner direction 2026-07-10 ("all sarah shit must die"; supersedes the
 * rev-24 retention of Sarah routes as regression substrate). Per the
 * behavior-contract discipline, owner statements are never silently deleted:
 * every contract that lived in apps/sarah/src/contracts/ is preserved here
 * verbatim with state "retired" and a retirement note. Oracle refs are
 * historical — their files were deleted with apps/sarah and no longer run in
 * any sweep. Human rendering: docs/sarah/SARAH_CONTRACTS.md (also retained
 * as history).
 *
 * If a future surface resurrects any of these expectations, register a NEW
 * contract version on the owning surface instead of flipping these back.
 */
export const sarahRetiredContractRegistry: BehaviorContractRegistryDocument = {
  "contracts": [
    {
      "authorityBoundary": "This contract binds Sarah's own read/serve paths (session index, turn store, avatar brain, and the KHS-2 prospect-memory seam from #8601). It does not claim collective learning exists, does not arm any capture sink, and grants no authority over the openagents.com CRM boundary, which remains the system of record. Any NEW prospect-scoped read path added to apps/sarah must gain a bun-test oracle under this contract in the same change.",
      "blockerRefs": [],
      "contractId": "sarah.cross_prospect_isolation.v1",
      "enforcementTier": "unenforced",
      "evidenceRefs": [
        "apps/sarah/src/services/session-index.ts",
        "apps/sarah/src/services/turn-store.ts",
        "apps/sarah/src/services/prospect-memory.ts",
        "apps/sarah/src/services/customer-blueprint.ts",
        "apps/sarah/src/llm-openai-compat.ts",
        "docs/sarah/SARAH_CONTRACTS.md",
        "docs/fable/2026-07-09-sarah-khala-connection-assessment.md",
        "issue:#8602",
        "issue:#8599",
        "issue:#8608"
      ],
      "oracles": [
        {
          "description": "Query-layer scoping: seeds transcript turns, CRM projections, and tool receipts for two different prospect refs (sharing a session id to prove prospect_ref is the filter, not session id), then reads each prospect back through every prospect-scoped read helper in session-index and asserts zero rows, markers, or CRM fields from the other prospect appear.",
          "id": "cross_prospect_query_scoping.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/contracts/isolation-contracts.test.ts"
        },
        {
          "description": "Injection probe on the avatar brain endpoint: seeds a distinctive secret into prospect B's persisted turns, then POSTs /sarah/api/llm/chat/completions (bearer-armed, model deliberately unarmed so only the deterministic layers answer) with a conversation_ref for prospect A and a user message asking what the last customer said. Asserts the reply and prospect A's recorded transcript contain nothing from prospect B.",
          "id": "cross_prospect_injection_probe.rpc",
          "kind": "bun-test",
          "mode": "rpc",
          "ref": "apps/sarah/src/contracts/isolation-contracts.test.ts"
        },
        {
          "description": "KHS-2 memory seam (#8601, merged): getProspectMemoryContext takes exactly one prospectRef; prospectRefAliases only re-encodes that same identity (visitor: refs never alias); an empty ref yields no aliases so no unscoped query is possible; and without a durable store memory fails soft to null instead of falling back to any cross-prospect source. Asserted both in the KHS-2 seam suite and in the isolation contract suite.",
          "id": "prospect_memory_isolation.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/services/prospect-memory.test.ts"
        },
        {
          "description": "KHS-9 customer-blueprint seam (#8608): buildCustomerBlueprintDraft takes exactly one prospectRef and every store read is bound to prospectRefAliases(prospectRef) (the exact identity's re-encodings, asserted via the injected reader seam); drafts composed for two prospects sharing seeded data never carry the other prospect's facts, needs, turn ids, or contact; an empty ref refuses instead of reading unscoped.",
          "id": "customer_blueprint_prospect_scoping.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/services/customer-blueprint.test.ts"
        }
      ],
      "productArea": "prospect memory + conversation serving",
      "source": {
        "channel": "sarah-production-conversation",
        "statedBy": "owner",
        "statedOn": "2026-07-09"
      },
      "state": "retired",
      "statement": "safeguards to make sure she's not taking stuff from one person and telling it to another",
      "surface": "sarah",
      "verification": "RETIRED 2026-07-10: the Sarah surface (web /sarah page and every /sarah/api/* route, apps/sarah) was removed at owner direction (“all sarah shit must die”, epic #8610; supersedes the rev-24 retention of Sarah routes as regression substrate). The statement is preserved verbatim per the behavior-contract discipline; the oracle refs below are historical (their files were deleted with apps/sarah at openagents commit history ≤ 2026-07-10) and no longer run in any sweep. Historical verification: bun test src/contracts/isolation-contracts.test.ts inside apps/sarah; runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main."
    },
    {
      "authorityBoundary": "This contract binds the KHS-4 pipeline (#8603): candidates distilled from cross-prospect transcripts are PII-redacted (redact-or-drop), stored pending, and cross into shared knowledge ONLY via an owner decision on the admin-bearer-guarded operator endpoints, each writing an approval receipt; answer-bank publications carry the receipt ref as approved_by. It authorizes no capture path and makes no public 'learning from conversations' claim while the data.khala_free_tier_trace_capture.v1 promise family is yellow — this is an internal owner-approved store, framed as such. Nothing crosses prospects without an owner-approval receipt.",
      "blockerRefs": [],
      "contractId": "sarah.collective_learning_owner_gated.v1",
      "enforcementTier": "unenforced",
      "evidenceRefs": [
        "apps/sarah/src/services/collective-learning.ts",
        "apps/sarah/src/services/semantic-answer-cache.ts",
        "apps/sarah/src/server.ts",
        "docs/sarah/SARAH_CONTRACTS.md",
        "docs/fable/2026-07-09-sarah-khala-connection-assessment.md",
        "issue:#8603",
        "issue:#8599"
      ],
      "oracles": [
        {
          "description": "Shared-knowledge reads come ONLY from the owner-approved store: seeds pending candidates via distillation, approves one with a receipt, and asserts listApprovedLearnings returns exclusively the approved entry (receipt attached) while pending/rejected candidates stay unreachable; the published answer-bank entry's approved_by is the approval receipt ref, so a live answer dereferences back to its receipt and redacted source turns.",
          "id": "approved_store_only.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/services/collective-learning.test.ts"
        },
        {
          "description": "Admin guard is mandatory and fails closed on the HTTP surface: with no admin token configured, GET /sarah/api/operator/learning and the distill/approve/reject POSTs return 503 (an approve without the guard is impossible); with the token armed, a missing or wrong bearer is 401 and no decision or receipt is ever written; only the exact bearer can list, distill, approve, and reject.",
          "id": "learning_admin_guard.rpc",
          "kind": "bun-test",
          "mode": "rpc",
          "ref": "apps/sarah/src/services/collective-learning.test.ts"
        },
        {
          "description": "PII never enters candidates: seeded examples with emails, phone numbers, long digit runs, and URLs come out scrubbed; name introductions and ambiguous residue drop the example entirely (when in doubt, drop); distilled candidates are asserted free of the seeded contact data across summary, canonical question, proposed answer, and examples.",
          "id": "learning_pii_redaction.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/services/collective-learning.test.ts"
        }
      ],
      "productArea": "collective learning",
      "source": {
        "channel": "sarah-production-conversation",
        "statedBy": "owner",
        "statedOn": "2026-07-09"
      },
      "state": "retired",
      "statement": "admin approval for determining what she's able to learn generally from everyone else",
      "surface": "sarah",
      "verification": "RETIRED 2026-07-10: the Sarah surface (web /sarah page and every /sarah/api/* route, apps/sarah) was removed at owner direction (“all sarah shit must die”, epic #8610; supersedes the rev-24 retention of Sarah routes as regression substrate). The statement is preserved verbatim per the behavior-contract discipline; the oracle refs below are historical (their files were deleted with apps/sarah at openagents commit history ≤ 2026-07-10) and no longer run in any sweep. Historical verification: bun test src/services/collective-learning.test.ts inside apps/sarah; runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main."
    },
    {
      "authorityBoundary": "This contract binds the read discipline (exact prospect_ref filter at the query layer), not the storage engine. It does not forbid owner/operator surfaces that intentionally list all prospects (e.g. session receipts for the operator dashboard); those are owner-facing, never prospect-facing, and must never feed a prospect-facing serve path. The KHS-2 prospect-memory reads (#8601) are bound by the seam oracle below.",
      "blockerRefs": [],
      "contractId": "sarah.memory_query_scoped.v1",
      "enforcementTier": "unenforced",
      "evidenceRefs": [
        "apps/sarah/src/services/session-index.ts",
        "apps/sarah/src/services/turn-store.ts",
        "apps/sarah/src/services/prospect-memory.ts",
        "docs/sarah/SARAH_CONTRACTS.md",
        "issue:#8601",
        "issue:#8602"
      ],
      "oracles": [
        {
          "description": "Every prospect-scoped read helper that exists today (getSarahSessionTranscript, getSarahProspectToolReceipts, getSarahProspectCrmProjection, findSarahProspectByContactEmail) takes an exact prospect ref (or resolves to exactly one) and returns only that prospect's rows; an unknown ref returns empty, never a fallback to another prospect's data.",
          "id": "memory_query_scoping.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/contracts/isolation-contracts.test.ts"
        },
        {
          "description": "KHS-2 memory seam (#8601, merged): every SQL read in prospect-memory.ts is bound to prospectRefAliases(prospectRef) (WHERE prospect_ref IN — exact identity re-encodings only, never a pattern or cross-prospect list), and the alias derivation plus the no-store null path are unit-oracled so a prompt-side 'filter' can never substitute for the query-layer one.",
          "id": "prospect_memory_query_scoping.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/services/prospect-memory.test.ts"
        }
      ],
      "productArea": "prospect memory + conversation serving",
      "source": {
        "channel": "sarah-production-conversation",
        "statedBy": "owner",
        "statedOn": "2026-07-09"
      },
      "state": "retired",
      "statement": "All prospect-scoped reads are filtered by exact prospect_ref at the query layer, not prompt-side.",
      "surface": "sarah",
      "verification": "RETIRED 2026-07-10: the Sarah surface (web /sarah page and every /sarah/api/* route, apps/sarah) was removed at owner direction (“all sarah shit must die”, epic #8610; supersedes the rev-24 retention of Sarah routes as regression substrate). The statement is preserved verbatim per the behavior-contract discipline; the oracle refs below are historical (their files were deleted with apps/sarah at openagents commit history ≤ 2026-07-10) and no longer run in any sweep. Historical verification: bun test src/contracts/isolation-contracts.test.ts inside apps/sarah; runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main."
    },
    {
      "authorityBoundary": "Deal-rule evaluation and human handoff remain the only pricing-adjacent actions, and the openagents.com API remains the authority for checkout and credits. Retrieval, memory, or any future Khala lookup can inform language, never prices — this guard binds regardless of what memory returns.",
      "blockerRefs": [],
      "contractId": "sarah.no_improvised_pricing.v1",
      "enforcementTier": "unenforced",
      "evidenceRefs": [
        "apps/sarah/src/llm-openai-compat.ts",
        "apps/sarah/src/agent-runtime/owned-runtime.ts",
        "apps/sarah/src/services/deal-rules.ts",
        "apps/sarah/INVARIANTS.md",
        "docs/fable/2026-07-07-sarah-sales-agent-spec.md"
      ],
      "oracles": [
        {
          "description": "Text lane: POST /sarah/api/eve/turn with pricing pressure returns modelPath deterministic_guard with the no-improvised-discounts reply — the model path is never reached.",
          "id": "pricing_guard_text_lane.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/server.test.ts"
        },
        {
          "description": "Avatar brain: POST /sarah/api/llm/chat/completions with pricing pressure answers from the deterministic guard before the model, holding the same law on the voice lane.",
          "id": "pricing_guard_avatar_brain.rpc",
          "kind": "bun-test",
          "mode": "rpc",
          "ref": "apps/sarah/src/server.test.ts"
        }
      ],
      "productArea": "pricing + deal rules",
      "source": {
        "channel": "sarah-spec",
        "statedBy": "owner",
        "statedOn": "2026-07-07"
      },
      "state": "retired",
      "statement": "No improvised discounts; deal-rules code + public packs only; owner-priced params from runtime config.",
      "surface": "sarah",
      "verification": "RETIRED 2026-07-10: the Sarah surface (web /sarah page and every /sarah/api/* route, apps/sarah) was removed at owner direction (“all sarah shit must die”, epic #8610; supersedes the rev-24 retention of Sarah routes as regression substrate). The statement is preserved verbatim per the behavior-contract discipline; the oracle refs below are historical (their files were deleted with apps/sarah at openagents commit history ≤ 2026-07-10) and no longer run in any sweep. Historical verification: bun test src/server.test.ts inside apps/sarah (pricing-guard tests on both the text lane and the avatar brain); runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main."
    },
    {
      "authorityBoundary": "This contract binds the account-linking seam only (KHS-7, #8606): the openagents.com API remains the identity and credit authority; sign-in happens on the existing /login + OpenAuth rails (apps/sarah never touches password/OAuth internals and never mints sessions); identity for a link comes ONLY from the first-party session cookie verified against GET /api/auth/session, never from a request body; and the linked identity (user ref + email) lands only in sarah_prospect_contacts. The payment half of the owner's directive (attaching a card, paying in-chat) is KHS-8 (epic #8599) and must gain its own contract when it lands.",
      "blockerRefs": [],
      "contractId": "sarah.in_chat_account_linking.v1",
      "enforcementTier": "unenforced",
      "evidenceRefs": [
        "apps/sarah/src/services/account-link.ts",
        "apps/sarah/src/server.ts",
        "apps/sarah/src/ui/main.ts",
        "docs/sarah/SARAH_CONTRACTS.md",
        "issue:#8606",
        "issue:#8599"
      ],
      "oracles": [
        {
          "description": "Link routes: GET /sarah/api/account/status reports anonymous without a prospect cookie; POST /sarah/api/account/link returns 400 without a prospect cookie, 401 for an unauthenticated request, and on a verified openagents.com session upserts contact_id oa_user:<userId> + contact_email onto the prospect ref.",
          "id": "account_link_routes.rpc",
          "kind": "bun-test",
          "mode": "rpc",
          "ref": "apps/sarah/src/server.test.ts"
        },
        {
          "description": "Link seam units: the pure contact-row shape (oa_user: prefix, account_link mode), the single account-awareness prompt line (may suggest once, never pushy, null when the store cannot persist a link so Sarah never pitches a link that would not stick), test-mode session parsing, and the no-oa_access-cookie anonymous fast path.",
          "id": "account_link_seam.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/services/account-link.test.ts"
        }
      ],
      "productArea": "account linking + attribution",
      "source": {
        "channel": "sarah-production-conversation",
        "statedBy": "owner",
        "statedOn": "2026-07-09"
      },
      "state": "retired",
      "statement": "Sarah prompts the user to create an account without leaving the conversation",
      "surface": "sarah",
      "verification": "RETIRED 2026-07-10: the Sarah surface (web /sarah page and every /sarah/api/* route, apps/sarah) was removed at owner direction (“all sarah shit must die”, epic #8610; supersedes the rev-24 retention of Sarah routes as regression substrate). The statement is preserved verbatim per the behavior-contract discipline; the oracle refs below are historical (their files were deleted with apps/sarah at openagents commit history ≤ 2026-07-10) and no longer run in any sweep. Historical verification: bun test src/server.test.ts and src/services/account-link.test.ts inside apps/sarah; runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main."
    },
    {
      "authorityBoundary": "This contract binds Sarah's knowledge object (KHS-5, #8604): her persona/playbook/knowledge live in a typed Blueprint — facts with per-fact provenance ({source, ref, at}; owner_kb_v2 | owner_directive | promise_registry | deal_rules | learning_receipt:<id>), versioned revisions (retire is a new revision, never a delete), and admin-guarded operator writes with a change note. The KB doc is GENERATED from the blueprint (render-kb-from-blueprint.ts), not hand-edited. Consumption is flag-armed (SARAH_BLUEPRINT=1); flag-off keeps the file-based path unchanged. It grants no authority: deal-rules code remains the only pricing authority and the openagents.com API the system of record — blueprint facts inform language, never prices.",
      "blockerRefs": [],
      "contractId": "sarah.blueprint_versioned_provenance.v1",
      "enforcementTier": "unenforced",
      "evidenceRefs": [
        "apps/sarah/src/services/sarah-blueprint.ts",
        "apps/sarah/config/blueprint-seed.json",
        "apps/sarah/scripts/render-kb-from-blueprint.ts",
        "docs/sarah/SARAH_KNOWLEDGE_BASE.md",
        "docs/fable/2026-07-09-sarah-khala-connection-assessment.md",
        "issue:#8604",
        "issue:#8599"
      ],
      "oracles": [
        {
          "description": "Seed → compile roundtrip stability: the checked-in seed loads as revision 1 with typed facts in every section; the committed KB doc is byte-identical to the compiled blueprint output (generated, not hand-edited); parse→render→parse is a fixpoint; and the compiled system prompt preserves Section A ordering (identity → engine → hard rules → knowledge).",
          "id": "blueprint_compile_roundtrip.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/services/sarah-blueprint.test.ts"
        },
        {
          "description": "Revision immutability: adding and retiring facts each create a new receipted revision (changed_by + change_note); the retired fact row remains with revision_retired stamped — never deleted — while leaving the compiled surfaces; retiring twice is a conflict; every fact references the revision that added it.",
          "id": "blueprint_revision_immutability.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/services/sarah-blueprint.test.ts"
        },
        {
          "description": "Provenance required on every fact: all seed facts carry owner_kb_v2 with ref + timestamp; adds with an empty or unknown source (or no change note, or an invalid section) are rejected; typed pricing facts carry dealRuleRefs into the deal-rule config and product facts carry promiseIds into the public registry; an approved winning_answer learning promotes to a playbook fact whose provenance source is the KHS-4 approval receipt ref (learning_receipt:<id>), and pending candidates cannot be promoted.",
          "id": "blueprint_provenance_required.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/services/sarah-blueprint.test.ts"
        },
        {
          "description": "Safe rollout + fail-closed guard: with SARAH_BLUEPRINT unset the file-based instructions path is unchanged (no compiled sections leak in); armed, the compiled blueprint leads while the tool protocol stays; the operator endpoints are admin-bearer-guarded (unarmed → 503, missing/wrong bearer → 401 with nothing written) and a full receipted revision cycle (add → retire → read back) works only with the exact bearer.",
          "id": "blueprint_admin_guard.rpc",
          "kind": "bun-test",
          "mode": "rpc",
          "ref": "apps/sarah/src/services/sarah-blueprint.test.ts"
        }
      ],
      "productArea": "knowledge object + persona compilation",
      "source": {
        "channel": "sarah-production-conversation",
        "statedBy": "owner",
        "statedOn": "2026-07-09"
      },
      "state": "retired",
      "statement": "I want her to have that Blueprint of her own",
      "surface": "sarah",
      "verification": "RETIRED 2026-07-10: the Sarah surface (web /sarah page and every /sarah/api/* route, apps/sarah) was removed at owner direction (“all sarah shit must die”, epic #8610; supersedes the rev-24 retention of Sarah routes as regression substrate). The statement is preserved verbatim per the behavior-contract discipline; the oracle refs below are historical (their files were deleted with apps/sarah at openagents commit history ≤ 2026-07-10) and no longer run in any sweep. Historical verification: bun test src/services/sarah-blueprint.test.ts inside apps/sarah; runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main."
    },
    {
      "authorityBoundary": "This contract binds the /sarah browser surface shell: the top-level 50/50 split, Effect Native Tabs (Blueprint map / chat / actions / receipts), video-pane overlay controls, compact disclosure banner, live GraphFigure Blueprint map (BM-2), and BM-5 deploy-smoke gates. It does not claim GPU media quality or store-submission readiness.",
      "blockerRefs": [],
      "contractId": "sarah.split_screen_blueprint_map.v1",
      "enforcementTier": "unenforced",
      "evidenceRefs": [
        "apps/sarah/src/ui/main.ts",
        "apps/sarah/src/ui/sarah.css",
        "apps/sarah/src/ui/index.html",
        "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs",
        "docs/sarah/SARAH_CONTRACTS.md",
        "issue:#8629",
        "issue:#8631"
      ],
      "oracles": [
        {
          "description": "Effect Native surface tree oracle: the right pane is an EN Tabs node with Blueprint map selected by default, Chat/Actions/Receipts panels kept mounted, transcript+composer inside the Chat panel, card receipts inside the Receipts panel, GraphFigure present, and no standalone Sarah title/caption/control row.",
          "id": "split_layout_surface_tree.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/ui/surface.test.ts"
        },
        {
          "description": "Source layout oracle: the host shell uses a 50/50 viewport split, compact disclosure inside the right shell, EN-keyed video overlay controls, and rejects the old 480px/720px centered grid plus the audited caption/control row strings.",
          "id": "split_layout_source_cutlist.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/contracts/split-layout-contracts.test.ts"
        },
        {
          "description": "BM-5 bus isolation oracle: blueprint_delta fact_added events publish only to the prospect's conversation_ref aliases and never to a concurrent foreign ref (KHS-3).",
          "id": "blueprint_delta_isolation.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/services/prospect-memory.test.ts"
        },
        {
          "description": "BM-5 deploy-smoke oracle: synthetic-prospect e2e smoke asserts split-layout shell markers, owned mint, optional live blueprint_delta learning, and concurrent-ref isolation against the live deployment rail.",
          "id": "bm5_split_blueprint_smoke.e2e",
          "kind": "script",
          "mode": "e2e",
          "ref": "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs"
        }
      ],
      "productArea": "Sarah Blueprint map surface",
      "source": {
        "channel": "openagents-codex-thread",
        "statedBy": "owner",
        "statedOn": "2026-07-09"
      },
      "state": "retired",
      "statement": "the split layout — your video full-height left ~50%, tabbed canvas right (map / chat / actions), with the audit's cut list applied (the caption row, controls row, and the 480px centered grid that made the page mostly padding — the disclosure banner stays, it's a contract).",
      "surface": "sarah",
      "verification": "RETIRED 2026-07-10: the Sarah surface (web /sarah page and every /sarah/api/* route, apps/sarah) was removed at owner direction (“all sarah shit must die”, epic #8610; supersedes the rev-24 retention of Sarah routes as regression substrate). The statement is preserved verbatim per the behavior-contract discipline; the oracle refs below are historical (their files were deleted with apps/sarah at openagents commit history ≤ 2026-07-10) and no longer run in any sweep. Historical verification: bun test src/ui/surface.test.ts src/contracts/split-layout-contracts.test.ts src/services/prospect-memory.test.ts inside apps/sarah; deploy gate: bun apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs (SQ-4 + BM-5)."
    },
    {
      "authorityBoundary": "Binds the owned avatar path in apps/sarah (mint route, owned-renderer speaking bridge, SSE bus). Grants no authority over the render service's internal frame scheduling; hydralisk owns frame truth. The greeting is a fixed line, not a brain turn — it may not invent pricing or claims.",
      "blockerRefs": [],
      "contractId": "sarah.avatar_greets_first.v1",
      "enforcementTier": "unenforced",
      "evidenceRefs": [
        "apps/sarah/src/services/owned-renderer.ts",
        "apps/sarah/src/server.ts",
        "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs",
        "issue:#8621",
        "issue:#8610"
      ],
      "oracles": [
        {
          "description": "Unit: speaking the greeting on a minted owned session publishes the fixed greeting on the SSE transcript bus and streams greeting PCM to the fake render service (speak chunks sharing one event_id, then speak_end); a TTS outage degrades soft (transcript still lands, session unharmed).",
          "id": "avatar_greeting_speaks.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/services/owned-renderer.test.ts"
        },
        {
          "description": "E2E smoke (synthetic prospect against a live deployment): mint a real session and require the greeting transcript on the SSE stream within the deadline; fails loudly otherwise.",
          "id": "avatar_greeting_deadline.smoke",
          "kind": "script",
          "mode": "e2e",
          "ref": "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs"
        }
      ],
      "productArea": "avatar surface + owned renderer",
      "source": {
        "channel": "sarah-production-conversation",
        "statedBy": "owner",
        "statedOn": "2026-07-09"
      },
      "state": "retired",
      "statement": "It is not advancing beyond this. [...] I don't see anything else from her. Fix it now.",
      "surface": "sarah",
      "verification": "RETIRED 2026-07-10: the Sarah surface (web /sarah page and every /sarah/api/* route, apps/sarah) was removed at owner direction (“all sarah shit must die”, epic #8610; supersedes the rev-24 retention of Sarah routes as regression substrate). The statement is preserved verbatim per the behavior-contract discipline; the oracle refs below are historical (their files were deleted with apps/sarah at openagents commit history ≤ 2026-07-10) and no longer run in any sweep. Historical verification: bun test src/services/owned-renderer.test.ts and src/contracts/avatar-ux-contracts.test.ts inside apps/sarah (normal sweep); bun apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs against staging/production for the live gate."
    },
    {
      "authorityBoundary": "Binds the /sarah browser surface and the speak bridge. Browser SpeechRecognition is the v1 transport; a native owned-ASR lane may replace it without weakening this contract, provided speech still reaches the brain and the unavailable case stays typed and visible.",
      "blockerRefs": [],
      "contractId": "sarah.avatar_hears_speech.v1",
      "enforcementTier": "unenforced",
      "evidenceRefs": [
        "apps/sarah/src/ui/avatar-session.ts",
        "apps/sarah/src/server.ts",
        "issue:#8621"
      ],
      "oracles": [
        {
          "description": "Surface source oracle: the owned session wiring constructs browser SpeechRecognition, forwards final utterances to the speak bridge (serialized so fast talkers cannot interleave turns), restarts recognition when the browser ends it, and surfaces a typed fallback card when recognition is unsupported or mic permission is denied — asserted against the UI source so a refactor cannot silently drop the mic path.",
          "id": "avatar_mic_wiring.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/contracts/avatar-ux-contracts.test.ts"
        }
      ],
      "productArea": "avatar surface + owned renderer",
      "source": {
        "channel": "sarah-production-conversation",
        "statedBy": "owner",
        "statedOn": "2026-07-09"
      },
      "state": "retired",
      "statement": "She can't hear a fucking thing I'm saying. Fix it now.",
      "surface": "sarah",
      "verification": "RETIRED 2026-07-10: the Sarah surface (web /sarah page and every /sarah/api/* route, apps/sarah) was removed at owner direction (“all sarah shit must die”, epic #8610; supersedes the rev-24 retention of Sarah routes as regression substrate). The statement is preserved verbatim per the behavior-contract discipline; the oracle refs below are historical (their files were deleted with apps/sarah at openagents commit history ≤ 2026-07-10) and no longer run in any sweep. Historical verification: bun test src/contracts/avatar-ux-contracts.test.ts inside apps/sarah; runs in the package test glob and the repo test:sarah sweep before pushes to main."
    },
    {
      "authorityBoundary": "Binds session admission across apps/sarah and the hydralisk render service compat surface. Every successful browser mint owns exactly one idempotent authoritative server stop independent from local media teardown. Watched sessions (connected WebRTC peer) are never evicted; only peer-less abandoned sessions yield the slot. Capacity truth stays with the render service.",
      "blockerRefs": [],
      "contractId": "sarah.avatar_slot_never_wedges.v1",
      "enforcementTier": "unenforced",
      "evidenceRefs": [
        "apps/sarah/src/ui/avatar-session.ts",
        "apps/sarah/src/ui/avatar-session.test.ts",
        "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs",
        "hydralisk:hydralisk/avatar/session.py#evict_one_stale",
        "issue:#8621"
      ],
      "oracles": [
        {
          "description": "Adversarial one-slot browser lifecycle oracle: constructor/acquire/start/attach/EventSource and owned peer-constructor/acquire/offer/remote-description/EventSource/peer failures each issue exactly one authoritative stop; successful cleanup permits the next mint, while stop 503 remains typed cleanup-unconfirmed and forbids remint. Post-handle attach/disconnect/peer terminals synchronously block the shared client replacement gate before cleanup settles and produce zero additional mint requests unless exact cleanup confirms. Repeated or late handle.stop joins that same proof, and false/throwing beacons use one keepalive fallback.",
          "id": "avatar_slot_release.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/ui/avatar-session.test.ts"
        },
        {
          "description": "E2E smoke: after an abandoned mint (no WebRTC connect), a second mint must succeed by evicting the stale session instead of returning avatar_session_limit; the surface must also send a stop beacon on unload so abandonment is usually explicit.",
          "id": "avatar_slot_eviction.smoke",
          "kind": "script",
          "mode": "e2e",
          "ref": "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs"
        }
      ],
      "productArea": "avatar surface + owned renderer",
      "source": {
        "channel": "sarah-production-conversation",
        "statedBy": "owner",
        "statedOn": "2026-07-09"
      },
      "state": "retired",
      "statement": "i click start conversation and theres some session post or something that took forever and then gave {...avatar_session_limit...} wht the fuck fix it",
      "surface": "sarah",
      "verification": "RETIRED 2026-07-10: the Sarah surface (web /sarah page and every /sarah/api/* route, apps/sarah) was removed at owner direction (“all sarah shit must die”, epic #8610; supersedes the rev-24 retention of Sarah routes as regression substrate). The statement is preserved verbatim per the behavior-contract discipline; the oracle refs below are historical (their files were deleted with apps/sarah at openagents commit history ≤ 2026-07-10) and no longer run in any sweep. Historical verification: bun test src/ui/avatar-session.test.ts src/contracts/avatar-ux-contracts.test.ts inside apps/sarah; bun apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs against staging/production; hydralisk-side eviction verified live 2026-07-09 (two back-to-back mints, second evicted the peer-less first)."
    },
    {
      "authorityBoundary": "Binds only browser-observable Sarah video health and its Effect Native presentation. A decoded frame on a live MediaStream video track grants one short browser-local transport lease; it grants no admission, capacity, provider, reservation, or cost truth. Text and any exact-scope Fleet authority remain independent and available while video recovers.",
      "blockerRefs": [],
      "contractId": "sarah.avatar_media_truth_never_frozen_live.v1",
      "enforcementTier": "unenforced",
      "evidenceRefs": [
        "apps/sarah/src/ui/avatar-media-health.ts",
        "apps/sarah/src/ui/avatar-session-attempt-gate.ts",
        "apps/sarah/src/ui/avatar-start-deadline.ts",
        "apps/sarah/src/ui/avatar-stop-deadline.ts",
        "apps/sarah/src/ui/avatar-video-latch.ts",
        "apps/sarah/src/ui/main.ts",
        "apps/sarah/src/contracts/fleet-continuity-projection.ts",
        "issue:#8610"
      ],
      "oracles": [
        {
          "description": "Deterministic fake-clock/video oracle: no decoded frame never becomes LIVE; a frame on a live video track leases LIVE only until bounded expiry; burst frames renew that internal expiry without projecting state at frame rate; the next frame after stale recovers; requestVideoFrameCallback has a currentTime-advance fallback; hostile clocks/listeners cannot emit invalid leases; and stop removes every callback, timer, and listener.",
          "id": "avatar_browser_media_lease.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/ui/avatar-media-health.test.ts"
        },
        {
          "description": "Effect Native surface oracle: a text-live conversation with stale media renders VIDEO RECONNECTING plus one typed Reconnect video action, explicit accessible fallback copy, and leaves the composer and Fleet surface present; VIDEO LIVE requires the fresh typed lease variant.",
          "id": "avatar_media_status_surface.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/ui/surface.test.ts"
        },
        {
          "description": "Attempt-fence and bounded lifecycle oracles: rapid reconnect actions remain single-flight; an older async completion cannot replace a newer attempt; hung start/stop work reaches a typed deadline, refuses replacement without wedging cleanup, and disposal permanently rejects late completion or restart; pending video-element acquisition is rejected on disposal.",
          "id": "avatar_media_reconnect_fence.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/ui/avatar-session-attempt-gate.test.ts"
        },
        {
          "description": "Deadline and lifecycle units prove a never-settling start after a successful stop releases the interaction transition, visibly blocks replacement, fences its late handle, and runs that handle through bounded cleanup before retry is admitted.",
          "id": "avatar_media_start_deadline.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/ui/avatar-start-deadline.test.ts"
        },
        {
          "description": "Deadline units independently prove stop success, failure, and timeout outcomes are bounded and non-throwing while retaining eventual stop truth after the deadline.",
          "id": "avatar_media_stop_deadline.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/ui/avatar-stop-deadline.test.ts"
        },
        {
          "description": "Disposable video-latch units prove pending and future media-host acquisition rejects with fixed copy after surface disposal, so unmount cannot strand an avatar start awaiting a removed host.",
          "id": "avatar_media_video_latch.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/ui/avatar-video-latch.test.ts"
        }
      ],
      "productArea": "avatar browser media health",
      "source": {
        "channel": "openagents-codex-thread",
        "statedBy": "owner",
        "statedOn": "2026-07-09"
      },
      "state": "retired",
      "statement": "LIVE media requires an actual recent video-frame/transport lease, goes stale on bounded expiry, and exposes an explicit reconnect-media action/status while text and Fleet controls remain available.",
      "surface": "sarah",
      "verification": "RETIRED 2026-07-10: the Sarah surface (web /sarah page and every /sarah/api/* route, apps/sarah) was removed at owner direction (“all sarah shit must die”, epic #8610; supersedes the rev-24 retention of Sarah routes as regression substrate). The statement is preserved verbatim per the behavior-contract discipline; the oracle refs below are historical (their files were deleted with apps/sarah at openagents commit history ≤ 2026-07-10) and no longer run in any sweep. Historical verification: bun test src/ui/avatar-media-health.test.ts src/ui/avatar-session-attempt-gate.test.ts src/ui/avatar-start-deadline.test.ts src/ui/avatar-stop-deadline.test.ts src/ui/avatar-video-latch.test.ts src/ui/surface.test.ts src/contracts/avatar-ux-contracts.test.ts inside apps/sarah; runs in the package and repo test:sarah sweeps."
    },
    {
      "authorityBoundary": "Binds the Sarah clip tier: the closed shippable catalog in apps/sarah/src/services/opener-clips.ts, the /sarah/api/clips routes, the mint greeting:\"client_clip\" option, and the browser clip layer. License law is part of this contract: only the raw MIT Hallo2 512² renders are servable; the CodeFormer-derived *-sr.mp4 variants (S-Lab 1.0, non-commercial) are unrepresentable in the catalog and must never ship. The clip carries owner-approved scripted lines only — it grants no pricing or claims authority. Clip failure always degrades to the live/TTS greeting path (never dead air, never a double greeting).",
      "blockerRefs": [],
      "contractId": "sarah.avatar_opens_with_shippable_opener_clip.v1",
      "enforcementTier": "unenforced",
      "evidenceRefs": [
        "apps/sarah/src/services/opener-clips.ts",
        "apps/sarah/src/ui/avatar-clip-layer.ts",
        "apps/sarah/src/ui/avatar-session.ts",
        "apps/sarah/src/server.ts",
        "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs",
        "docs/sarah/2026-07-09-oav-quality-strategy.md",
        "issue:#8610",
        "issue:#8605"
      ],
      "oracles": [
        {
          "description": "Unit: the clip catalog contains only MIT Hallo2 renders (no *-sr filenames representable), the clips route serves video/mp4 with immutable caching and range support while refusing unknown names, a greeting:\"client_clip\" mint returns the opener clip and publishes ONLY the transcript line (no TTS request — no double greeting), a mint without an available clip falls back to the TTS greeting, and /api/avatar/greet restores the TTS greeting for clip-playback failures.",
          "id": "avatar_opener_clip_tier.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/services/opener-clips.test.ts"
        },
        {
          "description": "Unit: the browser clip layer plays the opener immediately, fades in on playback, holds the final frame until live media is ready before crossfading out, plays canned clips over the live stream, drops the clip on user barge-in, degrades muted-autoplay and unplayable clips to the typed fallback callbacks, and is destroyed on session teardown.",
          "id": "avatar_clip_layer.unit",
          "kind": "bun-test",
          "mode": "unit",
          "ref": "apps/sarah/src/ui/avatar-clip-layer.test.ts"
        },
        {
          "description": "E2E smoke (live deployment): the clips manifest lists the shippable tier with no SR variants, the opener clip URL serves real MP4 bytes with immutable caching, and a greeting:\"client_clip\" mint returns the opener clip while the greeting transcript still lands on SSE within the deadline.",
          "id": "avatar_opener_clip.smoke",
          "kind": "script",
          "mode": "e2e",
          "ref": "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs"
        }
      ],
      "productArea": "avatar surface + clip tier",
      "source": {
        "channel": "owner-directive",
        "statedBy": "owner",
        "statedOn": "2026-07-10"
      },
      "state": "retired",
      "statement": "Get the Hallo2-quality pre-rendered clip technology working in the LIVE web /sarah surface ASAP — actually fucking working. [openers-v2 playback verdict, verbatim: \"those v2s are much better - opener-05-show-you-hallo2.mp4 is for example close to shippable so proceed in that direction.\"]",
      "surface": "sarah",
      "verification": "RETIRED 2026-07-10: the Sarah surface (web /sarah page and every /sarah/api/* route, apps/sarah) was removed at owner direction (“all sarah shit must die”, epic #8610; supersedes the rev-24 retention of Sarah routes as regression substrate). The statement is preserved verbatim per the behavior-contract discipline; the oracle refs below are historical (their files were deleted with apps/sarah at openagents commit history ≤ 2026-07-10) and no longer run in any sweep. Historical verification: bun test src/services/opener-clips.test.ts src/ui/avatar-clip-layer.test.ts src/contracts/avatar-ux-contracts.test.ts inside apps/sarah (normal sweep); bun apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs against staging/production for the live gate."
    },
    {
      "authorityBoundary": "This contract binds the owner-facing Sarah fleet-management outcome: named Codex, Claude, and Grok work streams may use owner-local or managed-cloud capacity, but all authority remains in authenticated owner scope, typed run/claim/approval services, named isolated provider accounts, and independent closeout evidence. It does not authorize pooled third-party subscriptions, default provider homes, raw-event publication, spend, deployment, or repository mutation outside the bounded approved plan.",
      "blockerRefs": [
        "issue:#8637",
        "issue:#8633",
        "issue:#8639",
        "issue:#8640"
      ],
      "contractId": "sarah.fleet_command_multi_harness.v1",
      "enforcementTier": "unenforced",
      "evidenceRefs": [
        "docs/sol/MASTER_ROADMAP.md",
        "docs/sol/issues/fc-1-run-contract.md",
        "docs/sol/issues/fc-2-local-executor.md",
        "docs/sol/issues/fc-3-supervision.md",
        "docs/sol/issues/fc-5-dogfood.md"
      ],
      "oracles": [],
      "productArea": "Sarah Fleet Command",
      "source": {
        "channel": "openagents-codex-thread",
        "statedBy": "owner",
        "statedOn": "2026-07-09"
      },
      "state": "retired",
      "statement": "I should clarify that my top priority is having Sarah able to manage coding fleets more or less immediately, like all of our previous fleet ideas using Khala and the clients and such. I need those updated to the point where we can start delegating out via Sarah multiple streams of work using the different Codex, Claude, and Grok accounts, some of which will be on my desktop, some of which may be in the cloud, but I need to unblock our coding right now.",
      "surface": "sarah",
      "verification": "RETIRED 2026-07-10: the Sarah surface (web /sarah page and every /sarah/api/* route, apps/sarah) was removed at owner direction (“all sarah shit must die”, epic #8610; supersedes the rev-24 retention of Sarah routes as regression substrate). The statement is preserved verbatim per the behavior-contract discipline; the oracle refs below are historical (their files were deleted with apps/sarah at openagents commit history ≤ 2026-07-10) and no longer run in any sweep. Historical verification: Pending #8637/#8633/#8639 and the #8640 Phase A live dogfood receipt: Sarah starts and manages at least three simultaneous pinned real work units across Codex, Claude, and Grok, including a steer or approval round trip, with zero duplicate claims or default-home execution and verified closeouts visible after reconnect."
    }
  ],
  "schemaVersion": BehaviorContractSchemaVersion,
  "version": "2026-07-10.1"
}
