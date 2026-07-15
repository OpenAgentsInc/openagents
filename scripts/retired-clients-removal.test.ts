import { existsSync, readFileSync } from "node:fs"
import { resolve } from "node:path"
import { describe, expect, test } from "vite-plus/test"

const root = resolve(import.meta.dirname, "..")
const retiredRoots = [
  "clients/khala-cli",
  "clients/khala-ios",
  "clients/khala-mobile",
] as const

const liveAuthorities = [
  "pnpm-workspace.yaml",
  "package.json",
  "vite.config.ts",
  ".githooks/pre-push",
  "INSTALL.md",
  "apps/qa-runner/package.json",
  "apps/qa-runner/src/index.ts",
  "scripts/qa-nightly-matrix.ts",
  "scripts/public-cli-artifact-catalog.mjs",
  "apps/pylon/src/index.ts",
  "apps/openagents.com/docs/live/INSTALL.md",
  "apps/openagents.com/apps/start/src/routes/-artanis-console-page.tsx",
  "apps/openagents.com/apps/start/src/routes/-funnel-data.ts",
] as const

describe("retired clients stay outside live authority", () => {
  test("all three client trees are absent", () => {
    for (const path of retiredRoots) {
      expect(existsSync(resolve(root, path)), path).toBe(false)
    }
  })

  test("live workspace, onboarding, QA, and release authorities do not invoke them", () => {
    for (const path of liveAuthorities) {
      const source = readFileSync(resolve(root, path), "utf8")
      expect(source, path).not.toMatch(/clients\/(?:khala-cli|khala-ios|khala-mobile)/)
      expect(source, path).not.toMatch(/@openagentsinc\/khala(?:["'`\s]|$)/)
      expect(source, path).not.toMatch(/\bkhala fleet (?:connect|status|list)\b/)
    }
  })

  test("client-exclusive QA and native release planners are absent", () => {
    for (const path of [
      "apps/qa-runner/src/mobile-nightly.ts",
      "apps/qa-runner/src/mobile-nightly.test.ts",
      "packages/khala-qa-harness/src/bless-ios-mobile-visual-baselines.ts",
      "packages/khala-qa-harness/src/mobile-visual-tier.ts",
      "packages/khala-qa-harness/src/mobile-visual-tier.test.ts",
      "packages/autopilot-control-protocol/src/eas-build-plan.ts",
      "packages/autopilot-control-protocol/src/eas-build-plan.test.ts",
      "packages/autopilot-control-protocol/src/ship-pipeline-plan.ts",
      "packages/autopilot-control-protocol/src/ship-pipeline-plan.test.ts",
    ]) {
      expect(existsSync(resolve(root, path)), path).toBe(false)
    }
  })
})
