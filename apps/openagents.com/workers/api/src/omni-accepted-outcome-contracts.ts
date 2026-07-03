import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonWithSchema } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const OmniAcceptedOutcomeWorkKind = S.Literals([
  'site',
  'coding',
  'adjustment',
  'existing_project_import',
  'business',
  'legal_sensitive',
])
export type OmniAcceptedOutcomeWorkKind =
  typeof OmniAcceptedOutcomeWorkKind.Type

export const OmniAcceptedOutcomeArtifactKind = S.Literals([
  'site_url',
  'site_version',
  'source_commit',
  'pull_request',
  'diff',
  'build_log',
  'test_report',
  'screenshot',
  'email_receipt',
  'research_brief',
  'redaction_report',
  'operator_receipt',
])
export type OmniAcceptedOutcomeArtifactKind =
  typeof OmniAcceptedOutcomeArtifactKind.Type

export const OmniAcceptedOutcomeReviewPolicy = S.Literals([
  'operator_review',
  'customer_review',
  'dual_review',
  'owner_review',
  'no_review',
])
export type OmniAcceptedOutcomeReviewPolicy =
  typeof OmniAcceptedOutcomeReviewPolicy.Type

export const OmniAcceptedOutcomeAcceptanceState = S.Literals([
  'draft',
  'pending_review',
  'provisionally_accepted',
  'accepted',
  'rejected',
  'revision_requested',
  'reopened',
  'unavailable',
])
export type OmniAcceptedOutcomeAcceptanceState =
  typeof OmniAcceptedOutcomeAcceptanceState.Type

export const OmniAcceptedOutcomeProofPolicy = S.Literals([
  'private_receipt',
  'customer_safe_summary',
  'public_safe_proof',
  'legal_sensitive_private',
])
export type OmniAcceptedOutcomeProofPolicy =
  typeof OmniAcceptedOutcomeProofPolicy.Type

export const OmniAcceptedOutcomeEconomicState = S.Literals([
  'free_beta',
  'paid_required',
  'credits_required',
  'sats_required',
  'internal_only',
])
export type OmniAcceptedOutcomeEconomicState =
  typeof OmniAcceptedOutcomeEconomicState.Type

export const OmniAcceptedOutcomeServicePromiseState = S.Literals([
  'not_promised',
  'proposed',
  'active',
  'fulfilled',
  'paused',
  'breached',
  'cancelled',
])
export type OmniAcceptedOutcomeServicePromiseState =
  typeof OmniAcceptedOutcomeServicePromiseState.Type

export const OmniAcceptedOutcomeBackingCapabilityState = S.Literals([
  'green',
  'yellow',
  'degraded',
  'manual_gate',
  'red',
  'planned',
])
export type OmniAcceptedOutcomeBackingCapabilityState =
  typeof OmniAcceptedOutcomeBackingCapabilityState.Type

export const OmniAcceptedOutcomeFulfillmentReceiptState = S.Literals([
  'fulfilled',
  'partial',
  'failed',
  'blocked',
])
export type OmniAcceptedOutcomeFulfillmentReceiptState =
  typeof OmniAcceptedOutcomeFulfillmentReceiptState.Type

export const OmniAcceptedOutcomeCloseoutRequirementKind = S.Literals([
  'customer_review',
  'operator_review',
  'build_passed',
  'tests_passed',
  'source_exported',
  'email_sent',
  'deployment_live',
  'proof_bundle_ready',
  'legal_review',
  'redaction_passed',
])
export type OmniAcceptedOutcomeCloseoutRequirementKind =
  typeof OmniAcceptedOutcomeCloseoutRequirementKind.Type

export const OmniAcceptedOutcomeExpectedArtifact = S.Struct({
  artifactKind: OmniAcceptedOutcomeArtifactKind,
  publicSafe: S.Boolean,
  required: S.Boolean,
  sourceRef: S.String,
})
export type OmniAcceptedOutcomeExpectedArtifact =
  typeof OmniAcceptedOutcomeExpectedArtifact.Type

export const OmniAcceptedOutcomeCloseoutRequirement = S.Struct({
  requirementKind: OmniAcceptedOutcomeCloseoutRequirementKind,
  required: S.Boolean,
  sourceRef: S.String,
})
export type OmniAcceptedOutcomeCloseoutRequirement =
  typeof OmniAcceptedOutcomeCloseoutRequirement.Type

