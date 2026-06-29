import {
  TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS,
  TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF,
} from '@openagentsinc/tassadar-executor/linked-dense-module'
import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeBlueprintRoutes } from './blueprint-routes'
import {
  BLUEPRINT_CONTRACT_EXPORT_SEED,
  BlueprintContractExportSeed,
} from './blueprint/exports/contract-export'
import {
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY,
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF,
} from './blueprint/fixtures/program-registry'
import type { RecordBlueprintActionSubmissionProposalInput } from './blueprint/repositories/action-submissions'
import {
  BlueprintProgramRunValidationError,
  type RecordBlueprintProgramRunInput,
} from './blueprint/repositories/program-runs'
import {
  BLUEPRINT_TASSADAR_MODULE_REGISTRY_VERSION_REF,
  BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE,
  BlueprintTassadarModuleRegistryProjection,
  type BlueprintTassadarModuleRegistryEntry,
} from './blueprint/repositories/tassadar-module-registry'
import type { BlueprintActionSubmission } from './blueprint/schemas/action-submission'
import { BlueprintProgramRegistryProjection } from './blueprint/schemas/program-registry'
import type { BlueprintProgramRunRecord } from './blueprint/schemas/program-run'

const env = {}

const routes = (options: {
  readonly authorized?: boolean
  readonly intakeAuthorized?: boolean
  readonly listActionSubmissions?: ReadonlyArray<BlueprintActionSubmission>
  readonly listProgramRuns?: ReadonlyArray<BlueprintProgramRunRecord>
  readonly recordActionSubmission?: (
    input: RecordBlueprintActionSubmissionProposalInput,
  ) => BlueprintActionSubmission
  readonly recordProgramRun?: (
    input: RecordBlueprintProgramRunInput,
  ) => BlueprintProgramRunRecord
  readonly registryProjection?: typeof BlueprintProgramRegistryProjection.Type
}) =>
  makeBlueprintRoutes<typeof env>({
    ...(options.listActionSubmissions === undefined
      ? {}
      : {
          listActionSubmissions: () =>
            Effect.succeed(options.listActionSubmissions ?? []),
        }),
    ...(options.listProgramRuns === undefined
      ? {}
      : {
          listProgramRuns: () => Effect.succeed(options.listProgramRuns ?? []),
        }),
    ...(options.recordActionSubmission === undefined
      ? {}
      : {
          recordActionSubmissionProposal: (_env, input) =>
            Effect.sync(() => options.recordActionSubmission!(input)),
        }),
    ...(options.recordProgramRun === undefined
      ? {}
      : {
          recordProgramRun: (_env, input) =>
            Effect.sync(() => options.recordProgramRun!(input)),
        }),
    ...(options.registryProjection === undefined
      ? {}
      : { registryProjection: options.registryProjection }),
    requireAdminApiToken: request =>
      Promise.resolve(
        options.authorized === true &&
          request.headers.get('authorization') === 'Bearer admin',
      ),
    requireActionSubmissionIntake: request =>
      Promise.resolve(
        options.intakeAuthorized === true &&
          request.headers.get('authorization') === 'Bearer runner',
      ),
    requireProgramRunEvidenceIntake: request =>
      Promise.resolve(
        options.intakeAuthorized === true &&
          request.headers.get('authorization') === 'Bearer runner',
      ),
  })

const continuationProgramType =
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY.programTypes.find(
    programType => programType.id === 'program_type.autopilot.continue',
  )!
const continuationProgramSignature =
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY.programSignatures.find(
    signature => signature.id === 'program_signature.autopilot.continue.v1',
  )!
const continuationModuleVersion =
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY.moduleVersions.find(
    moduleVersion =>
      moduleVersion.id === 'module_version.autopilot.continue.candidate_1',
  )!

