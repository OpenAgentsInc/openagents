import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { join } from "node:path"
import { initialModel, Model, PaneId } from "../src/ui/model"
import { view } from "../src/ui/view"

// AO-6 (#5447, EPIC #5441): black-screen regression guard for commit 73cada159.
//
// Root cause (the bug this guards): Foldkit's runtime renders `view(model).body`.
// `view` (and the crash boundary `crashView`) MUST return a `Document`
// (`{ title, body }`), not a bare `Html` (`h.div(...)`). When `view` returned an
// `Html`, `.body` was `undefined`, so nothing ever mounted → a blank/black
// window — and the crash boundary had the same bug, so even errors rendered
// blank. See `view.ts` (export const view) + `main.ts` (crashView).
//
// These tests FAIL if a future change regresses `view`/`crashView` back to a bare
// Html (or otherwise drops the Document `{ title, body }` shape that the runtime
// mounts), catching the regression before it ships a black window.

// A foldkit Document is `{ title: string, body: Html }`. A "black screen" is any
// `view` result whose `.body` is missing/undefined (the exact 73cada159 failure).
const isMountableDocument = (doc: unknown): boolean => {
  if (typeof doc !== "object" || doc === null) return false
  const record = doc as Record<string, unknown>
  // The runtime reads `.body`; a bare Html has no `.body`, so `.body` is the
  // load-bearing field. `.title` is part of the Document contract too.
  return (
    typeof record.title === "string" &&
    "body" in record &&
    record.body !== undefined &&
    record.body !== null &&
    typeof record.body === "object"
  )
}

describe("black-screen guard: view() returns a mountable Document (73cada159)", () => {
  test("the default (network) first-run model mounts a Document, not a bare Html", () => {
    const doc = view(initialModel) as unknown
    expect(isMountableDocument(doc)).toBe(true)
    // A bare Html (the regressed shape) has children/tag but no string title.
    expect((doc as Record<string, unknown>).title).toBe("Autopilot")
    expect((doc as Record<string, unknown>).body).toBeDefined()
  })

  test("the onboarding (Get started) pane mounts a Document too (the first-run wizard)", () => {
    // AO-4 wizard pane: a first-run user lands here; it must never render blank.
    const onboardingModel = Model.make({ ...initialModel, pane: "onboarding" })
    const doc = view(onboardingModel) as unknown
    expect(isMountableDocument(doc)).toBe(true)
    expect((doc as Record<string, unknown>).body).toBeDefined()
  })

  test("every pane mounts a Document (no pane regresses to a blank window)", () => {
    // PaneId is a closed literal set; render each so a new pane that returns a
    // bare Html (or undefined body) is caught here, not in production.
    const panes = PaneId.literals as readonly string[]
    expect(panes.length).toBeGreaterThan(0)
    for (const pane of panes) {
      const model = Model.make({ ...initialModel, pane })
      const doc = view(model) as unknown
      expect({ pane, mountable: isMountableDocument(doc) }).toEqual({
        pane,
        mountable: true,
      })
    }
  })
})

describe("black-screen guard: crashView returns a Document (73cada159)", () => {
  // `crashView` is module-private inside main.ts (importing main.ts boots the
  // Electroview/Runtime, which needs a real webview), so we guard its shape at
  // the source level: it MUST be typed to return a `Document` and MUST return a
  // `{ title, body }` object literal — not a bare `ch.div(...)`. This fails if a
  // future edit drops the Document shape that the crash boundary mounts.
  const mainSrc = readFileSync(
    join(import.meta.dir, "..", "src", "ui", "main.ts"),
    "utf8",
  )

  test("crashView is declared to return a Document", () => {
    // Tolerant of formatting/whitespace; requires the `: Document` return type.
    expect(mainSrc).toMatch(/crashView\s*=\s*\([^)]*\)\s*:\s*Document\s*=>/)
  })

  test("crashView returns a Document literal with title + body (not a bare Html)", () => {
    // The body of crashView must include the Document keys. We assert both the
    // `title:` and `body:` fields appear in the crashView definition block.
    const start = mainSrc.indexOf("const crashView")
    expect(start).toBeGreaterThanOrEqual(0)
    // Look at the crashView definition region (its returned object literal).
    const region = mainSrc.slice(start, start + 600)
    expect(region).toMatch(/title\s*:/)
    expect(region).toMatch(/body\s*:/)
  })
})
