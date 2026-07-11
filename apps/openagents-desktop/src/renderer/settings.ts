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
  IntentRef,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  defineIntent,
  type View,
} from "@effect-native/core"
import { Effect, Exit, Schema, SubscriptionRef } from "@effect-native/core/effect"

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
}>

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
})

const accountRefPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/
const userCodePattern = /^[A-Z0-9]{4}-[A-Z0-9]{4,6}$/

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

export const settingsIntents = [
  DesktopSettingsToggled,
  DesktopCodexConnectRequested,
  DesktopCodexReconnectRequested,
  DesktopCodexVerificationOpened,
  DesktopOpenAgentsSignInRequested,
  DesktopOpenAgentsSignOutRequested,
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
) => {
  const update = (transform: (current: S) => S) => SubscriptionRef.update(state, transform)

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
          settings: withSettingsClaudeAccounts(
            withSettingsAccounts(next.settings, { state: "loading" }),
            { state: "loading" },
          ),
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
            Button({
              key: `settings-account-${account.ref}-reconnect`,
              label: connectLive ? "Waiting…" : "Reconnect",
              variant: "secondary",
              disabled: connectLive,
              onPress: IntentRef("DesktopCodexReconnectRequested", StaticPayload(account.ref)),
              a11y: {
                label: `Reconnect Codex account ${account.ref} with the isolated device-auth flow`,
              },
            }),
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

export const settingsView = (settings: SettingsState): View => {
  const connectLive = connectStatusIsLive(settings.connect)
  return Card(
    {
      key: "settings-screen",
      padding: "3",
      radius: "lg",
      style: {
        width: "full",
        maxWidth: 840,
        alignSelf: "center",
        backgroundColor: "surfaceRaised",
        borderColor: "border",
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
      ]),
    ],
  )
}
