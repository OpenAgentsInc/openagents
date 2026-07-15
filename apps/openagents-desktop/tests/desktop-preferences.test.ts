import { afterEach, describe, expect, test } from "vite-plus/test"
import { mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  DESKTOP_PREFERENCES_SCHEMA_ID,
  DESKTOP_PREFERENCES_VERSION,
  decodeDesktopPreferences,
  decodeDesktopPreferencesPatch,
  defaultDesktopPreferences,
  migrateDesktopPreferences,
} from "../src/desktop-preferences-contract.ts"
import { openDesktopPreferencesStore } from "../src/desktop-preferences-host.ts"
import {
  applyPreferencesToTheme,
  densityFactor,
  fontScaleFactor,
  preferencesRootAttributes,
  reduceMotionAttributeValue,
  themeForPreferences,
} from "../src/desktop-preferences-effects.ts"
import { khalaTheme } from "@effect-native/tokens"

const dirs: string[] = []
const scratch = (): string => {
  const dir = mkdtempSync(path.join(tmpdir(), "oa-prefs-"))
  dirs.push(dir)
  return dir
}
afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true })
})

describe("preferences migration", () => {
  test("hostile patches retain only plain known sections", () => {
    expect(decodeDesktopPreferencesPatch({
      presentation: "collapsed",
      appearance: { density: "compact" },
      unknown: { secret: "drop-me" },
    })).toEqual({ appearance: { density: "compact" } })
  })
  test("a valid current-version document is accepted unchanged", () => {
    const doc = defaultDesktopPreferences()
    const result = migrateDesktopPreferences(doc)
    expect(result.origin).toBe("current")
    expect(result.changed).toBe(false)
    expect(result.fromVersion).toBe(DESKTOP_PREFERENCES_VERSION)
    expect(result.preferences).toEqual(doc)
  })

  test("the default document decodes cleanly against the schema", () => {
    expect(decodeDesktopPreferences(defaultDesktopPreferences())).not.toBeNull()
  })

  test("unusable inputs seed defaults", () => {
    for (const input of [undefined, null, 42, "nope", [], true] as const) {
      const result = migrateDesktopPreferences(input)
      expect(result.origin).toBe("defaults")
      expect(result.changed).toBe(true)
      expect(result.preferences).toEqual(defaultDesktopPreferences())
    }
  })

  test("a legacy pre-versioned flat blob is lifted into the current shape, preserving values", () => {
    const legacy = { density: "compact", reducedMotion: "always", fontScale: "large" }
    const result = migrateDesktopPreferences(legacy)
    expect(result.origin).toBe("legacy_v0")
    expect(result.changed).toBe(true)
    expect(result.fromVersion).toBeNull()
    expect(result.preferences.version).toBe(DESKTOP_PREFERENCES_VERSION)
    expect(result.preferences.schema).toBe(DESKTOP_PREFERENCES_SCHEMA_ID)
    expect(result.preferences.appearance).toEqual({
      density: "compact",
      fontScale: "large",
      reducedMotion: "always",
    })
    // Fields the legacy blob never had fall back to defaults.
    expect(result.preferences.updates).toEqual(defaultDesktopPreferences().updates)
    expect(result.preferences.presentation).toEqual({ sidebarCollapsed: false })
  })

  test("a v1 document preserves its nested values and gains presentation defaults", () => {
    const v1 = {
      schema: "openagents.desktop.preferences.store.v1",
      version: 1,
      appearance: { density: "compact", fontScale: "large", reducedMotion: "always" },
      providerDefaults: { defaultProvider: "codex", defaultCodexAccountRef: "codex-2", defaultClaudeAccountRef: null },
      privacy: { redactDiagnosticsExport: true, shareCrashDiagnostics: false },
      notifications: { attentionBadge: false, taskCompletion: true, onlyWhenUnfocused: true },
      updates: { channel: "rc", autoCheck: true, autoDownload: false },
    }
    const result = migrateDesktopPreferences(v1)
    expect(result).toMatchObject({ origin: "legacy_v1", changed: true, fromVersion: 1 })
    expect(result.preferences.version).toBe(2)
    expect(result.preferences.appearance).toEqual(v1.appearance)
    expect(result.preferences.providerDefaults).toEqual(v1.providerDefaults)
    expect(result.preferences.presentation).toEqual({ sidebarCollapsed: false })
  })

  test("a dirty v2 document is field-normalized (bad enum → default) and marked changed", () => {
    const dirty = {
      schema: DESKTOP_PREFERENCES_SCHEMA_ID,
      version: DESKTOP_PREFERENCES_VERSION,
      appearance: { density: "ENORMOUS", fontScale: "large", reducedMotion: "always" },
      providerDefaults: { defaultProvider: "codex", defaultCodexAccountRef: "codex-2", defaultClaudeAccountRef: null },
      privacy: { redactDiagnosticsExport: true, shareCrashDiagnostics: "yes" },
      notifications: { attentionBadge: false, taskCompletion: true, onlyWhenUnfocused: true },
      updates: { channel: "rc", autoCheck: true, autoDownload: false },
      presentation: { sidebarCollapsed: "sometimes" },
    }
    const result = migrateDesktopPreferences(dirty)
    expect(result.origin).toBe("merged")
    expect(result.changed).toBe(true)
    // Bad enum → default; valid siblings preserved.
    expect(result.preferences.appearance.density).toBe("comfortable")
    expect(result.preferences.appearance.fontScale).toBe("large")
    // Non-boolean coerced to default.
    expect(result.preferences.privacy.shareCrashDiagnostics).toBe(false)
    // Valid values survive.
    expect(result.preferences.providerDefaults.defaultProvider).toBe("codex")
    expect(result.preferences.providerDefaults.defaultCodexAccountRef).toBe("codex-2")
    expect(result.preferences.updates.channel).toBe("rc")
    expect(result.preferences.presentation.sidebarCollapsed).toBe(false)
  })

  test("a future version is downgraded — unknown fields dropped, known ones kept", () => {
    const future = {
      schema: "openagents.desktop.preferences.store.v9",
      version: 99,
      appearance: { density: "cozy", fontScale: "small", reducedMotion: "never", experimentalHolograms: true },
      providerDefaults: { defaultProvider: "claude", defaultCodexAccountRef: null, defaultClaudeAccountRef: null },
      privacy: { redactDiagnosticsExport: true, shareCrashDiagnostics: false },
      notifications: { attentionBadge: true, taskCompletion: true, onlyWhenUnfocused: false },
      updates: { channel: "stable", autoCheck: false, autoDownload: false },
      presentation: { sidebarCollapsed: true, futureDockMode: "floating" },
      quantumTunneling: { enabled: true },
    }
    const result = migrateDesktopPreferences(future)
    expect(result.origin).toBe("downgraded")
    expect(result.changed).toBe(true)
    expect(result.fromVersion).toBe(99)
    expect(result.preferences.version).toBe(DESKTOP_PREFERENCES_VERSION)
    expect(result.preferences.appearance).toEqual({ density: "cozy", fontScale: "small", reducedMotion: "never" })
    expect(result.preferences.providerDefaults.defaultProvider).toBe("claude")
    expect(result.preferences.presentation).toEqual({ sidebarCollapsed: true })
    expect("quantumTunneling" in result.preferences).toBe(false)
  })

  test("a malformed account ref is dropped to null; a valid one survives", () => {
    const withRefs = {
      ...defaultDesktopPreferences(),
      providerDefaults: {
        defaultProvider: "auto",
        defaultCodexAccountRef: "has spaces and $ymbols",
        defaultClaudeAccountRef: "claude-primary",
      },
    }
    const result = migrateDesktopPreferences(withRefs)
    expect(result.preferences.providerDefaults.defaultCodexAccountRef).toBeNull()
    expect(result.preferences.providerDefaults.defaultClaudeAccountRef).toBe("claude-primary")
  })
})

