/**
 * Public-safe debug-log export bundle for Khala Code desktop (issue #8441).
 *
 * Builds a schema-versioned manifest plus per-category plaintext log files
 * from a diagnostics log snapshot, and packages them into a real ZIP archive
 * using the dependency-free writer in diagnostics-zip.ts. Every string that
 * lands in the bundle has already passed through the log store's redaction
 * on write; this module applies the same redaction helpers again to the
 * manifest fields as defense in depth (the manifest never carries a message
 * body, but app version/platform/arch are still passed through the text
 * redactor in case a future field addition captures something sensitive by
 * accident).
 */

import {
  redactKhalaCodeDesktopDiagnosticsText,
} from "./diagnostics-redaction.js"
import {
  buildKhalaCodeDesktopZipArchive,
  type KhalaCodeDesktopZipArchiveEntry,
} from "./diagnostics-zip.js"
import {
  KHALA_CODE_DESKTOP_DIAGNOSTICS_CATEGORIES,
  type KhalaCodeDesktopDiagnosticsCategory,
  type KhalaCodeDesktopDiagnosticsLogEntry,
  type KhalaCodeDesktopDiagnosticsLogSnapshot,
} from "./diagnostics-log-store.js"

export const KHALA_CODE_DESKTOP_DIAGNOSTICS_BUNDLE_SCHEMA =
  "openagents.khala_code_desktop.diagnostics_bundle.v1"

export type KhalaCodeDesktopDiagnosticsBundleManifest = Readonly<{
  appVersion: string
  arch: string
  fatalCount: number
  generatedAt: string
  platform: string
  publicSafe: true
  redactionApplied: true
  schema: typeof KHALA_CODE_DESKTOP_DIAGNOSTICS_BUNDLE_SCHEMA
  totalsByCategory: Readonly<Record<KhalaCodeDesktopDiagnosticsCategory, number>>
}>

export type KhalaCodeDesktopDiagnosticsBundle = Readonly<{
  entriesByCategory: Readonly<
    Record<KhalaCodeDesktopDiagnosticsCategory, readonly KhalaCodeDesktopDiagnosticsLogEntry[]>
  >
  fatalEntries: readonly KhalaCodeDesktopDiagnosticsLogEntry[]
  manifest: KhalaCodeDesktopDiagnosticsBundleManifest
}>

export type KhalaCodeDesktopDiagnosticsBundleInput = Readonly<{
  appVersion: string
  arch: string
  generatedAt?: string
  platform: string
  snapshot: KhalaCodeDesktopDiagnosticsLogSnapshot
}>

export const buildKhalaCodeDesktopDiagnosticsBundle = (
  input: KhalaCodeDesktopDiagnosticsBundleInput,
): KhalaCodeDesktopDiagnosticsBundle => {
  const totalsByCategory = Object.fromEntries(
    KHALA_CODE_DESKTOP_DIAGNOSTICS_CATEGORIES.map(category => [
      category,
      input.snapshot.entriesByCategory[category].length,
    ]),
  ) as Record<KhalaCodeDesktopDiagnosticsCategory, number>

  return {
    entriesByCategory: input.snapshot.entriesByCategory,
    fatalEntries: input.snapshot.fatalEntries,
    manifest: {
      appVersion: redactKhalaCodeDesktopDiagnosticsText(input.appVersion),
      arch: redactKhalaCodeDesktopDiagnosticsText(input.arch),
      fatalCount: input.snapshot.fatalEntries.length,
      generatedAt: input.generatedAt ?? new Date().toISOString(),
      platform: redactKhalaCodeDesktopDiagnosticsText(input.platform),
      publicSafe: true,
      redactionApplied: true,
      schema: KHALA_CODE_DESKTOP_DIAGNOSTICS_BUNDLE_SCHEMA,
      totalsByCategory,
    },
  }
}

const formatLogEntry = (entry: KhalaCodeDesktopDiagnosticsLogEntry): string => {
  const source = entry.source === undefined ? "" : ` (${entry.source})`
  const fatalTag = entry.fatal ? " [FATAL]" : ""
  const contextSuffix = entry.context === undefined
    ? ""
    : ` ${JSON.stringify(entry.context)}`
  return `${entry.observedAt} ${entry.level.toUpperCase()}${fatalTag}${source}: ${entry.message}${contextSuffix}`
}

const logFileName = (category: KhalaCodeDesktopDiagnosticsCategory): string =>
  `${category}.log`

/**
 * Flattens a bundle into the flat file list the zip writer expects:
 * manifest.json, one .log file per category, and a fatal-errors.log summary.
 */
export const khalaCodeDesktopDiagnosticsBundleArchiveEntries = (
  bundle: KhalaCodeDesktopDiagnosticsBundle,
): readonly KhalaCodeDesktopZipArchiveEntry[] => {
  const encoder = new TextEncoder()
  const entries: KhalaCodeDesktopZipArchiveEntry[] = [
    {
      data: encoder.encode(`${JSON.stringify(bundle.manifest, null, 2)}\n`),
      path: "manifest.json",
    },
  ]
  for (const category of KHALA_CODE_DESKTOP_DIAGNOSTICS_CATEGORIES) {
    const lines = bundle.entriesByCategory[category].map(formatLogEntry)
    entries.push({
      data: encoder.encode(lines.length === 0 ? "" : `${lines.join("\n")}\n`),
      path: logFileName(category),
    })
  }
  const fatalLines = bundle.fatalEntries.map(formatLogEntry)
  entries.push({
    data: encoder.encode(fatalLines.length === 0 ? "" : `${fatalLines.join("\n")}\n`),
    path: "fatal-errors.log",
  })
  return entries
}

export const buildKhalaCodeDesktopDiagnosticsArchive = (
  bundle: KhalaCodeDesktopDiagnosticsBundle,
): Uint8Array =>
  buildKhalaCodeDesktopZipArchive(khalaCodeDesktopDiagnosticsBundleArchiveEntries(bundle))
