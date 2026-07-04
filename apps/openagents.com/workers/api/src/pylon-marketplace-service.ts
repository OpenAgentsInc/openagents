import { Schema as S } from 'effect'
import type { SyncSql } from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { sha256Hex } from './agent-registration'
import { OmniProjectionAudience } from './omni-data-classification'
import { logWorkerRouteWarning } from './observability'
import {
  PYLON_MARKETPLACE_NO_SPEND_AUTHORITY,
  PylonMarketplaceAssignmentRecord,
  type PylonMarketplaceIntakeState,
  PylonMarketplaceJobIntakeRecord,
  PylonMarketplaceJobKind,
  type PylonMarketplaceJobSource,
  PylonMarketplaceLedgerProjection,
  PylonMarketplaceLedgerRecord,
  PylonMarketplacePrivacyClass,
  PylonMarketplaceUnsafe,
  projectPylonMarketplaceLedger,
} from './pylon-marketplace-jobs'
import {
  pylonDispatchFlagsFromEnv,
  type PylonDispatchFlagEnv,
  type PylonDispatchLog,
} from './pylon-dispatch-store'
import { PylonResourceMode } from './pylon-resource-mode-setup'
import { decodeUnknownWithSchema, parseJsonUnknown } from './json-boundary'
import { publicRefSegment, uniqueRefs } from './public-ref-format'
import { openAgentsDatabase } from './runtime'

export const PylonMarketplaceTriageOutcome = S.Literals([
  'accepted_for_review',
  'needs_input',
  'proposed_assignment',
  'rejected',
])
export type PylonMarketplaceTriageOutcome =
  typeof PylonMarketplaceTriageOutcome.Type

export class PylonMarketplaceCreateJobIntakeRequest extends S.Class<PylonMarketplaceCreateJobIntakeRequest>(
  'PylonMarketplaceCreateJobIntakeRequest',
)({
  benchmarkRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  budgetRefs: S.optionalKey(S.Array(S.String)),
  caveatRefs: S.optionalKey(S.Array(S.String)),
  dataRefs: S.optionalKey(S.Array(S.String)),
  eligibilityRequirementRefs: S.optionalKey(S.Array(S.String)),
  evidenceExpectationRefs: S.optionalKey(S.Array(S.String)),
  intakeRef: S.optionalKey(S.String),
  jobKind: PylonMarketplaceJobKind,
  jobRef: S.optionalKey(S.String),
  modelRefs: S.optionalKey(S.Array(S.String)),
  policyGateRefs: S.optionalKey(S.Array(S.String)),
  privacyClass: S.optionalKey(PylonMarketplacePrivacyClass),
  requesterRef: S.String,
  resourceModePreference: PylonResourceMode,
  resourceRequirementRefs: S.optionalKey(S.Array(S.String)),
  resultExpectationRefs: S.optionalKey(S.Array(S.String)),
  source: S.optionalKey(S.Literals([
    'external_agent',
    'external_human',
    'openagents_seeded',
  ])),
  sourceRefs: S.optionalKey(S.Array(S.String)),
  spendCaveatRefs: S.optionalKey(S.Array(S.String)),
}) {}

export class PylonMarketplaceAssignmentProposalRequest extends S.Class<PylonMarketplaceAssignmentProposalRequest>(
  'PylonMarketplaceAssignmentProposalRequest',
)({
  acceptanceCriteriaRefs: S.Array(S.String),
  assignmentAuthorityRefs: S.Array(S.String),
  assignmentRef: S.optionalKey(S.String),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  caveatRefs: S.optionalKey(S.Array(S.String)),
  payoutCaveatRefs: S.optionalKey(S.Array(S.String)),
  providerEligibilityRefs: S.Array(S.String),
  providerRefs: S.optionalKey(S.Array(S.String)),
  resourceMode: S.optionalKey(PylonResourceMode),
}) {}

export class PylonMarketplaceTriageJobIntakeRequest extends S.Class<PylonMarketplaceTriageJobIntakeRequest>(
  'PylonMarketplaceTriageJobIntakeRequest',
)({
  assignment: S.optionalKey(PylonMarketplaceAssignmentProposalRequest),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  caveatRefs: S.optionalKey(S.Array(S.String)),
  outcome: PylonMarketplaceTriageOutcome,
}) {}

export class PylonMarketplaceJobApiResponse extends S.Class<PylonMarketplaceJobApiResponse>(
  'PylonMarketplaceJobApiResponse',
)({
  authority: S.Struct({
    buyerChargeMutationAllowed: S.Boolean,
    paidAssignmentDispatchAllowed: S.Boolean,
    payoutMutationAllowed: S.Boolean,
    proposalAllowed: S.Boolean,
    settlementMutationAllowed: S.Boolean,
    triageAllowed: S.Boolean,
  }),
  idempotent: S.Boolean,
  liveDispatchAllowed: S.Boolean,
  operatorProjection: PylonMarketplaceLedgerProjection,
  publicProjection: PylonMarketplaceLedgerProjection,
  settlementMutationAllowed: S.Boolean,
}) {}

