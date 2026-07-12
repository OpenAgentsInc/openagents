/**
 * Settings screen (#8574, #8640 unblock): the most minimal UI needed to
 * reconnect Codex fleet accounts through the app instead of the CLI.
 *
 * Pure Effect Native data over the shared catalog (v29): connected-accounts
 * list with readiness chips (revoked in a warning tone) and one primary
 * "Connect Codex account" button that drives the pylon isolated device-auth
 * flow through the hardened bridge. The renderer only ever sees refs,
 * readiness states, the verification URL + user code, and typed status —
 * schema-decoded here, never trusted raw.
 */
import {
  Badge,
  Button,
  Card,
  ComponentValueBinding,
  Divider,
  IntentRef,
  RadioGroup,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  TextField,
  Toggle,
  Tooltip,
  defineIntent,
  type View,
} from "@effect-native/core"
import { Effect, Exit, Schema, SubscriptionRef } from "@effect-native/core/effect"

import {
  decodeMcpConfigListResult,
  decodeMcpConfigMutationResult,
  mcpReservedServerName,
  mcpServerListCap,
  mcpServerNamePattern,
  type McpConfigServerView,
} from "../mcp-config-contract.ts"
import type { FableLocalMcpServerConfig } from "../fable-local-contract.ts"

// ---------------------------------------------------------------------------
// Renderer-side bridge decoding (Effect Schema; mirrors the main-process
// contract in ../codex-connect-contract.ts — the renderer import boundary
// only allows sibling modules, so the schemas live on both sides and the
// bridge-contract test asserts they agree).
// ---------------------------------------------------------------------------

export type CodexAccountItem = Readonly<{ ref: string; readiness: string }>

export type CodexAccountsView =
  | Readonly<{ state: "loading" }>
  | Readonly<{ state: "loaded"; accounts: ReadonlyArray<CodexAccountItem> }>
  | Readonly<{ state: "unavailable"; message: string }>

export type ClaudeAccountsView =
  | Readonly<{ state: "loading" }>
  | Readonly<{ state: "loaded"; accounts: ReadonlyArray<CodexAccountItem> }>
  | Readonly<{ state: "unavailable"; message: string }>

export type CodexConnectStatusView =
  | Readonly<{ state: "idle" }>
  | Readonly<{ state: "starting" }>
  | Readonly<{ state: "awaiting_browser"; url: string; code: string }>
  | Readonly<{ state: "connected"; ref: string }>
  | Readonly<{ state: "failed"; reason: string }>

export type SettingsState = Readonly<{
  accounts: CodexAccountsView
  claudeAccounts: ClaudeAccountsView
  connect: CodexConnectStatusView
  /**
   * The account ref a RECONNECT flow is targeting (EP250 UI-owned
   * reconnect), or null when the flow is a new-account connect. Display
   * only — the authoritative target lives in main.
   */
  connectTarget: string | null
  openAgentsSession: DesktopOpenAgentsSessionView
  /** User-configured MCP servers (I2, EP250 wave-2). */
  mcp: McpSettingsState
}>

// ---------------------------------------------------------------------------
// User-configured MCP servers (I2, EP250 wave-2). All view-model state; the
// runtime substrate + FROZEN config schema landed on a prior lane. The Add
// form draft is client-validated against the same schema bounds before it
// ever crosses to main, and main re-validates against the frozen schema.
// ---------------------------------------------------------------------------

export type McpServersView =
  | Readonly<{ state: "loading" }>
  | Readonly<{ state: "loaded"; servers: ReadonlyArray<McpConfigServerView>; dropped: number }>
  | Readonly<{ state: "unavailable"; message: string }>

export type McpAddDraft = Readonly<{
  name: string
  transport: "stdio" | "http"
  command: string
  /** Whitespace/newline-separated argv (stdio). */
  argsText: string
  /** `KEY=value` per line (stdio). */
  envText: string
  url: string
  /** `Key: Value` per line (http). */
  headersText: string
}>

export type McpSettingsState = Readonly<{
  servers: McpServersView
  draft: McpAddDraft
  /** Inline Add-form error (client validation OR main's typed rejection). */
  formError: string | null
  /**
   * Optional runtime-reported unavailability by server name. The runtime emits
   * `mcp_server_unavailable` during a turn; threading that live status into
   * settings is a separate lane, so this defaults to none and the UI shows
   * config state (enabled/disabled) until it is wired.
   */
  status: Readonly<Record<string, string>>
}>

export const emptyMcpAddDraft = (): McpAddDraft => ({
  name: "",
  transport: "stdio",
  command: "",
  argsText: "",
  envText: "",
  url: "",
  headersText: "",
})

export const initialMcpSettingsState = (): McpSettingsState => ({
  servers: { state: "loading" },
  draft: emptyMcpAddDraft(),
  formError: null,
  status: {},
})

export type DesktopOpenAgentsSessionView =
  | "loading"
  | "authenticating"
  | "signed_out"
  | "unverified"
  | "session_ready"
  | "denied"
  | "unavailable"

export const initialSettingsState = (): SettingsState => ({
  accounts: { state: "loading" },
  claudeAccounts: { state: "loading" },
  connect: { state: "idle" },
  connectTarget: null,
  openAgentsSession: "loading",
  mcp: initialMcpSettingsState(),
})

const accountRefPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/
const userCodePattern = /^[A-Z0-9]{4}-[A-Z0-9]{4,6}$/

// Frozen-schema field bounds (fable-local-contract.ts) mirrored client-side so
// the Add form fails fast with an inline reason before anything crosses to
// main; main re-validates against the same frozen schema regardless.
const MCP_NAME_MAX = 64
const MCP_COMMAND_MAX = 512
const MCP_ARG_MAX = 1_024
const MCP_ARGS_COUNT_MAX = 64
const MCP_URL_MAX = 2_048
const MCP_ENV_VALUE_MAX = 4_096
const MCP_HEADER_VALUE_MAX = 4_096

/**
 * Bounded, deterministic argv parse (whitespace/newline-separated). This runs
 * only AFTER the "add an MCP server" route is already chosen — a bounded-field
 * parser, not an intent router.
 */
export const parseMcpArgs = (text: string): ReadonlyArray<string> =>
  text
    .split(/\s+/)
    .map((token) => token.trim())
    .filter((token) => token.length > 0)
    .slice(0, MCP_ARGS_COUNT_MAX)

