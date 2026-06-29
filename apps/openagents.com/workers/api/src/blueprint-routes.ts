import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import {
  BLUEPRINT_CONTRACT_EXPORT_SEED,
  BlueprintContractExportSeed,
  blueprintContractExportSeedHasCatalogs,
  blueprintContractExportSeedIsPrivateDataSafe,
} from './blueprint/exports/contract-export'
import {
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY,
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF,
} from './blueprint/fixtures/program-registry'
import {
  type BlueprintActionSubmissionError,
  type RecordBlueprintActionSubmissionProposalInput,
} from './blueprint/repositories/action-submissions'
import {
  type BlueprintProgramRunError,
  type RecordBlueprintProgramRunInput,
} from './blueprint/repositories/program-runs'
import {
  type BlueprintTassadarModuleRegistryResolveError,
  listBlueprintTassadarModuleRegistry,
  resolveBlueprintTassadarModuleRegistryEntry,
} from './blueprint/repositories/tassadar-module-registry'
import {
  BlueprintActionSubmission,
  type BlueprintActionSubmission as BlueprintActionSubmissionType,
  type BlueprintActionSubmissionKind,
} from './blueprint/schemas/action-submission'
import {
  BlueprintProgramRunDetailProjection,
  BlueprintProgramRegistryProjection,
  blueprintProgramRegistryProjection,
  blueprintProgramRegistryProjectionIsSafe,
  blueprintProgramRunDetailProjection,
  type BlueprintProgramRegistryProjection as BlueprintProgramRegistryProjectionType,
} from './blueprint/schemas/program-registry'
import type { BlueprintProgramRunRecord } from './blueprint/schemas/program-run'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonUnknown } from './json-boundary'

type HttpResponse = globalThis.Response

const ProbeBlueprintProgramRunUsage = S.Struct({
  completionTokens: S.optionalKey(S.Number),
  promptTokens: S.optionalKey(S.Number),
  totalTokens: S.optionalKey(S.Number),
  truth: S.Literals(['exact', 'estimated', 'unknown']),
})

export const ProbeBlueprintProgramRunEvidence = S.Struct({
  actorRef: S.String,
  assignmentRef: S.optionalKey(S.String),
  authorityBoundary: S.Literal('evidence_only'),
  backendKind: S.String,
  backendProfileId: S.String,
  contentRedacted: S.Literal(true),
  costRef: S.String,
  directMutationDisabled: S.Boolean,
  evidenceRefs: S.Array(S.String),
  inputSnapshotHash: S.String,
  kind: S.Literal('probe_blueprint_program_run_evidence'),
  latencyMs: S.Number,
  lookupId: S.String,
  menuId: S.String,
  model: S.String,
  moduleVersionId: S.String,
  noDeploy: S.Boolean,
  noEmail: S.Boolean,
  noSourceMutation: S.Boolean,
  noSpend: S.Boolean,
  observedAt: S.String,
  orderRef: S.optionalKey(S.String),
  programRunRef: S.String,
  programSignatureId: S.String,
  programTypeId: S.String,
  promptSummaryRef: S.String,
  receiptRefs: S.Array(S.String),
  registryVersionRef: S.String,
  routeRef: S.String,
  runnerRef: S.optionalKey(S.String),
  threadRef: S.optionalKey(S.String),
  toolCallbackRefs: S.Array(S.String),
  typedOutput: S.Record(S.String, S.Unknown),
  usage: ProbeBlueprintProgramRunUsage,
  workroomRef: S.optionalKey(S.String),
})
export type ProbeBlueprintProgramRunEvidence =
  typeof ProbeBlueprintProgramRunEvidence.Type

export const BlueprintProgramRunEvidenceIntakeResponse = S.Struct({
  programRun: BlueprintProgramRunDetailProjection,
  receiptRefs: S.Array(S.String),
  registryVersionRef: S.String,
})
export type BlueprintProgramRunEvidenceIntakeResponse =
  typeof BlueprintProgramRunEvidenceIntakeResponse.Type

const ProbeRequestedEffectKind = S.Literals([
  'create_pull_request',
  'deploy',
  'send_email',
  'post_public_claim',
  'spend_money',
  'legal_sensitive_commitment',
  'mutate_source_backed_business_fact',
  'local_sandbox_file_edit',
  'local_sandbox_read',
  'local_evidence_record',
])
type ProbeRequestedEffectKind = typeof ProbeRequestedEffectKind.Type

