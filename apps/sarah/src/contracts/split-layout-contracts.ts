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
        "This contract binds the /sarah browser surface shell only: the top-level split, Effect Native Tabs composition, video-pane overlay controls, compact disclosure banner, and removal of the audited caption/control/centered-grid padding. It does not claim the live Blueprint graph exists yet (BM-2) and does not replace BM-5's later screenshot smoke gate.",
      blockerRefs: [],
      contractId: "sarah.split_screen_blueprint_map.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/sarah/src/ui/main.ts",
        "apps/sarah/src/ui/sarah.css",
        "apps/sarah/src/ui/index.html",
        "docs/sarah/SARAH_CONTRACTS.md",
        "issue:#8629",
      ],
      oracles: [
        {
          description:
            "Effect Native surface tree oracle: the right pane is an EN Tabs node with Blueprint map selected by default, Chat/Actions/Receipts panels kept mounted, transcript+composer inside the Chat panel, card receipts inside the Receipts panel, and no standalone Sarah title/caption/control row.",
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
        "bun test src/ui/surface.test.ts src/contracts/split-layout-contracts.test.ts inside apps/sarah; runs in the package test glob and gives BM-5 a named contract for the later screenshot smoke deploy gate.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-09.1",
}
