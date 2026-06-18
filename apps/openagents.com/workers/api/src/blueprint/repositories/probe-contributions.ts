import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord, parseJsonStringArray } from '../../json-boundary'
import { compactRandomId, currentIsoTimestamp } from '../../runtime-primitives'
import type {
  BlueprintDeveloperPackageContributionCapabilityFamily,
  BlueprintDeveloperPackageContributionProjection,
  BlueprintDeveloperPackageContributionRecord,
} from '../schemas/developer-package-contribution'
import type {
  BlueprintSignatureContributionDraft,
  BlueprintSignatureContributionProjection,
} from '../schemas/signature-contribution'
import {
  blueprintDeveloperPackageContributionBlockerRefs,
  blueprintDeveloperPackageContributionCanEnterReleaseGate,
  blueprintDeveloperPackageContributionHasRuntimeAuthority,
  projectBlueprintDeveloperPackageContribution,
} from '../services/developer-package-contribution'
import {
  blueprintSignatureContributionDraftBlockerRefs,
  blueprintSignatureContributionDraftCanEnterReleaseGate,
  blueprintSignatureContributionDraftHasRuntimeAuthority,
  projectBlueprintSignatureContributionDraft,
} from '../services/signature-contribution'

export const BlueprintProbeContributionKind = S.Literals([
  'developer_package_contribution',
  'repo_study_packet.v0',
  'signature_contribution',
  'studybench.evidence_span_extraction.v0',
  'studybench.rubric_authoring.v0',
  'studybench.rubric_judging.v0',
  'studybench.task_authoring.v0',
])
export type BlueprintProbeContributionKind =
  typeof BlueprintProbeContributionKind.Type

export const BlueprintStudybenchProbeContributionKind = S.Literals([
  'repo_study_packet.v0',
  'studybench.evidence_span_extraction.v0',
  'studybench.rubric_authoring.v0',
  'studybench.rubric_judging.v0',
  'studybench.task_authoring.v0',
])
export type BlueprintStudybenchProbeContributionKind =
  typeof BlueprintStudybenchProbeContributionKind.Type

export const BLUEPRINT_STUDYBENCH_CONTRIBUTION_CAPABILITY_FAMILY: Record<
  BlueprintStudybenchProbeContributionKind,
  BlueprintDeveloperPackageContributionCapabilityFamily
> = {
  'repo_study_packet.v0': 'context_package',
  'studybench.evidence_span_extraction.v0': 'retrieval_package',
  'studybench.rubric_authoring.v0': 'outcome_template',
  'studybench.rubric_judging.v0': 'outcome_template',
  'studybench.task_authoring.v0': 'context_package',
}

export const BlueprintProbeContributionRecord = S.Struct({
  blockerRefs: S.Array(S.String),
  candidateRuntimeAllowed: S.Boolean,
  contributionKind: BlueprintProbeContributionKind,
  createdAt: S.String,
  developerPackageContribution: S.NullOr(S.Record(S.String, S.Unknown)),
  fixtureRefs: S.Array(S.String),
  id: S.String,
  idempotencyKey: S.String,
  productionRuntimeAllowed: S.Boolean,
  projection: S.Record(S.String, S.Unknown),
  releaseGateReady: S.Boolean,
  releaseGateRefs: S.Array(S.String),
  retainedFailureRefs: S.Array(S.String),
  reviewStatus: S.String,
  signatureContribution: S.NullOr(S.Record(S.String, S.Unknown)),
  status: S.String,
  targetRefs: S.Array(S.String),
  updatedAt: S.String,
})
export type BlueprintProbeContributionRecord =
  typeof BlueprintProbeContributionRecord.Type

export type BlueprintProbeContributionProjection =
  | BlueprintDeveloperPackageContributionProjection
  | BlueprintSignatureContributionProjection

export type BlueprintProbeContributionsRuntime = Readonly<{
  makeContributionId: () => string
  nowIso: () => string
}>

export const systemBlueprintProbeContributionsRuntime: BlueprintProbeContributionsRuntime =
  {
    makeContributionId: () => compactRandomId('blueprint_probe_contribution'),
    nowIso: currentIsoTimestamp,
  }

export type RecordBlueprintProbeContributionInput = Readonly<{
  candidateRuntimeAllowed: boolean
  contributionKind: BlueprintProbeContributionKind
  developerPackageContribution?:
    | BlueprintDeveloperPackageContributionRecord
    | undefined
  dogfoodScopeRef?: string | null | undefined
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  productionRuntimeAllowed: boolean
  retainedFailureRefs: ReadonlyArray<string>
  signatureContribution?: BlueprintSignatureContributionDraft | undefined
}>

