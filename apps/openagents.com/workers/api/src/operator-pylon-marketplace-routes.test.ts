import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeOperatorPylonMarketplaceRoutes } from './operator-pylon-marketplace-routes'
import {
  type PylonMarketplaceJobStore,
  type PylonMarketplaceStoredAssignment,
  type PylonMarketplaceStoredIntake,
  type PylonMarketplaceStoredTriageAction,
} from './pylon-marketplace-service'

const nowIso = '2026-06-07T06:30:00.000Z'

const executionContext = {
  passThroughOnException: () => undefined,
  props: {},
  waitUntil: () => undefined,
} satisfies ExecutionContext

class MemoryPylonMarketplaceStore implements PylonMarketplaceJobStore {
  readonly assignments: Array<PylonMarketplaceStoredAssignment> = []
  readonly intakes: Array<PylonMarketplaceStoredIntake> = []
  readonly triageActions: Array<PylonMarketplaceStoredTriageAction> = []

  insertAssignment = async (
    assignment: PylonMarketplaceStoredAssignment,
  ): Promise<void> => {
    this.assignments.push(assignment)
  }

  insertIntake = async (
    intake: PylonMarketplaceStoredIntake,
  ): Promise<void> => {
    this.intakes.push(intake)
  }

  insertTriageAction = async (
    action: PylonMarketplaceStoredTriageAction,
  ): Promise<void> => {
    this.triageActions.push(action)
  }

  listAssignments = async (
    limit: number,
  ): Promise<ReadonlyArray<PylonMarketplaceStoredAssignment>> =>
    [...this.assignments]
      .sort((left, right) => right.updatedAtIso.localeCompare(left.updatedAtIso))
      .slice(0, limit)

  listIntakes = async (
    limit: number,
  ): Promise<ReadonlyArray<PylonMarketplaceStoredIntake>> =>
    [...this.intakes]
      .sort((left, right) => right.updatedAtIso.localeCompare(left.updatedAtIso))
      .slice(0, limit)

  readIntakeByIdempotencyKey = async (
    idempotencyKey: string,
  ): Promise<PylonMarketplaceStoredIntake | null> =>
    this.intakes.find(intake => intake.idempotencyKey === idempotencyKey) ??
      null

  readIntakeByRef = async (
    intakeRef: string,
  ): Promise<PylonMarketplaceStoredIntake | null> =>
    this.intakes.find(intake => intake.intakeRef === intakeRef) ?? null

  readTriageActionByIdempotencyKey = async (
    idempotencyKey: string,
  ): Promise<PylonMarketplaceStoredTriageAction | null> =>
    this.triageActions.find(action =>
      action.idempotencyKey === idempotencyKey
    ) ?? null

  updateIntake = async (
    intake: PylonMarketplaceStoredIntake,
  ): Promise<void> => {
    const index = this.intakes.findIndex(row =>
      row.intakeRef === intake.intakeRef
    )

    if (index === -1) {
      this.intakes.push(intake)

      return
    }

    this.intakes[index] = intake
  }
}

const route = (
  store: MemoryPylonMarketplaceStore,
  options: Readonly<{
    adminToken?: boolean
    browserEmail?: string | undefined
    ids?: Array<string> | undefined
  }> = {},
) => {
  const ids = [...(options.ids ?? [])]

  return makeOperatorPylonMarketplaceRoutes({
    appendRefreshedSessionCookies: response => response,
    currentEpochMillis: () => Date.parse(nowIso),
    isOpenAgentsAdminEmail: email => email === 'chris@openagents.com',
    makeId: () => ids.shift() ?? 'fixed-id',
    makeStore: () => store,
    requireAdminApiToken: request =>
      Promise.resolve(
        options.adminToken === true &&
          request.headers.get('authorization') === 'Bearer admin',
      ),
    requireBrowserSession: () =>
      Promise.resolve(
        options.browserEmail === undefined
          ? undefined
          : {
              user: {
                email: options.browserEmail,
                userId: 'github:operator',
              },
            },
      ),
  }).routeOperatorPylonMarketplaceRequest
}

