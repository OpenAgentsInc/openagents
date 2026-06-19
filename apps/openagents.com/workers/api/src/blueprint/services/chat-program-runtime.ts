import { Effect, Schema as S } from 'effect'

import { compactRandomId, currentIsoTimestamp } from '../../runtime-primitives'
import {
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY,
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF,
} from '../fixtures/program-registry'
import type { BlueprintModuleVersion } from '../schemas/module'
import {
  BlueprintObjectiveSurface,
  type BlueprintObjectiveSurface as BlueprintObjectiveSurfaceType,
} from '../schemas/objective'
import {
  BlueprintProgramFamily,
  BlueprintProgramRiskClass,
  type BlueprintProgramRiskClass as BlueprintProgramRiskClassType,
  type BlueprintProgramSignature as BlueprintProgramSignatureType,
  type BlueprintProgramStatus as BlueprintProgramStatusType,
  BlueprintProgramToolScope,
  type BlueprintProgramToolScope as BlueprintProgramToolScopeType,
  BlueprintReplayModuleBinding,
  BlueprintTassadarModuleStepBinding,
  type BlueprintProgramType,
  BlueprintToolAccess,
} from '../schemas/program'
import {
  blueprintProgramRegistryProjectionIsSafe,
  type BlueprintProgramRegistryEntry,
  type BlueprintProgramRegistryProjection as BlueprintProgramRegistryProjectionType,
} from '../schemas/program-registry'
import {
  BlueprintProgramRunRecord,
  type BlueprintProgramRunRecord as BlueprintProgramRunRecordType,
} from '../schemas/program-run'
import {
  assertProgramRunEvidenceOnly,
  BlueprintProgramRunDirectEffectKind,
  BlueprintProgramRunDirectEffectDenied,
  denyProgramRunDirectEffect,
} from './program-run-authority'
import {
  BlueprintTassadarModuleStepEvidence,
  type BlueprintTassadarModuleStepEvidence as BlueprintTassadarModuleStepEvidenceType,
  executeBlueprintTassadarModuleStep,
} from './tassadar-module-step'
import {
  BlueprintReplayModuleEvidence,
  type BlueprintReplayModuleEvidence as BlueprintReplayModuleEvidenceType,
  type BlueprintReplayModuleRuntime,
  executeBlueprintReplayModule,
} from './replay-module'

export const BlueprintChatProgramSessionAdapter = S.Literals([
  'codex',
  'claude_agent',
])
export type BlueprintChatProgramSessionAdapter =
  typeof BlueprintChatProgramSessionAdapter.Type

export const BlueprintChatProgramSessionLane = S.Literals([
  'auto',
  'local',
  'cloud-gcp',
  'cloud-shc',
])
export type BlueprintChatProgramSessionLane =
  typeof BlueprintChatProgramSessionLane.Type

export const BlueprintChatProgramTurnInput = S.Struct({
  actorRef: S.String,
  allowedSurfaces: S.Array(BlueprintObjectiveSurface),
  backendCapabilityRefs: S.Array(S.String),
  backendKind: S.String,
  backendProfileId: S.String,
  contextPackRef: S.optional(S.String),
  costRef: S.String,
  deniedToolRefs: S.optional(S.Array(S.String)),
  inputSnapshotHash: S.String,
  maxToolCount: S.optional(S.Number),
  model: S.String,
  observedAt: S.optional(S.String),
  preferredFamily: S.optional(BlueprintProgramFamily),
  programRunRef: S.optional(S.String),
  programSignatureIds: S.optional(S.Array(S.String)),
  programTypeIds: S.optional(S.Array(S.String)),
  promptSummaryRef: S.String,
  replayIntentRef: S.optional(S.String),
  replaySlug: S.optional(S.String),
  replayTargetRef: S.optional(S.String),
  registryVersionRef: S.optional(S.String),
  riskCeiling: BlueprintProgramRiskClass,
  routeRef: S.String,
  runnerRef: S.optional(S.String),
  sessionAdapter: BlueprintChatProgramSessionAdapter,
  sessionLane: S.optional(BlueprintChatProgramSessionLane),
  sessionObjectiveRef: S.String,
  sourceAuthorityRefs: S.Array(S.String),
  supportedToolRefs: S.Array(S.String),
  threadRef: S.optional(S.String),
  timeoutSeconds: S.optional(S.Number),
  turnRef: S.String,
  workroomRef: S.optional(S.String),
})
export type BlueprintChatProgramTurnInput =
  typeof BlueprintChatProgramTurnInput.Type

export const BlueprintChatProgramToolPolicy = S.Literals([
  'allow',
  'approval_required',
  'deny',
])
export type BlueprintChatProgramToolPolicy =
  typeof BlueprintChatProgramToolPolicy.Type

export const BlueprintChatProgramToolDefinition = S.Struct({
  access: BlueprintToolAccess,
  approvalPolicyRef: S.String,
  allowedSurfaces: S.Array(BlueprintObjectiveSurface),
  contextPackRefs: S.Array(S.String),
  evidenceRequirementRefs: S.Array(S.String),
  outputSchemaRef: S.String,
  policy: BlueprintChatProgramToolPolicy,
  programSignatureId: S.String,
  programTypeId: S.String,
  receiptRequirementRefs: S.Array(S.String),
  replayModule: S.optional(BlueprintReplayModuleBinding),
  sourceAuthorityRefs: S.Array(S.String),
  tassadarModuleStep: S.optional(BlueprintTassadarModuleStepBinding),
  toolRef: S.String,
})
export type BlueprintChatProgramToolDefinition =
  typeof BlueprintChatProgramToolDefinition.Type

