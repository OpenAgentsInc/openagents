// #5470: tests for the shared DiffReview component's richer rendering options
// (file-tree grouping, side-by-side split hunks, expand-on-demand per file).
// The base flat-list / unified rendering is unchanged when no option is passed.

import { describe, expect, test } from "bun:test"
import type { Html } from "foldkit/html"
import type { DiffReviewFile, DiffReviewInput } from "../src/diff-review"

Object.assign(globalThis, {
  window: {
    requestAnimationFrame: (callback: FrameRequestCallback): number => {
      callback(0)
      return 0
    },
  },
})

const { DiffReview } = await import("../src/diff-review")

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    class?: Record<string, boolean>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === "object" && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, enabled]) => enabled)
    .map(([className]) => className)
    .join(" ")
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes.length === 0 ? [] : [["class", classes] as const]),
  ]
  return pairs
    .filter(([, value]) => value !== false && value !== undefined && value !== null)
    .map(([name, value]) => (value === true ? ` ${name}` : ` ${name}="${String(value)}"`))
    .join("")
}

const renderHtml = (html: Html): string => {
  if (html === null) return ""
  if (!isVNodeLike(html)) return ""
  const tag = html.sel ?? "node"
  const children = (html.children ?? [])
    .map((child) => (typeof child === "string" ? child : renderHtml(child)))
    .join("")
  const text = html.text ?? ""
  return `<${tag}${attrsToString(html)}>${text}${children}</${tag}>`
}

const file = (over: Partial<DiffReviewFile> = {}): DiffReviewFile => ({
  path: over.path ?? "src/a.ts",
  status: over.status ?? "modified",
  added: over.added ?? 1,
  removed: over.removed ?? 0,
  ...(over.hunkLines !== undefined ? { hunkLines: over.hunkLines } : {}),
})

const summaryFor = (files: ReadonlyArray<DiffReviewFile>): DiffReviewInput["summary"] =>
  files.reduce(
    (acc, f) => ({
      fileCount: acc.fileCount + 1,
      totalAdded: acc.totalAdded + f.added,
      totalRemoved: acc.totalRemoved + f.removed,
    }),
    { fileCount: 0, totalAdded: 0, totalRemoved: 0 },
  )

const render = (input: Omit<DiffReviewInput, "summary"> & { summary?: DiffReviewInput["summary"] }): string =>
  renderHtml(DiffReview({ ...input, summary: input.summary ?? summaryFor(input.files) }))

describe("DiffReview (#5470) — base behavior unchanged", () => {
  test("flat list by default; no tree, default unified view mode", () => {
    const rendered = render({ files: [file({ path: "src/a.ts" }), file({ path: "src/b.ts" })] })
    expect(rendered).toContain('data-autopilot-diff-review=""')
    expect(rendered).toContain('data-autopilot-diff-view-mode="unified"')
    expect(rendered).not.toContain('data-autopilot-diff-tree=""')
    expect(rendered).toContain('data-autopilot-diff-file="src/a.ts"')
    expect(rendered).toContain('data-autopilot-diff-file="src/b.ts"')
  })

  test("file with hunks and no expandedFiles renders the hunk body inline (no details)", () => {
    const rendered = render({
      files: [file({ path: "src/a.ts", hunkLines: [{ kind: "added", text: "const x = 1" }] })],
    })
    expect(rendered).toContain('data-autopilot-diff-hunks=""')
    expect(rendered).not.toContain("data-autopilot-diff-file-details")
    expect(rendered).toContain("+const x = 1")
  })
})

describe("DiffReview (#5470) — file tree", () => {
  test("groups files under their directory with a per-dir summary", () => {
    const rendered = render({
      fileTree: true,
      files: [
        file({ path: "src/ui/a.ts" }),
        file({ path: "src/ui/b.ts" }),
        file({ path: "docs/c.md" }),
        file({ path: "top.ts" }),
      ],
    })
    expect(rendered).toContain('data-autopilot-diff-tree=""')
    expect(rendered).toContain('data-autopilot-diff-tree-dir="src/ui"')
    expect(rendered).toContain('data-autopilot-diff-tree-dir="docs"')
    // a root-level file falls under "."
    expect(rendered).toContain('data-autopilot-diff-tree-dir="."')
    expect(rendered).toContain("src/ui/ · 2 files")
    expect(rendered).toContain("(root)/ · 1 file")
  })
})

describe("DiffReview (#5470) — expand on demand", () => {
  test("collapses non-expanded files behind a native details/summary", () => {
    const rendered = render({
      expandedFiles: ["src/a.ts"],
      files: [
        file({ path: "src/a.ts", hunkLines: [{ kind: "added", text: "open" }] }),
        file({ path: "src/b.ts", hunkLines: [{ kind: "added", text: "collapsed" }] }),
      ],
    })
    // both bodies are present (the collapsed one lives inside <details>), but the
    // collapsed file is wrapped in a details/summary so a large diff opens compact.
    expect(rendered).toContain("data-autopilot-diff-file-details")
    expect(rendered).toContain("+open")
    expect(rendered).toContain("+collapsed")
  })
})

describe("DiffReview (#5470) — split view", () => {
  test("renders a side-by-side hunk grid in split mode", () => {
    const rendered = render({
      viewMode: "split",
      files: [
        file({
          path: "src/a.ts",
          hunkLines: [
            { kind: "removed", text: "old line" },
            { kind: "added", text: "new line" },
            { kind: "context", text: "unchanged" },
          ],
        }),
      ],
    })
    expect(rendered).toContain('data-autopilot-diff-view-mode="split"')
    expect(rendered).toContain('data-autopilot-diff-hunks-split=""')
    expect(rendered).toContain("-old line")
    expect(rendered).toContain("+new line")
  })
})

describe("DiffReview (#5470) — empty", () => {
  test("renders the empty state with no files", () => {
    const rendered = render({ files: [], fileTree: true })
    expect(rendered).toContain("No file changes in this turn")
    expect(rendered).not.toContain('data-autopilot-diff-tree=""')
  })
})
