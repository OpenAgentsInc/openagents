/**
 * Settings view-program tests (#8574, #8640 unblock): pure state -> view for
 * the Codex reconnect screen, plus the full typed intent loop with a fake
 * bridge — open Settings, load accounts, start connect, poll to connected.
 */
import { describe, expect, test } from "bun:test"
import { resolveIntentRef, type View } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"

import {
  connectStatusIsLive,
  decodeAccountsView,
  decodeConnectStatusView,
  decodeOpenAgentsSessionView,
  initialSettingsState,
  settingsView,
  withSettingsAccounts,
  withSettingsConnectStatus,
  type CodexSettingsBridge,
  type OpenAgentsSessionSettingsBridge,
} from "./settings.ts"
import {
  desktopShellIntents,
  desktopShellView,
  initialDesktopShellState,
  makeDesktopShellHandlers,
  type DesktopShellState,
} from "./shell.ts"

const { makeIntentRegistry } = await import("@effect-native/core")

type AnyNode = Readonly<Record<string, unknown>>

const collectNodes = (root: unknown): Array<AnyNode> => {
  const found: Array<AnyNode> = []
  const walk = (value: unknown): void => {
    if (Array.isArray(value)) {
      for (const item of value) walk(item)
      return
    }
    if (typeof value !== "object" || value === null) return
    const node = value as AnyNode
    if (typeof node._tag === "string") found.push(node)
    for (const [prop, child] of Object.entries(node)) {
      if (prop === "_tag" || prop === "style" || prop === "a11y") continue
      walk(child)
    }
  }
  walk(root)
  return found
}

const nodeByKey = (view: View, key: string): AnyNode | undefined =>
  collectNodes(view).find((node) => node.key === key)

const baseState: DesktopShellState = initialDesktopShellState("electron/darwin", "18:04")

const loadedAccounts = withSettingsAccounts(initialSettingsState(), {
  state: "loaded",
  accounts: [
    { ref: "codex-2", readiness: "credentials_revoked" },
    { ref: "codex-4", readiness: "ready" },
  ],
})

