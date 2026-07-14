import { describe, expect, test } from "vite-plus/test"
import { resolveIntentRef, type View } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"

import {
  diagnosticsView,
  initialDiagnosticsState,
  makeDiagnosticsHandlers,
  type DiagnosticsBridge,
  type DiagnosticsState,
} from "./diagnostics.ts"
import { buildDiagnosticsReport, type DiagnosticsInputs } from "../diagnostics-report.ts"

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

const nodesByTag = (view: View, tag: string): Array<AnyNode> =>
  collectNodes(view).filter((node) => node._tag === tag)

const healthy = (): DiagnosticsInputs => ({
  appVersion: "0.0.1",
  generatedAt: 1,
  provider: { state: "ok", accounts: [{ ref: "codex", readiness: "ready" }] },
  runtimeGateway: { state: "present", lifecycle: "ready", sessionPhase: "session_ready", capabilities: [{ id: "khala-sync", state: "available" }] },
  sync: { state: "local_ready", syncPhase: "live", pendingMutationCount: 0 },
  workspace: { state: "selected", git: "clean", entryCount: 5 },
  pty: { state: "available", sessionCount: 0 },
  extensions: { state: "ok", enabledCount: 1, totalCount: 1, dropped: 0 },
})

const loadedState = (): DiagnosticsState => ({ report: { state: "loaded", report: buildDiagnosticsReport(healthy()) }, busy: null, notice: null })

describe("diagnostics view", () => {
  test("loading state renders a single info line and disabled export", () => {
    const view = diagnosticsView(initialDiagnosticsState())
    expect(collectNodes(view).find((node) => node.key === "diagnostics-loading")).toBeDefined()
    const exportButton = collectNodes(view).find((node) => node.key === "diagnostics-export")
    expect(exportButton?.disabled).toBe(true)
  })

  test("loaded state renders a row + level badge for every health domain", () => {
    const view = diagnosticsView(loadedState())
    for (const domain of ["provider", "runtimeGateway", "sync", "workspace", "pty", "extensions"]) {
      expect(collectNodes(view).find((node) => node.key === `diagnostics-row-${domain}`)).toBeDefined()
      expect(collectNodes(view).find((node) => node.key === `diagnostics-row-${domain}-level`)?._tag).toBe("Badge")
    }
  })

  test("a11y: every interactive control has a non-empty accessible name", () => {
    const view = diagnosticsView(loadedState())
    const buttons = nodesByTag(view, "Button")
    expect(buttons.length).toBeGreaterThan(0)
    for (const button of buttons) {
      expect(typeof button.label).toBe("string")
      expect((button.label as string).length).toBeGreaterThan(0)
    }
    // Each health row is a labelled group region.
    const row = collectNodes(view).find((node) => node.key === "diagnostics-row-sync") as AnyNode
    expect((row.a11y as { label?: string } | undefined)?.label).toContain("Khala Sync")
  })

  test("privacy: no rendered text carries a path, url, or token-like blob", () => {
    const view = diagnosticsView(loadedState())
    for (const text of nodesByTag(view, "Text")) {
      const content = text.content
      if (typeof content === "string") {
        expect(content).not.toMatch(/\/[A-Za-z]|:\/\/|Bearer|sk-|\\[A-Za-z]/)
      }
    }
  })

  test("export button disabled while nothing is loaded, enabled once loaded", () => {
    expect(collectNodes(diagnosticsView(loadedState())).find((n) => n.key === "diagnostics-export")?.disabled).toBe(false)
  })
})

describe("diagnostics handler loop", () => {
  const runWith = async (bridge: DiagnosticsBridge, drive: (handlers: ReturnType<typeof makeDiagnosticsHandlers>, state: SubscriptionRef.SubscriptionRef<{ diagnostics: DiagnosticsState }>) => Effect.Effect<void>) => {
    return Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make({ diagnostics: initialDiagnosticsState() })
        const handlers = makeDiagnosticsHandlers(state, bridge)
        yield* drive(handlers, state)
        return (yield* SubscriptionRef.get(state)).diagnostics
      }),
    )
  }

  test("refresh gathers the report into state", async () => {
    const report = buildDiagnosticsReport(healthy())
    const bridge: DiagnosticsBridge = { gather: async () => report, runAction: async () => ({ ok: true }), exportRedacted: async () => ({ ok: true }) }
    const result = await runWith(bridge, (h) => h.DesktopDiagnosticsRefreshRequested())
    expect(result.report.state).toBe("loaded")
    expect(result.busy).toBeNull()
  })

  test("a corrupt gather resolves to an unavailable view, never a throw", async () => {
    const bridge: DiagnosticsBridge = { gather: async () => ({ not: "a report" }), runAction: async () => ({ ok: false }), exportRedacted: async () => ({ ok: false }) }
    const result = await runWith(bridge, (h) => h.DesktopDiagnosticsRefreshRequested())
    expect(result.report.state).toBe("unavailable")
  })

  test("export sets a public-safe notice", async () => {
    const bridge: DiagnosticsBridge = { gather: async () => buildDiagnosticsReport(healthy()), runAction: async () => ({ ok: true }), exportRedacted: async () => ({ ok: true, notice: "Redacted diagnostics exported" }) }
    const result = await runWith(bridge, (h) => h.DesktopDiagnosticsExportRequested())
    expect(result.notice).toBe("Redacted diagnostics exported")
  })

  test("a successful recovery action re-gathers the report", async () => {
    let gathers = 0
    const bridge: DiagnosticsBridge = {
      gather: async () => {
        gathers += 1
        return buildDiagnosticsReport(healthy())
      },
      runAction: async () => ({ ok: true, notice: "Providers re-checked" }),
      exportRedacted: async () => ({ ok: true }),
    }
    const result = await runWith(bridge, (h) => h.DesktopDiagnosticsActionRequested("reprobe_providers"))
    expect(gathers).toBe(1)
    expect(result.report.state).toBe("loaded")
    expect(result.notice).toBe("Providers re-checked")
  })

  test("the action IntentRef payload resolves to the bounded action enum", () => {
    const degraded = buildDiagnosticsReport({ ...healthy(), provider: { state: "unavailable" } })
    const view = diagnosticsView({ report: { state: "loaded", report: degraded }, busy: null, notice: null })
    const actionButton = collectNodes(view).find((node) => node.key === "diagnostics-row-provider-action-reprobe_providers") as AnyNode
    expect(actionButton).toBeDefined()
    const resolved = resolveIntentRef(actionButton.onPress as never, {})
    expect(resolved.name).toBe("DesktopDiagnosticsActionRequested")
    expect(resolved.payload).toBe("reprobe_providers")
  })
})