export const BlueprintChatProgramToolMenu = S.Struct({
  actionSubmissionRequiredForDirectEffects: S.Literal(true),
  backendKind: S.String,
  deniedTools: S.Array(BlueprintChatProgramToolDefinition),
  evidenceRequirementRefs: S.Array(S.String),
  lookupId: S.String,
  menuId: S.String,
  moduleVersionIds: S.Array(S.String),
  policyRef: S.String,
  programSignatureIds: S.Array(S.String),
  programTypeIds: S.Array(S.String),
  receiptRequirementRefs: S.Array(S.String),
  registryVersionRef: S.String,
  safeProjection: S.Boolean,
  tools: S.Array(BlueprintChatProgramToolDefinition),
  warnings: S.Array(S.String),
})
export type BlueprintChatProgramToolMenu =
  typeof BlueprintChatProgramToolMenu.Type

export const BlueprintChatProgramSignatureSelection = S.Struct({
  actionSubmissionRequiredForDirectEffects: S.Literal(true),
  backendCapabilityRefs: S.Array(S.String),
  candidateEntryIds: S.Array(S.String),
  contextPackRef: S.optional(S.String),
  directMutationAllowed: S.Boolean,
  evidenceRequirementRefs: S.Array(S.String),
  lookupId: S.String,
  moduleVersionIds: S.Array(S.String),
  policyRef: S.String,
  programSignatureIds: S.Array(S.String),
  programTypeIds: S.Array(S.String),
  purposeRef: S.String,
  receiptRequirementRefs: S.Array(S.String),
  registryVersionRef: S.String,
  releaseGateRefs: S.Array(S.String),
  requiresContextPackRef: S.Boolean,
  safeProjection: S.Boolean,
  toolScopes: S.Array(BlueprintProgramToolScope),
})
export type BlueprintChatProgramSignatureSelection =
  typeof BlueprintChatProgramSignatureSelection.Type

export const BlueprintChatProgramSessionEvent = S.Struct({
  eventRef: S.String,
  evidenceRefs: S.Array(S.String),
  observedAt: S.String,
  phase: S.String,
  receiptRefs: S.Array(S.String),
  safeProjection: S.Literal(true),
  state: S.String,
  summaryRef: S.String,
  toolRef: S.optional(S.String),
})
export type BlueprintChatProgramSessionEvent =
  typeof BlueprintChatProgramSessionEvent.Type

export const BlueprintChatProgramUsage = S.Struct({
  completionTokens: S.optional(S.Number),
  promptTokens: S.optional(S.Number),
  totalTokens: S.optional(S.Number),
  truth: S.Literals(['exact', 'estimated', 'unknown']),
})
export type BlueprintChatProgramUsage =
  typeof BlueprintChatProgramUsage.Type

export const BlueprintChatProgramSessionResult = S.Struct({
  confidence: S.Number,
  evidenceRefs: S.Array(S.String),
  events: S.Array(BlueprintChatProgramSessionEvent),
  latencyMs: S.Number,
  receiptRefs: S.Array(S.String),
  renderedResponseRef: S.String,
  requestedDirectEffects: S.Array(BlueprintProgramRunDirectEffectKind),
  responseRef: S.String,
  responseSummaryRef: S.String,
  sessionRef: S.String,
  toolCallbackRefs: S.Array(S.String),
  typedOutput: S.Record(S.String, S.Unknown),
  usage: BlueprintChatProgramUsage,
})
export type BlueprintChatProgramSessionResult =
  typeof BlueprintChatProgramSessionResult.Type

export const BlueprintChatProgramSessionSpawnInput = S.Struct({
  adapter: BlueprintChatProgramSessionAdapter,
  lane: S.optional(BlueprintChatProgramSessionLane),
  menu: BlueprintChatProgramToolMenu,
  objectiveRef: S.String,
  selection: BlueprintChatProgramSignatureSelection,
  timeoutSeconds: S.optional(S.Number),
  turn: BlueprintChatProgramTurnInput,
  verifyRefs: S.Array(S.String),
})
export type BlueprintChatProgramSessionSpawnInput =
  typeof BlueprintChatProgramSessionSpawnInput.Type

export const BlueprintChatProgramResponse = S.Struct({
  contentRedacted: S.Literal(true),
  renderedResponseRef: S.String,
  responseRef: S.String,
  responseSummaryRef: S.String,
  sessionRef: S.String,
})
export type BlueprintChatProgramResponse =
  typeof BlueprintChatProgramResponse.Type

