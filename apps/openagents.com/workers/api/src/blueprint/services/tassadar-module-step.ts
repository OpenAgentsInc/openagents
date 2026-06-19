import {
  TASSADAR_EXECUTOR_CAPABILITY_REF,
} from '@openagentsinc/tassadar-executor'
import {
  executeTassadarDenseWeightModule,
  tassadarDenseProgramFixture,
  tassadarDenseWeightModuleDigest,
} from '@openagentsinc/tassadar-executor/dense-weight-module'
import {
  TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS,
  TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
  TASSADAR_ALM_LINKED_DENSE_REQUIRED_TRUST_POSTURE,
  TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF,
  tassadarLinkedDenseProgramFixture,
  verifyTassadarLinkedDenseComposition,
} from '@openagentsinc/tassadar-executor/linked-dense-module'
import { Effect, Schema as S } from 'effect'

import {
  BLUEPRINT_TASSADAR_DENSE_MODULE_CLAIM_CLASS,
  BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE,
  resolveBlueprintTassadarModuleRegistryEntry,
  type BlueprintTassadarModuleRegistryEntry,
  type BlueprintTassadarModuleRegistryResolver,
} from '../repositories/tassadar-module-registry'
import {
  type BlueprintProgramToolScope,
  BlueprintTassadarModuleStepKind,
} from '../schemas/program'
import { currentIsoTimestamp } from '../../runtime-primitives'

export const BLUEPRINT_TASSADAR_MODULE_FIXTURE_REGISTRY_REF =
  'registry.tassadar_modules.fixture.v0'
export const BLUEPRINT_TASSADAR_DENSE_FIXTURE_MODULE_REF =
  tassadarDenseProgramFixture.denseModule.moduleId
export const BLUEPRINT_TASSADAR_LINKED_FIXTURE_MODULE_REF =
  TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF
export {
  BLUEPRINT_TASSADAR_DENSE_MODULE_CLAIM_CLASS,
  BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE,
}

export const BlueprintTassadarModuleStepVerdict = S.Literals([
  'verified',
  'rejected',
])
export type BlueprintTassadarModuleStepVerdict =
  typeof BlueprintTassadarModuleStepVerdict.Type

export const BlueprintTassadarModuleStepEvidence = S.Struct({
  authorityBoundary: S.Literal('evidence_only'),
  blockerRefs: S.Array(S.String),
  capabilityRef: S.String,
  claimClass: S.String,
  contentRedacted: S.Literal(true),
  directMutationDisabled: S.Literal(true),
  evidenceRefs: S.Array(S.String),
  expectedModuleDigest: S.String,
  expectedTraceDigest: S.String,
  kind: S.Literal('blueprint_tassadar_module_step_evidence'),
  moduleDigest: S.String,
  moduleKind: BlueprintTassadarModuleStepKind,
  moduleRef: S.String,
  noDeploy: S.Literal(true),
  noEmail: S.Literal(true),
  noSourceMutation: S.Literal(true),
  noSpend: S.Literal(true),
  observedAt: S.String,
  receiptRefs: S.Array(S.String),
  registryRef: S.String,
  replayedTraceDigest: S.NullOr(S.String),
  result: S.Record(S.String, S.Unknown),
  stepRef: S.String,
  toolRef: S.String,
  trustPosture: S.String,
  verdict: BlueprintTassadarModuleStepVerdict,
})
export type BlueprintTassadarModuleStepEvidence =
  typeof BlueprintTassadarModuleStepEvidence.Type

export class BlueprintTassadarModuleStepRefused extends S.TaggedErrorClass<BlueprintTassadarModuleStepRefused>()(
  'BlueprintTassadarModuleStepRefused',
  {
    reason: S.String,
    stepRef: S.String,
  },
) {}

export class BlueprintTassadarModuleStepUnsafe extends S.TaggedErrorClass<BlueprintTassadarModuleStepUnsafe>()(
  'BlueprintTassadarModuleStepUnsafe',
  {
    path: S.String,
    reason: S.String,
    stepRef: S.String,
  },
) {}

export type BlueprintTassadarModuleStepError =
  | BlueprintTassadarModuleStepRefused
  | BlueprintTassadarModuleStepUnsafe

