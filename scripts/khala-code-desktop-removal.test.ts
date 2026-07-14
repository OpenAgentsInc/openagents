import { describe, expect, test } from "vite-plus/test"
import { existsSync, readFileSync } from "node:fs"
import { resolve } from "node:path"

const root = resolve(import.meta.dirname, "..")
const removedPath = "clients/khala-code-desktop"
const executableDependencyFiles = [
  "package.json",
  "apps/pylon/package.json",
  "packages/harness-conformance/package.json",
  "packages/khala-qa-harness/package.json",
  "scripts/qa-nightly-matrix.ts",
] as const

describe("Khala Code desktop supersession", () => {
  test("keeps the deprecated client deleted", () => {
    expect(existsSync(resolve(root, removedPath))).toBe(false)
  })

  test("keeps executable package and QA boundaries free of the deleted client", () => {
    for (const file of executableDependencyFiles) {
      const text = readFileSync(resolve(root, file), "utf8")
      expect(text).not.toContain(removedPath)
      expect(text).not.toContain("@openagentsinc/khala-code-desktop")
    }
  })
})
