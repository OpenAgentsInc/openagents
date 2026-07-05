import { describe, expect, test } from "bun:test"

import {
  KHALA_CODE_REVIEW_LAYOUT_MAX_WIDTH_PX,
  KHALA_CODE_REVIEW_LAYOUT_MIN_WIDTH_PX,
  khalaCodeClampReviewLayoutWidth,
  khalaCodeDefaultReviewFocus,
  khalaCodeDefaultReviewLayout,
  khalaCodeGroupReviewFiles,
  khalaCodeParseReviewLayout,
  khalaCodeProjectReviewDiff,
  khalaCodeReviewDiffKindLabel,
  khalaCodeReviewPublicSafeSummary,
  khalaCodeReviewRevertState,
  khalaCodeSerializeReviewLayout,
  type KhalaCodeReviewFileEntry,
} from "../src/shared/review-panel"

const modifiedDiff = [
  "diff --git a/src/example.ts b/src/example.ts",
  "index 1111111..2222222 100644",
  "--- a/src/example.ts",
  "+++ b/src/example.ts",
  "@@ -1,3 +1,4 @@",
  " export const kept = true",
  "-export const oldName = 1",
  "+export const newName = 1",
  "+export const another = 2",
  "",
].join("\n")

const addedDiff = [
  "diff --git a/src/new-file.ts b/src/new-file.ts",
  "new file mode 100644",
  "index 0000000..3333333",
  "--- /dev/null",
  "+++ b/src/new-file.ts",
  "@@ -0,0 +1,2 @@",
  "+export const brandNew = true",
  "+export const another = 1",
  "",
].join("\n")

const deletedDiff = [
  "diff --git a/src/old-file.ts b/src/old-file.ts",
  "deleted file mode 100644",
  "index 4444444..0000000",
  "--- a/src/old-file.ts",
  "+++ /dev/null",
  "@@ -1,2 +0,0 @@",
  "-export const goingAway = true",
  "-export const alsoGoingAway = 1",
  "",
].join("\n")

const renamedDiff = [
  "diff --git a/src/before.ts b/src/after.ts",
  "similarity index 100%",
  "rename from src/before.ts",
  "rename to src/after.ts",
  "",
].join("\n")

describe("Khala Code review panel diff-kind projection", () => {
  test("classifies added, modified, and deleted files with add/delete counts", () => {
    const files = khalaCodeProjectReviewDiff([modifiedDiff, addedDiff, deletedDiff].join(""))
    expect(files).toHaveLength(3)

    const modified = files.find(file => file.path === "src/example.ts")
    expect(modified).toMatchObject({ additions: 2, deletions: 1, diffKind: "modified", path: "src/example.ts" })

    const added = files.find(file => file.path === "src/new-file.ts")
    expect(added).toMatchObject({ additions: 2, deletions: 0, diffKind: "added" })

    const deleted = files.find(file => file.path === "src/old-file.ts")
    expect(deleted).toMatchObject({ additions: 0, deletions: 2, diffKind: "deleted" })
  })

  test("classifies a pure rename as modified and records renamedFrom", () => {
    const files = khalaCodeProjectReviewDiff(renamedDiff)
    expect(files).toHaveLength(1)
    expect(files[0]).toMatchObject({
      additions: 0,
      deletions: 0,
      diffKind: "modified",
      path: "src/after.ts",
      renamedFrom: "src/before.ts",
    })
  })

  test("returns an empty list for a no-change diff (empty/no-change state)", () => {
    expect(khalaCodeProjectReviewDiff("")).toEqual([])
    expect(khalaCodeProjectReviewDiff("   \n  \n")).toEqual([])
  })

  test("ignores unrelated diff noise (hunk headers, index lines) when counting", () => {
    const files = khalaCodeProjectReviewDiff(modifiedDiff)
    expect(files).toHaveLength(1)
    // Only real +/- content lines counted; the " export const kept" context line,
    // the "@@" hunk header, and the "index"/"---"/"+++" lines must not be counted.
    expect(files[0]?.additions).toBe(2)
    expect(files[0]?.deletions).toBe(1)
  })

  test("groups files by diff kind in added/modified/deleted order and sorts paths", () => {
    const files: ReadonlyArray<KhalaCodeReviewFileEntry> = [
      { additions: 1, deletions: 0, diffKind: "modified", path: "z.ts" },
      { additions: 1, deletions: 0, diffKind: "added", path: "b.ts" },
      { additions: 1, deletions: 0, diffKind: "added", path: "a.ts" },
      { additions: 0, deletions: 1, diffKind: "deleted", path: "c.ts" },
    ]
    const grouped = khalaCodeGroupReviewFiles(files)
    expect(grouped.map(group => group.diffKind)).toEqual(["added", "modified", "deleted"])
    expect(grouped[0]?.files.map(file => file.path)).toEqual(["a.ts", "b.ts"])
  })

  test("empty groups are omitted rather than rendered as empty sections", () => {
    const grouped = khalaCodeGroupReviewFiles([
      { additions: 1, deletions: 0, diffKind: "added", path: "only.ts" },
    ])
    expect(grouped).toEqual([{ diffKind: "added", files: [{ additions: 1, deletions: 0, diffKind: "added", path: "only.ts" }] }])
  })

  test("labels diff kinds legibly", () => {
    expect(khalaCodeReviewDiffKindLabel("added")).toBe("Added")
    expect(khalaCodeReviewDiffKindLabel("modified")).toBe("Modified")
    expect(khalaCodeReviewDiffKindLabel("deleted")).toBe("Deleted")
  })

  test("default review focus starts with no active file", () => {
    expect(khalaCodeDefaultReviewFocus()).toEqual({ filePath: null })
  })
})

