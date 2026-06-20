import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OmniMobileWorkroomProjection,
  exampleOmniMobileApprovalCard,
  exampleOmniMobileApprovalCards,
  exampleOmniMobileWorkroom,
} from './omni-mobile-workroom-approval-cards'
import {
  MOBILE_WORKROOM_APPROVAL_PROJECTION_BLOCKERS,
  MobileWorkroomApprovalProjectionEndpoint,
  handleMobileWorkroomApprovalProjectionApi,
  isMobileWorkroomApprovalProjectionEnabled,
  makeInMemoryMobileWorkroomApprovalProjectionStore,
} from './mobile-workroom-approval-projection-routes'

const nowIso = '2026-06-06T22:30:00.000Z'

const request = (suffix = '', init: RequestInit = {}) =>
  new Request(
    `https://openagents.com${MobileWorkroomApprovalProjectionEndpoint}${suffix}`,
    init,
  )

const exampleStore = () =>
  makeInMemoryMobileWorkroomApprovalProjectionStore(
    [exampleOmniMobileWorkroom()],
    exampleOmniMobileApprovalCards(),
  )

describe('mobile workroom approval projection flag', () => {
  test('defaults OFF and only arms on explicit truthy tokens', () => {
    expect(isMobileWorkroomApprovalProjectionEnabled(undefined)).toBe(false)
    expect(isMobileWorkroomApprovalProjectionEnabled('')).toBe(false)
    expect(isMobileWorkroomApprovalProjectionEnabled('false')).toBe(false)
    expect(isMobileWorkroomApprovalProjectionEnabled('0')).toBe(false)
    expect(isMobileWorkroomApprovalProjectionEnabled('off')).toBe(false)
    expect(isMobileWorkroomApprovalProjectionEnabled('true')).toBe(true)
    expect(isMobileWorkroomApprovalProjectionEnabled('1')).toBe(true)
    expect(isMobileWorkroomApprovalProjectionEnabled('ON')).toBe(true)
    expect(isMobileWorkroomApprovalProjectionEnabled(' yes ')).toBe(true)
  })
})

describe('mobile workroom approval projection route', () => {
  test('rejects non-GET with 405', async () => {
    const response = await Effect.runPromise(
      handleMobileWorkroomApprovalProjectionApi(
        request('', { method: 'POST' }),
        { enabled: true, nowIso: () => nowIso, store: exampleStore() },
      ),
    )
    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
  })

  test('INERT by default: disabled ignores an injected store and stays yellow', async () => {
    const response = await Effect.runPromise(
      handleMobileWorkroomApprovalProjectionApi(request(), {
        enabled: false,
        nowIso: () => nowIso,
        store: exampleStore(),
      }),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')

    const payload = (await response.json()) as Record<string, unknown>
    expect(payload.promiseId).toBe('mobile.voice_approval_companion.v1')
    expect(payload.promiseState).toBe('yellow')
    expect(payload.enabled).toBe(false)
    expect(payload.inert).toBe(true)
    expect(payload.projectionAvailable).toBe(true)
    expect(payload.workroomCount).toBe(0)
    expect(payload.pendingApprovalCount).toBe(0)
    expect(payload.approvalMutationAllowed).toBe(false)
    expect(payload.executionMutationAllowed).toBe(false)
    expect(payload.paymentMutationAllowed).toBe(false)
    expect(payload.blockerCleared).toBe(
      MOBILE_WORKROOM_APPROVAL_PROJECTION_BLOCKERS.cleared,
    )
    expect(payload.remainingBlockers).toEqual([
      'blocker.product_promises.voice_command_approval_receipts_missing',
      'blocker.product_promises.cross_device_workroom_sync_missing',
    ])
  })

  test('ARMED: projects read-only workroom approval cards by audience', async () => {
    const response = await Effect.runPromise(
      handleMobileWorkroomApprovalProjectionApi(request('?audience=public'), {
        enabled: true,
        nowIso: () => nowIso,
        store: exampleStore(),
      }),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')

    const payload = (await response.json()) as Record<string, unknown>
    expect(payload.promiseState).toBe('yellow')
    expect(payload.enabled).toBe(true)
    expect(payload.inert).toBe(false)
    expect(payload.audience).toBe('public')
    expect(payload.workroomCount).toBe(1)
    expect(payload.pendingApprovalCount).toBe(1)
    expect(payload.blockedApprovalCount).toBe(1)
    expect(payload.criticalApprovalCount).toBe(1)
    expect(payload.expiredApprovalCount).toBe(1)

    const workrooms = payload.workrooms as ReadonlyArray<unknown>
    expect(workrooms).toHaveLength(1)
    const projection = S.decodeUnknownSync(OmniMobileWorkroomProjection)(
      workrooms[0],
    )
    expect(projection).toMatchObject({
      approvalMutationAllowed: false,
      audience: 'public',
      executionMutationAllowed: false,
      providerMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      runnerLaunchAllowed: false,
      statusLabel: 'Waiting for review',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(JSON.stringify(payload)).not.toContain('2026-06-06T')
    expect(JSON.stringify(payload)).not.toMatch(
      /(approval|artifact|provider|receipt|server_authority|source|wallet)\.private/,
    )
  })

  test('rejects invalid audiences with a safe 400', async () => {
    const response = await Effect.runPromise(
      handleMobileWorkroomApprovalProjectionApi(request('?audience=owner'), {
        enabled: true,
        nowIso: () => nowIso,
        store: exampleStore(),
      }),
    )
    expect(response.status).toBe(400)
    const payload = (await response.json()) as Record<string, unknown>
    expect(JSON.stringify(payload)).toContain('audience')
  })

  test('rejects unsafe projected records with 422 and no raw ref leak', async () => {
    const response = await Effect.runPromise(
      handleMobileWorkroomApprovalProjectionApi(request('?audience=operator'), {
        enabled: true,
        nowIso: () => nowIso,
        store: makeInMemoryMobileWorkroomApprovalProjectionStore(
          [exampleOmniMobileWorkroom()],
          [
            exampleOmniMobileApprovalCard({
              receiptRefs: ['receipt.public.payment_hash_abcd'],
            }),
          ],
        ),
      }),
    )
    expect(response.status).toBe(422)
    const payload = (await response.json()) as Record<string, unknown>
    expect(payload.error).toBe('mobile_workroom_approval_projection_unsafe')
    expect(typeof payload.reason).toBe('string')
    expect(JSON.stringify(payload)).not.toContain('payment_hash_abcd')
  })
})