export const OmniAcceptedOutcomeCommittedDeliverable = S.Struct({
  backingCapabilityRef: S.String,
  backingCapabilityState: OmniAcceptedOutcomeBackingCapabilityState,
  deliverableRef: S.String,
  expectedArtifactKind: OmniAcceptedOutcomeArtifactKind,
  required: S.Boolean,
  sourceRef: S.String,
})
export type OmniAcceptedOutcomeCommittedDeliverable =
  typeof OmniAcceptedOutcomeCommittedDeliverable.Type

export const OmniAcceptedOutcomeSlaTerm = S.Struct({
  dueAt: S.NullOr(S.String),
  metricRef: S.String,
  sourceRef: S.String,
  target: S.Number,
  termRef: S.String,
  unit: S.Literals(['hours', 'days', 'business_days', 'receipts', 'percent']),
})
export type OmniAcceptedOutcomeSlaTerm = typeof OmniAcceptedOutcomeSlaTerm.Type

export const OmniAcceptedOutcomeFulfillmentReceipt = S.Struct({
  blockerRefs: S.Array(S.String),
  deliverableRef: S.String,
  evaluatedAt: S.String,
  evidenceRef: S.String,
  receiptRef: S.String,
  state: OmniAcceptedOutcomeFulfillmentReceiptState,
  verifierRef: S.String,
})
export type OmniAcceptedOutcomeFulfillmentReceipt =
  typeof OmniAcceptedOutcomeFulfillmentReceipt.Type

const ExpectedArtifactArray = S.Array(OmniAcceptedOutcomeExpectedArtifact)
const CloseoutRequirementArray = S.Array(
  OmniAcceptedOutcomeCloseoutRequirement,
)
const CommittedDeliverableArray = S.Array(
  OmniAcceptedOutcomeCommittedDeliverable,
)
const SlaTermArray = S.Array(OmniAcceptedOutcomeSlaTerm)
const FulfillmentReceiptArray = S.Array(
  OmniAcceptedOutcomeFulfillmentReceipt,
)

export const OmniAcceptedOutcomeContractRecord = S.Struct({
  acceptanceState: OmniAcceptedOutcomeAcceptanceState,
  archivedAt: S.NullOr(S.String),
  closeoutRequirements: S.Array(OmniAcceptedOutcomeCloseoutRequirement),
  committedDeliverables: S.Array(OmniAcceptedOutcomeCommittedDeliverable),
  createdAt: S.String,
  customerRef: S.NullOr(S.String),
  economicState: OmniAcceptedOutcomeEconomicState,
  expectedArtifacts: S.Array(OmniAcceptedOutcomeExpectedArtifact),
  fulfillmentReceipts: S.Array(OmniAcceptedOutcomeFulfillmentReceipt),
  id: S.String,
  idempotencyKey: S.String,
  legalSensitive: S.Boolean,
  metadata: S.Record(S.String, S.Unknown),
  proofPolicy: OmniAcceptedOutcomeProofPolicy,
  publicReceiptRef: S.String,
  reviewPolicy: OmniAcceptedOutcomeReviewPolicy,
  servicePromiseState: OmniAcceptedOutcomeServicePromiseState,
  slaTerms: S.Array(OmniAcceptedOutcomeSlaTerm),
  subjectRef: S.String,
  updatedAt: S.String,
  workKind: OmniAcceptedOutcomeWorkKind,
})
export type OmniAcceptedOutcomeContractRecord =
  typeof OmniAcceptedOutcomeContractRecord.Type

export type OmniAcceptedOutcomeContractsRuntime = Readonly<{
  makeContractId: () => string
  nowIso: () => string
}>

export const systemOmniAcceptedOutcomeContractsRuntime: OmniAcceptedOutcomeContractsRuntime =
  {
    makeContractId: () => compactRandomId('omni_accepted_outcome_contract'),
    nowIso: currentIsoTimestamp,
  }