export const ProbeBlueprintActionSubmissionProposal = S.Struct({
  actionSubmissionRef: S.String,
  actorRef: S.String,
  approvalPolicyRef: S.String,
  approvalRequired: S.Literal(true),
  assignmentRef: S.optionalKey(S.String),
  contentRedacted: S.Literal(true),
  contextPackRefs: S.Array(S.String),
  directExecution: S.Literal(false),
  directProgramRunExecutionAllowed: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  effectKind: ProbeRequestedEffectKind,
  inputSnapshotHash: S.String,
  kind: S.Literal('probe_blueprint_action_submission_proposal'),
  modelConfidenceBypassDisabled: S.Literal(true),
  moduleVersionId: S.optionalKey(S.String),
  observedAt: S.String,
  programRunAuthorityBoundary: S.Literal('evidence_only'),
  programRunRef: S.String,
  programSignatureId: S.optionalKey(S.String),
  programTypeId: S.optionalKey(S.String),
  proposalOnly: S.Literal(true),
  receiptRefs: S.Array(S.String),
  sourceAuthorityRefs: S.Array(S.String),
  status: S.Literal('proposed'),
  summaryRef: S.String,
  toolRefs: S.Array(S.String),
  typedIntent: S.Record(S.String, S.Unknown),
})
export type ProbeBlueprintActionSubmissionProposal =
  typeof ProbeBlueprintActionSubmissionProposal.Type

export const BlueprintActionSubmissionIntakeResponse = S.Struct({
  actionSubmission: BlueprintActionSubmission,
  receiptRefs: S.Array(S.String),
  reviewRequired: S.Boolean,
})
export type BlueprintActionSubmissionIntakeResponse =
  typeof BlueprintActionSubmissionIntakeResponse.Type

type BlueprintRoutesDependencies<Bindings> = Readonly<{
  contractExportSeed?: typeof BlueprintContractExportSeed.Type
  listActionSubmissions?: (
    env: Bindings,
  ) => Effect.Effect<
    ReadonlyArray<BlueprintActionSubmissionType>,
    BlueprintActionSubmissionError
  >
  listProgramRuns?: (
    env: Bindings,
  ) => Effect.Effect<
    ReadonlyArray<BlueprintProgramRunRecord>,
    BlueprintProgramRunError
  >
  recordActionSubmissionProposal?: (
    env: Bindings,
    input: RecordBlueprintActionSubmissionProposalInput,
  ) => Effect.Effect<BlueprintActionSubmissionType, BlueprintActionSubmissionError>
  recordProgramRun?: (
    env: Bindings,
    input: RecordBlueprintProgramRunInput,
  ) => Effect.Effect<BlueprintProgramRunRecord, BlueprintProgramRunError>
  registryProjection?: BlueprintProgramRegistryProjectionType
  registryVersionRef?: string
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  requireActionSubmissionIntake?: (
    request: Request,
    env: Bindings,
  ) => Promise<boolean>
  requireProgramRunEvidenceIntake?: (
    request: Request,
    env: Bindings,
  ) => Promise<boolean>
}>

class BlueprintRouteDependencyError extends S.TaggedErrorClass<BlueprintRouteDependencyError>()(
  'BlueprintRouteDependencyError',
  {
    error: S.Defect,
  },
) {}

class BlueprintProgramRunEvidenceIntakeValidationError extends S.TaggedErrorClass<BlueprintProgramRunEvidenceIntakeValidationError>()(
  'BlueprintProgramRunEvidenceIntakeValidationError',
  {
    reason: S.String,
  },
) {}

class BlueprintActionSubmissionIntakeValidationError extends S.TaggedErrorClass<BlueprintActionSubmissionIntakeValidationError>()(
  'BlueprintActionSubmissionIntakeValidationError',
  {
    reason: S.String,
  },
) {}

const decodeOrThrow = <A>(
  schema: S.Decoder<A>,
  value: unknown,
): A => S.decodeUnknownSync(schema)(value)

const decodeEffect = <A>(
  schema: S.Decoder<A>,
  value: unknown,
  reason: string,
): Effect.Effect<A, BlueprintProgramRunEvidenceIntakeValidationError> =>
  S.decodeUnknownEffect(schema)(value).pipe(
    Effect.mapError(
      () => new BlueprintProgramRunEvidenceIntakeValidationError({ reason }),
    ),
  )

const unauthorizedResponse = () =>
  noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })

const unsafeRegistryResponse = () =>
  noStoreJsonResponse(
    {
      error: 'unsafe_blueprint_registry_projection',
      reason: 'Blueprint registry projections must be operator-safe refs only.',
    },
    { status: 500 },
  )