type ProbeContributionRow = Readonly<{
  archived_at: string | null
  blocker_refs_json: string
  candidate_runtime_allowed: number
  contribution_kind: BlueprintProbeContributionKind
  created_at: string
  developer_package_contribution_json: string | null
  fixture_refs_json: string
  id: string
  idempotency_key: string
  metadata_json: string
  production_runtime_allowed: number
  projection_json: string
  release_gate_ready: number
  release_gate_refs_json: string
  retained_failure_refs_json: string
  review_status: string
  signature_contribution_json: string | null
  status: string
  target_refs_json: string
  updated_at: string
}>

export class BlueprintProbeContributionValidationError extends S.TaggedErrorClass<BlueprintProbeContributionValidationError>()(
  'BlueprintProbeContributionValidationError',
  { reason: S.String },
) {}

export class BlueprintProbeContributionStorageError extends S.TaggedErrorClass<BlueprintProbeContributionStorageError>()(
  'BlueprintProbeContributionStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export type BlueprintProbeContributionError =
  | BlueprintProbeContributionStorageError
  | BlueprintProbeContributionValidationError

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(access_token|bearer|callback[_. -]?(token|url)|checkout_id|cookie|customer[_. -]?(email|name|value)|email[_. -]?body|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|invoice|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|mdk[_. -]?(access[_. -]?token|mnemonic|webhook[_. -]?secret)|mnemonic|oauth|payment[_. -]?(hash|id|preimage)|payout[_. -]?(address|destination|target)|preimage|private[_. -]?(key|repo)|provider[_. -]?(grant|payload|token)|raw[_. -]?(email|payload|prompt|runner|run[_. -]?log|source[_. -]?archive|webhook)|runner[_. -]?log|secret|sk-[a-z0-9]+|source[_. -]?archive|token|wallet|webhook[_. -]?secret|xprv)\b|@/i
const RAW_TIMESTAMP_PATTERN = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueStrings = (
  values: ReadonlyArray<string>,
): ReadonlyArray<string> => [
  ...new Set(values.map(value => value.trim()).filter(value => value !== '')),
]

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) &&
  !PROHIBITED_TEXT_PATTERN.test(value) &&
  !RAW_TIMESTAMP_PATTERN.test(value)

const assertSafeRef = (
  field: string,
  value: string | null | undefined,
): void => {
  if (value === null || value === undefined) {
    return
  }

  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new BlueprintProbeContributionValidationError({
      reason: `${field} must be a redacted contribution ref without raw prompts, source archives, runner logs, provider material, wallet/payment material, customer data, timestamps, or secrets.`,
    })
  }
}

const assertSafeRefs = (field: string, values: ReadonlyArray<string>): void => {
  values.forEach(value => assertSafeRef(field, value))
}

const assertSafeRecord = (
  field: string,
  value: Readonly<Record<string, unknown>>,
): void => {
  const json = JSON.stringify(value)

  if (
    containsProviderSecretMaterial(json) ||
    PROHIBITED_TEXT_PATTERN.test(json) ||
    RAW_TIMESTAMP_PATTERN.test(json)
  ) {
    throw new BlueprintProbeContributionValidationError({
      reason: `${field} must not contain private contribution material.`,
    })
  }
}

const storageError = (
  operation: string,
  error: unknown,
): BlueprintProbeContributionStorageError =>
  new BlueprintProbeContributionStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, BlueprintProbeContributionStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const contributionTargetRefs = (
  input: RecordBlueprintProbeContributionInput,
): ReadonlyArray<string> =>
  uniqueStrings([
    ...(input.signatureContribution?.proposedProgramTypeRef === null ||
    input.signatureContribution?.proposedProgramTypeRef === undefined
      ? []
      : [input.signatureContribution.proposedProgramTypeRef]),
    ...(input.signatureContribution?.proposedProgramSignatureRef === null ||
    input.signatureContribution?.proposedProgramSignatureRef === undefined
      ? []
      : [input.signatureContribution.proposedProgramSignatureRef]),
    ...(input.signatureContribution?.proposedModuleVersionRef === null ||
    input.signatureContribution?.proposedModuleVersionRef === undefined
      ? []
      : [input.signatureContribution.proposedModuleVersionRef]),
    ...(input.developerPackageContribution?.proposedProgramTypeRefs ?? []),
    ...(input.developerPackageContribution?.proposedProgramSignatureRefs ?? []),
    ...(input.developerPackageContribution?.proposedModuleVersionRefs ?? []),
    ...(input.developerPackageContribution?.backendProjectionAdapterRefs ?? []),
    ...(input.developerPackageContribution?.contextPackageRefs ?? []),
    ...(input.developerPackageContribution?.outcomeTemplateRefs ?? []),
    ...(input.developerPackageContribution?.toolPackageRefs ?? []),
    ...(input.developerPackageContribution?.uiBindingRefs ?? []),
  ])