const probeProgramRunEvidence = (
  overrides: Partial<Record<string, unknown>> = {},
) => ({
  actorRef: 'actor.probe.local',
  assignmentRef: 'agent_run_assignment.probe.1',
  authorityBoundary: 'evidence_only',
  backendKind: 'apple_foundation_models',
  backendProfileId: 'backend_profile.apple_fm.local',
  contentRedacted: true,
  costRef: 'cost.probe.local.1',
  directMutationDisabled: true,
  evidenceRefs: ['evidence.probe.context_pack.1'],
  inputSnapshotHash: 'sha256:0123456789abcdef',
  kind: 'probe_blueprint_program_run_evidence',
  latencyMs: 321,
  lookupId: 'blueprint_signature_lookup.autopilot.continue.1',
  menuId: 'blueprint_signature_menu.autopilot.continue.v1',
  model: 'apple.foundation-models.local',
  moduleVersionId: continuationModuleVersion.id,
  noDeploy: true,
  noEmail: true,
  noSourceMutation: true,
  noSpend: true,
  observedAt: '2026-06-07T17:00:00.000Z',
  orderRef: 'order.probe.test.1',
  programRunRef: 'blueprint_program_run.probe.continue.1',
  programSignatureId: continuationProgramSignature.id,
  programTypeId: continuationProgramType.id,
  promptSummaryRef: 'prompt_summary.probe.redacted.1',
  receiptRefs: ['receipt.program_run.probe.1'],
  registryVersionRef: AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF,
  routeRef: 'route.autopilot.continue',
  runnerRef: 'runner.probe.local',
  threadRef: 'thread.probe.test.1',
  toolCallbackRefs: ['tool_callback.probe.signature_lookup.1'],
  typedOutput: {
    action: 'continue',
    confidence: 0.74,
    reasonRef: 'reason.probe.needs_next_step',
  },
  usage: {
    completionTokens: 6,
    promptTokens: 12,
    totalTokens: 18,
    truth: 'estimated',
  },
  workroomRef: 'workroom.probe.test.1',
  ...overrides,
})

const programRunRecordFromInput = (
  input: RecordBlueprintProgramRunInput,
): BlueprintProgramRunRecord => ({
  actorRef: input.actorRef,
  archivedAt: null,
  authorityBoundary: 'evidence_only',
  confidence: input.confidence,
  costRef: input.costRef,
  createdAt: '2026-06-07T17:01:00.000Z',
  directMutationDisabled: true,
  evidenceRefs: [...(input.evidenceRefs ?? [])],
  id: input.id ?? 'blueprint_program_run.generated',
  idempotencyKey: input.idempotencyKey,
  inputSnapshotHash: input.inputSnapshotHash,
  latencyMs: input.latencyMs,
  metadata: input.metadata ?? {},
  moduleVersionId: input.moduleVersionId,
  noDeploy: true,
  noEmail: true,
  noSourceMutation: true,
  noSpend: true,
  programSignatureId: input.programSignatureId,
  programTypeId: input.programTypeId,
  purposeRef: input.purposeRef,
  receiptRefs: [...(input.receiptRefs ?? [])],
  routeRef: input.routeRef,
  typedOutput: input.typedOutput,
  updatedAt: '2026-06-07T17:01:00.000Z',
})

const probeActionSubmissionProposal = (
  overrides: Partial<Record<string, unknown>> = {},
) => ({
  actionSubmissionRef: 'action_submission.probe.pr_1',
  actorRef: 'actor.probe.local',
  approvalPolicyRef: 'policy.blueprint.action_submission.proposals_only.v1',
  approvalRequired: true,
  assignmentRef: 'agent_run_assignment.probe.1',
  contentRedacted: true,
  contextPackRefs: ['context_pack.probe.thread_1'],
  directExecution: false,
  directProgramRunExecutionAllowed: false,
  evidenceRefs: ['evidence.probe.pr_summary_1'],
  effectKind: 'create_pull_request',
  inputSnapshotHash: 'sha256:abcdef0123456789',
  kind: 'probe_blueprint_action_submission_proposal',
  modelConfidenceBypassDisabled: true,
  moduleVersionId: continuationModuleVersion.id,
  observedAt: '2026-06-07T17:02:00.000Z',
  programRunAuthorityBoundary: 'evidence_only',
  programRunRef: 'blueprint_program_run.probe.continue.1',
  programSignatureId: continuationProgramSignature.id,
  programTypeId: continuationProgramType.id,
  proposalOnly: true,
  receiptRefs: ['receipt.action_submission.probe.pr_1'],
  sourceAuthorityRefs: ['source_authority.repo.openagents.probe'],
  status: 'proposed',
  summaryRef: 'summary.probe.create_pull_request.1',
  toolRefs: ['tool.probe.propose_action_submission'],
  typedIntent: {
    diffRef: 'artifact.diff.redacted_1',
    targetRef: 'repo.openagents.probe',
    titleRef: 'intent.title.create_pull_request.1',
  },
  ...overrides,
})

