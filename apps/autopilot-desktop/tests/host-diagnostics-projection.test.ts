// VCODE-14 (#5931): host readiness and diagnostics pane.

import { describe, expect, test } from "bun:test"

import type { NodeStateMessage } from "../src/shared/rpc"
import { projectCodeModeSyncSnapshot } from "../src/ui/code-mode-sync"
import { projectHostDiagnosticsPanel } from "../src/ui/host-diagnostics-projection"
import { initialModel, Model, modelPaneLayer } from "../src/ui/model"
import {
  GotNodeState,
  NavigatedTo,
  OpenedManagedPane,
} from "../src/ui/message"
import { update } from "../src/ui/update"
import {
  clearVerseSceneDiagnosticsForTest,
  recordVerseSceneDiagnostic,
} from "../src/ui/verse-scene-diagnostics"
import { view } from "../src/ui/view"

const renderTree = (node: unknown): string => {
  const seen = new WeakSet<object>()
  return JSON.stringify(node, (_key, value) => {
    if (typeof value === "function") return "[fn]"
    if (typeof value === "object" && value !== null) {
      if (seen.has(value)) return "[cycle]"
      seen.add(value)
    }
    return value
  })
}

const accountHash = "account.pylon.codex.work.abcdef0123456789abcdef0123456789"

const nodeState = (): NodeStateMessage => ({
  ok: true,
  schema: "openagents.pylon.control.v0.3",
  sessions: [
    {
      sessionRef: "session.pylon.codex.diag",
      adapter: "codex",
      state: "running",
      objectiveRef: "objective.diag",
      workspaceRef: "workspace.openagents.desktop",
      accountRefHash: accountHash,
      latestActivity: "running diagnostics test",
      updatedAt: "2026-06-21T23:30:00.000Z",
    },
  ],
  events: {
    "session.pylon.codex.diag": [
      {
        eventIndex: 0,
        phase: "progress",
        state: "running",
        observedAt: "2026-06-21T23:30:00.000Z",
        detail: "running /Users/christopherdavid/work/private-repo",
      },
    ],
  },
  accounts: [
    {
      provider: "codex",
      homeState: "present",
      ready: false,
      accountRef: "work",
      accountRefHash: accountHash,
      selector: "registry_ref",
      blockerRefs: ["codex.login_required"],
      priority: 1,
    },
  ],
})

describe("host diagnostics projection", () => {
  test("projects node, bridge, accounts, stream, transcript, scene, and input rows", () => {
    const sync = projectCodeModeSyncSnapshot({
      source: "node_state",
      node: nodeState(),
      managedAccounts: null,
      inferenceGatewayReadiness: null,
      builtInAgentReadiness: null,
      appleFmReadiness: null,
      selectedSessionRef: null,
      composerAccountRef: "work",
    })
    const panel = projectHostDiagnosticsPanel({
      nodeLaunchStatus: "online",
      node: nodeState(),
      sync,
      generatedAt: "2026-06-21T23:30:00.000Z",
      sceneDiagnostics: [
        {
          at: "2026-06-21T23:30:01.000Z",
          event: "verse-host.remount.swapped",
          detail: { path: "/Users/christopherdavid/work/private-repo", token: "sk-secretsecretsecret" },
        },
        {
          at: "2026-06-21T23:30:02.000Z",
          event: "verse-host.camera-control",
          detail: { movementX: 20, movementY: 5 },
        },
      ],
    })

    expect(panel.rows.map((row) => row.section)).toEqual([
      "node",
      "bridge",
      "accounts",
      "stream",
      "transcript",
      "scene",
      "input",
    ])
    expect(panel.rows.find((row) => row.key === "accounts.codex.work")?.status).toBe("blocked")
    expect(panel.counters.sceneRemounts).toBe(1)
    expect(panel.counters.cameraControlEvents).toBe(1)
    expect(panel.counters.streamEvents).toBe(1)

    const exported = JSON.stringify(panel.exportData)
    expect(exported).not.toContain(accountHash)
    expect(exported).not.toContain("/Users/christopherdavid")
    expect(exported).not.toContain("sk-secretsecretsecret")
    expect(exported).toContain("[local-path]")
    expect(exported).toContain("[secret-ref]")
  })
})

describe("host diagnostics pane", () => {
  test("Diagnostics pane renders a public-safe export", () => {
    clearVerseSceneDiagnosticsForTest()
    recordVerseSceneDiagnostic("verse-host.camera-control", {
      movementX: 12,
      path: "/Users/christopherdavid/work/private-repo",
    })
    let model = Model.make({
      ...initialModel,
      pane: "diagnostics",
      nodeLaunchStatus: "online",
    })
    ;[model] = update(model, GotNodeState({ node: nodeState() }))
    const html = renderTree(view(model).body)
    expect(html).toContain("autopilot-host-diagnostics")
    expect(html).toContain("Pylon node")
    expect(html).toContain("Codex work")
    expect(html).toContain("Public-safe export")
    const exportStart = html.indexOf("autopilot-host-diagnostics-export")
    const statusHudStart = html.indexOf("status-hud-overlay", exportStart)
    const exportSlice = html.slice(
      exportStart,
      statusHudStart === -1 ? undefined : statusHudStart,
    )
    expect(exportSlice).not.toContain(accountHash)
    expect(exportSlice).not.toContain("/Users/christopherdavid")
    expect(exportSlice).toContain("account#23456789")
    expect(exportSlice).toContain("[local-path]")
    clearVerseSceneDiagnosticsForTest()
  })

  test("opening Diagnostics refreshes host readiness inputs", () => {
    const [navigated, navCommands] = update(initialModel, NavigatedTo({ pane: "diagnostics" }))
    expect(navigated.pane).toBe("diagnostics")
    expect(navCommands.map((command) => command.name)).toEqual([
      "LoadManagedAccounts",
      "LoadInferenceGatewayReadiness",
      "LoadBuiltInAgentReadiness",
      "LoadAppleFmReadiness",
      "LoadInstallReadiness",
    ])

    const [withPane, paneCommands] = update(
      Model.make({ ...initialModel, verseMode: "code" }),
      OpenedManagedPane({ pane: "diagnostics" }),
    )
    expect(modelPaneLayer(withPane).panes.map((pane) => pane.kind)).toEqual(["diagnostics"])
    expect(paneCommands.map((command) => command.name)).toContain("LoadInstallReadiness")
  })
})
