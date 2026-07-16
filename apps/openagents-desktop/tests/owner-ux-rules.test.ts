/**
 * Owner micro-interaction / typography rules (owner-directive, 2026-07-13).
 *
 * This suite is the enforcing oracle set for three registry contracts in
 * apps/openagents-desktop/src/contracts/ux-contracts.ts:
 *
 *  - openagents_desktop.microinteraction.owner_review_register.v1
 *  - openagents_desktop.microinteraction.icon_slot_no_raw_text.v1
 *  - openagents_desktop.typography.approved_fonts_only.v1
 *
 * Owner statements (verbatim, recorded in the registry):
 *  1. "I want the ones that we have for that for this app to include micro
 *     interactions and things that I do and don't want to see. There are some
 *     things I don't want to see, such as long streams of text where icons
 *     should be."
 *  2. "I want all that enforced in the assurance pieces. I want to be able to
 *     specify rules there. For example, I don't want to see certain things
 *     like strings where icons should be, certain fonts, and that must be
 *     specified"
 *
 * Enforcement shape (honest): the icon oracle proves the STRUCTURAL subset on
 * the real typed view trees (closed-catalog glyphs in icon slots, bounded
 * single-line dock labels, glyph+accessible-label IconButtons); the font
 * oracle scans every non-test source/style file under src/ so a rogue
 * font-family declaration fails the sweep. The fully general pixel-level
 * claim is an AssuranceSpec `visual` obligation later — see
 * docs/assurance/UX_CONTRACTS_AND_ASSURANCE.md. Each validator here is
 * exercised against a known-bad fixture (assurance design law 4: oracles must
 * demonstrate sensitivity) so the suite can never rot into an always-green
 * check.
 */
import { describe, expect, test } from "vite-plus/test"
import { readFileSync, readdirSync, statSync } from "node:fs"
import path from "node:path"

import { iconNames } from "@effect-native/core"

import {
  desktopShellView,
  initialDesktopShellState,
  type DesktopShellState,
} from "../src/renderer/shell.ts"
import { openAgentsDesktopUxContractRegistry } from "../src/contracts/ux-contracts.ts"

const testsDir = path.dirname(new URL(import.meta.url).pathname)
const appDir = path.dirname(testsDir)
const srcDir = path.join(appDir, "src")
const repoRoot = path.dirname(path.dirname(appDir))
const sharedUiSrcDir = path.join(repoRoot, "packages", "ui", "src")

// ---------------------------------------------------------------------------
// Shared: recursive source listing for the desktop app (non-test sources).
// ---------------------------------------------------------------------------

const sourceFiles = (dir: string): ReadonlyArray<string> => {
  const found: Array<string> = []
  for (const name of readdirSync(dir)) {
    const full = path.join(dir, name)
    if (statSync(full).isDirectory()) {
      found.push(...sourceFiles(full))
      continue
    }
    if (/\.test\.tsx?$/.test(name)) continue
    if (name.endsWith(".ts") || name.endsWith(".tsx") || name.endsWith(".cts") || name.endsWith(".css"))
      found.push(full)
  }
  return found
}

// ---------------------------------------------------------------------------
// openagents_desktop.typography.approved_fonts_only.v1
//
// The app's approved rendered type system is the owner-selected shadcn preset:
// Oxanium for body/UI copy and Geist for headings, with the host system stack
// as the resilient body fallback. The generic `monospace` family remains the
// approved code-surface fallback (the shared @effect-native/render-dom
// CodeBlock lowering uses it). Nothing else may be declared anywhere in the
// desktop app's sources or styles.
// ---------------------------------------------------------------------------

const approvedFontFamilies: ReadonlySet<string> = new Set([
  "Oxanium Variable",
  "Geist Variable",
  "-apple-system",
  "BlinkMacSystemFont",
  "SF Pro Text",
  "sans-serif",
  "monospace",
])

const approvedBaseStackDeclaration =
  'font-family: "Oxanium Variable", -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;'

const approvedHeadingStackDeclaration =
  'font-family: "Geist Variable", "Oxanium Variable", sans-serif;'