export type CreateOmniAcceptedOutcomeContractInput = Readonly<{
  acceptanceState?: OmniAcceptedOutcomeAcceptanceState | undefined
  closeoutRequirements: ReadonlyArray<OmniAcceptedOutcomeCloseoutRequirement>
  committedDeliverables?:
    | ReadonlyArray<OmniAcceptedOutcomeCommittedDeliverable>
    | undefined
  customerRef?: string | undefined
  economicState: OmniAcceptedOutcomeEconomicState
  expectedArtifacts: ReadonlyArray<OmniAcceptedOutcomeExpectedArtifact>
  fulfillmentReceipts?:
    | ReadonlyArray<OmniAcceptedOutcomeFulfillmentReceipt>
    | undefined
  id?: string | undefined
  idempotencyKey: string
  legalSensitive?: boolean | undefined
  metadata?: Readonly<Record<string, unknown>> | undefined
  proofPolicy: OmniAcceptedOutcomeProofPolicy
  publicReceiptRef?: string | undefined
  reviewPolicy: OmniAcceptedOutcomeReviewPolicy
  servicePromiseState?: OmniAcceptedOutcomeServicePromiseState | undefined
  slaTerms?: ReadonlyArray<OmniAcceptedOutcomeSlaTerm> | undefined
  subjectRef: string
  workKind: OmniAcceptedOutcomeWorkKind
}>

type ContractRow = Readonly<{
  acceptance_state: OmniAcceptedOutcomeAcceptanceState
  archived_at: string | null
  closeout_requirements_json: string
  committed_deliverables_json: string
  created_at: string
  customer_ref: string | null
  economic_state: OmniAcceptedOutcomeEconomicState
  expected_artifacts_json: string
  fulfillment_receipts_json: string
  id: string
  idempotency_key: string
  legal_sensitive: number
  metadata_json: string
  proof_policy: OmniAcceptedOutcomeProofPolicy
  public_receipt_ref: string
  review_policy: OmniAcceptedOutcomeReviewPolicy
  service_promise_state: OmniAcceptedOutcomeServicePromiseState
  sla_terms_json: string
  subject_ref: string
  updated_at: string
  work_kind: OmniAcceptedOutcomeWorkKind
}>

export class OmniAcceptedOutcomeContractValidationError extends S.TaggedErrorClass<OmniAcceptedOutcomeContractValidationError>()(
  'OmniAcceptedOutcomeContractValidationError',
  { reason: S.String },
) {}

export class OmniAcceptedOutcomeContractStorageError extends S.TaggedErrorClass<OmniAcceptedOutcomeContractStorageError>()(
  'OmniAcceptedOutcomeContractStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export type OmniAcceptedOutcomeContractError =
  | OmniAcceptedOutcomeContractStorageError
  | OmniAcceptedOutcomeContractValidationError

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?email|email[_ -]?body|contact[_ -]?email|customer[_ -]?email|customer[_ -]?name|run[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|xprv|mnemonic)\b|@/i

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new OmniAcceptedOutcomeContractValidationError({
      reason: `${field} must be a public-safe ref without raw provider, run log, email, payment, wallet, or private customer material.`,
    })
  }
}

const assertSafeMetadata = (
  metadata: Readonly<Record<string, unknown>> | undefined,
): void => {
  if (metadata === undefined) {
    return
  }

  const json = JSON.stringify(metadata)

  if (
    containsProviderSecretMaterial(json) ||
    PROHIBITED_TEXT_PATTERN.test(json)
  ) {
    throw new OmniAcceptedOutcomeContractValidationError({
      reason:
        'metadata must not contain raw provider, run log, email, payment, wallet, or private customer material.',
    })
  }
}

const assertExpectedArtifacts = (
  artifacts: ReadonlyArray<OmniAcceptedOutcomeExpectedArtifact>,
): void => {
  if (artifacts.length === 0) {
    throw new OmniAcceptedOutcomeContractValidationError({
      reason: 'expectedArtifacts must include at least one artifact.',
    })
  }

  artifacts.forEach(artifact => {
    assertSafeRef('expectedArtifacts.sourceRef', artifact.sourceRef)
  })
}