export class PylonMarketplaceStoreError extends S.TaggedErrorClass<PylonMarketplaceStoreError>()(
  'PylonMarketplaceStoreError',
  {
    kind: S.Literals([
      'conflict',
      'not_found',
      'storage_error',
      'unsafe_request',
      'validation_error',
    ]),
    reason: S.String,
  },
) {}

export type PylonMarketplaceStoredIntake = Readonly<{
  createdAtIso: string
  idempotencyKey: string
  intakeRef: string
  jobRef: string
  record: PylonMarketplaceJobIntakeRecord
  requestHash: string
  state: string
  updatedAtIso: string
}>

export type PylonMarketplaceStoredAssignment = Readonly<{
  assignmentRef: string
  createdAtIso: string
  idempotencyKey: string
  intakeRef: string
  jobRef: string
  payoutState: string
  record: PylonMarketplaceAssignmentRecord
  requestHash: string
  state: string
  updatedAtIso: string
}>

export type PylonMarketplaceStoredTriageAction = Readonly<{
  createdAtIso: string
  idempotencyKey: string
  outcome: PylonMarketplaceTriageOutcome
  requestHash: string
  response: PylonMarketplaceJobApiResponse
  targetIntakeRef: string
}>

export type PylonMarketplaceJobStore = Readonly<{
  insertAssignment: (
    assignment: PylonMarketplaceStoredAssignment,
  ) => Promise<void>
  insertIntake: (intake: PylonMarketplaceStoredIntake) => Promise<void>
  insertTriageAction: (
    action: PylonMarketplaceStoredTriageAction,
  ) => Promise<void>
  listAssignments: (
    limit: number,
  ) => Promise<ReadonlyArray<PylonMarketplaceStoredAssignment>>
  listIntakes: (
    limit: number,
  ) => Promise<ReadonlyArray<PylonMarketplaceStoredIntake>>
  readIntakeByIdempotencyKey: (
    idempotencyKey: string,
  ) => Promise<PylonMarketplaceStoredIntake | null>
  readIntakeByRef: (
    intakeRef: string,
  ) => Promise<PylonMarketplaceStoredIntake | null>
  readTriageActionByIdempotencyKey: (
    idempotencyKey: string,
  ) => Promise<PylonMarketplaceStoredTriageAction | null>
  updateIntake: (
    intake: PylonMarketplaceStoredIntake,
  ) => Promise<void>
}>

type StoredIntakeRow = Readonly<{
  created_at: string
  idempotency_key: string
  intake_ref: string
  job_ref: string
  record_json: string
  request_hash: string
  state: string
  updated_at: string
}>

type StoredAssignmentRow = Readonly<{
  assignment_ref: string
  created_at: string
  idempotency_key: string
  intake_ref: string
  job_ref: string
  payout_state: string
  record_json: string
  request_hash: string
  state: string
  updated_at: string
}>

type StoredTriageActionRow = Readonly<{
  created_at: string
  idempotency_key: string
  outcome: PylonMarketplaceTriageOutcome
  request_hash: string
  response_json: string
  target_intake_ref: string
}>

const requiredRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)

  if (normalized.length === 0) {
    throw new PylonMarketplaceStoreError({
      kind: 'validation_error',
      reason: `${label} is required.`,
    })
  }

  return normalized
}

const stableValue = (value: unknown): unknown => {
  if (Array.isArray(value)) {
    return value.map(stableValue)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, item]) => [key, stableValue(item)]),
    )
  }

  return value
}

const stableJson = (value: unknown): string => JSON.stringify(stableValue(value))

const recordId = (prefix: string, value: string): string =>
  `${prefix}.${publicRefSegment(value, 'record')}`

const defaultPrivacyClass = (
  source: PylonMarketplaceJobSource,
): typeof PylonMarketplacePrivacyClass.Type =>
  source === 'openagents_seeded' ? 'public' : 'customer_private'

const requestHash = (value: unknown): Promise<string> => sha256Hex(stableJson(value))