describe("settingsView (state -> component tree)", () => {
  test("renders honest OpenAgents session phases and typed actions", () => {
    const signedOut = settingsView({
      ...initialSettingsState(),
      openAgentsSession: "signed_out",
    })
    expect(nodeByKey(signedOut, "settings-openagents-session-status")?.label).toBe("Signed out")
    const signIn = nodeByKey(signedOut, "settings-openagents-session-action")
    expect(signIn?.label).toBe("Sign in with GitHub")
    expect((signIn as { onPress?: { name?: string } }).onPress?.name).toBe(
      "DesktopOpenAgentsSignInRequested",
    )

    const ready = settingsView({
      ...initialSettingsState(),
      openAgentsSession: "session_ready",
    })
    expect(nodeByKey(ready, "settings-openagents-session-status")?.label).toBe(
      "OpenAgents session verified",
    )
    const signOut = nodeByKey(ready, "settings-openagents-session-action")
    expect(signOut?.label).toBe("Sign out")
    expect((signOut as { onPress?: { name?: string } }).onPress?.name).toBe(
      "DesktopOpenAgentsSignOutRequested",
    )

    const working = settingsView({
      ...initialSettingsState(),
      openAgentsSession: "authenticating",
    })
    expect(nodeByKey(working, "settings-openagents-session-action")?.disabled).toBe(true)
    expect(nodeByKey(working, "settings-openagents-session-status")?.label).toBe(
      "Waiting for secure browser…",
    )
  })

  test("accounts list renders ref rows with readiness chips; revoked is a warning tone", () => {
    const view = settingsView(loadedAccounts)
    expect(nodeByKey(view, "settings-title")?.content).toBe("Settings")
    expect(nodeByKey(view, "settings-back")?._tag).toBe("Button")

    expect(nodeByKey(view, "settings-account-codex-2-ref")?.content).toBe("codex-2")
    const revoked = nodeByKey(view, "settings-account-codex-2-readiness")
    expect(revoked?._tag).toBe("Badge")
    expect(revoked?.label).toBe("credentials_revoked")
    expect(revoked?.tone).toBe("warn")

    const ready = nodeByKey(view, "settings-account-codex-4-readiness")
    expect(ready?.label).toBe("ready")
    expect(ready?.tone).toBe("success")

    const connect = nodeByKey(view, "settings-connect-codex")
    expect(connect?._tag).toBe("Button")
    expect(connect?.label).toBe("Connect Codex account")
    expect(connect?.variant).toBe("primary")
    expect(connect?.disabled).toBe(false)
  })

  test("loading and unavailable account states render honest placeholders", () => {
    const loading = settingsView(initialSettingsState())
    expect(nodeByKey(loading, "settings-accounts-loading")?.content).toBe(
      "Loading connected accounts…",
    )
    const unavailable = settingsView(
      withSettingsAccounts(initialSettingsState(), {
        state: "unavailable",
        message: "Local Pylon runtime is unavailable. No accounts were read.",
      }),
    )
    expect(nodeByKey(unavailable, "settings-accounts-unavailable")?.content).toContain(
      "unavailable",
    )
    const empty = settingsView(
      withSettingsAccounts(initialSettingsState(), { state: "loaded", accounts: [] }),
    )
    expect(nodeByKey(empty, "settings-accounts-empty")?.content).toBe(
      "No Codex accounts connected yet.",
    )
  })

  test("awaiting_browser shows the clickable link and the user code LARGE (heading)", () => {
    const awaiting = settingsView(
      withSettingsConnectStatus(loadedAccounts, {
        state: "awaiting_browser",
        url: "https://auth.openai.com/codex/device",
        code: "8260-DUG55",
      }),
    )
    const link = awaiting && nodeByKey(awaiting, "settings-connect-link")
    expect(link?._tag).toBe("Button")
    expect(link?.label).toBe("https://auth.openai.com/codex/device")
    expect((link as { onPress?: { name?: string } }).onPress?.name).toBe(
      "DesktopCodexVerificationOpened",
    )
    const code = nodeByKey(awaiting, "settings-connect-code")
    expect(code?._tag).toBe("Text")
    expect(code?.content).toBe("8260-DUG55")
    expect(code?.variant).toBe("heading")
    // the primary button disables while a flow is live
    expect(nodeByKey(awaiting, "settings-connect-codex")?.disabled).toBe(true)
    expect(nodeByKey(awaiting, "settings-connect-codex")?.label).toBe("Connecting…")
  })

  test("connected and failed statuses render typed badges", () => {
    const connected = settingsView(
      withSettingsConnectStatus(loadedAccounts, { state: "connected", ref: "codex-4" }),
    )
    const connectedBadge = nodeByKey(connected, "settings-connect-status")
    expect(connectedBadge?.label).toBe("Connected: codex-4")
    expect(connectedBadge?.tone).toBe("success")

    const failed = settingsView(
      withSettingsConnectStatus(loadedAccounts, { state: "failed", reason: "device_auth_timeout" }),
    )
    const failedBadge = nodeByKey(failed, "settings-connect-status")
    expect(failedBadge?.label).toBe("Connect failed: device_auth_timeout")
    expect(failedBadge?.tone).toBe("warn")
  })

  test("shell swaps to the settings screen and back via the titlebar toggle", () => {
    const chat = desktopShellView(baseState)
    expect(nodeByKey(chat, "shell-settings-toggle")?.label).toBe("Settings")
    expect(nodeByKey(chat, "settings-screen")).toBeUndefined()
    expect(nodeByKey(chat, "shell-input")?._tag).toBe("TextField")

    const settings = desktopShellView({ ...baseState, workspace: "settings" })
    expect(nodeByKey(settings, "shell-title")?.content).toBe("Settings")
    expect(nodeByKey(settings, "shell-settings-toggle")?.label).toBe("Back to chat")
    expect(nodeByKey(settings, "settings-screen")?._tag).toBe("Card")
    expect(nodeByKey(settings, "settings-connect-codex")?._tag).toBe("Button")
    // the chat surface is swapped out, not stacked under
    expect(nodeByKey(settings, "shell-input")).toBeUndefined()
    expect(nodeByKey(settings, "shell-transcript")).toBeUndefined()
  })
})

