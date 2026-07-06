import { describe, expect, test } from "bun:test"

/**
 * MM-H1 (#8487): source-string composition check for the Settings rework —
 * explicitly labeled stopgap (same allowance as
 * `khala_mobile.android.stt_module_typed_asyncfunction_signature.v1`, see
 * `src/contracts/ux-contracts.ts`'s Rules section), used here because fully
 * mounting `SettingsScreen` under `bun test` would require mocking
 * `expo-notifications`, `expo-constants`, and the auth/on-device-readiness
 * contexts — a real RN-mount oracle is the honest follow-up, tracked under
 * `khala_mobile.platform.launched_app_interaction_smoke.v1`. This still
 * proves something real: the exact source text a build ships.
 */
const readSettingsScreenSource = async (): Promise<string> =>
  Bun.file(new URL("../src/screens/settings-screen.tsx", import.meta.url).pathname).text()

describe("contract khala_mobile.settings.no_desktop_dependent_sections.v1", () => {
  test("settings_screen_excludes_fleet_desktop_copy.source — no Fleet section or desktop-only language", async () => {
    const source = await readSettingsScreenSource()
    expect(source).not.toContain("FleetSection")
    expect(source).not.toContain("Fleet run")
    expect(source).not.toContain("fleet run")
    expect(source).not.toContain("never leaves the desktop")
    expect(source).not.toContain("KHALA_SYNC_DEMO_FLEET_RUN_ID")
    expect(source).not.toContain("FLEET_RUN_ENTITY_TYPE")
    expect(source).not.toContain("FLEET_ACCOUNT_ENTITY_TYPE")
    expect(source).not.toContain("FLEET_WORKER_ENTITY_TYPE")
  })

  test("settings_screen_has_mobile_only_sections.source — Account, Credits, Models, Notifications, About are all present", async () => {
    const source = await readSettingsScreenSource()
    for (const label of ["Account", "Credits", "Models", "Notifications", "About"]) {
      expect(source).toContain(label)
    }
  })

  test("settings_screen_stubs_are_honest.source — Credits/Models sections state what's real without fabricating live data", async () => {
    const source = await readSettingsScreenSource()
    expect(source).toContain("coming soon")
    // Never claims a live balance figure or a real model list exists yet.
    expect(source).not.toMatch(/balance:\s*\$/i)
  })
})