const responseFromLedger = (
  intakes: ReadonlyArray<PylonMarketplaceJobIntakeRecord>,
  assignments: ReadonlyArray<PylonMarketplaceAssignmentRecord>,
  nowIso: string,
  idempotent: boolean,
): PylonMarketplaceJobApiResponse => {
  const ledger = new PylonMarketplaceLedgerRecord({
    agentRef: 'agent_artanis',
    assignmentRecords: assignments,
    authority: PYLON_MARKETPLACE_NO_SPEND_AUTHORITY,
    caveatRefs: [
      'caveat.public.marketplace_assignment_not_payment',
      'caveat.public.no_live_spend_without_nexus_authority',
    ],
    intakeRecords: intakes,
    ledgerRef: 'ledger.public.artanis.pylon_marketplace_jobs',
    sourceRefs: [
      'docs/artanis/2026-06-06-pylon-marketplace-job-contract.md',
      'docs/artanis/2026-06-06-pylon-resource-mode-setup.md',
      'docs/artanis/2026-06-06-nexus-pylon-admin-adapters.md',
    ],
    updatedAtIso: nowIso,
  })

  return new PylonMarketplaceJobApiResponse({
    authority: PYLON_MARKETPLACE_NO_SPEND_AUTHORITY,
    idempotent,
    liveDispatchAllowed: false,
    operatorProjection: projectPylonMarketplaceLedger(
      ledger,
      'operator',
      nowIso,
    ),
    publicProjection: projectPylonMarketplaceLedger(
      ledger,
      'public',
      nowIso,
    ),
    settlementMutationAllowed: false,
  })
}

const storedIntakeResponse = (
  intake: PylonMarketplaceStoredIntake,
  assignments: ReadonlyArray<PylonMarketplaceStoredAssignment>,
  nowIso: string,
  idempotent: boolean,
): PylonMarketplaceJobApiResponse =>
  responseFromLedger(
    [intake.record],
    assignments
      .filter(assignment => assignment.intakeRef === intake.intakeRef)
      .map(assignment => assignment.record),
    nowIso,
    idempotent,
  )

const readMarketplaceProjection = async (
  store: PylonMarketplaceJobStore,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
  limit: number,
): Promise<PylonMarketplaceLedgerProjection | null> => {
  const intakes = await store.listIntakes(limit)

  if (intakes.length === 0) {
    return null
  }

  const assignments = await store.listAssignments(limit)
  const ledger = new PylonMarketplaceLedgerRecord({
    agentRef: 'agent_artanis',
    assignmentRecords: assignments.map(assignment => assignment.record),
    authority: PYLON_MARKETPLACE_NO_SPEND_AUTHORITY,
    caveatRefs: ['caveat.public.marketplace_assignment_not_payment'],
    intakeRecords: intakes.map(intake => intake.record),
    ledgerRef: 'ledger.public.artanis.pylon_marketplace_jobs',
    sourceRefs: ['docs/artanis/2026-06-06-pylon-marketplace-job-contract.md'],
    updatedAtIso: nowIso,
  })

  return projectPylonMarketplaceLedger(ledger, audience, nowIso)
}

export const listPylonMarketplaceJobs = async (
  store: PylonMarketplaceJobStore,
  input: Readonly<{
    audience: typeof OmniProjectionAudience.Type
    limit: number
    nowIso: string
  }>,
): Promise<Readonly<{ projection: PylonMarketplaceLedgerProjection | null }>> =>
  ({ projection: await readMarketplaceProjection(
    store,
    input.audience,
    input.nowIso,
    input.limit,
  ) })

const makeIntakeRecord = (
  request: PylonMarketplaceCreateJobIntakeRequest,
  input: Readonly<{ id: string; nowIso: string }>,
): PylonMarketplaceJobIntakeRecord => {
  const source = request.source ?? 'openagents_seeded'
  const privacyClass = request.privacyClass ?? defaultPrivacyClass(source)
  const suffix = publicRefSegment(`${source}_${request.jobKind}_${input.id}`, 'record')
  const intakeRef =
    request.intakeRef ?? recordId('intake.public.pylon_marketplace', suffix)
  const jobRef =
    request.jobRef ?? recordId('job.public.pylon_marketplace', suffix)

  return new PylonMarketplaceJobIntakeRecord({
    benchmarkRefs: uniqueRefs(request.benchmarkRefs ?? [
      `benchmark.public.${request.jobKind}`,
    ]),
    blockerRefs: uniqueRefs(request.blockerRefs),
    budgetRefs: uniqueRefs(request.budgetRefs ?? [
      'budget.public.marketplace.requested_spend_cap',
    ]),
    caveatRefs: uniqueRefs(request.caveatRefs ?? [
      'caveat.public.marketplace_assignment_not_payment',
    ]),
    createdAtIso: input.nowIso,
    dataRefs: uniqueRefs(request.dataRefs ?? [
      `dataset.public.${request.jobKind}.manifest`,
    ]),
    eligibilityRequirementRefs: uniqueRefs(
      request.eligibilityRequirementRefs ?? [
        'eligibility.public.pylon_provider_registered',
        'eligibility.public.resource_mode_supported',
      ],
    ),
    evidenceExpectationRefs: uniqueRefs(request.evidenceExpectationRefs ?? [
      'evidence_expectation.public.redacted_artifact_manifest',
    ]),
    intakeRef,
    jobKind: request.jobKind,
    jobRef,
    modelRefs: uniqueRefs(request.modelRefs ?? [
      `model.public.${request.jobKind}.target`,
    ]),
    policyGateRefs: uniqueRefs(request.policyGateRefs),
    privacyClass,
    requesterRef: request.requesterRef,
    resourceModePreference: request.resourceModePreference,
    resourceRequirementRefs: uniqueRefs(request.resourceRequirementRefs ?? [
      `resource.public.pylon.${request.resourceModePreference}`,
    ]),
    resultExpectationRefs: uniqueRefs(request.resultExpectationRefs ?? [
      `result_expectation.public.${request.jobKind}.summary`,
    ]),
    source,
    sourceRefs: uniqueRefs(request.sourceRefs ?? [
      `source.public.${source}.${request.jobKind}`,
    ]),
    spendCaveatRefs: uniqueRefs(request.spendCaveatRefs ?? [
      'spend.public.no_live_spend_without_nexus_authority',
    ]),
    state: source === 'openagents_seeded' ? 'intake_ready' : 'policy_gated',
    updatedAtIso: input.nowIso,
  })
}