/** `KEY=value` per line → record (first `=` splits; later `=`s stay in value). */
export const parseMcpKeyValueLines = (text: string, separator: "=" | ":"): Record<string, string> => {
  const out: Record<string, string> = {}
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim()
    if (trimmed === "") continue
    const index = trimmed.indexOf(separator)
    if (index <= 0) continue
    const key = trimmed.slice(0, index).trim()
    const value = trimmed.slice(index + 1).trim()
    if (key === "") continue
    out[key] = value
  }
  return out
}

/**
 * Client-side build+validate of one Add-form draft into a frozen config, or a
 * single inline error reason. Mirrors the frozen schema bounds and the
 * reserved/duplicate/transport rules exactly. Newly added servers default
 * `enabled: true` so they take effect on the next turn.
 */
export const buildMcpConfigFromDraft = (
  draft: McpAddDraft,
  existingNames: ReadonlyArray<string>,
): { readonly ok: true; readonly config: FableLocalMcpServerConfig } | {
  readonly ok: false
  readonly error: string
} => {
  const name = draft.name.trim()
  if (name === "") return { ok: false, error: "Name is required." }
  if (name.length > MCP_NAME_MAX) return { ok: false, error: `Name must be at most ${MCP_NAME_MAX} characters.` }
  if (!mcpServerNamePattern.test(name)) {
    return { ok: false, error: "Name may use letters, digits, _ or -, and cannot start or end with a separator." }
  }
  if (name === mcpReservedServerName) {
    return { ok: false, error: `"${mcpReservedServerName}" is reserved for the internal delegate server.` }
  }
  if (existingNames.includes(name)) return { ok: false, error: "A server with that name already exists." }

  if (draft.transport === "stdio") {
    const command = draft.command.trim()
    if (command === "") return { ok: false, error: "A stdio server needs a command." }
    if (command.length > MCP_COMMAND_MAX) return { ok: false, error: `Command must be at most ${MCP_COMMAND_MAX} characters.` }
    const args = parseMcpArgs(draft.argsText)
    if (args.some((arg) => arg.length > MCP_ARG_MAX)) {
      return { ok: false, error: `Each argument must be at most ${MCP_ARG_MAX} characters.` }
    }
    const env = parseMcpKeyValueLines(draft.envText, "=")
    if (Object.values(env).some((value) => value.length > MCP_ENV_VALUE_MAX)) {
      return { ok: false, error: `Each environment value must be at most ${MCP_ENV_VALUE_MAX} characters.` }
    }
    return {
      ok: true,
      config: {
        name,
        transport: "stdio",
        enabled: true,
        command,
        ...(args.length > 0 ? { args: [...args] } : {}),
        ...(Object.keys(env).length > 0 ? { env } : {}),
      },
    }
  }

  const url = draft.url.trim()
  if (url === "") return { ok: false, error: "An http server needs a URL." }
  if (url.length > MCP_URL_MAX) return { ok: false, error: `URL must be at most ${MCP_URL_MAX} characters.` }
  if (!/^https?:\/\//i.test(url)) return { ok: false, error: "URL must start with http:// or https://." }
  const headers = parseMcpKeyValueLines(draft.headersText, ":")
  if (Object.values(headers).some((value) => value.length > MCP_HEADER_VALUE_MAX)) {
    return { ok: false, error: `Each header value must be at most ${MCP_HEADER_VALUE_MAX} characters.` }
  }
  return {
    ok: true,
    config: {
      name,
      transport: "http",
      enabled: true,
      url,
      ...(Object.keys(headers).length > 0 ? { headers } : {}),
    },
  }
}

const RendererAccountsResultSchema = Schema.Union([
  Schema.Struct({
    state: Schema.Literal("ok"),
    accounts: Schema.Array(Schema.Struct({ ref: Schema.String, readiness: Schema.String })),
  }),
  Schema.Struct({ state: Schema.Literal("unavailable"), message: Schema.String }),
])

const RendererProviderAccountsSchema = Schema.Struct({
  generatedAt: Schema.String,
  accounts: Schema.Array(Schema.Struct({
    ref: Schema.String,
    provider: Schema.String,
    readiness: Schema.String,
    email: Schema.optional(Schema.NullOr(Schema.String)),
  })),
})

const RendererConnectStatusSchema = Schema.Union([
  Schema.Struct({ state: Schema.Literal("idle") }),
  Schema.Struct({ state: Schema.Literal("starting") }),
  Schema.Struct({
    state: Schema.Literal("awaiting_browser"),
    url: Schema.String,
    code: Schema.String,
  }),
  Schema.Struct({ state: Schema.Literal("connected"), ref: Schema.String }),
  Schema.Struct({ state: Schema.Literal("failed"), reason: Schema.String }),
])

const RendererOpenAgentsSessionSchema = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("query_result"),
    requestId: Schema.String,
    result: Schema.Struct({
      kind: Schema.Literal("runtime.bootstrap"),
      sessionPhase: Schema.Literals(["signed_out", "unverified", "session_ready", "denied", "unavailable"]),
    }),
  }),
  Schema.Struct({
    kind: Schema.Literal("session_outcome"),
    commandId: Schema.String,
    status: Schema.Literals(["completed", "cancelled", "unavailable"]),
    phase: Schema.Literals(["session_ready", "signed_out", "unavailable"]),
  }),
])

export const decodeAccountsView = (value: unknown): CodexAccountsView => {
  const decoded = Schema.decodeUnknownExit(RendererAccountsResultSchema)(value)
  if (!Exit.isSuccess(decoded)) {
    return { state: "unavailable", message: "Account list is unavailable." }
  }
  if (decoded.value.state === "unavailable") {
    return { state: "unavailable", message: decoded.value.message.slice(0, 200) }
  }
  return {
    state: "loaded",
    accounts: decoded.value.accounts.filter(
      (account) => accountRefPattern.test(account.ref) && account.readiness.length <= 80,
    ),
  }
}

export const decodeClaudeAccountsView = (value: unknown): ClaudeAccountsView => {
  const decoded = Schema.decodeUnknownExit(RendererProviderAccountsSchema)(value)
  if (!Exit.isSuccess(decoded)) {
    return { state: "unavailable", message: "Claude account listing is unavailable on this build." }
  }
  return {
    state: "loaded",
    accounts: decoded.value.accounts
      .filter((account) =>
        account.provider === "claude_agent" &&
        accountRefPattern.test(account.ref) &&
        account.readiness.length <= 80)
      .map((account) => ({ ref: account.ref, readiness: account.readiness })),
  }
}

