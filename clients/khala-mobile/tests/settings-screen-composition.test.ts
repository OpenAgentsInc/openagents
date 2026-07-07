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
    expect(source).toContain("Delete account")
    expect(source).toContain("KHALA_ACCOUNT_DELETION_POLICY_COPY")
  })

  test("settings_screen_stubs_are_honest.source — Credits/Models sections state what's real without fabricating live data", async () => {
    const source = await readSettingsScreenSource()
    expect(source).toContain("coming soon")
    // Never claims a live balance figure or a real model list exists yet.
    expect(source).not.toMatch(/balance:\s*\$/i)
  })
})

/** The slice of source belonging to a top-level `const`/`export const NAME =`
 * component, up to the next top-level component declaration. Used to prove
 * WHICH section a control lives in, not just that the file mentions it. */
const componentSlice = (source: string, name: string): string => {
  const start = source.indexOf(`const ${name} = `)
  if (start === -1) return ""
  const rest = source.slice(start + name.length)
  // End at the earliest boundary: the next top-level component declaration OR
  // the JSDoc block that precedes it (so a following section's doc comment
  // never bleeds into this slice).
  const boundaries = [/\n(?:export )?const [A-Z]\w+ = /, /\n\/\*\*/]
    .map(re => rest.match(re)?.index)
    .filter((i): i is number => i !== undefined)
  const end = boundaries.length === 0 ? rest.length : Math.min(...boundaries)
  return rest.slice(0, end)
}

describe("contract khala_mobile.settings.sign_out_button_fill_on_inner_view.v1", () => {
  test("sign_out_uses_fabric_safe_inner_view_fill.source — Sign out is a Pressable+inner-View pill, not an Ignite no-fill Button", async () => {
    const source = await readSettingsScreenSource()
    const account = componentSlice(source, "AccountSection")
    // The old invisible control was `<Button preset="reversed" text="Sign out" ...>`.
    expect(source).not.toContain('text="Sign out"')
    // The fill lives on the inner plain View (paints under Fabric), the label
    // is a plain RNText, and the Pressable owns only the touch target.
    expect(account).toContain("styles.signOutButton")
    expect(account).toContain("styles.signOutPressable")
    expect(account).toContain("<RNText style={styles.signOutButtonText}>Sign out</RNText>")
    expect(account).toContain('accessibilityRole="button"')
    // The Pressable's own style must NOT be where the background is painted.
    expect(source).toContain("signOutButton: {")
    expect(source).toContain('backgroundColor: "#141d33"')
  })
})

describe("contract khala_mobile.settings.delete_account_isolated_at_bottom.v1", () => {
  test("delete_account_isolated_at_bottom.source — Delete account lives in its own bottom section, never adjacent to Sign out", async () => {
    const source = await readSettingsScreenSource()

    // Delete account is NOT in the Account card anymore.
    const account = componentSlice(source, "AccountSection")
    expect(account).not.toContain("Delete account")
    expect(account).not.toContain("deleteAccount")

    // It has its own dedicated destructive section component.
    expect(source).toContain("export const DeleteAccountSection")
    const danger = componentSlice(source, "DeleteAccountSection")
    // The trigger + its confirmation modal + policy copy + deleteAccount() all
    // live here now (behavior relocated intact).
    expect(danger).toContain("Delete account")
    expect(danger).toContain("KHALA_ACCOUNT_DELETION_POLICY_COPY")
    expect(danger).toContain("deleteAccount")
    // Destructive marking: a red outlined pill trigger + a red-bordered card.
    expect(danger).toContain("styles.deleteButton")
    expect(source).toContain("$dangerCard")

    // Rendered at the very bottom of the scroll: after About & diagnostics.
    const aboutAt = source.indexOf("<AboutSection />")
    const deleteAt = source.indexOf("<DeleteAccountSection />")
    expect(aboutAt).toBeGreaterThan(-1)
    expect(deleteAt).toBeGreaterThan(aboutAt)
    // And it is the last section rendered in the body (nothing after it).
    const lastSectionRender = source.lastIndexOf("Section />")
    expect(source.slice(0, lastSectionRender + "Section />".length)).toMatch(/<DeleteAccountSection \/>$/)
  })
})
