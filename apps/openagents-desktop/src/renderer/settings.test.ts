/**
 * Settings view-program tests (#8574, #8640 unblock): pure state -> view for
 * the Codex reconnect screen, plus the full typed intent loop with a fake
 * bridge — open Settings, load accounts, start connect, poll to connected.
 */
import { describe, expect, test } from "bun:test"
import { resolveIntentRef, type View } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"

import {
  buildMcpConfigFromDraft,
  connectStatusIsLive,
  decodeAccountsView,
  decodeClaudeAccountsView,
  decodeConnectStatusView,
  decodeOpenAgentsSessionView,
  emptyMcpAddDraft,
  initialMcpSettingsState,
  initialSettingsState,
  makeSettingsHandlers,
  parseMcpArgs,
  parseMcpKeyValueLines,
  settingsExtensionAudit,
  settingsView,
  withSettingsAccounts,
  withSettingsClaudeAccounts,
  withSettingsConnectStatus,
  type CodexSettingsBridge,
  type McpConfigSettingsBridge,
  type McpSettingsState,
  type OpenAgentsSessionSettingsBridge,
  type ProviderAccountsSettingsBridge,
  type SettingsState,
} from "./settings.ts"
import type { McpConfigServerView } from "../mcp-config-contract.ts"
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

const navItemById = (view: View, id: string): AnyNode | undefined => {
  const nav = nodeByKey(view, "sidebar-navigation")
  const sections = nav?.sections as ReadonlyArray<{ items?: ReadonlyArray<AnyNode> }> | undefined
  return sections?.flatMap((section) => section.items ?? []).find((item) => item.id === id)
}

const baseState: DesktopShellState = initialDesktopShellState("electron/darwin", "18:04")

const loadedAccounts = withSettingsAccounts(initialSettingsState(), {
  state: "loaded",
  accounts: [
    { ref: "codex-2", readiness: "credentials_revoked" },
    { ref: "codex-4", readiness: "ready" },
  ],
})