export const decodeConnectStatusView = (value: unknown): CodexConnectStatusView => {
  const decoded = Schema.decodeUnknownExit(RendererConnectStatusSchema)(value)
  if (!Exit.isSuccess(decoded)) {
    return { state: "failed", reason: "invalid_bridge_payload" }
  }
  const status = decoded.value
  if (status.state === "awaiting_browser") {
    if (!status.url.startsWith("https://") || status.url.length > 200) {
      return { state: "failed", reason: "invalid_verification_url" }
    }
    if (!userCodePattern.test(status.code)) {
      return { state: "failed", reason: "invalid_user_code" }
    }
  }
  if (status.state === "connected" && !accountRefPattern.test(status.ref)) {
    return { state: "failed", reason: "invalid_account_ref" }
  }
  if (status.state === "failed") {
    return { state: "failed", reason: status.reason.slice(0, 120) }
  }
  return status
}

export const decodeOpenAgentsSessionView = (
  value: unknown,
): Exclude<DesktopOpenAgentsSessionView, "loading" | "authenticating"> => {
  const decoded = Schema.decodeUnknownExit(RendererOpenAgentsSessionSchema)(value)
  if (!Exit.isSuccess(decoded)) return "unavailable"
  return decoded.value.kind === "query_result"
    ? decoded.value.result.sessionPhase
    : decoded.value.phase
}

// ---------------------------------------------------------------------------
// Typed bridge surface the settings handlers need (injected from boot.ts;
// defaults are honest "unavailable" stand-ins for headless tests).
// ---------------------------------------------------------------------------

export type CodexSettingsBridge = Readonly<{
  listAccounts: () => Promise<unknown>
  connectStart: () => Promise<unknown>
  /**
   * Per-account re-auth into the SAME isolated ref/home (EP250 UI-owned
   * reconnect). Optional so older hosts degrade to the honest failure.
   */
  reconnectStart?: (ref: string) => Promise<unknown>
  connectStatus: () => Promise<unknown>
  openVerification: () => Promise<unknown>
}>

export const unavailableCodexSettingsBridge: CodexSettingsBridge = {
  listAccounts: async () => ({
    state: "unavailable",
    message: "Local Pylon runtime is unavailable. No accounts were read.",
  }),
  connectStart: async () => ({ state: "failed", reason: "pylon_runtime_unavailable" }),
  reconnectStart: async () => ({ state: "failed", reason: "pylon_runtime_unavailable" }),
  connectStatus: async () => ({ state: "failed", reason: "pylon_runtime_unavailable" }),
  openVerification: async () => false,
}

export type ProviderAccountsSettingsBridge = Readonly<{
  list: () => Promise<unknown>
}>

export const unavailableProviderAccountsSettingsBridge: ProviderAccountsSettingsBridge = {
  list: async () => null,
}

export type OpenAgentsSessionSettingsBridge = Readonly<{
  status: () => Promise<unknown>
  signIn: () => Promise<unknown>
  signOut: () => Promise<unknown>
}>

export const unavailableOpenAgentsSessionSettingsBridge: OpenAgentsSessionSettingsBridge = {
  status: async () => null,
  signIn: async () => null,
  signOut: async () => null,
}

export type McpConfigSettingsBridge = Readonly<{
  list: () => Promise<unknown>
  add: (config: FableLocalMcpServerConfig) => Promise<unknown>
  remove: (name: string) => Promise<unknown>
  toggle: (name: string, enabled: boolean) => Promise<unknown>
}>

export const unavailableMcpConfigSettingsBridge: McpConfigSettingsBridge = {
  list: async () => ({ state: "unavailable", message: "MCP server configuration is unavailable on this build." }),
  add: async () => ({ state: "unavailable", message: "MCP server configuration is unavailable on this build." }),
  remove: async () => ({ state: "unavailable", message: "MCP server configuration is unavailable on this build." }),
  toggle: async () => ({ state: "unavailable", message: "MCP server configuration is unavailable on this build." }),
}

export const mcpServersViewFromListResult = (value: unknown): McpServersView => {
  const result = decodeMcpConfigListResult(value)
  return result.state === "unavailable"
    ? { state: "unavailable", message: result.message }
    : { state: "loaded", servers: result.servers, dropped: result.dropped }
}

// ---------------------------------------------------------------------------
// Pure transitions
// ---------------------------------------------------------------------------

export const withSettingsAccounts = (
  settings: SettingsState,
  accounts: CodexAccountsView,
): SettingsState => ({ ...settings, accounts })

export const withSettingsClaudeAccounts = (
  settings: SettingsState,
  claudeAccounts: ClaudeAccountsView,
): SettingsState => ({ ...settings, claudeAccounts })

export const withSettingsConnectStatus = (
  settings: SettingsState,
  connect: CodexConnectStatusView,
): SettingsState => ({ ...settings, connect })

/** Whether the poll loop should keep asking main for connect status. */
export const connectStatusIsLive = (connect: CodexConnectStatusView): boolean =>
  connect.state === "starting" || connect.state === "awaiting_browser"

// ---------------------------------------------------------------------------
// Intents
// ---------------------------------------------------------------------------

export const DesktopSettingsToggled = defineIntent("DesktopSettingsToggled", Schema.Null)
export const DesktopCodexConnectRequested = defineIntent(
  "DesktopCodexConnectRequested",
  Schema.Null,
)
/**
 * Per-account reconnect (EP250 owner mandate: "the UI controls need to be
 * working" — no CLI). Payload is the target account ref; the handler and
 * main both re-validate it against the listed accounts.
 */
export const DesktopCodexReconnectRequested = defineIntent(
  "DesktopCodexReconnectRequested",
  Schema.String,
)
export const DesktopCodexVerificationOpened = defineIntent(
  "DesktopCodexVerificationOpened",
  Schema.Null,
)
export const DesktopOpenAgentsSignInRequested = defineIntent(
  "DesktopOpenAgentsSignInRequested",
  Schema.Null,
)
export const DesktopOpenAgentsSignOutRequested = defineIntent(
  "DesktopOpenAgentsSignOutRequested",
  Schema.Null,
)