export const BlueprintChatProgramTurnResult = S.Struct({
  actionSubmissionRequiredForDirectEffects: S.Literal(true),
  directMutationDisabled: S.Literal(true),
  events: S.Array(BlueprintChatProgramSessionEvent),
  kind: S.Literal('blueprint_chat_program_turn_result'),
  lookup: BlueprintChatProgramSignatureSelection,
  noDeploy: S.Literal(true),
  noEmail: S.Literal(true),
  noSourceMutation: S.Literal(true),
  noSpend: S.Literal(true),
  programRun: BlueprintProgramRunRecord,
  response: BlueprintChatProgramResponse,
  replayModuleEvidence: S.Array(BlueprintReplayModuleEvidence),
  safeProjection: S.Literal(true),
  sessionSubstrateRef: S.Literal('substrate.autopilot.session_spawn.node_state_poll'),
  tassadarModuleStepEvidence: S.Array(BlueprintTassadarModuleStepEvidence),
  toolMenu: BlueprintChatProgramToolMenu,
  turnRef: S.String,
})
export type BlueprintChatProgramTurnResult =
  typeof BlueprintChatProgramTurnResult.Type

export const BlueprintChatProgramRuntimeStage = S.Literals([
  'registry',
  'signature_selection',
  'tool_menu',
  'session',
  'replay_module',
  'tassadar_step',
  'unsafe_projection',
])
export type BlueprintChatProgramRuntimeStage =
  typeof BlueprintChatProgramRuntimeStage.Type

export class BlueprintChatProgramRuntimeError extends S.TaggedErrorClass<BlueprintChatProgramRuntimeError>()(
  'BlueprintChatProgramRuntimeError',
  {
    reason: S.String,
    stage: BlueprintChatProgramRuntimeStage,
    turnRef: S.String,
  },
) {}

export type BlueprintChatProgramTurnError =
  | BlueprintChatProgramRuntimeError
  | BlueprintProgramRunDirectEffectDenied

export type BlueprintChatProgramRuntimePrimitives = Readonly<{
  makeLookupId: () => string
  makeMenuId: () => string
  makeProgramRunId: () => string
  nowIso: () => string
}>

export const systemBlueprintChatProgramRuntimePrimitives: BlueprintChatProgramRuntimePrimitives =
  {
    makeLookupId: () => compactRandomId('blueprint_signature_lookup'),
    makeMenuId: () => compactRandomId('blueprint_tool_menu'),
    makeProgramRunId: () => compactRandomId('blueprint_program_run'),
    nowIso: currentIsoTimestamp,
  }

export type BlueprintChatProgramSessionRuntime = Readonly<{
  spawnSession: (
    input: BlueprintChatProgramSessionSpawnInput,
  ) => Effect.Effect<BlueprintChatProgramSessionResult, BlueprintChatProgramRuntimeError>
}>

export type ExecuteBlueprintChatProgramTurnInput = Readonly<{
  replayRuntime?: BlueprintReplayModuleRuntime | undefined
  registryProjection?: BlueprintProgramRegistryProjectionType | undefined
  registryVersionRef?: string | undefined
  runtime?: BlueprintChatProgramRuntimePrimitives | undefined
  sessionRuntime: BlueprintChatProgramSessionRuntime
  turn: BlueprintChatProgramTurnInput
}>

const RISK_ORDER: Record<BlueprintProgramRiskClassType, number> = {
  high: 3,
  legal_sensitive: 4,
  low: 1,
  medium: 2,
  payment_sensitive: 4,
}

const SELECTABLE_STATUSES: ReadonlySet<BlueprintProgramStatusType> =
  new Set(['draft', 'active'])

const TOOL_OUTPUT_SCHEMA_REFS: Readonly<Record<string, string>> = {
  'tool.action_submission.propose':
    'schema.blueprint.BlueprintActionSubmission.v1',
  'tool.context_pack.read': 'schema.blueprint.BlueprintContextPack.v1',
  'tool.proof_replay.bundle.show':
    'schema.blueprint.BlueprintReplayModuleEvidence.v1',
  'tool.tassadar.module.execute':
    'schema.blueprint.BlueprintTassadarModuleStepEvidence.v1',
}

const PROHIBITED_RESULT_TEXT_PATTERN =
  /\b(access_token|authorization|bearer|callback_url|callback_token|client_secret|customer_email|customer_name|email_body|id_token|invoice|mnemonic|oauth|payment_hash|payment_id|payment_preimage|payout_address|preimage|private_key|private_repo|provider_grant|provider_payload|provider_token|raw_email|raw_payload|raw_prompt|raw_run_log|raw_source_archive|raw_trace|raw_webhook|refresh_token|runner_log|secret|source_archive|token|wallet|xprv)\b/i

