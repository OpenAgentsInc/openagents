import { existsSync, readdirSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"

import {
  khalaMobileGateFixtureTierStatus,
  khalaMobileGateGeneratorConformanceStatus,
  khalaMobileGateScreenMountBundles,
  khalaMobileGateScreenMountWaivers,
  khalaMobileGateVisualTierStatus,
} from "../src/qa/mobile-release-gate"

const mobileRoot = new URL("../", import.meta.url).pathname
const repoRoot = join(mobileRoot, "../..")
const fromRoot = (path: string): string => join(mobileRoot, path)
const fromRepoRoot = (path: string): string => join(repoRoot, path)

const screenFiles = (): readonly string[] =>
  readdirSync(fromRoot("src/screens"))
    .filter(file => file.endsWith("-screen.tsx") || file === "onboarding-flow.tsx")
    .map(file => `src/screens/${file}`)
    .sort()

describe("QAM-1 mobile release gate policy", () => {
  test("package exposes the local qa:mobile:gate runner", async () => {
    const packageJson = JSON.parse(await Bun.file(fromRoot("package.json")).text()) as {
      scripts?: Record<string, string>
    }

    expect(packageJson.scripts?.["qa:mobile:gate"]).toBe("bun run scripts/qa-mobile-gate.ts")
  })

  test("every screen has a mount artifact and no QAM-2 screen waiver remains", () => {
    const covered = new Map(khalaMobileGateScreenMountBundles.map(bundle => [bundle.screenFile, bundle]))
    const waived = new Map(khalaMobileGateScreenMountWaivers.map(waiver => [waiver.screenFile, waiver]))
    const missing = screenFiles().filter(screen => !covered.has(screen) && !waived.has(screen))

    expect(missing).toEqual([])

    for (const bundle of khalaMobileGateScreenMountBundles) {
      expect(screenFiles()).toContain(bundle.screenFile)
      expect(bundle.mountTest).not.toBeNull()
      expect(existsSync(fromRoot(bundle.mountTest!))).toBe(true)
      const source = readFileSync(fromRoot(bundle.mountTest!), "utf8")
      expect(source).toContain("react-test-renderer")
    }

    for (const waiver of khalaMobileGateScreenMountWaivers) {
      expect(screenFiles()).toContain(waiver.screenFile)
      expect(waiver.issueRef).toMatch(/^#\d+$/)
      expect(waiver.issueRef).toBe("#8537")
      expect(waiver.blockerRef).toMatch(/^blocker\./)
      expect(waiver.targetArtifact).toMatch(/^tests\/.+\.test\.tsx$/)
      expect(waiver.reason.length).toBeGreaterThan(40)
    }
    expect(khalaMobileGateScreenMountWaivers).toEqual([])
  })

  test("generator conformance is enforced in the gate", () => {
    expect(khalaMobileGateGeneratorConformanceStatus.issueRef).toBe("#8538")
    expect(khalaMobileGateGeneratorConformanceStatus.state).toBe("qam_3_generator_bundle_enforced")
    expect(khalaMobileGateGeneratorConformanceStatus.statement).toContain("mount tests")
    expect(khalaMobileGateGeneratorConformanceStatus.statement).toContain("visual registration")
    for (const artifact of khalaMobileGateGeneratorConformanceStatus.enforcedArtifacts) {
      expect(existsSync(fromRoot(artifact))).toBe(true)
    }
  })

  test("fixture tier names the existing runtime tests and the enforced QAM-2 streaming suite", () => {
    for (const artifact of khalaMobileGateFixtureTierStatus.enforcedArtifacts) {
      expect(existsSync(fromRoot(artifact))).toBe(true)
    }
    expect(khalaMobileGateFixtureTierStatus.enforcedArtifacts).toContain("tests/thread-messages-screen.test.tsx")
    expect(khalaMobileGateFixtureTierStatus.pendingArtifacts).toEqual([])
    expect(khalaMobileGateFixtureTierStatus.state).toBe("qam_2_streaming_fixture_tier_enforced")
  })

  test("visual tier uses the owned baseline engine and records simulator truth separately", () => {
    expect(khalaMobileGateVisualTierStatus.issueRef).toBe("#8539")
    expect(khalaMobileGateVisualTierStatus.baselineEngine).toBe("packages/khala-qa-harness/src/visual-baseline.ts")
    expect(khalaMobileGateVisualTierStatus.reportSchema).toBe("openagents.khala_mobile.visual_tier_report.v1")
    expect(khalaMobileGateVisualTierStatus.statement).toContain("openagents.khala_visual_baselines.v1")
    expect(khalaMobileGateVisualTierStatus.statement).toContain("simulator screenshot truth")
    for (const artifact of khalaMobileGateVisualTierStatus.enforcedArtifacts) {
      expect(existsSync(fromRepoRoot(artifact))).toBe(true)
    }
  })
})
