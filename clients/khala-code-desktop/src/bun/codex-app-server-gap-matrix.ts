import {
  KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT,
} from "./codex-parity-contract"

export const KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX_REFERENCE_COMMIT =
  KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT

export type KhalaCodeCodexAppServerGapDecision =
  | "covered_by_app_server"
  | "khala_adapter_with_test"
  | "upstream_app_server_gap"

export type KhalaCodeCodexAppServerGapRow = Readonly<{
  id: string
  area: string
  decision: KhalaCodeCodexAppServerGapDecision
  codexSourceRefs: readonly string[]
  slashCommands: readonly string[]
  appServerMethods: readonly string[]
  experimentalAppServerMethods?: readonly string[]
  upstreamGapId?: string
  khalaAdapter?: string
  rationale: string
  linkedIssues: readonly string[]
  testRefs: readonly string[]
  updateTrigger: string
}>

export const KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX: readonly KhalaCodeCodexAppServerGapRow[] = [
  {
    id: "thread-turn-session-lifecycle",
    area: "threads, turns, review, goals, and lifecycle commands",
    decision: "covered_by_app_server",
    codexSourceRefs: [
      "codex-rs/tui/src/slash_command.rs",
      "codex-rs/app-server-protocol/schema/typescript/ClientRequest.ts",
      "codex-rs/app-server-protocol/schema/typescript/v2/ThreadItem.ts",
    ],
    slashCommands: [
      "review",
      "rename",
      "new",
      "archive",
      "delete",
      "resume",
      "fork",
      "compact",
      "goal",
    ],
    appServerMethods: [
      "review/start",
      "thread/name/set",
      "thread/start",
      "thread/archive",
      "thread/delete",
      "thread/resume",
      "thread/fork",
      "thread/compact/start",
      "thread/goal/set",
      "thread/goal/get",
      "thread/goal/clear",
      "thread/read",
      "thread/list",
      "turn/start",
      "turn/steer",
      "turn/interrupt",
    ],
    rationale:
      "Codex already exposes thread, turn, review, compact, and goal APIs; Khala should keep using those instead of recreating session state.",
    linkedIssues: [
      "https://github.com/OpenAgentsInc/openagents/issues/7782",
      "https://github.com/OpenAgentsInc/openagents/issues/7785",
      "https://github.com/OpenAgentsInc/openagents/issues/7795",
    ],
    testRefs: [
      "clients/khala-code-desktop/tests/codex-app-server-chat-runtime.test.ts",
      "clients/khala-code-desktop/tests/codex-slash-commands.test.ts",
      "clients/khala-code-desktop/tests/codex-parity-contract.test.ts",
    ],
    updateTrigger:
      "Update when the pinned Codex thread, turn, goal, review, or SlashCommand contract changes.",
  },
  {
    id: "settings-ecosystem-account-surfaces",
    area: "models, permissions, experimental flags, usage, MCP, apps, plugins, and logout",
    decision: "covered_by_app_server",
    codexSourceRefs: [
      "codex-rs/tui/src/slash_command.rs",
      "codex-rs/app-server-protocol/schema/typescript/ClientRequest.ts",
      "codex-rs/app-server/src/request_processors.rs",
    ],
    slashCommands: [
      "model",
      "permissions",
      "experimental",
      "usage",
      "mcp",
      "app",
      "apps",
      "plugins",
      "logout",
    ],
    appServerMethods: [
      "model/list",
      "modelProvider/capabilities/read",
      "permissionProfile/list",
      "experimentalFeature/list",
      "experimentalFeature/enablement/set",
      "account/usage/read",
      "account/rateLimits/read",
      "account/rateLimitResetCredit/consume",
      "account/read",
      "mcpServerStatus/list",
      "mcpServer/resource/read",
      "mcpServer/tool/call",
      "mcpServer/oauth/login",
      "config/mcpServer/reload",
      "app/list",
      "plugin/list",
      "plugin/read",
      "plugin/installed",
      "account/logout",
      "config/read",
      "config/value/write",
      "config/batchWrite",
      "configRequirements/read",
    ],
    khalaAdapter:
      "Khala owns the web settings panels and cards, but values and mutations must round-trip through app-server.",
    rationale:
      "These surfaces have stable app-server processors. Khala UI can add navigation and layout without owning parallel config.",
    linkedIssues: [
      "https://github.com/OpenAgentsInc/openagents/issues/7788",
      "https://github.com/OpenAgentsInc/openagents/issues/7795",
    ],
    testRefs: [
      "clients/khala-code-desktop/tests/codex-settings.test.ts",
      "clients/khala-code-desktop/tests/codex-ecosystem.test.ts",
      "clients/khala-code-desktop/tests/rpc-handlers.test.ts",
    ],
    updateTrigger:
      "Update when Codex adds/removes model, permission, account, ecosystem, or config app-server methods.",
  },
  {
    id: "desktop-local-ui-adapters",
    area: "desktop shell actions and display-only UI",
    decision: "khala_adapter_with_test",
    codexSourceRefs: [
      "codex-rs/tui/src/slash_command.rs",
      "codex-rs/tui/src/bottom_pane/command_popup.rs",
      "codex-rs/tui/src/status_indicator_widget.rs",
    ],
    slashCommands: [
      "copy",
      "raw",
      "status",
      "debug-config",
      "title",
      "quit",
      "exit",
      "feedback",
      "rollout",
      "clear",
      "test-approval",
    ],
    appServerMethods: [
      "config/read",
      "account/usage/read",
      "feedback/upload",
      "getAuthStatus",
      "getConversationSummary",
    ],
    khalaAdapter:
      "Keep these as tiny desktop UI adapters with tests because they operate on browser selection, window title, local display state, or debug-only diagnostics.",
    rationale:
      "These commands are shell affordances, not Codex Core execution. Khala can implement the desktop behavior while still reading Codex state through app-server where needed.",
    linkedIssues: [
      "https://github.com/OpenAgentsInc/openagents/issues/7785",
      "https://github.com/OpenAgentsInc/openagents/issues/7795",
    ],
    testRefs: [
      "clients/khala-code-desktop/tests/app-shell.test.ts",
      "clients/khala-code-desktop/tests/codex-slash-commands.test.ts",
    ],
    updateTrigger:
      "Update when Codex moves a shell-only slash command into app-server or changes TUI availability semantics.",
  },
  {
    id: "tui-preferences-and-appearance",
    area: "keymap, vim mode, statusline, theme, pets, and personality",
    decision: "upstream_app_server_gap",
    codexSourceRefs: [
      "codex-rs/tui/src/slash_command.rs",
      "codex-rs/tui/src/theme_picker.rs",
      "codex-rs/tui/src/keymap.rs",
      "codex-rs/tui/src/bottom_pane/status_line_setup.rs",
    ],
    slashCommands: [
      "keymap",
      "vim",
      "statusline",
      "theme",
      "pets",
      "personality",
    ],
    appServerMethods: [
      "config/read",
      "config/value/write",
      "config/batchWrite",
    ],
    upstreamGapId: "codex.app_server.gap.tui_preferences",
    khalaAdapter:
      "Khala may show desktop controls only after mapping each control to app-server config keys or a new upstream processor.",
    rationale:
      "The TUI owns picker behavior and visual preferences today. Khala should not invent durable config semantics without a Codex-owned method or tested config-key mapping.",
    linkedIssues: [
      "https://github.com/OpenAgentsInc/openagents/issues/7785",
      "https://github.com/OpenAgentsInc/openagents/issues/7795",
    ],
    testRefs: [
      "clients/khala-code-desktop/tests/codex-slash-commands.test.ts",
      "clients/khala-code-desktop/tests/codex-app-server-gap-matrix.test.ts",
    ],
    updateTrigger:
      "Update when Codex exposes preference metadata or changes TUI preference command behavior.",
  },
  {
    id: "workspace-knowledge-memory-and-import",
    area: "AGENTS.md init/import, memories, skills, hooks, and debug memory hooks",
    decision: "upstream_app_server_gap",
    codexSourceRefs: [
      "codex-rs/tui/src/slash_command.rs",
      "codex-rs/tui/src/bottom_pane/skills_toggle_view.rs",
      "codex-rs/app-server-protocol/schema/typescript/ClientRequest.ts",
    ],
    slashCommands: [
      "memories",
      "skills",
      "import",
      "hooks",
      "init",
      "debug-m-drop",
      "debug-m-update",
    ],
    appServerMethods: [
      "skills/list",
      "skills/config/write",
      "skills/extraRoots/set",
      "hooks/list",
      "externalAgentConfig/detect",
      "externalAgentConfig/import",
      "externalAgentConfig/import/readHistories",
      "fs/readFile",
      "fs/writeFile",
      "fs/getMetadata",
    ],
    upstreamGapId: "codex.app_server.gap.memory_and_import_management",
    khalaAdapter:
      "Implemented wrapper coverage reads skills, hooks, importable external config, import histories, and fs metadata through Codex app-server, and passes skill/import/fs mutations through unchanged. Memory mutation and rich slash-command flows still need Codex-owned semantics.",
    rationale:
      "The stable app-server primitives are now wrapped for desktop settings and diagnostics. The row remains an upstream gap because /memories, /init, and debug memory slash-command semantics are still TUI-owned and should not become a second Khala implementation.",
    linkedIssues: [
      "https://github.com/OpenAgentsInc/openagents/issues/7785",
      "https://github.com/OpenAgentsInc/openagents/issues/7795",
    ],
    testRefs: [
      "clients/khala-code-desktop/tests/codex-slash-commands.test.ts",
      "clients/khala-code-desktop/tests/codex-ecosystem.test.ts",
      "clients/khala-code-desktop/tests/rpc-handlers.test.ts",
      "clients/khala-code-desktop/tests/codex-app-server-gap-matrix.test.ts",
    ],
    updateTrigger:
      "Update when Codex adds app-server memory, AGENTS.md init, or external import processors.",
  },
  {
    id: "multi-agent-side-conversation-and-plan",
    area: "auto-review approval, plan editing, agents, subagents, side conversations, and BTW steering",
    decision: "upstream_app_server_gap",
    codexSourceRefs: [
      "codex-rs/tui/src/slash_command.rs",
      "codex-rs/app-server-protocol/schema/typescript/ClientRequest.ts",
      "codex-rs/app-server-protocol/schema/typescript/ServerRequest.ts",
    ],
    slashCommands: [
      "approve",
      "plan",
      "agent",
      "subagents",
      "side",
      "btw",
    ],
    appServerMethods: [
      "turn/steer",
      "thread/fork",
      "thread/inject_items",
      "thread/approveGuardianDeniedAction",
      "thread/metadata/update",
      "thread/read",
    ],
    upstreamGapId: "codex.app_server.gap.side_agent_plan_controls",
    khalaAdapter:
      "Khala can add sidebar affordances, but side-thread, subagent, plan-edit, and auto-review semantics must stay aligned to Codex app-server events.",
    rationale:
      "The TUI currently combines command parsing, popup state, and turn/server-request state. Khala should request narrower app-server methods before cloning that behavior.",
    linkedIssues: [
      "https://github.com/OpenAgentsInc/openagents/issues/7785",
      "https://github.com/OpenAgentsInc/openagents/issues/7795",
    ],
    testRefs: [
      "clients/khala-code-desktop/tests/codex-approval-decisions.test.ts",
      "clients/khala-code-desktop/tests/codex-slash-commands.test.ts",
      "clients/khala-code-desktop/tests/codex-app-server-gap-matrix.test.ts",
    ],
    updateTrigger:
      "Update when Codex stabilizes multi-agent, side-conversation, plan-edit, or auto-review app-server APIs.",
  },
  {
    id: "ide-file-mention-and-diff",
    area: "IDE context, file mention insertion, and diff viewers",
    decision: "upstream_app_server_gap",
    codexSourceRefs: [
      "codex-rs/tui/src/slash_command.rs",
      "codex-rs/tui/src/bottom_pane/mentions_v2",
      "codex-rs/app-server-protocol/schema/typescript/ClientRequest.ts",
    ],
    slashCommands: [
      "ide",
      "diff",
      "mention",
    ],
    appServerMethods: [
      "fuzzyFileSearch",
      "gitDiffToRemote",
      "fs/readDirectory",
      "fs/readFile",
      "config/read",
    ],
    upstreamGapId: "codex.app_server.gap.ide_mentions_diff",
    khalaAdapter:
      "Khala can build richer browser pickers, but must source candidates and diff content from app-server methods or request missing APIs.",
    rationale:
      "Mention and diff UI are attractive desktop advantages, but Codex owns workspace interpretation, ignored files, and diff semantics.",
    linkedIssues: [
      "https://github.com/OpenAgentsInc/openagents/issues/7785",
      "https://github.com/OpenAgentsInc/openagents/issues/7795",
    ],
    testRefs: [
      "clients/khala-code-desktop/tests/codex-slash-commands.test.ts",
      "clients/khala-code-desktop/tests/codex-app-server-gap-matrix.test.ts",
    ],
    updateTrigger:
      "Update when Codex exposes full local diff, IDE context, or mention metadata through app-server.",
  },
  {
    id: "windows-sandbox-setup-and-readable-roots",
    area: "Windows sandbox setup and sandbox readable-root mutation",
    decision: "upstream_app_server_gap",
    codexSourceRefs: [
      "codex-rs/tui/src/slash_command.rs",
      "codex-rs/app-server-protocol/schema/typescript/v2/WindowsSandboxSetupStartParams.ts",
      "codex-rs/app-server/src/request_processors/windows_sandbox_processor.rs",
    ],
    slashCommands: [
      "setup-default-sandbox",
      "sandbox-add-read-dir",
    ],
    appServerMethods: [
      "windowsSandbox/setupStart",
      "windowsSandbox/readiness",
      "config/read",
      "config/value/write",
    ],
    upstreamGapId: "codex.app_server.gap.windows_sandbox_read_roots",
    khalaAdapter:
      "Use windowsSandbox/setupStart/readiness where possible; request a narrower readable-root method before implementing /sandbox-add-read-dir behavior in Khala.",
    rationale:
      "Codex exposes setup/readiness, but the TUI command surface includes read-root mutation details that need a precise app-server contract.",
    linkedIssues: [
      "https://github.com/OpenAgentsInc/openagents/issues/7785",
      "https://github.com/OpenAgentsInc/openagents/issues/7795",
    ],
    testRefs: [
      "clients/khala-code-desktop/tests/codex-slash-commands.test.ts",
      "clients/khala-code-desktop/tests/codex-app-server-gap-matrix.test.ts",
    ],
    updateTrigger:
      "Update when Codex changes Windows sandbox setup/readiness params or adds read-root mutation support.",
  },
  {
    id: "background-terminal-management",
    area: "background terminal listing and cleanup",
    decision: "upstream_app_server_gap",
    codexSourceRefs: [
      "codex-rs/tui/src/slash_command.rs",
      "codex-rs/app-server-protocol/src/protocol/common.rs",
      "codex-rs/app-server/src/request_processors/thread_processor.rs",
    ],
    slashCommands: [
      "ps",
      "stop",
    ],
    appServerMethods: [],
    experimentalAppServerMethods: [
      "thread/backgroundTerminals/list",
      "thread/backgroundTerminals/clean",
      "thread/backgroundTerminals/terminate",
    ],
    upstreamGapId: "codex.app_server.gap.background_terminals",
    khalaAdapter:
      "Khala may call the experimental methods only behind parity checks; stable product copy should keep this row as an upstream gap until Codex stabilizes the methods.",
    rationale:
      "Codex protocol marks these methods experimental, so Khala must avoid treating them as stable feature parity.",
    linkedIssues: [
      "https://github.com/OpenAgentsInc/openagents/issues/7785",
      "https://github.com/OpenAgentsInc/openagents/issues/7795",
    ],
    testRefs: [
      "clients/khala-code-desktop/tests/codex-slash-commands.test.ts",
      "clients/khala-code-desktop/tests/codex-app-server-gap-matrix.test.ts",
    ],
    updateTrigger:
      "Update when Codex stabilizes, renames, or removes background terminal app-server methods.",
  },
]

export const KHALA_CODE_CODEX_APP_SERVER_GAP_DOC_PATH =
  "docs/khala-code/2026-07-01-codex-app-server-gap-matrix.md"

export const KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX_ISSUE =
  "https://github.com/OpenAgentsInc/openagents/issues/7795"
