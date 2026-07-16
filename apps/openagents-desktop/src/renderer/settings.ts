/**
 * Settings screen (#8574, #8640 unblock): the most minimal UI needed to
 * configure local Desktop behavior without exposing fleet account custody.
 *
 * The MVP uses the ordinary Codex session already logged in on this machine.
 * Named account linking and Pylon device-auth are fleet capabilities and do
 * not appear on this surface.
 */
import {
  Badge,
  Button,
  Card,
  ComponentValueBinding,
  Divider,
  IntentRef,
  SegmentedControl,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  TextField,
  Toggle,
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
import { PluginRefSchema, decodePluginConfigResult, type PluginConfigView, type PluginRef } from "../plugin-config-contract.ts"
import {
  unifiedExtensionLifecycle,
  type ExtensionLifecycleAudit,
  type ExtensionLifecycleEntry,
} from "../extension-lifecycle-contract.ts"
import {
  DesktopAcpProviderActionRequested,
  DesktopAcpSupportExportRequested,
  availableAcpProviderActions,
  decodeAcpProviderActionPayload,
  decodeAcpProviderSettings,
  initialAcpProviderSettingsState,
  unavailableAcpProviderSettingsBridge,
  type AcpProviderSettingsBridge,
  type AcpProviderSettingsState,
} from "../acp-provider-contract.ts"
import { acpProviderSettingsView } from "./acp-provider-settings.ts"

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

// ---------------------------------------------------------------------------
// Typed per-harness maintenance (MAINT-1, #8785): the renderer projection of
// installed version + channel + advisory per coding harness, plus the
// one-click update affordance state. Public-safe by construction — versions,
// channel, and advisory only; never paths or command output.
// ---------------------------------------------------------------------------

export type HarnessMaintenanceHarnessName = "codex"

export type HarnessMaintenanceItemView = Readonly<{
  harness: HarnessMaintenanceHarnessName
  installed: boolean
  installedVersion: string | null
  latestVersion: string | null
  channel: string
  advisory: "current" | "behind_latest" | "unknown"
  updateSupported: boolean
  runtimeState?: string
  recoveryMessage?: string | null
}>

export type CodexReleaseNotesView = Readonly<{
  version: string
  title: string
  body: string
  publishedAt: string | null
}>

export type HarnessMaintenanceListView =
  | Readonly<{ state: "loading" }>
  | Readonly<{ state: "loaded"; harnesses: ReadonlyArray<HarnessMaintenanceItemView> }>
  | Readonly<{ state: "unavailable"; message: string }>

export type HarnessMaintenanceState = Readonly<{
  view: HarnessMaintenanceListView
  /** Harness with an update in flight, or null. One update at a time. */
  updating: HarnessMaintenanceHarnessName | null
  /** Human-readable result of the last update attempt (public-safe). */
  lastOutcome: string | null
  /** Best-effort official GitHub release notes for the registry target. */
  codexReleaseNotes?: CodexReleaseNotesView | null
}>

export const initialHarnessMaintenanceState = (): HarnessMaintenanceState => ({
  view: { state: "loading" },
  updating: null,
  lastOutcome: null,
  codexReleaseNotes: null,
})

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
  plugins: PluginSettingsState
  /** Per-harness maintenance (MAINT-1, #8785). */
  harnessMaintenance: HarnessMaintenanceState
  localCodexUsageControlAvailable: boolean
  shareLocalCodexUsage: boolean
  acpProviders: AcpProviderSettingsState
  acpSupportNotice: string | null
}>