const releaseGateRefs = (
  input: RecordBlueprintProbeContributionInput,
): ReadonlyArray<string> =>
  uniqueStrings([
    ...(input.signatureContribution?.releaseGateRefs ?? []),
    ...(input.developerPackageContribution?.releaseGateRefs ?? []),
  ])

const fixtureRefs = (
  input: RecordBlueprintProbeContributionInput,
): ReadonlyArray<string> =>
  uniqueStrings([
    ...(input.signatureContribution?.requiredFixtureRefs ?? []),
    ...(input.developerPackageContribution?.requiredFixtureRefs ?? []),
  ])

const contributionStatus = (
  input: RecordBlueprintProbeContributionInput,
): string =>
  input.signatureContribution?.status ??
  input.developerPackageContribution?.status ??
  'draft'

const contributionReviewStatus = (
  input: RecordBlueprintProbeContributionInput,
): string =>
  input.signatureContribution?.reviewStatus ??
  input.developerPackageContribution?.reviewStatus ??
  'not_requested'

const contributionPromotionRef = (
  input: RecordBlueprintProbeContributionInput,
): string | null =>
  input.signatureContribution?.promotionRef ??
  input.developerPackageContribution?.promotionRef ??
  null

const contributionRejectionRef = (
  input: RecordBlueprintProbeContributionInput,
): string | null =>
  input.signatureContribution?.rejectionRef ??
  input.developerPackageContribution?.rejectionRef ??
  null

const contributionHasRuntimeAuthority = (
  input: RecordBlueprintProbeContributionInput,
): boolean =>
  (input.signatureContribution === undefined
    ? false
    : blueprintSignatureContributionDraftHasRuntimeAuthority(
        input.signatureContribution,
      )) ||
  (input.developerPackageContribution === undefined
    ? false
    : blueprintDeveloperPackageContributionHasRuntimeAuthority(
        input.developerPackageContribution,
      ))

const contributionReleaseGateReady = (
  input: RecordBlueprintProbeContributionInput,
): boolean =>
  input.retainedFailureRefs.length > 0 &&
  (input.signatureContribution !== undefined
    ? blueprintSignatureContributionDraftCanEnterReleaseGate(
        input.signatureContribution,
      )
    : input.developerPackageContribution !== undefined &&
      blueprintDeveloperPackageContributionCanEnterReleaseGate(
        input.developerPackageContribution,
      ))

const contributionProductionRuntimeReady = (
  input: RecordBlueprintProbeContributionInput,
): boolean =>
  !contributionHasRuntimeAuthority(input) &&
  input.developerPackageContribution?.selfPromotionAttempt !== true &&
  input.developerPackageContribution?.noProductionRuntimeAuthority !== false &&
  contributionStatus(input) === 'promoted' &&
  contributionReviewStatus(input) === 'approved' &&
  contributionPromotionRef(input) !== null &&
  contributionRejectionRef(input) === null &&
  contributionTargetRefs(input).length > 0 &&
  releaseGateRefs(input).length > 0 &&
  fixtureRefs(input).length > 0 &&
  input.retainedFailureRefs.length > 0

const contributionGateOrProductionReady = (
  input: RecordBlueprintProbeContributionInput,
): boolean =>
  contributionReleaseGateReady(input) ||
  contributionProductionRuntimeReady(input)

const contributionBlockerRefs = (
  input: RecordBlueprintProbeContributionInput,
): ReadonlyArray<string> => {
  if (contributionProductionRuntimeReady(input)) {
    return []
  }

  return uniqueStrings([
    ...(input.signatureContribution === undefined
      ? []
      : blueprintSignatureContributionDraftBlockerRefs(
          input.signatureContribution,
        )),
    ...(input.developerPackageContribution === undefined
      ? []
      : blueprintDeveloperPackageContributionBlockerRefs(
          input.developerPackageContribution,
        )),
    ...(input.productionRuntimeAllowed &&
    !contributionProductionRuntimeReady(input)
      ? ['blocker.probe_contribution.production_runtime_without_promotion']
      : []),
    ...(input.retainedFailureRefs.length === 0
      ? ['blocker.probe_contribution.retained_failure_refs_missing']
      : []),
  ])
}

