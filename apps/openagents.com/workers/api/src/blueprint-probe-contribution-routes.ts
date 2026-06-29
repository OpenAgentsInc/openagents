import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import type {
  BlueprintProbeContributionError,
  RecordBlueprintProbeContributionInput,
} from './blueprint/repositories/probe-contributions'
import {
  BlueprintProbeContributionRecord,
  type BlueprintProbeContributionRecord as BlueprintProbeContributionRecordType,
} from './blueprint/repositories/probe-contributions'
import type {
  BlueprintDeveloperPackageContributionCapabilityFamily,
  BlueprintDeveloperPackageContributionRecord,
} from './blueprint/schemas/developer-package-contribution'
import {
  BlueprintProgramFamily as BlueprintProgramFamilySchema,
  BlueprintProgramRiskClass as BlueprintProgramRiskClassSchema,
} from './blueprint/schemas/program'
import type { BlueprintSignatureContributionDraft } from './blueprint/schemas/signature-contribution'
import { BLUEPRINT_DEVELOPER_PACKAGE_CONTRIBUTION_NO_AUTHORITY } from './blueprint/services/developer-package-contribution'
import { BLUEPRINT_SIGNATURE_CONTRIBUTION_NO_AUTHORITY } from './blueprint/services/signature-contribution'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonUnknown } from './json-boundary'
import { currentIsoTimestamp } from './runtime-primitives'

const ProbeBlueprintContributionKind = S.Literals([
  'developer_package_contribution',
  'signature_contribution',
])
type ProbeBlueprintContributionKind = typeof ProbeBlueprintContributionKind.Type

const ProbeBlueprintContributionStatus = S.Literals([
  'approved_for_release_gate',
  'archived',
  'draft',
  'in_review',
  'needs_changes',
  'promoted',
  'rejected',
  'submitted',
])

const ProbeBlueprintContributionReviewStatus = S.Literals([
  'approved',
  'changes_requested',
  'not_requested',
  'pending',
  'rejected',
])

const ProbeBlueprintContributionCapabilityFamily = S.Literals([
  'agent_tool',
  'backend_projection_adapter',
  'context_package',
  'outcome_template',
  'program_signature',
  'retrieval_package',
  'route_policy',
  'tool_package',
  'ui_binding',
  'workroom_template',
])
type ProbeBlueprintContributionCapabilityFamily =
  typeof ProbeBlueprintContributionCapabilityFamily.Type

const ProbeBlueprintContributionAuthority = S.Struct({
  canChangePublicClaims: S.Boolean,
  canCreateSite: S.Boolean,
  canDeploy: S.Boolean,
  canDispatchRuntime: S.Boolean,
  canExecute: S.Boolean,
  canMutateRepository: S.Boolean,
  canPostPublicly: S.Boolean,
  canSendEmail: S.Boolean,
  canSpend: S.Boolean,
  deniedEffectRefs: S.Array(S.String),
})
type ProbeBlueprintContributionAuthority =
  typeof ProbeBlueprintContributionAuthority.Type

export const ProbeBlueprintContributionDraft = S.Struct({
  authority: ProbeBlueprintContributionAuthority,
  backendProjectionAdapterRefs: S.Array(S.String),
  capabilityFamily: ProbeBlueprintContributionCapabilityFamily,
  capabilitySummaryRef: S.String,
  contentRedacted: S.Literal(true),
  contextPackageRefs: S.Array(S.String),
  contributionKind: ProbeBlueprintContributionKind,
  contributorRefs: S.Array(S.String),
  dogfoodScopeRef: S.NullOr(S.String),
  fixtureRefs: S.Array(S.String),
  id: S.String,
  intendedProgramFamily: BlueprintProgramFamilySchema,
  noProductionRuntimeAuthority: S.Literal(true),
  outcomeTemplateRefs: S.Array(S.String),
  paymentAttributionRefs: S.Array(S.String),
  promotionRef: S.NullOr(S.String),
  proposedModuleVersionRefs: S.Array(S.String),
  proposedProgramSignatureRefs: S.Array(S.String),
  proposedProgramTypeRefs: S.Array(S.String),
  rejectionRef: S.NullOr(S.String),
  releaseGateRefs: S.Array(S.String),
  retainedFailureRefs: S.Array(S.String),
  reviewStatus: ProbeBlueprintContributionReviewStatus,
  riskClass: BlueprintProgramRiskClassSchema,
  selfPromotionAttempt: S.Boolean,
  sourceRefs: S.Array(S.String),
  status: ProbeBlueprintContributionStatus,
  toolPackageRefs: S.Array(S.String),
  uiBindingRefs: S.Array(S.String),
})
export type ProbeBlueprintContributionDraft =
  typeof ProbeBlueprintContributionDraft.Type

