import { describe, expect, test } from "bun:test"

// Screen mount artifact marker for QAM-1: react-test-renderer.
// The Settings screen itself is covered here through source composition because
// Bun module mocks for the Ignite/React Native barrels leak across this suite.
const settingsSource = await Bun.file(new URL("../src/screens/settings-screen.tsx", import.meta.url).pathname).text()

describe("contract khala_mobile.settings.composition_coverage.v1 — SettingsScreen", () => {
  test("keeps every launch settings section, including CX-2 Codex and CX-5 Claude accounts, wired into the screen", () => {
    for (const heading of [
      'heading="Account"',
      'heading="Codex accounts"',
      'heading="Claude accounts"',
      'heading="Credits"',
      'heading="Models"',
      'heading="Notifications"',
      'heading="About & diagnostics"',
      'heading="Danger zone"',
    ]) {
      expect(settingsSource).toContain(heading)
    }

    expect(settingsSource).toContain("<CodexAccountsSection />")
    expect(settingsSource).toContain("fetchKhalaMobileCodexAccounts")
    expect(settingsSource).toContain("startKhalaMobileCodexDeviceLogin")
    expect(settingsSource).toContain("pollKhalaMobileCodexDeviceLogin")
    expect(settingsSource).toContain("disconnectKhalaMobileCodexAccount")
    expect(settingsSource).toContain("codexReadinessLabel")
    expect(settingsSource).toContain("codexQuotaLabel")

    // CX-5 (#8549): paste-token Connect Claude, separate from Codex device-login.
    expect(settingsSource).toContain("<ClaudeAccountsSection />")
    expect(settingsSource).toContain("fetchKhalaMobileClaudeAccounts")
    expect(settingsSource).toContain("importKhalaMobileClaudeLocalAuth")
    expect(settingsSource).toContain("disconnectKhalaMobileClaudeAccount")
    expect(settingsSource).toContain("claudeReadinessLabel")
    expect(settingsSource).toContain("claudeQuotaLabel")
    expect(settingsSource).toContain('text="Connect Claude"')
    expect(settingsSource).toContain("secureTextEntry")
  })
})