// User-configured MCP servers (I2, EP250 wave-2). Field edits carry the raw
// component string (ComponentValueBinding); the add/remove/toggle actions
// carry a name or nothing and re-read the authoritative value from state/main.
export const DesktopMcpNameChanged = defineIntent("DesktopMcpNameChanged", Schema.String)
export const DesktopMcpTransportChanged = defineIntent("DesktopMcpTransportChanged", Schema.String)
export const DesktopMcpCommandChanged = defineIntent("DesktopMcpCommandChanged", Schema.String)
export const DesktopMcpArgsChanged = defineIntent("DesktopMcpArgsChanged", Schema.String)
export const DesktopMcpEnvChanged = defineIntent("DesktopMcpEnvChanged", Schema.String)
export const DesktopMcpUrlChanged = defineIntent("DesktopMcpUrlChanged", Schema.String)
export const DesktopMcpHeadersChanged = defineIntent("DesktopMcpHeadersChanged", Schema.String)
export const DesktopMcpAddRequested = defineIntent("DesktopMcpAddRequested", Schema.Null)
export const DesktopMcpRemoveRequested = defineIntent("DesktopMcpRemoveRequested", Schema.String)
export const DesktopMcpToggleRequested = defineIntent("DesktopMcpToggleRequested", Schema.String)

export const settingsIntents = [
  DesktopSettingsToggled,
  DesktopCodexConnectRequested,
  DesktopCodexReconnectRequested,
  DesktopCodexVerificationOpened,
  DesktopOpenAgentsSignInRequested,
  DesktopOpenAgentsSignOutRequested,
  DesktopMcpNameChanged,
  DesktopMcpTransportChanged,
  DesktopMcpCommandChanged,
  DesktopMcpArgsChanged,
  DesktopMcpEnvChanged,
  DesktopMcpUrlChanged,
  DesktopMcpHeadersChanged,
  DesktopMcpAddRequested,
  DesktopMcpRemoveRequested,
  DesktopMcpToggleRequested,
] as const

/**
 * Settings intent handlers over the shared shell state. Generic in the shell
 * state shape so shell.ts stays the single owner of its state type; settings
 * only needs the `workspace` + `settings` slices ("settings" is one of the
 * shell's workspace names).
 */
export type SettingsCapableState = Readonly<{
  workspace: string
  settings: SettingsState
}>