export const BlueprintProbeContributionIntakeResponse = S.Struct({
  contribution: BlueprintProbeContributionRecord,
  releaseGateReady: S.Boolean,
  runtime: S.Struct({
    candidateRuntimeAllowed: S.Boolean,
    productionRuntimeAllowed: S.Boolean,
  }),
})
export type BlueprintProbeContributionIntakeResponse =
  typeof BlueprintProbeContributionIntakeResponse.Type

type BlueprintProbeContributionRoutesDependencies<Env> = Readonly<{
  listContributions: (
    env: Env,
  ) => Effect.Effect<
    ReadonlyArray<BlueprintProbeContributionRecordType>,
    BlueprintProbeContributionError
  >
  recordContribution: (
    env: Env,
    input: RecordBlueprintProbeContributionInput,
  ) => Effect.Effect<
    BlueprintProbeContributionRecordType,
    BlueprintProbeContributionError
  >
  requireAdminApiToken: (request: Request, env: Env) => Promise<boolean>
  requireContributionIntake: (request: Request, env: Env) => Promise<boolean>
}>

class BlueprintProbeContributionRouteDependencyError extends S.TaggedErrorClass<BlueprintProbeContributionRouteDependencyError>()(
  'BlueprintProbeContributionRouteDependencyError',
  { error: S.Defect },
) {}

class BlueprintProbeContributionIntakeValidationError extends S.TaggedErrorClass<BlueprintProbeContributionIntakeValidationError>()(
  'BlueprintProbeContributionIntakeValidationError',
  { reason: S.String },
) {}

const unauthorizedResponse = () =>
  noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })

const badContributionResponse = (reason: string) =>
  noStoreJsonResponse(
    { error: 'bad_probe_contribution', reason },
    { status: 400 },
  )

const storageErrorResponse = () =>
  noStoreJsonResponse(
    { error: 'blueprint_probe_contribution_storage_error' },
    { status: 500 },
  )

const dependencyErrorResponse = () =>
  noStoreJsonResponse(
    { error: 'blueprint_probe_contribution_dependency_error' },
    { status: 500 },
  )

const PROHIBITED_CONTRIBUTION_TEXT_PATTERN =
  /\b(access_token|bearer|callback[_ -]?(token|url)|checkout_id|cookie|customer[_ -]?(email|name|value)|email[_ -]?body|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|invoice|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|mdk[_ -]?(access[_ -]?token|mnemonic|webhook[_ -]?secret)|mnemonic|oauth|payment[_ -]?(hash|id|preimage)|payout[_ -]?(address|destination|target)|preimage|private[_ -]?(key|repo)|provider[_ -]?(grant|payload|token)|raw[_ -]?(email|payload|prompt|runner|run[_ -]?log|source[_ -]?archive|webhook)|runner[_ -]?log|secret|sk-[a-z0-9]+|source[_ -]?archive|token|wallet|webhook[_ -]?secret|xprv)\b|@/i
const RAW_TIMESTAMP_PATTERN = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const textIsSafe = (text: string): boolean =>
  !containsProviderSecretMaterial(text) &&
  !PROHIBITED_CONTRIBUTION_TEXT_PATTERN.test(text) &&
  !RAW_TIMESTAMP_PATTERN.test(text)

const uniqueStrings = (
  values: ReadonlyArray<string>,
): ReadonlyArray<string> => [
  ...new Set(values.map(value => value.trim()).filter(value => value !== '')),
]

const contributionHasRuntimeAuthority = (
  contribution: ProbeBlueprintContributionDraft,
): boolean =>
  contribution.authority.canChangePublicClaims ||
  contribution.authority.canCreateSite ||
  contribution.authority.canDeploy ||
  contribution.authority.canDispatchRuntime ||
  contribution.authority.canExecute ||
  contribution.authority.canMutateRepository ||
  contribution.authority.canPostPublicly ||
  contribution.authority.canSendEmail ||
  contribution.authority.canSpend