describe("bridge payload decoding (renderer side)", () => {
  test("decodes only bounded Runtime Gateway session phases", () => {
    expect(decodeOpenAgentsSessionView({
      kind: "query_result",
      requestId: "status",
      result: { kind: "runtime.bootstrap", sessionPhase: "session_ready" },
    })).toBe("session_ready")
    expect(decodeOpenAgentsSessionView({
      kind: "session_outcome",
      commandId: "sign-out",
      status: "completed",
      phase: "signed_out",
    })).toBe("signed_out")
    expect(decodeOpenAgentsSessionView({
      kind: "session_outcome",
      commandId: "bad",
      status: "completed",
      phase: "live",
      accessToken: "forbidden",
    })).toBe("unavailable")
  })

  test("accounts decode: ok, unavailable, and garbage", () => {
    expect(
      decodeAccountsView({
        state: "ok",
        accounts: [{ ref: "codex-2", readiness: "credentials_revoked" }],
      }),
    ).toEqual({
      state: "loaded",
      accounts: [{ ref: "codex-2", readiness: "credentials_revoked" }],
    })
    expect(decodeAccountsView({ state: "unavailable", message: "down" })).toEqual({
      state: "unavailable",
      message: "down",
    })
    expect(decodeAccountsView(null).state).toBe("unavailable")
    expect(decodeAccountsView({ state: "ok", accounts: [{ ref: "../evil", readiness: "x" }] }))
      .toEqual({ state: "loaded", accounts: [] })
  })

  test("connect status decode rejects non-https URLs and malformed codes/refs", () => {
    expect(
      decodeConnectStatusView({
        state: "awaiting_browser",
        url: "https://auth.openai.com/codex/device",
        code: "8260-DUG55",
      }).state,
    ).toBe("awaiting_browser")
    expect(
      decodeConnectStatusView({ state: "awaiting_browser", url: "http://evil", code: "8260-DUG55" }),
    ).toEqual({ state: "failed", reason: "invalid_verification_url" })
    expect(
      decodeConnectStatusView({
        state: "awaiting_browser",
        url: "https://auth.openai.com/codex/device",
        code: "lowercase-code",
      }),
    ).toEqual({ state: "failed", reason: "invalid_user_code" })
    expect(decodeConnectStatusView({ state: "connected", ref: "not a ref!" })).toEqual({
      state: "failed",
      reason: "invalid_account_ref",
    })
    expect(decodeConnectStatusView(undefined)).toEqual({
      state: "failed",
      reason: "invalid_bridge_payload",
    })
  })

  test("connectStatusIsLive marks only starting/awaiting_browser as live", () => {
    expect(connectStatusIsLive({ state: "starting" })).toBe(true)
    expect(
      connectStatusIsLive({ state: "awaiting_browser", url: "https://x", code: "1111-AAAA" }),
    ).toBe(true)
    expect(connectStatusIsLive({ state: "idle" })).toBe(false)
    expect(connectStatusIsLive({ state: "connected", ref: "codex-4" })).toBe(false)
    expect(connectStatusIsLive({ state: "failed", reason: "x" })).toBe(false)
  })
})

