/**
 * Diagnostics / watchdog panel (CUT-24 criterion 4, #8704).
 *
 * A typed, read-only health panel over provider, Runtime Gateway, Sync,
 * workspace, PTY, and extension health, with a redacted export and per-domain
 * recovery/restart actions. Pure `state -> View`; the live report is gathered
 * in main (`diagnostics-host.ts`) and threaded in through the bridge. Every
 * surfaced value is a bounded enum, a count, or a public-safe ref — never a
 * path, prompt, token, or url (structural privacy of the contract).
 */
import {
  Badge,
  Button,
  Card,
  Divider,
  Spacer,
  Stack,
  Text,
  defineIntent,
  IntentRef,
  StaticPayload,
  type View,
} from "@effect-native/core"
import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"

import {
  decodeDiagnosticsReport,
  diagnosticsActions,
  type DiagnosticsAction,
  type DiagnosticsDomain,
  type DiagnosticsLevel,
  type DiagnosticsReport,
  type DiagnosticsRow,
} from "../diagnostics-contract.ts"

export type DiagnosticsReportView =
  | Readonly<{ state: "loading" }>
  | Readonly<{ state: "loaded"; report: DiagnosticsReport }>
  | Readonly<{ state: "unavailable"; message: string }>

export type DiagnosticsState = Readonly<{
  report: DiagnosticsReportView
  /** The in-flight action key, or null. Disables the row's controls while busy. */
  busy: string | null
  /** A one-line, public-safe notice after an export/action (e.g. a saved path is NOT shown). */
  notice: string | null
}>

export const initialDiagnosticsState = (): DiagnosticsState => ({
  report: { state: "loading" },
  busy: null,
  notice: null,
})

// ---------------------------------------------------------------------------
// Bridge (main-process IPC). Decoupled + defaulted to an unavailable stub so
// the module is unit-testable and never throws when the preload is absent.
// ---------------------------------------------------------------------------

export type DiagnosticsBridge = Readonly<{
  /** Gather a fresh report. Returns the raw report (schema-decoded here). */
  gather: () => Promise<unknown>
  /** Run a recovery/restart action; returns a public-safe `{ ok, notice? }`. */
  runAction: (action: DiagnosticsAction) => Promise<unknown>
  /** Produce + persist a redacted export; returns a public-safe `{ ok, notice? }`. */
  exportRedacted: () => Promise<unknown>
}>

export const unavailableDiagnosticsBridge: DiagnosticsBridge = {
  gather: async () => null,
  runAction: async () => ({ ok: false, notice: "Diagnostics unavailable" }),
  exportRedacted: async () => ({ ok: false, notice: "Diagnostics unavailable" }),
}

const NoticeResultSchema = Schema.Struct({
  ok: Schema.Boolean,
  notice: Schema.optional(Schema.String.check(Schema.isMaxLength(200))),
})
const decodeNotice = (value: unknown): { ok: boolean; notice?: string } => {
  const decoded = Schema.decodeUnknownExit(NoticeResultSchema)(value)
  return decoded._tag === "Success" ? decoded.value : { ok: false }
}

// ---------------------------------------------------------------------------
// Intents.
// ---------------------------------------------------------------------------

export const DesktopDiagnosticsRefreshRequested = defineIntent("DesktopDiagnosticsRefreshRequested", Schema.Null)
export const DesktopDiagnosticsExportRequested = defineIntent("DesktopDiagnosticsExportRequested", Schema.Null)
/** Payload is the bounded action enum. */
export const DesktopDiagnosticsActionRequested = defineIntent(
  "DesktopDiagnosticsActionRequested",
  Schema.Literals(diagnosticsActions),
)

export const diagnosticsIntents = [
  DesktopDiagnosticsRefreshRequested,
  DesktopDiagnosticsExportRequested,
  DesktopDiagnosticsActionRequested,
] as const

export type DiagnosticsCapableState = Readonly<{ diagnostics: DiagnosticsState }>

const reportViewFromGather = (value: unknown): DiagnosticsReportView => {
  const report = decodeDiagnosticsReport(value)
  return report === null ? { state: "unavailable", message: "Diagnostics unavailable" } : { state: "loaded", report }
}

export const makeDiagnosticsHandlers = <S extends DiagnosticsCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  bridge: DiagnosticsBridge = unavailableDiagnosticsBridge,
) => {
  const update = (transform: (current: S) => S) => SubscriptionRef.update(state, transform)
  const patch = (next: Partial<DiagnosticsState>) =>
    update((current) => ({ ...current, diagnostics: { ...current.diagnostics, ...next } }))

  const refresh = Effect.gen(function* () {
    yield* patch({ busy: "refresh" })
    const raw = yield* Effect.promise(() => bridge.gather().catch(() => null))
    yield* patch({ report: reportViewFromGather(raw), busy: null })
  })

  return {
    DesktopDiagnosticsRefreshRequested: () => refresh,
    DesktopDiagnosticsExportRequested: () =>
      Effect.gen(function* () {
        yield* patch({ busy: "export", notice: null })
        const raw = yield* Effect.promise(() => bridge.exportRedacted().catch(() => null))
        const result = decodeNotice(raw)
        yield* patch({
          busy: null,
          notice: result.notice ?? (result.ok ? "Redacted diagnostics exported" : "Export failed"),
        })
      }),
    DesktopDiagnosticsActionRequested: (action: DiagnosticsAction) =>
      Effect.gen(function* () {
        yield* patch({ busy: action, notice: null })
        const raw = yield* Effect.promise(() => bridge.runAction(action).catch(() => null))
        const result = decodeNotice(raw)
        // A successful recovery action re-gathers so the panel reflects the fix.
        if (result.ok) {
          const fresh = yield* Effect.promise(() => bridge.gather().catch(() => null))
          yield* patch({ report: reportViewFromGather(fresh), busy: null, notice: result.notice ?? "Recovery action ran" })
        } else {
          yield* patch({ busy: null, notice: result.notice ?? "Recovery action failed" })
        }
      }),
  }
}