const assertCloseoutRequirements = (
  requirements: ReadonlyArray<OmniAcceptedOutcomeCloseoutRequirement>,
): void => {
  if (requirements.length === 0) {
    throw new OmniAcceptedOutcomeContractValidationError({
      reason: 'closeoutRequirements must include at least one requirement.',
    })
  }

  requirements.forEach(requirement => {
    assertSafeRef('closeoutRequirements.sourceRef', requirement.sourceRef)
  })
}

const assertCommittedDeliverables = (
  deliverables:
    | ReadonlyArray<OmniAcceptedOutcomeCommittedDeliverable>
    | undefined,
): void => {
  if (deliverables === undefined) {
    return
  }

  deliverables.forEach(deliverable => {
    assertSafeRef('committedDeliverables.deliverableRef', deliverable.deliverableRef)
    assertSafeRef(
      'committedDeliverables.backingCapabilityRef',
      deliverable.backingCapabilityRef,
    )
    assertSafeRef('committedDeliverables.sourceRef', deliverable.sourceRef)

    if (
      deliverable.backingCapabilityState === 'red' ||
      deliverable.backingCapabilityState === 'planned'
    ) {
      throw new OmniAcceptedOutcomeContractValidationError({
        reason:
          'committedDeliverables cannot include a deliverable backed by a red or planned capability record.',
      })
    }
  })
}

const assertSlaTerms = (
  terms: ReadonlyArray<OmniAcceptedOutcomeSlaTerm> | undefined,
): void => {
  if (terms === undefined) {
    return
  }

  terms.forEach(term => {
    assertSafeRef('slaTerms.termRef', term.termRef)
    assertSafeRef('slaTerms.metricRef', term.metricRef)
    assertSafeRef('slaTerms.sourceRef', term.sourceRef)

    if (term.target < 0) {
      throw new OmniAcceptedOutcomeContractValidationError({
        reason: 'slaTerms.target must be non-negative.',
      })
    }
  })
}

const assertFulfillmentReceipts = (
  receipts:
    | ReadonlyArray<OmniAcceptedOutcomeFulfillmentReceipt>
    | undefined,
  committedDeliverables:
    | ReadonlyArray<OmniAcceptedOutcomeCommittedDeliverable>
    | undefined,
): void => {
  if (receipts === undefined) {
    return
  }

  const deliverableRefs = new Set(
    (committedDeliverables ?? []).map(deliverable => deliverable.deliverableRef),
  )

  receipts.forEach(receipt => {
    assertSafeRef('fulfillmentReceipts.receiptRef', receipt.receiptRef)
    assertSafeRef('fulfillmentReceipts.deliverableRef', receipt.deliverableRef)
    assertSafeRef('fulfillmentReceipts.verifierRef', receipt.verifierRef)
    assertSafeRef('fulfillmentReceipts.evidenceRef', receipt.evidenceRef)
    receipt.blockerRefs.forEach(ref =>
      assertSafeRef('fulfillmentReceipts.blockerRefs', ref),
    )

    if (!deliverableRefs.has(receipt.deliverableRef)) {
      throw new OmniAcceptedOutcomeContractValidationError({
        reason:
          'fulfillmentReceipts must reference a committed deliverable; receipts evidence fulfillment and do not create promises.',
      })
    }
  })
}

const assertServicePromiseState = (
  input: CreateOmniAcceptedOutcomeContractInput,
): void => {
  if (
    input.servicePromiseState === 'not_promised' &&
    (input.committedDeliverables?.length ?? 0) > 0
  ) {
    throw new OmniAcceptedOutcomeContractValidationError({
      reason:
        'not_promised contracts cannot carry committedDeliverables; choose an explicit service promise state.',
    })
  }
}

const assertPolicyCompatibility = (
  input: CreateOmniAcceptedOutcomeContractInput,
): void => {
  if (
    input.workKind === 'legal_sensitive' &&
    input.proofPolicy !== 'legal_sensitive_private'
  ) {
    throw new OmniAcceptedOutcomeContractValidationError({
      reason: 'legal_sensitive work must use legal_sensitive_private proof policy.',
    })
  }

  if (
    input.proofPolicy === 'public_safe_proof' &&
    input.expectedArtifacts.some(artifact => !artifact.publicSafe)
  ) {
    throw new OmniAcceptedOutcomeContractValidationError({
      reason: 'public_safe_proof contracts cannot require private artifacts.',
    })
  }
}