describe("typed intent loop end-to-end (settings)", () => {
  const makeBridge = (
    statuses: Array<unknown>,
    opened: Array<string>,
  ): CodexSettingsBridge => ({
    listAccounts: async () => ({
      state: "ok",
      accounts: [{ ref: "codex-2", readiness: "credentials_revoked" }],
    }),
    connectStart: async () => ({ state: "starting" }),
    connectStatus: async () => statuses.shift() ?? { state: "failed", reason: "exhausted" },
    openVerification: async () => {
      opened.push("opened")
      return true
    },
  })

  test("Settings toggle loads accounts; connect polls through awaiting_browser to connected", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const opened: Array<string> = []
        const statuses: Array<unknown> = [
          { state: "awaiting_browser", url: "https://auth.openai.com/codex/device", code: "8260-DUG55" },
          { state: "connected", ref: "codex-4" },
        ]
        const state = yield* SubscriptionRef.make(baseState)
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(
            state,
            () => "18:05",
            undefined,
            undefined,
            undefined,
            makeBridge(statuses, opened),
            async () => {}, // no real sleeping in tests
          ),
        )

        // Open Settings via the SAME IntentRef the titlebar button carries.
        const chatView = desktopShellView(yield* SubscriptionRef.get(state))
        const toggle = nodeByKey(chatView, "shell-settings-toggle") as {
          onPress: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(toggle.onPress, null))
        const afterOpen = yield* SubscriptionRef.get(state)
        expect(afterOpen.workspace).toBe("settings")
        expect(afterOpen.settings.accounts).toEqual({
          state: "loaded",
          accounts: [{ ref: "codex-2", readiness: "credentials_revoked" }],
        })

        // Connect: the handler polls status until terminal.
        const settingsScreen = desktopShellView(afterOpen)
        const connect = nodeByKey(settingsScreen, "settings-connect-codex") as {
          onPress: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(connect.onPress, null))
        const afterConnect = yield* SubscriptionRef.get(state)
        expect(afterConnect.settings.connect).toEqual({ state: "connected", ref: "codex-4" })
        expect(statuses.length).toBe(0)

        // Back to chat.
        const back = nodeByKey(desktopShellView(afterConnect), "settings-back") as {
          onPress: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(back.onPress, null))
        expect((yield* SubscriptionRef.get(state)).workspace).toBe("chat")
      }),
    )
  })

  test("verification link press asks main to open the URL it holds (no URL crosses)", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const opened: Array<string> = []
        const awaitingState: DesktopShellState = {
          ...baseState,
          workspace: "settings",
          settings: withSettingsConnectStatus(initialSettingsState(), {
            state: "awaiting_browser",
            url: "https://auth.openai.com/codex/device",
            code: "8260-DUG55",
          }),
        }
        const state = yield* SubscriptionRef.make(awaitingState)
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(state, () => "18:05", undefined, undefined, undefined, makeBridge([], opened)),
        )
        const view = desktopShellView(yield* SubscriptionRef.get(state))
        const link = nodeByKey(view, "settings-connect-link") as {
          onPress: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(link.onPress, null))
        expect(opened).toEqual(["opened"])
      }),
    )
  })

  test("connect failure surfaces as a typed failed status", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const settingsScreenState: DesktopShellState = { ...baseState, workspace: "settings" }
        const state = yield* SubscriptionRef.make(settingsScreenState)
        const failingBridge: CodexSettingsBridge = {
          listAccounts: async () => ({ state: "unavailable", message: "down" }),
          connectStart: async () => ({ state: "failed", reason: "pylon_runtime_unavailable" }),
          connectStatus: async () => ({ state: "failed", reason: "pylon_runtime_unavailable" }),
          openVerification: async () => false,
        }
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(state, () => "18:05", undefined, undefined, undefined, failingBridge, async () => {}),
        )
        yield* registry.dispatch(
          resolveIntentRef(
            (nodeByKey(
              desktopShellView(yield* SubscriptionRef.get(state)),
              "settings-connect-codex",
            ) as { onPress: Parameters<typeof resolveIntentRef>[0] }).onPress,
            null,
          ),
        )
        const next = yield* SubscriptionRef.get(state)
        expect(next.settings.connect).toEqual({
          state: "failed",
          reason: "pylon_runtime_unavailable",
        })
      }),
    )
  })

  // Oracle for openagents_desktop.session.effect_native_controls.v1.
  test("OpenAgents sign-in and sign-out use typed tokenless Runtime Gateway intents", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const calls: Array<string> = []
        const sessionBridge: OpenAgentsSessionSettingsBridge = {
          status: async () => ({
            kind: "query_result",
            requestId: "status",
            result: { kind: "runtime.bootstrap", sessionPhase: "signed_out" },
          }),
          signIn: async () => {
            calls.push("sign-in")
            return {
              kind: "session_outcome",
              commandId: "sign-in",
              status: "completed",
              phase: "session_ready",
            }
          },
          signOut: async () => {
            calls.push("sign-out")
            return {
              kind: "session_outcome",
              commandId: "sign-out",
              status: "completed",
              phase: "signed_out",
            }
          },
        }
        const state = yield* SubscriptionRef.make<DesktopShellState>({
          ...baseState,
          workspace: "settings",
          settings: { ...initialSettingsState(), openAgentsSession: "signed_out" },
        })
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(
            state,
            () => "18:05",
            undefined,
            undefined,
            undefined,
            makeBridge([], []),
            async () => {},
            sessionBridge,
          ),
        )

        const signIn = nodeByKey(
          desktopShellView(yield* SubscriptionRef.get(state)),
          "settings-openagents-session-action",
        ) as { onPress: Parameters<typeof resolveIntentRef>[0] }
        yield* registry.dispatch(resolveIntentRef(signIn.onPress, null))
        expect((yield* SubscriptionRef.get(state)).settings.openAgentsSession).toBe("session_ready")

        const signOut = nodeByKey(
          desktopShellView(yield* SubscriptionRef.get(state)),
          "settings-openagents-session-action",
        ) as { onPress: Parameters<typeof resolveIntentRef>[0] }
        yield* registry.dispatch(resolveIntentRef(signOut.onPress, null))
        expect((yield* SubscriptionRef.get(state)).settings.openAgentsSession).toBe("signed_out")
        expect(calls).toEqual(["sign-in", "sign-out"])
      }),
    )
  })
})