const unsafeContractResponse = () =>
  noStoreJsonResponse(
    {
      error: 'unsafe_blueprint_contract_export',
      reason: 'Blueprint contract exports must expose catalogs without private payload material.',
    },
    { status: 500 },
  )

const dependencyErrorResponse = () =>
  noStoreJsonResponse(
    { error: 'blueprint_route_dependency_error' },
    { status: 500 },
  )

const badProgramRunEvidenceResponse = (reason: string) =>
  noStoreJsonResponse(
    { error: 'bad_program_run_evidence', reason },
    { status: 400 },
  )

const badActionSubmissionProposalResponse = (reason: string) =>
  noStoreJsonResponse(
    { error: 'bad_action_submission_proposal', reason },
    { status: 400 },
  )

const storageErrorResponse = () =>
  noStoreJsonResponse(
    { error: 'blueprint_program_run_storage_error' },
    { status: 500 },
  )

const actionSubmissionStorageErrorResponse = () =>
  noStoreJsonResponse(
    { error: 'blueprint_action_submission_storage_error' },
    { status: 500 },
  )

const tassadarRegistryErrorResponse = (
  error: BlueprintTassadarModuleRegistryResolveError,
) => {
  if (error.kind === 'module_not_found') {
    return noStoreJsonResponse(
      { error: 'tassadar_module_not_found', reason: error.reason },
      { status: 404 },
    )
  }

  if (
    error.kind === 'claim_class_refused' ||
    error.kind === 'module_kind_refused' ||
    error.kind === 'trust_posture_refused'
  ) {
    return noStoreJsonResponse(
      { error: 'tassadar_module_refused', reason: error.reason },
      { status: 409 },
    )
  }

  return noStoreJsonResponse(
    { error: 'unsafe_tassadar_module_registry', reason: error.reason },
    { status: 500 },
  )
}

const PROGRAM_RUN_EVIDENCE_PROHIBITED_TEXT_PATTERN =
  /\b(access_token|auth[_ -]?grant|callback[_ -]?(token|url)|callbackToken|callbackUrl|customer[_ -]?(email|name)|customerEmail|customerName|email[_ -]?body|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|mdk_access_token|mnemonic|payment[_ -]?(preimage|secret)|private[_ -]?(file|key|repo)|provider[_ -]?(account|payload|token)|raw[_ -]?(file|prompt|run[_ -]?log|source)|refresh_token|sk-[a-z0-9]+|wallet[_ -]?secret|webhook[_ -]?secret|xprv)\b|@/i

const programRunEvidenceTextIsPrivateDataSafe = (text: string): boolean =>
  !containsProviderSecretMaterial(text) &&
  !PROGRAM_RUN_EVIDENCE_PROHIBITED_TEXT_PATTERN.test(text)

const uniqueStrings = (
  values: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> => [
  ...new Set(values.filter((value): value is string => value !== undefined)),
]

const confidenceFromTypedOutput = (
  typedOutput: Readonly<Record<string, unknown>>,
): number => {
  const confidence = typedOutput.confidence

  return typeof confidence === 'number' &&
    Number.isFinite(confidence) &&
    confidence >= 0 &&
    confidence <= 1
    ? confidence
    : 1
}

const safeRefSegment = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.-]/g, '_')

const actionKindForProbeEffectKind = (
  effectKind: ProbeRequestedEffectKind,
): Effect.Effect<
  BlueprintActionSubmissionKind,
  BlueprintActionSubmissionIntakeValidationError
> => {
  if (effectKind === 'create_pull_request') {
    return Effect.succeed('create_pull_request')
  }

  if (effectKind === 'deploy') {
    return Effect.succeed('deploy')
  }

  if (effectKind === 'send_email') {
    return Effect.succeed('send_email')
  }

  if (effectKind === 'post_public_claim') {
    return Effect.succeed('public_claim_upgrade')
  }

  if (effectKind === 'spend_money') {
    return Effect.succeed('payment')
  }

  if (effectKind === 'legal_sensitive_commitment') {
    return Effect.succeed('legal_sensitive_action')
  }

  if (effectKind === 'mutate_source_backed_business_fact') {
    return Effect.succeed('source_writeback')
  }

  return Effect.fail(
    new BlueprintActionSubmissionIntakeValidationError({
      reason: 'Only external write-side Probe effects can become Blueprint Action Submission proposals.',
    }),
  )
}