export const createPylonMarketplaceJobIntake = async (
  store: PylonMarketplaceJobStore,
  request: PylonMarketplaceCreateJobIntakeRequest,
  input: Readonly<{
    idempotencyKey: string
    makeId: () => string
    nowIso: string
  }>,
): Promise<PylonMarketplaceJobApiResponse> => {
  const hash = await requestHash(request)
  const existing = await store.readIntakeByIdempotencyKey(input.idempotencyKey)
  const assignments = await store.listAssignments(200)

  if (existing !== null) {
    if (existing.requestHash !== hash) {
      throw new PylonMarketplaceStoreError({
        kind: 'conflict',
        reason:
          'Pylon marketplace intake idempotency key was reused with different content.',
      })
    }

    return storedIntakeResponse(existing, assignments, input.nowIso, true)
  }

  const record = makeIntakeRecord(request, {
    id: input.makeId(),
    nowIso: input.nowIso,
  })
  const response = responseFromLedger([record], [], input.nowIso, false)

  await store.insertIntake({
    createdAtIso: record.createdAtIso,
    idempotencyKey: input.idempotencyKey,
    intakeRef: record.intakeRef,
    jobRef: record.jobRef,
    record,
    requestHash: hash,
    state: record.state,
    updatedAtIso: record.updatedAtIso,
  })

  return response
}

const outcomeState = (
  outcome: PylonMarketplaceTriageOutcome,
): PylonMarketplaceIntakeState => {
  if (outcome === 'proposed_assignment') {
    return 'assignment_proposed'
  }

  return outcome
}

const makeAssignmentRecord = (
  intake: PylonMarketplaceJobIntakeRecord,
  assignment: PylonMarketplaceAssignmentProposalRequest,
  input: Readonly<{ id: string; nowIso: string }>,
): PylonMarketplaceAssignmentRecord =>
  new PylonMarketplaceAssignmentRecord({
    acceptanceCriteriaRefs: requiredRefs(
      'assignment.acceptanceCriteriaRefs',
      assignment.acceptanceCriteriaRefs,
    ),
    acceptedWorkRefs: [],
    artifactEvidenceRefs: [],
    assignmentAuthorityRefs: requiredRefs(
      'assignment.assignmentAuthorityRefs',
      assignment.assignmentAuthorityRefs,
    ),
    assignmentRef: assignment.assignmentRef ??
      recordId('assignment.public.pylon_marketplace', input.id),
    blockerRefs: uniqueRefs(assignment.blockerRefs),
    caveatRefs: uniqueRefs(assignment.caveatRefs ?? [
      'caveat.public.proposed_assignment_not_dispatch',
      'caveat.public.marketplace_assignment_not_payment',
    ]),
    intakeRef: intake.intakeRef,
    jobRef: intake.jobRef,
    nexusReceiptRefs: [],
    payoutCaveatRefs: uniqueRefs(assignment.payoutCaveatRefs ?? [
      'caveat.public.no_payout_before_acceptance_receipts',
    ]),
    payoutState: 'planned',
    providerEligibilityRefs: requiredRefs(
      'assignment.providerEligibilityRefs',
      assignment.providerEligibilityRefs,
    ),
    providerRefs: uniqueRefs(assignment.providerRefs ?? [
      'provider.public.pylon_eligible_pool',
    ]),
    pylonReceiptRefs: [],
    resourceMode: assignment.resourceMode ?? intake.resourceModePreference,
    resultEvidenceRefs: [],
    state: 'proposed',
    treasuryReceiptRefs: [],
    updatedAtIso: input.nowIso,
  })

