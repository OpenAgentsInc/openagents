import { describe, expect, test } from "bun:test"

const mobileRoot = new URL("../", import.meta.url)
const repoRoot = new URL("../../../", import.meta.url)

const read = (path: string) => Bun.file(new URL(path, mobileRoot)).text()
const readRepo = (path: string) => Bun.file(new URL(path, repoRoot)).text()

describe("Khala mobile Maestro flows", () => {
  test("define local-only startup and smoke flows", async () => {
    const [startup, fallback, signInInteraction, signedIn] = await Promise.all([
      read(".maestro/shared/_OnFlowStart.yaml"),
      read(".maestro/flows/LaunchFallback.yaml"),
      read(".maestro/flows/LaunchGitHubSignInInteraction.yaml"),
      read(".maestro/flows/SignedInThreadSmoke.yaml"),
    ])

    expect(startup).toContain("appId: ${MAESTRO_APP_ID}")
    expect(startup).toContain("clearState: true")
    expect(startup).toContain("clearKeychain: true")

    expect(fallback).toContain("Log in with GitHub")

    expect(signInInteraction).toContain("Log in with GitHub")
    expect(signInInteraction).toContain("tapOn")
    expect(signInInteraction).toContain("assertNotVisible")

    expect(signedIn).toContain("${KHALA_MAESTRO_OWNER_USER_ID}")
    expect(signedIn).toContain("${KHALA_MAESTRO_TOKEN}")
    expect(signedIn).toContain("${KHALA_MAESTRO_THREAD_TITLE}")
    expect(signedIn).toContain("Khala mobile public Maestro smoke")
  })

  test("do not commit real credentials or hosted CI hooks", async () => {
    const files = await Promise.all([
      read(".maestro/shared/_OnFlowStart.yaml"),
      read(".maestro/flows/LaunchFallback.yaml"),
      read(".maestro/flows/LaunchGitHubSignInInteraction.yaml"),
      read(".maestro/flows/SignedInThreadSmoke.yaml"),
    ])
    const allFlowText = files.join("\n")

    expect(allFlowText).not.toMatch(/oa_agent_[A-Za-z0-9_-]{8,}/)
    expect(allFlowText).not.toMatch(/Bearer\s+[A-Za-z0-9._-]+/)
    expect(allFlowText).not.toContain("eas ")
  })

  // Oracle for khala_mobile.qa.android_emulator_lane_definition.v1
  test("defines an Android emulator lane with boot proof, Maestro parity, and screencap capture", async () => {
    const [script, packageJsonText] = await Promise.all([
      read("scripts/android-emulator-test-run.sh"),
      read("package.json"),
    ])
    const packageJson = JSON.parse(packageJsonText) as { scripts: Record<string, string> }

    expect(packageJson.scripts["qa:android:emulator"]).toBe("bash scripts/android-emulator-test-run.sh")
    expect(script).toContain("avdmanager create avd")
    expect(script).toContain("adb wait-for-device")
    expect(script).toContain("sys.boot_completed")
    expect(script).toContain("maestro test .maestro/flows/LaunchFallback.yaml")
    expect(script).toContain("maestro test .maestro/flows/LaunchGitHubSignInInteraction.yaml")
    expect(script).toContain("adb exec-out screencap -p")
    expect(script).toContain("KHALA_ANDROID_EMULATOR_RECEIPT")
    expect(script).not.toContain("eas ")
    expect(script).not.toMatch(/oa_agent_[A-Za-z0-9_-]{8,}/)
    expect(script).not.toMatch(/Bearer\s+[A-Za-z0-9._-]+/)
  })

  test("records Android-keyed visual baselines from emulator screencaps", async () => {
    const [manifestText, reportText] = await Promise.all([
      readRepo("docs/khala-code/receipts/qam-4-baselines/manifest.json"),
      readRepo("docs/khala-code/receipts/2026-07-07-qam-6-android-visual-baselines.json"),
    ])
    const manifest = JSON.parse(manifestText) as {
      entries: Array<{ id: string; screenshot: string; viewport: string }>
      schema: string
    }
    const report = JSON.parse(reportText) as {
      ok: boolean
      results: Array<{ id: string; status: string }>
      schema: string
      simulatorTruth: string
    }
    const androidEntries = manifest.entries.filter(entry => entry.viewport === "pixel-8-emulator")

    expect(manifest.schema).toBe("openagents.khala_visual_baselines.v1")
    expect(report.schema).toBe("openagents.khala_mobile.visual_tier_report.v1")
    expect(report.ok).toBe(true)
    expect(report.simulatorTruth).toBe("captured")
    expect(report.results.map(result => result.status)).toEqual(["blessed", "blessed"])
    expect(androidEntries.map(entry => entry.id).sort()).toEqual([
      "khala.mobile.android.github-sign-in-interaction.pixel-8.dark",
      "khala.mobile.android.launch-fallback.pixel-8.dark",
    ])
    for (const entry of androidEntries) {
      await expect(
        Bun.file(new URL(`docs/khala-code/receipts/qam-4-baselines/${entry.screenshot}`, repoRoot)).exists(),
      ).resolves.toBe(true)
    }
  })
})