const uniqueStrings = (
  values: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> => [
  ...new Set(values.filter((value): value is string => value !== undefined)),
]

export const executeBlueprintChatProgramTurn = (
  input: ExecuteBlueprintChatProgramTurnInput,
): Effect.Effect<BlueprintChatProgramTurnResult, BlueprintChatProgramTurnError> =>
  Effect.gen(function* () {
    const runtime = input.runtime ?? systemBlueprintChatProgramRuntimePrimitives
    const registry =
      input.registryProjection ?? AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY
    const registryVersionRef =
      input.registryVersionRef ??
      input.turn.registryVersionRef ??
      AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF
    const observedAt = input.turn.observedAt ?? runtime.nowIso()

    if (!blueprintProgramRegistryProjectionIsSafe(registry)) {
      return yield* runtimeError(
        input.turn.turnRef,
        'registry',
        'Blueprint chat runtime requires an operator-safe Blueprint registry projection.',
      )
    }

    const lookup = yield* selectBlueprintChatProgramSignature({
      lookupId: runtime.makeLookupId(),
      registry,
      registryVersionRef,
      turn: input.turn,
    })
    const toolMenu = yield* buildBlueprintChatProgramToolMenu({
      lookup,
      menuId: runtime.makeMenuId(),
      turn: input.turn,
    })
    const tassadarModuleStepEvidence =
      yield* executeTassadarModuleStepsForMenu(input.turn, observedAt, toolMenu)
    const replayModuleEvidence = yield* executeReplayModulesForMenu(
      input.turn,
      observedAt,
      toolMenu,
      input.replayRuntime,
    )
    const session = yield* input.sessionRuntime.spawnSession({
      adapter: input.turn.sessionAdapter,
      menu: toolMenu,
      objectiveRef: input.turn.sessionObjectiveRef,
      selection: lookup,
      turn: input.turn,
      verifyRefs: uniqueStrings([
        'receipt.program_run',
        ...replayModuleEvidence.flatMap(evidence => evidence.receiptRefs),
      ]),
      ...(input.turn.sessionLane === undefined
        ? {}
        : { lane: input.turn.sessionLane }),
      ...(input.turn.timeoutSeconds === undefined
        ? {}
        : { timeoutSeconds: input.turn.timeoutSeconds }),
    })
    const programRun = buildBlueprintChatProgramRunRecord({
      lookup,
      observedAt,
      programRunId: input.turn.programRunRef ?? runtime.makeProgramRunId(),
      replayModuleEvidence,
      session,
      tassadarModuleStepEvidence,
      toolMenu,
      turn: input.turn,
    })

    yield* assertProgramRunEvidenceOnly(programRun)

    const directEffect = session.requestedDirectEffects[0]
    if (directEffect !== undefined) {
      return yield* denyProgramRunDirectEffect(programRun, directEffect)
    }

    const result: BlueprintChatProgramTurnResult = {
      actionSubmissionRequiredForDirectEffects: true,
      directMutationDisabled: true,
      events: session.events,
      kind: 'blueprint_chat_program_turn_result',
      lookup,
      noDeploy: true,
      noEmail: true,
      noSourceMutation: true,
      noSpend: true,
      programRun,
      replayModuleEvidence,
      response: {
        contentRedacted: true,
        renderedResponseRef: session.renderedResponseRef,
        responseRef: session.responseRef,
        responseSummaryRef: session.responseSummaryRef,
        sessionRef: session.sessionRef,
      },
      safeProjection: true,
      sessionSubstrateRef:
        'substrate.autopilot.session_spawn.node_state_poll',
      tassadarModuleStepEvidence,
      toolMenu,
      turnRef: input.turn.turnRef,
    }

    if (!blueprintChatProgramTurnResultIsSafe(result)) {
      return yield* runtimeError(
        input.turn.turnRef,
        'unsafe_projection',
        'Blueprint chat turn results must expose refs, digests, receipts, and redacted response handles only.',
      )
    }

    return result
  })

export const selectBlueprintChatProgramSignature = (
  input: Readonly<{
    lookupId: string
    registry: BlueprintProgramRegistryProjectionType
    registryVersionRef: string
    turn: BlueprintChatProgramTurnInput
  }>,
): Effect.Effect<BlueprintChatProgramSignatureSelection, BlueprintChatProgramRuntimeError> =>
  Effect.gen(function* () {
    const entries = selectCandidateEntries(input.turn, input.registry)

    if (entries.length === 0) {
      return yield* runtimeError(
        input.turn.turnRef,
        'signature_selection',
        'No Blueprint Program Registry entry matched the typed chat turn constraints.',
      )
    }

    const programTypes = selectedProgramTypes(input.registry, entries)
    const signatures = selectedProgramSignatures(input.registry, entries)
    const moduleVersions = selectedModuleVersions(
      input.registry,
      entries,
      signatures,
      programTypes,
    )

    if (programTypes.length === 0 || signatures.length === 0) {
      return yield* runtimeError(
        input.turn.turnRef,
        'signature_selection',
        'Selected Blueprint registry entries do not include complete Program Type and Signature refs.',
      )
    }

    const toolScopes = selectedToolScopes(
      signatures,
      input.turn.allowedSurfaces,
      input.turn.supportedToolRefs,
      input.turn.maxToolCount,
    )

    if (toolScopes.length === 0) {
      return yield* runtimeError(
        input.turn.turnRef,
        'signature_selection',
        'Selected Blueprint signatures produced no tool scopes supported by this chat runtime.',
      )
    }

    const releaseGateRefs = selectedReleaseGateRefs(input.registry, entries)
    const requiresContextPackRef = signatures.some(
      signature => signature.supportsContext,
    )

    if (
      requiresContextPackRef &&
      input.turn.contextPackRef === undefined
    ) {
      return yield* runtimeError(
        input.turn.turnRef,
        'signature_selection',
        'Selected Blueprint signature requires a contextPackRef.',
      )
    }

    return {
      actionSubmissionRequiredForDirectEffects: true,
      backendCapabilityRefs: [...input.turn.backendCapabilityRefs],
      candidateEntryIds: entries.map(entry => entry.id),
      directMutationAllowed:
        entries.every(entry => entry.directMutationAllowed) &&
        programTypes.every(programType => programType.directMutationAllowed),
      evidenceRequirementRefs: uniqueStrings([
        ...entries.flatMap(entry => entry.evidenceRefs),
        ...programTypes.flatMap(programType =>
          programType.evidenceRequirements.map(
            requirement => requirement.descriptionRef,
          ),
        ),
        ...signatures.flatMap(signature =>
          signature.evidenceRequirements.map(
            requirement => requirement.descriptionRef,
          ),
        ),
      ]),
      lookupId: input.lookupId,
      moduleVersionIds: moduleVersions.map(moduleVersion => moduleVersion.id),
      policyRef: input.registry.policyRef,
      programSignatureIds: signatures.map(signature => signature.id),
      programTypeIds: programTypes.map(programType => programType.id),
      purposeRef: programTypes[0]!.purposeRef,
      receiptRequirementRefs: uniqueStrings([
        ...entries.flatMap(entry => entry.receiptRefs),
        ...programTypes.flatMap(programType =>
          programType.receiptRequirements.map(
            requirement => requirement.receiptRef,
          ),
        ),
        ...signatures.flatMap(signature =>
          signature.receiptRequirements.map(
            requirement => requirement.receiptRef,
          ),
        ),
      ]),
      registryVersionRef: input.registryVersionRef,
      releaseGateRefs,
      requiresContextPackRef,
      safeProjection: input.registry.safeProjection,
      toolScopes,
      ...(input.turn.contextPackRef === undefined
        ? {}
        : { contextPackRef: input.turn.contextPackRef }),
    }
  })

export const buildBlueprintChatProgramToolMenu = (
  input: Readonly<{
    lookup: BlueprintChatProgramSignatureSelection
    menuId: string
    turn: BlueprintChatProgramTurnInput
  }>,
): Effect.Effect<BlueprintChatProgramToolMenu, BlueprintChatProgramRuntimeError> =>
  Effect.gen(function* () {
    const warnings: Array<string> = []
    const tools: Array<BlueprintChatProgramToolDefinition> = []
    const deniedTools: Array<BlueprintChatProgramToolDefinition> = []
    const contextPackRefs = uniqueStrings([input.turn.contextPackRef])

    for (const scope of input.lookup.toolScopes) {
      const outputSchemaRef = TOOL_OUTPUT_SCHEMA_REFS[scope.toolRef]

      if (outputSchemaRef === undefined) {
        warnings.push(`unsupported_tool_scope:${scope.toolRef}`)
        continue
      }

      const policy = toolPolicy(scope, input.turn.deniedToolRefs ?? [])
      const definition = toolDefinitionFromScope({
        contextPackRefs,
        lookup: input.lookup,
        outputSchemaRef,
        policy,
        scope,
        sourceAuthorityRefs: input.turn.sourceAuthorityRefs,
      })

      if (policy === 'deny') {
        deniedTools.push(definition)
        warnings.push(`denied_tool_scope:${scope.toolRef}`)
        continue
      }

      tools.push(definition)
    }

    if (tools.length === 0 && deniedTools.length === 0) {
      return yield* runtimeError(
        input.turn.turnRef,
        'tool_menu',
        'Blueprint chat tool menu produced no supported tool definitions.',
      )
    }

    return {
      actionSubmissionRequiredForDirectEffects: true,
      backendKind: input.turn.backendKind,
      deniedTools,
      evidenceRequirementRefs: input.lookup.evidenceRequirementRefs,
      lookupId: input.lookup.lookupId,
      menuId: input.menuId,
      moduleVersionIds: input.lookup.moduleVersionIds,
      policyRef: input.lookup.policyRef,
      programSignatureIds: input.lookup.programSignatureIds,
      programTypeIds: input.lookup.programTypeIds,
      receiptRequirementRefs: input.lookup.receiptRequirementRefs,
      registryVersionRef: input.lookup.registryVersionRef,
      safeProjection: input.lookup.safeProjection,
      tools,
      warnings,
    }
  })

export const blueprintChatProgramTurnResultIsSafe = (
  value: unknown,
): boolean => {
  if (value === null || value === undefined) {
    return true
  }

  if (typeof value === 'string') {
    return !PROHIBITED_RESULT_TEXT_PATTERN.test(value)
  }

  if (Array.isArray(value)) {
    return value.every(blueprintChatProgramTurnResultIsSafe)
  }

  if (typeof value !== 'object') {
    return true
  }

  return Object.entries(value).every(
    ([key, child]) =>
      !PROHIBITED_RESULT_TEXT_PATTERN.test(key) &&
      blueprintChatProgramTurnResultIsSafe(child),
  )
}

const executeTassadarModuleStepsForMenu = (
  turn: BlueprintChatProgramTurnInput,
  observedAt: string,
  menu: BlueprintChatProgramToolMenu,
): Effect.Effect<
  ReadonlyArray<BlueprintTassadarModuleStepEvidenceType>,
  BlueprintChatProgramRuntimeError
> =>
  Effect.all(
    menu.tools
      .filter(tool => tool.tassadarModuleStep !== undefined)
      .map(tool =>
        executeBlueprintTassadarModuleStep(
          {
            access: tool.access,
            allowedSurfaces: tool.allowedSurfaces,
            requiresApproval: tool.policy === 'approval_required',
            tassadarModuleStep: tool.tassadarModuleStep,
            toolRef: tool.toolRef,
          },
          { observedAt },
        ).pipe(
          Effect.mapError(
            error =>
              new BlueprintChatProgramRuntimeError({
                reason: error.reason,
                stage: 'tassadar_step',
                turnRef: turn.turnRef,
              }),
          ),
        ),
      ),
  )

const executeReplayModulesForMenu = (
  turn: BlueprintChatProgramTurnInput,
  observedAt: string,
  menu: BlueprintChatProgramToolMenu,
  replayRuntime: BlueprintReplayModuleRuntime | undefined,
): Effect.Effect<
  ReadonlyArray<BlueprintReplayModuleEvidenceType>,
  BlueprintChatProgramRuntimeError
> => {
  const replayTools = menu.tools.filter(tool => tool.replayModule !== undefined)

  if (replayTools.length === 0) {
    return Effect.succeed([])
  }

  if (replayRuntime === undefined) {
    return runtimeError(
      turn.turnRef,
      'replay_module',
      'Selected Blueprint replay signature requires a proof replay runtime.',
    )
  }

  return Effect.all(
    replayTools.map(tool =>
      executeBlueprintReplayModule({
        binding: tool.replayModule!,
        intentRef: turn.replayIntentRef ?? turn.sessionObjectiveRef,
        observedAt,
        replaySlug: turn.replaySlug,
        runtime: replayRuntime,
        targetRef: turn.replayTargetRef,
        toolRef: tool.toolRef,
      }).pipe(
        Effect.mapError(
          error =>
            new BlueprintChatProgramRuntimeError({
              reason: error.reason,
              stage: 'replay_module',
              turnRef: turn.turnRef,
            }),
        ),
      ),
    ),
  )
}

const buildBlueprintChatProgramRunRecord = (
  input: Readonly<{
    lookup: BlueprintChatProgramSignatureSelection
    observedAt: string
    programRunId: string
    replayModuleEvidence: ReadonlyArray<BlueprintReplayModuleEvidenceType>
    session: BlueprintChatProgramSessionResult
    tassadarModuleStepEvidence: ReadonlyArray<BlueprintTassadarModuleStepEvidenceType>
    toolMenu: BlueprintChatProgramToolMenu
    turn: BlueprintChatProgramTurnInput
  }>,
): BlueprintProgramRunRecordType => {
  const tassadarEvidenceRefs = input.tassadarModuleStepEvidence.flatMap(
    evidence => evidence.evidenceRefs,
  )
  const tassadarReceiptRefs = input.tassadarModuleStepEvidence.flatMap(
    evidence => evidence.receiptRefs,
  )
  const replayEvidenceRefs = input.replayModuleEvidence.flatMap(
    evidence => evidence.evidenceRefs,
  )
  const replayReceiptRefs = input.replayModuleEvidence.flatMap(
    evidence => evidence.receiptRefs,
  )

  return {
    actorRef: input.turn.actorRef,
    archivedAt: null,
    authorityBoundary: 'evidence_only',
    confidence: input.session.confidence,
    costRef: input.turn.costRef,
    createdAt: input.observedAt,
    directMutationDisabled: true,
    evidenceRefs: uniqueStrings([
      input.turn.promptSummaryRef,
      input.turn.contextPackRef,
      ...input.session.evidenceRefs,
      ...input.session.events.flatMap(event => event.evidenceRefs),
      ...input.session.toolCallbackRefs,
      ...tassadarEvidenceRefs,
      ...replayEvidenceRefs,
    ]),
    id: input.programRunId,
    idempotencyKey: `blueprint_chat_program_turn:${input.turn.turnRef}`,
    inputSnapshotHash: input.turn.inputSnapshotHash,
    latencyMs: input.session.latencyMs,
    metadata: {
      actionSubmissionRequiredForDirectEffects: true,
      backendKind: input.turn.backendKind,
      backendProfileId: input.turn.backendProfileId,
      contentRedacted: true,
      kind: 'blueprint_chat_program_turn',
      lookupId: input.lookup.lookupId,
      menuId: input.toolMenu.menuId,
      model: input.turn.model,
      observedAt: input.observedAt,
      registryVersionRef: input.lookup.registryVersionRef,
      renderedResponseRef: input.session.renderedResponseRef,
      responseRef: input.session.responseRef,
      responseSummaryRef: input.session.responseSummaryRef,
      replayBundleRefs: input.replayModuleEvidence.map(
        evidence => evidence.bundleRef,
      ),
      replayModuleStepRefs: input.replayModuleEvidence.map(
        evidence => evidence.stepRef,
      ),
      replaySlugs: input.replayModuleEvidence.map(
        evidence => evidence.replaySlug,
      ),
      runnerRef: input.turn.runnerRef,
      sessionRef: input.session.sessionRef,
      sessionSubstrateRef:
        'substrate.autopilot.session_spawn.node_state_poll',
      sourceAuthorityRefs: input.turn.sourceAuthorityRefs,
      tassadarModuleStepRefs: input.tassadarModuleStepEvidence.map(
        evidence => evidence.stepRef,
      ),
      threadRef: input.turn.threadRef,
      toolCallbackRefs: input.session.toolCallbackRefs,
      toolRefs: input.toolMenu.tools.map(tool => tool.toolRef),
      turnRef: input.turn.turnRef,
      usage: input.session.usage,
      workroomRef: input.turn.workroomRef,
    },
    moduleVersionId:
      input.lookup.moduleVersionIds[0] ??
      'module_version.blueprint_chat_program.unresolved',
    noDeploy: true,
    noEmail: true,
    noSourceMutation: true,
    noSpend: true,
    programSignatureId: input.lookup.programSignatureIds[0]!,
    programTypeId: input.lookup.programTypeIds[0]!,
    purposeRef: input.lookup.purposeRef,
    receiptRefs: uniqueStrings([
      'receipt.program_run',
      ...input.session.receiptRefs,
      ...input.session.events.flatMap(event => event.receiptRefs),
      ...tassadarReceiptRefs,
      ...replayReceiptRefs,
    ]),
    routeRef: input.turn.routeRef,
    typedOutput: {
      actionSubmissionRequiredForDirectEffects: true,
      directEffectRequests: input.session.requestedDirectEffects,
      renderedResponseRef: input.session.renderedResponseRef,
      responseRef: input.session.responseRef,
      responseSummaryRef: input.session.responseSummaryRef,
      replayBundles: input.replayModuleEvidence.map(evidence => ({
        bundle: evidence.bundle,
        bundleRef: evidence.bundleRef,
        renderPlan: evidence.renderPlan,
        replaySlug: evidence.replaySlug,
        replayViewSpec: evidence.replayViewSpec,
      })),
      selectedProgramSignatureId: input.lookup.programSignatureIds[0],
      sessionRef: input.session.sessionRef,
      tassadarModuleStepVerdicts: input.tassadarModuleStepEvidence.map(
        evidence => ({
          moduleRef: evidence.moduleRef,
          stepRef: evidence.stepRef,
          verdict: evidence.verdict,
        }),
      ),
      toolRefs: input.toolMenu.tools.map(tool => tool.toolRef),
    },
    updatedAt: input.observedAt,
  }
}

const selectCandidateEntries = (
  turn: BlueprintChatProgramTurnInput,
  registry: BlueprintProgramRegistryProjectionType,
): ReadonlyArray<BlueprintProgramRegistryEntry> => {
  const exactSignatureIds = turn.programSignatureIds ?? []
  const exactTypeIds = turn.programTypeIds ?? []
  const hasExactRefs = exactSignatureIds.length > 0 || exactTypeIds.length > 0
  const entries = hasExactRefs
    ? registry.entries.filter(
        entry =>
          exactTypeIds.includes(entry.programTypeId) ||
          entry.programSignatureIds.some(signatureId =>
            exactSignatureIds.includes(signatureId),
          ),
      )
    : registry.entries.filter(entry =>
        structuredEntryMatches(turn, registry, entry),
      )

  return entries.filter(entry => entryIsSelectable(turn, registry, entry))
}

const structuredEntryMatches = (
  turn: BlueprintChatProgramTurnInput,
  registry: BlueprintProgramRegistryProjectionType,
  entry: BlueprintProgramRegistryEntry,
): boolean => {
  const family = turn.preferredFamily ?? 'continuation'
  const programType = registry.programTypes.find(
    candidate => candidate.id === entry.programTypeId,
  )

  return (
    entry.family === family &&
    programType !== undefined &&
    riskWithinCeiling(programType.riskClass, turn.riskCeiling)
  )
}

const entryIsSelectable = (
  turn: BlueprintChatProgramTurnInput,
  registry: BlueprintProgramRegistryProjectionType,
  entry: BlueprintProgramRegistryEntry,
): boolean => {
  if (!entry.safeProjection || entry.directMutationAllowed) {
    return false
  }

  if (!SELECTABLE_STATUSES.has(entry.status)) {
    return false
  }

  if (!riskWithinCeiling(entry.riskClass, turn.riskCeiling)) {
    return false
  }

  const programType = registry.programTypes.find(
    candidate => candidate.id === entry.programTypeId,
  )

  if (
    programType === undefined ||
    programType.directMutationAllowed ||
    !SELECTABLE_STATUSES.has(programType.status)
  ) {
    return false
  }

  const signatures = registry.programSignatures.filter(signature =>
    entry.programSignatureIds.includes(signature.id),
  )

  return signatures.some(
    signature =>
      SELECTABLE_STATUSES.has(signature.status) &&
      signature.toolScopes.some(scope =>
        toolScopeMatchesTurn(turn, scope),
      ),
  )
}

const selectedProgramTypes = (
  registry: BlueprintProgramRegistryProjectionType,
  entries: ReadonlyArray<BlueprintProgramRegistryEntry>,
): ReadonlyArray<BlueprintProgramType> => {
  const ids = new Set(entries.map(entry => entry.programTypeId))
  return registry.programTypes.filter(programType => ids.has(programType.id))
}

const selectedProgramSignatures = (
  registry: BlueprintProgramRegistryProjectionType,
  entries: ReadonlyArray<BlueprintProgramRegistryEntry>,
): ReadonlyArray<BlueprintProgramSignatureType> => {
  const ids = new Set(entries.flatMap(entry => entry.programSignatureIds))
  return registry.programSignatures.filter(signature => ids.has(signature.id))
}

const selectedModuleVersions = (
  registry: BlueprintProgramRegistryProjectionType,
  entries: ReadonlyArray<BlueprintProgramRegistryEntry>,
  signatures: ReadonlyArray<BlueprintProgramSignatureType>,
  programTypes: ReadonlyArray<BlueprintProgramType>,
): ReadonlyArray<BlueprintModuleVersion> => {
  const entryModuleIds = new Set(entries.flatMap(entry => entry.moduleVersionIds))
  const signatureIds = new Set(signatures.map(signature => signature.id))
  const programTypeIds = new Set(programTypes.map(programType => programType.id))

  return registry.moduleVersions.filter(
    moduleVersion =>
      entryModuleIds.has(moduleVersion.id) ||
      (moduleVersion.programSignatureId !== null &&
        signatureIds.has(moduleVersion.programSignatureId)) ||
      programTypeIds.has(moduleVersion.programTypeId),
  )
}

const selectedReleaseGateRefs = (
  registry: BlueprintProgramRegistryProjectionType,
  entries: ReadonlyArray<BlueprintProgramRegistryEntry>,
): ReadonlyArray<string> => {
  const ids = new Set(entries.flatMap(entry => entry.releaseGateIds))
  return registry.releaseGates
    .filter(gate => ids.has(gate.id))
    .map(gate => gate.id)
}

const selectedToolScopes = (
  signatures: ReadonlyArray<BlueprintProgramSignatureType>,
  allowedSurfaces: ReadonlyArray<BlueprintObjectiveSurfaceType>,
  supportedToolRefs: ReadonlyArray<string>,
  maxToolCount: number | undefined,
): ReadonlyArray<BlueprintProgramToolScopeType> => {
  const scopes = uniqueToolScopes(
    signatures
      .flatMap(signature => signature.toolScopes)
      .filter(
        scope =>
          scope.allowedSurfaces.some(surface =>
            allowedSurfaces.includes(surface),
          ) && supportedToolRefs.includes(scope.toolRef),
      ),
  )

  return maxToolCount === undefined ? scopes : scopes.slice(0, maxToolCount)
}

const toolScopeMatchesTurn = (
  turn: BlueprintChatProgramTurnInput,
  scope: BlueprintProgramToolScopeType,
): boolean =>
  scope.allowedSurfaces.some(surface => turn.allowedSurfaces.includes(surface)) &&
  turn.supportedToolRefs.includes(scope.toolRef)

const uniqueToolScopes = (
  scopes: ReadonlyArray<BlueprintProgramToolScopeType>,
): ReadonlyArray<BlueprintProgramToolScopeType> => {
  const seen = new Set<string>()
  const output: Array<BlueprintProgramToolScopeType> = []

  for (const scope of scopes) {
    const key = [
      scope.toolRef,
      scope.access,
      scope.requiresApproval,
      scope.replayModule?.stepRef ?? '',
      scope.tassadarModuleStep?.stepRef ?? '',
    ].join(':')
    if (seen.has(key)) {
      continue
    }

    seen.add(key)
    output.push(scope)
  }

  return output
}

const toolPolicy = (
  scope: BlueprintProgramToolScopeType,
  deniedToolRefs: ReadonlyArray<string>,
): BlueprintChatProgramToolPolicy => {
  if (deniedToolRefs.includes(scope.toolRef)) {
    return 'deny'
  }

  return scope.requiresApproval || scope.access === 'propose_action'
    ? 'approval_required'
    : 'allow'
}

const toolDefinitionFromScope = (
  input: Readonly<{
    contextPackRefs: ReadonlyArray<string>
    lookup: BlueprintChatProgramSignatureSelection
    outputSchemaRef: string
    policy: BlueprintChatProgramToolPolicy
    scope: BlueprintProgramToolScopeType
    sourceAuthorityRefs: ReadonlyArray<string>
  }>,
): BlueprintChatProgramToolDefinition => ({
  access: input.scope.access,
  allowedSurfaces: [...input.scope.allowedSurfaces],
  approvalPolicyRef: `policy.blueprint_chat.${input.scope.toolRef}.${input.policy}.v1`,
  contextPackRefs: [...input.contextPackRefs],
  evidenceRequirementRefs: input.lookup.evidenceRequirementRefs,
  outputSchemaRef: input.outputSchemaRef,
  policy: input.policy,
  programSignatureId:
    input.lookup.programSignatureIds[0] ?? 'program_signature.unknown',
  programTypeId: input.lookup.programTypeIds[0] ?? 'program_type.unknown',
  receiptRequirementRefs: input.lookup.receiptRequirementRefs,
  sourceAuthorityRefs: [...input.sourceAuthorityRefs],
  toolRef: input.scope.toolRef,
  ...(input.scope.tassadarModuleStep === undefined
    ? {}
    : { tassadarModuleStep: input.scope.tassadarModuleStep }),
  ...(input.scope.replayModule === undefined
    ? {}
    : { replayModule: input.scope.replayModule }),
})

const riskWithinCeiling = (
  risk: BlueprintProgramRiskClassType,
  ceiling: BlueprintProgramRiskClassType,
): boolean => RISK_ORDER[risk] <= RISK_ORDER[ceiling]

const runtimeError = (
  turnRef: string,
  stage: BlueprintChatProgramRuntimeStage,
  reason: string,
): Effect.Effect<never, BlueprintChatProgramRuntimeError> =>
  Effect.fail(new BlueprintChatProgramRuntimeError({ reason, stage, turnRef }))
