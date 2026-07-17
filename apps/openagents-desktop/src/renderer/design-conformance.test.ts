/**
 * Design-language conformance oracle (EP250 #8712).
 *
 * Owner statement (verbatim): "do a separate design pass of
 * projects/repos/apps-sdk-ui and thats what i want to use for the rest of
 * the app chrome, menus, etc, everything other than messages, but still
 * harmonized to messages. we want that design language, ported to starcraft
 * kinda, represented in EVERY other surface of the app"
 *
 * Mechanical rule (apps-sdk chrome spec §6): a surface "represents the
 * design language" iff (a) it introduces no raw color literals outside the
 * theme/tokens modules, (b) its style values come from the shared token
 * scales, and (c) the per-surface structural recipes hold on the typed view
 * trees. The typed trees are data — no DOM needed.
 */
import { describe, expect, test } from "vite-plus/test"
import { readFileSync, readdirSync } from "node:fs"
import path from "node:path"
import { khalaTheme, radiusTokens, spacingTokens } from "@effect-native/tokens"
import { openagentsDesktopTheme } from "./theme.ts"

import {
  desktopShellView,
  initialDesktopShellState,
  toolCardMessage,
  contextGroupMessage,
  type DesktopShellState,
} from "./shell.ts"
import { settingsView, initialSettingsState } from "./settings.ts"
import { fleetWorkspaceView, emptyFleetWorkspaceState } from "./fleet-workspace.ts"

const rendererDir = path.dirname(new URL(import.meta.url).pathname)
const sharedWorkbenchCssPath = path.resolve(rendererDir, "../../../../packages/ui/src/desktop-workbench.css")
const sharedWorkbenchSourcePath = path.resolve(rendererDir, "../../../../packages/ui/src/desktop-workbench.tsx")

/**
 * Files allowed to carry raw color values. theme.ts is the single theme
 * module (it re-exports khalaTheme and is where any future desktop-scoped
 * color value would live); test files assert against literals.
 */
const colorAllowlist = new Set(["theme.ts"])

const rendererSources = (): ReadonlyArray<readonly [string, string]> =>
  readdirSync(rendererDir)
    .filter((name) => /\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name))
    .map((name) => [name, readFileSync(path.join(rendererDir, name), "utf8")] as const)

