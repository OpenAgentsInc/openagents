export type KhalaCodeDesktopSlashCommandGroup =
  | "background"
  | "diagnostics"
  | "ecosystem"
  | "exit"
  | "session"
  | "settings"
  | "turn_task"
  | "workspace"

export type KhalaCodeDesktopSlashCommandVisibility =
  | { readonly kind: "always" }
  | { readonly kind: "debug" }
  | { readonly kind: "not_android" }
  | { readonly kind: "platform"; readonly platforms: readonly string[] }

export type KhalaCodeDesktopSlashCommandDispatch =
  | {
      readonly kind: "app_server"
      readonly method: string
      readonly appServerDependency?: string
      readonly experimental?: boolean
      readonly requiresArgs?: boolean
      readonly requiresThread?: boolean
    }
  | {
      readonly kind: "client"
      readonly action: string
    }
  | {
      readonly kind: "gap"
      readonly dependency: string
      readonly unavailable?: {
        readonly gapId: string
        readonly kind: "upstream_app_server_gap"
      }
      readonly issueRef: string
    }

export type KhalaCodeDesktopSlashCommand = {
  readonly aliases: readonly string[]
  readonly availableDuringTask: boolean
  readonly availableInSideConversation: boolean
  readonly command: string
  readonly debug: boolean
  readonly description: string
  readonly dispatch: KhalaCodeDesktopSlashCommandDispatch
  readonly enumName: string
  readonly group: KhalaCodeDesktopSlashCommandGroup
  readonly supportsInlineArgs: boolean
  readonly visibility: KhalaCodeDesktopSlashCommandVisibility
}

export type KhalaCodeDesktopSlashCommandAvailability = {
  readonly available: boolean
  readonly reason?: string
}

export type KhalaCodeDesktopSlashCommandListOptions = {
  readonly debug?: boolean
  readonly platform?: string
}

export type KhalaCodeDesktopSlashCommandContext = {
  readonly activeTurn?: boolean
  readonly sideConversation?: boolean
}

export type KhalaCodeDesktopParsedSlashCommand = {
  readonly args: string
  readonly command: KhalaCodeDesktopSlashCommand
  readonly rawCommand: string
}

export type KhalaCodeDesktopSlashCommandWithAvailability =
  KhalaCodeDesktopSlashCommand & {
    readonly availability: KhalaCodeDesktopSlashCommandAvailability
  }

const GAP_ISSUE_REF = "https://github.com/OpenAgentsInc/openagents/issues/7785"

const always = { kind: "always" } as const
const debugOnly = { kind: "debug" } as const
const notAndroid = { kind: "not_android" } as const
const macOrWindows = { kind: "platform", platforms: ["darwin", "win32"] } as const
const windowsOnly = { kind: "platform", platforms: ["win32"] } as const

const appServer = (
  method: string,
  options: Omit<Extract<KhalaCodeDesktopSlashCommandDispatch, { kind: "app_server" }>, "kind" | "method"> = {},
): KhalaCodeDesktopSlashCommandDispatch => ({
  kind: "app_server",
  method,
  ...options,
})

const client = (action: string): KhalaCodeDesktopSlashCommandDispatch => ({
  kind: "client",
  action,
})

const SIDE_AGENT_PLAN_CONTROLS_GAP = "codex.app_server.gap.side_agent_plan_controls"

const gap = (
  dependency: string,
  options: {
    readonly unavailable?: Extract<KhalaCodeDesktopSlashCommandDispatch, { kind: "gap" }>["unavailable"]
  } = {},
): KhalaCodeDesktopSlashCommandDispatch => ({
  kind: "gap",
  dependency,
  ...(options.unavailable === undefined ? {} : { unavailable: options.unavailable }),
  issueRef: GAP_ISSUE_REF,
})

const codexConfigPreference = (preference: string): KhalaCodeDesktopSlashCommandDispatch =>
  appServer("config/read", {
    appServerDependency: `${preference} is backed by Codex config/read and config/value/write; Khala only renders the desktop control.`,
  })

const sideAgentPlanControlsGap = (dependency: string): KhalaCodeDesktopSlashCommandDispatch =>
  gap(dependency, {
    unavailable: {
      gapId: SIDE_AGENT_PLAN_CONTROLS_GAP,
      kind: "upstream_app_server_gap",
    },
  })

