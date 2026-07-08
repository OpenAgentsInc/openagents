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
      read(".maestro/flows/SignedInThreadReply.yaml"),
      read(".maestro/flows/SignedInScreensVisual.yaml"),
      read(".maestro/flows/SignedInScreensPopulatedVisual.yaml"),
      read(".maestro/flows/OnboardingFirstRunVisual.yaml"),
    ])
    const allFlowText = files.join("\n")

    expect(allFlowText).not.toMatch(/oa_agent_[A-Za-z0-9_-]{8,}/)
    expect(allFlowText).not.toMatch(/Bearer\s+[A-Za-z0-9._-]+/)
    expect(allFlowText).not.toContain("eas ")
  })

  // The SignedInThreadReply flow must actually ASSERT an assistant reply comes
  // back after Send — not just that the sent bubble renders. This is the guard
  // for the "message sent, NO server reply" class of prod bug (issue #8510):
  // the smoke flow only proved the SENT message rendered. The reply flow sends
  // a deterministic prompt whose answer token is NOT present in the prompt and
  // waits (bounded) for that token to become visible — a real reply assertion.
  test("SignedInThreadReply asserts a real assistant reply after Send", async () => {
    const reply = await read(".maestro/flows/SignedInThreadReply.yaml")

    // Same sign-in preamble contract as the smoke flow.
    expect(reply).toContain("${KHALA_MAESTRO_OWNER_USER_ID}")
    expect(reply).toContain("${KHALA_MAESTRO_TOKEN}")
    expect(reply).toContain("${KHALA_MAESTRO_THREAD_TITLE}")

    // Sends a message, then bounded-waits for the assistant REPLY token.
    expect(reply).toContain("tapOn: \"Send\"")
    expect(reply).toContain("extendedWaitUntil")
    expect(reply).toContain("${KHALA_MAESTRO_REPLY_EXPECT}")

    // The answer token ("Paris") must NOT appear in the prompt, so a visible
    // match can only be the reply, never an echo of the sent text.
    const promptMatch = reply.match(/KHALA_MAESTRO_REPLY_PROMPT:\s*"([^"]*)"/)
    const expectMatch = reply.match(/KHALA_MAESTRO_REPLY_EXPECT:\s*"([^"]*)"/)
    expect(promptMatch).not.toBeNull()
    expect(expectMatch).not.toBeNull()
    const prompt = (promptMatch?.[1] ?? "").toLowerCase()
    const expected = (expectMatch?.[1] ?? "").toLowerCase()
    expect(expected.length).toBeGreaterThan(0)
    expect(prompt.includes(expected)).toBe(false)
  })

  // Oracle for the QAM-4 (#8539) POPULATED-happy-path follow-up: the two screens
  // whose SignedInScreensVisual baselines can only render the honest DEGRADED
  // state (Credit history + repo picker read owner-scoped mobile REST routes that
  // require a real mobile OpenAuth USER session — never the seeded agent token —
  // per INVARIANTS.md) get a dedicated populated flow whose oracles are
  // FAIL-CLOSED: they assert the real-data markers ARE visible AND the degraded
  // "unavailable" empty states are NOT, so the flow can never silently pass on a
  // degraded (agent-token) build. This is a file-shape oracle (the captured
  // populated baselines themselves are gated on a one-time owner-provided real
  // session — see docs/khala-code/receipts/2026-07-07-qam-4-populated-happy-path.md).
  test("define populated signed-in visual flow with fail-closed populated oracles", async () => {
    const [populated, buildScript, runner] = await Promise.all([
      read(".maestro/flows/SignedInScreensPopulatedVisual.yaml"),
      read("scripts/build-populated-ios.sh"),
      read("scripts/mobile-visual-tier-run.sh"),
    ])

    // Reaches the same two screens and captures POPULATED-keyed checkpoints.
    expect(populated).toContain("takeScreenshot: khala.mobile.screen.credits-history.populated.iphone-17-pro.dark")
    expect(populated).toContain("takeScreenshot: khala.mobile.screen.repo-picker.populated.iphone-17-pro.dark")

    // Fail-closed oracle: real data asserted present, degraded empty states
    // asserted absent, and NO agent-token manual-sign-in fallback (which would
    // only ever reach the degraded state).
    expect(populated).toContain('assertNotVisible: "History unavailable"')
    expect(populated).toContain('assertNotVisible: "Repositories unavailable"')
    expect(populated).toContain('text: ".*\\\\$[0-9].*"')
    expect(populated).toContain('text: "public|private"')
    expect(populated).not.toContain("${KHALA_MAESTRO_TOKEN}")

    // The populated build script bakes a REAL session and refuses an agent
    // token via a live credits-balance 200 guard before building anything.
    expect(buildScript).toContain("khala-mobile-session.env")
    expect(buildScript).toContain("/api/mobile/credits/balance")
    expect(buildScript).not.toContain("eas ")
    expect(buildScript).not.toMatch(/oa_agent_[A-Za-z0-9_-]{8,}/)

    // The runner resets the seeded thread for the populated flow too and still
    // blesses/verifies through the owned engine, never hosted CI.
    expect(runner).toContain("SignedInScreensPopulatedVisual")
    expect(runner).toContain("bless-ios-mobile-visual-baselines.ts")
  })

  // Oracle for the QAM-4 (#8539) iOS signed-in screen visual sweep: the flows
  // that close the device-flow coverage gap for the four previously-uncovered
  // product screens each reach their screen against a real signed-in session
  // and drop a `takeScreenshot` checkpoint keyed to its baseline id.
  test("define signed-in screen + onboarding visual flows with screenshot checkpoints", async () => {
    const [signedInScreens, onboarding, runner] = await Promise.all([
      read(".maestro/flows/SignedInScreensVisual.yaml"),
      read(".maestro/flows/OnboardingFirstRunVisual.yaml"),
      read("scripts/mobile-visual-tier-run.sh"),
    ])

    // Settings reached via the real drawer hamburger, credit history + repo
    // picker reached (drawer/deep-link + thread), each captured.
    expect(signedInScreens).toContain('tapOn: "☰"')
    expect(signedInScreens).toContain('tapOn: "Settings"')
    expect(signedInScreens).toContain("takeScreenshot: khala.mobile.screen.settings.iphone-17-pro.dark")
    expect(signedInScreens).toContain("takeScreenshot: khala.mobile.screen.credits-history.iphone-17-pro.dark")
    expect(signedInScreens).toContain("takeScreenshot: khala.mobile.screen.repo-picker.iphone-17-pro.dark")

    // Onboarding first-run renders on an empty thread list; captured too.
    expect(onboarding).toContain('assertVisible: "Get started"')
    expect(onboarding).toContain("takeScreenshot: khala.mobile.screen.onboarding-welcome.iphone-17-pro.dark")

    // The runner blesses/verifies through the owned engine, never hosted CI.
    expect(runner).toContain("bless-ios-mobile-visual-baselines.ts")
    expect(runner).toContain("--verify")
    expect(runner).not.toContain("eas ")
  })

  test("records iOS signed-in screen visual baselines captured on the simulator", async () => {
    const [manifestText, reportText] = await Promise.all([
      readRepo("docs/khala-code/receipts/qam-4-baselines/manifest.json"),
      readRepo("docs/khala-code/receipts/2026-07-07-qam-4-ios-signed-in-screens.json"),
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
    const iosScreenIds = [
      "khala.mobile.screen.credits-history.iphone-17-pro.dark",
      "khala.mobile.screen.onboarding-welcome.iphone-17-pro.dark",
      "khala.mobile.screen.repo-picker.iphone-17-pro.dark",
      "khala.mobile.screen.settings.iphone-17-pro.dark",
    ]

    expect(manifest.schema).toBe("openagents.khala_visual_baselines.v1")
    expect(report.schema).toBe("openagents.khala_mobile.visual_tier_report.v1")
    expect(report.ok).toBe(true)
    expect(report.simulatorTruth).toBe("captured")

    const iosEntries = manifest.entries.filter(entry => iosScreenIds.includes(entry.id))
    expect(iosEntries.map(entry => entry.id).sort()).toEqual(iosScreenIds)
    for (const entry of iosEntries) {
      expect(entry.viewport).toBe("iphone-17-pro")
      await expect(
        Bun.file(new URL(`docs/khala-code/receipts/qam-4-baselines/${entry.screenshot}`, repoRoot)).exists(),
      ).resolves.toBe(true)
    }

    const reportedIds = report.results.filter(result => iosScreenIds.includes(result.id))
    expect(reportedIds.map(result => result.id).sort()).toEqual(iosScreenIds)
    for (const result of reportedIds) {
      expect(result.status).toBe("blessed")
    }
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