const privateFieldPattern =
  /(^|[._-])(access_token|authorization|bearer|callback_url|callback_token|client_secret|customer_email|customer_name|id_token|invoice|mnemonic|oauth|password|payment_hash|payment_id|payment_preimage|payout_address|payout_destination|payout_target|preimage|private_key|private_repo|provider_grant|provider_payload|provider_token|raw_email|raw_payload|raw_prompt|raw_run_log|raw_runner|raw_source_archive|raw_trace|raw_webhook|refresh_token|runner_log|secret|source_archive|token|wallet|xprv)([._-]|$)/i
const privateCamelFieldPattern =
  /^(accessToken|authorization|bearer|callbackUrl|callbackToken|clientSecret|customerEmail|customerName|idToken|invoice|mnemonic|oauth|password|paymentHash|paymentId|paymentPreimage|payoutAddress|payoutDestination|payoutTarget|preimage|privateKey|privateRepo|providerGrant|providerPayload|providerToken|rawEmail|rawPayload|rawPrompt|rawRunLog|rawRunner|rawSourceArchive|rawTrace|rawWebhook|refreshToken|runnerLog|secret|sourceArchive|token|wallet|xprv)$/i
const privateValuePattern =
  /\b(access_token|authorization|bearer|callback_url|callback_token|client_secret|customer_email|customer_name|id_token|invoice|mnemonic|oauth|payment_hash|payment_id|payment_preimage|payout_address|payout_destination|payout_target|preimage|private_key|private_repo|provider_grant|provider_payload|provider_token|raw_email|raw_payload|raw_prompt|raw_run_log|raw_runner|raw_source_archive|raw_trace|raw_webhook|refresh_token|runner_log|source_archive|wallet|xprv)\b/i

const uniqueSorted = (values: ReadonlyArray<string>): Array<string> =>
  [...new Set(values)].sort()

const stepRefForError = (scope: BlueprintProgramToolScope): string =>
  scope.tassadarModuleStep?.stepRef ?? scope.toolRef

const failRefused = (
  scope: BlueprintProgramToolScope,
  reason: string,
): Effect.Effect<never, BlueprintTassadarModuleStepRefused> =>
  Effect.fail(
    new BlueprintTassadarModuleStepRefused({
      reason,
      stepRef: stepRefForError(scope),
    }),
  )

const failUnsafe = (
  stepRef: string,
  path: string,
  reason: string,
): Effect.Effect<never, BlueprintTassadarModuleStepUnsafe> =>
  Effect.fail(new BlueprintTassadarModuleStepUnsafe({ path, reason, stepRef }))

export const executeBlueprintTassadarModuleStep = (
  scope: BlueprintProgramToolScope,
  input?: Readonly<{
    observedAt?: string
    resolveModule?: BlueprintTassadarModuleRegistryResolver
  }>,
): Effect.Effect<
  BlueprintTassadarModuleStepEvidence,
  BlueprintTassadarModuleStepError
> =>
  Effect.gen(function* () {
    const binding = scope.tassadarModuleStep
    if (binding === undefined) {
      return yield* failRefused(
        scope,
        'Blueprint tool scope is not bound to a Tassadar module step',
      )
    }

    if (scope.access !== 'evidence' || scope.requiresApproval) {
      return yield* failRefused(
        scope,
        'Tassadar module steps must stay read/evidence-only and must not require write approval authority',
      )
    }

    if (binding.expectedCapabilityRef !== TASSADAR_EXECUTOR_CAPABILITY_REF) {
      return yield* failRefused(
        scope,
        'Tassadar module step capability envelope is not receipted for the executor lane',
      )
    }

    if (!blueprintTassadarStepProjectionIsSafe({ scope })) {
      return yield* failUnsafe(
        binding.stepRef,
        'scope',
        'Tassadar module step scope contains private-data-shaped material',
      )
    }

    const observedAt = input?.observedAt ?? currentIsoTimestamp()
    const registryEntry = yield* (
      input?.resolveModule ?? resolveBlueprintTassadarModuleRegistryEntry
    )({
      moduleRef: binding.moduleRef,
      requiredClaimClass: binding.expectedClaimClass,
      requiredModuleKind: binding.moduleKind,
      requiredTrustPosture: binding.expectedTrustPosture,
    }).pipe(
      Effect.mapError(
        error =>
          new BlueprintTassadarModuleStepRefused({
            reason: error.reason,
            stepRef: binding.stepRef,
          }),
      ),
    )
    const evidence =
      binding.moduleKind === 'dense_weight_module'
        ? yield* executeDenseStep(scope, observedAt, registryEntry)
        : yield* executeLinkedStep(scope, observedAt, registryEntry)

    if (!blueprintTassadarStepProjectionIsSafe(evidence)) {
      return yield* failUnsafe(
        binding.stepRef,
        'evidence',
        'Tassadar module step evidence contains raw prompt, raw trace, private repo, wallet, provider, customer, payment, or secret material',
      )
    }

    return evidence
  })