const command = (
  input: Omit<KhalaCodeDesktopSlashCommand, "aliases" | "debug" | "visibility"> & {
    readonly aliases?: readonly string[]
    readonly debug?: boolean
    readonly visibility?: KhalaCodeDesktopSlashCommandVisibility
  },
): KhalaCodeDesktopSlashCommand => ({
  aliases: input.aliases ?? [],
  debug: input.debug ?? false,
  visibility: input.visibility ?? always,
  ...input,
})

export const KHALA_CODE_DESKTOP_SLASH_COMMANDS = [
  command({
    enumName: "Model",
    command: "model",
    description: "Switch model and reasoning effort",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "settings",
    dispatch: appServer("model/list", {
      appServerDependency: "Khala must add the model picker UI on top of model/list.",
    }),
  }),
  command({
    enumName: "Ide",
    command: "ide",
    description: "Manage IDE integration",
    supportsInlineArgs: true,
    availableInSideConversation: true,
    availableDuringTask: true,
    group: "settings",
    dispatch: appServer("config/read", {
      appServerDependency:
        "Khala projects IDE integration status from Codex app-server config; richer IDE mutation remains upstream-owned.",
    }),
  }),
  command({
    enumName: "Permissions",
    command: "permissions",
    description: "Choose what Codex can do without approval",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "settings",
    dispatch: appServer("permissionProfile/list", {
      appServerDependency: "Khala must add the permission profile picker UI on top of permissionProfile/list.",
    }),
  }),
  command({
    enumName: "Keymap",
    command: "keymap",
    description: "Open keymap editor",
    supportsInlineArgs: true,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "settings",
    dispatch: codexConfigPreference("Codex TUI keymap"),
  }),
  command({
    enumName: "Vim",
    command: "vim",
    description: "Toggle vim keybindings",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "settings",
    dispatch: codexConfigPreference("Codex Vim mode"),
  }),
  command({
    enumName: "ElevateSandbox",
    command: "setup-default-sandbox",
    description: "Configure the default Windows sandbox",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "settings",
    dispatch: gap("Codex Windows sandbox setup needs the Windows-only app-server sandbox setup flow."),
  }),
  command({
    enumName: "SandboxReadRoot",
    command: "sandbox-add-read-dir",
    description: "Add a folder to the Windows sandbox readable roots",
    supportsInlineArgs: true,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "settings",
    visibility: windowsOnly,
    dispatch: gap("Codex Windows sandbox read-root mutation needs the Windows-only app-server sandbox setup flow."),
  }),
  command({
    enumName: "Experimental",
    command: "experimental",
    description: "Open experimental features",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "settings",
    dispatch: appServer("experimentalFeature/list", {
      appServerDependency: "Khala must add the experimental feature picker UI on top of experimentalFeature/list.",
    }),
  }),
  command({
    enumName: "AutoReview",
    command: "approve",
    description: "Review and apply auto-approval suggestions",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "turn_task",
    dispatch: sideAgentPlanControlsGap("Codex auto-approval review currently arrives as item/server-request state and needs a desktop review adapter."),
  }),
  command({
    enumName: "Memories",
    command: "memories",
    description: "Manage memory entries",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "workspace",
    dispatch: gap("Codex memory management needs a desktop UI over app-server memory and thread memory-mode APIs."),
  }),
  command({
    enumName: "Skills",
    command: "skills",
    description: "Open skills browser",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "workspace",
    dispatch: gap("Codex skills browsing needs a desktop UI over app-server skill and plugin read APIs."),
  }),
  command({
    enumName: "Import",
    command: "import",
    description: "Import AGENTS.md into Codex memory",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "workspace",
    dispatch: gap("Codex memory import is currently TUI-local and needs an app-server import method or desktop adapter."),
  }),
  command({
    enumName: "Hooks",
    command: "hooks",
    description: "Manage hooks",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "workspace",
    dispatch: gap("Codex hook management needs a desktop UI over plugin/read hook state."),
  }),
  command({
    enumName: "Review",
    command: "review",
    description: "Review current changes",
    supportsInlineArgs: true,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "turn_task",
    dispatch: appServer("review/start", { requiresThread: true }),
  }),
  command({
    enumName: "Rename",
    command: "rename",
    description: "Rename current thread",
    supportsInlineArgs: true,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "session",
    dispatch: appServer("thread/name/set", { requiresArgs: true, requiresThread: true }),
  }),
  command({
    enumName: "New",
    command: "new",
    description: "Start a new thread",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "session",
    dispatch: appServer("thread/start"),
  }),
  command({
    enumName: "Archive",
    command: "archive",
    description: "Archive current thread",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "session",
    dispatch: appServer("thread/archive", { requiresThread: true }),
  }),
  command({
    enumName: "Delete",
    command: "delete",
    description: "Delete current thread",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "session",
    dispatch: appServer("thread/delete", { requiresThread: true }),
  }),
  command({
    enumName: "Resume",
    command: "resume",
    description: "Resume a previous thread",
    supportsInlineArgs: true,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "session",
    dispatch: appServer("thread/resume", {
      appServerDependency: "The no-arg thread picker still needs a desktop picker; inline thread id resumes are supported.",
    }),
  }),
  command({
    enumName: "Fork",
    command: "fork",
    description: "Fork current thread",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "session",
    dispatch: appServer("thread/fork", { requiresThread: true }),
  }),
  command({
    enumName: "App",
    command: "app",
    description: "Manage app integrations",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "ecosystem",
    visibility: macOrWindows,
    dispatch: appServer("app/list", {
      appServerDependency: "Khala must add the single-app management UI on top of app/list.",
    }),
  }),
  command({
    enumName: "Init",
    command: "init",
    description: "Create an AGENTS.md file with instructions for Codex",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "workspace",
    dispatch: gap("Codex AGENTS.md initialization is currently TUI-local and needs a desktop file-authoring adapter."),
  }),
  command({
    enumName: "Compact",
    command: "compact",
    description: "Compact conversation to free context space",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "session",
    dispatch: appServer("thread/compact/start", { requiresThread: true }),
  }),
  command({
    enumName: "Architect",
    command: "architect",
    description: "Create a read-only architect plan card",
    supportsInlineArgs: true,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "turn_task",
    dispatch: client("create_architect_plan"),
  }),
  command({
    enumName: "Plan",
    command: "plan",
    description: "Create or update a plan",
    supportsInlineArgs: true,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "turn_task",
    dispatch: sideAgentPlanControlsGap("Codex plan editing is currently represented by turn plan updates and needs a desktop plan adapter."),
  }),
  command({
    enumName: "Goal",
    command: "goal",
    description: "Create, inspect, or update the current goal",
    supportsInlineArgs: true,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "turn_task",
    dispatch: appServer("thread/goal/set", { requiresThread: true }),
  }),
  command({
    enumName: "Agent",
    command: "agent",
    description: "Delegate work to another agent",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "turn_task",
    dispatch: sideAgentPlanControlsGap("Codex agent delegation needs the desktop multi-agent RPC bridge and UI from the later swarm issues."),
  }),
  command({
    enumName: "Side",
    command: "side",
    description: "Start a side conversation",
    supportsInlineArgs: true,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "turn_task",
    dispatch: sideAgentPlanControlsGap("Codex side conversations need a desktop side-thread surface and app-server mapping."),
  }),
  command({
    enumName: "Btw",
    command: "btw",
    description: "Add a side note to the current turn",
    supportsInlineArgs: true,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "turn_task",
    dispatch: appServer("turn/steer", { requiresArgs: true }),
  }),
  command({
    enumName: "Copy",
    command: "copy",
    description: "Copy last assistant message",
    supportsInlineArgs: false,
    availableInSideConversation: true,
    availableDuringTask: true,
    group: "session",
    visibility: notAndroid,
    dispatch: client("copy_last_assistant_message"),
  }),
  command({
    enumName: "Raw",
    command: "raw",
    description: "Show raw message text",
    supportsInlineArgs: true,
    availableInSideConversation: true,
    availableDuringTask: true,
    group: "session",
    dispatch: client("show_raw_message_text"),
  }),
  command({
    enumName: "Diff",
    command: "diff",
    description: "Show git diff",
    supportsInlineArgs: false,
    availableInSideConversation: true,
    availableDuringTask: true,
    group: "turn_task",
    dispatch: appServer("gitDiffToRemote"),
  }),
  command({
    enumName: "Mention",
    command: "mention",
    description: "Insert a file or symbol mention",
    supportsInlineArgs: false,
    availableInSideConversation: true,
    availableDuringTask: true,
    group: "turn_task",
    dispatch: appServer("fuzzyFileSearch"),
  }),
  command({
    enumName: "Status",
    command: "status",
    description: "Show current status",
    supportsInlineArgs: false,
    availableInSideConversation: true,
    availableDuringTask: true,
    group: "diagnostics",
    dispatch: client("show_desktop_status"),
  }),
  command({
    enumName: "Usage",
    command: "usage",
    description: "Show token usage",
    supportsInlineArgs: true,
    availableInSideConversation: true,
    availableDuringTask: true,
    group: "diagnostics",
    dispatch: appServer("account/usage/read"),
  }),
  command({
    enumName: "DebugConfig",
    command: "debug-config",
    description: "Show debug configuration",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "diagnostics",
    dispatch: client("show_debug_config"),
  }),
  command({
    enumName: "Title",
    command: "title",
    description: "Set or show the title",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "session",
    dispatch: client("show_or_set_window_title"),
  }),
  command({
    enumName: "Statusline",
    command: "statusline",
    description: "Configure statusline",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "settings",
    dispatch: codexConfigPreference("Codex statusline"),
  }),
  command({
    enumName: "Theme",
    command: "theme",
    description: "Choose a theme",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "settings",
    dispatch: codexConfigPreference("Codex theme"),
  }),
  command({
    enumName: "Pets",
    command: "pets",
    aliases: ["pet"],
    description: "Manage pets",
    supportsInlineArgs: true,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "settings",
    dispatch: codexConfigPreference("Codex pet"),
  }),
  command({
    enumName: "Mcp",
    command: "mcp",
    description: "Inspect MCP servers",
    supportsInlineArgs: true,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "ecosystem",
    dispatch: appServer("mcpServerStatus/list"),
  }),
  command({
    enumName: "Apps",
    command: "apps",
    description: "Browse available apps",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "ecosystem",
    dispatch: appServer("app/list"),
  }),
  command({
    enumName: "Plugins",
    command: "plugins",
    description: "Browse plugins",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "ecosystem",
    dispatch: appServer("plugin/list"),
  }),
  command({
    enumName: "Logout",
    command: "logout",
    description: "Sign out of Codex",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "exit",
    dispatch: appServer("account/logout"),
  }),
  command({
    enumName: "Quit",
    command: "quit",
    description: "Quit Codex",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "exit",
    dispatch: client("quit_application"),
  }),
  command({
    enumName: "Exit",
    command: "exit",
    description: "Exit Codex",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "exit",
    dispatch: client("quit_application"),
  }),
  command({
    enumName: "Feedback",
    command: "feedback",
    description: "Send feedback",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "diagnostics",
    dispatch: client("open_feedback"),
  }),
  command({
    enumName: "Rollout",
    command: "rollout",
    description: "Show rollout diagnostics",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "diagnostics",
    debug: true,
    visibility: debugOnly,
    dispatch: client("show_rollout_diagnostics"),
  }),
  command({
    enumName: "Ps",
    command: "ps",
    description: "List background terminal commands",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "background",
    dispatch: appServer("thread/backgroundTerminals/list", {
      experimental: true,
      requiresThread: true,
    }),
  }),
  command({
    enumName: "Stop",
    command: "stop",
    aliases: ["clean"],
    description: "Stop all background terminal commands",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "background",
    dispatch: appServer("thread/backgroundTerminals/clean", {
      experimental: true,
      requiresThread: true,
    }),
  }),
  command({
    enumName: "Clear",
    command: "clear",
    description: "Clear the visible transcript",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "session",
    dispatch: client("clear_visible_transcript"),
  }),
  command({
    enumName: "Personality",
    command: "personality",
    description: "Configure assistant personality",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "settings",
    dispatch: codexConfigPreference("Codex personality"),
  }),
  command({
    enumName: "TestApproval",
    command: "test-approval",
    description: "Trigger test approval flow",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "diagnostics",
    debug: true,
    visibility: debugOnly,
    dispatch: client("trigger_test_approval"),
  }),
  command({
    enumName: "MultiAgents",
    command: "subagents",
    description: "Manage subagents",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: true,
    group: "turn_task",
    dispatch: sideAgentPlanControlsGap("Codex subagent management needs the desktop multi-agent RPC bridge from the later swarm issues."),
  }),
  command({
    enumName: "MemoryDrop",
    command: "debug-m-drop",
    description: "Debug memory drop",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "diagnostics",
    debug: true,
    dispatch: gap("Debug memory drop is a Codex memory test hook and should not be exposed beyond debug diagnostics."),
  }),
  command({
    enumName: "MemoryUpdate",
    command: "debug-m-update",
    description: "Debug memory update",
    supportsInlineArgs: false,
    availableInSideConversation: false,
    availableDuringTask: false,
    group: "diagnostics",
    debug: true,
    dispatch: gap("Debug memory update is a Codex memory test hook and should not be exposed beyond debug diagnostics."),
  }),
] as const satisfies readonly KhalaCodeDesktopSlashCommand[]