const updatedIntakeRecord = (
  intake: PylonMarketplaceJobIntakeRecord,
  request: PylonMarketplaceTriageJobIntakeRequest,
  nowIso: string,
): PylonMarketplaceJobIntakeRecord =>
  new PylonMarketplaceJobIntakeRecord({
    ...intake,
    blockerRefs: uniqueRefs([
      ...intake.blockerRefs,
      ...(request.blockerRefs ?? []),
    ]),
    caveatRefs: uniqueRefs([
      ...intake.caveatRefs,
      ...(request.caveatRefs ?? []),
    ]),
    state: outcomeState(request.outcome),
    updatedAtIso: nowIso,
  })

export const triagePylonMarketplaceJobIntake = async (
  store: PylonMarketplaceJobStore,
  intakeRef: string,
  request: PylonMarketplaceTriageJobIntakeRequest,
  input: Readonly<{
    idempotencyKey: string
    makeId: () => string
    nowIso: string
  }>,
): Promise<PylonMarketplaceJobApiResponse> => {
  const hash = await requestHash({ intakeRef, request })
  const existingAction =
    await store.readTriageActionByIdempotencyKey(input.idempotencyKey)

  if (existingAction !== null) {
    if (existingAction.requestHash !== hash) {
      throw new PylonMarketplaceStoreError({
        kind: 'conflict',
        reason:
          'Pylon marketplace triage idempotency key was reused with different content.',
      })
    }

    return new PylonMarketplaceJobApiResponse({
      ...existingAction.response,
      idempotent: true,
    })
  }

  const existingIntake = await store.readIntakeByRef(intakeRef)

  if (existingIntake === null) {
    throw new PylonMarketplaceStoreError({
      kind: 'not_found',
      reason: 'Pylon marketplace intake was not found.',
    })
  }

  if (
    request.outcome === 'proposed_assignment' &&
    request.assignment === undefined
  ) {
    throw new PylonMarketplaceStoreError({
      kind: 'validation_error',
      reason:
        'assignment is required when triage outcome is proposed_assignment.',
    })
  }

  if (
    (request.outcome === 'needs_input' || request.outcome === 'rejected') &&
    uniqueRefs(request.blockerRefs).length === 0
  ) {
    throw new PylonMarketplaceStoreError({
      kind: 'validation_error',
      reason: 'blockerRefs are required for needs_input and rejected triage.',
    })
  }

  const nextIntakeRecord = updatedIntakeRecord(
    existingIntake.record,
    request,
    input.nowIso,
  )
  const nextIntake: PylonMarketplaceStoredIntake = {
    ...existingIntake,
    record: nextIntakeRecord,
    state: nextIntakeRecord.state,
    updatedAtIso: input.nowIso,
  }
  const assignmentRecord = request.outcome === 'proposed_assignment'
    ? makeAssignmentRecord(nextIntakeRecord, request.assignment!, {
        id: input.makeId(),
        nowIso: input.nowIso,
      })
    : null
  const existingAssignments = await store.listAssignments(200)
  const nextAssignments = assignmentRecord === null
    ? existingAssignments.filter(
        assignment => assignment.intakeRef === nextIntake.intakeRef,
      )
    : [
        ...existingAssignments.filter(
          assignment => assignment.intakeRef === nextIntake.intakeRef,
        ),
        {
          assignmentRef: assignmentRecord.assignmentRef,
          createdAtIso: input.nowIso,
          idempotencyKey: input.idempotencyKey,
          intakeRef: assignmentRecord.intakeRef,
          jobRef: assignmentRecord.jobRef,
          payoutState: assignmentRecord.payoutState,
          record: assignmentRecord,
          requestHash: hash,
          state: assignmentRecord.state,
          updatedAtIso: input.nowIso,
        },
      ]
  const response = responseFromLedger(
    [nextIntakeRecord],
    nextAssignments.map(assignment => assignment.record),
    input.nowIso,
    false,
  )

  await store.updateIntake(nextIntake)

  if (assignmentRecord !== null) {
    await store.insertAssignment(nextAssignments[nextAssignments.length - 1]!)
  }

  await store.insertTriageAction({
    createdAtIso: input.nowIso,
    idempotencyKey: input.idempotencyKey,
    outcome: request.outcome,
    requestHash: hash,
    response,
    targetIntakeRef: nextIntake.intakeRef,
  })

  return response
}

