import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import type { AdjutantAssignment } from './adjutant-assignments'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const AdjutantResearchPolicyMode = S.Literals([
  'research_required',
  'research_optional',
  'research_not_applicable',
  'research_bypassed_by_operator',
])
export type AdjutantResearchPolicyMode =
  typeof AdjutantResearchPolicyMode.Type

export const AdjutantResearchPolicy = S.Struct({
  actorUserId: S.NullOr(S.String),
  assignmentId: S.String,
  customerSafeStatus: S.Literals([
    'research_required',
    'research_optional',
    'research_not_needed',
    'research_bypassed',
  ]),
  customerSafeSummary: S.String,
  defaultMode: AdjutantResearchPolicyMode,
  effectiveMode: AdjutantResearchPolicyMode,
  reason: S.NullOr(S.String),
  source: S.Literals(['default_assignment_kind', 'operator_override']),
  sourceAuthorityRef: S.NullOr(S.String),
  updatedAt: S.String,
})
export type AdjutantResearchPolicy = typeof AdjutantResearchPolicy.Type

export const SetAdjutantResearchPolicyInput = S.Struct({
  actorUserId: S.String,
  assignmentId: S.String,
  customerSafeSummary: S.String,
  policyMode: AdjutantResearchPolicyMode,
  reason: S.String,
  sourceAuthorityRef: S.optionalKey(S.NullOr(S.String)),
})
export type SetAdjutantResearchPolicyInput =
  typeof SetAdjutantResearchPolicyInput.Type

export type AdjutantResearchPolicyRuntime = Readonly<{
  nowIso: () => string
}>

export const systemAdjutantResearchPolicyRuntime: AdjutantResearchPolicyRuntime =
  {
    nowIso: currentIsoTimestamp,
  }

type PolicyRow = Readonly<{
  actor_user_id: string | null
  assignment_id: string
  customer_safe_summary: string
  policy_mode: AdjutantResearchPolicyMode
  reason: string
  source_authority_ref: string | null
  updated_at: string
}>

export class AdjutantResearchPolicyStorageError extends S.TaggedErrorClass<AdjutantResearchPolicyStorageError>()(
  'AdjutantResearchPolicyStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class AdjutantResearchPolicyUnsafePayload extends S.TaggedErrorClass<AdjutantResearchPolicyUnsafePayload>()(
  'AdjutantResearchPolicyUnsafePayload',
  {
    reason: S.String,
  },
) {}

export class AdjutantResearchPolicyValidationError extends S.TaggedErrorClass<AdjutantResearchPolicyValidationError>()(
  'AdjutantResearchPolicyValidationError',
  {
    reason: S.String,
  },
) {}

export type AdjutantResearchPolicyError =
  | AdjutantResearchPolicyStorageError
  | AdjutantResearchPolicyUnsafePayload
  | AdjutantResearchPolicyValidationError

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, AdjutantResearchPolicyStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new AdjutantResearchPolicyStorageError({ operation, error }),
  })

const nonEmptyBoundedText = (
  field: string,
  value: string,
  limit: number,
): Effect.Effect<string, AdjutantResearchPolicyValidationError> => {
  const text = value.trim()

  if (text === '') {
    return Effect.fail(
      new AdjutantResearchPolicyValidationError({
        reason: `${field} is required.`,
      }),
    )
  }

  if (text.length > limit) {
    return Effect.fail(
      new AdjutantResearchPolicyValidationError({
        reason: `${field} must be ${limit} characters or fewer.`,
      }),
    )
  }

  return Effect.succeed(text)
}

const nullableBoundedText = (
  field: string,
  value: string | null | undefined,
  limit: number,
): Effect.Effect<string | null, AdjutantResearchPolicyValidationError> => {
  if (value === null || value === undefined) {
    return Effect.succeed(null)
  }

  const text = value.trim()

  if (text === '') {
    return Effect.succeed(null)
  }

  if (text.length > limit) {
    return Effect.fail(
      new AdjutantResearchPolicyValidationError({
        reason: `${field} must be ${limit} characters or fewer.`,
      }),
    )
  }

  return Effect.succeed(text)
}

const assertSafeText = (
  value: string,
): Effect.Effect<void, AdjutantResearchPolicyUnsafePayload> =>
  containsProviderSecretMaterial(value)
    ? Effect.fail(
        new AdjutantResearchPolicyUnsafePayload({
          reason: 'Research policy contains secret-shaped material.',
        }),
      )
    : Effect.void

export const defaultResearchPolicyModeForAssignment = (
  assignment: Pick<AdjutantAssignment, 'assignmentKind' | 'siteId'>,
): AdjutantResearchPolicyMode => {
  switch (assignment.assignmentKind) {
    case 'site_generation':
      return 'research_required'
    case 'general_order_fulfillment':
    case 'site_adjustment':
      return 'research_optional'
    case 'site_deployment':
    case 'site_review':
      return 'research_not_applicable'
  }
}

const customerSafeStatus = (
  mode: AdjutantResearchPolicyMode,
): AdjutantResearchPolicy['customerSafeStatus'] => {
  switch (mode) {
    case 'research_bypassed_by_operator':
      return 'research_bypassed'
    case 'research_not_applicable':
      return 'research_not_needed'
    case 'research_optional':
      return 'research_optional'
    case 'research_required':
      return 'research_required'
  }
}

