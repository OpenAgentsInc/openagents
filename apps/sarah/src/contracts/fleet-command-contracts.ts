import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts"

/** Owner-stated Sarah Fleet Command behavior, pending its live dogfood oracle. */
export const SARAH_FLEET_COMMAND_CONTRACTS_DOC_PATH =
  "docs/sarah/SARAH_CONTRACTS.md"

export const sarahFleetCommandContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "This contract binds the owner-facing Sarah fleet-management outcome: named Codex, Claude, and Grok work streams may use owner-local or managed-cloud capacity, but all authority remains in authenticated owner scope, typed run/claim/approval services, named isolated provider accounts, and independent closeout evidence. It does not authorize pooled third-party subscriptions, default provider homes, raw-event publication, spend, deployment, or repository mutation outside the bounded approved plan.",
      blockerRefs: [
        "issue:#8637",
        "issue:#8633",
        "issue:#8639",
        "issue:#8640",
      ],
      contractId: "sarah.fleet_command_multi_harness.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/MASTER_ROADMAP.md",
        "docs/sol/issues/fc-1-run-contract.md",
        "docs/sol/issues/fc-2-local-executor.md",
        "docs/sol/issues/fc-3-supervision.md",
        "docs/sol/issues/fc-5-dogfood.md",
      ],
      oracles: [],
      productArea: "Sarah Fleet Command",
      source: {
        channel: "openagents-codex-thread",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "pending",
      statement:
        "I should clarify that my top priority is having Sarah able to manage coding fleets more or less immediately, like all of our previous fleet ideas using Khala and the clients and such. I need those updated to the point where we can start delegating out via Sarah multiple streams of work using the different Codex, Claude, and Grok accounts, some of which will be on my desktop, some of which may be in the cloud, but I need to unblock our coding right now.",
      surface: "sarah",
      verification:
        "Pending #8637/#8633/#8639 and the #8640 Phase A live dogfood receipt: Sarah starts and manages at least three simultaneous pinned real work units across Codex, Claude, and Grok, including a steer or approval round trip, with zero duplicate claims or default-home execution and verified closeouts visible after reconnect.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-09.1",
}
