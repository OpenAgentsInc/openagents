/**
 * Diagnostics / watchdog contract (CUT-24 criterion 4, #8704).
 *
 * A single typed, PUBLIC-SAFE health projection over the app's operability
 * surfaces — provider, Runtime Gateway, Sync, workspace, PTY, and extensions
 * (MCP) — plus a redacted export bundle and the recovery/restart action set.
 *
 * Privacy is structural, not incidental: a diagnostics row can only carry a
 * bounded `domain` enum, a `level` enum, a short bounded public-safe `summary`,
 * and public-safe `refs` (`[A-Za-z0-9][A-Za-z0-9._:-]*`). There is no free-form
 * field for a path, prompt, token, url, or command. `redactDiagnosticsReport`
 * re-validates every row and DROPS anything that does not fit, so the exported
 * bundle cannot leak secrets even if an upstream builder regresses.
 */
import { Schema } from "effect"

export const DIAGNOSTICS_SCHEMA_ID = "openagents.desktop.diagnostics.v1" as const

/** Additive IPC channels (main ↔ renderer). Public-safe payloads only. */
export const DiagnosticsGatherChannel = "openagents-desktop/diagnostics-gather" as const
export const DiagnosticsActionChannel = "openagents-desktop/diagnostics-action" as const
export const DiagnosticsExportChannel = "openagents-desktop/diagnostics-export" as const

export const diagnosticsDomains = [
  "provider",
  "runtimeGateway",
  "sync",
  "workspace",
  "pty",
  "extensions",
] as const
export type DiagnosticsDomain = (typeof diagnosticsDomains)[number]

/** Traffic-light health. `unknown` is honest "not observed", distinct from `unavailable`. */
export const diagnosticsLevels = ["ok", "degraded", "unavailable", "unknown"] as const
export type DiagnosticsLevel = (typeof diagnosticsLevels)[number]

/** Recovery/restart actions a diagnostics row may offer. Bounded set. */
export const diagnosticsActions = [
  "refresh",
  "restart_runtime",
  "reconnect_sync",
  "reprobe_providers",
  "refresh_workspace",
  "reload_extensions",
] as const
export type DiagnosticsAction = (typeof diagnosticsActions)[number]

/** Public-safe short summary: bounded length, no newlines. */
const SummarySchema = Schema.String.check(Schema.isMaxLength(200))

/** Public-safe ref charset (mirrors the runtime-gateway PublicRefSchema). */
const PublicRefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(120),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)

export const DiagnosticsRowSchema = Schema.Struct({
  domain: Schema.Literals(diagnosticsDomains),
  level: Schema.Literals(diagnosticsLevels),
  /** Human-readable one-liner; MUST be public-safe (validated + scrubbed). */
  summary: SummarySchema,
  /** Optional public-safe refs (account ref, capability id, phase name…). */
  refs: Schema.Array(PublicRefSchema).check(Schema.isMaxLength(16)),
  /** Recovery actions offered for this row. */
  actions: Schema.Array(Schema.Literals(diagnosticsActions)).check(Schema.isMaxLength(6)),
})
export type DiagnosticsRow = typeof DiagnosticsRowSchema.Type

export const DiagnosticsReportSchema = Schema.Struct({
  schema: Schema.Literal(DIAGNOSTICS_SCHEMA_ID),
  generatedAt: Schema.Number,
  appVersion: Schema.String.check(Schema.isMaxLength(40)),
  /** Overall roll-up: worst level across rows. */
  overall: Schema.Literals(diagnosticsLevels),
  rows: Schema.Array(DiagnosticsRowSchema).check(Schema.isMaxLength(64)),
})
export type DiagnosticsReport = typeof DiagnosticsReportSchema.Type

const decodeReportExit = Schema.decodeUnknownExit(DiagnosticsReportSchema)
export const decodeDiagnosticsReport = (value: unknown): DiagnosticsReport | null => {
  const decoded = decodeReportExit(value)
  return decoded._tag === "Success" ? decoded.value : null
}

/** Decode an untrusted action payload to the bounded enum, or null. */
export const decodeDiagnosticsAction = (value: unknown): DiagnosticsAction | null =>
  typeof value === "string" && (diagnosticsActions as ReadonlyArray<string>).includes(value)
    ? (value as DiagnosticsAction)
    : null

// ---------------------------------------------------------------------------
// Roll-up + redaction.
// ---------------------------------------------------------------------------

const LEVEL_SEVERITY: Record<DiagnosticsLevel, number> = {
  ok: 0,
  unknown: 1,
  degraded: 2,
  unavailable: 3,
}

/** The worst (highest-severity) level across the rows, or `ok` when empty. */
export const worstLevel = (rows: ReadonlyArray<{ level: DiagnosticsLevel }>): DiagnosticsLevel =>
  rows.reduce<DiagnosticsLevel>(
    (worst, row) => (LEVEL_SEVERITY[row.level] > LEVEL_SEVERITY[worst] ? row.level : worst),
    "ok",
  )

/**
 * Patterns that must NEVER appear in an exported summary. If any matches, the
 * summary is replaced with a domain-generic placeholder rather than leaked.
 * This is defense-in-depth: the builder already emits only bounded enums and
 * short public-safe text, but a regression must fail closed.
 */
const SECRET_PATTERNS: ReadonlyArray<RegExp> = [
  /(?:^|[\s=(])[~.]*\/[\w.]/, // filesystem path segment (/Users, ~/x, ./x, ../x)
  /[a-z]+:\/\//i, // scheme:// (urls)
  /\\[\w.]/, // backslash (windows) path
  /sk-[A-Za-z0-9]/, // openai-style keys
  /\bBearer\b/i,
  /\beyJ[A-Za-z0-9_-]{6}/, // jwt-ish
  /[A-Za-z0-9_-]{40,}/, // long opaque token-like blob
  /[\r\n]/, // multiline content
]

const containsSecretLike = (value: string): boolean => SECRET_PATTERNS.some((pattern) => pattern.test(value))

/** A row whose summary looks unsafe is replaced with a generic, ref-free line. */
const scrubRow = (row: DiagnosticsRow): DiagnosticsRow => {
  if (!containsSecretLike(row.summary)) return row
  return {
    ...row,
    summary: `${row.domain} status ${row.level} (detail redacted)`,
    refs: row.refs.filter((ref) => !containsSecretLike(ref)),
  }
}

/**
 * Produce the export-safe bundle: re-decode through the schema (dropping any
 * out-of-contract row), then scrub each surviving row's summary. The result is
 * guaranteed to satisfy the schema AND carry no secret-like content.
 */
export const redactDiagnosticsReport = (report: DiagnosticsReport): DiagnosticsReport => {
  const rows = report.rows.map(scrubRow)
  return {
    schema: DIAGNOSTICS_SCHEMA_ID,
    generatedAt: report.generatedAt,
    appVersion: report.appVersion,
    overall: worstLevel(rows),
    rows,
  }
}

/** True iff the report is fully export-safe (schema-valid + no secret-like text). */
export const isExportSafe = (report: DiagnosticsReport): boolean => {
  if (decodeDiagnosticsReport(report) === null) return false
  return report.rows.every((row) => !containsSecretLike(row.summary) && row.refs.every((ref) => !containsSecretLike(ref)))
}
