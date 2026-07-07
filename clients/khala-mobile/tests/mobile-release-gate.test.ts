import { existsSync, readdirSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"

import {
  khalaMobileGateFixtureTierStatus,
  khalaMobileGateGeneratorConformanceStatus,
  khalaMobileGateScreenMountBundles,
  khalaMobileGateScreenMountWaivers,
} from "../src/qa/mobile-release-gate"

const mobileRoot = new URL("../", import.meta.url).pathname
const fromRoot = (path: string): string => join(mobileRoot, path)

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

  test("every screen has a mount artifact or a typed QAM-2 waiver", () => {
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
  })

  test("generator conformance is an explicit QAM-3 blocker until templates emit full bundles", () => {
    expect(khalaMobileGateGeneratorConformanceStatus.issueRef).toBe("#8538")
    expect(khalaMobileGateGeneratorConformanceStatus.blockerRef).toBe("blocker.qam_3.generator_bundle_upgrade")
    expect(khalaMobileGateGeneratorConformanceStatus.state).toBe("stubbed_until_qam_3")
    expect(khalaMobileGateGeneratorConformanceStatus.statement).toContain("mount tests")
    expect(khalaMobileGateGeneratorConformanceStatus.statement).toContain("visual registration")
  })

  test("fixture tier names the existing runtime tests and the QAM-2 streaming gap", () => {
    for (const artifact of khalaMobileGateFixtureTierStatus.enforcedArtifacts) {
      expect(existsSync(fromRoot(artifact))).toBe(true)
    }
    expect(khalaMobileGateFixtureTierStatus.pendingArtifacts).toEqual([
      {
        blockerRef: "blocker.qam_2.agent_computer_streaming_fixture_suite",
        issueRef: "#8537",
        targetArtifact: "tests/thread-messages-streaming-fixtures.test.tsx",
      },
    ])
  })
})
