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

export type CodexConnectStatusView =
  | Readonly<{ state: "idle" }>
  | Readonly<{ state: "starting" }>
  | Readonly<{ state: "awaiting_browser"; url: string; code: string }>
  | Readonly<{ state: "connected"; ref: string }>
  | Readonly<{ state: "failed"; reason: string }>

export type SettingsState = Readonly<{
  accounts: CodexAccountsView
  connect: CodexConnectStatusView
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
  connect: { state: "idle" },
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
  connectStatus: () => Promise<unknown>
  openVerification: () => Promise<unknown>
}>

export const unavailableCodexSettingsBridge: CodexSettingsBridge = {
  listAccounts: async () => ({
    state: "unavailable",
    message: "Local Pylon runtime is unavailable. No accounts were read.",
  }),
  connectStart: async () => ({ state: "failed", reason: "pylon_runtime_unavailable" }),
  connectStatus: async () => ({ state: "failed", reason: "pylon_runtime_unavailable" }),
  openVerification: async () => false,
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
) => {
  const update = (transform: (current: S) => S) => SubscriptionRef.update(state, transform)

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
      if (!connectStatusIsLive(status)) return
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
          settings: withSettingsAccounts(next.settings, { state: "loading" }),
        } as S))
        const accounts = decodeAccountsView(
          yield* Effect.promise(() => bridge.listAccounts().catch(() => null)),
        )
        const openAgentsSession = decodeOpenAgentsSessionView(
          yield* Effect.promise(() => openAgentsBridge.status().catch(() => null)),
        )
        yield* update((next) => ({
          ...next,
          settings: {
            ...withSettingsAccounts(next.settings, accounts),
            openAgentsSession,
          },
        }))
      }),
    DesktopCodexConnectRequested: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        if (connectStatusIsLive(current.settings.connect)) return
        yield* update((next) => ({
          ...next,
          settings: withSettingsConnectStatus(next.settings, { state: "starting" }),
        }))
        const started = decodeConnectStatusView(
          yield* Effect.promise(() => bridge.connectStart().catch(() => null)),
        )
        yield* update((next) => ({
          ...next,
          settings: withSettingsConnectStatus(next.settings, started),
        }))
        if (connectStatusIsLive(started)) {
          yield* pollConnectStatus
        }
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

const accountRow = (account: CodexAccountItem): View =>
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
    ],
  )

const accountsSection = (accounts: CodexAccountsView): ReadonlyArray<View> => {
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
  return accounts.accounts.map(accountRow)
}

const connectStatusSection = (connect: CodexConnectStatusView): ReadonlyArray<View> => {
  if (connect.state === "idle") return []
  if (connect.state === "starting") {
    return [
      Text({
        key: "settings-connect-status",
        content: "Starting the Codex device-auth flow…",
        variant: "body",
        color: "textMuted",
      }),
    ]
  }
  if (connect.state === "awaiting_browser") {
    return [
      Text({
        key: "settings-connect-status",
        content: "Open this link in your browser and enter the code below:",
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
        label: `Connected: ${connect.ref}`,
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
    ? "OpenAgents session verified"
    : phase === "signed_out"
      ? "Signed out"
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
      label: phase === "session_ready" ? "Sign out" : phase === "authenticating" ? "Working…" : "Sign in with GitHub",
      variant: phase === "session_ready" ? "secondary" : "primary",
      disabled: blocked,
      onPress: IntentRef(phase === "session_ready" ? "DesktopOpenAgentsSignOutRequested" : "DesktopOpenAgentsSignInRequested"),
      a11y: { label: phase === "session_ready" ? "Sign out of OpenAgents" : "Sign in to OpenAgents with GitHub" },
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
          content: "OpenAgents session",
          variant: "label",
          color: "textMuted",
        }),
        ...openAgentsSessionSection(settings.openAgentsSession),
        Text({
          key: "settings-accounts-title",
          content: "Codex accounts",
          variant: "label",
          color: "textMuted",
        }),
        ...accountsSection(settings.accounts),
        Button({
          key: "settings-connect-codex",
          label: connectLive ? "Connecting…" : "Connect Codex account",
          variant: "primary",
          disabled: connectLive,
          onPress: IntentRef("DesktopCodexConnectRequested"),
          a11y: { label: "Connect a Codex account with the isolated device-auth flow" },
        }),
        ...connectStatusSection(settings.connect),
      ]),
    ],
  )
}