const normalizedPlatform = (platform?: string): string =>
  (platform ?? (typeof process === "undefined" ? "unknown" : process.platform)).toLowerCase()

const isVisible = (
  command: KhalaCodeDesktopSlashCommand,
  options: KhalaCodeDesktopSlashCommandListOptions,
): boolean => {
  const platform = normalizedPlatform(options.platform)
  switch (command.visibility.kind) {
    case "always":
      return true
    case "debug":
      return options.debug === true
    case "not_android":
      return platform !== "android"
    case "platform":
      return command.visibility.platforms.includes(platform)
  }
}

export const khalaCodeDesktopSlashCommands = (
  options: KhalaCodeDesktopSlashCommandListOptions = {},
): readonly KhalaCodeDesktopSlashCommand[] =>
  KHALA_CODE_DESKTOP_SLASH_COMMANDS.filter(command => isVisible(command, options))

export const evaluateKhalaCodeDesktopSlashCommandAvailability = (
  command: KhalaCodeDesktopSlashCommand,
  context: KhalaCodeDesktopSlashCommandContext = {},
): KhalaCodeDesktopSlashCommandAvailability => {
  if (context.sideConversation === true && !command.availableInSideConversation) {
    return {
      available: false,
      reason: `/${command.command} is only available from the main thread.`,
    }
  }
  if (context.activeTurn === true && !command.availableDuringTask) {
    return {
      available: false,
      reason: `/${command.command} is not available while Codex is working.`,
    }
  }
  return { available: true }
}