describe("Khala Code review panel revert availability", () => {
  test("revert is always explicitly unavailable with a stated reason (no safe backend)", () => {
    const state = khalaCodeReviewRevertState()
    expect(state.kind).toBe("unavailable")
    expect(state.reason).toBe("no_safe_backend")
    expect(state.message.length).toBeGreaterThan(0)
    expect(state.message.toLowerCase()).toContain("revert")
  })
})

describe("Khala Code review panel layout persistence", () => {
  test("defaults to files tab, expanded, and a sane default width", () => {
    expect(khalaCodeDefaultReviewLayout()).toEqual({
      activeTab: "files",
      collapsed: false,
      widthPx: 320,
    })
  })

  test("clamps width to the configured min/max bounds", () => {
    expect(khalaCodeClampReviewLayoutWidth(10)).toBe(KHALA_CODE_REVIEW_LAYOUT_MIN_WIDTH_PX)
    expect(khalaCodeClampReviewLayoutWidth(10_000)).toBe(KHALA_CODE_REVIEW_LAYOUT_MAX_WIDTH_PX)
    expect(khalaCodeClampReviewLayoutWidth(400)).toBe(400)
    expect(khalaCodeClampReviewLayoutWidth(Number.NaN)).toBe(320)
  })

  test("round-trips a serialized layout through parse", () => {
    const layout = { activeTab: "comments" as const, collapsed: true, widthPx: 480 }
    const serialized = khalaCodeSerializeReviewLayout(layout)
    expect(khalaCodeParseReviewLayout(serialized)).toEqual(layout)
  })

  test("falls back to defaults for missing, empty, or corrupt persisted layout", () => {
    expect(khalaCodeParseReviewLayout(null)).toEqual(khalaCodeDefaultReviewLayout())
    expect(khalaCodeParseReviewLayout("")).toEqual(khalaCodeDefaultReviewLayout())
    expect(khalaCodeParseReviewLayout("{not json")).toEqual(khalaCodeDefaultReviewLayout())
    expect(khalaCodeParseReviewLayout(JSON.stringify({ activeTab: "bogus" }))).toEqual(khalaCodeDefaultReviewLayout())
  })

  test("clamps an out-of-range persisted width on parse", () => {
    const parsed = khalaCodeParseReviewLayout(JSON.stringify({
      activeTab: "files",
      collapsed: false,
      widthPx: 9999,
    }))
    expect(parsed.widthPx).toBe(KHALA_CODE_REVIEW_LAYOUT_MAX_WIDTH_PX)
  })
})

describe("Khala Code review panel public-safe projection", () => {
  test("summarizes counts only, never file paths or diff content", () => {
    const files = khalaCodeProjectReviewDiff([modifiedDiff, addedDiff, deletedDiff].join(""))
    const summary = khalaCodeReviewPublicSafeSummary(files)
    expect(summary).toEqual({
      added: 1,
      deleted: 1,
      modified: 1,
      totalAdditions: 4,
      totalDeletions: 3,
    })
    const serialized = JSON.stringify(summary)
    expect(serialized).not.toContain("src/")
    expect(serialized).not.toContain(".ts")
  })

  test("empty file list summarizes to all zeros", () => {
    expect(khalaCodeReviewPublicSafeSummary([])).toEqual({
      added: 0,
      deleted: 0,
      modified: 0,
      totalAdditions: 0,
      totalDeletions: 0,
    })
  })
})
