export const CLAUDE_AGENT_SDK_PARITY_VERSION = "^0.3.172"

export type ClaudeParityContractRow = {
  readonly id: string
  readonly sdkSurface: string
  readonly status: "covered" | "deferred"
  readonly verification: string
}

export const CLAUDE_PARITY_CONTRACT: readonly ClaudeParityContractRow[] = [
  {
    id: "claude.phase1.query_stream",
    sdkSurface: "query()",
    status: "covered",
    verification: "claude-app-sdk-chat-runtime.test.ts projects assistant, tool, result, and stream_event messages.",
  },
  {
    id: "claude.phase2.approvals",
    sdkSurface: "query({ options.canUseTool })",
    status: "covered",
    verification: "Claude-native allow/deny decisions are queued for desktop and auto-denied with headless_auto_deny in headless mode.",
  },
  {
    id: "claude.phase2.telemetry",
    sdkSurface: "result.usage/result.modelUsage",
    status: "covered",
    verification: "Exact result usage is written to the desktop token event path; persistent failures create Inbox flags.",
  },
  {
    id: "claude.phase3.sidebar",
    sdkSurface: "listSessions()/getSessionMessages()",
    status: "covered",
    verification: "The runtime's listThreads/readThread methods are backed directly by mocked SDK session APIs.",
  },
  {
    id: "claude.phase3.slash",
    sdkSurface: "supportedCommands() and system/init.slash_commands",
    status: "covered",
    verification: "The slash registry refreshes from supportedCommands(), merges init slash_commands, and sends /name args as the prompt.",
  },
]