export const makeSettingsHandlers = <S extends SettingsCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  bridge: CodexSettingsBridge = unavailableCodexSettingsBridge,
  openAgentsBridge: OpenAgentsSessionSettingsBridge = unavailableOpenAgentsSessionSettingsBridge,
  sleep: (ms: number) => Promise<void> = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  maxPolls = 1_300, // ~15 minutes at 700ms — matches main's device-auth timeout
  providerAccountsBridge: ProviderAccountsSettingsBridge = unavailableProviderAccountsSettingsBridge,
  mcpBridge: McpConfigSettingsBridge = unavailableMcpConfigSettingsBridge,
) => {
  const update = (transform: (current: S) => S) => SubscriptionRef.update(state, transform)

  /** Re-read the MCP server list through the bridge into state. */
  const refreshMcpServers = Effect.gen(function* () {
    const servers = mcpServersViewFromListResult(
      yield* Effect.promise(() => mcpBridge.list().catch(() => null)),
    )
    yield* update((next) => ({
      ...next,
      settings: { ...next.settings, mcp: { ...next.settings.mcp, servers } },
    }))
  })

  /** Immutably patch the MCP add-form draft, bounding each stored field. */
  const updateMcpDraft = (patch: Partial<McpAddDraft>) =>
    update((next) => ({
      ...next,
      settings: {
        ...next.settings,
        mcp: { ...next.settings.mcp, draft: { ...next.settings.mcp.draft, ...patch }, formError: null },
      },
    }))

  /**
   * Re-read the accounts projection through the bridge. Runs after EVERY
   * terminal connect/reconnect status — success AND failure (EP250 receipt:
   * a flow that exited non-zero had still written valid credentials and
   * registered a new ref; the list re-read must surface it either way), so
   * readiness flips without an app restart.
   */
  const refreshAccounts = Effect.gen(function* () {
    const accounts = decodeAccountsView(
      yield* Effect.promise(() => bridge.listAccounts().catch(() => null)),
    )
    yield* update((next) => ({
      ...next,
      settings: withSettingsAccounts(next.settings, accounts),
    }))
  })

  const pollConnectStatus = Effect.gen(function* () {
    for (let index = 0; index < maxPolls; index += 1) {
      const current = yield* SubscriptionRef.get(state)
      if (current.workspace !== "settings" || !connectStatusIsLive(current.settings.connect)) {
        return
      }
      yield* Effect.promise(() => sleep(700))
      const status = decodeConnectStatusView(
        yield* Effect.promise(() => bridge.connectStatus().catch(() => null)),
      )
      yield* update((next) => ({
        ...next,
        settings: withSettingsConnectStatus(next.settings, status),
      }))
      if (!connectStatusIsLive(status)) {
        yield* refreshAccounts
        return
      }
    }
  })

  /** Shared connect/reconnect flow: start, then poll to terminal + re-list. */
  const runConnectFlow = (
    target: string | null,
    startCall: () => Promise<unknown>,
  ) =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      if (connectStatusIsLive(current.settings.connect)) return
      yield* update((next) => ({
        ...next,
        settings: {
          ...withSettingsConnectStatus(next.settings, { state: "starting" }),
          connectTarget: target,
        },
      }))
      const started = decodeConnectStatusView(
        yield* Effect.promise(() => startCall().catch(() => null)),
      )
      yield* update((next) => ({
        ...next,
        settings: withSettingsConnectStatus(next.settings, started),
      }))
      if (connectStatusIsLive(started)) {
        yield* pollConnectStatus
      } else {
        yield* refreshAccounts
      }
    })

  return {
    DesktopSettingsToggled: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        if (current.workspace === "settings") {
          // "settings" and "chat" are shell workspace names; the generic
          // constraint only sees `string`, so narrow back to S explicitly.
          yield* update((next) => ({ ...next, workspace: "chat" } as S))
          return
        }
        yield* update((next) => ({
          ...next,
          workspace: "settings",
          settings: {
            ...withSettingsClaudeAccounts(
              withSettingsAccounts(next.settings, { state: "loading" }),
              { state: "loading" },
            ),
            mcp: { ...next.settings.mcp, servers: { state: "loading" } },
          },
        } as S))
        const accounts = decodeAccountsView(
          yield* Effect.promise(() => bridge.listAccounts().catch(() => null)),
        )
        const claudeAccounts = decodeClaudeAccountsView(
          yield* Effect.promise(() => providerAccountsBridge.list().catch(() => null)),
        )
        const openAgentsSession = decodeOpenAgentsSessionView(
          yield* Effect.promise(() => openAgentsBridge.status().catch(() => null)),
        )
        yield* update((next) => ({
          ...next,
          settings: {
            ...withSettingsClaudeAccounts(
              withSettingsAccounts(next.settings, accounts),
              claudeAccounts,
            ),
            openAgentsSession,
          },
        }))
        yield* refreshMcpServers
      }),
    DesktopCodexConnectRequested: () => runConnectFlow(null, () => bridge.connectStart()),
    DesktopCodexReconnectRequested: (ref: string) =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        // The renderer may only target a ref it is actually displaying; main
        // re-validates against its own registry listing regardless.
        const accounts = current.settings.accounts
        if (accounts.state !== "loaded" || !accounts.accounts.some((account) => account.ref === ref)) {
          return
        }
        yield* runConnectFlow(ref, () =>
          bridge.reconnectStart === undefined
            ? Promise.resolve({ state: "failed", reason: "pylon_runtime_unavailable" })
            : bridge.reconnectStart(ref))
      }),
    DesktopCodexVerificationOpened: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        if (current.settings.connect.state !== "awaiting_browser") return
        // No URL crosses the bridge: main opens only the URL it parsed itself.
        yield* Effect.promise(() => bridge.openVerification().catch(() => false))
      }),
    DesktopOpenAgentsSignInRequested: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        if (
          current.settings.openAgentsSession === "authenticating" ||
          current.settings.openAgentsSession === "unverified" ||
          current.settings.openAgentsSession === "session_ready"
        ) return
        yield* update(next => ({
          ...next,
          settings: { ...next.settings, openAgentsSession: "authenticating" },
        }))
        const phase = decodeOpenAgentsSessionView(
          yield* Effect.promise(() => openAgentsBridge.signIn().catch(() => null)),
        )
        yield* update(next => ({
          ...next,
          settings: { ...next.settings, openAgentsSession: phase },
        }))
      }),
    DesktopOpenAgentsSignOutRequested: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        if (current.settings.openAgentsSession !== "session_ready") return
        yield* update(next => ({
          ...next,
          settings: { ...next.settings, openAgentsSession: "authenticating" },
        }))
        const phase = decodeOpenAgentsSessionView(
          yield* Effect.promise(() => openAgentsBridge.signOut().catch(() => null)),
        )
        yield* update(next => ({
          ...next,
          settings: { ...next.settings, openAgentsSession: phase },
        }))
      }),
    // -----------------------------------------------------------------------
    // User-configured MCP servers (I2, EP250 wave-2).
    // -----------------------------------------------------------------------
    DesktopMcpNameChanged: (value: string) => updateMcpDraft({ name: value.slice(0, MCP_NAME_MAX) }),
    DesktopMcpTransportChanged: (value: string) =>
      value === "stdio" || value === "http" ? updateMcpDraft({ transport: value }) : Effect.void,
    DesktopMcpCommandChanged: (value: string) => updateMcpDraft({ command: value.slice(0, MCP_COMMAND_MAX) }),
    DesktopMcpArgsChanged: (value: string) => updateMcpDraft({ argsText: value.slice(0, 8_192) }),
    DesktopMcpEnvChanged: (value: string) => updateMcpDraft({ envText: value.slice(0, 16_384) }),
    DesktopMcpUrlChanged: (value: string) => updateMcpDraft({ url: value.slice(0, MCP_URL_MAX) }),
    DesktopMcpHeadersChanged: (value: string) => updateMcpDraft({ headersText: value.slice(0, 16_384) }),
    DesktopMcpAddRequested: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        const mcp = current.settings.mcp
        const existingNames = mcp.servers.state === "loaded"
          ? mcp.servers.servers.map((server) => server.name)
          : []
        const built = buildMcpConfigFromDraft(mcp.draft, existingNames)
        if (!built.ok) {
          yield* update((next) => ({
            ...next,
            settings: { ...next.settings, mcp: { ...next.settings.mcp, formError: built.error } },
          }))
          return
        }
        const result = decodeMcpConfigMutationResult(
          yield* Effect.promise(() => mcpBridge.add(built.config).catch(() => null)),
        )
        if (result.state === "ok") {
          yield* update((next) => ({
            ...next,
            settings: {
              ...next.settings,
              mcp: {
                ...next.settings.mcp,
                servers: { state: "loaded", servers: result.servers, dropped: result.dropped },
                draft: emptyMcpAddDraft(),
                formError: null,
              },
            },
          }))
          return
        }
        const message = result.state === "rejected" ? result.reason : result.message
        yield* update((next) => ({
          ...next,
          settings: { ...next.settings, mcp: { ...next.settings.mcp, formError: message } },
        }))
      }),
    DesktopMcpRemoveRequested: (name: string) =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        const mcp = current.settings.mcp
        // The renderer may only remove a name it is actually displaying; main
        // re-validates against its own stored list regardless.
        if (mcp.servers.state !== "loaded" || !mcp.servers.servers.some((server) => server.name === name)) {
          return
        }
        const result = decodeMcpConfigMutationResult(
          yield* Effect.promise(() => mcpBridge.remove(name).catch(() => null)),
        )
        if (result.state === "ok") {
          yield* update((next) => ({
            ...next,
            settings: {
              ...next.settings,
              mcp: {
                ...next.settings.mcp,
                servers: { state: "loaded", servers: result.servers, dropped: result.dropped },
              },
            },
          }))
        }
      }),
    DesktopMcpToggleRequested: (name: string) =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        const mcp = current.settings.mcp
        if (mcp.servers.state !== "loaded") return
        const server = mcp.servers.servers.find((entry) => entry.name === name)
        if (server === undefined) return
        const result = decodeMcpConfigMutationResult(
          yield* Effect.promise(() => mcpBridge.toggle(name, !server.enabled).catch(() => null)),
        )
        if (result.state === "ok") {
          yield* update((next) => ({
            ...next,
            settings: {
              ...next.settings,
              mcp: {
                ...next.settings.mcp,
                servers: { state: "loaded", servers: result.servers, dropped: result.dropped },
              },
            },
          }))
        }
      }),
  }
}