describe("settingsView (state -> component tree)", () => {
  test("local plugins expose opaque lifecycle state and typed controls", () => {
    const ref = "plugin.local.0123456789abcdef01234567" as const
    const view = settingsView({
      ...initialSettingsState(),
      plugins: {
        state: "loaded",
        dropped: 0,
        message: null,
        plugins: [{
          ref, name: "review-tools", provider: "claude_agent", provenance: "user_local",
          scope: "app", readiness: "ready", enabled: true, restartRequired: false,
          perSessionUse: "next_turn", capabilities: ["commands", "skills"], skills: ["review"],
        }],
      },
    })
    expect(nodeByKey(view, `settings-plugin-${ref}-name`)?.content).toBe("review-tools")
    expect(nodeByKey(view, `settings-plugin-${ref}-status`)?.label).toBe("ready")
    expect((nodeByKey(view, `settings-plugin-${ref}-toggle`)?.onChange as { name?: string })?.name)
      .toBe("DesktopPluginToggleRequested")
    expect((nodeByKey(view, "settings-plugin-add")?.onPress as { name?: string })?.name)
      .toBe("DesktopPluginChooseRequested")
    expect(JSON.stringify(view)).not.toContain("/Users/")
  })
  test("extension lifecycle audit unifies MCP/plugin/skill grants over loaded state (CUT-23)", () => {
    const ref = "plugin.local.0123456789abcdef01234567" as const
    const view = settingsView({
      ...initialSettingsState(),
      mcp: {
        ...initialMcpSettingsState(),
        servers: {
          state: "loaded",
          dropped: 1,
          servers: [{
            name: "search", transport: "stdio", enabled: false, command: "search-mcp",
            argsCount: 0, envCount: 2, headersCount: 0,
          }],
        },
      },
      plugins: {
        state: "loaded",
        dropped: 0,
        message: null,
        plugins: [{
          ref, name: "review-tools", provider: "claude_agent", provenance: "user_local",
          scope: "app", readiness: "ready", enabled: true, restartRequired: false,
          perSessionUse: "next_turn", capabilities: ["skills"], skills: ["review"],
        }],
      },
    })
    // Summary tallies: plugin + its skill granted; the disabled server revoked;
    // the host-dropped invalid row surfaces honestly.
    expect(nodeByKey(view, "settings-lifecycle-summary")?.content).toBe(
      "2 granted · 1 revoked · 1 invalid dropped",
    )
    expect(nodeByKey(view, "settings-lifecycle-partial")).toBeUndefined()
    // Revoked MCP server row: stage badge + withdrawn grant caption.
    expect(nodeByKey(view, "settings-lifecycle-mcp_server-search-stage")?.label).toBe("revoked")
    expect(nodeByKey(view, "settings-lifecycle-mcp_server-search-grant")?.content).toBe("grant revoked")
    // Skill grant is scoped under its parent plugin and requires explicit /skill use.
    expect(nodeByKey(view, `settings-lifecycle-skill-${ref}/review-stage`)?.label).toBe("granted")
    expect(nodeByKey(view, `settings-lifecycle-skill-${ref}/review-grant`)?.content).toBe(
      "granted · explicit /skill",
    )
    // Provider disagreement is explicit on every row.
    expect(nodeByKey(view, `settings-lifecycle-plugin-${ref}-provider`)?.content).toBe("Claude only")
    // The audit never leaks secret-bearing fields or absolute paths.
    expect(JSON.stringify(view)).not.toContain("/Users/")
  })
  test("extension lifecycle audit is honestly partial while a registry is unavailable", () => {
    const view = settingsView(initialSettingsState())
    expect(nodeByKey(view, "settings-lifecycle-partial")?.content).toBe(
      "Some registries are still loading or unavailable — this audit is partial.",
    )
    expect(nodeByKey(view, "settings-lifecycle-empty")).toBeUndefined()
    const audit = settingsExtensionAudit(initialSettingsState())
    expect(audit.partial).toBe(true)
    expect(audit.entries).toEqual([])
  })
  test("renders honest OpenAgents session phases and typed actions", () => {
    const signedOut = settingsView({
      ...initialSettingsState(),
      openAgentsSession: "signed_out",
    })
    expect(nodeByKey(signedOut, "settings-openagents-session-status")?.label).toBe("Local device ready")
    const signIn = nodeByKey(signedOut, "settings-openagents-session-action")
    expect(signIn?.label).toBe("Link OpenAgents account")
    expect((signIn as { onPress?: { name?: string } }).onPress?.name).toBe(
      "DesktopOpenAgentsSignInRequested",
    )

    const ready = settingsView({
      ...initialSettingsState(),
      openAgentsSession: "session_ready",
    })
    expect(nodeByKey(ready, "settings-openagents-session-status")?.label).toBe(
      "OpenAgents account linked",
    )
    const signOut = nodeByKey(ready, "settings-openagents-session-action")
    expect(signOut?.label).toBe("Disconnect account")
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

  test("MVP shows the ordinary Codex session and no Pylon account-linking surface", () => {
    const view = settingsView(loadedAccounts)
    expect(nodeByKey(view, "settings-title")?.content).toBe("Settings")
    expect(nodeByKey(view, "settings-back")?._tag).toBe("Button")
    expect(nodeByKey(view, "settings-codex-session-title")?.content).toBe("Codex session")
    expect(nodeByKey(view, "settings-codex-session-copy")?.content).toContain("already signed in on this Mac")
    expect(nodeByKey(view, "settings-connect-codex")).toBeUndefined()
    expect(nodeByKey(view, "settings-accounts-title")).toBeUndefined()
    expect(nodeByKey(view, "settings-account-codex-2-ref")).toBeUndefined()
    expect(nodeByKey(view, "settings-claude-accounts-title")).toBeUndefined()
  })

  test.skip("retired Pylon account placeholders", () => {
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

  test.skip("retired Pylon Claude account rows", () => {
    const view = settingsView(
      withSettingsClaudeAccounts(initialSettingsState(), {
        state: "loaded",
        accounts: [
          { ref: "claude", readiness: "ready" },
          { ref: "claude-2", readiness: "credentials-missing" },
        ],
      }),
    )
    expect(nodeByKey(view, "settings-claude-accounts-title")?.content).toBe("Claude accounts")
    expect(nodeByKey(view, "settings-claude-account-claude-ref")?.content).toBe("claude")
    const ready = nodeByKey(view, "settings-claude-account-claude-readiness")
    expect(ready?._tag).toBe("Badge")
    expect(ready?.label).toBe("ready")
    expect(ready?.tone).toBe("success")
    expect(nodeByKey(view, "settings-claude-account-claude-2-readiness")?.label).toBe("credentials-missing")
    expect(collectNodes(view).filter(node => node._tag === "Button" && typeof node.key === "string" && node.key.startsWith("settings-claude"))).toEqual([])
  })

  test.skip("retired Pylon Claude placeholder states", () => {
    const empty = settingsView(
      withSettingsClaudeAccounts(initialSettingsState(), { state: "loaded", accounts: [] }),
    )
    expect(nodeByKey(empty, "settings-claude-accounts-empty")?.content).toBe(
      "No Claude accounts linked on this machine.",
    )
    const unavailable = settingsView(
      withSettingsClaudeAccounts(initialSettingsState(), {
        state: "unavailable",
        message: "Claude account listing is unavailable on this build.",
      }),
    )
    expect(nodeByKey(unavailable, "settings-claude-accounts-unavailable")?.content).toContain(
      "unavailable",
    )
    expect(nodeByKey(settingsView(initialSettingsState()), "settings-claude-accounts-loading")?.content).toBe(
      "Loading Claude accounts…",
    )
  })

  test.skip("retired isolated device-auth presentation", () => {
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

  test.skip("retired isolated connect status badges", () => {
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

  test("shell swaps to the settings screen and back via the sidebar icon", () => {
    const chat = desktopShellView(baseState)
    expect(navItemById(chat, "shell-settings-toggle")?.accessibilityLabel).toBe("Open Settings")
    expect(nodeByKey(chat, "settings-screen")).toBeUndefined()
    expect(nodeByKey(chat, "shell-input")?._tag).toBe("TextField")

    const settings = desktopShellView({ ...baseState, workspace: "settings" })
    expect(nodeByKey(settings, "shell-title")).toBeUndefined()
    expect(navItemById(settings, "shell-settings-toggle")?.accessibilityLabel).toBe("Close Settings")
    expect(nodeByKey(settings, "settings-screen")?._tag).toBe("Card")
    expect(nodeByKey(settings, "settings-codex-session-copy")?.content).toContain("already signed in")
    expect(nodeByKey(settings, "settings-connect-codex")).toBeUndefined()
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

  test("claude accounts decode: keeps only bounded claude_agent entries, degrades to unavailable", () => {
    expect(decodeClaudeAccountsView({
      generatedAt: "2026-07-11T12:00:00.000Z",
      accounts: [
        { ref: "claude", provider: "claude_agent", email: "owner@example.com", readiness: "ready" },
        { ref: "codex-2", provider: "codex", email: null, readiness: "ready" },
        { ref: "../evil", provider: "claude_agent", readiness: "ready" },
      ],
    })).toEqual({
      state: "loaded",
      accounts: [{ ref: "claude", readiness: "ready" }],
    })
    expect(decodeClaudeAccountsView(null)).toEqual({
      state: "unavailable",
      message: "Claude account listing is unavailable on this build.",
    })
    expect(decodeClaudeAccountsView({ accounts: "garbage" }).state).toBe("unavailable")
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

  test.skip("retired Pylon connect flow", async () => {
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

        // Open Settings via the SAME IntentRef the sidebar icon carries.
        const chatView = desktopShellView(yield* SubscriptionRef.get(state))
        const toggle = navItemById(chatView, "shell-settings-toggle") as {
          onSelect: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(toggle.onSelect, null))
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

  test.skip("retired Pylon Claude account listing", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const providerBridge: ProviderAccountsSettingsBridge = {
          list: async () => ({
            generatedAt: "2026-07-11T12:00:00.000Z",
            accounts: [
              { ref: "claude", provider: "claude_agent", email: "owner@example.com", readiness: "ready" },
              { ref: "codex-2", provider: "codex", email: null, readiness: "ready" },
            ],
          }),
        }
        const state = yield* SubscriptionRef.make(baseState)
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
            undefined,
            undefined,
            undefined,
            providerBridge,
          ),
        )
        const toggle = navItemById(desktopShellView(yield* SubscriptionRef.get(state)), "shell-settings-toggle") as {
          onSelect: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(toggle.onSelect, null))
        const after = yield* SubscriptionRef.get(state)
        expect(after.settings.claudeAccounts).toEqual({
          state: "loaded",
          accounts: [{ ref: "claude", readiness: "ready" }],
        })
        const view = desktopShellView(after)
        expect(nodeByKey(view, "settings-claude-account-claude-readiness")?.label).toBe("ready")
      }),
    )
  })

  test.skip("retired Pylon Claude unavailable state", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make(baseState)
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
          ),
        )
        const toggle = navItemById(desktopShellView(yield* SubscriptionRef.get(state)), "shell-settings-toggle") as {
          onSelect: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(toggle.onSelect, null))
        const after = yield* SubscriptionRef.get(state)
        expect(after.settings.claudeAccounts).toEqual({
          state: "unavailable",
          message: "Claude account listing is unavailable on this build.",
        })
        expect(nodeByKey(desktopShellView(after), "settings-claude-accounts-unavailable")?.content).toContain("unavailable")
      }),
    )
  })

  test.skip("retired isolated verification link", async () => {
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

  test.skip("retired isolated connect failure", async () => {
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

// ---------------------------------------------------------------------------
// EP250 owner mandate: "don't recommend me the CLI command for Fleet Connect.
// We're doing stuff with the UI now. Like, CLI stuff is nice, but the UI
// controls need to be working." — per-account Reconnect in Settings, no CLI
// copy anywhere on the screen.
// ---------------------------------------------------------------------------

describe.skip("retired per-account Pylon reconnect surface", () => {
  test("credential-failed rows render a working Reconnect button; ready rows render none", () => {
    const view = settingsView(loadedAccounts)
    const reconnect = nodeByKey(view, "settings-account-codex-2-reconnect")
    expect(reconnect?._tag).toBe("Button")
    expect(reconnect?.label).toBe("Reconnect")
    expect(reconnect?.disabled).toBe(false)
    const press = (reconnect as { onPress?: { name?: string } }).onPress
    expect(press?.name).toBe("DesktopCodexReconnectRequested")
    // The payload carries the account ref.
    expect(JSON.stringify(press)).toContain("codex-2")
    // A clean ready account gets no reconnect affordance.
    expect(nodeByKey(view, "settings-account-codex-4-reconnect")).toBeUndefined()
  })

  test("reconnect buttons disable while a flow is live", () => {
    const live = settingsView(
      withSettingsConnectStatus(loadedAccounts, { state: "starting" }),
    )
    const reconnect = nodeByKey(live, "settings-account-codex-2-reconnect")
    expect(reconnect?.disabled).toBe(true)
    expect(reconnect?.label).toBe("Waiting…")
  })

  test("awaiting_browser during a reconnect names the target ref beside the URL and code", () => {
    const awaiting = settingsView({
      ...withSettingsConnectStatus(loadedAccounts, {
        state: "awaiting_browser",
        url: "https://auth.openai.com/codex/device",
        code: "8260-DUG55",
      }),
      connectTarget: "codex-2",
    })
    expect(nodeByKey(awaiting, "settings-connect-status")?.content).toBe(
      "Reconnecting codex-2 — open this link in your browser and enter the code below:",
    )
    expect(nodeByKey(awaiting, "settings-connect-code")?.content).toBe("8260-DUG55")
    expect(nodeByKey(awaiting, "settings-connect-link")?.label).toBe(
      "https://auth.openai.com/codex/device",
    )
  })

  test("a completed reconnect renders the Reconnected badge for the SAME ref", () => {
    const done = settingsView({
      ...withSettingsConnectStatus(loadedAccounts, { state: "connected", ref: "codex-2" }),
      connectTarget: "codex-2",
    })
    expect(nodeByKey(done, "settings-connect-status")?.label).toBe("Reconnected: codex-2")
  })

  test("NO CLI COPY: no settings state ever renders text instructing a CLI command", () => {
    const cliCopy = /pylon auth|khala fleet|npm install|bun apps|--force-device-login|codex login|run the command|\bCLI\b/i
    const states = [
      loadedAccounts,
      withSettingsConnectStatus(loadedAccounts, { state: "starting" }),
      {
        ...withSettingsConnectStatus(loadedAccounts, {
          state: "awaiting_browser",
          url: "https://auth.openai.com/codex/device",
          code: "8260-DUG55",
        }),
        connectTarget: "codex-2",
      },
      withSettingsConnectStatus(loadedAccounts, { state: "connected", ref: "codex-2" }),
      withSettingsConnectStatus(loadedAccounts, { state: "failed", reason: "device_auth_timeout" }),
      initialSettingsState(),
    ]
    for (const settings of states) {
      for (const node of collectNodes(settingsView(settings))) {
        for (const value of [node.content, node.label]) {
          if (typeof value === "string") {
            expect(value).not.toMatch(cliCopy)
          }
        }
      }
    }
  })
})

describe.skip("retired Pylon reconnect intent loop", () => {
  test("Reconnect dispatch calls the ref-targeted bridge, polls to connected, and re-lists accounts so readiness flips without restart", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const reconnectCalls: Array<string> = []
        let listCalls = 0
        const statuses: Array<unknown> = [
          { state: "awaiting_browser", url: "https://auth.openai.com/codex/device", code: "8260-DUG55" },
          { state: "connected", ref: "codex-2" },
        ]
        const bridge: CodexSettingsBridge = {
          listAccounts: async () => {
            listCalls += 1
            return {
              state: "ok",
              // After the reconnect completes, the registry read flips the
              // readiness — the UI must show it without an app restart.
              accounts: [{
                ref: "codex-2",
                readiness: listCalls >= 2 ? "ready" : "credentials_revoked",
              }],
            }
          },
          connectStart: async () => ({ state: "failed", reason: "wrong_channel" }),
          reconnectStart: async (ref) => {
            reconnectCalls.push(ref)
            return { state: "starting" }
          },
          connectStatus: async () => statuses.shift() ?? { state: "failed", reason: "exhausted" },
          openVerification: async () => true,
        }
        const state = yield* SubscriptionRef.make(baseState)
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(
            state,
            () => "18:05",
            undefined,
            undefined,
            undefined,
            bridge,
            async () => {},
          ),
        )

        // Open Settings (loads the revoked account), then press Reconnect.
        const toggle = navItemById(
          desktopShellView(yield* SubscriptionRef.get(state)),
          "shell-settings-toggle",
        ) as { onSelect: Parameters<typeof resolveIntentRef>[0] }
        yield* registry.dispatch(resolveIntentRef(toggle.onSelect, null))
        const opened = yield* SubscriptionRef.get(state)
        const reconnect = nodeByKey(
          desktopShellView(opened),
          "settings-account-codex-2-reconnect",
        ) as { onPress: Parameters<typeof resolveIntentRef>[0] }
        yield* registry.dispatch(resolveIntentRef(reconnect.onPress, null))

        const after = yield* SubscriptionRef.get(state)
        expect(reconnectCalls).toEqual(["codex-2"])
        expect(after.settings.connect).toEqual({ state: "connected", ref: "codex-2" })
        expect(after.settings.connectTarget).toBe("codex-2")
        // Accounts were re-listed after the terminal status: readiness flips.
        expect(after.settings.accounts).toEqual({
          state: "loaded",
          accounts: [{ ref: "codex-2", readiness: "ready" }],
        })
        expect(listCalls).toBe(2)
      }),
    )
  })

  test("accounts re-list even when the flow FAILS (a failed exit can still have registered a new ref)", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        let listCalls = 0
        const bridge: CodexSettingsBridge = {
          listAccounts: async () => {
            listCalls += 1
            return {
              state: "ok",
              accounts: listCalls >= 2
                // The codex-5 case: the CLI exited 1 AFTER registering a new
                // valid account; the re-list must surface it regardless.
                ? [
                    { ref: "codex-2", readiness: "credentials_revoked" },
                    { ref: "codex-5", readiness: "ready" },
                  ]
                : [{ ref: "codex-2", readiness: "credentials_revoked" }],
            }
          },
          connectStart: async () => ({
            state: "failed",
            reason: "pylon_auth_failed: Unable to connect.",
          }),
          reconnectStart: async () => ({ state: "failed", reason: "unused" }),
          connectStatus: async () => ({ state: "failed", reason: "unused" }),
          openVerification: async () => true,
        }
        const state = yield* SubscriptionRef.make(baseState)
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(
            state,
            () => "18:05",
            undefined,
            undefined,
            undefined,
            bridge,
            async () => {},
          ),
        )
        const toggle = navItemById(
          desktopShellView(yield* SubscriptionRef.get(state)),
          "shell-settings-toggle",
        ) as { onSelect: Parameters<typeof resolveIntentRef>[0] }
        yield* registry.dispatch(resolveIntentRef(toggle.onSelect, null))
        const connect = nodeByKey(
          desktopShellView(yield* SubscriptionRef.get(state)),
          "settings-connect-codex",
        ) as { onPress: Parameters<typeof resolveIntentRef>[0] }
        yield* registry.dispatch(resolveIntentRef(connect.onPress, null))

        const after = yield* SubscriptionRef.get(state)
        expect(after.settings.connect.state).toBe("failed")
        // The bounded public-safe failure detail reaches the badge state.
        expect((after.settings.connect as { reason?: string }).reason).toBe(
          "pylon_auth_failed: Unable to connect.",
        )
        // The re-list surfaced the half-succeeded registration anyway.
        expect(after.settings.accounts).toEqual({
          state: "loaded",
          accounts: [
            { ref: "codex-2", readiness: "credentials_revoked" },
            { ref: "codex-5", readiness: "ready" },
          ],
        })
        expect(listCalls).toBe(2)
      }),
    )
  })

  test("a reconnect for a ref the renderer is not displaying is refused before the bridge", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const reconnectCalls: Array<string> = []
        const bridge: CodexSettingsBridge = {
          listAccounts: async () => ({
            state: "ok",
            accounts: [{ ref: "codex-2", readiness: "credentials_revoked" }],
          }),
          connectStart: async () => ({ state: "failed", reason: "unused" }),
          reconnectStart: async (ref) => {
            reconnectCalls.push(ref)
            return { state: "starting" }
          },
          connectStatus: async () => ({ state: "failed", reason: "unused" }),
          openVerification: async () => true,
        }
        const state = yield* SubscriptionRef.make(baseState)
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(
            state, () => "18:05", undefined, undefined, undefined, bridge, async () => {},
          ),
        )
        const toggle = navItemById(
          desktopShellView(yield* SubscriptionRef.get(state)),
          "shell-settings-toggle",
        ) as { onSelect: Parameters<typeof resolveIntentRef>[0] }
        yield* registry.dispatch(resolveIntentRef(toggle.onSelect, null))
        yield* registry.dispatch({ name: "DesktopCodexReconnectRequested", payload: "codex-999" })
        expect(reconnectCalls).toEqual([])
        expect((yield* SubscriptionRef.get(state)).settings.connect).toEqual({ state: "idle" })
      }),
    )
  })
})