const assertProgramRunEvidenceIsEvidenceOnly = (
  evidence: ProbeBlueprintProgramRunEvidence,
): Effect.Effect<void, BlueprintProgramRunEvidenceIntakeValidationError> => {
  if (
    evidence.authorityBoundary === 'evidence_only' &&
    evidence.contentRedacted === true &&
    evidence.directMutationDisabled &&
    evidence.noDeploy &&
    evidence.noEmail &&
    evidence.noSourceMutation &&
    evidence.noSpend
  ) {
    return Effect.void
  }

  return Effect.fail(
    new BlueprintProgramRunEvidenceIntakeValidationError({
      reason: 'Probe Program Run evidence must be evidence-only and cannot carry deploy, email, spend, source mutation, or direct mutation authority.',
    }),
  )
}

const programRunEvidencePurposeRef = (
  evidence: ProbeBlueprintProgramRunEvidence,
  registryProjection: BlueprintProgramRegistryProjectionType,
): Effect.Effect<string, BlueprintProgramRunEvidenceIntakeValidationError> => {
  const programType = registryProjection.programTypes.find(
    candidate => candidate.id === evidence.programTypeId,
  )
  const programSignature = registryProjection.programSignatures.find(
    candidate => candidate.id === evidence.programSignatureId,
  )
  const moduleVersion = registryProjection.moduleVersions.find(
    candidate => candidate.id === evidence.moduleVersionId,
  )

  if (programType === undefined) {
    return Effect.fail(
      new BlueprintProgramRunEvidenceIntakeValidationError({
        reason: 'Program Run evidence references an unknown Blueprint Program Type.',
      }),
    )
  }

  if (
    programSignature === undefined ||
    programSignature.programTypeId !== evidence.programTypeId
  ) {
    return Effect.fail(
      new BlueprintProgramRunEvidenceIntakeValidationError({
        reason: 'Program Run evidence references an unknown or mismatched Blueprint Program Signature.',
      }),
    )
  }

  if (
    moduleVersion === undefined ||
    moduleVersion.programTypeId !== evidence.programTypeId ||
    (moduleVersion.programSignatureId !== null &&
      moduleVersion.programSignatureId !== evidence.programSignatureId)
  ) {
    return Effect.fail(
      new BlueprintProgramRunEvidenceIntakeValidationError({
        reason: 'Program Run evidence references an unknown or mismatched Blueprint Module Version.',
      }),
    )
  }

  return Effect.succeed(programType.purposeRef)
}

const probeProgramRunEvidenceToRecordInput = (
  evidence: ProbeBlueprintProgramRunEvidence,
  registryProjection: BlueprintProgramRegistryProjectionType,
  registryVersionRef: string,
): Effect.Effect<
  RecordBlueprintProgramRunInput,
  BlueprintProgramRunEvidenceIntakeValidationError
> =>
  Effect.gen(function* () {
    yield* assertProgramRunEvidenceIsEvidenceOnly(evidence)

    if (evidence.registryVersionRef !== registryVersionRef) {
      return yield* new BlueprintProgramRunEvidenceIntakeValidationError({
        reason: 'Program Run evidence registryVersionRef does not match the Omega Blueprint registry version.',
      })
    }

    const purposeRef = yield* programRunEvidencePurposeRef(
      evidence,
      registryProjection,
    )

    return {
      actorRef: evidence.actorRef,
      confidence: confidenceFromTypedOutput(evidence.typedOutput),
      costRef: evidence.costRef,
      evidenceRefs: uniqueStrings([
        ...evidence.evidenceRefs,
        evidence.promptSummaryRef,
        ...evidence.toolCallbackRefs,
      ]),
      id: evidence.programRunRef,
      idempotencyKey: `probe_blueprint_program_run:${evidence.programRunRef}`,
      inputSnapshotHash: evidence.inputSnapshotHash,
      latencyMs: evidence.latencyMs,
      metadata: {
        assignmentRef: evidence.assignmentRef,
        backendKind: evidence.backendKind,
        backendProfileId: evidence.backendProfileId,
        contentRedacted: evidence.contentRedacted,
        kind: evidence.kind,
        lookupId: evidence.lookupId,
        menuId: evidence.menuId,
        model: evidence.model,
        observedAt: evidence.observedAt,
        orderRef: evidence.orderRef,
        promptSummaryRef: evidence.promptSummaryRef,
        registryVersionRef: evidence.registryVersionRef,
        runnerRef: evidence.runnerRef,
        threadRef: evidence.threadRef,
        toolCallbackRefs: evidence.toolCallbackRefs,
        usage: evidence.usage,
        workroomRef: evidence.workroomRef,
      },
      moduleVersionId: evidence.moduleVersionId,
      programSignatureId: evidence.programSignatureId,
      programTypeId: evidence.programTypeId,
      purposeRef,
      receiptRefs: evidence.receiptRefs,
      routeRef: evidence.routeRef,
      typedOutput: evidence.typedOutput,
    }
  })