// ---------------------------------------------------------------------------
// View — pure `state -> View` over the shared catalog.
// ---------------------------------------------------------------------------

const readinessTone = (readiness: string): "success" | "warn" | "neutral" =>
  readiness === "ready" ? "success" : readiness.startsWith("credentials_") ? "warn" : "neutral"

const accountRow = (keyPrefix: string) => (account: CodexAccountItem): View =>
  Stack(
    {
      key: `${keyPrefix}-${account.ref}`,
      direction: "row",
      gap: "2",
      align: "center",
      style: { width: "full" },
    },
    [
      Text({
        key: `${keyPrefix}-${account.ref}-ref`,
        content: account.ref,
        variant: "body",
        color: "textPrimary",
      }),
      Spacer({ key: `${keyPrefix}-${account.ref}-fill`, flex: true }),
      Badge({
        key: `${keyPrefix}-${account.ref}-readiness`,
        label: account.readiness,
        tone: readinessTone(account.readiness),
        a11y: { label: `Account ${account.ref} readiness: ${account.readiness}` },
      }),
    ],
  )

/**
 * A Codex account row: ref + readiness chip, plus a per-account Reconnect
 * button whenever the account's credential evidence is anything other than
 * a clean "ready" (EP250 owner mandate: the UI owns reconnect — the button
 * drives the receipted per-ref re-auth `--account <ref> --force-device-login`
 * into the SAME isolated home; no CLI instruction ever renders).
 */
const reconnectButton = (ref: string, connectLive: boolean): View =>
  Button({
    key: `settings-account-${ref}-reconnect`,
    label: connectLive ? "Waiting…" : "Reconnect",
    variant: "secondary",
    disabled: connectLive,
    onPress: IntentRef("DesktopCodexReconnectRequested", StaticPayload(ref)),
    a11y: {
      label: connectLive
        ? `Reconnect Codex account ${ref} unavailable: another device-auth flow is already running`
        : `Reconnect Codex account ${ref} with the isolated device-auth flow`,
    },
  })

const codexAccountRow = (connectLive: boolean) => (account: CodexAccountItem): View =>
  Stack(
    {
      key: `settings-account-${account.ref}`,
      direction: "row",
      gap: "2",
      align: "center",
      style: { width: "full" },
    },
    [
      Text({
        key: `settings-account-${account.ref}-ref`,
        content: account.ref,
        variant: "body",
        color: "textPrimary",
      }),
      Spacer({ key: `settings-account-${account.ref}-fill`, flex: true }),
      Badge({
        key: `settings-account-${account.ref}-readiness`,
        label: account.readiness,
        tone: readinessTone(account.readiness),
        a11y: { label: `Account ${account.ref} readiness: ${account.readiness}` },
      }),
      ...(account.readiness === "ready"
        ? []
        : [
            // Disabled-control reason popover (owner contract EP250): while
            // another device-auth flow is live the disabled Reconnect
            // explains itself on hover/focus — never a standing caption.
            (connectLive
              ? Tooltip(
                  {
                    key: `settings-account-${account.ref}-reconnect-reason`,
                    content: "Another Codex device-auth flow is already running — finish or wait for it first.",
                    placement: { side: "top", align: "start" },
                  },
                  [reconnectButton(account.ref, connectLive)],
                )
              : reconnectButton(account.ref, connectLive)),
          ]),
    ],
  )

const accountsSection = (
  accounts: CodexAccountsView,
  connectLive: boolean,
): ReadonlyArray<View> => {
  if (accounts.state === "loading") {
    return [
      Text({
        key: "settings-accounts-loading",
        content: "Loading connected accounts…",
        variant: "body",
        color: "textMuted",
      }),
    ]
  }
  if (accounts.state === "unavailable") {
    return [
      Text({
        key: "settings-accounts-unavailable",
        content: accounts.message,
        variant: "body",
        color: "textMuted",
      }),
    ]
  }
  if (accounts.accounts.length === 0) {
    return [
      Text({
        key: "settings-accounts-empty",
        content: "No Codex accounts connected yet.",
        variant: "body",
        color: "textMuted",
      }),
    ]
  }
  return accounts.accounts.map(codexAccountRow(connectLive))
}

const claudeAccountsSection = (accounts: ClaudeAccountsView): ReadonlyArray<View> => {
  if (accounts.state === "loading") {
    return [
      Text({
        key: "settings-claude-accounts-loading",
        content: "Loading Claude accounts…",
        variant: "body",
        color: "textMuted",
      }),
    ]
  }
  if (accounts.state === "unavailable") {
    return [
      Text({
        key: "settings-claude-accounts-unavailable",
        content: accounts.message,
        variant: "body",
        color: "textMuted",
      }),
    ]
  }
  if (accounts.accounts.length === 0) {
    return [
      Text({
        key: "settings-claude-accounts-empty",
        content: "No Claude accounts linked on this machine.",
        variant: "body",
        color: "textMuted",
      }),
    ]
  }
  return accounts.accounts.map(accountRow("settings-claude-account"))
}

const connectStatusSection = (
  connect: CodexConnectStatusView,
  connectTarget: string | null,
): ReadonlyArray<View> => {
  if (connect.state === "idle") return []
  if (connect.state === "starting") {
    return [
      Text({
        key: "settings-connect-status",
        content: connectTarget === null
          ? "Starting the Codex device-auth flow…"
          : `Starting the Codex device-auth flow for ${connectTarget}…`,
        variant: "body",
        color: "textMuted",
      }),
    ]
  }
  if (connect.state === "awaiting_browser") {
    return [
      Text({
        key: "settings-connect-status",
        content: connectTarget === null
          ? "Open this link in your browser and enter the code below:"
          : `Reconnecting ${connectTarget} — open this link in your browser and enter the code below:`,
        variant: "body",
        color: "textPrimary",
      }),
      Button({
        key: "settings-connect-link",
        label: connect.url,
        variant: "ghost",
        onPress: IntentRef("DesktopCodexVerificationOpened"),
        a11y: { label: `Open verification link ${connect.url} in your browser` },
      }),
      Text({
        key: "settings-connect-code",
        content: connect.code,
        variant: "heading",
        color: "textPrimary",
      }),
    ]
  }
  if (connect.state === "connected") {
    return [
      Badge({
        key: "settings-connect-status",
        label: connectTarget !== null && connectTarget === connect.ref
          ? `Reconnected: ${connect.ref}`
          : `Connected: ${connect.ref}`,
        tone: "success",
        a11y: { label: `Codex account ${connect.ref} connected` },
      }),
    ]
  }
  return [
    Badge({
      key: "settings-connect-status",
      label: `Connect failed: ${connect.reason}`,
      tone: "warn",
      a11y: { label: `Codex connect failed: ${connect.reason}` },
    }),
  ]
}

