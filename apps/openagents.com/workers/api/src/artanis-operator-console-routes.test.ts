import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeOperatorArtanisConsoleRoutes } from './artanis-operator-console-routes'
import { readEffectiveArtanisPylonDispatchApproval } from './artanis-operator-dispatch-execution'
import { runArtanisScheduledTick } from './artanis-scheduled-runner'
import {
  ArtanisPersistenceTestStore,
  artanisPersistenceTestDb,
} from './test/artanis-persistence-fixture'

const nowIso = '2026-06-07T05:20:00.000Z'

const executionContext = {
  passThroughOnException: () => undefined,
  props: {},
  waitUntil: () => undefined,
} satisfies ExecutionContext

const seedStore = async () => {
  const store = new ArtanisPersistenceTestStore()
  const db = artanisPersistenceTestDb(store)

  await Effect.runPromise(
    runArtanisScheduledTick({
      db,
      enabled: true,
      nowIso,
      scheduleRef: 'cron.public.artanis.20260607T0520',
    }),
  )

  return { db, store }
}

const route = (options: {
  readonly adminToken?: boolean
  readonly browserEmail?: string | undefined
  readonly nowIso?: string | undefined
}) =>
  makeOperatorArtanisConsoleRoutes({
    appendRefreshedSessionCookies: response => response,
    currentEpochMillis: () => Date.parse(options.nowIso ?? nowIso),
    isOpenAgentsAdminEmail: email => email === 'chris@openagents.com',
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
  }).routeOperatorArtanisConsoleRequest