const splitFamilies = (value: string): ReadonlyArray<string> =>
  value
    .split(",")
    .map((family) => family.trim().replace(/^["']|["']$/g, "").trim())
    .filter((family) => family.length > 0)

/** Every font-family / fontFamily declaration in one source, with offenders. */
export const fontDeclarationOffenders = (
  fileName: string,
  source: string,
): ReadonlyArray<string> => {
  const offenders: Array<string> = []
  const record = (declaration: string): void => {
    for (const family of splitFamilies(declaration)) {
      if (!approvedFontFamilies.has(family)) {
        offenders.push(`${fileName}: font family "${family}" is not in the approved stack`)
      }
    }
  }
  for (const match of source.matchAll(/font-family\s*:\s*([^;{}]+)/g)) record(match[1]!)
  for (const match of source.matchAll(/fontFamily\s*[:=]\s*["'`]([^"'`]+)["'`]/g)) record(match[1]!)
  if (fileName.endsWith(".css")) {
    // The `font:` shorthand could smuggle a family past the family checks; in
    // this app it is only ever the exact `font: inherit` form-control reset.
    for (const match of source.matchAll(/(?<![\w-])font\s*:\s*([^;{}]+)/g)) {
      if (match[1]!.trim() !== "inherit") {
        offenders.push(`${fileName}: font shorthand "${match[1]!.trim()}" must be exactly "inherit"`)
      }
    }
  }
  return offenders
}

describe("openagents_desktop.typography.approved_fonts_only.v1", () => {
  test("every font declaration under src/ resolves to the approved stack", () => {
    const offenders: Array<string> = []
    for (const file of [...sourceFiles(srcDir), ...sourceFiles(sharedUiSrcDir)]) {
      offenders.push(...fontDeclarationOffenders(path.relative(appDir, file), readFileSync(file, "utf8")))
    }
    expect(offenders).toEqual([])
  })

  test("the approved body and heading stacks remain declared on their host stylesheets", () => {
    const css = readFileSync(path.join(srcDir, "renderer", "app.css"), "utf8")
    const workbenchCss = readFileSync(path.join(sharedUiSrcDir, "desktop-workbench.css"), "utf8")
    expect(css).toContain(approvedBaseStackDeclaration)
    expect(workbenchCss).toContain(approvedHeadingStackDeclaration)
  })

  test("falsifier: a rogue font-family declaration is rejected", () => {
    expect(
      fontDeclarationOffenders("bad.css", 'body { font-family: "Comic Sans MS", cursive; }'),
    ).toEqual([
      'bad.css: font family "Comic Sans MS" is not in the approved stack',
      'bad.css: font family "cursive" is not in the approved stack',
    ])
    expect(
      fontDeclarationOffenders("bad.ts", 'style.fontFamily = "Papyrus"'),
    ).toEqual(['bad.ts: font family "Papyrus" is not in the approved stack'])
    expect(
      fontDeclarationOffenders("bad.css", "input { font: 16px Arial; }"),
    ).toEqual(['bad.css: font shorthand "16px Arial" must be exactly "inherit"'])
    // And the approved stack passes untouched.
    expect(fontDeclarationOffenders("app.css", `html { ${approvedBaseStackDeclaration} }`)).toEqual([])
    expect(
      fontDeclarationOffenders("react-workbench.css", `.heading { ${approvedHeadingStackDeclaration} }`),
    ).toEqual([])
  })
})

// ---------------------------------------------------------------------------
// openagents_desktop.microinteraction.icon_slot_no_raw_text.v1
//
// Icon slots on the real typed view trees: dock items carry a closed-catalog
// glyph plus bounded single-line micro-copy, every icon-carrying node's glyph
// resolves in the closed catalog, and every IconButton is glyph +
// accessible-label with no rendered text content.
// ---------------------------------------------------------------------------

const iconCatalog: ReadonlySet<string> = new Set(iconNames)

/** Dock labels are micro-copy, never streams of text. */
const dockLabelMaxLength = 24

type AnyNode = Readonly<Record<string, unknown>>

const walkObjects = (root: unknown, visit: (node: AnyNode) => void): void => {
  if (Array.isArray(root)) {
    for (const item of root) walkObjects(item, visit)
    return
  }
  if (typeof root !== "object" || root === null) return
  visit(root as AnyNode)
  for (const value of Object.values(root)) walkObjects(value, visit)
}

export const iconSlotOffenders = (view: unknown): ReadonlyArray<string> => {
  const offenders: Array<string> = []
  walkObjects(view, (node) => {
    // (a) Any node carrying an `icon` prop must name a closed-catalog glyph —
    // a raw string that is not a glyph name can never ride an icon slot.
    if (typeof node.icon === "string" && !iconCatalog.has(node.icon)) {
      offenders.push(`icon "${node.icon}" is not in the closed @effect-native/core catalog`)
    }
    // (b) Icon nodes render glyphs only.
    if (node._tag === "Icon" && typeof node.name === "string" && !iconCatalog.has(node.name)) {
      offenders.push(`Icon name "${node.name}" is not in the closed catalog`)
    }
    // (c) IconButtons are glyph + accessible label; they never carry rendered
    // text content (the accessible label is announced, not painted).
    if (node._tag === "IconButton") {
      if (typeof node.icon !== "string" || node.icon.length === 0) {
        offenders.push(`IconButton ${String(node.key ?? "<unkeyed>")} carries no glyph`)
      }
      if (typeof node.accessibilityLabel !== "string" || node.accessibilityLabel.trim() === "") {
        offenders.push(`IconButton ${String(node.key ?? "<unkeyed>")} has no accessible label`)
      }
      if ("content" in node && node.content !== undefined) {
        offenders.push(`IconButton ${String(node.key ?? "<unkeyed>")} carries rendered text content`)
      }
    }
  })
  return offenders
}

type DockItem = { id?: string; icon?: unknown; label?: unknown; accessibilityLabel?: unknown }

export const dockItemOffenders = (items: ReadonlyArray<DockItem>): ReadonlyArray<string> => {
  const offenders: Array<string> = []
  for (const item of items) {
    const ref = item.id ?? "<no id>"
    if (typeof item.icon !== "string" || !iconCatalog.has(item.icon)) {
      offenders.push(`dock item ${ref} must carry a closed-catalog glyph, got ${JSON.stringify(item.icon)}`)
    }
    if (typeof item.label !== "string" || item.label.trim() === "") {
      offenders.push(`dock item ${ref} must carry a non-empty label`)
    } else {
      if (item.label.includes("\n")) offenders.push(`dock item ${ref} label must be single-line`)
      if (item.label.length > dockLabelMaxLength) {
        offenders.push(
          `dock item ${ref} label "${item.label}" exceeds the ${dockLabelMaxLength}-char micro-copy bound`,
        )
      }
    }
    if (typeof item.accessibilityLabel !== "string" || item.accessibilityLabel.trim() === "") {
      offenders.push(`dock item ${ref} must carry an accessible label`)
    }
  }
  return offenders
}

const baseState = (): DesktopShellState => initialDesktopShellState("electron/darwin", "18:00")

const sampledStates = (): ReadonlyArray<DesktopShellState> => [
  baseState(),
  { ...baseState(), commandPaletteOpen: true },
  {
    ...baseState(),
    harnessLanes: {
      fable: { available: true, reason: null },
      codex: { available: true, reason: null },
    },
  },
]

const workspaceDockItems = (view: unknown): ReadonlyArray<DockItem> => {
  let items: ReadonlyArray<DockItem> = []
  walkObjects(view, (node) => {
    if (node.key !== "sidebar-navigation") return
    const sections = node.sections as ReadonlyArray<{ id: string; items?: ReadonlyArray<DockItem> }>
    items = sections.find((section) => section.id === "sidebar-workspace-dock")?.items ?? []
  })
  return items
}

describe("openagents_desktop.microinteraction.icon_slot_no_raw_text.v1", () => {
  test("every workspace dock item is a catalog glyph plus bounded single-line micro-copy", () => {
    for (const state of sampledStates()) {
      const items = workspaceDockItems(desktopShellView(state))
      expect(items.length).toBeGreaterThan(0)
      expect(dockItemOffenders(items)).toEqual([])
    }
  })

  test("every icon slot across the sampled shell trees resolves in the closed catalog", () => {
    for (const state of sampledStates()) {
      expect(iconSlotOffenders(desktopShellView(state))).toEqual([])
    }
  })

  test("falsifier: raw text and unknown glyphs in icon slots are rejected", () => {
    expect(
      iconSlotOffenders({
        _tag: "IconButton",
        key: "bad-button",
        icon: "NotARealGlyph",
        accessibilityLabel: "",
        content: "click here to open the settings panel and configure things",
      }),
    ).toEqual([
      'icon "NotARealGlyph" is not in the closed @effect-native/core catalog',
      "IconButton bad-button has no accessible label",
      "IconButton bad-button carries rendered text content",
    ])
    expect(
      dockItemOffenders([
        {
          id: "bad-dock-item",
          icon: undefined,
          label: "This is a long stream of text where an icon should be",
          accessibilityLabel: "bad",
        },
      ]),
    ).toEqual([
      "dock item bad-dock-item must carry a closed-catalog glyph, got undefined",
      `dock item bad-dock-item label "This is a long stream of text where an icon should be" exceeds the ${dockLabelMaxLength}-char micro-copy bound`,
    ])
  })
})

// ---------------------------------------------------------------------------
// openagents_desktop.microinteraction.owner_review_register.v1
//
// The register: both concrete owner rules exist in the registry, enforced,
// with this suite as their oracle ref, both owner statements recorded
// verbatim, and the assurance clarification doc names both contractIds so the
// next owner rule has a documented home.
// ---------------------------------------------------------------------------

const ownerStatement1 =
  "I want the ones that we have for that for this app to include micro interactions and things that I do and don't want to see. There are some things I don't want to see, such as long streams of text where icons should be."
const ownerStatement2 =
  "I want all that enforced in the assurance pieces. I want to be able to specify rules there. For example, I don't want to see certain things like strings where icons should be, certain fonts, and that must be specified"

const registeredRuleIds = [
  "openagents_desktop.microinteraction.icon_slot_no_raw_text.v1",
  "openagents_desktop.typography.approved_fonts_only.v1",
] as const

describe("openagents_desktop.microinteraction.owner_review_register.v1", () => {
  test("both concrete owner rules are registered, enforced, and point at this suite", () => {
    for (const contractId of registeredRuleIds) {
      const contract = openAgentsDesktopUxContractRegistry.contracts.find(
        (candidate) => candidate.contractId === contractId,
      )
      expect(contract, contractId).toBeDefined()
      expect(contract?.state, contractId).toBe("enforced")
      expect(
        contract?.oracles.some((oracle) =>
          oracle.ref === "apps/openagents-desktop/tests/owner-ux-rules.test.ts"
        ),
        contractId,
      ).toBe(true)
    }
    const register = openAgentsDesktopUxContractRegistry.contracts.find(
      (candidate) =>
        candidate.contractId === "openagents_desktop.microinteraction.owner_review_register.v1",
    )
    expect(register?.state).toBe("enforced")
  })

  test("both owner statements are recorded verbatim in the registry", () => {
    const statements = openAgentsDesktopUxContractRegistry.contracts.map(
      (contract) => contract.statement,
    )
    expect(statements).toContain(ownerStatement1)
    expect(statements).toContain(ownerStatement2)
  })

  test("the assurance clarification doc exists and names both contract ids", () => {
    const doc = readFileSync(
      path.join(repoRoot, "docs", "assurance", "UX_CONTRACTS_AND_ASSURANCE.md"),
      "utf8",
    )
    for (const contractId of registeredRuleIds) expect(doc).toContain(contractId)
    expect(doc).toContain("openagents_desktop.microinteraction.owner_review_register.v1")
  })
})