const runRoute = async (
  store: MemoryPylonMarketplaceStore,
  request: Request,
  options: Readonly<{
    adminToken?: boolean
    browserEmail?: string | undefined
    ids?: Array<string> | undefined
  }> = {},
): Promise<Response> => {
  const matched = route(store, options)(
    request,
    { OPENAGENTS_DB: {} as D1Database },
    executionContext,
  )

  if (matched === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(matched)
}

const jsonRequest = (
  path: string,
  idempotencyKey: string,
  body: unknown,
): Request =>
  new Request(`https://openagents.com${path}`, {
    body: JSON.stringify(body),
    headers: {
      authorization: 'Bearer admin',
      'content-type': 'application/json',
      'idempotency-key': idempotencyKey,
    },
    method: 'POST',
  })

describe('operator Pylon marketplace routes', () => {
  test('requires operator authority', async () => {
    const store = new MemoryPylonMarketplaceStore()
    const request = new Request(
      'https://openagents.com/api/operator/artanis/pylon-marketplace/jobs',
    )

    const anonymous = await runRoute(store, request)
    const nonAdmin = await runRoute(store, request, {
      browserEmail: 'user@example.com',
    })

    expect(anonymous.status).toBe(401)
    expect(nonAdmin.status).toBe(403)
  })

  test('creates idempotent external artifact-review intakes with public redaction', async () => {
    const store = new MemoryPylonMarketplaceStore()
    const body = {
      jobKind: 'artifact_review',
      policyGateRefs: ['policy.public.marketplace.external_agent_review'],
      requesterRef: 'requester.private.external_agent_redacted',
      resourceModePreference: 'balanced',
      source: 'external_agent',
    }
    const request = jsonRequest(
      '/api/operator/artanis/pylon-marketplace/jobs',
      'intake-create-1',
      body,
    )
    const first = await runRoute(store, request, {
      adminToken: true,
      ids: ['artifact-review-1'],
    })
    const replay = await runRoute(
      store,
      jsonRequest(
        '/api/operator/artanis/pylon-marketplace/jobs',
        'intake-create-1',
        body,
      ),
      { adminToken: true },
    )
    const conflict = await runRoute(
      store,
      jsonRequest(
        '/api/operator/artanis/pylon-marketplace/jobs',
        'intake-create-1',
        { ...body, jobKind: 'inference' },
      ),
      { adminToken: true },
    )
    const firstBody = await first.json() as Record<string, any>
    const replayBody = await replay.json() as Record<string, any>

    expect(first.status).toBe(201)
    expect(replay.status).toBe(200)
    expect(conflict.status).toBe(409)
    expect(firstBody).toMatchObject({
      authority: {
        buyerChargeMutationAllowed: false,
        paidAssignmentDispatchAllowed: false,
        payoutMutationAllowed: false,
        settlementMutationAllowed: false,
      },
      idempotent: false,
      liveDispatchAllowed: false,
      settlementMutationAllowed: false,
    })
    expect(firstBody.publicProjection.intakeRecords[0]).toMatchObject({
      jobKind: 'artifact_review',
      requesterRef: 'requester.redacted',
      state: 'policy_gated',
    })
    expect(replayBody.idempotent).toBe(true)
  })

  test('triages an intake into a proposed assignment without dispatch or payment authority', async () => {
    const store = new MemoryPylonMarketplaceStore()
    const create = await runRoute(
      store,
      jsonRequest(
        '/api/operator/artanis/pylon-marketplace/jobs',
        'intake-create-2',
        {
          intakeRef: 'intake.public.test.gepa',
          jobKind: 'gepa_dspy_optimization',
          jobRef: 'job.public.test.gepa',
          requesterRef: 'requester.public.openagents',
          resourceModePreference: 'overnight_full',
          source: 'openagents_seeded',
        },
      ),
      { adminToken: true, ids: ['gepa-1'] },
    )
    const triage = await runRoute(
      store,
      jsonRequest(
        '/api/operator/artanis/pylon-marketplace/jobs/intake.public.test.gepa/triage',
        'triage-gepa-1',
        {
          assignment: {
            acceptanceCriteriaRefs: [
              'acceptance.public.autopilot_benchmark_delta',
            ],
            assignmentAuthorityRefs: [
              'approval.public.artanis.pylon_assignment_required',
            ],
            providerEligibilityRefs: [
              'eligibility.public.provider.capability_snapshot_ok',
            ],
            providerRefs: ['provider.public.pylon_eligible_pool'],
          },
          outcome: 'proposed_assignment',
        },
      ),
      { adminToken: true, ids: ['gepa-assignment-1'] },
    )
    const replay = await runRoute(
      store,
      jsonRequest(
        '/api/operator/artanis/pylon-marketplace/jobs/intake.public.test.gepa/triage',
        'triage-gepa-1',
        {
          assignment: {
            acceptanceCriteriaRefs: [
              'acceptance.public.autopilot_benchmark_delta',
            ],
            assignmentAuthorityRefs: [
              'approval.public.artanis.pylon_assignment_required',
            ],
            providerEligibilityRefs: [
              'eligibility.public.provider.capability_snapshot_ok',
            ],
            providerRefs: ['provider.public.pylon_eligible_pool'],
          },
          outcome: 'proposed_assignment',
        },
      ),
      { adminToken: true },
    )
    const triageBody = await triage.json() as Record<string, any>
    const replayBody = await replay.json() as Record<string, any>

    expect(create.status).toBe(201)
    expect(triage.status).toBe(200)
    expect(replay.status).toBe(200)
    expect(triageBody.operatorProjection.intakeRecords[0]).toMatchObject({
      state: 'assignment_proposed',
    })
    expect(triageBody.operatorProjection.assignmentRecords[0]).toMatchObject({
      acceptedWorkClaimAllowed: false,
      paidAssignmentClaimAllowed: false,
      payoutState: 'planned',
      providerEligibilityRefs: [
        'eligibility.public.provider.capability_snapshot_ok',
      ],
      state: 'proposed',
    })
    expect(triageBody.liveDispatchAllowed).toBe(false)
    expect(triageBody.authority.paidAssignmentDispatchAllowed).toBe(false)
    expect(replayBody.idempotent).toBe(true)
    expect(store.assignments).toHaveLength(1)
  })

  test('rejects missing blockers for needs-input triage and unsafe raw refs', async () => {
    const store = new MemoryPylonMarketplaceStore()
    await runRoute(
      store,
      jsonRequest(
        '/api/operator/artanis/pylon-marketplace/jobs',
        'intake-create-3',
        {
          intakeRef: 'intake.public.test.validation',
          jobKind: 'validation',
          jobRef: 'job.public.test.validation',
          requesterRef: 'requester.public.openagents',
          resourceModePreference: 'background_20',
          source: 'openagents_seeded',
        },
      ),
      { adminToken: true },
    )

    const missingBlocker = await runRoute(
      store,
      jsonRequest(
        '/api/operator/artanis/pylon-marketplace/jobs/intake.public.test.validation/triage',
        'triage-validation-needs-input',
        { outcome: 'needs_input' },
      ),
      { adminToken: true },
    )
    const unsafe = await runRoute(
      store,
      jsonRequest(
        '/api/operator/artanis/pylon-marketplace/jobs',
        'intake-create-unsafe',
        {
          dataRefs: ['dataset.raw.customer_payload'],
          jobKind: 'training',
          requesterRef: 'requester.public.openagents',
          resourceModePreference: 'dedicated_full_blast',
          source: 'openagents_seeded',
        },
      ),
      { adminToken: true },
    )

    expect(missingBlocker.status).toBe(400)
    expect(unsafe.status).toBe(400)
  })
})
