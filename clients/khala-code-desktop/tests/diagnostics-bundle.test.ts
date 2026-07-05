import { describe, expect, test } from "bun:test"

import {
  buildKhalaCodeDesktopDiagnosticsArchive,
  buildKhalaCodeDesktopDiagnosticsBundle,
  KHALA_CODE_DESKTOP_DIAGNOSTICS_BUNDLE_SCHEMA,
  khalaCodeDesktopDiagnosticsBundleArchiveEntries,
} from "../src/shared/diagnostics-bundle"
import { KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER } from "../src/shared/diagnostics-redaction"
import { createKhalaCodeDesktopDiagnosticsLogStore } from "../src/shared/diagnostics-log-store"
import { readKhalaCodeDesktopZipArchiveEntries } from "../src/shared/diagnostics-zip"

const decoder = new TextDecoder()

const bundleFixture = () => {
  const store = createKhalaCodeDesktopDiagnosticsLogStore()
  store.record({ category: "main", message: "app booted" })
  store.record({
    category: "renderer",
    fatal: true,
    level: "error",
    message: "uncaught TypeError: cannot read property of undefined",
  })
  store.record({
    category: "service",
    message: "codex app-server call failed with token sk-abcdefghijklmnopqrstuvwx",
    source: "codex-app-server",
  })
  store.record({ category: "native-shell", message: "main window created" })
  return buildKhalaCodeDesktopDiagnosticsBundle({
    appVersion: "0.1.0-rc.1",
    arch: "arm64",
    generatedAt: "2026-07-05T00:00:00.000Z",
    platform: "darwin",
    snapshot: store.snapshot(),
  })
}

// Oracle for khala_code.diagnostics.debug_log_export_public_safe_and_recovery_visible.v1
describe("buildKhalaCodeDesktopDiagnosticsBundle", () => {
  test("produces a schema-versioned, public-safe manifest with per-category totals", () => {
    const bundle = bundleFixture()
    expect(bundle.manifest.schema).toBe(KHALA_CODE_DESKTOP_DIAGNOSTICS_BUNDLE_SCHEMA)
    expect(bundle.manifest.publicSafe).toBe(true)
    expect(bundle.manifest.redactionApplied).toBe(true)
    expect(bundle.manifest.totalsByCategory).toEqual({
      main: 1,
      "native-shell": 1,
      renderer: 1,
      service: 1,
    })
    expect(bundle.manifest.fatalCount).toBe(1)
    expect(bundle.manifest.appVersion).toBe("0.1.0-rc.1")
  })

  test("never carries an un-redacted secret anywhere in the bundle", () => {
    const bundle = bundleFixture()
    const serialized = JSON.stringify(bundle)
    expect(serialized).not.toContain("sk-abcdefghijklmnopqrstuvwx")
    expect(serialized).toContain(KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER)
  })
})

describe("khalaCodeDesktopDiagnosticsBundleArchiveEntries", () => {
  test("emits manifest.json plus one .log file per category and a fatal-errors.log", () => {
    const entries = khalaCodeDesktopDiagnosticsBundleArchiveEntries(bundleFixture())
    expect(entries.map(entry => entry.path)).toEqual([
      "manifest.json",
      "main.log",
      "native-shell.log",
      "renderer.log",
      "service.log",
      "fatal-errors.log",
    ])
  })

  test("manifest.json entry parses back to the same manifest", () => {
    const bundle = bundleFixture()
    const entries = khalaCodeDesktopDiagnosticsBundleArchiveEntries(bundle)
    const manifestEntry = entries.find(entry => entry.path === "manifest.json")
    const parsed = JSON.parse(decoder.decode(manifestEntry?.data))
    expect(parsed).toEqual(bundle.manifest)
  })

  test("service.log contains the source label and redacted message text", () => {
    const entries = khalaCodeDesktopDiagnosticsBundleArchiveEntries(bundleFixture())
    const serviceLog = decoder.decode(entries.find(entry => entry.path === "service.log")?.data)
    expect(serviceLog).toContain("codex-app-server")
    expect(serviceLog).not.toContain("sk-abcdefghijklmnopqrstuvwx")
  })
})

describe("buildKhalaCodeDesktopDiagnosticsArchive", () => {
  test("produces a real zip archive that round-trips through the reader", () => {
    const bundle = bundleFixture()
    const archive = buildKhalaCodeDesktopDiagnosticsArchive(bundle)
    const entries = readKhalaCodeDesktopZipArchiveEntries(archive)
    expect(entries.map(entry => entry.path)).toContain("manifest.json")
    expect(entries.map(entry => entry.path)).toContain("fatal-errors.log")
    const fatalLog = decoder.decode(entries.find(entry => entry.path === "fatal-errors.log")?.data)
    expect(fatalLog).toContain("uncaught TypeError")
  })
})