const assertValidInput = (
  input: CreateOmniAcceptedOutcomeContractInput,
): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('subjectRef', input.subjectRef)
  assertSafeRef('customerRef', input.customerRef)
  assertSafeRef('publicReceiptRef', input.publicReceiptRef)
  assertSafeMetadata(input.metadata)
  assertExpectedArtifacts(input.expectedArtifacts)
  assertCloseoutRequirements(input.closeoutRequirements)
  assertCommittedDeliverables(input.committedDeliverables)
  assertSlaTerms(input.slaTerms)
  assertFulfillmentReceipts(input.fulfillmentReceipts, input.committedDeliverables)
  assertServicePromiseState(input)
  assertPolicyCompatibility(input)
}

const storageError = (
  operation: string,
  error: unknown,
): OmniAcceptedOutcomeContractStorageError =>
  new OmniAcceptedOutcomeContractStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OmniAcceptedOutcomeContractStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const contractFromRow = (
  row: ContractRow,
): OmniAcceptedOutcomeContractRecord => ({
  acceptanceState: row.acceptance_state,
  archivedAt: row.archived_at,
  closeoutRequirements: parseJsonWithSchema(
    CloseoutRequirementArray,
    row.closeout_requirements_json,
  ),
  committedDeliverables: parseJsonWithSchema(
    CommittedDeliverableArray,
    row.committed_deliverables_json,
  ),
  createdAt: row.created_at,
  customerRef: row.customer_ref,
  economicState: row.economic_state,
  expectedArtifacts: parseJsonWithSchema(
    ExpectedArtifactArray,
    row.expected_artifacts_json,
  ),
  fulfillmentReceipts: parseJsonWithSchema(
    FulfillmentReceiptArray,
    row.fulfillment_receipts_json,
  ),
  id: row.id,
  idempotencyKey: row.idempotency_key,
  legalSensitive: row.legal_sensitive === 1,
  metadata: parseJsonWithSchema(S.Record(S.String, S.Unknown), row.metadata_json),
  proofPolicy: row.proof_policy,
  publicReceiptRef: row.public_receipt_ref,
  reviewPolicy: row.review_policy,
  servicePromiseState: row.service_promise_state,
  slaTerms: parseJsonWithSchema(SlaTermArray, row.sla_terms_json),
  subjectRef: row.subject_ref,
  updatedAt: row.updated_at,
  workKind: row.work_kind,
})

const publicReceiptRef = (
  workKind: OmniAcceptedOutcomeWorkKind,
  idempotencyKey: string,
): string => `omni_accepted_outcome:${workKind}:${idempotencyKey}`

const readContractByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<
  OmniAcceptedOutcomeContractRecord | null,
  OmniAcceptedOutcomeContractStorageError
> =>
  d1Effect('omniAcceptedOutcomeContracts.byIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM omni_accepted_outcome_contracts
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<ContractRow>(),
  ).pipe(Effect.map(row => (row === null ? null : contractFromRow(row))))

export const createOmniAcceptedOutcomeContract = (
  db: D1Database,
  input: CreateOmniAcceptedOutcomeContractInput,
  runtime: OmniAcceptedOutcomeContractsRuntime =
    systemOmniAcceptedOutcomeContractsRuntime,
): Effect.Effect<
  OmniAcceptedOutcomeContractRecord,
  OmniAcceptedOutcomeContractError
