import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import { resolve } from "node:path"

const root = resolve(import.meta.dirname, "../../..")
const read = (path: string): string => readFileSync(resolve(root, path), "utf8")
const timeline = read("apps/openagents-desktop/src/renderer/react-timeline.tsx")
const styles = read("apps/openagents-desktop/src/renderer/react-workbench.css")
const shell = read("apps/openagents-desktop/src/renderer/react-primitive-adapters.tsx")
const releaseAcceptance = read("apps/openagents-desktop/scripts/run-release-acceptance.ts")
const gates = read("docs/mvp/openagents-desktop-mvp-phase-2-react-codex-workbench.assurance-gates.md")

const transcriptHoverGeometryViolations = (css: string): ReadonlyArray<string> => {
  const geometry = /(?:^|;)\s*(?:display|height|min-height|max-height|width|min-width|max-width|margin(?:-[a-z]+)?|padding(?:-[a-z]+)?|border(?:-[a-z]+)?-width|font-size|line-height|gap|grid-template(?:-[a-z]+)?|flex-basis)\s*:/imu
  return [...css.matchAll(/([^{}]+)\{([^{}]*)\}/gu)]
    .filter(match => /\.oa-react-timeline-item(?::hover|:focus-within)/u.test(match[1] ?? "") && geometry.test(match[2] ?? ""))
    .map(match => (match[1] ?? "").trim())
}

describe("revision 3 conversation-first assurance gates", () => {
  test("suppresses accounting and scaffolding from the primary history projection", () => {
    expect(timeline).toContain('["session", "context", "metadata", "usage"]')
    expect(timeline).toContain('item.kind === "reasoning" && item.redacted')
    expect(timeline).not.toContain('record.kind === "usage" && record.fields.length')
  })

  test("keeps authored messages primary and raw work collapsed", () => {
    expect(timeline).toContain('data-tone={isUserRecord(record) ? "user" : "assistant"}')
    expect(timeline).toContain('<details className="oa-react-work-entry"')
    expect(timeline).toContain('"Worked"')
    expect(timeline).toContain('aria-label="Codex is working"')
    expect(timeline).not.toContain('oa-react-message-meta')
    expect(styles).not.toContain('.oa-react-message-meta')
    expect(transcriptHoverGeometryViolations(styles)).toEqual([])
  })

  test("falsifier: hover-revealed transcript content may not change readable layout geometry", () => {
    expect(transcriptHoverGeometryViolations(`
      .oa-react-timeline-item:hover .hidden-meta { opacity: 1; height: auto; margin-bottom: 4px; }
    `)).toEqual([".oa-react-timeline-item:hover .hidden-meta"])
    expect(transcriptHoverGeometryViolations(`
      .oa-react-timeline-item:hover .stable-control { opacity: 1; color: white; }
    `)).toEqual([])
  })

  test("binds active streaming state and the actual rail scroll viewport", () => {
    expect(shell).toContain('working={state.pending}')
    expect(styles).toContain('.oa-react-session-scroll')
    expect(styles).toContain('[data-slot="scroll-area-viewport"]')
    expect(styles).toContain('overscroll-behavior: contain')
  })

  test("keeps the React composer centered despite compatibility-host selectors and the collapsed rail control clear of macOS chrome", () => {
    expect(styles).toContain('width: min(760px, calc(100% - var(--en-spacing-6))) !important')
    expect(styles).toContain('margin: var(--en-spacing-3) auto var(--en-spacing-4) !important')
    expect(styles).toContain('html[data-desktop-platform="darwin"] .oa-react-workbench[data-rail-collapsed="true"] .oa-react-sidebar-expand')
    expect(styles).toContain('left: 92px')
    expect(styles).toContain('z-index: 20')
    expect(styles).toContain('width: 24px')
    expect(styles).toContain('-webkit-app-region: no-drag')
  })

  test("requires reduced motion and preserves the canonical token boundary", () => {
    expect(styles).toContain('@media (prefers-reduced-motion: reduce)')
    expect(styles).toContain('var(--en-color-background)')
    expect(styles).toContain('var(--en-color-textPrimary)')
    expect(styles).not.toMatch(/#[0-9a-f]{3,8}\b/i)
  })

  test("documents every owner-directed hierarchy and release gate", () => {
    for (const phrase of [
      "Authored messages are primary",
      "The transcript is not an event ledger",
      "Internal work is compact",
      "Settled work folds; active work stays legible",
      "Streaming is stable",
      "The session rail scrolls at its real boundary",
      "Khala color authority is preserved",
      "RC15-to-RC16 update",
    ]) expect(gates).toContain(phrase)
  })

  test("release reinstall detaches the whole DMG device before reattachment", () => {
    expect(releaseAcceptance).toContain('attach.match(/^(\\/dev\\/disk\\d+)\\b/m)')
    expect(releaseAcceptance).toContain('["detach", device]')
    expect(releaseAcceptance).toContain('["detach", "-force", device]')
  })
})
