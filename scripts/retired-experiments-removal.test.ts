import { existsSync, readFileSync } from "node:fs"
import { resolve } from "node:path"
import { describe, expect, test } from "vite-plus/test"

const root = resolve(import.meta.dirname, "..")

const retiredPaths = [
  "apps/ai-sdk-harness-poc/package.json",
  "runners/py-bench-runner/pyproject.toml",
  "ops/owned-runner/khala-code-qa-nightly.service",
  "ops/owned-runner/khala-code-qa-nightly.timer",
  "scripts/cloud/gcp-benchmark-bootstrap.sh",
  "scripts/cloud/gcp-benchmark-cleanup.sh",
  "scripts/cloud/gcp-benchmark-smoke.sh",
  "scripts/cloud/gcp-benchmark-submit-batch.sh",
] as const

describe("retired experiments stay outside live authority", () => {
  test("removes the unused harness, Python benchmark lane, and nightly unit", () => {
    for (const path of retiredPaths) {
      expect(existsSync(resolve(root, path)), path).toBe(false)
    }
  })

  test("does not keep the harness in the workspace", () => {
    const workspace = readFileSync(resolve(root, "pnpm-workspace.yaml"), "utf8")
    expect(workspace).not.toContain("apps/ai-sdk-harness-poc")
  })
})
