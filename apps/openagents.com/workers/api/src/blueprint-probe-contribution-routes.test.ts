import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeBlueprintProbeContributionRoutes } from './blueprint-probe-contribution-routes'
import type {
  BlueprintProbeContributionRecord,
  RecordBlueprintProbeContributionInput,
} from './blueprint/repositories/probe-contributions'

const env = {}

const routes = (options: {
  readonly authorized?: boolean
  readonly contributions?: ReadonlyArray<BlueprintProbeContributionRecord>
  readonly intakeAuthorized?: boolean
  readonly recordContribution?: (
    input: RecordBlueprintProbeContributionInput,
  ) => BlueprintProbeContributionRecord
}) =>
  makeBlueprintProbeContributionRoutes<typeof env>({
    listContributions: () => Effect.succeed(options.contributions ?? []),
    recordContribution: (_env, input) =>
      Effect.sync(() => options.recordContribution!(input)),
    requireAdminApiToken: request =>
      Promise.resolve(
        options.authorized === true &&
          request.headers.get('authorization') === 'Bearer admin',
      ),
    requireContributionIntake: request =>
      Promise.resolve(
        options.intakeAuthorized === true &&
          request.headers.get('authorization') === 'Bearer runner',
      ),
  })

const contributionDraft = (
  overrides: Partial<Record<string, unknown>> = {},
) => ({
  authority: {
    canChangePublicClaims: false,
    canCreateSite: false,
    canDeploy: false,
    canDispatchRuntime: false,
    canExecute: false,
    canMutateRepository: false,
    canPostPublicly: false,
    canSendEmail: false,
    canSpend: false,
    deniedEffectRefs: [
      'effect.execute',
      'effect.dispatch_runtime',
      'effect.deploy',
      'effect.spend',
      'effect.send_email',
      'effect.mutate_repository',
      'effect.post_publicly',
      'effect.create_site',
      'effect.change_public_claims',
    ],
  },
  backendProjectionAdapterRefs: ['adapter.probe.apple_fm.blueprint_tools.v1'],
  capabilityFamily: 'program_signature',
  capabilitySummaryRef: 'capability.probe.tool_menu.package.summary.v1',
  contentRedacted: true,
  contextPackageRefs: ['context_package.probe.repo.readonly.v1'],
  contributionKind: 'signature_contribution',
  contributorRefs: ['contributor.openagents.probe'],
  dogfoodScopeRef: 'dogfood.probe.assignment_only.v1',
  fixtureRefs: ['fixture.probe.tool_menu.decode.v1'],
  id: 'probe_blueprint_contribution.tool_menu.v1',
  intendedProgramFamily: 'action_planning',
  noProductionRuntimeAuthority: true,
  outcomeTemplateRefs: [],
  paymentAttributionRefs: [
    'payment_attribution.probe.tool_menu.promoted_ref.v1',
  ],
  promotionRef: null,
  proposedModuleVersionRefs: ['module_version.probe.tool_menu.seed.v1'],
  proposedProgramSignatureRefs: [
    'program_signature.probe.tool_menu.project.v1',
  ],
  proposedProgramTypeRefs: ['program_type.probe.tool_menu.project'],
  rejectionRef: null,
  releaseGateRefs: ['release_gate.probe.tool_menu.seed.v1'],
  retainedFailureRefs: ['failure.probe.tool_menu.fixture_retained.v1'],
  reviewStatus: 'approved',
  riskClass: 'medium',
  selfPromotionAttempt: false,
  sourceRefs: ['source_ref.probe.contribution.audit.v1'],
  status: 'approved_for_release_gate',
  toolPackageRefs: ['tool_package.probe.readonly_repo_tools.v1'],
  uiBindingRefs: [],
  ...overrides,
})

const recordFromInput = (
  input: RecordBlueprintProbeContributionInput,
): BlueprintProbeContributionRecord => ({
  blockerRefs: [],
  candidateRuntimeAllowed: input.candidateRuntimeAllowed,
  contributionKind: input.contributionKind,
  createdAt: '2026-06-07T22:00:00.000Z',
  developerPackageContribution:
    input.developerPackageContribution === undefined
      ? null
      : (input.developerPackageContribution as unknown as Record<
          string,
          unknown
        >),
  fixtureRefs:
    input.signatureContribution?.requiredFixtureRefs ??
    input.developerPackageContribution?.requiredFixtureRefs ??
    [],
  id: input.id ?? 'probe_blueprint_contribution.generated',
  idempotencyKey: input.idempotencyKey,
  productionRuntimeAllowed: input.productionRuntimeAllowed,
  projection: {
    id: input.id ?? 'probe_blueprint_contribution.generated',
    nonAuthoritative: true,
  },
  releaseGateReady: true,
  releaseGateRefs:
    input.signatureContribution?.releaseGateRefs ??
    input.developerPackageContribution?.releaseGateRefs ??
    [],
  retainedFailureRefs: input.retainedFailureRefs,
  reviewStatus:
    input.signatureContribution?.reviewStatus ??
    input.developerPackageContribution?.reviewStatus ??
    'not_requested',
  signatureContribution:
    input.signatureContribution === undefined
      ? null
      : (input.signatureContribution as unknown as Record<string, unknown>),
  status:
    input.signatureContribution?.status ??
    input.developerPackageContribution?.status ??
    'draft',
  targetRefs:
    input.signatureContribution === undefined
      ? [
          ...(input.developerPackageContribution
            ?.backendProjectionAdapterRefs ?? []),
          ...(input.developerPackageContribution?.toolPackageRefs ?? []),
        ]
      : [
          input.signatureContribution.proposedProgramTypeRef,
          input.signatureContribution.proposedProgramSignatureRef,
          input.signatureContribution.proposedModuleVersionRef,
        ].filter((value): value is string => value !== null),
  updatedAt: '2026-06-07T22:00:00.000Z',
})

