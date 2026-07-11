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
import { describe, expect, test } from "bun:test"
import { readFileSync, readdirSync } from "node:fs"
import path from "node:path"
import { radiusTokens, spacingTokens } from "@effect-native/tokens"

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

/**
 * Files allowed to carry raw color values. theme.ts is the single theme
 * module (it re-exports khalaTheme and is where any future desktop-scoped
 * color value would live); test files assert against literals.
 */
const colorAllowlist = new Set(["theme.ts"])

const rendererSources = (): ReadonlyArray<readonly [string, string]> =>
  readdirSync(rendererDir)
    .filter((name) => name.endsWith(".ts") && !name.endsWith(".test.ts"))
    .map((name) => [name, readFileSync(path.join(rendererDir, name), "utf8")] as const)

describe("design conformance (a): no raw color literals in renderer modules", () => {
  test("renderer .ts modules carry no hex/rgb/hsl color literals outside the theme module", () => {
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
    // And it actually consumes the token vocabulary.
    expect(css).toContain("var(--en-color-stateHover)")
    expect(css).toContain("var(--en-color-stateSelected)")
    expect(css).toContain("var(--en-color-textFaint)")
    expect(css).toContain("var(--en-elevation-overlay-shadow)")
    expect(css).toContain("var(--en-motion-fast)")
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

  test("numeric dimension literals in renderer style objects stay on the small documented allowlist", () => {
    // Dimensions (widths/heights/pane bounds) are schema-legal numbers; each
    // allowed value carries its reason here. Anything new must be added
    // deliberately — that is the point of the oracle.
    const allowed = new Set([
      840, // shared reading-measure column (columnWidth) + settings panel
      420, // command palette width
      360, // chat center pane minimum
      280, // inspector pane minimum
      480, // inspector pane maximum
      336, // inspector pane default size
      240, // files list minimum width / 240px output cap dimension
      320, // files list maximum width
      4, // sidebar connected-accounts usage meter: thin 4px track height (EP250)
    ])
    const offenders: Array<string> = []
    for (const [name, source] of rendererSources()) {
      for (const match of source.matchAll(
        /(?:minWidth|maxWidth|minHeight|maxHeight|width|height|size|min|max)\s*:\s*(\d+)/g,
      )) {
        const value = Number(match[1])
        if (value === 0 || value === 1) continue // minWidth: 0 / flex guards / hairlines
        if (!allowed.has(value)) offenders.push(`${name}: ${match[0]}`)
      }
    }
    expect(offenders).toEqual([])
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
    const chord = byKey(view, "desktop-command-chord-chat.new") as { content?: unknown }
    expect(chord?.content).toBe("⌘N")
    // Item rows are ghost buttons on the nested-radius rule (outer xl 8 −
    // 6px gutter -> sm 2).
    const row = byKey(view, "desktop-command-chat.new") as { variant?: string; style?: Record<string, unknown> }
    expect(row?.variant).toBe("ghost")
    expect(row?.style).toMatchObject({ borderRadius: "sm" })
  })

  test("composer: radius capped at xl; recessed segmented harness track with elevated selected thumb", () => {
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
    const track = byKey(view, "shell-harness-row") as { style?: Record<string, unknown>; gap?: string }
    expect(track?.style).toMatchObject({
      backgroundColor: "background",
      borderRadius: "lg",
      padding: "0.5",
    })
    const selected = byKey(view, "shell-harness-codex") as { style?: Record<string, unknown> }
    expect(selected?.style).toMatchObject({ backgroundColor: "surfaceRaised", borderRadius: "md" })
    const idle = byKey(view, "shell-harness-fable") as { style?: Record<string, unknown> }
    expect(idle?.style).toMatchObject({ borderRadius: "md", color: "textMuted" })
  })

  test("disabled-control reason popover: a disabled harness chip with a reason is wrapped in a Tooltip carrying that exact reason", () => {
    const state = baseState() // codex lane starts unavailable with a reason
    const view = desktopShellView(state)
    const tooltip = byKey(view, "shell-harness-codex-reason") as {
      _tag?: string
      content?: string
      children?: ReadonlyArray<AnyNode>
    }
    expect(tooltip?._tag).toBe("Tooltip")
    expect(tooltip?.content).toBe(state.harnessLanes.codex.reason ?? "")
    expect(tooltip?.children?.[0]?.key).toBe("shell-harness-codex")
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
    expect(byKey(availableView, "shell-harness-codex-reason")).toBeUndefined()
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