export type PluginSettingsState = Readonly<{
  state: "loading" | "loaded" | "unavailable"
  plugins: ReadonlyArray<PluginConfigView>
  dropped: number
  message: string | null
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
  plugins: { state: "loading", plugins: [], dropped: 0, message: null },
  harnessMaintenance: initialHarnessMaintenanceState(),
  localCodexUsageControlAvailable: false,
  shareLocalCodexUsage: false,
  acpProviders: initialAcpProviderSettingsState(),
  acpSupportNotice: null,
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

// Renderer mirror of the gateway maintenance responses (the same both-sides
// schema pattern as the session/connect contracts above).
const harnessNameSchema = Schema.Literal("codex")
const RendererHarnessMaintenanceStatusSchema = Schema.Struct({
  kind: Schema.Literal("harness_maintenance_status"),
  observedAt: Schema.String,
  harnesses: Schema.Array(Schema.Struct({
    harness: harnessNameSchema,
    installed: Schema.Boolean,
    installedVersion: Schema.NullOr(Schema.String),
    latestVersion: Schema.NullOr(Schema.String),
    channel: Schema.String,
    advisory: Schema.Literals(["current", "behind_latest", "unknown"]),
    updateSupported: Schema.Boolean,
    runtimeState: Schema.optional(Schema.String),
    recoveryMessage: Schema.optional(Schema.NullOr(Schema.String)),
  })),
  codexReleaseNotes: Schema.optional(Schema.NullOr(Schema.Struct({
    version: Schema.String,
    title: Schema.String,
    body: Schema.String,
    publishedAt: Schema.NullOr(Schema.String),
  }))),
})
const RendererHarnessMaintenanceOutcomeSchema = Schema.Struct({
  kind: Schema.Literal("harness_maintenance_outcome"),
  harness: harnessNameSchema,
  status: Schema.Literals(["completed", "unavailable"]),
  outcome: Schema.NullOr(
    Schema.Literals(["updated", "already_current", "channel_jump_refused", "failed"]),
  ),
  failureReason: Schema.NullOr(Schema.String),
  beforeVersion: Schema.NullOr(Schema.String),
  afterVersion: Schema.NullOr(Schema.String),
  receiptId: Schema.NullOr(Schema.String),
})

const versionText = (value: string | null): string | null =>
  value === null ? null : value.slice(0, 40)

export const decodeHarnessMaintenanceStatus = (value: unknown): Readonly<{
  view: HarnessMaintenanceListView
  codexReleaseNotes: CodexReleaseNotesView | null
}> => {
  const decoded = Schema.decodeUnknownExit(RendererHarnessMaintenanceStatusSchema)(value)
  if (!Exit.isSuccess(decoded)) {
    return {
      view: { state: "unavailable", message: "Harness maintenance is unavailable on this build." },
      codexReleaseNotes: null,
    }
  }
  return {
    view: {
      state: "loaded",
      harnesses: decoded.value.harnesses.map((entry) => ({
        harness: entry.harness,
        installed: entry.installed,
        installedVersion: versionText(entry.installedVersion),
        latestVersion: versionText(entry.latestVersion),
        channel: entry.channel.slice(0, 20),
        advisory: entry.advisory,
        updateSupported: entry.updateSupported,
        ...(entry.runtimeState === undefined ? {} : { runtimeState: entry.runtimeState.slice(0, 40) }),
        ...(entry.recoveryMessage === undefined ? {} : { recoveryMessage: entry.recoveryMessage?.slice(0, 160) ?? null }),
      })),
    },
    codexReleaseNotes: decoded.value.codexReleaseNotes == null ? null : {
      version: decoded.value.codexReleaseNotes.version.slice(0, 40),
      title: decoded.value.codexReleaseNotes.title.slice(0, 160),
      body: decoded.value.codexReleaseNotes.body.slice(0, 12_000),
      publishedAt: decoded.value.codexReleaseNotes.publishedAt,
    },
  }
}

export const decodeHarnessMaintenanceListView = (value: unknown): HarnessMaintenanceListView =>
  decodeHarnessMaintenanceStatus(value).view

/** Public-safe, human-readable summary of an update attempt. */
export const decodeHarnessMaintenanceOutcomeText = (value: unknown): string => {
  const decoded = Schema.decodeUnknownExit(RendererHarnessMaintenanceOutcomeSchema)(value)
  if (!Exit.isSuccess(decoded)) return "Update failed: the maintenance runtime is unavailable."
  const result = decoded.value
  const name = harnessDisplayName(result.harness)
  if (result.status === "unavailable") {
    return result.failureReason === "update_already_running"
      ? `${name}: an update is already running.`
      : `${name}: the maintenance runtime is unavailable.`
  }
  switch (result.outcome) {
    case "updated":
      return `${name} updated to ${versionText(result.afterVersion) ?? "a new version"} (re-probe verified).`
    case "already_current":
      return `${name} is already current.`
    case "channel_jump_refused":
      return `${name}: refused a channel change. Updates stay on the detected install channel.`
    default:
      if (result.harness === "codex" && result.failureReason?.startsWith("repair_openagents") === true) {
        return result.failureReason === "repair_openagents_update_available"
          ? "An OpenAgents update is available. Install it from App updates below to repair bundled Codex; your sign-in is preserved."
          : "Codex is bundled with OpenAgents. Update or reinstall OpenAgents to repair it; your Codex sign-in is preserved."
      }
      return `${name} update failed: ${(result.failureReason ?? "unknown").slice(0, 80)}. Previous install left intact.`
  }
}

export const harnessDisplayName = (_harness: HarnessMaintenanceHarnessName): string => "Codex CLI"

// ---------------------------------------------------------------------------
// Typed bridge surface the settings handlers need (injected from boot.ts;
// defaults are honest "unavailable" stand-ins for headless tests).
// ---------------------------------------------------------------------------

export type HarnessMaintenanceSettingsBridge = Readonly<{
  status: () => Promise<unknown>
  update: (harness: HarnessMaintenanceHarnessName) => Promise<unknown>
}>

export const unavailableHarnessMaintenanceSettingsBridge: HarnessMaintenanceSettingsBridge = {
  status: async () => null,
  update: async () => null,
}

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
export type PluginConfigSettingsBridge = Readonly<{
  list: () => Promise<unknown>
  choose: () => Promise<unknown>
  toggle: (ref: PluginRef, enabled: boolean) => Promise<unknown>
  remove: (ref: PluginRef) => Promise<unknown>
}>
export const unavailablePluginConfigSettingsBridge: PluginConfigSettingsBridge = {
  list: async () => ({ state: "unavailable", message: "Local plugin configuration is unavailable." }),
  choose: async () => ({ state: "unavailable", message: "Local plugin configuration is unavailable." }),
  toggle: async () => ({ state: "unavailable", message: "Local plugin configuration is unavailable." }),
  remove: async () => ({ state: "unavailable", message: "Local plugin configuration is unavailable." }),
}

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
export const DesktopLocalCodexUsageSharingToggled = defineIntent(
  "DesktopLocalCodexUsageSharingToggled",
  Schema.Boolean,
)
/**
 * One-click harness update (MAINT-1, #8785). Payload is the harness name;
 * the handler re-validates it against the loaded maintenance list and the
 * host re-validates against the typed gateway contract regardless.
 */
export const DesktopHarnessUpdateRequested = defineIntent(
  "DesktopHarnessUpdateRequested",
  Schema.String,
)
export const DesktopHarnessMaintenanceRefreshRequested = defineIntent(
  "DesktopHarnessMaintenanceRefreshRequested",
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
export const DesktopPluginChooseRequested = defineIntent("DesktopPluginChooseRequested", Schema.Null)
export const DesktopPluginToggleRequested = defineIntent("DesktopPluginToggleRequested", PluginRefSchema)
export const DesktopPluginRemoveRequested = defineIntent("DesktopPluginRemoveRequested", PluginRefSchema)

export const settingsIntents = [
  DesktopSettingsToggled,
  DesktopCodexConnectRequested,
  DesktopCodexReconnectRequested,
  DesktopCodexVerificationOpened,
  DesktopOpenAgentsSignInRequested,
  DesktopOpenAgentsSignOutRequested,
  DesktopLocalCodexUsageSharingToggled,
  DesktopHarnessUpdateRequested,
  DesktopHarnessMaintenanceRefreshRequested,
  DesktopAcpProviderActionRequested,
  DesktopAcpSupportExportRequested,
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
  DesktopPluginChooseRequested,
  DesktopPluginToggleRequested,
  DesktopPluginRemoveRequested,
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
  pluginBridge: PluginConfigSettingsBridge = unavailablePluginConfigSettingsBridge,
  maintenanceBridge: HarnessMaintenanceSettingsBridge = unavailableHarnessMaintenanceSettingsBridge,
  acpBridge: AcpProviderSettingsBridge = unavailableAcpProviderSettingsBridge,
) => {
  const update = (transform: (current: S) => S) => SubscriptionRef.update(state, transform)

  /** Re-read per-harness version/channel truth through the gateway. */
  const refreshHarnessMaintenance = Effect.gen(function* () {
    const decoded = decodeHarnessMaintenanceStatus(
      yield* Effect.promise(() => maintenanceBridge.status().catch(() => null)),
    )
    yield* update((next) => ({
      ...next,
      settings: {
        ...next.settings,
        harnessMaintenance: {
          ...next.settings.harnessMaintenance,
          view: decoded.view,
          codexReleaseNotes: decoded.codexReleaseNotes,
        },
      },
    }))
  })

  const refreshAcpProviders = Effect.gen(function* () {
    const acpProviders = decodeAcpProviderSettings(
      yield* Effect.promise(() => acpBridge.status().catch(() => null)),
    )
    yield* update((next) => ({ ...next, settings: { ...next.settings, acpProviders } }))
  })

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
  const refreshPlugins = Effect.gen(function* () {
    const result = decodePluginConfigResult(yield* Effect.promise(() => pluginBridge.list().catch(() => null)))
    yield* update(next => ({
      ...next,
      settings: {
        ...next.settings,
        plugins: result.state === "ok"
          ? { state: "loaded", plugins: result.plugins, dropped: result.dropped, message: null }
          : { state: "unavailable", plugins: [], dropped: 0, message: result.state === "unavailable" ? result.message : "Local plugins unavailable." },
      },
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
            plugins: { state: "loading", plugins: [], dropped: 0, message: null },
            harnessMaintenance: {
              ...next.settings.harnessMaintenance,
              view: { state: "loading" },
            },
            acpProviders: { state: "loading" },
            acpSupportNotice: null,
          },
        } as S))
        const openAgentsSession = decodeOpenAgentsSessionView(
          yield* Effect.promise(() => openAgentsBridge.status().catch(() => null)),
        )
        yield* update((next) => ({
          ...next,
          settings: {
            ...next.settings,
            openAgentsSession,
          },
        }))
        yield* refreshMcpServers
        yield* refreshPlugins
        yield* refreshHarnessMaintenance
        yield* refreshAcpProviders
      }),
    DesktopAcpProviderActionRequested: (raw: string) =>
      Effect.gen(function* () {
        const requested = decodeAcpProviderActionPayload(raw)
        if (requested === null) return
        const current = yield* SubscriptionRef.get(state)
        const projection = current.settings.acpProviders.state === "loaded"
          ? current.settings.acpProviders.providers.find((provider) => provider.provider === requested.provider)
          : undefined
        if (projection === undefined || !availableAcpProviderActions(projection).includes(requested.action)) return
        const acpProviders = decodeAcpProviderSettings(
          yield* Effect.promise(() => acpBridge.action(requested.provider, requested.action).catch(() => null)),
        )
        yield* update((next) => ({ ...next, settings: { ...next.settings, acpProviders } }))
      }),
    DesktopAcpSupportExportRequested: () =>
      Effect.gen(function* () {
        const result = yield* Effect.promise(() => acpBridge.supportExport().catch(() => null))
        const notice = typeof result === "object" && result !== null && "notice" in result && typeof result.notice === "string"
          ? result.notice.slice(0, 200)
          : "ACP support export failed."
        yield* update((next) => ({ ...next, settings: { ...next.settings, acpSupportNotice: notice } }))
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
    // Typed per-harness maintenance (MAINT-1, #8785).
    // -----------------------------------------------------------------------
    DesktopHarnessMaintenanceRefreshRequested: () => refreshHarnessMaintenance,
    DesktopHarnessUpdateRequested: (raw: string) =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        const maintenance = current.settings.harnessMaintenance
        // Only a harness the view actually lists (with a supported channel)
        // can be targeted, and only one update runs at a time.
        if (maintenance.updating !== null || maintenance.view.state !== "loaded") return
        const item = maintenance.view.harnesses.find((entry) => entry.harness === raw)
        if (item === undefined || !item.updateSupported) return
        const harness = item.harness
        yield* update((next) => ({
          ...next,
          settings: {
            ...next.settings,
            harnessMaintenance: {
              ...next.settings.harnessMaintenance,
              updating: harness,
              lastOutcome: null,
            },
          },
        }))
        const outcomeText = decodeHarnessMaintenanceOutcomeText(
          yield* Effect.promise(() => maintenanceBridge.update(harness).catch(() => null)),
        )
        yield* update((next) => ({
          ...next,
          settings: {
            ...next.settings,
            harnessMaintenance: {
              ...next.settings.harnessMaintenance,
              updating: null,
              lastOutcome: outcomeText,
            },
          },
        }))
        // RE-PROBE truth reaches the row through a full status re-read: the
        // displayed version is always a fresh detection, never an assumption.
        yield* refreshHarnessMaintenance
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
    DesktopPluginChooseRequested: () => Effect.gen(function* () {
      const result = decodePluginConfigResult(yield* Effect.promise(() => pluginBridge.choose().catch(() => null)))
      if (result.state === "ok") yield* update(next => ({ ...next, settings: { ...next.settings, plugins: { state: "loaded", plugins: result.plugins, dropped: result.dropped, message: null } } }))
    }),
    DesktopPluginToggleRequested: (ref: PluginRef) => Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const plugin = current.settings.plugins.plugins.find(item => item.ref === ref)
      if (plugin === undefined) return
      const result = decodePluginConfigResult(yield* Effect.promise(() => pluginBridge.toggle(ref, !plugin.enabled).catch(() => null)))
      if (result.state === "ok") yield* update(next => ({ ...next, settings: { ...next.settings, plugins: { state: "loaded", plugins: result.plugins, dropped: result.dropped, message: null } } }))
    }),
    DesktopPluginRemoveRequested: (ref: PluginRef) => Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      if (!current.settings.plugins.plugins.some(item => item.ref === ref)) return
      const result = decodePluginConfigResult(yield* Effect.promise(() => pluginBridge.remove(ref).catch(() => null)))
      if (result.state === "ok") yield* update(next => ({ ...next, settings: { ...next.settings, plugins: { state: "loaded", plugins: result.plugins, dropped: result.dropped, message: null } } }))
    }),
  }
}

