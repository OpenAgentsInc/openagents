import { describe, expect, test } from "bun:test"
import {
  buildDiffReviewArtifact,
  summarizeChangeSet,
  type ChangeSet,
} from "../src/tas/diff-review"

const digestRef = `sha256:${"b".repeat(64)}`

describe("diff review artifact core", () => {
  test("summarizes change set totals", () => {
    const changeSet: ChangeSet = {
      files: [
        { path: "src/new.ts", added: 12, removed: 0, status: "added" },
        { path: "src/existing.ts", added: 5, removed: 3, status: "modified" },
        { path: "src/old.ts", added: 0, removed: 8, status: "deleted" },
      ],
    }

    expect(summarizeChangeSet(changeSet)).toEqual({
      fileCount: 3,
      totalAdded: 17,
      totalRemoved: 11,
    })
  })

  test("preserves per-file status in review artifacts", () => {
    const changeSet: ChangeSet = {
      files: [
        { path: "src/new.ts", added: 1, removed: 0, status: "added" },
        { path: "src/existing.ts", added: 2, removed: 1, status: "modified" },
        { path: "src/old.ts", added: 0, removed: 3, status: "deleted" },
      ],
    }

    expect(buildDiffReviewArtifact(changeSet, { digestRef }).files).toEqual([
      { path: "src/new.ts", added: 1, removed: 0, status: "added" },
      { path: "src/existing.ts", added: 2, removed: 1, status: "modified" },
      { path: "src/old.ts", added: 0, removed: 3, status: "deleted" },
    ])
  })

  test("builds refs-only artifact without raw content fields", () => {
    const changeSet = {
      files: [
        {
          path: "src/private.ts",
          added: 4,
          removed: 2,
          status: "modified" as const,
          content: "secret file content",
          patch: "@@ raw patch text",
          diff: "-private\n+private",
        },
      ],
    }

    const artifact = buildDiffReviewArtifact(changeSet, { digestRef })

    expect(artifact).toEqual({
      artifactKind: "diff_review",
      digestRef,
      summary: {
        fileCount: 1,
        totalAdded: 4,
        totalRemoved: 2,
      },
      files: [
        {
          path: "src/private.ts",
          added: 4,
          removed: 2,
          status: "modified",
        },
      ],
    })
    expect(Object.keys(artifact.files[0]).sort()).toEqual([
      "added",
      "path",
      "removed",
      "status",
    ])
    expect(JSON.stringify(artifact)).not.toContain("secret file content")
    expect(JSON.stringify(artifact)).not.toContain("@@ raw patch text")
    expect(JSON.stringify(artifact)).not.toContain("-private")
  })
})