const readProbeProgramRunEvidence = (
  request: Request,
): Effect.Effect<
  ProbeBlueprintProgramRunEvidence,
  BlueprintProgramRunEvidenceIntakeValidationError
> =>
  Effect.gen(function* () {
    const text = yield* Effect.tryPromise({
      catch: () =>
        new BlueprintProgramRunEvidenceIntakeValidationError({
          reason: 'Program Run evidence body must be readable JSON.',
        }),
      try: () => request.text(),
    })

    if (text.trim() === '') {
      return yield* new BlueprintProgramRunEvidenceIntakeValidationError({
        reason: 'Program Run evidence body must be a JSON object.',
      })
    }

    if (!programRunEvidenceTextIsPrivateDataSafe(text)) {
      return yield* new BlueprintProgramRunEvidenceIntakeValidationError({
        reason: 'Program Run evidence must not include raw prompts, callback material, provider payloads, wallet material, private files, private repo refs, customer private data, or provider secrets.',
      })
    }

    const value = yield* Effect.try({
      catch: () =>
        new BlueprintProgramRunEvidenceIntakeValidationError({
          reason: 'Program Run evidence body must be valid JSON.',
        }),
      try: () => parseJsonUnknown(text),
    })

    return yield* decodeEffect(
      ProbeBlueprintProgramRunEvidence,
      value,
      'Program Run evidence body does not match the Probe Blueprint evidence schema.',
    )
  })

const readProbeActionSubmissionProposal = (
  request: Request,
): Effect.Effect<
  ProbeBlueprintActionSubmissionProposal,
  BlueprintActionSubmissionIntakeValidationError
> =>
  Effect.gen(function* () {
    const text = yield* Effect.tryPromise({
      catch: () =>
        new BlueprintActionSubmissionIntakeValidationError({
          reason: 'Action Submission proposal body must be readable JSON.',
        }),
      try: () => request.text(),
    })

    if (text.trim() === '') {
      return yield* new BlueprintActionSubmissionIntakeValidationError({
        reason: 'Action Submission proposal body must be a JSON object.',
      })
    }

    if (!programRunEvidenceTextIsPrivateDataSafe(text)) {
      return yield* new BlueprintActionSubmissionIntakeValidationError({
        reason: 'Action Submission proposals must not include raw prompts, raw emails, callback material, provider payloads, wallet material, private files, private repo refs, customer private data, or provider secrets.',
      })
    }

    const value = yield* Effect.try({
      catch: () =>
        new BlueprintActionSubmissionIntakeValidationError({
          reason: 'Action Submission proposal body must be valid JSON.',
        }),
      try: () => parseJsonUnknown(text),
    })

    return yield* decodeEffect(
      ProbeBlueprintActionSubmissionProposal,
      value,
      'Action Submission proposal body does not match the Probe Blueprint proposal schema.',
    ).pipe(
      Effect.mapError(
        error =>
          new BlueprintActionSubmissionIntakeValidationError({
            reason: error.reason,
          }),
      ),
    )
  })

const assertActionSubmissionProposalIsReviewOnly = (
  proposal: ProbeBlueprintActionSubmissionProposal,
): Effect.Effect<void, BlueprintActionSubmissionIntakeValidationError> => {
  if (
    proposal.approvalRequired &&
    proposal.approvalPolicyRef ===
      'policy.blueprint.action_submission.proposals_only.v1' &&
    proposal.contentRedacted &&
    !proposal.directExecution &&
    !proposal.directProgramRunExecutionAllowed &&
    proposal.modelConfidenceBypassDisabled &&
    proposal.programRunAuthorityBoundary === 'evidence_only' &&
    proposal.proposalOnly &&
    proposal.status === 'proposed' &&
    proposal.evidenceRefs.length > 0
  ) {
    return Effect.void
  }

  return Effect.fail(
    new BlueprintActionSubmissionIntakeValidationError({
      reason: 'Action Submission proposals must be proposal-only, approval-required, evidence-backed, redacted, and disconnected from direct Program Run execution.',
    }),
  )
}