// ---------------------------------------------------------------------------
// View — pure `state -> View` over the shared catalog.
// ---------------------------------------------------------------------------

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
    Text({
      key: "settings-mcp-field-transport-label",
      content: "Transport",
      variant: "label",
      color: "textMuted",
    }),
    // Connection-mode switch (harmonization #8712 Phase 3, item 15):
    // stdio vs http is a single-choice INPUT control, exactly SegmentedControl's
    // contract — not an ad hoc button row and not a peer-panel Tabs selector.
    SegmentedControl({
      key: "settings-mcp-field-transport",
      value: draft.transport,
      options: [
        { id: "stdio", label: "stdio" },
        { id: "http", label: "http" },
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

const pluginSection = (plugins: PluginSettingsState): ReadonlyArray<View> => [
  Divider({ key: "settings-plugins-divider" }),
  Text({ key: "settings-plugins-title", content: "Local Claude plugins", variant: "label", color: "textMuted" }),
  ...(plugins.state === "loading"
    ? [Text({ key: "settings-plugins-loading", content: "Loading local plugins…", variant: "body", color: "textMuted" })]
    : plugins.state === "unavailable"
      ? [Text({ key: "settings-plugins-unavailable", content: plugins.message ?? "Local plugins unavailable.", variant: "body", color: "textMuted" })]
      : plugins.plugins.length === 0
        ? [Text({ key: "settings-plugins-empty", content: "No local plugins registered.", variant: "body", color: "textMuted" })]
        : plugins.plugins.map(plugin => Stack(
            { key: `settings-plugin-${plugin.ref}`, direction: "row", gap: "2", align: "center", style: { width: "full" } },
            [
              Text({ key: `settings-plugin-${plugin.ref}-name`, content: plugin.name, variant: "body", color: "textPrimary" }),
              Badge({ key: `settings-plugin-${plugin.ref}-status`, label: plugin.readiness, tone: plugin.readiness === "ready" ? "success" : "warn", a11y: { label: `${plugin.name} is ${plugin.readiness}` } }),
              Text({ key: `settings-plugin-${plugin.ref}-scope`, content: "Claude · app scope · next turn", variant: "label", color: "textMuted" }),
              ...(plugin.skills.length === 0 ? [] : [Text({ key: `settings-plugin-${plugin.ref}-skills`, content: plugin.skills.map(name => `/skill ${plugin.name}/${name}`).join(" · "), variant: "label", color: "textMuted" })]),
              Spacer({ key: `settings-plugin-${plugin.ref}-fill`, flex: true }),
              Toggle({ key: `settings-plugin-${plugin.ref}-toggle`, value: plugin.enabled, label: plugin.enabled ? "Enabled" : "Disabled", onChange: IntentRef("DesktopPluginToggleRequested", StaticPayload(plugin.ref)), a11y: { label: `${plugin.enabled ? "Disable" : "Enable"} plugin ${plugin.name}` } }),
              Button({ key: `settings-plugin-${plugin.ref}-remove`, label: "Remove", variant: "ghost", onPress: IntentRef("DesktopPluginRemoveRequested", StaticPayload(plugin.ref)), a11y: { label: `Remove plugin ${plugin.name}` } }),
            ],
          ))),
  Button({ key: "settings-plugin-add", label: "Add local plugin", variant: "secondary", onPress: IntentRef("DesktopPluginChooseRequested"), a11y: { label: "Choose a local Claude plugin directory" } }),
]

// ---------------------------------------------------------------------------
// Unified extension lifecycle audit (CUT-23 #8703): one derived view of
// declare → validate → enable → run → revoke across MCP servers, plugins,
// and skills. PURE derivation over state this screen already holds — no new
// IPC, no parallel registry; the per-surface sections above stay the only
// mutation (grant/revoke) controls.
// ---------------------------------------------------------------------------

export const settingsExtensionAudit = (settings: SettingsState): ExtensionLifecycleAudit =>
  unifiedExtensionLifecycle({
    mcpServers: settings.mcp.servers.state === "loaded" ? settings.mcp.servers.servers : null,
    mcpDropped: settings.mcp.servers.state === "loaded" ? settings.mcp.servers.dropped : 0,
    plugins: settings.plugins.state === "loaded" ? settings.plugins.plugins : null,
    pluginsDropped: settings.plugins.state === "loaded" ? settings.plugins.dropped : 0,
  })

const lifecycleStageTone = (stage: ExtensionLifecycleEntry["stage"]): "success" | "warn" | "neutral" =>
  stage === "granted" ? "success" : stage === "invalid" ? "warn" : "neutral"

const lifecycleKindLabel: Readonly<Record<ExtensionLifecycleEntry["kind"], string>> = {
  mcp_server: "MCP server",
  plugin: "Plugin",
  skill: "Skill",
}

const lifecycleGrantCaption = (entry: ExtensionLifecycleEntry): string => {
  const use = entry.grant.use === "next_turn" ? "next turn" : "explicit /skill"
  const grant = entry.grant.state === "active"
    ? `granted · ${use}`
    : entry.grant.state === "revoked"
      ? "grant revoked"
      : "grant blocked"
  return entry.duplicateLabel ? `${grant} · duplicate name` : grant
}

const lifecycleEntryRow = (entry: ExtensionLifecycleEntry): View =>
  Stack(
    {
      key: `settings-lifecycle-${entry.kind}-${entry.id}`,
      direction: "row",
      gap: "2",
      align: "center",
      style: { width: "full" },
    },
    [
      Text({
        key: `settings-lifecycle-${entry.kind}-${entry.id}-kind`,
        content: lifecycleKindLabel[entry.kind],
        variant: "label",
        color: "textMuted",
      }),
      Text({
        key: `settings-lifecycle-${entry.kind}-${entry.id}-label`,
        content: entry.label,
        variant: "body",
        color: "textPrimary",
      }),
      Badge({
        key: `settings-lifecycle-${entry.kind}-${entry.id}-stage`,
        label: entry.stage,
        tone: lifecycleStageTone(entry.stage),
        a11y: { label: `${lifecycleKindLabel[entry.kind]} ${entry.label} lifecycle stage: ${entry.stage}` },
      }),
      Text({
        key: `settings-lifecycle-${entry.kind}-${entry.id}-grant`,
        content: lifecycleGrantCaption(entry),
        variant: "label",
        color: "textMuted",
      }),
      Spacer({ key: `settings-lifecycle-${entry.kind}-${entry.id}-fill`, flex: true }),
      // Provider disagreement stays explicit — never emulated.
      Text({
        key: `settings-lifecycle-${entry.kind}-${entry.id}-provider`,
        content: entry.providerSupport.codex === "supported" ? "Claude · Codex" : "Claude only",
        variant: "label",
        color: "textMuted",
      }),
    ],
  )

const extensionLifecycleSection = (settings: SettingsState): ReadonlyArray<View> => {
  const audit = settingsExtensionAudit(settings)
  const dropped = audit.droppedInvalid.mcpServers + audit.droppedInvalid.plugins
  const summary = [
    `${audit.granted} granted`,
    `${audit.revoked} revoked`,
    ...(audit.blocked > 0 ? [`${audit.blocked} blocked`] : []),
    ...(dropped > 0 ? [`${dropped} invalid dropped`] : []),
  ].join(" · ")
  return [
    Divider({ key: "settings-lifecycle-divider" }),
    Text({
      key: "settings-lifecycle-title",
      content: "Extension lifecycle",
      variant: "label",
      color: "textMuted",
    }),
    Text({
      key: "settings-lifecycle-summary",
      content: summary,
      variant: "label",
      color: "textMuted",
    }),
    ...(audit.partial
      ? [Text({
          key: "settings-lifecycle-partial",
          content: "Some registries are still loading or unavailable — this audit is partial.",
          variant: "body",
          color: "textMuted",
        })]
      : []),
    ...(audit.entries.length === 0 && !audit.partial
      ? [Text({
          key: "settings-lifecycle-empty",
          content: "No MCP servers, plugins, or skills registered.",
          variant: "body",
          color: "textMuted",
        })]
      : audit.entries.map(lifecycleEntryRow)),
  ]
}

export const settingsView = (settings: SettingsState): View => {
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
        maxWidth: "2xl",
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
              // Ghost variant already resolves to a transparent border and the
              // "md" control step already resolves to the same 13px/radius-4
              // metrics this used to hand-spell via `style` (harmonization
              // #8712 Phase 3, item 15) — the recipe was a zero-visual-change
              // duplicate of the matrix default.
              variant: "ghost",
              onPress: IntentRef("DesktopSettingsToggled"),
              a11y: { label: "Back to chat" },
            }),
          ],
        ),
        Text({
          key: "settings-codex-session-title",
          content: "Codex session",
          variant: "label",
          color: "textMuted",
        }),
        Text({
          key: "settings-codex-session-copy",
          content: "OpenAgents uses the Codex session already signed in on this Mac. No separate account linking is required.",
          variant: "body",
          color: "textMuted",
        }),
        ...(settings.localCodexUsageControlAvailable
          ? [
              Divider({ key: "settings-local-usage-divider" }),
              Stack(
                { key: "settings-local-usage-row", direction: "row", gap: "3", align: "center" },
                [
                  Stack(
                    { key: "settings-local-usage-copy-stack", direction: "column", gap: "1" },
                    [
                      Text({
                        key: "settings-local-usage-title",
                        content: "Share local Codex usage",
                        variant: "label",
                        color: "textPrimary",
                      }),
                      Text({
                        key: "settings-local-usage-copy",
                        content: "When on, OpenAgents reports how many tokens each turn used — the input, cached-input, output, reasoning, and total token counts — plus the model name and a one-time turn reference. Only those numbers are sent: never your prompts, responses, files, paths, account names, or credentials. This updates the aggregate public tokens-served counter. Turn it off any time; queued reports are deleted.",
                        variant: "body",
                        color: "textMuted",
                      }),
                    ],
                  ),
                  Spacer({ key: "settings-local-usage-fill", flex: true }),
                  Toggle({
                    key: "settings-local-usage-toggle",
                    value: settings.shareLocalCodexUsage,
                    label: settings.shareLocalCodexUsage ? "On" : "Off",
                    onChange: IntentRef("DesktopLocalCodexUsageSharingToggled", ComponentValueBinding()),
                    a11y: {
                      label: settings.shareLocalCodexUsage
                        ? "Stop sharing local Codex token usage"
                        : "Share local Codex token usage",
                    },
                  }),
                ],
              ),
            ]
          : []),
        Divider({ key: "settings-harness-maintenance-divider" }),
        ...harnessMaintenanceSection(settings.harnessMaintenance),
        ...acpProviderSettingsView(settings.acpProviders, settings.acpSupportNotice),
      ]),
    ],
  )
}

