import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import { resolve } from "node:path"

const root = resolve(import.meta.dirname, "../../..")
const read = (path: string): string => readFileSync(resolve(root, path), "utf8")
const timeline = read("apps/openagents-desktop/src/renderer/react-timeline.tsx")
const styles = read("apps/openagents-desktop/src/renderer/react-workbench.css")
const shell = read("apps/openagents-desktop/src/renderer/react-primitive-adapters.tsx")
const gates = read("docs/mvp/openagents-desktop-mvp-phase-2-react-codex-workbench.assurance-gates.md")

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
    expect(styles).toContain('.oa-react-message-meta')
    expect(styles).toContain('opacity: 0')
  })

  test("binds active streaming state and the actual rail scroll viewport", () => {
    expect(shell).toContain('working={state.pending}')
    expect(styles).toContain('.oa-react-session-scroll')
    expect(styles).toContain('[data-slot="scroll-area-viewport"]')
    expect(styles).toContain('overscroll-behavior: contain')
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
})