describe("preferences host", () => {
  test("missing file → defaults; update round-trips; snapshot reflects the write", () => {
    const file = path.join(scratch(), "preferences.json")
    const store = openDesktopPreferencesStore(file)
    expect(store.snapshot()).toEqual(defaultDesktopPreferences())

    const next = store.update({ appearance: { density: "compact", fontScale: "large", reducedMotion: "always" } })
    expect(next.appearance.density).toBe("compact")
    // A fresh store over the same file reads back the persisted change.
    expect(openDesktopPreferencesStore(file).snapshot().appearance).toEqual({
      density: "compact",
      fontScale: "large",
      reducedMotion: "always",
    })

    const presentation = store.update({ presentation: { sidebarCollapsed: true } })
    expect(presentation.presentation.sidebarCollapsed).toBe(true)
    expect(openDesktopPreferencesStore(file).snapshot().presentation.sidebarCollapsed).toBe(true)
  })

  test("an update with a bad value is normalized, never persisted raw", () => {
    const file = path.join(scratch(), "preferences.json")
    const store = openDesktopPreferencesStore(file)
    // Force a bad enum past the type boundary as a hostile caller would.
    const next = store.update({ appearance: { density: "HUGE" as never } })
    expect(next.appearance.density).toBe("comfortable")
    const onDisk = JSON.parse(readFileSync(file, "utf8"))
    expect(onDisk.appearance.density).toBe("comfortable")

    const presentation = store.update({ presentation: { sidebarCollapsed: "yes" as never } })
    expect(presentation.presentation.sidebarCollapsed).toBe(false)
    expect(JSON.parse(readFileSync(file, "utf8")).presentation.sidebarCollapsed).toBe(false)
  })

  test("the persisted file is owner-only (mode 0600)", () => {
    const file = path.join(scratch(), "preferences.json")
    openDesktopPreferencesStore(file).update({ updates: { channel: "rc" } as never })
    const mode = statSync(file).mode & 0o777
    expect(mode).toBe(0o600)
  })

  test("a legacy on-disk file self-heals to the current version on read", () => {
    const file = path.join(scratch(), "preferences.json")
    writeFileSync(file, JSON.stringify({ density: "cozy", reducedMotion: "always" }))
    const store = openDesktopPreferencesStore(file)
    const snap = store.snapshot()
    expect(snap.version).toBe(DESKTOP_PREFERENCES_VERSION)
    expect(snap.appearance.density).toBe("cozy")
    expect(store.lastOrigin()).toBe("legacy_v0")
    // The bytes were rewritten to the normalized current form.
    const onDisk = JSON.parse(readFileSync(file, "utf8"))
    expect(onDisk.version).toBe(DESKTOP_PREFERENCES_VERSION)
    expect(onDisk.schema).toBe(DESKTOP_PREFERENCES_SCHEMA_ID)
  })

  test("reset restores defaults", () => {
    const file = path.join(scratch(), "preferences.json")
    const store = openDesktopPreferencesStore(file)
    store.update({ appearance: { density: "compact" } as never })
    expect(store.reset()).toEqual(defaultDesktopPreferences())
    expect(store.snapshot()).toEqual(defaultDesktopPreferences())
  })
})