// ---------------------------------------------------------------------------
// Typed per-harness maintenance section (MAINT-1, #8785): version/channel
// truth per coding harness with a one-click update driving the typed gateway
// command. Updates swap BINARIES only — sign-in state is never touched.
// ---------------------------------------------------------------------------

const harnessChannelLabel = (channel: string): string =>
  channel === "desktop-bundle"
    ? "OpenAgents bundle"
    : channel === "npm-global"
    ? "npm"
    : channel === "bun-global"
      ? "bun"
      : channel === "pnpm-global"
        ? "pnpm"
        : channel === "homebrew"
          ? "Homebrew"
          : channel === "native"
            ? "native installer"
            : "unknown channel"

const harnessMaintenanceRow = (
  item: HarnessMaintenanceItemView,
  updating: HarnessMaintenanceHarnessName | null,
): View => {
  const versionLine = !item.installed
    ? "Not installed"
    : item.installedVersion === null
      ? `Installed (${harnessChannelLabel(item.channel)}) — version probe failed`
      : `${item.installedVersion} via ${harnessChannelLabel(item.channel)}${
          item.advisory === "behind_latest" && item.latestVersion !== null
            ? ` — ${item.latestVersion} available`
            : item.advisory === "current"
              ? " — up to date"
              : ""
        }`
  const isUpdating = updating === item.harness
  return Stack(
    {
      key: `settings-harness-${item.harness}-row`,
      direction: "row",
      gap: "2",
      align: "center",
    },
    [
      Stack(
        { key: `settings-harness-${item.harness}-info`, direction: "column", gap: "1" },
        [
          Text({
            key: `settings-harness-${item.harness}-name`,
            content: harnessDisplayName(item.harness),
            variant: "body",
            color: "textPrimary",
          }),
          Text({
            key: `settings-harness-${item.harness}-version`,
            content: versionLine,
            variant: "label",
            color: "textMuted",
          }),
          ...(item.recoveryMessage === undefined || item.recoveryMessage === null
            ? []
            : [Text({
                key: `settings-harness-${item.harness}-recovery`,
                content: item.recoveryMessage,
                variant: "label",
                color: "textMuted",
              })]),
        ],
      ),
      Spacer({ key: `settings-harness-${item.harness}-fill`, flex: true }),
      ...(item.installed && !item.updateSupported
        ? [
            Text({
              key: `settings-harness-${item.harness}-manual`,
              content: "Manual update required",
              variant: "label",
              color: "textMuted",
            }),
          ]
        : []),
      ...(item.updateSupported
        ? [
            Button({
              key: `settings-harness-${item.harness}-update`,
              label: isUpdating
                ? "Updating…"
                : item.channel === "desktop-bundle"
                  ? "Repair OpenAgents"
                  : item.advisory === "behind_latest" && item.latestVersion !== null
                  ? `Update to ${item.latestVersion}`
                  : "Check & update",
              variant: item.advisory === "behind_latest" ? "primary" : "ghost",
              disabled: isUpdating || updating !== null,
              onPress: IntentRef("DesktopHarnessUpdateRequested", StaticPayload(item.harness)),
              a11y: { label: item.channel === "desktop-bundle" ? "Repair OpenAgents bundled Codex" : `Update ${harnessDisplayName(item.harness)}` },
            }),
          ]
        : []),
    ],
  )
}

export const harnessMaintenanceSection = (maintenance: HarnessMaintenanceState): View[] => [
  Text({
    key: "settings-harness-maintenance-title",
    content: "Codex CLI",
    variant: "label",
    color: "textMuted",
  }),
  ...(maintenance.view.state === "loading"
    ? [
        Text({
          key: "settings-harness-maintenance-loading",
          content: "Checking installed harness versions…",
          variant: "body",
          color: "textMuted",
        }),
      ]
    : maintenance.view.state === "unavailable"
      ? [
          Text({
            key: "settings-harness-maintenance-unavailable",
            content: maintenance.view.message,
            variant: "body",
            color: "textMuted",
          }),
        ]
      : maintenance.view.harnesses.filter(item => item.harness === "codex").map((item) =>
          harnessMaintenanceRow(item, maintenance.updating),
        )),
  ...(maintenance.lastOutcome === null
    ? []
    : [
        Text({
          key: "settings-harness-maintenance-outcome",
          content: maintenance.lastOutcome,
          variant: "label",
          color: "textMuted",
        }),
      ]),
]
