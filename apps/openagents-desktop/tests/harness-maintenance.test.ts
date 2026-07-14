/**
 * Typed per-harness maintenance through the Runtime Gateway (MAINT-1, #8785).
 *
 * Proves the desktop seam end to end at the unit tier: the maintenance
 * query/command decode through the versioned gateway contract, dispatch to
 * injected host actions with per-harness single-flight, and the Settings
 * surface renders version/channel truth with the one-click update affordance
 * driving the typed command — with the failure and channel-jump-refusal
 * outcomes surfaced honestly.
 */
import { describe, expect, test } from "vite-plus/test"
import { resolveIntentRef, type View } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"

import {
  decodeDesktopRuntimeGatewayRequest,
  type DesktopHarnessMaintenanceEntry,
  type DesktopMaintenanceHarness,
} from "../src/runtime-gateway-contract.ts"
import { createDesktopRuntimeGateway } from "../src/runtime-gateway.ts"
import {
  decodeHarnessMaintenanceListView,
  decodeHarnessMaintenanceOutcomeText,
  initialSettingsState,
  makeSettingsHandlers,
  settingsView,
  type HarnessMaintenanceSettingsBridge,
  type SettingsState,
} from "../src/renderer/settings.ts"

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

const codexEntry: DesktopHarnessMaintenanceEntry = {
  harness: "codex",
  installed: true,
  installedVersion: "0.44.0",
  latestVersion: "0.45.0",
  channel: "npm-global",
  advisory: "behind_latest",
  updateSupported: true,
}

const makeMaintenanceActions = (overrides?: {
  update?: (harness: DesktopMaintenanceHarness) => Promise<{
    outcome: "updated" | "already_current" | "channel_jump_refused" | "failed"
    failureReason: string | null
    beforeVersion: string | null
    afterVersion: string | null
    receiptId: string | null
  }>
}) => ({
  status: async () => ({ observedAt: "2026-07-14T00:00:00.000Z", harnesses: [codexEntry] }),
  update:
    overrides?.update ??
    (async () => ({
      outcome: "updated" as const,
      failureReason: null,
      beforeVersion: "0.44.0",
      afterVersion: "0.45.0",
      receiptId: "hmr-test",
    })),
})

const gatewayWith = (maintenance?: ReturnType<typeof makeMaintenanceActions>) =>
  createDesktopRuntimeGateway(
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    maintenance,
  )

describe("gateway contract", () => {
  test("maintenance status query and update command decode through the versioned request schema", () => {
    expect(
      decodeDesktopRuntimeGatewayRequest({
        kind: "query",
        requestId: "r1",
        query: { id: "maintenance.harness_status" },
      }),
    ).not.toBeNull()
    expect(
      decodeDesktopRuntimeGatewayRequest({
        kind: "command",
        commandId: "c1",
        command: { id: "maintenance.harness_update", harness: "codex" },
      }),
    ).not.toBeNull()
    // Unknown harnesses never enter the seam.
    expect(
      decodeDesktopRuntimeGatewayRequest({
        kind: "command",
        commandId: "c2",
        command: { id: "maintenance.harness_update", harness: "grok" },
      }),
    ).toBeNull()
  })
})