const executeDenseStep = (
  scope: BlueprintProgramToolScope,
  observedAt: string,
  registryEntry: BlueprintTassadarModuleRegistryEntry,
): Effect.Effect<BlueprintTassadarModuleStepEvidence, BlueprintTassadarModuleStepRefused> =>
  Effect.gen(function* () {
    const binding = scope.tassadarModuleStep
    if (binding === undefined) {
      return yield* failRefused(scope, 'missing Tassadar step binding')
    }
    if (binding.moduleRef !== BLUEPRINT_TASSADAR_DENSE_FIXTURE_MODULE_REF) {
      return yield* failRefused(scope, 'fixture-bound dense module ref is not resolvable')
    }

    const trace = yield* Effect.tryPromise({
      catch: error =>
        new BlueprintTassadarModuleStepRefused({
          reason: `dense module execution refused: ${String(error)}`,
          stepRef: binding.stepRef,
        }),
      try: () =>
        executeTassadarDenseWeightModule(
          tassadarDenseProgramFixture.denseModule,
          tassadarDenseProgramFixture.steps,
        ),
    })
    const moduleDigestMatches =
      binding.expectedModuleDigest === registryEntry.moduleDigest &&
      registryEntry.moduleDigest === tassadarDenseWeightModuleDigest
    const traceDigestMatches =
      binding.expectedTraceDigest === registryEntry.traceDigest &&
      registryEntry.traceDigest === tassadarDenseProgramFixture.expectedTraceDigest &&
      trace.traceDigest === tassadarDenseProgramFixture.expectedTraceDigest
    const envelopeMatches =
      binding.expectedClaimClass === BLUEPRINT_TASSADAR_DENSE_MODULE_CLAIM_CLASS &&
      binding.expectedTrustPosture === BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE
    const blockerRefs = uniqueSorted([
      ...(moduleDigestMatches
        ? []
        : ['blocker.public.blueprint_tassadar_step.module_digest_mismatch']),
      ...(traceDigestMatches
        ? []
        : ['blocker.public.blueprint_tassadar_step.trace_digest_mismatch']),
      ...(envelopeMatches
        ? []
        : ['blocker.public.blueprint_tassadar_step.capability_envelope_mismatch']),
    ])
    const verified = blockerRefs.length === 0

    return {
      authorityBoundary: 'evidence_only',
      blockerRefs,
      capabilityRef: binding.expectedCapabilityRef,
      claimClass: registryEntry.claimClass,
      contentRedacted: true,
      directMutationDisabled: true,
      evidenceRefs: uniqueSorted(registryEntry.artifactRefs),
      expectedModuleDigest: binding.expectedModuleDigest,
      expectedTraceDigest: binding.expectedTraceDigest,
      kind: 'blueprint_tassadar_module_step_evidence',
      moduleDigest: tassadarDenseWeightModuleDigest,
      moduleKind: 'dense_weight_module',
      moduleRef: binding.moduleRef,
      noDeploy: true,
      noEmail: true,
      noSourceMutation: true,
      noSpend: true,
      observedAt,
      receiptRefs: verified
        ? uniqueSorted([
            ...tassadarDenseProgramFixture.compileReceiptRefs,
            `receipt.openagents.blueprint_tassadar_step.${trace.traceDigest.slice(0, 16)}`,
          ])
        : [],
      registryRef: registryEntry.registryVersionRef,
      replayedTraceDigest: trace.traceDigest,
      result: {
        halted: tassadarDenseProgramFixture.halted,
        outputValues: tassadarDenseProgramFixture.expectedOutputs,
        stepCount: trace.stepOutputs.length,
      },
      stepRef: binding.stepRef,
      toolRef: scope.toolRef,
      trustPosture: registryEntry.trustPosture,
      verdict: verified ? 'verified' : 'rejected',
    }
  })

