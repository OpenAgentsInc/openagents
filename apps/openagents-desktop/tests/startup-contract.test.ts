/**
 * Desktop startup incident contract (2026-07-13) — the enforcing oracle set
 * for `openagents_desktop.startup.window_first_no_blank_frame.v1` in
 * src/contracts/ux-contracts.ts.
 *
 * Owner statement (verbatim, recorded in the registry): "Opening the
 * openagents app, via our new oa command or in dev, shows a blank/brown
 * screen for ~5 seconds before opening the UI. This is unacceptable. ...
 * Need good bootup process. No brown screen. If any loading, show beautiful
 * starcraft version of it, or something. Time to seeing stuff and then
 * interactable elements on bootup is extremely important."
 *
 * Measured root causes (docs/fable/2026-07-13-desktop-startup-incident.md):
 *  - the renderer awaited the full local coding-history scan (5.3–6.5 s
 *    against a real ~/.codex) BEFORE mounting the shell;
 *  - Electron main awaited an unbounded network session verification BEFORE
 *    creating the window.
 *
 * Enforced structure (each validator is exercised against a known-bad
 * fixture — assurance design law 4: oracles must demonstrate sensitivity):
 *  1. main.ts whenReady: the window is created FIRST on the production path —
 *     no SQLite open, no keychain custody, and no network session
 *     verification before `createWindow()`; the network settle after the
 *     window is fire-and-forget (`void`), never awaited.
 *  2. boot.ts: the shell mounts BEFORE the coding-history hydration
 *     (`hydrateAfterMount`), catalog metadata paints before selected-thread
 *     detail starts, and the static boot frame is removed after mount.
 *  3. index.html: a branded boot frame paints with the first HTML parse and
 *     every color literal in it is an exact khalaTheme token value
 *     (mechanically synced to @effect-native/tokens — the same rule the
 *     BrowserWindow backgroundColor follows).
 *  4. Sidebar honesty: until hydration settles the sidebar says
 *     "Scanning coding history…" — never the "No local Codex history found."
 *     claim, which would be a lie mid-scan.
 */
import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import path from "node:path"

import { khalaTheme } from "@effect-native/tokens"

import {
  desktopShellView,
  initialDesktopShellState,
} from "../src/renderer/shell.ts"

const testsDir = path.dirname(new URL(import.meta.url).pathname)
const appDir = path.dirname(testsDir)
const mainSource = readFileSync(path.join(appDir, "src", "main.ts"), "utf8")
const bootSource = readFileSync(path.join(appDir, "src", "renderer", "boot.ts"), "utf8")
const indexHtml = readFileSync(path.join(appDir, "index.html"), "utf8")

// ---------------------------------------------------------------------------
// 1. main.ts: window-first ordering inside app.whenReady.
// ---------------------------------------------------------------------------