describe("gateway dispatch", () => {
  test("status query returns the typed public-safe projection", async () => {
    const gateway = gatewayWith(makeMaintenanceActions())
    const response = await gateway.request({
      kind: "query",
      requestId: "r1",
      query: { id: "maintenance.harness_status" },
    })
    expect(response.kind).toBe("harness_maintenance_status")
    if (response.kind !== "harness_maintenance_status") return
    expect(response.harnesses).toHaveLength(1)
    expect(response.harnesses[0]).toEqual(codexEntry)
    // No paths or command output reach the renderer projection.
    expect(JSON.stringify(response)).not.toContain("/")
  })

  test("update command returns the typed outcome after the host re-probe", async () => {
    const gateway = gatewayWith(makeMaintenanceActions())
    const response = await gateway.request({
      kind: "command",
      commandId: "c1",
      command: { id: "maintenance.harness_update", harness: "codex" },
    })
    expect(response.kind).toBe("harness_maintenance_outcome")
    if (response.kind !== "harness_maintenance_outcome") return
    expect(response.status).toBe("completed")
    expect(response.outcome).toBe("updated")
    expect(response.afterVersion).toBe("0.45.0")
    expect(response.receiptId).toBe("hmr-test")
  })

  test("per-harness single flight: a concurrent second update reports update_already_running", async () => {
    let release: () => void = () => undefined
    const gate = new Promise<void>((resolve) => {
      release = resolve
    })
    const gateway = gatewayWith(
      makeMaintenanceActions({
        update: async () => {
          await gate
          return {
            outcome: "updated",
            failureReason: null,
            beforeVersion: "0.44.0",
            afterVersion: "0.45.0",
            receiptId: "hmr-test",
          }
        },
      }),
    )
    const first = gateway.request({
      kind: "command",
      commandId: "c1",
      command: { id: "maintenance.harness_update", harness: "codex" },
    })
    const second = await gateway.request({
      kind: "command",
      commandId: "c2",
      command: { id: "maintenance.harness_update", harness: "codex" },
    })
    expect(second.kind).toBe("harness_maintenance_outcome")
    if (second.kind === "harness_maintenance_outcome") {
      expect(second.status).toBe("unavailable")
      expect(second.failureReason).toBe("update_already_running")
    }
    release()
    const firstResolved = await first
    expect(firstResolved.kind).toBe("harness_maintenance_outcome")
    if (firstResolved.kind === "harness_maintenance_outcome") {
      expect(firstResolved.status).toBe("completed")
    }
  })

  test("maintenance is honestly unavailable when the host wires no actions", async () => {
    const gateway = gatewayWith(undefined)
    const status = await gateway.request({
      kind: "query",
      requestId: "r1",
      query: { id: "maintenance.harness_status" },
    })
    expect(status.kind).toBe("request_rejected")
    const outcome = await gateway.request({
      kind: "command",
      commandId: "c1",
      command: { id: "maintenance.harness_update", harness: "codex" },
    })
    expect(outcome.kind).toBe("harness_maintenance_outcome")
    if (outcome.kind === "harness_maintenance_outcome") {
      expect(outcome.status).toBe("unavailable")
    }
  })
})

describe("renderer decoding", () => {
  test("status projection decodes; garbage degrades to unavailable", () => {
    const view = decodeHarnessMaintenanceListView({
      kind: "harness_maintenance_status",
      requestId: "r1",
      observedAt: "2026-07-14T00:00:00.000Z",
      harnesses: [codexEntry],
    })
    expect(view.state).toBe("loaded")
    if (view.state === "loaded") {
      expect(view.harnesses[0]!.installedVersion).toBe("0.44.0")
      expect(view.harnesses[0]!.channel).toBe("npm-global")
    }
    expect(decodeHarnessMaintenanceListView(null).state).toBe("unavailable")
    expect(decodeHarnessMaintenanceListView({ kind: "request_rejected" }).state).toBe("unavailable")
  })

  test("outcome text covers success, failure (previous state intact), and channel-jump refusal", () => {
    const base = {
      kind: "harness_maintenance_outcome",
      commandId: "c1",
      harness: "codex",
      beforeVersion: "0.44.0",
      receiptId: "hmr-test",
    }
    expect(
      decodeHarnessMaintenanceOutcomeText({
        ...base,
        status: "completed",
        outcome: "updated",
        failureReason: null,
        afterVersion: "0.45.0",
      }),
    ).toBe("Codex CLI updated to 0.45.0 (re-probe verified).")
    expect(
      decodeHarnessMaintenanceOutcomeText({
        ...base,
        status: "completed",
        outcome: "failed",
        failureReason: "post_update_probe_failed",
        afterVersion: null,
      }),
    ).toContain("Previous install left intact")
    expect(
      decodeHarnessMaintenanceOutcomeText({
        ...base,
        status: "completed",
        outcome: "channel_jump_refused",
        failureReason: null,
        afterVersion: null,
      }),
    ).toContain("refused a channel change")
  })
})