const probeActionSubmissionProposalToRecordInput = (
  proposal: ProbeBlueprintActionSubmissionProposal,
): Effect.Effect<
  RecordBlueprintActionSubmissionProposalInput,
  BlueprintActionSubmissionIntakeValidationError
> =>
  Effect.gen(function* () {
    yield* assertActionSubmissionProposalIsReviewOnly(proposal)
    const actionKind = yield* actionKindForProbeEffectKind(proposal.effectKind)

    return {
      actionKind,
      approvalPolicyRef: proposal.approvalPolicyRef,
      contextPackRefs: proposal.contextPackRefs,
      evidenceRefs: uniqueStrings([
        ...proposal.evidenceRefs,
        proposal.summaryRef,
        ...proposal.contextPackRefs,
        ...proposal.sourceAuthorityRefs,
      ]),
      id: proposal.actionSubmissionRef,
      idempotencyKey: `probe_blueprint_action_submission:${proposal.actionSubmissionRef}`,
      metadata: {
        actorRef: proposal.actorRef,
        assignmentRef: proposal.assignmentRef,
        directExecution: proposal.directExecution,
        directProgramRunExecutionAllowed:
          proposal.directProgramRunExecutionAllowed,
        effectKind: proposal.effectKind,
        inputSnapshotHash: proposal.inputSnapshotHash,
        kind: proposal.kind,
        moduleVersionId: proposal.moduleVersionId,
        observedAt: proposal.observedAt,
        programSignatureId: proposal.programSignatureId,
        programTypeId: proposal.programTypeId,
        typedIntent: proposal.typedIntent,
      },
      proposedByProgramRunId: proposal.programRunRef,
      proposedEffectRef: `effect.probe.${proposal.effectKind}.${safeRefSegment(
        proposal.actionSubmissionRef,
      )}`,
      receiptRefs: proposal.receiptRefs,
      sourceAuthorityRefs: proposal.sourceAuthorityRefs,
      summaryRef: proposal.summaryRef,
      toolRefs: proposal.toolRefs,
    }
  })

const requireOperatorRead = <Bindings>(
  dependencies: BlueprintRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
) =>
  Effect.tryPromise({
    catch: error => new BlueprintRouteDependencyError({ error }),
    try: () => dependencies.requireAdminApiToken(request, env),
  })

const requireProgramRunEvidenceIntake = <Bindings>(
  dependencies: BlueprintRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
) =>
  Effect.tryPromise({
    catch: error => new BlueprintRouteDependencyError({ error }),
    try: () =>
      (dependencies.requireProgramRunEvidenceIntake ??
        dependencies.requireAdminApiToken)(request, env),
  })

const requireActionSubmissionIntake = <Bindings>(
  dependencies: BlueprintRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
) =>
  Effect.tryPromise({
    catch: error => new BlueprintRouteDependencyError({ error }),
    try: () =>
      (dependencies.requireActionSubmissionIntake ??
        dependencies.requireProgramRunEvidenceIntake ??
        dependencies.requireAdminApiToken)(request, env),
  })

const withOperatorRead = <Bindings>(
  dependencies: BlueprintRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  effect: Effect.Effect<HttpResponse, unknown>,
) =>
  Effect.gen(function* () {
    const authorized = yield* requireOperatorRead(dependencies, request, env)

    if (!authorized) {
      return unauthorizedResponse()
    }

    return yield* effect
  }).pipe(Effect.catch(() => Effect.succeed(dependencyErrorResponse())))

const listProgramRuns = <Bindings>(
  dependencies: BlueprintRoutesDependencies<Bindings>,
  env: Bindings,
): Effect.Effect<ReadonlyArray<BlueprintProgramRunRecord>, BlueprintProgramRunError> =>
  dependencies.listProgramRuns?.(env) ?? Effect.succeed([])

const listActionSubmissions = <Bindings>(
  dependencies: BlueprintRoutesDependencies<Bindings>,
  env: Bindings,
): Effect.Effect<
  ReadonlyArray<BlueprintActionSubmissionType>,
  BlueprintActionSubmissionError
> => dependencies.listActionSubmissions?.(env) ?? Effect.succeed([])

const recordProgramRun = <Bindings>(
  dependencies: BlueprintRoutesDependencies<Bindings>,
  env: Bindings,
  input: RecordBlueprintProgramRunInput,
): Effect.Effect<
  BlueprintProgramRunRecord,
  BlueprintProgramRunError | BlueprintRouteDependencyError