const actionSubmissionFromInput = (
  input: RecordBlueprintActionSubmissionProposalInput,
): BlueprintActionSubmission => ({
  actionKind: input.actionKind,
  approvalPolicyRef: input.approvalPolicyRef,
  approvalReceiptRef: null,
  approvalState: 'pending',
  approvedByRef: null,
  contentRedacted: true,
  contextPackRefs: [...(input.contextPackRefs ?? [])],
  createdAt: '2026-06-07T17:03:00.000Z',
  directExecution: false,
  directProgramRunExecutionAllowed: false,
  dryRunReceiptRef: null,
  dryRunRequired: input.dryRunRequired ?? true,
  evidenceRefs: [...input.evidenceRefs],
  executionReceiptRef: null,
  failureRef: null,
  id: input.id ?? 'action_submission.generated',
  idempotencyKey: input.idempotencyKey,
  modelConfidenceBypassDisabled: true,
  programRunAuthorityBoundary: 'evidence_only',
  proposalOnly: true,
  proposedByProgramRunId: input.proposedByProgramRunId,
  proposedEffectRef: input.proposedEffectRef,
  receiptRefs: ['receipt.action_submission', ...(input.receiptRefs ?? [])],
  sourceAuthorityRefs: [...(input.sourceAuthorityRefs ?? [])],
  status: 'pending_approval',
  summaryRef: input.summaryRef,
  toolRefs: [...(input.toolRefs ?? [])],
  updatedAt: '2026-06-07T17:03:00.000Z',
})