export const khalaCodeDesktopSlashCommandsWithAvailability = (
  options: KhalaCodeDesktopSlashCommandListOptions & KhalaCodeDesktopSlashCommandContext = {},
): readonly KhalaCodeDesktopSlashCommandWithAvailability[] =>
  khalaCodeDesktopSlashCommands(options).map(command => ({
    ...command,
    availability: evaluateKhalaCodeDesktopSlashCommandAvailability(command, options),
  }))

export const findKhalaCodeDesktopSlashCommand = (
  commandText: string,
  options: KhalaCodeDesktopSlashCommandListOptions = {},
): KhalaCodeDesktopSlashCommand | null => {
  const normalized = commandText.trim().replace(/^\/+/, "").toLowerCase()
  if (normalized.length === 0) return null
  return khalaCodeDesktopSlashCommands(options).find(command =>
    command.command === normalized || command.aliases.includes(normalized)
  ) ?? null
}

export const parseKhalaCodeDesktopSlashCommand = (
  text: string,
  options: KhalaCodeDesktopSlashCommandListOptions = {},
): KhalaCodeDesktopParsedSlashCommand | null => {
  const trimmed = text.trim()
  if (!trimmed.startsWith("/")) return null
  const [raw = "", ...rest] = trimmed.slice(1).split(/\s+/)
  const command = findKhalaCodeDesktopSlashCommand(raw, options)
  if (command === null) return null
  return {
    args: rest.join(" ").trim(),
    command,
    rawCommand: raw,
  }
}

export const khalaCodeDesktopSlashCommandDispatchCoverage = (): readonly {
  readonly command: string
  readonly dispatchKind: KhalaCodeDesktopSlashCommandDispatch["kind"]
  readonly dependency?: string
  readonly experimental?: boolean
  readonly method?: string
}[] =>
  KHALA_CODE_DESKTOP_SLASH_COMMANDS.map(command => ({
    command: command.command,
    dispatchKind: command.dispatch.kind,
    ...(command.dispatch.kind === "app_server"
      ? {
        experimental: command.dispatch.experimental === true,
        method: command.dispatch.method,
      }
      : {}),
    ...(command.dispatch.kind === "gap" ? { dependency: command.dispatch.dependency } : {}),
  }))