> =>
  dependencies.recordProgramRun?.(env, input) ??
  Effect.fail(
    new BlueprintRouteDependencyError({
      error: new Error(
        'Blueprint Program Run evidence intake is not configured.',
      ),
    }),
  )

const recordActionSubmissionProposal = <Bindings>(
  dependencies: BlueprintRoutesDependencies<Bindings>,
  env: Bindings,
  input: RecordBlueprintActionSubmissionProposalInput,
): Effect.Effect<
  BlueprintActionSubmissionType,
  BlueprintActionSubmissionError | BlueprintRouteDependencyError
> =>
  dependencies.recordActionSubmissionProposal?.(env, input) ??
  Effect.fail(
    new BlueprintRouteDependencyError({
      error: new Error(
        'Blueprint Action Submission proposal intake is not configured.',
      ),
    }),
  )

const programRunEvidenceIntakeErrorResponse = (
  error:
    | BlueprintProgramRunError
    | BlueprintProgramRunEvidenceIntakeValidationError
    | BlueprintRouteDependencyError,
): HttpResponse => {
  if (error._tag === 'BlueprintProgramRunEvidenceIntakeValidationError') {
    return badProgramRunEvidenceResponse(error.reason)
  }

  if (error._tag === 'BlueprintProgramRunValidationError') {
    return badProgramRunEvidenceResponse(error.reason)
  }

  if (error._tag === 'BlueprintProgramRunStorageError') {
    return storageErrorResponse()
  }

  return dependencyErrorResponse()
}

const actionSubmissionIntakeErrorResponse = (
  error:
    | BlueprintActionSubmissionError
    | BlueprintActionSubmissionIntakeValidationError
    | BlueprintRouteDependencyError,
): HttpResponse => {
  if (error._tag === 'BlueprintActionSubmissionIntakeValidationError') {
    return badActionSubmissionProposalResponse(error.reason)
  }

  if (error._tag === 'BlueprintActionSubmissionValidationError') {
    return badActionSubmissionProposalResponse(error.reason)
  }

  if (error._tag === 'BlueprintActionSubmissionStorageError') {
    return actionSubmissionStorageErrorResponse()
  }

  return dependencyErrorResponse()
}

const routeProgramRegistry = <Bindings>(
  dependencies: BlueprintRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return withOperatorRead(
    dependencies,
    request,
    env,
    Effect.gen(function* () {
      const baseProjection = decodeOrThrow(
        BlueprintProgramRegistryProjection,
        dependencies.registryProjection ?? AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY,
      )
      const runs = yield* listProgramRuns(dependencies, env)
      const projection =
        runs.length === 0
          ? baseProjection
          : blueprintProgramRegistryProjection({
              moduleVersions: baseProjection.moduleVersions,
              programSignatures: baseProjection.programSignatures,
              programTypes: baseProjection.programTypes,
              releaseGates: baseProjection.releaseGates,
              runs,
            })

      if (!blueprintProgramRegistryProjectionIsSafe(projection)) {
        return unsafeRegistryResponse()
      }

      return noStoreJsonResponse(projection, {
        headers: {
          'x-blueprint-registry-version-ref':
            dependencies.registryVersionRef ??
            AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF,
        },
      })
    }),
  )
}

const routeProgramRunEvidenceIntake = <Bindings>(
  dependencies: BlueprintRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
) => {
  if (request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['POST']))
  }

  return Effect.gen(function* () {
    const authorized = yield* requireProgramRunEvidenceIntake(
      dependencies,
      request,
      env,
    )

    if (!authorized) {
      return unauthorizedResponse()
    }

    const registryVersionRef =
      dependencies.registryVersionRef ??
      AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF
    const registryProjection = decodeOrThrow(
      BlueprintProgramRegistryProjection,
      dependencies.registryProjection ?? AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY,
    )
    const evidence = yield* readProbeProgramRunEvidence(request)
    const input = yield* probeProgramRunEvidenceToRecordInput(
      evidence,
      registryProjection,
      registryVersionRef,
    )
    const programRun = yield* recordProgramRun(dependencies, env, input)
    const projection = blueprintProgramRunDetailProjection(programRun, {
      moduleVersions: registryProjection.moduleVersions,
      releaseGates: registryProjection.releaseGates,
    })

    if (
      !blueprintProgramRegistryProjectionIsSafe({
        ...registryProjection,
        runDetails: [projection],
      })
    ) {
      return unsafeRegistryResponse()
    }

    return noStoreJsonResponse(
      {
        programRun: projection,
        receiptRefs: projection.receiptRefs,
        registryVersionRef,
      },
      { status: 201 },
    )
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(programRunEvidenceIntakeErrorResponse(error)),
    ),
  )
}