const intakeFromRow = (
  row: StoredIntakeRow,
): PylonMarketplaceStoredIntake => ({
  createdAtIso: row.created_at,
  idempotencyKey: row.idempotency_key,
  intakeRef: row.intake_ref,
  jobRef: row.job_ref,
  record: decodeUnknownWithSchema(
    PylonMarketplaceJobIntakeRecord,
    parseJsonUnknown(row.record_json),
  ),
  requestHash: row.request_hash,
  state: row.state,
  updatedAtIso: row.updated_at,
})

const assignmentFromRow = (
  row: StoredAssignmentRow,
): PylonMarketplaceStoredAssignment => ({
  assignmentRef: row.assignment_ref,
  createdAtIso: row.created_at,
  idempotencyKey: row.idempotency_key,
  intakeRef: row.intake_ref,
  jobRef: row.job_ref,
  payoutState: row.payout_state,
  record: decodeUnknownWithSchema(
    PylonMarketplaceAssignmentRecord,
    parseJsonUnknown(row.record_json),
  ),
  requestHash: row.request_hash,
  state: row.state,
  updatedAtIso: row.updated_at,
})

const triageActionFromRow = (
  row: StoredTriageActionRow,
): PylonMarketplaceStoredTriageAction => ({
  createdAtIso: row.created_at,
  idempotencyKey: row.idempotency_key,
  outcome: row.outcome,
  requestHash: row.request_hash,
  response: decodeUnknownWithSchema(
    PylonMarketplaceJobApiResponse,
    parseJsonUnknown(row.response_json),
  ),
  targetIntakeRef: row.target_intake_ref,
})

const storageError = (error: unknown): Error =>
  error instanceof Error ? error : new Error(String(error))

