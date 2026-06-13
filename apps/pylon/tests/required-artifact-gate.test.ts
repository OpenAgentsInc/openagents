import { describe, expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { checkRequiredArtifacts } from "../src/required-artifact-gate"

describe("checkRequiredArtifacts", () => {
  test("returns satisfied=false and lists the absent ref when one of two files is missing", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-artifact-gate-"))
    try {
      await writeFile(join(root, "present.txt"), "ok")
      const result = await checkRequiredArtifacts(root, [
        { label: "report", relativePath: "present.txt" },
        { label: "summary", relativePath: "absent.txt" },
      ])
      expect(result.satisfied).toBe(false)
      expect(result.missingRefs).toContain("artifact.required.summary")
      expect(result.missingRefs).not.toContain("artifact.required.report")
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("treats a '..' traversal path as missing without touching the filesystem", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-artifact-gate-"))
    try {
      const result = await checkRequiredArtifacts(root, [
        { label: "escape", relativePath: "../secret.txt" },
      ])
      expect(result.satisfied).toBe(false)
      expect(result.missingRefs).toContain("artifact.required.escape")
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("returns satisfied=true with empty missingRefs when all required files exist", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-artifact-gate-"))
    try {
      await writeFile(join(root, "alpha.txt"), "a")
      await writeFile(join(root, "beta.txt"), "b")
      const result = await checkRequiredArtifacts(root, [
        { label: "alpha", relativePath: "alpha.txt" },
        { label: "beta", relativePath: "beta.txt" },
      ])
      expect(result.satisfied).toBe(true)
      expect(result.missingRefs).toHaveLength(0)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })
})
