/**
 * Bounded, redaction-on-write diagnostics log store for Khala Code desktop
 * (issue #8441). This is the durable home for the desktop parity gap-audit
 * requirement to "capture main-process, renderer, service, and native-shell
 * logs in a supportable structure".
 *
 * Redaction happens on write, not only on export: an entry's message/context
 * is passed through the shared redaction helpers before it is ever retained
 * in memory, so a later change to the export path cannot accidentally leak a
 * secret or raw prompt that was captured earlier in the process lifetime.
 */

import {
  redactKhalaCodeDesktopDiagnosticsText,
  redactKhalaCodeDesktopDiagnosticsValue,
} from "./diagnostics-redaction.js"

export type KhalaCodeDesktopDiagnosticsCategory =
  | "main"
  | "native-shell"
  | "renderer"
  | "service"

export type KhalaCodeDesktopDiagnosticsLevel = "debug" | "error" | "info" | "warn"

export type KhalaCodeDesktopDiagnosticsLogEntry = Readonly<{
  category: KhalaCodeDesktopDiagnosticsCategory
  context?: Readonly<Record<string, unknown>>
  fatal: boolean
  level: KhalaCodeDesktopDiagnosticsLevel
  message: string
  observedAt: string
  /** Optional source label, e.g. "codex-app-server", "pylon", "khala-sync". */
  source?: string
}>

export type KhalaCodeDesktopDiagnosticsLogRecordInput = Readonly<{
  category: KhalaCodeDesktopDiagnosticsCategory
  context?: Readonly<Record<string, unknown>>
  fatal?: boolean
  level?: KhalaCodeDesktopDiagnosticsLevel
  message: string
  observedAt?: string
  source?: string
}>

export type KhalaCodeDesktopDiagnosticsLogSnapshot = Readonly<{
  entriesByCategory: Readonly<
    Record<KhalaCodeDesktopDiagnosticsCategory, readonly KhalaCodeDesktopDiagnosticsLogEntry[]>
  >
  fatalEntries: readonly KhalaCodeDesktopDiagnosticsLogEntry[]
}>

export type KhalaCodeDesktopDiagnosticsLogStore = Readonly<{
  clear: () => void
  record: (input: KhalaCodeDesktopDiagnosticsLogRecordInput) => KhalaCodeDesktopDiagnosticsLogEntry
  snapshot: () => KhalaCodeDesktopDiagnosticsLogSnapshot
}>

export const KHALA_CODE_DESKTOP_DIAGNOSTICS_CATEGORIES: readonly KhalaCodeDesktopDiagnosticsCategory[] = [
  "main",
  "native-shell",
  "renderer",
  "service",
]

const DEFAULT_LIMIT_PER_CATEGORY = 500
const DEFAULT_FATAL_LIMIT = 100

export type KhalaCodeDesktopDiagnosticsLogStoreOptions = Readonly<{
  fatalLimit?: number
  limitPerCategory?: number
  now?: () => string
}>

const redactEntry = (
  input: KhalaCodeDesktopDiagnosticsLogRecordInput,
  observedAt: string,
): KhalaCodeDesktopDiagnosticsLogEntry => ({
  category: input.category,
  ...(input.context === undefined
    ? {}
    : {
      context: redactKhalaCodeDesktopDiagnosticsValue(input.context) as Readonly<
        Record<string, unknown>
      >,
    }),
  fatal: input.fatal ?? false,
  level: input.level ?? "info",
  message: redactKhalaCodeDesktopDiagnosticsText(input.message),
  observedAt,
  ...(input.source === undefined ? {} : { source: input.source }),
})

export const createKhalaCodeDesktopDiagnosticsLogStore = (
  options: KhalaCodeDesktopDiagnosticsLogStoreOptions = {},
): KhalaCodeDesktopDiagnosticsLogStore => {
  const limitPerCategory = options.limitPerCategory ?? DEFAULT_LIMIT_PER_CATEGORY
  const fatalLimit = options.fatalLimit ?? DEFAULT_FATAL_LIMIT
  const now = options.now ?? (() => new Date().toISOString())

  const byCategory: Record<KhalaCodeDesktopDiagnosticsCategory, KhalaCodeDesktopDiagnosticsLogEntry[]> = {
    main: [],
    "native-shell": [],
    renderer: [],
    service: [],
  }
  let fatalEntries: KhalaCodeDesktopDiagnosticsLogEntry[] = []

  const record = (
    input: KhalaCodeDesktopDiagnosticsLogRecordInput,
  ): KhalaCodeDesktopDiagnosticsLogEntry => {
    const entry = redactEntry(input, input.observedAt ?? now())
    const bucket = byCategory[entry.category]
    bucket.push(entry)
    while (bucket.length > limitPerCategory) bucket.shift()
    if (entry.fatal) {
      fatalEntries.push(entry)
      while (fatalEntries.length > fatalLimit) fatalEntries.shift()
    }
    return entry
  }

  const snapshot = (): KhalaCodeDesktopDiagnosticsLogSnapshot => ({
    entriesByCategory: {
      main: [...byCategory.main],
      "native-shell": [...byCategory["native-shell"]],
      renderer: [...byCategory.renderer],
      service: [...byCategory.service],
    },
    fatalEntries: [...fatalEntries],
  })

  const clear = (): void => {
    for (const category of KHALA_CODE_DESKTOP_DIAGNOSTICS_CATEGORIES) byCategory[category] = []
    fatalEntries = []
  }

  return { clear, record, snapshot }
}