describe("settings surface", () => {
  const stateWith = (settings: SettingsState) => settings

  test("renders version/channel truth and the one-click update affordance", () => {
    const settings = stateWith({
      ...initialSettingsState(),
      harnessMaintenance: {
        view: {
          state: "loaded",
          harnesses: [
            { ...codexEntry },
            {
              harness: "claude_code",
              installed: true,
              installedVersion: "1.0.63",
              latestVersion: "1.0.63",
              channel: "native",
              advisory: "current",
              updateSupported: true,
            },
            {
              harness: "opencode",
              installed: false,
              installedVersion: null,
              latestVersion: null,
              channel: "unknown",
              advisory: "unknown",
              updateSupported: false,
            },
          ],
        },
        updating: null,
        lastOutcome: null,
      },
    })
    const view = settingsView(settings)
    expect(nodeByKey(view, "settings-harness-maintenance-title")?.content).toBe("Coding harnesses")
    expect(nodeByKey(view, "settings-harness-codex-version")?.content).toBe(
      "0.44.0 via npm — 0.45.0 available",
    )
    const updateButton = nodeByKey(view, "settings-harness-codex-update")
    expect(updateButton?._tag).toBe("Button")
    expect(updateButton?.label).toBe("Update to 0.45.0")
    expect(updateButton?.disabled).toBe(false)
    // The button drives the typed intent with the harness payload.
    const resolved = resolveIntentRef(updateButton?.onPress as never)
    expect(resolved.name).toBe("DesktopHarnessUpdateRequested")
    expect(resolved.payload).toBe("codex")
    expect(nodeByKey(view, "settings-harness-claude_code-version")?.content).toBe(
      "1.0.63 via native installer — up to date",
    )
    expect(nodeByKey(view, "settings-harness-opencode-version")?.content).toBe("Not installed")
    expect(nodeByKey(view, "settings-harness-opencode-update")).toBeUndefined()
  })

  test("while updating, the affordance shows progress and blocks a second click", () => {
    const settings = stateWith({
      ...initialSettingsState(),
      harnessMaintenance: {
        view: { state: "loaded", harnesses: [codexEntry] },
        updating: "codex",
        lastOutcome: null,
      },
    })
    const view = settingsView(settings)
    const updateButton = nodeByKey(view, "settings-harness-codex-update")
    expect(updateButton?.label).toBe("Updating…")
    expect(updateButton?.disabled).toBe(true)
  })

  test("full intent loop: open settings loads the list; update drives the bridge and re-reads status", async () => {
    const calls: string[] = []
    let statusVersion = "0.44.0"
    const bridge: HarnessMaintenanceSettingsBridge = {
      status: async () => {
        calls.push("status")
        return {
          kind: "harness_maintenance_status",
          requestId: "r",
          observedAt: "2026-07-14T00:00:00.000Z",
          harnesses: [
            {
              ...codexEntry,
              installedVersion: statusVersion,
              advisory: statusVersion === "0.45.0" ? "current" : "behind_latest",
            },
          ],
        }
      },
      update: async (harness) => {
        calls.push(`update:${harness}`)
        statusVersion = "0.45.0"
        return {
          kind: "harness_maintenance_outcome",
          commandId: "c",
          harness,
          status: "completed",
          outcome: "updated",
          failureReason: null,
          beforeVersion: "0.44.0",
          afterVersion: "0.45.0",
          receiptId: "hmr-test",
        }
      },
    }
    const state = await Effect.runPromise(
      SubscriptionRef.make({ workspace: "chat", settings: initialSettingsState() }),
    )
    const handlers = makeSettingsHandlers(
      state,
      undefined,
      undefined,
      async () => undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      bridge,
    )
    await Effect.runPromise(handlers.DesktopSettingsToggled())
    let current = await Effect.runPromise(SubscriptionRef.get(state))
    expect(current.settings.harnessMaintenance.view.state).toBe("loaded")

    await Effect.runPromise(handlers.DesktopHarnessUpdateRequested("codex"))
    current = await Effect.runPromise(SubscriptionRef.get(state))
    expect(calls).toContain("update:codex")
    expect(current.settings.harnessMaintenance.updating).toBeNull()
    expect(current.settings.harnessMaintenance.lastOutcome).toBe(
      "Codex CLI updated to 0.45.0 (re-probe verified).",
    )
    // The re-read after the update carries the RE-PROBED version.
    const view = current.settings.harnessMaintenance.view
    expect(view.state).toBe("loaded")
    if (view.state === "loaded") {
      expect(view.harnesses[0]!.installedVersion).toBe("0.45.0")
      expect(view.harnesses[0]!.advisory).toBe("current")
    }
    // An unknown harness name is ignored before it can reach the bridge.
    const callCount = calls.length
    await Effect.runPromise(handlers.DesktopHarnessUpdateRequested("grok"))
    expect(calls.length).toBe(callCount)
  })
})