const windowFirstViolations = (source: string): ReadonlyArray<string> => {
  const start = source.indexOf("void app.whenReady()")
  const end = source.indexOf('app.on("window-all-closed"')
  if (start < 0 || end < 0 || end <= start) return ["whenReady handler not found"]
  const slice = source.slice(start, end)
  const violations: Array<string> = []
  const idxWindow = slice.indexOf("const window = createWindow()")
  if (idxWindow < 0) return ["createWindow() call not found in whenReady"]
  const preWindow = slice.slice(0, idxWindow)
  const after = slice.slice(idxWindow)
  // The windowless local-turn-restart probe branch is the ONLY pre-window
  // region allowed to run persistence/vault/settle (it app.exit()s).
  const idxProbe = preWindow.indexOf("if (localTurnRestartProbe !== null) {")
  const preProbe = idxProbe >= 0 ? preWindow.slice(0, idxProbe) : preWindow
  if (preProbe.includes("openLocalSyncPersistence()")) {
    violations.push("SQLite sync persistence opens before createWindow on the production path")
  }
  if (preProbe.includes("recoverSessionVaultLocal()")) {
    violations.push("keychain vault recovery runs before createWindow on the production path")
  }
  if (preProbe.includes("settleSessionRecovery()")) {
    violations.push("session network verification runs before createWindow on the production path")
  }
  if (!after.includes("openLocalSyncPersistence()")) {
    violations.push("post-window persistence open missing (helper renamed or reverted to inline pre-window work)")
  }
  if (after.includes("await settleSessionRecovery()")) {
    violations.push("session network verification is awaited after createWindow (must be fire-and-forget)")
  }
  if (!after.includes("void settleSessionRecovery()")) {
    violations.push("background session verification missing after createWindow")
  }
  // The raw network call may appear ONLY inside the settle helper definition
  // (which is declared before the probe branch, executed after the window).
  const idxSettleDef = slice.indexOf("const settleSessionRecovery")
  if (idxSettleDef < 0) violations.push("settleSessionRecovery helper missing")
  for (const match of slice.matchAll(/recoverVerifiedDesktopSession\(/g)) {
    const at = match.index ?? -1
    if (idxSettleDef < 0 || at < idxSettleDef || (idxProbe >= 0 && at >= idxProbe)) {
      violations.push("recoverVerifiedDesktopSession is called outside the settle helper")
    }
  }
  return violations
}

describe("startup contract: window before persistence/keychain/network (main.ts)", () => {
  test("the production whenReady path creates the window first", () => {
    expect(windowFirstViolations(mainSource)).toEqual([])
  })

  test("falsifier: the pre-incident ordering (persistence and awaited network before the window) is rejected", () => {
    const bad = [
      "void app.whenReady().then(async () => {",
      "  const settleSessionRecovery = async () => { await recoverVerifiedDesktopSession({ vault }) }",
      "  openLocalSyncPersistence()",
      "  await recoverSessionVaultLocal()",
      "  await settleSessionRecovery()",
      "  if (localTurnRestartProbe !== null) { return }",
      "  const window = createWindow()",
      "})",
      'app.on("window-all-closed", () => {})',
    ].join("\n")
    const violations = windowFirstViolations(bad)
    expect(violations).toContain("SQLite sync persistence opens before createWindow on the production path")
    expect(violations).toContain("keychain vault recovery runs before createWindow on the production path")
    expect(violations).toContain("session network verification runs before createWindow on the production path")
  })

  test("falsifier: awaiting the network settle after the window is rejected", () => {
    const bad = [
      "void app.whenReady().then(async () => {",
      "  const settleSessionRecovery = async () => { await recoverVerifiedDesktopSession({ vault }) }",
      "  if (localTurnRestartProbe !== null) { return }",
      "  const window = createWindow()",
      "  openLocalSyncPersistence()",
      "  await settleSessionRecovery()",
      "})",
      'app.on("window-all-closed", () => {})',
    ].join("\n")
    expect(windowFirstViolations(bad)).toContain(
      "session network verification is awaited after createWindow (must be fire-and-forget)",
    )
  })

  test("the pre-boot BrowserWindow background stays the product-theme token background", () => {
    expect(mainSource).toContain(`backgroundColor: "${khalaTheme.color.background}"`)
  })

  test("ordinary windows fill the active display work area without entering fullscreen", () => {
    const resolveWorkArea = mainSource.indexOf("electronScreen.getDisplayNearestPoint(")
    const createBrowserWindow = mainSource.indexOf("const window = new BrowserWindow({", resolveWorkArea)
    expect(resolveWorkArea).toBeGreaterThan(-1)
    expect(mainSource.indexOf("electronScreen.getCursorScreenPoint()", resolveWorkArea)).toBeGreaterThan(resolveWorkArea)
    expect(createBrowserWindow).toBeGreaterThan(resolveWorkArea)
    for (const field of ["x", "y", "width", "height"]) {
      expect(mainSource.slice(createBrowserWindow, createBrowserWindow + 800)).toContain(`${field}: launchWorkArea.${field}`)
    }
    expect(mainSource.slice(createBrowserWindow, createBrowserWindow + 800)).toContain("fullscreen: false")
    expect(mainSource.slice(createBrowserWindow, createBrowserWindow + 800)).not.toContain("window.maximize()")
  })

  test("ordinary startup admits the validated launcher directory before restoring the WorkContext", () => {
    const resolve = mainSource.indexOf("const desktopLaunchWorkingDirectory = desktopLaunchWorkspaceRoot({")
    const select = mainSource.indexOf("selectWorkspace(desktopLaunchWorkingDirectory)")
    const restore = mainSource.indexOf("const restoredRoot = syncHost.codingCatalog()?.selectedRoot()")
    expect(resolve).toBeGreaterThan(-1)
    expect(select).toBeGreaterThan(resolve)
    expect(restore).toBeGreaterThan(select)
  })
})

// ---------------------------------------------------------------------------
// 2. boot.ts: shell mount precedes coding-history hydration.
// ---------------------------------------------------------------------------

const bootOrderViolations = (source: string): ReadonlyArray<string> => {
  const violations: Array<string> = []
  const idxHydrateDef = source.indexOf("const hydrateAfterMount")
  const idxCatalogFetch = source.indexOf("historyHost.catalog()")
  const idxMount = source.indexOf("renderer.mount(root")
  const idxHydrateRun = source.indexOf("yield* hydrateAfterMount")
  if (idxHydrateDef < 0) violations.push("hydrateAfterMount missing from boot.ts")
  if (idxCatalogFetch < 0) violations.push("history catalog fetch missing from boot.ts")
  if (idxMount < 0) violations.push("renderer.mount missing from boot.ts")
  if (idxHydrateRun < 0) violations.push("hydrateAfterMount is never run")
  if (violations.length > 0) return violations
  if (idxCatalogFetch < idxHydrateDef) {
    violations.push("history catalog is fetched outside hydrateAfterMount (pre-mount blocking hydration)")
  }
  if (idxHydrateRun < idxMount) {
    violations.push("hydration runs before the shell mounts")
  }
  if (!source.includes('getElementById("openagents-boot-frame")')) {
    violations.push("the boot frame is never removed after mount")
  }
  return violations
}

const metadataPaintViolations = (source: string): ReadonlyArray<string> => {
  const violations: Array<string> = []
  const catalogCommit = source.indexOf("catalog: historyCatalog")
  const visiblePaint = source.indexOf(
    "requestAnimationFrame(() => requestAnimationFrame(() => resolve()))",
    catalogCommit,
  )
  if (catalogCommit < 0) violations.push("history catalog metadata is never committed")
  if (visiblePaint < 0) violations.push("catalog metadata has no guaranteed visible paint")
  return violations
}

const startupNewSessionViolations = (source: string): ReadonlyArray<string> => {
  const violations: Array<string> = []
  const shellMounted = source.indexOf("marks.shellMounted = Date.now()")
  const hydrationRun = source.indexOf("yield* hydrateAfterMount")
  const hydrateStart = source.indexOf("const hydrateAfterMount")
  const hydrateEnd = source.indexOf("// Focus must land AFTER", hydrateStart)
  if (shellMounted < 0) violations.push("shell-mounted milestone is missing")
  if (hydrationRun < 0) violations.push("startup hydration run is missing")
  if (hydrateStart < 0 || hydrateEnd < 0) violations.push("startup hydration block is missing")
  if (violations.length > 0) return violations
  const hydration = source.slice(hydrateStart, hydrateEnd)
  for (const forbidden of ["restorableHistoryThreadRef", "historyHost.page(", "chat.openThread(", "chat.hydrateThread("]) {
    if (hydration.includes(forbidden)) violations.push(`startup hydration auto-selects conversation detail through ${forbidden}`)
  }
  if (!hydration.includes("withThreadCatalog(current, threads)")) {
    violations.push("startup thread metadata does not use the selection-preserving catalog projector")
  }
  return violations
}

describe("startup contract: shell mounts before history hydration (boot.ts)", () => {
  test("the renderer mounts the shell first and hydrates afterwards", () => {
    expect(bootOrderViolations(bootSource)).toEqual([])
    expect(metadataPaintViolations(bootSource)).toEqual([])
    expect(startupNewSessionViolations(bootSource)).toEqual([])
  })

  test("falsifier: fetching the history catalog before mount is rejected", () => {
    const bad = [
      "const catalog = historyHost.catalog()",
      "const hydrateAfterMount = Effect.gen(function* () {})",
      "yield* renderer.mount(root, program.viewStream, report)",
      'document.getElementById("openagents-boot-frame")?.remove()',
      "yield* hydrateAfterMount",
    ].join("\n")
    expect(bootOrderViolations(bad)).toContain(
      "history catalog is fetched outside hydrateAfterMount (pre-mount blocking hydration)",
    )
  })

  test("falsifier: history detail autoload during startup hydration is rejected", () => {
    const bad = [
      "const hydrateAfterMount = Effect.gen(function* () {",
      "historyHost.page(selected, 0, 1)",
      "// Focus must land AFTER",
      "marks.shellMounted = Date.now()",
      "withThreadCatalog(current, threads)",
      "yield* hydrateAfterMount",
    ].join("\n")
    expect(startupNewSessionViolations(bad)).toContain(
      "startup hydration auto-selects conversation detail through historyHost.page(",
    )
  })

  test("the MVP history host never scans the out-of-scope Claude store", () => {
    const historyHostStart = mainSource.indexOf("// MVP is Codex-only.")
    const historyHostEnd = mainSource.indexOf("}),()=>hostLifecycle.sync()", historyHostStart)
    expect(historyHostStart).toBeGreaterThan(-1)
    expect(historyHostEnd).toBeGreaterThan(historyHostStart)
    const historyHost = mainSource.slice(historyHostStart, historyHostEnd)
    expect(historyHost).toContain('claudeRoot: null')
    expect(historyHost).not.toContain("claudeProjectsRoot()")
  })
})

// ---------------------------------------------------------------------------
// 3. index.html: branded boot frame, colors mechanically synced to tokens.
// ---------------------------------------------------------------------------

describe("startup contract: branded boot frame (index.html)", () => {
  const styleBlock = indexHtml.slice(indexHtml.indexOf("<style>"), indexHtml.indexOf("</style>"))

  test("the boot frame exists and paints the product-theme background", () => {
    expect(indexHtml).toContain('id="openagents-boot-frame"')
    expect(styleBlock).toContain(`background: ${khalaTheme.color.background}`)
  })

  test("every boot-frame color literal is an exact khalaTheme token value", () => {
    const palette = new Set(
      Object.values(khalaTheme.color).map((value) => value.toLowerCase()),
    )
    const hexes = [...styleBlock.matchAll(/#[0-9a-fA-F]{3,8}\b/g)].map((match) => match[0].toLowerCase())
    expect(hexes.length).toBeGreaterThan(0)
    const offPalette = hexes.filter((hex) => !palette.has(hex))
    expect(offPalette).toEqual([])
  })

  test("falsifier: an off-palette color would be rejected", () => {
    const palette = new Set(
      Object.values(khalaTheme.color).map((value) => value.toLowerCase()),
    )
    expect(palette.has("#8b4513")).toBe(false) // an actual brown never enters the boot frame
  })
})

// ---------------------------------------------------------------------------
// 4. Sidebar honesty: scanning row until hydration settles.
// ---------------------------------------------------------------------------

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

describe("startup contract: sidebar scanning honesty", () => {
  const base = initialDesktopShellState("openagents-desktop/test")

  test("pre-hydration renders the scanning row, never the empty-history claim", () => {
    const nodes = collectNodes(desktopShellView(base))
    const scanning = nodes.find((node) => node.key === "sidebar-history-scanning")
    expect(scanning?.content).toBe("Scanning coding history…")
    expect(nodes.find((node) => node.key === "sidebar-chats-empty")).toBeUndefined()
  })

  test("post-hydration with a truly empty catalog renders the honest empty state", () => {
    const nodes = collectNodes(desktopShellView({
      ...base,
      history: { ...base.history, hydrated: true },
    }))
    expect(nodes.find((node) => node.key === "sidebar-history-scanning")).toBeUndefined()
    expect(nodes.find((node) => node.key === "sidebar-chats-empty")).toBeDefined()
  })
})