const openAgentsSessionSection = (
  phase: DesktopOpenAgentsSessionView,
): ReadonlyArray<View> => {
  const label = phase === "session_ready"
    ? "OpenAgents account linked"
    : phase === "signed_out"
      ? "Local device ready"
      : phase === "denied"
        ? "Session access removed"
        : phase === "unverified"
          ? "Verifying stored session…"
          : phase === "authenticating"
            ? "Waiting for secure browser…"
            : phase === "loading"
              ? "Checking session…"
              : "Session unavailable"
  const tone = phase === "session_ready" ? "success" as const
    : phase === "denied" || phase === "unavailable" ? "warn" as const
      : "neutral" as const
  const blocked = phase === "loading" || phase === "authenticating" || phase === "unverified"
  return [
    Badge({
      key: "settings-openagents-session-status",
      label,
      tone,
      a11y: { label },
    }),
    Button({
      key: "settings-openagents-session-action",
      label: phase === "session_ready" ? "Disconnect account" : phase === "authenticating" ? "Working…" : "Link OpenAgents account",
      variant: phase === "session_ready" ? "secondary" : "primary",
      disabled: blocked,
      onPress: IntentRef(phase === "session_ready" ? "DesktopOpenAgentsSignOutRequested" : "DesktopOpenAgentsSignInRequested"),
      a11y: { label: phase === "session_ready" ? "Disconnect OpenAgents account and keep local work" : "Link an optional OpenAgents account for cross-device Sync" },
    }),
  ]
}

// ---------------------------------------------------------------------------
// User-configured MCP servers (I2, EP250 wave-2) — pure view.
// ---------------------------------------------------------------------------

const mcpTransportLabel = (transport: "stdio" | "http"): string =>
  transport === "stdio" ? "stdio" : "http"

const mcpServerRow = (status: Readonly<Record<string, string>>) => (
  server: McpConfigServerView,
): View => {
  const unavailable = status[server.name]
  const detail = server.transport === "stdio"
    ? server.command ?? ""
    : server.url ?? ""
  const extras = [
    server.argsCount > 0 ? `${server.argsCount} arg${server.argsCount === 1 ? "" : "s"}` : null,
    server.envCount > 0 ? `${server.envCount} env` : null,
    server.headersCount > 0 ? `${server.headersCount} header${server.headersCount === 1 ? "" : "s"}` : null,
  ].filter((value): value is string => value !== null)
  return Stack(
    {
      key: `settings-mcp-server-${server.name}`,
      direction: "column",
      gap: "1",
      style: { width: "full" },
    },
    [
      Stack(
        {
          key: `settings-mcp-server-${server.name}-head`,
          direction: "row",
          gap: "2",
          align: "center",
          style: { width: "full" },
        },
        [
          Text({
            key: `settings-mcp-server-${server.name}-name`,
            content: server.name,
            variant: "body",
            color: "textPrimary",
          }),
          Badge({
            key: `settings-mcp-server-${server.name}-transport`,
            label: mcpTransportLabel(server.transport),
            tone: "neutral",
            a11y: { label: `${server.name} transport: ${mcpTransportLabel(server.transport)}` },
          }),
          ...(unavailable === undefined
            ? []
            : [
                Badge({
                  key: `settings-mcp-server-${server.name}-status`,
                  label: "unavailable",
                  tone: "warn",
                  a11y: { label: `${server.name} reported unavailable: ${unavailable}` },
                }),
              ]),
          Spacer({ key: `settings-mcp-server-${server.name}-fill`, flex: true }),
          Toggle({
            key: `settings-mcp-server-${server.name}-toggle`,
            value: server.enabled,
            label: server.enabled ? "Enabled" : "Disabled",
            onChange: IntentRef("DesktopMcpToggleRequested", StaticPayload(server.name)),
            a11y: {
              label: server.enabled
                ? `Disable MCP server ${server.name}`
                : `Enable MCP server ${server.name}`,
            },
          }),
          Button({
            key: `settings-mcp-server-${server.name}-remove`,
            label: "Remove",
            variant: "ghost",
            onPress: IntentRef("DesktopMcpRemoveRequested", StaticPayload(server.name)),
            a11y: { label: `Remove MCP server ${server.name}` },
          }),
        ],
      ),
      ...(detail === "" && extras.length === 0
        ? []
        : [
            Text({
              key: `settings-mcp-server-${server.name}-detail`,
              content: [detail, ...extras].filter((value) => value !== "").join(" · "),
              variant: "label",
              color: "textMuted",
            }),
          ]),
    ],
  )
}

const mcpServersSection = (mcp: McpSettingsState): ReadonlyArray<View> => {
  const servers = mcp.servers
  if (servers.state === "loading") {
    return [
      Text({
        key: "settings-mcp-loading",
        content: "Loading MCP servers…",
        variant: "body",
        color: "textMuted",
      }),
    ]
  }
  if (servers.state === "unavailable") {
    return [
      Text({
        key: "settings-mcp-unavailable",
        content: servers.message,
        variant: "body",
        color: "textMuted",
      }),
    ]
  }
  if (servers.servers.length === 0) {
    return [
      Text({
        key: "settings-mcp-empty",
        content: "No MCP servers configured yet. Add one below to expose its tools as mcp__<name>__<tool>.",
        variant: "body",
        color: "textMuted",
      }),
    ]
  }
  const rows = servers.servers.map(mcpServerRow(mcp.status))
  return servers.dropped > 0
    ? [
        Text({
          key: "settings-mcp-dropped",
          content: `${servers.dropped} stored server${servers.dropped === 1 ? "" : "s"} were dropped as invalid.`,
          variant: "label",
          color: "textMuted",
        }),
        ...rows,
      ]
    : rows
}