describe('Blueprint Probe contribution routes', () => {
  test('requires runner authorization for contribution intake', async () => {
    const response = await Effect.runPromise(
      routes({}).handleBlueprintProbeContributionsApi(
        new Request('https://openagents.com/api/blueprint/contributions', {
          body: JSON.stringify(contributionDraft()),
          method: 'POST',
        }),
        env,
      ),
    )

    expect(response.status).toBe(401)
  })

  test('accepts reviewed non-authoritative contribution drafts and lists them for operators', async () => {
    const contributions: Array<BlueprintProbeContributionRecord> = []
    const routeSet = routes({
      authorized: true,
      contributions,
      intakeAuthorized: true,
      recordContribution: input => {
        const record = recordFromInput(input)
        contributions.push(record)

        return record
      },
    })
    const response = await Effect.runPromise(
      routeSet.handleBlueprintProbeContributionsApi(
        new Request('https://openagents.com/api/blueprint/contributions', {
          body: JSON.stringify(contributionDraft()),
          headers: { authorization: 'Bearer runner' },
          method: 'POST',
        }),
        env,
      ),
    )
    const body = await response.json()

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      contribution: {
        candidateRuntimeAllowed: true,
        contributionKind: 'signature_contribution',
        id: 'probe_blueprint_contribution.tool_menu.v1',
        productionRuntimeAllowed: false,
      },
      releaseGateReady: true,
      runtime: {
        candidateRuntimeAllowed: true,
        productionRuntimeAllowed: false,
      },
    })

    const listResponse = await Effect.runPromise(
      routeSet.handleBlueprintProbeContributionsApi(
        new Request('https://openagents.com/api/blueprint/contributions', {
          headers: { authorization: 'Bearer admin' },
          method: 'GET',
        }),
        env,
      ),
    )
    const listBody = await listResponse.json()

    expect(listResponse.status).toBe(200)
    expect(listBody).toMatchObject({
      contributions: [
        {
          id: 'probe_blueprint_contribution.tool_menu.v1',
          productionRuntimeAllowed: false,
        },
      ],
    })
    expect(JSON.stringify(listBody)).not.toMatch(
      /provider_payload|raw_prompt|source_archive|runner_log|sk-[a-z0-9]/i,
    )
  })

  test('marks promoted Probe package contributions as production eligible', async () => {
    const response = await Effect.runPromise(
      routes({
        intakeAuthorized: true,
        recordContribution: recordFromInput,
      }).handleBlueprintProbeContributionsApi(
        new Request('https://openagents.com/api/blueprint/contributions', {
          body: JSON.stringify(
            contributionDraft({
              capabilityFamily: 'tool_package',
              contributionKind: 'developer_package_contribution',
              promotionRef: 'promotion.probe.tool_package.v1',
              status: 'promoted',
            }),
          ),
          headers: { authorization: 'Bearer runner' },
          method: 'POST',
        }),
        env,
      ),
    )
    const body = await response.json()

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      contribution: {
        contributionKind: 'developer_package_contribution',
        productionRuntimeAllowed: true,
      },
      runtime: {
        productionRuntimeAllowed: true,
      },
    })
  })

  test('rejects runtime authority, self-promotion, missing release-gate evidence, and private material', async () => {
    const routeSet = routes({
      intakeAuthorized: true,
      recordContribution: recordFromInput,
    })
    const authoritative = await Effect.runPromise(
      routeSet.handleBlueprintProbeContributionsApi(
        new Request('https://openagents.com/api/blueprint/contributions', {
          body: JSON.stringify(
            contributionDraft({
              authority: {
                ...(contributionDraft().authority as Record<string, unknown>),
                canDispatchRuntime: true,
              },
            }),
          ),
          headers: { authorization: 'Bearer runner' },
          method: 'POST',
        }),
        env,
      ),
    )
    const selfPromoting = await Effect.runPromise(
      routeSet.handleBlueprintProbeContributionsApi(
        new Request('https://openagents.com/api/blueprint/contributions', {
          body: JSON.stringify(
            contributionDraft({ selfPromotionAttempt: true }),
          ),
          headers: { authorization: 'Bearer runner' },
          method: 'POST',
        }),
        env,
      ),
    )
    const missingReleaseEvidence = await Effect.runPromise(
      routeSet.handleBlueprintProbeContributionsApi(
        new Request('https://openagents.com/api/blueprint/contributions', {
          body: JSON.stringify(
            contributionDraft({
              fixtureRefs: [],
              retainedFailureRefs: [],
            }),
          ),
          headers: { authorization: 'Bearer runner' },
          method: 'POST',
        }),
        env,
      ),
    )
    const privateMaterial = await Effect.runPromise(
      routeSet.handleBlueprintProbeContributionsApi(
        new Request('https://openagents.com/api/blueprint/contributions', {
          body: JSON.stringify(
            contributionDraft({
              sourceRefs: ['raw_prompt.do_not_publish'],
            }),
          ),
          headers: { authorization: 'Bearer runner' },
          method: 'POST',
        }),
        env,
      ),
    )

    expect(authoritative.status).toBe(400)
    expect(selfPromoting.status).toBe(400)
    expect(missingReleaseEvidence.status).toBe(400)
    expect(privateMaterial.status).toBe(400)
  })
})