const contributionTargetRefs = (
  contribution: ProbeBlueprintContributionDraft,
): ReadonlyArray<string> =>
  uniqueStrings([
    ...contribution.backendProjectionAdapterRefs,
    ...contribution.contextPackageRefs,
    ...contribution.outcomeTemplateRefs,
    ...contribution.proposedModuleVersionRefs,
    ...contribution.proposedProgramSignatureRefs,
    ...contribution.proposedProgramTypeRefs,
    ...contribution.toolPackageRefs,
    ...contribution.uiBindingRefs,
  ])

const candidateRuntimeAllowed = (
  contribution: ProbeBlueprintContributionDraft,
): boolean =>
  contribution.dogfoodScopeRef !== null &&
  !contributionHasRuntimeAuthority(contribution) &&
  !contribution.selfPromotionAttempt &&
  contribution.noProductionRuntimeAuthority &&
  contribution.status !== 'rejected' &&
  contribution.status !== 'archived' &&
  contribution.rejectionRef === null

const productionRuntimeAllowed = (
  contribution: ProbeBlueprintContributionDraft,
): boolean =>
  !contributionHasRuntimeAuthority(contribution) &&
  !contribution.selfPromotionAttempt &&
  contribution.status === 'promoted' &&
  contribution.promotionRef !== null &&
  contribution.rejectionRef === null

const assertContributionSafe = (
  contribution: ProbeBlueprintContributionDraft,
): Effect.Effect<void, BlueprintProbeContributionIntakeValidationError> => {
  if (!contribution.contentRedacted) {
    return Effect.fail(
      new BlueprintProbeContributionIntakeValidationError({
        reason: 'Probe contributions must be content-redacted.',
      }),
    )
  }

  if (contributionHasRuntimeAuthority(contribution)) {
    return Effect.fail(
      new BlueprintProbeContributionIntakeValidationError({
        reason: 'Probe contributions cannot carry runtime authority.',
      }),
    )
  }

  if (contribution.selfPromotionAttempt) {
    return Effect.fail(
      new BlueprintProbeContributionIntakeValidationError({
        reason: 'Probe contributions cannot self-promote.',
      }),
    )
  }

  if (!contribution.noProductionRuntimeAuthority) {
    return Effect.fail(
      new BlueprintProbeContributionIntakeValidationError({
        reason:
          'Probe contributions must preserve no-production-runtime authority until promotion.',
      }),
    )
  }

  if (
    (contribution.status === 'approved_for_release_gate' ||
      contribution.status === 'promoted') &&
    (contribution.reviewStatus !== 'approved' ||
      contribution.fixtureRefs.length === 0 ||
      contribution.retainedFailureRefs.length === 0 ||
      contribution.releaseGateRefs.length === 0 ||
      contributionTargetRefs(contribution).length === 0)
  ) {
    return Effect.fail(
      new BlueprintProbeContributionIntakeValidationError({
        reason:
          'Release-gated or promoted contributions require approved review, fixture refs, retained failure refs, release gate refs, and target refs.',
      }),
    )
  }

  return Effect.void
}

const packageCapabilityFamily = (
  family: ProbeBlueprintContributionCapabilityFamily,
): BlueprintDeveloperPackageContributionCapabilityFamily => family

const signatureAuthority = (
  authority: ProbeBlueprintContributionAuthority,
): BlueprintSignatureContributionDraft['authority'] => ({
  ...BLUEPRINT_SIGNATURE_CONTRIBUTION_NO_AUTHORITY,
  canChangePublicClaims: authority.canChangePublicClaims,
  canDeploy: authority.canDeploy,
  canExecute: authority.canExecute,
  canMutate:
    authority.canMutateRepository ||
    authority.canDispatchRuntime ||
    authority.canCreateSite ||
    authority.canPostPublicly,
  canSendEmail: authority.canSendEmail,
  canSpend: authority.canSpend,
  deniedEffectRefs:
    authority.deniedEffectRefs.length > 0
      ? authority.deniedEffectRefs
      : BLUEPRINT_SIGNATURE_CONTRIBUTION_NO_AUTHORITY.deniedEffectRefs,
})

const developerAuthority = (
  authority: ProbeBlueprintContributionAuthority,
): BlueprintDeveloperPackageContributionRecord['authority'] => ({
  ...BLUEPRINT_DEVELOPER_PACKAGE_CONTRIBUTION_NO_AUTHORITY,
  ...authority,
  deniedEffectRefs:
    authority.deniedEffectRefs.length > 0
      ? authority.deniedEffectRefs
      : BLUEPRINT_DEVELOPER_PACKAGE_CONTRIBUTION_NO_AUTHORITY.deniedEffectRefs,
})