// ---------------------------------------------------------------------------
// View.
// ---------------------------------------------------------------------------

const LEVEL_TONE: Record<DiagnosticsLevel, "neutral" | "info" | "success" | "warn" | "danger"> = {
  ok: "success",
  degraded: "warn",
  unavailable: "danger",
  unknown: "neutral",
}

const LEVEL_LABEL: Record<DiagnosticsLevel, string> = {
  ok: "OK",
  degraded: "Degraded",
  unavailable: "Unavailable",
  unknown: "Unknown",
}

const DOMAIN_LABEL: Record<DiagnosticsDomain, string> = {
  provider: "Provider accounts",
  runtimeGateway: "Runtime Gateway",
  sync: "Khala Sync",
  workspace: "Workspace",
  pty: "Terminal (PTY)",
  extensions: "Extensions (MCP)",
}

const ACTION_LABEL: Record<DiagnosticsAction, string> = {
  refresh: "Refresh",
  restart_runtime: "Restart runtime",
  reconnect_sync: "Reconnect sync",
  reprobe_providers: "Re-check providers",
  refresh_workspace: "Refresh workspace",
  reload_extensions: "Reload extensions",
}

const rowView = (row: DiagnosticsRow, busy: string | null): View =>
  Stack(
    {
      key: `diagnostics-row-${row.domain}`,
      direction: "column",
      gap: "1",
      padding: "2",
      // A labelled region so a screen reader announces "<domain>, status <level>".
      a11y: { role: "group", label: `${DOMAIN_LABEL[row.domain]}, status ${LEVEL_LABEL[row.level]}` },
    },
    [
      Stack({ key: `diagnostics-row-${row.domain}-head`, direction: "row", gap: "2", align: "center" }, [
        Text({ key: `diagnostics-row-${row.domain}-title`, content: DOMAIN_LABEL[row.domain], variant: "label" }),
        Badge({
          key: `diagnostics-row-${row.domain}-level`,
          label: LEVEL_LABEL[row.level],
          tone: LEVEL_TONE[row.level],
        }),
      ]),
      Text({ key: `diagnostics-row-${row.domain}-summary`, content: row.summary, variant: "caption", color: "textMuted" }),
      ...(row.actions.length > 0
        ? [
            Stack(
              { key: `diagnostics-row-${row.domain}-actions`, direction: "row", gap: "2" },
              row.actions.map((action) =>
                Button({
                  key: `diagnostics-row-${row.domain}-action-${action}`,
                  label: ACTION_LABEL[action],
                  variant: "secondary",
                  disabled: busy !== null,
                  onPress: IntentRef("DesktopDiagnosticsActionRequested", StaticPayload(action)),
                }),
              ),
            ),
          ]
        : []),
    ],
  )

export const diagnosticsView = (state: DiagnosticsState): View => {
  const header = Stack({ key: "diagnostics-header", direction: "row", gap: "2", align: "center" }, [
    Text({ key: "diagnostics-title", content: "Diagnostics", variant: "title" }),
    ...(state.report.state === "loaded"
      ? [
          Badge({
            key: "diagnostics-overall",
            label: `Overall ${LEVEL_LABEL[state.report.report.overall]}`,
            tone: LEVEL_TONE[state.report.report.overall],
          }),
        ]
      : []),
  ])

  const controls = Stack({ key: "diagnostics-controls", direction: "row", gap: "2" }, [
    Button({
      key: "diagnostics-refresh",
      label: "Refresh diagnostics",
      variant: "secondary",
      disabled: state.busy !== null,
      onPress: IntentRef("DesktopDiagnosticsRefreshRequested"),
    }),
    Button({
      key: "diagnostics-export",
      label: "Export redacted diagnostics",
      variant: "secondary",
      disabled: state.busy !== null || state.report.state !== "loaded",
      onPress: IntentRef("DesktopDiagnosticsExportRequested"),
    }),
  ])

  const body: ReadonlyArray<View> =
    state.report.state === "loading"
      ? [Text({ key: "diagnostics-loading", content: "Gathering health…", variant: "caption", color: "textMuted" })]
      : state.report.state === "unavailable"
        ? [Text({ key: "diagnostics-unavailable", content: state.report.message, variant: "caption", color: "danger" })]
        : state.report.report.rows.flatMap((row, index) =>
            index === 0 ? [rowView(row, state.busy)] : [Divider({ key: `diagnostics-divider-${row.domain}` }), rowView(row, state.busy)],
          )

  return Card({ key: "diagnostics-screen", padding: "3" }, [
    Stack({ key: "diagnostics-stack", direction: "column", gap: "3" }, [
      header,
      Text({
        key: "diagnostics-note",
        content: "Health is public-safe: no paths, prompts, or secrets. Export is always redacted.",
        variant: "caption",
        color: "textMuted",
      }),
      controls,
      ...(state.notice !== null ? [Text({ key: "diagnostics-notice", content: state.notice, variant: "caption", color: "info" })] : []),
      Spacer({ key: "diagnostics-spacer", size: "1" }),
      Stack({ key: "diagnostics-rows", direction: "column", gap: "1" }, body),
    ]),
  ])
}
