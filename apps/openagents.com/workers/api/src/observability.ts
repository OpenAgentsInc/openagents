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

export const logWorkerRouteError = (
  event: string,
  error: unknown,
  fields: WorkerLogFields = {},
): void => {
  Effect.runSync(
    Effect.logError(workerErrorLogEntry(event, error, fields)).pipe(
      Effect.withSpan(`WorkerRoute.${event}`),
    ),
  )
}

export const logWorkerRouteWarning = (
  event: string,
  fields: WorkerLogFields = {},
): void => {
  Effect.runSync(
    Effect.logWarning(workerLogEntry(event, fields)).pipe(
      Effect.withSpan(`WorkerRoute.${event}`),
    ),
  )
}

export const logWorkerRouteInfo = (
  event: string,
  fields: WorkerLogFields = {},
): void => {
  Effect.runSync(
    Effect.logInfo(workerLogEntry(event, fields)).pipe(
      Effect.withSpan(`WorkerRoute.${event}`),
    ),
  )
}

export const observedPromise = <A>(
  spanName: string,
  run: () => Promise<A>,
): Promise<A> => observedEffect(spanName, Effect.promise(run))

export const observedEffect = <A, E>(
  spanName: string,
  effect: Effect.Effect<A, E>,
): Promise<A> => Effect.runPromise(effect.pipe(Effect.withSpan(spanName)))