const firstRef = (refs: ReadonlyArray<string>): string | null => refs[0] ?? null

const signatureContributionFromProbe = (
  contribution: ProbeBlueprintContributionDraft,
  nowIso: string,
): BlueprintSignatureContributionDraft => ({
  authority: signatureAuthority(contribution.authority),
  capabilitySummaryRef: contribution.capabilitySummaryRef,
  contributorRefs: contribution.contributorRefs,
  createdAt: nowIso,
  id: contribution.id,
  intendedFamily: contribution.intendedProgramFamily,
  promotionRef: contribution.promotionRef,
  proposedModuleVersionRef: firstRef(contribution.proposedModuleVersionRefs),
  proposedProgramSignatureRef: firstRef(
    contribution.proposedProgramSignatureRefs,
  ),
  proposedProgramTypeRef: firstRef(contribution.proposedProgramTypeRefs),
  rejectionRef: contribution.rejectionRef,
  releaseGateRefs: contribution.releaseGateRefs,
  requiredFixtureRefs: contribution.fixtureRefs,
  reviewStatus: contribution.reviewStatus,
  riskClass: contribution.riskClass,
  sourceRefs: uniqueStrings([
    ...contribution.sourceRefs,
    ...contribution.retainedFailureRefs,
  ]),
  status: contribution.status,
  updatedAt: nowIso,
})

const developerPackageContributionFromProbe = (
  contribution: ProbeBlueprintContributionDraft,
  nowIso: string,
): BlueprintDeveloperPackageContributionRecord => ({
  authority: developerAuthority(contribution.authority),
  backendProjectionAdapterRefs: contribution.backendProjectionAdapterRefs,
  capabilityFamily: packageCapabilityFamily(contribution.capabilityFamily),
  capabilitySummaryRef: contribution.capabilitySummaryRef,
  contextPackageRefs: contribution.contextPackageRefs,
  contributorRefs: contribution.contributorRefs,
  createdAt: nowIso,
  dogfoodScopeRef: contribution.dogfoodScopeRef,
  id: contribution.id,
  intendedProgramFamily: contribution.intendedProgramFamily,
  noProductionRuntimeAuthority: contribution.noProductionRuntimeAuthority,
  outcomeTemplateRefs: contribution.outcomeTemplateRefs,
  paymentAttributionRefs: contribution.paymentAttributionRefs,
  promotionRef: contribution.promotionRef,
  proposedModuleVersionRefs: contribution.proposedModuleVersionRefs,
  proposedProgramSignatureRefs: contribution.proposedProgramSignatureRefs,
  proposedProgramTypeRefs: contribution.proposedProgramTypeRefs,
  rejectionRef: contribution.rejectionRef,
  releaseGateRefs: contribution.releaseGateRefs,
  requiredFixtureRefs: contribution.fixtureRefs,
  retainedFailureRefs: contribution.retainedFailureRefs,
  reviewStatus: contribution.reviewStatus,
  riskClass: contribution.riskClass,
  selfPromotionAttempt: contribution.selfPromotionAttempt,
  sourceRefs: contribution.sourceRefs,
  status: contribution.status,
  toolPackageRefs: contribution.toolPackageRefs,
  uiBindingRefs: contribution.uiBindingRefs,
  updatedAt: nowIso,
})

const contributionToRecordInput = (
  contribution: ProbeBlueprintContributionDraft,
): Effect.Effect<
  RecordBlueprintProbeContributionInput,
  BlueprintProbeContributionIntakeValidationError
> =>
  Effect.gen(function* () {
    yield* assertContributionSafe(contribution)
    const nowIso = currentIsoTimestamp()

    return {
      candidateRuntimeAllowed: candidateRuntimeAllowed(contribution),
      contributionKind: contribution.contributionKind,
      developerPackageContribution:
        contribution.contributionKind === 'developer_package_contribution'
          ? developerPackageContributionFromProbe(contribution, nowIso)
          : undefined,
      dogfoodScopeRef: contribution.dogfoodScopeRef,
      id: contribution.id,
      idempotencyKey: `probe_blueprint_contribution:${contribution.id}`,
      metadata: {
        noProductionRuntimeAuthority: contribution.noProductionRuntimeAuthority,
        paymentAttributionRefs: contribution.paymentAttributionRefs,
      },
      productionRuntimeAllowed: productionRuntimeAllowed(contribution),
      retainedFailureRefs: contribution.retainedFailureRefs,
      signatureContribution:
        contribution.contributionKind === 'signature_contribution'
          ? signatureContributionFromProbe(contribution, nowIso)
          : undefined,
    }
  })