// ---------------------------------------------------------------------------
// User-configured MCP servers (I2, EP250 wave-2). The UI oracle for behavior
// contract openagents_desktop.settings.mcp_servers.v1: list render + status
// chip, transport-specific Add form with client-side validation (happy + sad),
// and the toggle/remove/add dispatch loop through a fake bridge.
// ---------------------------------------------------------------------------

const loadedMcp = (
  servers: ReadonlyArray<McpConfigServerView>,
  over: Partial<McpSettingsState> = {},
): SettingsState => ({
  ...initialSettingsState(),
  mcp: {
    ...initialMcpSettingsState(),
    servers: { state: "loaded", servers, dropped: 0 },
    ...over,
  },
})

const stdioView = (over: Partial<McpConfigServerView> = {}): McpConfigServerView => ({
  name: "docs",
  transport: "stdio",
  enabled: true,
  command: "docs-mcp",
  argsCount: 0,
  envCount: 0,
  headersCount: 0,
  ...over,
})

describe("MCP servers — pure client-side validation", () => {
  test("parseMcpArgs splits on whitespace/newlines, drops empties, caps at 64", () => {
    expect(parseMcpArgs("--root /x\n--verbose")).toEqual(["--root", "/x", "--verbose"])
    expect(parseMcpArgs("   ")).toEqual([])
    expect(parseMcpArgs(Array.from({ length: 100 }, (_, i) => `a${i}`).join(" ")).length).toBe(64)
  })

  test("parseMcpKeyValueLines keeps later separators in the value; drops keyless lines", () => {
    expect(parseMcpKeyValueLines("TOKEN=a=b\nEMPTY", "=")).toEqual({ TOKEN: "a=b" })
    expect(parseMcpKeyValueLines("Authorization: Bearer x", ":")).toEqual({ Authorization: "Bearer x" })
  })

  test("buildMcpConfigFromDraft — happy stdio path yields a frozen config", () => {
    const draft = { ...emptyMcpAddDraft(), name: "docs", command: "docs-mcp", argsText: "--root /x", envText: "TOKEN=secret" }
    const built = buildMcpConfigFromDraft(draft, [])
    expect(built.ok).toBe(true)
    if (built.ok) {
      expect(built.config).toEqual({
        name: "docs", transport: "stdio", enabled: true, command: "docs-mcp",
        args: ["--root", "/x"], env: { TOKEN: "secret" },
      })
    }
  })

  test("buildMcpConfigFromDraft — happy http path yields a frozen config", () => {
    const draft = { ...emptyMcpAddDraft(), transport: "http" as const, name: "remote", url: "https://example.test/mcp", headersText: "Authorization: Bearer x" }
    const built = buildMcpConfigFromDraft(draft, [])
    expect(built.ok).toBe(true)
    if (built.ok) {
      expect(built.config).toEqual({
        name: "remote", transport: "http", enabled: true, url: "https://example.test/mcp",
        headers: { Authorization: "Bearer x" },
      })
    }
  })

  test("buildMcpConfigFromDraft — sad paths: empty/invalid/reserved/duplicate name, missing transport field", () => {
    const base = emptyMcpAddDraft()
    expect(buildMcpConfigFromDraft({ ...base, name: "" }, []).ok).toBe(false)
    expect(buildMcpConfigFromDraft({ ...base, name: "bad name!" }, []).ok).toBe(false)
    expect(buildMcpConfigFromDraft({ ...base, name: "codex", command: "x" }, []).ok).toBe(false)
    expect(buildMcpConfigFromDraft({ ...base, name: "docs", command: "x" }, ["docs"]).ok).toBe(false)
    expect(buildMcpConfigFromDraft({ ...base, name: "docs" }, []).ok).toBe(false) // stdio, no command
    expect(buildMcpConfigFromDraft({ ...base, transport: "http", name: "r", url: "ftp://x" }, []).ok).toBe(false)
  })
})