export const makeD1PylonMarketplaceJobStore = (
  db: D1Database,
): PylonMarketplaceJobStore => ({
  insertAssignment: assignment =>
    db
      .prepare(
        `INSERT INTO pylon_marketplace_assignments (
           id,
           assignment_ref,
           intake_ref,
           job_ref,
           idempotency_key,
           request_hash,
           state,
           payout_state,
           record_json,
           created_at,
           updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        `assignment:${assignment.assignmentRef}`,
        assignment.assignmentRef,
        assignment.intakeRef,
        assignment.jobRef,
        assignment.idempotencyKey,
        assignment.requestHash,
        assignment.state,
        assignment.payoutState,
        stableJson(assignment.record),
        assignment.createdAtIso,
        assignment.updatedAtIso,
      )
      .run()
      .then(() => undefined)
      .catch(error => {
        throw storageError(error)
      }),
  insertIntake: intake =>
    db
      .prepare(
        `INSERT INTO pylon_marketplace_job_intakes (
           id,
           intake_ref,
           job_ref,
           idempotency_key,
           request_hash,
           state,
           source,
           job_kind,
           privacy_class,
           record_json,
           created_at,
           updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        `intake:${intake.intakeRef}`,
        intake.intakeRef,
        intake.jobRef,
        intake.idempotencyKey,
        intake.requestHash,
        intake.state,
        intake.record.source,
        intake.record.jobKind,
        intake.record.privacyClass,
        stableJson(intake.record),
        intake.createdAtIso,
        intake.updatedAtIso,
      )
      .run()
      .then(() => undefined)
      .catch(error => {
        throw storageError(error)
      }),
  insertTriageAction: action =>
    db
      .prepare(
        `INSERT INTO pylon_marketplace_triage_actions (
           id,
           target_intake_ref,
           idempotency_key,
           request_hash,
           outcome,
           response_json,
           created_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        `triage:${action.idempotencyKey}`,
        action.targetIntakeRef,
        action.idempotencyKey,
        action.requestHash,
        action.outcome,
        stableJson(action.response),
        action.createdAtIso,
      )
      .run()
      .then(() => undefined)
      .catch(error => {
        throw storageError(error)
      }),
  listAssignments: limit =>
    db
      .prepare(
        `SELECT assignment_ref,
                created_at,
                idempotency_key,
                intake_ref,
                job_ref,
                payout_state,
                record_json,
                request_hash,
                state,
                updated_at
           FROM pylon_marketplace_assignments
          ORDER BY updated_at DESC
          LIMIT ?`,
      )
      .bind(limit)
      .all<StoredAssignmentRow>()
      .then(result => (result.results ?? []).map(assignmentFromRow))
      .catch(error => {
        throw storageError(error)
      }),
  listIntakes: limit =>
    db
      .prepare(
        `SELECT created_at,
                idempotency_key,
                intake_ref,
                job_ref,
                record_json,
                request_hash,
                state,
                updated_at
           FROM pylon_marketplace_job_intakes
          ORDER BY updated_at DESC
          LIMIT ?`,
      )
      .bind(limit)
      .all<StoredIntakeRow>()
      .then(result => (result.results ?? []).map(intakeFromRow))
      .catch(error => {
        throw storageError(error)
      }),
  readIntakeByIdempotencyKey: idempotencyKey =>
    db
      .prepare(
        `SELECT created_at,
                idempotency_key,
                intake_ref,
                job_ref,
                record_json,
                request_hash,
                state,
                updated_at
           FROM pylon_marketplace_job_intakes
          WHERE idempotency_key = ?`,
      )
      .bind(idempotencyKey)
      .first<StoredIntakeRow>()
      .then(row => row === null ? null : intakeFromRow(row))
      .catch(error => {
        throw storageError(error)
      }),
  readIntakeByRef: intakeRef =>
    db
      .prepare(
        `SELECT created_at,
                idempotency_key,
                intake_ref,
                job_ref,
                record_json,
                request_hash,
                state,
                updated_at
           FROM pylon_marketplace_job_intakes
          WHERE intake_ref = ?`,
      )
      .bind(intakeRef)
      .first<StoredIntakeRow>()
      .then(row => row === null ? null : intakeFromRow(row))
      .catch(error => {
        throw storageError(error)
      }),
  readTriageActionByIdempotencyKey: idempotencyKey =>
    db
      .prepare(
        `SELECT created_at,
                idempotency_key,
                outcome,
                request_hash,
                response_json,
                target_intake_ref
           FROM pylon_marketplace_triage_actions
          WHERE idempotency_key = ?`,
      )
      .bind(idempotencyKey)
      .first<StoredTriageActionRow>()
      .then(row => row === null ? null : triageActionFromRow(row))
      .catch(error => {
        throw storageError(error)
      }),
  updateIntake: intake =>
    db
      .prepare(
        `UPDATE pylon_marketplace_job_intakes
            SET state = ?,
                record_json = ?,
                updated_at = ?
          WHERE intake_ref = ?`,
      )
      .bind(
        intake.state,
        stableJson(intake.record),
        intake.updatedAtIso,
        intake.intakeRef,
      )
      .run()
      .then(() => undefined)
      .catch(error => {
        throw storageError(error)
      }),
})

type PylonMarketplaceJobWriteStore = Pick<
  PylonMarketplaceJobStore,
  'insertAssignment' | 'insertIntake' | 'insertTriageAction' | 'updateIntake'
>

export type MakePostgresPylonMarketplaceJobStoreDependencies = Readonly<{
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresPylonMarketplaceJobStore = (
  deps: MakePostgresPylonMarketplaceJobStoreDependencies,
): PylonMarketplaceJobWriteStore => {
  const withSql = async <A>(fn: (sql: SyncSql) => Promise<A>): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(client.sql)
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the sync push route.
      }
    }
  }

  return {
    insertAssignment: assignment =>
      withSql(async sql => {
        await sql`
          INSERT INTO pylon_marketplace_assignments
            (id, assignment_ref, intake_ref, job_ref, idempotency_key,
             request_hash, state, payout_state, record_json, created_at,
             updated_at)
          VALUES
            (${`assignment:${assignment.assignmentRef}`},
             ${assignment.assignmentRef}, ${assignment.intakeRef},
             ${assignment.jobRef}, ${assignment.idempotencyKey},
             ${assignment.requestHash}, ${assignment.state},
             ${assignment.payoutState}, ${stableJson(assignment.record)},
             ${assignment.createdAtIso}, ${assignment.updatedAtIso})
          ON CONFLICT (assignment_ref) DO NOTHING`
      }),

    insertIntake: intake =>
      withSql(async sql => {
        await sql`
          INSERT INTO pylon_marketplace_job_intakes
            (id, intake_ref, job_ref, idempotency_key, request_hash, state,
             source, job_kind, privacy_class, record_json, created_at,
             updated_at)
          VALUES
            (${`intake:${intake.intakeRef}`}, ${intake.intakeRef},
             ${intake.jobRef}, ${intake.idempotencyKey}, ${intake.requestHash},
             ${intake.state}, ${intake.record.source}, ${intake.record.jobKind},
             ${intake.record.privacyClass}, ${stableJson(intake.record)},
             ${intake.createdAtIso}, ${intake.updatedAtIso})
          ON CONFLICT (intake_ref) DO NOTHING`
      }),

    insertTriageAction: action =>
      withSql(async sql => {
        await sql`
          INSERT INTO pylon_marketplace_triage_actions
            (id, target_intake_ref, idempotency_key, request_hash, outcome,
             response_json, created_at)
          VALUES
            (${`triage:${action.idempotencyKey}`}, ${action.targetIntakeRef},
             ${action.idempotencyKey}, ${action.requestHash}, ${action.outcome},
             ${stableJson(action.response)}, ${action.createdAtIso})
          ON CONFLICT (idempotency_key) DO NOTHING`
      }),

    updateIntake: intake =>
      withSql(async sql => {
        await sql`
          INSERT INTO pylon_marketplace_job_intakes
            (id, intake_ref, job_ref, idempotency_key, request_hash, state,
             source, job_kind, privacy_class, record_json, created_at,
             updated_at)
          VALUES
            (${`intake:${intake.intakeRef}`}, ${intake.intakeRef},
             ${intake.jobRef}, ${intake.idempotencyKey}, ${intake.requestHash},
             ${intake.state}, ${intake.record.source}, ${intake.record.jobKind},
             ${intake.record.privacyClass}, ${stableJson(intake.record)},
             ${intake.createdAtIso}, ${intake.updatedAtIso})
          ON CONFLICT (intake_ref) DO UPDATE SET
            state = EXCLUDED.state,
            record_json = EXCLUDED.record_json,
            updated_at = EXCLUDED.updated_at`
      }),
  }
}

export type MakeDualWritePylonMarketplaceJobStoreDependencies = Readonly<{
  d1: PylonMarketplaceJobStore
  flags: Readonly<{ dualWrite: boolean }>
  log?: PylonDispatchLog | undefined
  postgres: PylonMarketplaceJobWriteStore | undefined
}>

const safeMarketplaceMirrorMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

export const makeDualWritePylonMarketplaceJobStore = (
  deps: MakeDualWritePylonMarketplaceJobStoreDependencies,
): PylonMarketplaceJobStore => {
  const { d1, flags, postgres } = deps
  const log = deps.log ?? (() => {})

  if (postgres === undefined) {
    return d1
  }

  const mirror = (
    op: string,
    refs: ReadonlyArray<string>,
    run: () => Promise<void>,
  ): Promise<void> =>
    !flags.dualWrite
      ? Promise.resolve()
      : run().catch((error: unknown) => {
          log('khala_sync_pylon_dual_write_failed', {
            messageSafe: safeMarketplaceMirrorMessage(error),
            op,
            refs,
          })
        })

  return {
    ...d1,
    insertAssignment: async assignment => {
      await d1.insertAssignment(assignment)
      await mirror(
        'insertMarketplaceAssignment',
        [assignment.assignmentRef, assignment.intakeRef],
        () => postgres.insertAssignment(assignment),
      )
    },
    insertIntake: async intake => {
      await d1.insertIntake(intake)
      await mirror(
        'insertMarketplaceIntake',
        [intake.intakeRef, intake.jobRef],
        () => postgres.insertIntake(intake),
      )
    },
    insertTriageAction: async action => {
      await d1.insertTriageAction(action)
      await mirror(
        'insertMarketplaceTriageAction',
        [action.targetIntakeRef, action.idempotencyKey],
        () => postgres.insertTriageAction(action),
      )
    },
    updateIntake: async intake => {
      await d1.updateIntake(intake)
      await mirror(
        'updateMarketplaceIntake',
        [intake.intakeRef, intake.state],
        () => postgres.updateIntake(intake),
      )
    },
  }
}

export type PylonMarketplaceJobStoreEnv = PylonDispatchFlagEnv &
  Readonly<{
    OPENAGENTS_DB: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakePylonMarketplaceJobStoreForEnvOptions = Readonly<{
  log?: PylonDispatchLog | undefined
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
}>

const defaultMarketplaceMirrorLog: PylonDispatchLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

export const makePylonMarketplaceJobStoreForEnv = (
  env: PylonMarketplaceJobStoreEnv,
  options: MakePylonMarketplaceJobStoreForEnvOptions = {},
): PylonMarketplaceJobStore => {
  const d1 = makeD1PylonMarketplaceJobStore(openAgentsDatabase(env))
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  const flags = pylonDispatchFlagsFromEnv(env)

  if (
    connectionString === undefined ||
    connectionString.length === 0 ||
    !flags.dualWrite
  ) {
    return d1
  }

  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  const postgres = makePostgresPylonMarketplaceJobStore({
    acquireSql: () => makeSqlClient(connectionString),
  })

  return makeDualWritePylonMarketplaceJobStore({
    d1,
    flags,
    log: options.log ?? defaultMarketplaceMirrorLog,
    postgres,
  })
}

export const pylonMarketplaceStoreErrorFromUnknown = (
  error: unknown,
): PylonMarketplaceStoreError => {
  if (error instanceof PylonMarketplaceStoreError) {
    return error
  }

  if (error instanceof PylonMarketplaceUnsafe) {
    return new PylonMarketplaceStoreError({
      kind: 'unsafe_request',
      reason: error.reason,
    })
  }

  return new PylonMarketplaceStoreError({
    kind: 'storage_error',
    reason: error instanceof Error ? error.message : String(error),
  })
}