const defaultCustomerSafeSummary = (
  mode: AdjutantResearchPolicyMode,
): string => {
  switch (mode) {
    case 'research_bypassed_by_operator':
      return 'Research was reviewed through an approved operator exception.'
    case 'research_not_applicable':
      return 'Additional public-source research is not needed for this assignment.'
    case 'research_optional':
      return 'Public-source research can improve this assignment but is not required before work begins.'
    case 'research_required':
      return 'Public-source research should be approved before this assignment launches.'
  }
}

const policyFromRow = (
  assignment: AdjutantAssignment,
  row: PolicyRow | null,
): AdjutantResearchPolicy => {
  const defaultMode = defaultResearchPolicyModeForAssignment(assignment)
  const effectiveMode = row?.policy_mode ?? defaultMode

  return {
    actorUserId: row?.actor_user_id ?? null,
    assignmentId: assignment.id,
    customerSafeStatus: customerSafeStatus(effectiveMode),
    customerSafeSummary:
      row?.customer_safe_summary ?? defaultCustomerSafeSummary(effectiveMode),
    defaultMode,
    effectiveMode,
    reason: row?.reason ?? null,
    source: row === null ? 'default_assignment_kind' : 'operator_override',
    sourceAuthorityRef: row?.source_authority_ref ?? null,
    updatedAt: row?.updated_at ?? assignment.updatedAt,
  }
}

const readPolicyOverride = (
  db: D1Database,
  assignmentId: string,
): Effect.Effect<PolicyRow | null, AdjutantResearchPolicyStorageError> =>
  d1Effect('adjutantResearchPolicies.override.read', () =>
    db
      .prepare(
        `SELECT assignment_id,
                policy_mode,
                reason,
                customer_safe_summary,
                actor_user_id,
                source_authority_ref,
                updated_at
           FROM adjutant_assignment_research_policies
          WHERE assignment_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(assignmentId)
      .first<PolicyRow>(),
  )

const readEffectivePolicy = (
  db: D1Database,
  assignment: AdjutantAssignment,
): Effect.Effect<AdjutantResearchPolicy, AdjutantResearchPolicyStorageError> =>
  readPolicyOverride(db, assignment.id).pipe(
    Effect.map(row => policyFromRow(assignment, row)),
  )

const setPolicyOverride = (
  db: D1Database,
  runtime: AdjutantResearchPolicyRuntime,
  assignment: AdjutantAssignment,
  input: SetAdjutantResearchPolicyInput,
): Effect.Effect<AdjutantResearchPolicy, AdjutantResearchPolicyError> =>
  Effect.gen(function* () {
    const reason = yield* nonEmptyBoundedText('reason', input.reason, 1000)
    const customerSafeSummary = yield* nonEmptyBoundedText(
      'customerSafeSummary',
      input.customerSafeSummary,
      500,
    )
    const actorUserId = yield* nonEmptyBoundedText(
      'actorUserId',
      input.actorUserId,
      200,
    )
    const sourceAuthorityRef = yield* nullableBoundedText(
      'sourceAuthorityRef',
      input.sourceAuthorityRef,
      500,
    )
    yield* assertSafeText(
      JSON.stringify({
        customerSafeSummary,
        reason,
        sourceAuthorityRef,
      }),
    )

    const now = runtime.nowIso()

    yield* d1Effect('adjutantResearchPolicies.override.upsert', () =>
      db
        .prepare(
          `INSERT INTO adjutant_assignment_research_policies
             (assignment_id,
              policy_mode,
              reason,
              customer_safe_summary,
              actor_user_id,
              source_authority_ref,
              created_at,
              updated_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)
           ON CONFLICT(assignment_id) DO UPDATE SET
             policy_mode = excluded.policy_mode,
             reason = excluded.reason,
             customer_safe_summary = excluded.customer_safe_summary,
             actor_user_id = excluded.actor_user_id,
             source_authority_ref = excluded.source_authority_ref,
             updated_at = excluded.updated_at,
             archived_at = NULL`,
        )
        .bind(
          input.assignmentId,
          input.policyMode,
          reason,
          customerSafeSummary,
          actorUserId,
          sourceAuthorityRef,
          now,
          now,
        )
        .run(),
    )

    const row = yield* readPolicyOverride(db, input.assignmentId)

    if (row === null) {
      return yield* new AdjutantResearchPolicyStorageError({
        error: new Error('Research policy override was not readable after upsert.'),
        operation: 'adjutantResearchPolicies.override.readAfterWrite',
      })
    }

    return policyFromRow(assignment, row)
  })

export const makeAdjutantResearchPolicyService = (
  db: D1Database,
  runtime: AdjutantResearchPolicyRuntime = systemAdjutantResearchPolicyRuntime,
) => ({
  readEffectivePolicy: Effect.fn(
    'AdjutantResearchPolicyService.readEffectivePolicy',
  )((assignment: AdjutantAssignment) => readEffectivePolicy(db, assignment)),
  setPolicyOverride: Effect.fn(
    'AdjutantResearchPolicyService.setPolicyOverride',
  )((assignment: AdjutantAssignment, input: SetAdjutantResearchPolicyInput) =>
    setPolicyOverride(db, runtime, assignment, input),
  ),
})

export const makeAdjutantResearchPolicyEventId = (): string =>
  compactRandomId('adjutant_research_policy')
