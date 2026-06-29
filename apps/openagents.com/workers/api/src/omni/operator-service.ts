import { Context, Effect, Layer } from 'effect'

import {
  type BillingRuntime,
  type BillingSummary,
  recordContainerUsageDebitForRun,
  requireMinimumRunCredits,
} from '../billing'
import type { AgentRunRecord } from '../omni-runs'
import { OmniBillingError, type OmniError } from './errors'

export type OmniBillingGate =
  | Readonly<{ ok: true; billing: BillingSummary }>
  | Readonly<{ ok: false; billing: BillingSummary; message: string }>

export type OmniOperatorServiceDependencies = Readonly<{
  requireMinimumRunCredits?: typeof requireMinimumRunCredits | undefined
  recordContainerUsageDebitForRun?:
    | typeof recordContainerUsageDebitForRun
    | undefined
}>

export type OmniOperatorServiceShape = Readonly<{
  requireRunCredits: (
    db: D1Database,
    userId: string,
    runtime?: BillingRuntime,
  ) => Effect.Effect<BillingSummary, OmniError>
  recordContainerUsageDebit: (
    db: D1Database,
    run: AgentRunRecord,
    input?: Readonly<{ billUntil?: string | undefined }>,
    runtime?: BillingRuntime,
  ) => Effect.Effect<void, OmniError>
}>

export class OmniOperatorService extends Context.Service<
  OmniOperatorService,
  OmniOperatorServiceShape
>()('openagents/OmniOperatorService') {}

const billingFailure = (userId: string, error: unknown): OmniBillingError =>
  new OmniBillingError({
    userId,
    message: error instanceof Error ? error.message : String(error),
  })

export const makeOmniOperatorService = (
  dependencies: OmniOperatorServiceDependencies = {},
): OmniOperatorServiceShape => {
  const requireCredits =
    dependencies.requireMinimumRunCredits ?? requireMinimumRunCredits
  const recordDebit =
    dependencies.recordContainerUsageDebitForRun ??
    recordContainerUsageDebitForRun

  return {
    requireRunCredits: (db, userId, runtime) =>
      Effect.tryPromise({
        try: () => requireCredits(db, userId, runtime),
        catch: error => billingFailure(userId, error),
      }).pipe(
        Effect.flatMap(gate =>
          gate.ok
            ? Effect.succeed(gate.billing)
            : Effect.fail(
                new OmniBillingError({
                  userId,
                  message: gate.message,
                }),
              ),
        ),
        Effect.withSpan('OmniOperatorService.requireRunCredits'),
      ),
    recordContainerUsageDebit: (db, run, input, runtime) =>
      Effect.tryPromise({
        try: () => recordDebit(db, run, input, runtime),
        catch: error => billingFailure(run.userId, error),
      }).pipe(Effect.withSpan('OmniOperatorService.recordContainerUsageDebit')),
  }
}

export const OmniOperatorServiceLive = Layer.succeed(
  OmniOperatorService,
  makeOmniOperatorService(),
)
