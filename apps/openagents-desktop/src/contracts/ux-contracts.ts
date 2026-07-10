/**
 * OpenAgents Desktop owns its behavior contracts here. These are intentionally
 * separate from the frozen Khala Code migration registry.
 */
export const openAgentsDesktopUxContractRegistry = {
  version: "2026-07-10.1",
  contracts: [
    {
      contractId: "openagents_desktop.seam.codex_recent_history_projection.v1",
      state: "enforced",
      statement:
        "When local Codex history is available, opening OpenAgents Desktop shows every top-level Codex chat updated in the last 24 hours in newest-first order while excluding known child, sub-agent, and side sessions. Selecting a chat shows its basic metadata and bounded recent user and assistant messages; unavailable history stays an honest empty state.",
      authorityBoundary:
        "The local history adapter is read-only and only projects bounded metadata and conversational display text through the fixed Electron bridge. It grants no session resume, send, filesystem browsing, cloud sync, worker dispatch, or provider authority.",
      verification:
        "apps/openagents-desktop/tests/codex-history.test.ts exercises the fixture parser/projection; apps/openagents-desktop/src/renderer/shell.test.ts proves the selected metadata view. Both run in the desktop test sweep.",
    },
  ],
} as const
