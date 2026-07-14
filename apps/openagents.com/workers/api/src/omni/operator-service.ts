import { Context, Effect, Layer } from 'effect'

import type { AgentRunRecord } from '../omni-runs'
import type { OmniError } from './errors'

/**
 * VP-1 retirement contract: Omni run admission is non-monetary. The operator
 * service remains as a stable Effect boundary for callers, but it neither reads
 * a credit balance nor writes a debit. Token/runner usage continues to be
 * recorded by the run store for quota and operations purposes.
 */
export type OmniNoSpendAdmission = Readonly<{
  allowed: true
  mode: 'no_spend'
}>

export type OmniOperatorServiceDependencies = Readonly<{
  /** Retained only so older composition roots can be upgraded independently. */
  requireMinimumRunCredits?:
    | ((...args: ReadonlyArray<unknown>) => unknown)
    | undefined
  /** Retained only so older composition roots can be upgraded independently. */
  recordContainerUsageDebitForRun?:
    | ((...args: ReadonlyArray<unknown>) => unknown)
    | undefined
}>

export type OmniOperatorServiceShape = Readonly<{
  requireRunCredits: (
    db: D1Database,
    userId: string,
    runtime?: unknown,
  ) => Effect.Effect<OmniNoSpendAdmission, OmniError>
  recordContainerUsageDebit: (
    db: D1Database,
    run: AgentRunRecord,
    input?: Readonly<{ billUntil?: string | undefined }>,
    runtime?: unknown,
  ) => Effect.Effect<void, OmniError>
}>

export class OmniOperatorService extends Context.Service<
  OmniOperatorService,
  OmniOperatorServiceShape
>()('openagents/OmniOperatorService') {}

export const makeOmniOperatorService = (
  _dependencies: OmniOperatorServiceDependencies = {},
): OmniOperatorServiceShape => ({
  requireRunCredits: (_db, _userId, _runtime) =>
    Effect.succeed({ allowed: true as const, mode: 'no_spend' as const }).pipe(
      Effect.withSpan('OmniOperatorService.requireRunCredits'),
    ),
  recordContainerUsageDebit: (_db, _run, _input, _runtime) =>
    Effect.void.pipe(
      Effect.withSpan('OmniOperatorService.recordContainerUsageDebit'),
    ),
})

export const OmniOperatorServiceLive = Layer.succeed(
  OmniOperatorService,
  makeOmniOperatorService(),
)