> =>
  Effect.gen(function* () {
    assertValidInput(input)

    const existing = yield* readContractByIdempotencyKey(
      db,
      input.idempotencyKey,
    )

    if (existing !== null) {
      return existing
    }

    const now = runtime.nowIso()
    const committedDeliverables = input.committedDeliverables ?? []
    const servicePromiseState =
      input.servicePromiseState ??
      (committedDeliverables.length === 0 ? 'not_promised' : 'active')
    const record: OmniAcceptedOutcomeContractRecord = {
      acceptanceState: input.acceptanceState ?? 'draft',
      archivedAt: null,
      closeoutRequirements: [...input.closeoutRequirements],
      committedDeliverables: [...committedDeliverables],
      createdAt: now,
      customerRef: input.customerRef ?? null,
      economicState: input.economicState,
      expectedArtifacts: [...input.expectedArtifacts],
      fulfillmentReceipts: [...(input.fulfillmentReceipts ?? [])],
      id: input.id ?? runtime.makeContractId(),
      idempotencyKey: input.idempotencyKey,
      legalSensitive:
        input.legalSensitive === true || input.workKind === 'legal_sensitive',
      metadata: input.metadata ?? {},
      proofPolicy: input.proofPolicy,
      publicReceiptRef:
        input.publicReceiptRef ??
        publicReceiptRef(input.workKind, input.idempotencyKey),
      reviewPolicy: input.reviewPolicy,
      servicePromiseState,
      slaTerms: [...(input.slaTerms ?? [])],
      subjectRef: input.subjectRef,
      updatedAt: now,
      workKind: input.workKind,
    }

    yield* d1Effect('omniAcceptedOutcomeContracts.insert', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO omni_accepted_outcome_contracts
             (id,
              idempotency_key,
              work_kind,
              subject_ref,
              customer_ref,
              expected_artifacts_json,
              review_policy,
              acceptance_state,
              proof_policy,
              economic_state,
              closeout_requirements_json,
              committed_deliverables_json,
              service_promise_state,
              sla_terms_json,
              fulfillment_receipts_json,
              legal_sensitive,
              public_receipt_ref,
              metadata_json,
              created_at,
              updated_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          record.id,
          record.idempotencyKey,
          record.workKind,
          record.subjectRef,
          record.customerRef,
          JSON.stringify(record.expectedArtifacts),
          record.reviewPolicy,
          record.acceptanceState,
          record.proofPolicy,
          record.economicState,
          JSON.stringify(record.closeoutRequirements),
          JSON.stringify(record.committedDeliverables),
          record.servicePromiseState,
          JSON.stringify(record.slaTerms),
          JSON.stringify(record.fulfillmentReceipts),
          record.legalSensitive ? 1 : 0,
          record.publicReceiptRef,
          JSON.stringify(record.metadata),
          record.createdAt,
          record.updatedAt,
        )
        .run()
        .then(() => undefined),
    )

    return (
      (yield* readContractByIdempotencyKey(db, record.idempotencyKey)) ??
      record
    )
  })

export const publicOmniAcceptedOutcomeContractProjection = (
  contract: OmniAcceptedOutcomeContractRecord,
) => ({
  acceptanceState: contract.acceptanceState,
  closeoutRequirementCount: contract.closeoutRequirements.length,
  committedDeliverableCount: contract.committedDeliverables.length,
  economicState: contract.economicState,
  expectedArtifactCount: contract.expectedArtifacts.length,
  fulfillmentReceiptSummary: {
    blocked: contract.fulfillmentReceipts.filter(
      receipt => receipt.state === 'blocked',
    ).length,
    failed: contract.fulfillmentReceipts.filter(
      receipt => receipt.state === 'failed',
    ).length,
    fulfilled: contract.fulfillmentReceipts.filter(
      receipt => receipt.state === 'fulfilled',
    ).length,
    partial: contract.fulfillmentReceipts.filter(
      receipt => receipt.state === 'partial',
    ).length,
    total: contract.fulfillmentReceipts.length,
  },
  legalSensitive: contract.legalSensitive,
  proofPolicy: contract.proofPolicy,
  publicCommittedDeliverables: contract.committedDeliverables.map(
    deliverable => ({
      backingCapabilityRef: deliverable.backingCapabilityRef,
      backingCapabilityState: deliverable.backingCapabilityState,
      deliverableRef: deliverable.deliverableRef,
      expectedArtifactKind: deliverable.expectedArtifactKind,
      required: deliverable.required,
    }),
  ),
  publicExpectedArtifacts: contract.expectedArtifacts
    .filter(artifact => artifact.publicSafe)
    .map(artifact => ({
      artifactKind: artifact.artifactKind,
      required: artifact.required,
      sourceRef: artifact.sourceRef,
    })),
  publicReceiptRef: contract.publicReceiptRef,
  reviewPolicy: contract.reviewPolicy,
  servicePromiseState: contract.servicePromiseState,
  slaTermCount: contract.slaTerms.length,
  subjectRef: contract.subjectRef,
  workKind: contract.workKind,
})