describe('Blueprint routes', () => {
  test('requires operator authorization before returning registry refs', async () => {
    const response = await Effect.runPromise(
      routes({}).handleBlueprintProgramRegistryApi(
        new Request('https://openagents.com/api/blueprint/program-registry'),
        env,
      ),
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('returns the operator-safe Program Registry projection with version header', async () => {
    const response = await Effect.runPromise(
      routes({ authorized: true }).handleBlueprintProgramRegistryApi(
        new Request('https://openagents.com/api/blueprint/program-registry', {
          headers: { authorization: 'Bearer admin' },
        }),
        env,
      ),
    )
    const body = await response.json()

    expect(response.status).toBe(200)
    expect(response.headers.get('x-blueprint-registry-version-ref')).toBe(
      AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF,
    )
    expect(
      S.decodeUnknownSync(BlueprintProgramRegistryProjection)(body),
    ).toMatchObject({
      safeProjection: true,
      policyRef: 'policy.blueprint.operator_safe_registry_projection.v1',
    })
    expect(JSON.stringify(body)).not.toMatch(
      /access_token|callback_token|private_key|provider_payload|raw_prompt|sk-[a-z0-9]/i,
    )
  })

  test('rejects unsafe registry projections before exposing them', async () => {
    const response = await Effect.runPromise(
      routes({
        authorized: true,
        registryProjection: {
          ...AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY,
          safeProjection: false,
        },
      }).handleBlueprintProgramRegistryApi(
        new Request('https://openagents.com/api/blueprint/program-registry', {
          headers: { authorization: 'Bearer admin' },
        }),
        env,
      ),
    )

    expect(response.status).toBe(500)
    await expect(response.json()).resolves.toMatchObject({
      error: 'unsafe_blueprint_registry_projection',
    })
  })

  test('returns a decodable contract export for Probe and Pylon consumers', async () => {
    const response = await Effect.runPromise(
      routes({ authorized: true }).handleBlueprintContractExportApi(
        new Request('https://openagents.com/api/blueprint/contracts', {
          headers: { authorization: 'Bearer admin' },
        }),
        env,
      ),
    )
    const body = await response.json()

    expect(response.status).toBe(200)
    expect(S.decodeUnknownSync(BlueprintContractExportSeed)(body)).toEqual(
      BLUEPRINT_CONTRACT_EXPORT_SEED,
    )
    expect(
      (body as typeof BLUEPRINT_CONTRACT_EXPORT_SEED).openApi.map(
        entry => entry.path,
      ),
    ).toEqual([
      '/api/blueprint/program-registry',
      '/api/blueprint/contracts',
      '/api/blueprint/tassadar-modules',
      '/api/blueprint/program-runs',
      '/api/blueprint/action-submissions',
      '/api/blueprint/action-submissions',
      '/api/blueprint/contributions',
      '/api/blueprint/contributions',
    ])
    expect((body as typeof BLUEPRINT_CONTRACT_EXPORT_SEED).consumers).toEqual(
      expect.arrayContaining(['probe', 'pylon']),
    )
  })

  test('requires operator authorization before returning Tassadar module refs', async () => {
    const response = await Effect.runPromise(
      routes({}).handleBlueprintTassadarModuleRegistryApi(
        new Request('https://openagents.com/api/blueprint/tassadar-modules'),
        env,
      ),
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('lists the public-safe Tassadar module registry for operators', async () => {
    const response = await Effect.runPromise(
      routes({ authorized: true }).handleBlueprintTassadarModuleRegistryApi(
        new Request('https://openagents.com/api/blueprint/tassadar-modules', {
          headers: { authorization: 'Bearer admin' },
        }),
        env,
      ),
    )
    const body = await response.json()
    const projection = S.decodeUnknownSync(
      BlueprintTassadarModuleRegistryProjection,
    )(body)

    expect(response.status).toBe(200)
    expect(
      response.headers.get(
        'x-blueprint-tassadar-module-registry-version-ref',
      ),
    ).toBe(BLUEPRINT_TASSADAR_MODULE_REGISTRY_VERSION_REF)
    expect(projection.safeProjection).toBe(true)
    expect(projection.modules.map(module => module.moduleKind)).toEqual([
      'dense_weight_module',
      'linked_dense_module',
    ])
    expect(JSON.stringify(body)).not.toMatch(
      /access_token|callback_token|private_key|provider_payload|raw_prompt|sk-[a-z0-9]/i,
    )
  })

  test('resolves a Tassadar module ref with claim and trust checks', async () => {
    const url = new URL('https://openagents.com/api/blueprint/tassadar-modules')
    url.searchParams.set('moduleRef', TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF)
    url.searchParams.set(
      'requiredClaimClass',
      TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS,
    )
    url.searchParams.set('requiredModuleKind', 'linked_dense_module')
    url.searchParams.set(
      'requiredTrustPosture',
      BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE,
    )

    const response = await Effect.runPromise(
      routes({ authorized: true }).handleBlueprintTassadarModuleRegistryApi(
        new Request(url, {
          headers: { authorization: 'Bearer admin' },
        }),
        env,
      ),
    )
    const body = (await response.json()) as {
      module: BlueprintTassadarModuleRegistryEntry
      registryVersionRef: string
    }

    expect(response.status).toBe(200)
    expect(body.registryVersionRef).toBe(
      BLUEPRINT_TASSADAR_MODULE_REGISTRY_VERSION_REF,
    )
    expect(body.module).toMatchObject({
      claimClass: TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS,
      moduleKind: 'linked_dense_module',
      moduleRef: TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF,
      publicSafe: true,
      trustPosture: BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE,
    })
    expect(body.module.moduleDigest).toMatch(/^[0-9a-f]{64}$/)
  })

  test('returns typed Tassadar registry miss and refusal responses', async () => {
    const missingUrl = new URL(
      'https://openagents.com/api/blueprint/tassadar-modules',
    )
    missingUrl.searchParams.set('moduleRef', 'module.public.tassadar.unknown')
    const refusedUrl = new URL(
      'https://openagents.com/api/blueprint/tassadar-modules',
    )
    refusedUrl.searchParams.set(
      'moduleRef',
      TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF,
    )
    refusedUrl.searchParams.set('requiredModuleKind', 'dense_weight_module')

    const missingResponse = await Effect.runPromise(
      routes({ authorized: true }).handleBlueprintTassadarModuleRegistryApi(
        new Request(missingUrl, {
          headers: { authorization: 'Bearer admin' },
        }),
        env,
      ),
    )
    const refusedResponse = await Effect.runPromise(
      routes({ authorized: true }).handleBlueprintTassadarModuleRegistryApi(
        new Request(refusedUrl, {
          headers: { authorization: 'Bearer admin' },
        }),
        env,
      ),
    )

    expect(missingResponse.status).toBe(404)
    await expect(missingResponse.json()).resolves.toMatchObject({
      error: 'tassadar_module_not_found',
    })
    expect(refusedResponse.status).toBe(409)
    await expect(refusedResponse.json()).resolves.toMatchObject({
      error: 'tassadar_module_refused',
    })
  })

  test('requires runner or operator authorization before accepting Probe Program Run evidence', async () => {
    const response = await Effect.runPromise(
      routes({}).handleBlueprintProgramRunEvidenceApi(
        new Request('https://openagents.com/api/blueprint/program-runs', {
          body: JSON.stringify(probeProgramRunEvidence()),
          method: 'POST',
        }),
        env,
      ),
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('accepts Probe Program Run evidence and includes the safe run projection in the registry', async () => {
    const storedRuns: Array<BlueprintProgramRunRecord> = []
    let capturedInput: RecordBlueprintProgramRunInput | undefined
    const routeSet = routes({
      authorized: true,
      intakeAuthorized: true,
      listProgramRuns: storedRuns,
      recordProgramRun: input => {
        capturedInput = input
        const run = programRunRecordFromInput(input)
        storedRuns.push(run)

        return run
      },
    })
    const response = await Effect.runPromise(
      routeSet.handleBlueprintProgramRunEvidenceApi(
        new Request('https://openagents.com/api/blueprint/program-runs', {
          body: JSON.stringify(probeProgramRunEvidence()),
          headers: { authorization: 'Bearer runner' },
          method: 'POST',
        }),
        env,
      ),
    )
    const body = await response.json()

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      programRun: {
        actorRef: 'actor.probe.local',
        authorityBoundary: 'evidence_only',
        directMutationDisabled: true,
        id: 'blueprint_program_run.probe.continue.1',
        noDeploy: true,
        noEmail: true,
        noSourceMutation: true,
        noSpend: true,
        programTypeId: 'program_type.autopilot.continue',
        safeProjection: true,
      },
      receiptRefs: ['receipt.program_run.probe.1'],
      registryVersionRef: AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF,
    })
    expect(capturedInput).toMatchObject({
      confidence: 0.74,
      evidenceRefs: [
        'evidence.probe.context_pack.1',
        'prompt_summary.probe.redacted.1',
        'tool_callback.probe.signature_lookup.1',
      ],
      idempotencyKey:
        'probe_blueprint_program_run:blueprint_program_run.probe.continue.1',
      purposeRef: 'purpose.autopilot.continue',
    })
    expect(capturedInput?.metadata).toMatchObject({
      assignmentRef: 'agent_run_assignment.probe.1',
      backendKind: 'apple_foundation_models',
      usage: {
        totalTokens: 18,
        truth: 'estimated',
      },
      workroomRef: 'workroom.probe.test.1',
    })

    const registryResponse = await Effect.runPromise(
      routeSet.handleBlueprintProgramRegistryApi(
        new Request('https://openagents.com/api/blueprint/program-registry', {
          headers: { authorization: 'Bearer admin' },
        }),
        env,
      ),
    )
    const registryBody = await registryResponse.json()

    expect(registryResponse.status).toBe(200)
    expect(
      S.decodeUnknownSync(BlueprintProgramRegistryProjection)(registryBody)
        .runDetails,
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'blueprint_program_run.probe.continue.1',
          safeProjection: true,
        }),
      ]),
    )
    expect(JSON.stringify(registryBody)).not.toMatch(
      /callbackUrl|callback_token|provider_payload|raw_prompt|sk-[a-z0-9]/i,
    )
  })

  test('rejects Probe Program Run evidence with write authority or private material', async () => {
    const routeSet = routes({
      intakeAuthorized: true,
      recordProgramRun: input => {
        throw new BlueprintProgramRunValidationError({
          reason: `unexpected record call for ${input.id}`,
        })
      },
    })
    const writeAuthorityResponse = await Effect.runPromise(
      routeSet.handleBlueprintProgramRunEvidenceApi(
        new Request('https://openagents.com/api/blueprint/program-runs', {
          body: JSON.stringify(probeProgramRunEvidence({ noDeploy: false })),
          headers: { authorization: 'Bearer runner' },
          method: 'POST',
        }),
        env,
      ),
    )
    const privateMaterialResponse = await Effect.runPromise(
      routeSet.handleBlueprintProgramRunEvidenceApi(
        new Request('https://openagents.com/api/blueprint/program-runs', {
          body: JSON.stringify({
            ...probeProgramRunEvidence(),
            callbackUrl: 'https://runner.local/callback/token',
          }),
          headers: { authorization: 'Bearer runner' },
          method: 'POST',
        }),
        env,
      ),
    )

    expect(writeAuthorityResponse.status).toBe(400)
    await expect(writeAuthorityResponse.json()).resolves.toMatchObject({
      error: 'bad_program_run_evidence',
    })
    expect(privateMaterialResponse.status).toBe(400)
    await expect(privateMaterialResponse.json()).resolves.toMatchObject({
      error: 'bad_program_run_evidence',
    })
  })

  test('requires runner or operator authorization before accepting Action Submission proposals', async () => {
    const response = await Effect.runPromise(
      routes({}).handleBlueprintActionSubmissionsApi(
        new Request('https://openagents.com/api/blueprint/action-submissions', {
          body: JSON.stringify(probeActionSubmissionProposal()),
          method: 'POST',
        }),
        env,
      ),
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('accepts Probe Action Submission proposals as pending review records', async () => {
    const storedSubmissions: Array<BlueprintActionSubmission> = []
    let capturedInput: RecordBlueprintActionSubmissionProposalInput | undefined
    const routeSet = routes({
      authorized: true,
      intakeAuthorized: true,
      listActionSubmissions: storedSubmissions,
      recordActionSubmission: input => {
        capturedInput = input
        const submission = actionSubmissionFromInput(input)
        storedSubmissions.push(submission)

        return submission
      },
    })
    const response = await Effect.runPromise(
      routeSet.handleBlueprintActionSubmissionsApi(
        new Request('https://openagents.com/api/blueprint/action-submissions', {
          body: JSON.stringify(probeActionSubmissionProposal()),
          headers: { authorization: 'Bearer runner' },
          method: 'POST',
        }),
        env,
      ),
    )
    const body = await response.json()

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      actionSubmission: {
        actionKind: 'create_pull_request',
        approvalPolicyRef:
          'policy.blueprint.action_submission.proposals_only.v1',
        approvalState: 'pending',
        directExecution: false,
        directProgramRunExecutionAllowed: false,
        executionReceiptRef: null,
        id: 'action_submission.probe.pr_1',
        status: 'pending_approval',
      },
      receiptRefs: [
        'receipt.action_submission',
        'receipt.action_submission.probe.pr_1',
      ],
      reviewRequired: true,
    })
    expect(capturedInput).toMatchObject({
      actionKind: 'create_pull_request',
      evidenceRefs: [
        'evidence.probe.pr_summary_1',
        'summary.probe.create_pull_request.1',
        'context_pack.probe.thread_1',
        'source_authority.repo.openagents.probe',
      ],
      idempotencyKey:
        'probe_blueprint_action_submission:action_submission.probe.pr_1',
      proposedByProgramRunId: 'blueprint_program_run.probe.continue.1',
      summaryRef: 'summary.probe.create_pull_request.1',
    })

    const listResponse = await Effect.runPromise(
      routeSet.handleBlueprintActionSubmissionsApi(
        new Request('https://openagents.com/api/blueprint/action-submissions', {
          headers: { authorization: 'Bearer admin' },
          method: 'GET',
        }),
        env,
      ),
    )
    const listBody = await listResponse.json()

    expect(listResponse.status).toBe(200)
    expect(listBody).toMatchObject({
      actionSubmissions: [
        {
          id: 'action_submission.probe.pr_1',
          status: 'pending_approval',
        },
      ],
    })
    expect(JSON.stringify(listBody)).not.toMatch(
      /callbackUrl|callback_token|provider_payload|raw_email|sk-[a-z0-9]/i,
    )
  })

  test('rejects local effects, direct execution, and private material in Action Submission proposals', async () => {
    const routeSet = routes({
      intakeAuthorized: true,
      recordActionSubmission: input => actionSubmissionFromInput(input),
    })
    const localEffectResponse = await Effect.runPromise(
      routeSet.handleBlueprintActionSubmissionsApi(
        new Request('https://openagents.com/api/blueprint/action-submissions', {
          body: JSON.stringify(
            probeActionSubmissionProposal({
              effectKind: 'local_sandbox_read',
            }),
          ),
          headers: { authorization: 'Bearer runner' },
          method: 'POST',
        }),
        env,
      ),
    )
    const directExecutionResponse = await Effect.runPromise(
      routeSet.handleBlueprintActionSubmissionsApi(
        new Request('https://openagents.com/api/blueprint/action-submissions', {
          body: JSON.stringify(
            probeActionSubmissionProposal({
              directProgramRunExecutionAllowed: true,
            }),
          ),
          headers: { authorization: 'Bearer runner' },
          method: 'POST',
        }),
        env,
      ),
    )
    const privateMaterialResponse = await Effect.runPromise(
      routeSet.handleBlueprintActionSubmissionsApi(
        new Request('https://openagents.com/api/blueprint/action-submissions', {
          body: JSON.stringify({
            ...probeActionSubmissionProposal(),
            typedIntent: {
              rawEmail: 'raw email body',
            },
          }),
          headers: { authorization: 'Bearer runner' },
          method: 'POST',
        }),
        env,
      ),
    )

    expect(localEffectResponse.status).toBe(400)
    expect(directExecutionResponse.status).toBe(400)
    expect(privateMaterialResponse.status).toBe(400)
  })
})