describe("MCP servers — view rendering", () => {
  test("loaded servers render name, transport badge, enabled toggle, and a Remove button", () => {
    const view = settingsView(loadedMcp([
      stdioView({ name: "docs", command: "docs-mcp" }),
      { name: "remote", transport: "http", enabled: false, url: "https://x.test", argsCount: 0, envCount: 0, headersCount: 1 },
    ]))
    expect(nodeByKey(view, "settings-mcp-server-docs-name")?.content).toBe("docs")
    expect(nodeByKey(view, "settings-mcp-server-docs-transport")?.label).toBe("stdio")
    const toggle = nodeByKey(view, "settings-mcp-server-docs-toggle")
    expect(toggle?._tag).toBe("Toggle")
    expect(toggle?.value).toBe(true)
    expect(nodeByKey(view, "settings-mcp-server-remote-toggle")?.value).toBe(false)
    expect(nodeByKey(view, "settings-mcp-server-remote-remove")?._tag).toBe("Button")
  })

  test("a runtime-reported unavailable status renders a warn chip", () => {
    const view = settingsView(loadedMcp([stdioView({ name: "docs" })], { status: { docs: "failed: needs auth" } }))
    const chip = nodeByKey(view, "settings-mcp-server-docs-status")
    expect(chip?.label).toBe("unavailable")
    expect(chip?.tone).toBe("warn")
  })

  test("empty and unavailable server states render honest placeholders", () => {
    expect(nodeByKey(settingsView(loadedMcp([])), "settings-mcp-empty")).toBeDefined()
    const unavailable = settingsView({
      ...initialSettingsState(),
      mcp: { ...initialMcpSettingsState(), servers: { state: "unavailable", message: "nope" } },
    })
    expect(nodeByKey(unavailable, "settings-mcp-unavailable")?.content).toBe("nope")
  })

  test("Add form shows stdio fields by default and http fields when transport is http", () => {
    const stdioForm = settingsView(initialSettingsState())
    expect(nodeByKey(stdioForm, "settings-mcp-field-command")).toBeDefined()
    expect(nodeByKey(stdioForm, "settings-mcp-field-url")).toBeUndefined()
    const httpForm = settingsView({
      ...initialSettingsState(),
      mcp: { ...initialMcpSettingsState(), draft: { ...emptyMcpAddDraft(), transport: "http" } },
    })
    expect(nodeByKey(httpForm, "settings-mcp-field-url")).toBeDefined()
    expect(nodeByKey(httpForm, "settings-mcp-field-command")).toBeUndefined()
  })

  test("the transport RadioGroup reflects the draft and offers stdio + http", () => {
    const radio = nodeByKey(settingsView(initialSettingsState()), "settings-mcp-field-transport") as {
      value?: string
      options?: ReadonlyArray<{ value: string }>
    }
    expect(radio?.value).toBe("stdio")
    expect(radio?.options?.map((o) => o.value)).toEqual(["stdio", "http"])
  })
})