const mcpAddForm = (mcp: McpSettingsState): ReadonlyArray<View> => {
  const draft = mcp.draft
  const transportFields = draft.transport === "stdio"
    ? [
        TextField({
          key: "settings-mcp-field-command",
          value: draft.command,
          placeholder: "Command (e.g. npx)",
          label: "Command",
          onChange: IntentRef("DesktopMcpCommandChanged", ComponentValueBinding()),
          a11y: { label: "MCP server command" },
        }),
        TextField({
          key: "settings-mcp-field-args",
          value: draft.argsText,
          multiline: true,
          placeholder: "Arguments (space or newline separated)",
          label: "Arguments",
          onChange: IntentRef("DesktopMcpArgsChanged", ComponentValueBinding()),
          a11y: { label: "MCP server arguments" },
        }),
        TextField({
          key: "settings-mcp-field-env",
          value: draft.envText,
          multiline: true,
          placeholder: "Environment (KEY=value per line)",
          label: "Environment",
          onChange: IntentRef("DesktopMcpEnvChanged", ComponentValueBinding()),
          a11y: { label: "MCP server environment variables" },
        }),
      ]
    : [
        TextField({
          key: "settings-mcp-field-url",
          value: draft.url,
          placeholder: "https://…",
          label: "URL",
          onChange: IntentRef("DesktopMcpUrlChanged", ComponentValueBinding()),
          a11y: { label: "MCP server URL" },
        }),
        TextField({
          key: "settings-mcp-field-headers",
          value: draft.headersText,
          multiline: true,
          placeholder: "Headers (Key: Value per line)",
          label: "Headers",
          onChange: IntentRef("DesktopMcpHeadersChanged", ComponentValueBinding()),
          a11y: { label: "MCP server request headers" },
        }),
      ]
  return [
    Divider({ key: "settings-mcp-add-divider" }),
    Text({
      key: "settings-mcp-add-title",
      content: "Add MCP server",
      variant: "label",
      color: "textMuted",
    }),
    TextField({
      key: "settings-mcp-field-name",
      value: draft.name,
      placeholder: "Name (letters, digits, _ or -)",
      label: "Name",
      onChange: IntentRef("DesktopMcpNameChanged", ComponentValueBinding()),
      a11y: { label: "MCP server name" },
    }),
    RadioGroup({
      key: "settings-mcp-field-transport",
      name: "mcp-transport",
      value: draft.transport,
      orientation: "horizontal",
      label: "Transport",
      options: [
        { value: "stdio", label: "stdio" },
        { value: "http", label: "http" },
      ],
      onChange: IntentRef("DesktopMcpTransportChanged", ComponentValueBinding()),
      a11y: { label: "MCP server transport" },
    }),
    ...transportFields,
    ...(mcp.formError === null
      ? []
      : [
          Text({
            key: "settings-mcp-form-error",
            content: mcp.formError,
            variant: "body",
            color: "danger",
          }),
        ]),
    Button({
      key: "settings-mcp-add",
      label: "Add server",
      variant: "primary",
      onPress: IntentRef("DesktopMcpAddRequested"),
      a11y: { label: "Add this MCP server to the configuration" },
    }),
  ]
}

const mcpSection = (mcp: McpSettingsState): ReadonlyArray<View> => [
  Text({
    key: "settings-mcp-title",
    content: "MCP servers",
    variant: "label",
    color: "textMuted",
  }),
  ...mcpServersSection(mcp),
  ...mcpAddForm(mcp),
]

export const settingsView = (settings: SettingsState): View => {
  const connectLive = connectStatusIsLive(settings.connect)
  return Card(
    {
      key: "settings-screen",
      // apps-sdk chrome port (EP250 #8712): in-flow panel — 16px section
      // padding, flat on a raised surface with a hairline `borderSubtle`
      // edge (overlay shadows are reserved for floating overlays).
      padding: "4",
      radius: "lg",
      style: {
        width: "full",
        maxWidth: 840,
        alignSelf: "center",
        backgroundColor: "surfaceRaised",
        borderColor: "borderSubtle",
        borderWidth: 1,
      },
    },
    [
      Stack({ key: "settings-content", direction: "column", gap: "2" }, [
        Stack(
          { key: "settings-header", direction: "row", gap: "2", align: "center" },
          [
            Text({
              key: "settings-title",
              content: "Settings",
              variant: "title",
              color: "textPrimary",
            }),
            Spacer({ key: "settings-header-fill", flex: true }),
            Button({
              key: "settings-back",
              label: "Back",
              variant: "ghost",
              style: { borderWidth: 0, borderRadius: "md", typeScale: "label" },
              onPress: IntentRef("DesktopSettingsToggled"),
              a11y: { label: "Back to chat" },
            }),
          ],
        ),
        Text({
          key: "settings-openagents-title",
          content: "Local device · optional OpenAgents account",
          variant: "label",
          color: "textMuted",
        }),
        ...openAgentsSessionSection(settings.openAgentsSession),
        Text({key:"settings-local-first-copy",content:"Local coding, conversations, and fleets work without an account. Link an account for cross-device Sync, hosted capacity, and network participation; disconnecting never deletes local work.",variant:"body",color:"textMuted"}),
        Text({
          key: "settings-accounts-title",
          content: "Codex accounts",
          variant: "label",
          color: "textMuted",
        }),
        ...accountsSection(settings.accounts, connectLive),
        Button({
          key: "settings-connect-codex",
          label: connectLive ? "Connecting…" : "Connect Codex account",
          variant: "primary",
          disabled: connectLive,
          onPress: IntentRef("DesktopCodexConnectRequested"),
          a11y: { label: "Connect a Codex account with the isolated device-auth flow" },
        }),
        ...connectStatusSection(settings.connect, settings.connectTarget),
        Text({
          key: "settings-claude-accounts-title",
          content: "Claude accounts",
          variant: "label",
          color: "textMuted",
        }),
        ...claudeAccountsSection(settings.claudeAccounts),
        ...mcpSection(settings.mcp),
      ]),
    ],
  )
}
