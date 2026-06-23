// #6046: theme-preserving evidence for the StyleX → plain-CSS migration.
//
// The desktop shell/pane chrome moved out of the deleted StyleX module
// `src/ui/desktop-stylex.ts` and into plain CSS in `src/ui/styles.css`, keyed by
// the literal class names the view emits. This test pins the load-bearing
// declarations to the EXACT values the StyleX styles produced, so any drift
// (a changed color, radius, shadow, z-index, or geometry) fails loudly.
//
// Every ported color/metric is `var(--oa-token, <original>)`; the design-tokens
// package separately pins each `--oa-*` token to the same literal as the
// fallback, so resolved-or-fallback renders identically.

import { readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"

const css = readFileSync(join(process.cwd(), "src/ui/styles.css"), "utf8")

const ruleBody = (selector: string): string => {
  // Grab the first `{ … }` block for a selector that starts a rule line.
  const re = new RegExp(
    `(^|\\n)\\s*${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]*)\\}`,
  )
  const match = re.exec(css)
  if (match === null) throw new Error(`no rule found for selector: ${selector}`)
  return match[2]
}

describe("#6046 StyleX removal is theme-preserving", () => {
  test("app-shell roots keep the black background", () => {
    const body = ruleBody(".app-shell-shell,\n.app-shell-network")
    expect(body).toContain("background-color: var(--oa-color-bg, #000)")
    expect(body).toContain("position: fixed")
    expect(body).toContain("inset: 0")
  })

  test("shell-input keeps its exact radius, border and focus chrome", () => {
    const body = ruleBody(".shell-input")
    // 10px radius (var fallback equals the original StyleX `borderRadius: 10px`).
    expect(body).toContain("border-radius: var(--oa-radius-2xl, 10px)")
    expect(body).toContain("border: 1px solid rgb(255 255 255 / 0.12)")
    expect(body).toContain("background-color: rgb(255 255 255 / 0.04)")
    expect(body).toContain("padding: 0.75rem 1rem")
    // The focus ring uses the SAME color-mix the StyleX `:focus` produced.
    expect(css).toContain(
      ".shell-input:focus {\n  border-color: color-mix(in srgb, var(--hud-primary) 32%, transparent);",
    )
  })

  test("pane-window keeps its 12px radius, pane shadow and motion", () => {
    const body = ruleBody(".pane-window")
    expect(body).toContain("border-radius: var(--oa-radius-3xl, 12px)")
    expect(body).toContain("background-color: rgb(6 9 13 / 0.96)")
    expect(body).toContain(
      "box-shadow: var(--oa-shadow-pane, 0 24px 64px rgb(0 0 0 / 0.6))",
    )
    expect(body).toContain("transition-duration: var(--oa-motion-fast, 0.14s)")
    expect(body).toContain("transition-timing-function: var(--oa-motion-easing, ease)")
  })

  test("pane-layer keeps its z-index and pointer behavior", () => {
    const body = ruleBody(".pane-layer")
    expect(body).toContain("z-index: var(--oa-z-pane-layer, 30)")
    expect(body).toContain("pointer-events: none")
  })

  test("shell-stream-part keeps the exact grid template and border accent", () => {
    const body = ruleBody(".shell-stream-part")
    expect(body).toContain(
      "grid-template-columns: minmax(4.5rem, 6rem) minmax(0, 1fr)",
    )
    expect(body).toContain("border: 1px solid rgb(255 255 255 / 0.12)")
    expect(body).toContain("border-left-color: rgb(255 255 255 / 0.34)")
    expect(body).toContain("border-radius: var(--oa-radius-md, 4px)")
  })

  test("the bottom-right resize handle keeps its exact geometry", () => {
    // Corner handles share a 12x12 size rule, then each corner sets its own
    // offset/cursor in a standalone rule. Both must preserve the StyleX values.
    expect(css).toContain(".pane-window-resize-bottomright {\n  bottom: -4px;\n  right: -4px;\n  cursor: nwse-resize;\n}")
    expect(css).toMatch(/\.pane-window-resize-bottomright[\s\S]*?\{[\s\S]*?width: 12px;[\s\S]*?height: 12px;/)
  })

  test("every ported color metric resolves to a fallback equal to its token", () => {
    // The fallbacks below are the literal values the StyleX module used; the
    // design-tokens package pins the matching --oa-* token to the same literal,
    // so the migration cannot silently change a value.
    const pairs: ReadonlyArray<readonly [string, string]> = [
      ["--oa-color-bg", "#000"],
      ["--oa-radius-md", "4px"],
      ["--oa-radius-2xl", "10px"],
      ["--oa-radius-3xl", "12px"],
      ["--oa-shadow-pane", "0 24px 64px rgb(0 0 0 / 0.6)"],
      ["--oa-z-pane-layer", "30"],
      ["--oa-z-return-button", "9999"],
      ["--oa-motion-fast", "0.14s"],
      ["--oa-motion-easing", "ease"],
    ]
    for (const [token, value] of pairs) {
      expect(css).toContain(`var(${token}, ${value})`)
    }
  })
})