const executeLinkedStep = (
  scope: BlueprintProgramToolScope,
  observedAt: string,
  registryEntry: BlueprintTassadarModuleRegistryEntry,
): Effect.Effect<BlueprintTassadarModuleStepEvidence, BlueprintTassadarModuleStepRefused> =>
  Effect.gen(function* () {
    const binding = scope.tassadarModuleStep
    if (binding === undefined) {
      return yield* failRefused(scope, 'missing Tassadar step binding')
    }
    if (binding.moduleRef !== BLUEPRINT_TASSADAR_LINKED_FIXTURE_MODULE_REF) {
      return yield* failRefused(scope, 'fixture-bound linked module ref is not resolvable')
    }

    const verification = yield* Effect.tryPromise({
      catch: error =>
        new BlueprintTassadarModuleStepRefused({
          reason: `linked module execution refused: ${String(error)}`,
          stepRef: binding.stepRef,
        }),
      try: () =>
        verifyTassadarLinkedDenseComposition(tassadarLinkedDenseProgramFixture),
    })
    const moduleDigestMatches =
      binding.expectedModuleDigest === registryEntry.moduleDigest &&
      registryEntry.moduleDigest === TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST
    const traceDigestMatches =
      binding.expectedTraceDigest === registryEntry.traceDigest &&
      registryEntry.traceDigest ===
        tassadarLinkedDenseProgramFixture.composedTraceDigest &&
      verification.composedTraceDigest ===
        tassadarLinkedDenseProgramFixture.composedTraceDigest
    const envelopeMatches =
      binding.expectedClaimClass === TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS &&
      binding.expectedTrustPosture === TASSADAR_ALM_LINKED_DENSE_REQUIRED_TRUST_POSTURE
    const blockerRefs = uniqueSorted([
      ...verification.blockerRefs,
      ...(moduleDigestMatches
        ? []
        : ['blocker.public.blueprint_tassadar_step.module_digest_mismatch']),
      ...(traceDigestMatches
        ? []
        : ['blocker.public.blueprint_tassadar_step.trace_digest_mismatch']),
      ...(envelopeMatches
        ? []
        : ['blocker.public.blueprint_tassadar_step.capability_envelope_mismatch']),
    ])
    const verified = blockerRefs.length === 0

    return {
      authorityBoundary: 'evidence_only',
      blockerRefs,
      capabilityRef: binding.expectedCapabilityRef,
      claimClass: registryEntry.claimClass,
      contentRedacted: true,
      directMutationDisabled: true,
      evidenceRefs: uniqueSorted(registryEntry.artifactRefs),
      expectedModuleDigest: binding.expectedModuleDigest,
      expectedTraceDigest: binding.expectedTraceDigest,
      kind: 'blueprint_tassadar_module_step_evidence',
      moduleDigest: TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
      moduleKind: 'linked_dense_module',
      moduleRef: binding.moduleRef,
      noDeploy: true,
      noEmail: true,
      noSourceMutation: true,
      noSpend: true,
      observedAt,
      receiptRefs: verified
        ? uniqueSorted([
            ...tassadarLinkedDenseProgramFixture.compileReceiptRefs,
            ...verification.receiptRefs,
            ...verification.compositionReceiptRefs,
            ...verification.linkCompatibility.receiptRefs,
            `receipt.openagents.blueprint_tassadar_step.${verification.linkedModuleDigest.slice(0, 16)}`,
          ])
        : [],
      registryRef: registryEntry.registryVersionRef,
      replayedTraceDigest: verification.composedTraceDigest,
      result: {
        compositionVerificationCleared:
          verification.compositionVerificationCleared,
        constituentVerificationCount:
          verification.constituentVerifications.length,
        replayVerificationCleared: verification.replayVerificationCleared,
      },
      stepRef: binding.stepRef,
      toolRef: scope.toolRef,
      trustPosture: registryEntry.trustPosture,
      verdict: verified ? 'verified' : 'rejected',
    }
  })

const blueprintTassadarStepProjectionIsSafe = (value: unknown): boolean => {
  if (value === null || value === undefined) {
    return true
  }

  if (typeof value === 'string') {
    return !privateValuePattern.test(value)
  }

  if (Array.isArray(value)) {
    return value.every(blueprintTassadarStepProjectionIsSafe)
  }

  if (typeof value !== 'object') {
    return true
  }

  for (const [key, child] of Object.entries(value)) {
    if (
      privateFieldKey(key) ||
      !blueprintTassadarStepProjectionIsSafe(child)
    ) {
      return false
    }
  }

  return true
}

const privateFieldKey = (key: string): boolean =>
  privateFieldPattern.test(key) || privateCamelFieldPattern.test(key)