describe("preferences effects (density / font / reduced-motion)", () => {
  test("scale factors are monotonic around the default identity", () => {
    expect(fontScaleFactor("default")).toBe(1)
    expect(fontScaleFactor("small")).toBeLessThan(1)
    expect(fontScaleFactor("large")).toBeGreaterThan(1)
    expect(fontScaleFactor("x-large")).toBeGreaterThan(fontScaleFactor("large"))
    expect(densityFactor("comfortable")).toBe(1)
    expect(densityFactor("cozy")).toBeLessThan(1)
    expect(densityFactor("compact")).toBeLessThan(densityFactor("cozy"))
  })

  test("the default preferences return the identical base theme (no allocation churn)", () => {
    const theme = applyPreferencesToTheme(khalaTheme, { fontScale: "default", density: "comfortable" })
    expect(theme).toBe(khalaTheme)
  })

  test("a larger font scales the type tokens up and keeps lineHeight >= fontSize", () => {
    const theme = applyPreferencesToTheme(khalaTheme, { fontScale: "x-large", density: "comfortable" })
    expect(theme.typeScale.body.fontSize).toBeGreaterThan(khalaTheme.typeScale.body.fontSize)
    for (const value of Object.values(theme.typeScale)) {
      expect(value.lineHeight).toBeGreaterThanOrEqual(value.fontSize)
    }
    // Colors (the blue identity) are untouched.
    expect(theme.color).toEqual(khalaTheme.color)
  })

  test("a compact density scales spacing and control tokens down", () => {
    const theme = applyPreferencesToTheme(khalaTheme, { fontScale: "default", density: "compact" })
    expect(theme.spacing["4"]).toBeLessThan(khalaTheme.spacing["4"])
    expect(theme.control.md.height).toBeLessThan(khalaTheme.control.md.height)
    // Zero stays zero.
    expect(theme.spacing["0"]).toBe(0)
  })

  test("reduced-motion attribute maps override modes and defers on system", () => {
    expect(reduceMotionAttributeValue("always")).toBe("true")
    expect(reduceMotionAttributeValue("never")).toBe("false")
    expect(reduceMotionAttributeValue("system")).toBeNull()
  })

  test("root attributes carry reduce-motion only for an explicit override", () => {
    const system = preferencesRootAttributes(defaultDesktopPreferences())
    expect("data-en-reduce-motion" in system).toBe(false)
    expect(system["data-en-density"]).toBe("comfortable")

    const forced = preferencesRootAttributes({
      ...defaultDesktopPreferences(),
      appearance: { density: "cozy", fontScale: "small", reducedMotion: "always" },
    })
    expect(forced["data-en-reduce-motion"]).toBe("true")
    expect(forced["data-en-density"]).toBe("cozy")
  })

  test("themeForPreferences threads the document through the scaler", () => {
    const theme = themeForPreferences({
      ...defaultDesktopPreferences(),
      appearance: { density: "compact", fontScale: "large", reducedMotion: "system" },
    })
    expect(theme.typeScale.body.fontSize).toBeGreaterThan(khalaTheme.typeScale.body.fontSize)
    expect(theme.spacing["4"]).toBeLessThan(khalaTheme.spacing["4"])
  })
})