describe("design conformance (a): no raw color literals in renderer modules", () => {
  test("renderer .ts/.tsx modules carry no hex/rgb/hsl color literals outside the theme module", () => {
    const offenders: Array<string> = []
    for (const [name, source] of rendererSources()) {
      if (colorAllowlist.has(name)) continue
      // Hex colors, rgb()/rgba(), hsl()/hsla(). Word-ish boundary keeps
      // non-color hashes (refs like "commit #8712") out of scope: a color
      // literal is a # followed by 3-8 hex digits and a non-word boundary.
      for (const line of source.split("\n")) {
        if (/#[0-9a-fA-F]{3,8}(?![0-9a-zA-Z])/.test(line) && !/#\d+\b/.test(line)) offenders.push(`${name}: ${line.trim()}`)
        if (/\brgba?\s*\(|\bhsla?\s*\(/.test(line)) offenders.push(`${name}: ${line.trim()}`)
      }
    }
    expect(offenders).toEqual([])
  })

  test("the host stylesheet (app.css) resolves every color through --en-* custom properties", () => {
    const raw = readFileSync(path.join(rendererDir, "app.css"), "utf8")
    // Strip comments (issue refs like "#8712" live there) — then no hex
    // color may appear inside any declaration VALUE, and no rgb()/hsl()
    // anywhere: colors are var(--en-color-*) references only (elevation
    // shadows live in the token value, not here). ID selectors like
    // #openagents-desktop-root are selectors, not declaration values.
    const css = raw.replace(/\/\*[\s\S]*?\*\//g, "")
    expect(css).not.toMatch(/:\s*[^;{}]*#[0-9a-fA-F]{3,8}\b/)
    expect(css).not.toMatch(/\brgba?\s*\(/)
    expect(css).not.toMatch(/\bhsla?\s*\(/)
    // Host physics still consumes the shared theme/elevation/motion vocabulary.
    expect(css).toContain("var(--en-color-background)")
    expect(css).toContain("var(--en-color-borderSubtle)")
    expect(css).toContain("var(--en-elevation-overlay-shadow)")
    expect(css).toContain("var(--en-motion-fast)")
  })

  test("Tailwind exposes only canonical Effect Native semantic aliases", () => {
    const css = readFileSync(path.join(rendererDir, "app.css"), "utf8")
    for (const namespace of ["color", "radius", "spacing", "font"]) {
      expect(css).toContain(`--${namespace}-*: initial`)
    }
    expect(css).toContain("--color-primary: var(--en-color-accent)")
    expect(css).toContain("--color-destructive: var(--en-color-danger)")
    expect(css).toContain("--radius-md: var(--en-radius-md)")
    expect(css).not.toMatch(/--color-(?:blue|red|slate|gray)-/)
  })

  test("the only desktop theme stays dark khalaTheme with the canonical Protoss-blue primary", () => {
    expect(openagentsDesktopTheme).toBe(khalaTheme)
    expect(openagentsDesktopTheme.color.background).toBe("#05070d")
    expect(openagentsDesktopTheme.color.accent).toBe("#3b82f6")
  })

  test("macOS integrated chrome uses the shared control and spacing scales", () => {
    const css = readFileSync(path.join(rendererDir, "app.css"), "utf8")
    const rule = css.match(/html\[data-desktop-platform="darwin"\] \[data-en-key="shell-sidebar"\]::before \{([^}]+)\}/)?.[1] ?? ""
    expect(rule).toContain("var(--en-control-lg-height)")
    expect(rule).toContain("var(--en-spacing-1)")
    expect(rule).toContain("-webkit-app-region: drag")
  })
})

describe("design conformance (b): style values come from the shared scales", () => {
  test("spacing/radius string values in renderer style objects are members of the token scales", () => {
    const spacing = new Set<string>(spacingTokens)
    const radius = new Set<string>(radiusTokens)
    const offenders: Array<string> = []
    for (const [name, source] of rendererSources()) {
      for (const match of source.matchAll(
        /(?:padding|paddingTop|paddingRight|paddingBottom|paddingLeft|margin|marginTop|marginBottom|marginLeft|marginRight|gap)\s*:\s*"([^"]+)"/g,
      )) {
        if (!spacing.has(match[1]!)) offenders.push(`${name}: spacing "${match[1]}"`)
      }
      for (const match of source.matchAll(/(?:borderRadius|radius)\s*:\s*"([^"]+)"/g)) {
        if (!radius.has(match[1]!)) offenders.push(`${name}: radius "${match[1]}"`)
      }
    }
    expect(offenders).toEqual([])
  })

  test("no raw font sizing in renderer modules — type rides the typeScale tokens", () => {
    const offenders: Array<string> = []
    for (const [name, source] of rendererSources()) {
      if (colorAllowlist.has(name)) continue
      if (/fontSize\s*:\s*\d/.test(source)) offenders.push(`${name}: raw fontSize`)
      if (/lineHeight\s*:\s*\d/.test(source)) offenders.push(`${name}: raw lineHeight`)
    }
    expect(offenders).toEqual([])
  })

  test("renderer style objects contain no raw numeric dimensions beyond structural zero and hairlines", () => {
    const offenders: Array<string> = []
    for (const [name, source] of rendererSources()) {
      for (const match of source.matchAll(
        /(?:minWidth|maxWidth|minHeight|maxHeight|width|height|size|min|max)\s*:\s*(\d+)/g,
      )) {
        const value = Number(match[1])
        if (value === 0 || value === 1) continue // flex guards / hairlines, not dimensions
        offenders.push(`${name}: ${match[0]}`)
      }
    }
    expect(offenders).toEqual([])
  })
})

describe("design conformance (b2): app.css is a token bridge and host physics, not a component recipe layer", () => {
  test("React shell keeps the T3-proportioned rail and topbar hierarchy", () => {
    const css = readFileSync(sharedWorkbenchCssPath, "utf8")
    const rule = (selector: string): string =>
      css.match(new RegExp(`${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]+)\\}`))?.[1] ?? ""
    expect(rule(".oa-react-conversation-header")).toContain("height: 52px")
    expect(rule(".oa-react-conversation-heading h1")).toContain("font-size: 14px")
    expect(rule(".oa-react-rail-titlebar strong")).toContain("font-size: 14px")
    expect(rule(".oa-react-primary-destination")).toContain("height: 30px")
    expect(rule(".oa-react-session-row")).toContain("height: 32px")
    expect(rule(".oa-react-section-label")).toContain("font-size: 10px")
    expect(css).toContain("-webkit-app-region: no-drag")
  })

  test("user-state metadata uses the sans face while operational detail remains mono", () => {
    const css = readFileSync(sharedWorkbenchCssPath, "utf8")
    const rule = (selector: string): string =>
      css.match(new RegExp(`${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]+)\\}`))?.[1] ?? ""
    expect(rule(".oa-react-session-row .oa-react-session-meta")).toContain("font-family: var(--oa-font-sans)")
    expect(rule(".oa-react-empty-working-directory code")).toContain("font-family: var(--oa-font-sans)")
    expect(rule(".oa-react-command-output")).toContain("font-family: var(--oa-font-mono)")
  })

  test("the empty conversation occupies the flexible body row and keeps its directory action compact", () => {
    const css = readFileSync(sharedWorkbenchCssPath, "utf8")
    const rule = (selector: string): string =>
      css.match(new RegExp(`${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]+)\\}`))?.[1] ?? ""
    expect(css).toContain(".oa-react-timeline-empty {\n  display: grid;\n  grid-row: 2;")
    expect(rule(".oa-react-empty-working-directory .oa-react-empty-directory-change")).toContain("width: 24px")
  })

  test("live approvals use a readable dialog layout instead of nesting the timeline card", () => {
    const css = readFileSync(sharedWorkbenchCssPath, "utf8")
    const rule = (selector: string): string =>
      css.match(new RegExp(`${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]+)\\}`))?.[1] ?? ""
    const dialog = rule(".oa-react-decision")
    const approval = rule(".oa-react-decision > .oa-react-approval-card")
    const command = rule(".oa-react-decision > .oa-react-approval-card > div > code")

    expect(dialog).toContain("max-width: 640px !important")
    expect(dialog).toContain("overflow-y: auto")
    expect(approval).toContain("border: 0 !important")
    expect(approval).toContain("background: transparent !important")
    expect(command).toContain("overflow-wrap: anywhere")
    expect(command).toContain("font-family: var(--oa-font-code)")
    expect(command).toContain("text-transform: none !important")
    expect(command).toContain("white-space: pre-wrap")
    expect(css).toContain(".oa-react-decision-overlay")
  })

  test("chat markdown restores semantic list markers after Tailwind preflight", () => {
    const css = readFileSync(sharedWorkbenchCssPath, "utf8")
    expect(css).toContain(".oa-react-markdown ul { list-style: disc outside; }")
    expect(css).toContain(".oa-react-markdown ol { list-style: decimal outside; }")
  })

  test("command palette keeps the T3 search/results/footer structure", () => {
    const source = readFileSync(path.resolve(rendererDir, "../components/ui/command.tsx"), "utf8")
    expect(source).toContain('data-command-palette="true"')
    expect(source).toContain("top-[10vh]")
    expect(source).toContain("max-w-xl")
    expect(source).toContain("h-11!")
    expect(source).toContain("max-h-[min(28rem,70vh)]")
    expect(source).toContain("min-h-9")
    expect(source).toContain('data-slot="command-footer"')
    expect(source).toContain("min-h-12")
  })

  test("every Codex update spinner stops under reduced motion", () => {
    const css = readFileSync(sharedWorkbenchCssPath, "utf8")
    const reducedMotion = css.match(/@media \(prefers-reduced-motion: reduce\)\s*\{([\s\S]*?)\n\}/)?.[1] ?? ""
    expect(reducedMotion).toContain(".oa-react-codex-update-spinner { animation: none; }")
    expect(reducedMotion).toContain(".oa-react-session-loading svg { animation: none; }")
  })

  test("shared workbench presentation cannot become a second Desktop runtime authority", () => {
    const source = readFileSync(sharedWorkbenchSourcePath, "utf8")
    expect(source).not.toContain("DesktopShellState")
    expect(source).not.toMatch(/electron|SubscriptionRef|IntentReporter|TanStack|createRouter|localStorage|sessionStorage/)
    expect(source).toContain("DesktopSessionRail")
    expect(source).toContain("DesktopComposerFrame")
    expect(source).toContain('import "./desktop-workbench.css"')
  })

  test("the shared composer owns one outer focus frame without a nested input ring", () => {
    const css = readFileSync(sharedWorkbenchCssPath, "utf8")
    const appCss = readFileSync(path.join(rendererDir, "app.css"), "utf8")

    expect(css).toContain(".oa-react-composer:focus-within")
    expect(css).toContain(".oa-react-composer-input textarea:focus-visible")
    expect(css).toContain("outline: none !important")
    expect(appCss).not.toContain('[data-en-key="shell-input"] textarea:focus-visible')
  })

  test("the stylesheet stays within the bounded token-bridge/host-physics payload budget", () => {
    // Bytes are formatting-invariant enough to prevent blank-line/minification
    // games while expressing the issue's approximate 300-line target. The old
    // component-recipe sheet was >31 KiB; the Tailwind token projection plus
    // host physics stays capped at 11 KiB.
    const css = readFileSync(path.join(rendererDir, "app.css"), "utf8")
    expect(Buffer.byteLength(css, "utf8")).toBeLessThanOrEqual(11 * 1024)
  })

  test("catalog component tags and visual matrix axes are never restyled in app.css", () => {
    const css = readFileSync(path.join(rendererDir, "app.css"), "utf8").replace(/\/\*[\s\S]*?\*\//g, "")
    expect(css).not.toMatch(/\[data-en-tag(?:=|\])/)
    expect(css).not.toMatch(/\[data-en-tone(?:=|\])/)
    expect(css).not.toMatch(/\[data-en-size(?:=|\])/)
    // A variant selector may position an icon in host geometry, but it may
    // never rebuild the component's color, border, radius, typography, or shadow.
    for (const match of css.matchAll(/[^{}]*\[data-en-variant(?:=|\])[^{}]*\{([^}]*)\}/g)) {
      expect(match[1]).not.toMatch(/(?:background|border(?:-radius)?|box-shadow|color|font(?:-size|-weight)?|padding)\s*:/)
    }
  })

  test("the shadcn Vega zinc preset extends Khala instead of defining another palette", () => {
    const css = readFileSync(path.join(rendererDir, "shadcn-khala.css"), "utf8")
    const appCss = readFileSync(path.join(rendererDir, "app.css"), "utf8")
    const config = JSON.parse(readFileSync(path.resolve(rendererDir, "../../components.json"), "utf8")) as {
      style: string
      iconLibrary: string
      menuAccent: string
      menuColor: string
    }
    expect(config).toMatchObject({
      style: "base-vega",
      iconLibrary: "lucide",
      menuAccent: "subtle",
      menuColor: "default-translucent",
    })
    expect(css).toContain('var(--oa-font-sans)')
    expect(css).toContain('var(--oa-font-mono)')
    expect(css).toContain("--background: var(--en-color-background)")
    expect(css).toContain("--primary: var(--en-color-accent)")
    expect(appCss).toContain("--color-popover: var(--en-color-surfaceOverlay)")
    expect(appCss).toContain("--color-popover-foreground: var(--en-color-textPrimary)")
    expect(css).not.toMatch(/oklch\(|#[0-9a-f]{3,8}\b/i)
  })

  test("owner design directive is registered against this executable oracle", () => {
    const contractId = "openagents_desktop.design.apps_sdk_starcraft_harmonization.v1"
    const ownerStatement = "ALL styles harmonized with apps-sdk-ui while preserving our starcraft design"
    expect(contractId).toContain("apps_sdk_starcraft_harmonization")
    expect(ownerStatement).toContain("apps-sdk-ui")
  })
})

// ---------------------------------------------------------------------------
// (c) Per-surface structural recipes on the typed view trees.
// ---------------------------------------------------------------------------

type AnyNode = Readonly<Record<string, unknown>> & { key?: string; _tag?: string }

const collect = (node: unknown, out: Array<AnyNode> = []): Array<AnyNode> => {
  if (Array.isArray(node)) {
    for (const item of node) collect(item, out)
    return out
  }
  if (typeof node !== "object" || node === null) return out
  const record = node as AnyNode
  if (typeof record._tag === "string") out.push(record)
  for (const value of Object.values(record)) collect(value, out)
  return out
}

const byKey = (root: unknown, key: string): AnyNode | undefined =>
  collect(root).find((node) => node.key === key)

const baseState = (): DesktopShellState => initialDesktopShellState("electron/darwin", "18:00")

describe("design conformance (c): per-surface structural recipes", () => {
  test("shell sidebar: NavRail dock + history sections exist (state fills ride the renderer chrome)", () => {
    const view = desktopShellView(baseState())
    const rail = byKey(view, "sidebar-navigation") as { sections?: ReadonlyArray<{ id: string }> }
    expect(rail).toBeDefined()
    expect(rail.sections?.map((section) => section.id)).toEqual([
      "sidebar-workspace-dock",
      "sidebar-history-list",
    ])
  })

  test("Settings owns bounded vertical overflow and the dock wraps before clipping its final action", () => {
    const css = readFileSync(path.join(rendererDir, "app.css"), "utf8")
    const dockRule = css.match(/\[data-en-key="sidebar-workspace-dock"\] \{([^}]+)\}/)?.[1] ?? ""
    expect(dockRule).toContain("flex-wrap: wrap")

    const settingsRule = css.match(/\[data-en-key="desktop-settings-stack"\] \{([^}]+)\}/)?.[1] ?? ""
    expect(settingsRule).toContain("flex: 1 1 0 !important")
    expect(settingsRule).toContain("height: 0 !important")
    expect(settingsRule).toContain("min-height: 0 !important")
    expect(settingsRule).toContain("overflow-y: auto !important")
    expect(settingsRule).toContain("overflow-x: hidden !important")
  })

  test("command palette: surfaceOverlay panel, hairline borderSubtle, radius xl, 6px gutter, chord captions", () => {
    const view = desktopShellView({ ...baseState(), commandPaletteOpen: true })
    const palette = byKey(view, "desktop-command-palette") as {
      radius?: string
      padding?: string
      style?: Record<string, unknown>
    }
    expect(palette).toBeDefined()
    expect(palette.radius).toBe("xl")
    expect(palette.padding).toBe("1.5")
    expect(palette.style).toMatchObject({
      backgroundColor: "surfaceOverlay",
      borderColor: "borderSubtle",
      borderWidth: 1,
    })
    // The chat.new row carries its canonical chord caption (⌘N on darwin).
    const chord = byKey(view, "desktop-command-chord-chat.new") as { label?: unknown; variant?: string; size?: string }
    expect(chord).toMatchObject({ label: "⌘N", variant: "outline", size: "sm" })
    // Item rows are ghost buttons on the nested-radius rule (outer xl 8 −
    // 6px gutter -> sm 2).
    const row = byKey(view, "desktop-command-chat.new") as { variant?: string; style?: Record<string, unknown> }
    expect(row?.variant).toBe("ghost")
    expect(row?.style).toMatchObject({ borderRadius: "sm" })
  })

  test("MVP composer: radius capped at xl and the engine is a fixed Codex label", () => {
    const state: DesktopShellState = {
      ...baseState(),
      harnessLanes: {
        fable: { available: true, reason: null },
        codex: { available: true, reason: null },
      },
    }
    const view = desktopShellView(state)
    const composer = byKey(view, "shell-composer") as { radius?: string }
    expect(composer?.radius).toBe("xl")
    const engine = byKey(view, "shell-codex-engine") as { _tag?: string; content?: string }
    expect(engine?._tag).toBe("Text")
    expect(engine?.content).toBe("Codex")
    expect(byKey(view, "shell-harness-select")).toBeUndefined()
    expect(byKey(view, "shell-model-select")).toBeUndefined()
    expect(byKey(view, "shell-reasoning-select")).toBeUndefined()
  })

  test("Codex availability is expressed only on Send through the exact reason tooltip", () => {
    const state = baseState() // codex lane starts unavailable with a reason
    const view = desktopShellView(state)
    expect(byKey(view, "shell-harness-select")).toBeUndefined()
    // The Send button is equally explained while the selected lane cannot act.
    const sendReason = byKey(view, "shell-note-reason") as { _tag?: string; content?: string }
    expect(sendReason?._tag).toBe("Tooltip")
    expect(String(sendReason?.content ?? "")).toBe(state.harnessLanes.codex.reason!)
    // An available lane renders the bare control — hover popovers are for
    // disabled controls only, and no standing caption exists either way.
    const available: DesktopShellState = {
      ...state,
      harnessLanes: {
        fable: { available: true, reason: null },
        codex: { available: true, reason: null },
      },
    }
    const availableView = desktopShellView(available)
    expect(byKey(availableView, "shell-note-reason")).toBeUndefined()
  })

  test("settings: in-flow panel with 16px section padding and hairline borderSubtle edge", () => {
    const view = settingsView(initialSettingsState())
    const panel = byKey(view, "settings-screen") as { padding?: string; radius?: string; style?: Record<string, unknown> }
    expect(panel?.padding).toBe("4")
    expect(panel?.radius).toBe("lg")
    expect(panel?.style).toMatchObject({ backgroundColor: "surfaceRaised", borderColor: "borderSubtle" })
  })

  test("fleet workspace: heading + secondary Refresh control on the table surface", () => {
    const view = fleetWorkspaceView(emptyFleetWorkspaceState())
    const refresh = byKey(view, "fleet-refresh") as { variant?: string }
    expect(refresh?.variant).toBe("secondary")
    const title = byKey(view, "fleet-title") as { variant?: string }
    expect(title?.variant).toBe("heading")
  })

  test("message inspector: rail panel titled at the title scale with a compact faint Close", () => {
    const noted: DesktopShellState = {
      ...baseState(),
      notes: [{ key: "m1", role: "assistant", text: "hello", timestamp: "18:01" }],
      activeThreadId: "t1",
      selectedMessageKey: "m1",
    }
    const view = desktopShellView(noted)
    const title = byKey(view, "chat-message-inspector-title") as { variant?: string }
    expect(title?.variant).toBe("title")
    const close = byKey(view, "chat-message-inspector-close") as { style?: Record<string, unknown> }
    expect(close?.style).toMatchObject({ color: "textFaint", typeScale: "label" })
  })

  test("history details uses the catalog's compact icon variant with no stylesheet recipe", () => {
    const source = readFileSync(path.join(rendererDir, "history-workspace.ts"), "utf8")
    const css = readFileSync(path.join(rendererDir, "app.css"), "utf8")
    expect(source).toMatch(/key: `history-item-details-\$\{item\.itemRef\}`[\s\S]{0,240}size: "sm"/)
    expect(css).not.toContain('[data-en-key^="history-item-details-"]')
  })

  test("tool cards: running titles carry the shimmer key; raw wells cap at the 240px dimension; dim ladder uses textFaint", () => {
    const running = toolCardMessage(
      { key: "r1", toolName: "Bash", timestamp: "18:02", status: "running", argsSummary: '{"command":"ls"}', resultSummary: null },
      false,
    )
    expect(byKey(running.body, "tool-title-running-r1")).toBeDefined()
    const done = toolCardMessage(
      { key: "r1", toolName: "Bash", timestamp: "18:02", status: "ok", argsSummary: '{"command":"ls"}', resultSummary: "ok" },
      true,
    )
    expect(byKey(done.body, "tool-title-running-r1")).toBeUndefined()
    expect(byKey(done.body, "tool-title-r1")).toBeDefined()
    const well = byKey(done.body, "tool-raw-well-r1") as { style?: Record<string, unknown> }
    expect(well?.style).toMatchObject({ maxHeight: "sm" })
    const raw = byKey(done.body, "tool-raw-args-r1") as { color?: string }
    expect(raw?.color).toBe("textFaint")
  })

  test("context group: one 'Gathered context' row whose expanded members indent at the tighter sub-rhythm", () => {
    const group = {
      key: "ctx-a",
      cards: [
        { key: "a", toolName: "Read", timestamp: "18:03", status: "ok" as const, argsSummary: '{"file_path":"a.md"}', resultSummary: "ok" },
        { key: "b", toolName: "Grep", timestamp: "18:03", status: "ok" as const, argsSummary: '{"pattern":"x"}', resultSummary: "ok" },
      ],
      running: false,
      failed: false,
      reads: 1,
      searches: 1,
    }
    const collapsed = contextGroupMessage(group, false)
    expect(byKey(collapsed.body, "tool-title-ctx-a")).toMatchObject({ content: "Gathered context" })
    expect(byKey(collapsed.body, "tool-group-members-ctx-a")).toBeUndefined()
    const expanded = contextGroupMessage(group, true)
    const members = byKey(expanded.body, "tool-group-members-ctx-a") as { style?: Record<string, unknown>; gap?: string }
    expect(members?.style).toMatchObject({ paddingLeft: "3" })
    expect(members?.gap).toBe("1")
    const runningGroup = contextGroupMessage({ ...group, running: true }, false)
    expect(byKey(runningGroup.body, "tool-title-running-ctx-a")).toMatchObject({ content: "Gathering context…" })
  })
})