const routeActionSubmissions = <Bindings>(
  dependencies: BlueprintRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
) => {
  if (request.method !== 'GET' && request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['GET', 'POST']))
  }

  if (request.method === 'GET') {
    return withOperatorRead(
      dependencies,
      request,
      env,
      Effect.gen(function* () {
        const actionSubmissions = yield* listActionSubmissions(
          dependencies,
          env,
        )

        return noStoreJsonResponse({ actionSubmissions })
      }),
    )
  }

  return Effect.gen(function* () {
    const authorized = yield* requireActionSubmissionIntake(
      dependencies,
      request,
      env,
    )

    if (!authorized) {
      return unauthorizedResponse()
    }

    const proposal = yield* readProbeActionSubmissionProposal(request)
    const input = yield* probeActionSubmissionProposalToRecordInput(proposal)
    const actionSubmission = yield* recordActionSubmissionProposal(
      dependencies,
      env,
      input,
    )

    return noStoreJsonResponse(
      {
        actionSubmission,
        receiptRefs: actionSubmission.receiptRefs,
        reviewRequired:
          actionSubmission.status === 'pending_approval' &&
          actionSubmission.approvalState === 'pending',
      },
      { status: 201 },
    )
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(actionSubmissionIntakeErrorResponse(error)),
    ),
  )
}

const routeContractExport = <Bindings>(
  dependencies: BlueprintRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return withOperatorRead(
    dependencies,
    request,
    env,
    Effect.sync(() => {
      const contractExportSeed = decodeOrThrow(
        BlueprintContractExportSeed,
        dependencies.contractExportSeed ?? BLUEPRINT_CONTRACT_EXPORT_SEED,
      )

      if (
        !blueprintContractExportSeedHasCatalogs(contractExportSeed) ||
        !blueprintContractExportSeedIsPrivateDataSafe(contractExportSeed)
      ) {
        return unsafeContractResponse()
      }

      return noStoreJsonResponse(contractExportSeed)
    }),
  )
}

const optionalModuleKind = (
  value: string | null,
): 'dense_weight_module' | 'linked_dense_module' | undefined => {
  if (value === null) {
    return undefined
  }

  if (value === 'dense_weight_module' || value === 'linked_dense_module') {
    return value
  }

  return undefined
}

const routeTassadarModuleRegistry = <Bindings>(
  dependencies: BlueprintRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return withOperatorRead(
    dependencies,
    request,
    env,
    Effect.gen(function* () {
      const url = new URL(request.url)
      const moduleRef = url.searchParams.get('moduleRef')?.trim()

      if (moduleRef === undefined || moduleRef === '') {
        const projection = yield* listBlueprintTassadarModuleRegistry()
        return noStoreJsonResponse(projection, {
          headers: {
            'x-blueprint-tassadar-module-registry-version-ref':
              projection.registryVersionRef,
          },
        })
      }

      const module = yield* resolveBlueprintTassadarModuleRegistryEntry({
        moduleRef,
        requiredClaimClass:
          url.searchParams.get('requiredClaimClass') ?? undefined,
        requiredModuleKind: optionalModuleKind(
          url.searchParams.get('requiredModuleKind'),
        ),
        requiredTrustPosture:
          url.searchParams.get('requiredTrustPosture') ?? undefined,
      })

      return noStoreJsonResponse(
        {
          module,
          registryVersionRef: module.registryVersionRef,
        },
        {
          headers: {
            'x-blueprint-tassadar-module-registry-version-ref':
              module.registryVersionRef,
          },
        },
      )
    }).pipe(
      Effect.catch(error =>
        Effect.succeed(tassadarRegistryErrorResponse(error)),
      ),
    ),
  )
}

export const makeBlueprintRoutes = <Bindings>(
  dependencies: BlueprintRoutesDependencies<Bindings>,
) => ({
  handleBlueprintActionSubmissionsApi: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> => routeActionSubmissions(dependencies, request, env),
  handleBlueprintContractExportApi: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> => routeContractExport(dependencies, request, env),
  handleBlueprintProgramRunEvidenceApi: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> =>
    routeProgramRunEvidenceIntake(dependencies, request, env),
  handleBlueprintProgramRegistryApi: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> => routeProgramRegistry(dependencies, request, env),
  handleBlueprintTassadarModuleRegistryApi: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> =>
    routeTassadarModuleRegistry(dependencies, request, env),
})
