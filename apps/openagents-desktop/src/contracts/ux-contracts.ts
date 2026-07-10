import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts"

export const openAgentsDesktopUxContractRegistry: BehaviorContractRegistryDocument = {
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-10.3",
  contracts: [
    {
      contractId: "openagents_desktop.seam.codex_recent_history_projection.v1",
      state: "enforced",
      surface: "openagents-desktop",
      productArea: "local Codex history",
      enforcementTier: "test-sweep",
      blockerRefs: [],
      source: { channel: "owner-codex-session", statedBy: "owner", statedOn: "2026-07-10" },
      statement: "When local Codex history is available, opening OpenAgents Desktop shows every top-level Codex chat updated in the last 24 hours in newest-first order while excluding known child, sub-agent, and side sessions. Selecting a chat shows its basic metadata and bounded recent user and assistant messages; unavailable history stays an honest empty state.",
      authorityBoundary: "The local history adapter is read-only and only projects bounded metadata and conversational display text through the fixed Electron bridge. It grants no session resume, send, filesystem browsing, cloud sync, worker dispatch, or provider authority.",
      seam: { client: "apps/openagents-desktop/src/renderer/boot.ts", server: "apps/openagents-desktop/src/codex-history-worker.ts" },
      evidenceRefs: ["apps/openagents-desktop/src/codex-history.ts", "apps/openagents-desktop/src/codex-history-worker.ts"],
      oracles: [{ id: "codex_recent_history_projection.e2e", kind: "bun-test", mode: "e2e", ref: "apps/openagents-desktop/tests/codex-history.e2e.test.ts", description: "Exercises the bounded rollout projection and oversized-thread first-content path." }],
      verification: "bun test apps/openagents-desktop/tests/codex-history.e2e.test.ts; runs in the normal desktop test sweep.",
    },
    {
      contractId: "openagents_desktop.chat.thread_first_content_under_50ms.v1",
      state: "enforced",
      surface: "openagents-desktop",
      productArea: "thread loading performance",
      enforcementTier: "test-sweep",
      blockerRefs: [],
      source: { channel: "owner-codex-session", statedBy: "owner", statedOn: "2026-07-10" },
      statement: "Threads must always show their first bounded message content in less than 50 milliseconds, regardless of total rollout size. Large threads must be chunked; full-rollout parsing is forbidden on the selection path.",
      authorityBoundary: "The 50-millisecond budget covers local first-content projection after selection. It does not authorize loading unbounded history, exposing raw events, or moving filesystem work onto Electron's main process.",
      evidenceRefs: ["apps/openagents-desktop/src/codex-history.ts", "apps/openagents-desktop/src/codex-history-worker.ts"],
      oracles: [{ id: "oversized_rollout_first_content.performance", kind: "bun-test", mode: "e2e", ref: "apps/openagents-desktop/tests/codex-history.e2e.test.ts", description: "Creates a 256 MiB sparse rollout and requires bounded first-content projection to finish under 50 ms." }],
      verification: "bun test apps/openagents-desktop/tests/codex-history.e2e.test.ts enforces the 50 ms wall-clock budget in the normal desktop test sweep.",
    },
  ],
}