describe("MCP servers — typed intent loop (fake bridge)", () => {
  const makeMcpBridge = (calls: Array<string>, servers: Array<McpConfigServerView>): McpConfigSettingsBridge => ({
    list: async () => ({ state: "ok", dropped: 0, servers }),
    add: async (config) => {
      calls.push(`add:${config.name}`)
      const view = { name: config.name, transport: config.transport, enabled: config.enabled, argsCount: 0, envCount: 0, headersCount: 0 }
      servers.push(view)
      return { state: "ok", dropped: 0, servers }
    },
    remove: async (name) => {
      calls.push(`remove:${name}`)
      const next = servers.filter((s) => s.name !== name)
      return { state: "ok", dropped: 0, servers: next }
    },
    toggle: async (name, enabled) => {
      calls.push(`toggle:${name}:${enabled}`)
      return { state: "ok", dropped: 0, servers: servers.map((s) => (s.name === name ? { ...s, enabled } : s)) }
    },
  })

  const runMcp = <A>(effect: (h: ReturnType<typeof makeSettingsHandlers>, state: SubscriptionRef.SubscriptionRef<{ workspace: string; settings: SettingsState }>) => Effect.Effect<A>, bridge: McpConfigSettingsBridge, seed: Partial<SettingsState> = {}) =>
    Effect.runPromise(Effect.gen(function* () {
      const state = yield* SubscriptionRef.make<{ workspace: string; settings: SettingsState }>({
        workspace: "settings",
        settings: { ...initialSettingsState(), ...seed },
      })
      const handlers = makeSettingsHandlers(state, undefined, undefined, undefined, undefined, undefined, bridge)
      return yield* effect(handlers, state)
    }))

  test("field-change intents update the draft and clear a prior form error", async () => {
    const result = await runMcp((h, state) => Effect.gen(function* () {
      yield* h.DesktopMcpNameChanged("docs")
      yield* h.DesktopMcpCommandChanged("docs-mcp")
      yield* h.DesktopMcpTransportChanged("http")
      return (yield* SubscriptionRef.get(state)).settings.mcp.draft
    }), makeMcpBridge([], []))
    expect(result.name).toBe("docs")
    expect(result.command).toBe("docs-mcp")
    expect(result.transport).toBe("http")
  })

  test("Add happy path calls the bridge, lists the new server, and resets the draft", async () => {
    const calls: Array<string> = []
    const bridge = makeMcpBridge(calls, [])
    const after = await runMcp((h, state) => Effect.gen(function* () {
      yield* h.DesktopMcpNameChanged("docs")
      yield* h.DesktopMcpCommandChanged("docs-mcp")
      yield* h.DesktopMcpAddRequested()
      return (yield* SubscriptionRef.get(state)).settings.mcp
    }), bridge, { mcp: { ...initialMcpSettingsState(), servers: { state: "loaded", servers: [], dropped: 0 } } })
    expect(calls).toEqual(["add:docs"])
    expect(after.servers.state).toBe("loaded")
    if (after.servers.state === "loaded") expect(after.servers.servers.map((s) => s.name)).toEqual(["docs"])
    expect(after.draft.name).toBe("")
    expect(after.formError).toBeNull()
  })

  test("Add sad path (reserved name) sets an inline formError and never calls the bridge", async () => {
    const calls: Array<string> = []
    const after = await runMcp((h, state) => Effect.gen(function* () {
      yield* h.DesktopMcpNameChanged("codex")
      yield* h.DesktopMcpCommandChanged("x")
      yield* h.DesktopMcpAddRequested()
      return (yield* SubscriptionRef.get(state)).settings.mcp
    }), makeMcpBridge(calls, []), { mcp: { ...initialMcpSettingsState(), servers: { state: "loaded", servers: [], dropped: 0 } } })
    expect(calls).toEqual([])
    expect(after.formError).toContain("reserved")
    // The inline error also renders in the view.
    expect(nodeByKey(settingsView({ ...initialSettingsState(), mcp: after }), "settings-mcp-form-error")?.content).toContain("reserved")
  })

  test("toggle dispatch flips enabled through the bridge; remove drops the row", async () => {
    const calls: Array<string> = []
    const seedServers = [stdioView({ name: "docs", enabled: true })]
    const bridge = makeMcpBridge(calls, [...seedServers])
    const after = await runMcp((h, state) => Effect.gen(function* () {
      yield* h.DesktopMcpToggleRequested("docs")
      yield* h.DesktopMcpRemoveRequested("docs")
      return (yield* SubscriptionRef.get(state)).settings.mcp
    }), bridge, { mcp: { ...initialMcpSettingsState(), servers: { state: "loaded", servers: seedServers, dropped: 0 } } })
    expect(calls).toEqual(["toggle:docs:false", "remove:docs"])
    if (after.servers.state === "loaded") expect(after.servers.servers).toEqual([])
  })

  test("remove/toggle for a name not displayed are ignored (renderer-side guard)", async () => {
    const calls: Array<string> = []
    await runMcp((h) => Effect.gen(function* () {
      yield* h.DesktopMcpToggleRequested("ghost")
      yield* h.DesktopMcpRemoveRequested("ghost")
    }), makeMcpBridge(calls, []), { mcp: { ...initialMcpSettingsState(), servers: { state: "loaded", servers: [], dropped: 0 } } })
    expect(calls).toEqual([])
  })
})
