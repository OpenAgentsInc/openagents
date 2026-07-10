import { describe, expect, test } from "bun:test"
import { existsSync, readdirSync, readFileSync } from "node:fs"
import { join } from "node:path"

import { CatalogVersion } from "@effect-native/core"

import {
  initialHomeState,
  renderChromeComposerView,
  renderChromeMenuButtonView,
  renderChromePillView,
  renderContentView,
  renderDrawerView,
  renderMineralsSheetView,
} from "../src/screens/home-core"

/**
 * OpenAgents mobile (#8597) component-sharing proof — one catalog, many hosts.
 *
 * The Home screen's view-program layer must stay structurally identical to the
 * web Effect Native consumers (`apps/openagents.com/apps/start` routes): a pure
 * module authoring the SAME `@effect-native/core` component catalog, with the
 * host renderer (`render-rn` here, `render-dom` on web) as the only
 * per-surface difference. These oracles keep that seam mechanical:
 *
 * 1. The core module imports nothing host-specific (no react/react-native/dom,
 *    no app-local UI primitives — no parallel design system).
 * 2. The authored tree carries the shared catalog version the web surfaces
 *    assert (`effect-native/v25`).
 * 3. The shell imports its theme from `@effect-native/tokens` (`khalaTheme`,
 *    the one Protoss-blue theme every Effect Native host mounts).
 */

const appRoot = join(import.meta.dir, "..")

const sourceFilesUnder = (...roots: ReadonlyArray<string>): ReadonlyArray<string> =>
  roots.flatMap((root) =>
    readdirSync(root, { recursive: true, encoding: "utf8" })
      .filter((entry) => entry.endsWith(".ts") || entry.endsWith(".tsx"))
      .map((entry) => join(root, entry)),
  )

const importSpecifiers = (source: string): ReadonlyArray<string> =>
  [...source.matchAll(/from\s+"([^"]+)"/g)].map((match) => match[1] ?? "")

describe("contract openagents_mobile.home.catalog_sharing.v1", () => {
  test("the view-program layer is host-agnostic: only @effect-native/core (or pure sibling) imports", () => {
    // GL-3 (#8649): the pure layer is now two sibling modules — home-core and
    // sarah-core. Each may import ONLY @effect-native/core (+ its effect
    // bridge) or the other pure sibling; nothing host-specific ever enters.
    const pureSiblings = ["./sarah-core", "./home-core"]
    for (const module of ["src/screens/home-core.ts", "src/screens/sarah-core.ts"]) {
      const source = readFileSync(join(appRoot, module), "utf8")
      const specifiers = importSpecifiers(source)
      expect(specifiers.length).toBeGreaterThan(0)
      for (const specifier of specifiers) {
        expect(
          specifier.startsWith("@effect-native/core") ||
            pureSiblings.includes(specifier),
        ).toBe(true)
      }
      expect(source).not.toContain('"react"')
      expect(source).not.toContain('"react-native"')
      expect(source).not.toContain('"expo')
    }
  })

  test("the authored trees are the shared catalog version the web EN surfaces author", () => {
    // Same vendored catalog the web start-app Effect Native routes author —
    // one catalog version across DOM and RN hosts, for BOTH projections.
    for (const view of [
      renderContentView(initialHomeState),
      renderDrawerView(initialHomeState),
      renderChromeMenuButtonView(initialHomeState),
      renderChromePillView(initialHomeState),
      renderChromeComposerView(initialHomeState),
      renderMineralsSheetView(initialHomeState),
    ]) {
      expect(JSON.stringify(view)).toContain(`"catalogVersion":"${CatalogVersion}"`)
    }
    expect(CatalogVersion.startsWith("effect-native/v")).toBe(true)
  })

  test("the shell mounts the shared khalaTheme from @effect-native/tokens (no parallel palette)", () => {
    const shell = readFileSync(
      join(appRoot, "src/screens/home-screen.tsx"),
      "utf8",
    )
    expect(shell).toContain('import { khalaTheme } from "@effect-native/tokens"')
    expect(shell).toContain("theme={khalaTheme}")
    // No hex colors hand-written in the shell beyond the theme values.
    expect(shell).not.toMatch(/#[0-9a-fA-F]{6}/)
  })

  test("GL-1 (#8647): @expo/ui is a render-rn-INTERNAL lowering target — app code never imports it", () => {
    // The hybrid contract (docs/fable/2026-07-09): @expo/ui is consumed
    // strictly INSIDE @effect-native/render-rn. The app declares the
    // dependency (the native-module installation vehicle) but no app source
    // file may import from it.
    const sources = sourceFilesUnder(join(appRoot, "src"))
    expect(sources.length).toBeGreaterThan(0)
    for (const file of sources) {
      const source = readFileSync(file, "utf8")
      if (file.endsWith("component-sharing.test.ts")) continue
      expect(source).not.toContain('from "@expo' + '/ui')
      expect(source).not.toContain('require("@expo' + '/ui')
    }
    const packageJson = JSON.parse(readFileSync(join(appRoot, "package.json"), "utf8")) as {
      dependencies: Record<string, string>
    }
    expect(packageJson.dependencies["@expo/ui"]).toBeDefined()
  })

  test("GL-1 (#8647): the D-MB-02 app-local liquid-glass island is DELETED — no module, no references", () => {
    // Converted to catalog Host-kind mounting through the render-rn driver
    // seam + the internal @expo/ui glass lowering; the expo-module is gone.
    expect(existsSync(join(appRoot, "modules/openagents-liquid-glass"))).toBe(false)
    const sources = sourceFilesUnder(join(appRoot, "src"), join(appRoot, "tests"))
    // Banned refs assembled by concatenation so this test file's own source
    // (which the sweep also scans) never contains them literally.
    const bannedRefs = [
      'from "openagents-liquid-' + 'glass"',
      "loadGlass" + "IconButton",
      "requireNative" + "ViewManager",
    ]
    for (const file of sources) {
      const source = readFileSync(file, "utf8")
      if (file.endsWith("component-sharing.test.ts")) continue
      for (const banned of bannedRefs) {
        expect(source).not.toContain(banned)
      }
    }
    const packageJson = readFileSync(join(appRoot, "package.json"), "utf8")
    expect(packageJson).not.toContain("openagents-liquid-" + "glass")
  })
})