describe('Artanis operator console routes', () => {
  test('requires operator authority before exposing Artanis evidence', async () => {
    const { db } = await seedStore()
    const request = new Request(
      'https://openagents.com/api/operator/artanis/console',
    )

    const anonymous = await Effect.runPromise(
      route({})(request, { OPENAGENTS_DB: db }, executionContext)!,
    )
    const nonAdmin = await Effect.runPromise(
      route({ browserEmail: 'user@example.com' })(
        request,
        { OPENAGENTS_DB: db },
        executionContext,
      )!,
    )

    expect(anonymous.status).toBe(401)
    expect(nonAdmin.status).toBe(403)
  })

  test('projects persisted Artanis state for operator inspection only', async () => {
    const { db } = await seedStore()
    const request = new Request(
      'https://openagents.com/api/operator/artanis/console',
      { headers: { authorization: 'Bearer admin' } },
    )
    const response = await Effect.runPromise(
      route({ adminToken: true })(
        request,
        { OPENAGENTS_DB: db },
        executionContext,
      )!,
    )
    const body = await response.json() as Record<string, unknown>
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      agentId: 'agent_artanis',
      status: {
        healthState: 'stale',
        lastTickRef: 'tick.public.artanis.cron_public_artanis_20260607T0520',
        loopState: 'running',
        pendingApprovalCount: 1,
        runtimeState: 'running',
      },
      steering: {
        supportedApprovalActions: expect.arrayContaining([
          'approve_risky_action',
          'reject_risky_action',
        ]),
        supportedGoalActions: expect.arrayContaining([
          'create_goal',
          'pause_goal',
          'resume_goal',
          'cancel_goal',
          'reprioritize_goal',
        ]),
      },
    })
    expect(serialized).toContain('evidence.private.artanis')
    expect(serialized).toContain('workroom.private.artanis')
    expect(serialized).toContain('receipt.operator.artanis')
    expect(serialized).not.toMatch(/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/)
    expect(serialized).not.toMatch(/\/Users\/|auth\.json|bearer [A-Za-z0-9._-]+|sk-[a-z0-9]/i)
  })

  test('allows admin bearer CLI access without a browser session', async () => {
    const { db } = await seedStore()
    const response = await Effect.runPromise(
      route({ adminToken: true })(
        new Request('https://openagents.com/api/operator/artanis/console', {
          headers: { authorization: 'Bearer admin' },
        }),
        { OPENAGENTS_DB: db },
        executionContext,
      )!,
    )

    await expect(response.json()).resolves.toMatchObject({
      consoleRef: 'operator.artanis.console',
    })
  })

  test('records operator approval actions as evidence without public leakage', async () => {
    const { db } = await seedStore()
    const gateRef =
      'gate.public.artanis.tassadar_executor_paid_sample.cron_public_artanis_20260607T0520'
    const response = await Effect.runPromise(
      route({
        adminToken: true,
        nowIso: '2026-06-07T02:20:00.000Z',
      })(
        new Request(
          `https://openagents.com/api/operator/artanis/approval-gates/${encodeURIComponent(gateRef)}/approve`,
          {
            headers: { authorization: 'Bearer admin' },
            method: 'POST',
          },
        ),
        { OPENAGENTS_DB: db },
        executionContext,
      )!,
    )
    const body = await response.json() as Record<string, unknown>
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      approvalGates: {
        gates: [
          {
            gateRef: `${gateRef}.approved`,
            state: 'approved',
            operatorReceiptRefs: expect.arrayContaining([
              'receipt.operator.artanis.approve_gate_public_artanis_tassadar_executor_paid_sample_cron_public_artanis_20260607t0520',
            ]),
            authorityReceiptRefs: expect.arrayContaining([
              'authority.public.artanis.operator_approve.gate_public_artanis_tassadar_executor_paid_sample_cron_public_artanis_20260607t0520',
            ]),
          },
        ],
      },
    })
    expect(serialized).toContain(
      'caveat.public.operator_decision_not_execution_authority',
    )
    expect(serialized).not.toMatch(/\/Users\/|auth\.json|bearer [A-Za-z0-9._-]+|sk-[a-z0-9]/i)
  })

  test('arming requires admin authority', async () => {
    const { db } = await seedStore()
    const create = () =>
      new Request(
        'https://openagents.com/api/operator/artanis/approval-gates',
        { method: 'POST' },
      )

    const anonymous = await Effect.runPromise(
      route({})(create(), { OPENAGENTS_DB: db }, executionContext)!,
    )
    const nonAdmin = await Effect.runPromise(
      route({ browserEmail: 'user@example.com' })(
        create(),
        { OPENAGENTS_DB: db },
        executionContext,
      )!,
    )

    expect(anonymous.status).toBe(401)
    expect(nonAdmin.status).toBe(403)
  })

  test('arms an effective pylon_job_dispatch approval without public leakage', async () => {
    const { db } = await seedStore()
    const response = await Effect.runPromise(
      route({ adminToken: true })(
        new Request(
          'https://openagents.com/api/operator/artanis/approval-gates',
          {
            headers: { authorization: 'Bearer admin' },
            method: 'POST',
          },
        ),
        { OPENAGENTS_DB: db },
        executionContext,
      )!,
    )
    const body = (await response.json()) as Record<string, unknown>
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      armedGate: {
        kind: 'pylon_job_dispatch',
        state: 'approved',
      },
    })

    // The newly armed gate must flip the dispatch-execution approval read to
    // true: an approved, non-expired, operator-authority pylon_job_dispatch row.
    await expect(
      readEffectiveArtanisPylonDispatchApproval(db, nowIso),
    ).resolves.toBe(true)

    // This is the operator-audience console, so operator-only authority material
    // is intentionally visible to the admin (matching the approve route above);
    // its public-safety is enforced at the store layer (saveArtanisApprovalGate
    // projects with the public_artanis audience and throws on private leakage),
    // so a successful arm already proves the public projection is clean. Here we
    // only guard against raw timestamps and secrets reaching the response.
    expect(serialized).toContain(
      'receipt.operator_approval.arm_pylon_dispatch.20260627',
    )
    expect(serialized).not.toMatch(/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/)
    expect(serialized).not.toMatch(/\/Users\/|auth\.json|bearer [A-Za-z0-9._-]+|sk-[a-z0-9]/i)
  })

  test('console survives stale/undecodable persisted rows', async () => {
    const { db, store } = await seedStore()
    // Simulate a row written under an older record schema that no longer
    // decodes. It must be skipped, not crash the whole operator console.
    store.rows('artanis_approval_gates').push({
      active: 0,
      agent_id: 'agent_artanis',
      closed_at: null,
      closeout_json: null,
      content_hash: 'stale',
      created_at: '2026-06-08T00:00:00.000Z',
      id: 'approval_gate:gate.public.artanis.stale_legacy',
      idempotency_key: 'artanis-approval:stale-legacy:v1',
      parent_ref: null,
      public_projection_json: '{}',
      record_json: '{"legacyOnlyField":true}',
      record_ref: 'gate.public.artanis.stale_legacy',
      scope_ref: 'pylon_job_dispatch',
      source_kind: 'approval_gate',
      state: 'approved',
      updated_at: '2026-06-08T00:00:00.000Z',
    } as never)

    const response = await Effect.runPromise(
      route({ adminToken: true })(
        new Request('https://openagents.com/api/operator/artanis/console', {
          headers: { authorization: 'Bearer admin' },
        }),
        { OPENAGENTS_DB: db },
        executionContext,
      )!,
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      consoleRef: 'operator.artanis.console',
    })
  })

  test('rejects a non-POST arm request', async () => {
    const { db } = await seedStore()
    const response = await Effect.runPromise(
      route({ adminToken: true })(
        new Request(
          'https://openagents.com/api/operator/artanis/approval-gates',
          {
            headers: { authorization: 'Bearer admin' },
            method: 'GET',
          },
        ),
        { OPENAGENTS_DB: db },
        executionContext,
      )!,
    )

    expect(response.status).toBe(405)
  })
})
