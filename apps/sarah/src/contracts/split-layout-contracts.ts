import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts"

/**
 * Sarah split-screen Blueprint map behavior contract (BM-3, #8629).
 *
 * The statement is the owner's implementation directive kept verbatim: the
 * split layout is not a visual preference, it is the contracted Sarah shell
 * until a later behavior-contract version supersedes it.
 */
export const SARAH_SPLIT_LAYOUT_CONTRACTS_DOC_PATH =
  "docs/sarah/SARAH_CONTRACTS.md"

export const sarahSplitLayoutContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "This contract binds the /sarah browser surface shell: the top-level 50/50 split, Effect Native Tabs (Blueprint map / chat / actions / receipts), video-pane overlay controls, compact disclosure banner, live GraphFigure Blueprint map (BM-2), and BM-5 deploy-smoke gates. It does not claim GPU media quality or store-submission readiness.",
      blockerRefs: [],
      contractId: "sarah.split_screen_blueprint_map.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/sarah/src/ui/main.ts",
        "apps/sarah/src/ui/sarah.css",
        "apps/sarah/src/ui/index.html",
        "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs",
        "docs/sarah/SARAH_CONTRACTS.md",
        "issue:#8629",
        "issue:#8631",
      ],
      oracles: [
        {
          description:
            "Effect Native surface tree oracle: the right pane is an EN Tabs node with Blueprint map selected by default, Chat/Actions/Receipts panels kept mounted, transcript+composer inside the Chat panel, card receipts inside the Receipts panel, GraphFigure present, and no standalone Sarah title/caption/control row.",
          id: "split_layout_surface_tree.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/ui/surface.test.ts",
        },
        {
          description:
            "Source layout oracle: the host shell uses a 50/50 viewport split, compact disclosure inside the right shell, EN-keyed video overlay controls, and rejects the old 480px/720px centered grid plus the audited caption/control row strings.",
          id: "split_layout_source_cutlist.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/contracts/split-layout-contracts.test.ts",
        },
        {
          description:
            "BM-5 bus isolation oracle: blueprint_delta fact_added events publish only to the prospect's conversation_ref aliases and never to a concurrent foreign ref (KHS-3).",
          id: "blueprint_delta_isolation.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/services/prospect-memory.test.ts",
        },
        {
          description:
            "BM-5 deploy-smoke oracle: synthetic-prospect e2e smoke asserts split-layout shell markers, owned mint, optional live blueprint_delta learning, and concurrent-ref isolation against the live deployment rail.",
          id: "bm5_split_blueprint_smoke.e2e",
          kind: "script",
          mode: "e2e",
          ref: "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs",
        },
      ],
      productArea: "Sarah Blueprint map surface",
      source: {
        channel: "openagents-codex-thread",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "enforced",
      statement:
        "the split layout — your video full-height left ~50%, tabbed canvas right (map / chat / actions), with the audit's cut list applied (the caption row, controls row, and the 480px centered grid that made the page mostly padding — the disclosure banner stays, it's a contract).",
      surface: "sarah",
      verification:
        "bun test src/ui/surface.test.ts src/contracts/split-layout-contracts.test.ts src/services/prospect-memory.test.ts inside apps/sarah; deploy gate: bun apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs (SQ-4 + BM-5).",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-09.2",
}
