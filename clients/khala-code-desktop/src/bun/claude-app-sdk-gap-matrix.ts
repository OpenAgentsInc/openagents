export type ClaudeAppSdkGapMatrixRow = {
  readonly id: string
  readonly phase: "phase_1" | "phase_2" | "phase_3"
  readonly status: "covered" | "deferred"
  readonly note: string
}

export const CLAUDE_APP_SDK_GAP_MATRIX: readonly ClaudeAppSdkGapMatrixRow[] = [
  {
    id: "claude.phase1.chat_stream",
    phase: "phase_1",
    status: "covered",
    note: "query() streams assistant text, reasoning blocks, tool_use/tool_result events, result status, and exact usage into neutral chat turn events.",
  },
  {
    id: "claude.phase1.interrupt",
    phase: "phase_1",
    status: "covered",
    note: "Desktop stop maps to Query.interrupt(); scoped teardown still calls close() and aborts the owned AbortController.",
  },
  {
    id: "claude.phase1.session_resume",
    phase: "phase_1",
    status: "covered",
    note: "Desktop session ids persist to ~/.khala-code/claude-sessions.json v1, with KHALA_CODE_DESKTOP_CLAUDE_STATE_PATH override.",
  },
  {
    id: "claude.phase2.approvals",
    phase: "phase_2",
    status: "covered",
    note: "canUseTool is bridged through a Deferred/Queue-backed approval service and resolved by renderer Claude-native allow/deny decisions.",
  },
  {
    id: "claude.phase2.telemetry_ingest",
    phase: "phase_2",
    status: "covered",
    note: "Exact SDK result usage/modelUsage is recorded on the desktop stats token path as pylon-claude-direct-local; khala_fleet MCP is injected via query options, and settings project SDK init/model/account data.",
  },
  {
    id: "claude.phase3.sidebar_catalog",
    phase: "phase_3",
    status: "deferred",
    note: "SDK listSessions()/getSessionMessages() backing for the sidebar is deferred to T8.4.",
  },
]
