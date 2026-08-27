import { existsSync, readFileSync } from "node:fs"
import { resolve } from "node:path"
import { describe, expect, test } from "vite-plus/test"

const root = resolve(import.meta.dirname, "..")

const retiredPaths = [
  "apps/aiur/package.json",
  "apps/ai-sdk-harness-poc/package.json",
  "apps/khala-capture/package.json",
  "apps/khala-live-hub/package.json",
  "apps/oa-updates/package.json",
  "apps/openagents-mobile/package.json",
  "runners/py-bench-runner/pyproject.toml",
  "ops/owned-runner/khala-code-qa-nightly.service",
  "ops/owned-runner/khala-code-qa-nightly.timer",
  "scripts/cloud/gcp-benchmark-bootstrap.sh",
  "scripts/cloud/gcp-benchmark-cleanup.sh",
  "scripts/cloud/gcp-benchmark-smoke.sh",
  "scripts/cloud/gcp-benchmark-submit-batch.sh",
] as const

describe("retired experiments stay outside live authority", () => {
  test("removes retired services, experiments, and runners", () => {
    for (const path of retiredPaths) {
      expect(existsSync(resolve(root, path)), path).toBe(false)
    }
  })

  test("does not keep the harness in the workspace", () => {
    const workspace = readFileSync(resolve(root, "pnpm-workspace.yaml"), "utf8")
    expect(workspace).not.toContain("apps/ai-sdk-harness-poc")
    expect(workspace).not.toContain("apps/aiur")
    expect(workspace).not.toContain("apps/khala-capture")
    expect(workspace).not.toContain("apps/khala-live-hub")
    expect(workspace).not.toContain("apps/oa-updates")
    expect(workspace).not.toContain("apps/openagents-mobile")
  })
})