const contributionProjection = (
  input: RecordBlueprintProbeContributionInput,
): BlueprintProbeContributionProjection =>
  input.signatureContribution !== undefined
    ? projectBlueprintSignatureContributionDraft(input.signatureContribution)
    : projectBlueprintDeveloperPackageContribution(
        input.developerPackageContribution!,
      )

const assertValidInput = (
  input: RecordBlueprintProbeContributionInput,
): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('dogfoodScopeRef', input.dogfoodScopeRef)
  assertSafeRecord('metadata', input.metadata ?? {})

  if (
    input.contributionKind === 'signature_contribution' &&
    input.signatureContribution === undefined
  ) {
    throw new BlueprintProbeContributionValidationError({
      reason:
        'Signature contributions require a normalized signature contribution draft.',
    })
  }

  if (
    input.contributionKind === 'developer_package_contribution' &&
    input.developerPackageContribution === undefined
  ) {
    throw new BlueprintProbeContributionValidationError({
      reason:
        'Developer package contributions require a normalized developer package contribution record.',
    })
  }

  if (
    blueprintProbeContributionKindUsesDeveloperPackage(input.contributionKind) &&
    input.developerPackageContribution === undefined
  ) {
    throw new BlueprintProbeContributionValidationError({
      reason:
        'StudyBench contribution kinds require a normalized developer package contribution record.',
    })
  }

  if (
    input.developerPackageContribution !== undefined &&
    isBlueprintStudybenchProbeContributionKind(input.contributionKind)
  ) {
    const expectedCapabilityFamily =
      BLUEPRINT_STUDYBENCH_CONTRIBUTION_CAPABILITY_FAMILY[
        input.contributionKind
      ]

    if (
      input.developerPackageContribution.capabilityFamily !==
      expectedCapabilityFamily
    ) {
      throw new BlueprintProbeContributionValidationError({
        reason:
          'StudyBench contribution kind must match its mapped Blueprint capability family.',
      })
    }

    if (input.developerPackageContribution.paymentAttributionRefs.length > 0) {
      throw new BlueprintProbeContributionValidationError({
        reason:
          'StudyBench contributions are evidence-only and cannot carry payment attribution refs.',
      })
    }
  }

  if (contributionHasRuntimeAuthority(input)) {
    throw new BlueprintProbeContributionValidationError({
      reason: 'Probe contributions cannot carry runtime authority.',
    })
  }

  if (
    input.productionRuntimeAllowed &&
    !contributionProductionRuntimeReady(input)
  ) {
    throw new BlueprintProbeContributionValidationError({
      reason:
        'Production runtime eligibility requires promoted, release-gated contribution refs.',
    })
  }

  const targets = contributionTargetRefs(input)
  const gates = releaseGateRefs(input)
  const fixtures = fixtureRefs(input)

  assertSafeRefs('targetRefs', targets)
  assertSafeRefs('releaseGateRefs', gates)
  assertSafeRefs('fixtureRefs', fixtures)
  assertSafeRefs('retainedFailureRefs', input.retainedFailureRefs)

  if (
    contributionStatus(input) === 'approved_for_release_gate' ||
    contributionStatus(input) === 'promoted'
  ) {
    if (
      contributionReviewStatus(input) !== 'approved' ||
      targets.length === 0 ||
      gates.length === 0 ||
      fixtures.length === 0 ||
      input.retainedFailureRefs.length === 0
    ) {
      throw new BlueprintProbeContributionValidationError({
        reason:
          'Release-gated or promoted contributions require approved review, targets, release gates, fixtures, and retained failure refs.',
      })
    }
  }
}

export const isBlueprintStudybenchProbeContributionKind = (
  kind: BlueprintProbeContributionKind,
): kind is BlueprintStudybenchProbeContributionKind =>
  kind in BLUEPRINT_STUDYBENCH_CONTRIBUTION_CAPABILITY_FAMILY

const blueprintProbeContributionKindUsesDeveloperPackage = (
  kind: BlueprintProbeContributionKind,
): boolean =>
  kind === 'developer_package_contribution' ||
  isBlueprintStudybenchProbeContributionKind(kind)

