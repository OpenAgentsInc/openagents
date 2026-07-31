import { redactProviderAccountLogValue } from '@openagentsinc/provider-account-schema'
import { Effect } from 'effect'

export type WorkerLogFields = Readonly<Record<string, unknown>>

export type WorkerLogEntry = Readonly<{
  event: string
  fields: Readonly<Record<string, string>>
}>

export type WorkerErrorLogEntry = WorkerLogEntry &
  Readonly<{
    errorMessage: string
    errorName: string
  }>

export const workerErrorName = (error: unknown): string =>
  error instanceof Error ? error.name : typeof error

// `Effect.tryPromise`'s bare-function form (the codebase's default when no
// domain-specific typed error is warranted) wraps a promise rejection in
// `Cause.UnknownError`, whose own `.message` is a generic
// "An error occurred in Effect.tryPromise" — the original rejection is
// preserved on `.cause`. Call sites that isolate per-item failures with
// `Effect.result`/`Effect.either`-style fan-outs should unwrap this before
// logging so the real underlying failure reason stays visible.
export const unwrapEffectTryPromiseCause = (error: unknown): unknown => {
  const cause = error instanceof Error ? error.cause : undefined
  return cause instanceof Error ? cause : error
}

const redactedLogValue = (value: unknown): string =>
  redactProviderAccountLogValue(value)

export const redactedWorkerLogFields = (
  fields: WorkerLogFields = {},
): Readonly<Record<string, string>> =>
  Object.fromEntries(
    Object.entries(fields)
      .filter(([, value]) => value !== undefined)
      .map(([key, value]) => [key, redactedLogValue(value)]),
  )

export const workerLogEntry = (
  event: string,
  fields: WorkerLogFields = {},
): WorkerLogEntry => ({
  event,
  fields: redactedWorkerLogFields(fields),
})

export const workerErrorLogEntry = (
  event: string,
  error: unknown,
  fields: WorkerLogFields = {},
): WorkerErrorLogEntry => ({
  ...workerLogEntry(event, fields),
  errorMessage: redactedLogValue(
    error instanceof Error ? error.message : String(error),
  ),
  errorName: workerErrorName(error),
})

/**
 * ONE LOG RECORD PER LINE, as Google Cloud Logging structured JSON.
 *
 * WHY (incident 2026-07-31, P0). These three functions used to emit through
 * `Effect.logError`/`logWarning`/`logInfo`, whose default logger PRETTY-PRINTS
 * across multiple lines:
 *
 *   [12:36:37.215] WARN (#125): {
 *     event: 'omega_nostr_storage_unavailable',
 *     fields: {
 *       pubkeyDigest: '...'
 *     }
 *   }
 *
 * Cloud Run ingests each LINE as its own log entry, so the severity landed on a
 * line holding only `{`, and the fields landed on separate entries with no
 * severity at all. Querying `severity>=ERROR` in production returned entries
 * with EMPTY payloads — the incident was undiagnosable by construction, which
 * is exactly how a 30-second auth failure went unexplained.
 *
 * Cloud Logging parses a single-line JSON object on stdout/stderr into
 * `jsonPayload` and lifts a top-level `severity` and `message`. Emitting that
 * shape directly makes every record self-contained and queryable by
 * `jsonPayload.event`. The entry builders above are unchanged, so redaction
 * still runs on every field before anything is written.
 *
 * The `Effect.withSpan` wrapper was dropped with the Effect logger: no tracer
 * is installed on the default runtime, so the span was inert and its only
 * observable effect was forcing the pretty logger.
 */
const emitStructured = (
  severity: 'ERROR' | 'WARNING' | 'INFO',
  event: string,
  entry: WorkerLogEntry | WorkerErrorLogEntry,
): void => {
  const line = JSON.stringify({ ...entry, message: event, severity })
  if (severity === 'ERROR') {
    console.error(line)
    return
  }
  if (severity === 'WARNING') {
    console.warn(line)
    return
  }
  console.log(line)
}

export const logWorkerRouteError = (
  event: string,
  error: unknown,
  fields: WorkerLogFields = {},
): void => {
  emitStructured('ERROR', event, workerErrorLogEntry(event, error, fields))
}

export const logWorkerRouteWarning = (
  event: string,
  fields: WorkerLogFields = {},
): void => {
  emitStructured('WARNING', event, workerLogEntry(event, fields))
}

export const logWorkerRouteInfo = (
  event: string,
  fields: WorkerLogFields = {},
): void => {
  emitStructured('INFO', event, workerLogEntry(event, fields))
}

export const observedPromise = <A>(
  spanName: string,
  run: () => Promise<A>,
): Promise<A> => observedEffect(spanName, Effect.promise(run))

export const observedEffect = <A, E>(
  spanName: string,
  effect: Effect.Effect<A, E>,
): Promise<A> => Effect.runPromise(effect.pipe(Effect.withSpan(spanName)))