const readProbeContribution = (
  request: Request,
): Effect.Effect<
  ProbeBlueprintContributionDraft,
  BlueprintProbeContributionIntakeValidationError
> =>
  Effect.gen(function* () {
    const text = yield* Effect.tryPromise({
      catch: () =>
        new BlueprintProbeContributionIntakeValidationError({
          reason: 'Probe contribution body must be readable JSON.',
        }),
      try: () => request.text(),
    })

    if (text.trim() === '') {
      return yield* new BlueprintProbeContributionIntakeValidationError({
        reason: 'Probe contribution body must be a JSON object.',
      })
    }

    if (!textIsSafe(text)) {
      return yield* new BlueprintProbeContributionIntakeValidationError({
        reason:
          'Probe contributions must not include raw prompts, source archives, runner logs, provider material, wallet/payment material, customer data, raw timestamps, or secrets.',
      })
    }

    const value = yield* Effect.try({
      catch: () =>
        new BlueprintProbeContributionIntakeValidationError({
          reason: 'Probe contribution body must be valid JSON.',
        }),
      try: () => parseJsonUnknown(text),
    })

    return yield* S.decodeUnknownEffect(ProbeBlueprintContributionDraft)(
      value,
    ).pipe(
      Effect.mapError(
        () =>
          new BlueprintProbeContributionIntakeValidationError({
            reason:
              'Probe contribution body does not match the Blueprint contribution schema.',
          }),
      ),
    )
  })

const requireOperatorRead = <Env>(
  dependencies: BlueprintProbeContributionRoutesDependencies<Env>,
  request: Request,
  env: Env,
) =>
  Effect.tryPromise({
    catch: error =>
      new BlueprintProbeContributionRouteDependencyError({ error }),
    try: () => dependencies.requireAdminApiToken(request, env),
  })

const requireContributionIntake = <Env>(
  dependencies: BlueprintProbeContributionRoutesDependencies<Env>,
  request: Request,
  env: Env,
) =>
  Effect.tryPromise({
    catch: error =>
      new BlueprintProbeContributionRouteDependencyError({ error }),
    try: () => dependencies.requireContributionIntake(request, env),
  })

const routeErrorResponse = (
  error:
    | BlueprintProbeContributionError
    | BlueprintProbeContributionIntakeValidationError
    | BlueprintProbeContributionRouteDependencyError,
): Response => {
  if (error._tag === 'BlueprintProbeContributionIntakeValidationError') {
    return badContributionResponse(error.reason)
  }

  if (error._tag === 'BlueprintProbeContributionValidationError') {
    return badContributionResponse(error.reason)
  }

  if (error._tag === 'BlueprintProbeContributionStorageError') {
    return storageErrorResponse()
  }

  return dependencyErrorResponse()
}

const routeContributions = <Env>(
  dependencies: BlueprintProbeContributionRoutesDependencies<Env>,
  request: Request,
  env: Env,
) => {
  if (request.method !== 'GET' && request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['GET', 'POST']))
  }

  if (request.method === 'GET') {
    return Effect.gen(function* () {
      const authorized = yield* requireOperatorRead(dependencies, request, env)

      if (!authorized) {
        return unauthorizedResponse()
      }

      const contributions = yield* dependencies.listContributions(env)

      return noStoreJsonResponse({ contributions })
    }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
  }

  return Effect.gen(function* () {
    const authorized = yield* requireContributionIntake(
      dependencies,
      request,
      env,
    )

    if (!authorized) {
      return unauthorizedResponse()
    }

    const contribution = yield* readProbeContribution(request)
    const input = yield* contributionToRecordInput(contribution)
    const record = yield* dependencies.recordContribution(env, input)

    return noStoreJsonResponse(
      {
        contribution: record,
        releaseGateReady: record.releaseGateReady,
        runtime: {
          candidateRuntimeAllowed: record.candidateRuntimeAllowed,
          productionRuntimeAllowed: record.productionRuntimeAllowed,
        },
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
}

export const makeBlueprintProbeContributionRoutes = <Env>(
  dependencies: BlueprintProbeContributionRoutesDependencies<Env>,
) => ({
  handleBlueprintProbeContributionsApi: (
    request: Request,
    env: Env,
  ): Effect.Effect<Response> => routeContributions(dependencies, request, env),
})