const probeContributionFromRow = (
  row: ProbeContributionRow,
): BlueprintProbeContributionRecord => ({
  blockerRefs: parseJsonStringArray(row.blocker_refs_json),
  candidateRuntimeAllowed: row.candidate_runtime_allowed === 1,
  contributionKind: row.contribution_kind,
  createdAt: row.created_at,
  developerPackageContribution:
    parseJsonRecord(row.developer_package_contribution_json) ?? null,
  fixtureRefs: parseJsonStringArray(row.fixture_refs_json),
  id: row.id,
  idempotencyKey: row.idempotency_key,
  productionRuntimeAllowed: row.production_runtime_allowed === 1,
  projection: parseJsonRecord(row.projection_json) ?? {},
  releaseGateReady: row.release_gate_ready === 1,
  releaseGateRefs: parseJsonStringArray(row.release_gate_refs_json),
  retainedFailureRefs: parseJsonStringArray(row.retained_failure_refs_json),
  reviewStatus: row.review_status,
  signatureContribution:
    parseJsonRecord(row.signature_contribution_json) ?? null,
  status: row.status,
  targetRefs: parseJsonStringArray(row.target_refs_json),
  updatedAt: row.updated_at,
})

export const listBlueprintProbeContributions = (
  db: D1Database,
  limit = 100,
): Effect.Effect<
  ReadonlyArray<BlueprintProbeContributionRecord>,
  BlueprintProbeContributionStorageError
> => {
  const boundedLimit = Math.max(1, Math.min(500, Math.trunc(limit)))

  return d1Effect('list blueprint probe contributions', () =>
    db
      .prepare(
        `SELECT *
           FROM blueprint_probe_contributions
          WHERE archived_at IS NULL
          ORDER BY created_at DESC
          LIMIT ?`,
      )
      .bind(boundedLimit)
      .all<ProbeContributionRow>(),
  ).pipe(
    Effect.map(result =>
      (result.results ?? []).map(row => probeContributionFromRow(row)),
    ),
  )
}

const readBlueprintProbeContributionByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<
  BlueprintProbeContributionRecord | null,
  BlueprintProbeContributionStorageError
> =>
  d1Effect('read blueprint probe contribution by idempotency key', () =>
    db
      .prepare(
        `SELECT *
           FROM blueprint_probe_contributions
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<ProbeContributionRow>(),
  ).pipe(
    Effect.map(row => (row === null ? null : probeContributionFromRow(row))),
  )

export const recordBlueprintProbeContribution = (
  db: D1Database,
  input: RecordBlueprintProbeContributionInput,
  runtime: BlueprintProbeContributionsRuntime = systemBlueprintProbeContributionsRuntime,
): Effect.Effect<
  BlueprintProbeContributionRecord,
  BlueprintProbeContributionError
> =>
  Effect.gen(function* () {
    assertValidInput(input)

    const existing = yield* readBlueprintProbeContributionByIdempotencyKey(
      db,
      input.idempotencyKey,
    )

    if (existing !== null) {
      return existing
    }

    const id = input.id ?? runtime.makeContributionId()
    const nowIso = runtime.nowIso()
    const targets = contributionTargetRefs(input)
    const gates = releaseGateRefs(input)
    const fixtures = fixtureRefs(input)
    const ready = contributionGateOrProductionReady(input)
    const blockers = contributionBlockerRefs(input)
    const projection = contributionProjection(input)

    yield* d1Effect('insert blueprint probe contribution', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO blueprint_probe_contributions (
             id,
             idempotency_key,
             contribution_kind,
             status,
             review_status,
             release_gate_ready,
             candidate_runtime_allowed,
             production_runtime_allowed,
             blocker_refs_json,
             release_gate_refs_json,
             fixture_refs_json,
             retained_failure_refs_json,
             target_refs_json,
             signature_contribution_json,
             developer_package_contribution_json,
             projection_json,
             metadata_json,
             created_at,
             updated_at,
             archived_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          id,
          input.idempotencyKey,
          input.contributionKind,
          contributionStatus(input),
          contributionReviewStatus(input),
          ready ? 1 : 0,
          input.candidateRuntimeAllowed ? 1 : 0,
          input.productionRuntimeAllowed ? 1 : 0,
          JSON.stringify(blockers),
          JSON.stringify(gates),
          JSON.stringify(fixtures),
          JSON.stringify(input.retainedFailureRefs),
          JSON.stringify(targets),
          input.signatureContribution === undefined
            ? null
            : JSON.stringify(input.signatureContribution),
          input.developerPackageContribution === undefined
            ? null
            : JSON.stringify(input.developerPackageContribution),
          JSON.stringify(projection),
          JSON.stringify(input.metadata ?? {}),
          nowIso,
          nowIso,
        )
        .run(),
    )

    const inserted = yield* readBlueprintProbeContributionByIdempotencyKey(
      db,
      input.idempotencyKey,
    )

    if (inserted === null) {
      return yield* new BlueprintProbeContributionStorageError({
        operation: 'read inserted blueprint probe contribution',
        reason: 'inserted or existing contribution was not readable.',
      })
    }

    return inserted
  })
