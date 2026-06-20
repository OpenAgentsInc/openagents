// Mobile workroom approval projection endpoint
// (promise mobile.voice_approval_companion.v1, yellow).
//
// INERT by default. The route is wired into the live Worker but reads from an
// injected store that the Worker leaves EMPTY unless the surface flag is
// explicitly armed (MOBILE_WORKROOM_APPROVAL_PROJECTION_ENABLED). When armed it
// projects existing workroom + approval-card records through the read-only
// mobile projection core. It never approves, executes, notifies, spends, mutates
// providers, launches runners, or upgrades public claims.

import { badRequest, jsonResponse } from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  OMNI_MOBILE_WORKROOM_READ_ONLY_AUTHORITY,
  type OmniMobileApprovalCardRecord,
  type OmniMobileWorkroomAudience,
  OmniMobileWorkroomApprovalUnsafe,
  type OmniMobileWorkroomCompactRecord,
  projectOmniMobileWorkroom,
} from './omni-mobile-workroom-approval-cards'

export const MobileWorkroomApprovalProjectionEndpoint =
  '/api/mobile/workroom-approval-projection'

export const MOBILE_WORKROOM_APPROVAL_PROJECTION_PROMISE_ID =
  'mobile.voice_approval_companion.v1'

export const MOBILE_WORKROOM_APPROVAL_PROJECTION_BLOCKERS = {
  cleared: 'blocker.product_promises.mobile_projection_missing',
  remaining: [
    'blocker.product_promises.voice_command_approval_receipts_missing',
    'blocker.product_promises.cross_device_workroom_sync_missing',
  ],
} as const

export type MobileWorkroomApprovalProjectionStore = Readonly<{
  listApprovalCards: () => ReadonlyArray<OmniMobileApprovalCardRecord>
  listWorkrooms: () => ReadonlyArray<OmniMobileWorkroomCompactRecord>
}>

export const emptyMobileWorkroomApprovalProjectionStore:
  MobileWorkroomApprovalProjectionStore = {
    listApprovalCards: () => [],
    listWorkrooms: () => [],
  }

export const makeInMemoryMobileWorkroomApprovalProjectionStore = (
  workrooms: ReadonlyArray<OmniMobileWorkroomCompactRecord>,
  approvalCards: ReadonlyArray<OmniMobileApprovalCardRecord>,
): MobileWorkroomApprovalProjectionStore => ({
  listApprovalCards: () => [...approvalCards],
  listWorkrooms: () => [...workrooms],
})

export const isMobileWorkroomApprovalProjectionEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

export type MobileWorkroomApprovalProjectionDeps = Readonly<{
  enabled: boolean
  nowIso: () => string
  store?: MobileWorkroomApprovalProjectionStore
}>

const mobileWorkroomAudienceValues: ReadonlyArray<OmniMobileWorkroomAudience> =
  ['public', 'agent', 'customer', 'team', 'operator']

const isMobileWorkroomAudience = (
  value: string,
): value is OmniMobileWorkroomAudience =>
  mobileWorkroomAudienceValues.includes(
    value as OmniMobileWorkroomAudience,
  )

const audienceFromRequest = (
  request: Request,
): OmniMobileWorkroomAudience | null => {
  const audience = new URL(request.url).searchParams.get('audience')

  if (audience === null) {
    return 'public'
  }

  return isMobileWorkroomAudience(audience) ? audience : null
}

const resolveStore = (
  deps: MobileWorkroomApprovalProjectionDeps,
): MobileWorkroomApprovalProjectionStore =>
  deps.enabled && deps.store !== undefined
    ? deps.store
    : emptyMobileWorkroomApprovalProjectionStore

const projectionPayload = (
  deps: MobileWorkroomApprovalProjectionDeps,
  audience: OmniMobileWorkroomAudience,
): Record<string, unknown> => {
  const store = resolveStore(deps)
  const nowIso = deps.nowIso()
  const approvalCards = store.listApprovalCards()
  const workrooms = store
    .listWorkrooms()
    .map(workroom =>
      projectOmniMobileWorkroom(workroom, approvalCards, audience, nowIso),
    )

  return {
    promiseId: MOBILE_WORKROOM_APPROVAL_PROJECTION_PROMISE_ID,
    promiseState: 'yellow' as const,
    enabled: deps.enabled,
    inert: !deps.enabled,
    projectionAvailable: true as const,
    audience,
    authority: OMNI_MOBILE_WORKROOM_READ_ONLY_AUTHORITY,
    approvalMutationAllowed: false as const,
    executionMutationAllowed: false as const,
    notificationMutationAllowed: false as const,
    paymentMutationAllowed: false as const,
    providerMutationAllowed: false as const,
    publicClaimUpgradeAllowed: false as const,
    runnerLaunchAllowed: false as const,
    blockerCleared: MOBILE_WORKROOM_APPROVAL_PROJECTION_BLOCKERS.cleared,
    remainingBlockers: MOBILE_WORKROOM_APPROVAL_PROJECTION_BLOCKERS.remaining,
    workroomCount: workrooms.length,
    pendingApprovalCount: workrooms.reduce(
      (count, workroom) => count + workroom.pendingApprovalCount,
      0,
    ),
    blockedApprovalCount: workrooms.reduce(
      (count, workroom) => count + workroom.blockedApprovalCount,
      0,
    ),
    criticalApprovalCount: workrooms.reduce(
      (count, workroom) => count + workroom.criticalApprovalCount,
      0,
    ),
    expiredApprovalCount: workrooms.reduce(
      (count, workroom) => count + workroom.expiredApprovalCount,
      0,
    ),
    workrooms,
    note:
      'Mobile workroom approval projection is read-only. It clears the ' +
      'mobile projection blocker only; voice-command approval receipts and ' +
      'cross-device workroom sync remain open, so the promise stays yellow.',
  }
}

/**
 * GET the mobile workroom approval projection. Read-only, no-store JSON.
 */
export const handleMobileWorkroomApprovalProjectionApi = (
  request: Request,
  deps: MobileWorkroomApprovalProjectionDeps,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const audience = audienceFromRequest(request)
  if (audience === null) {
    return Effect.succeed(
      badRequest(
        'Invalid mobile workroom projection request: audience must be one of ' +
          mobileWorkroomAudienceValues.join(', ') +
          '.',
      ),
    )
  }

  try {
    return Effect.succeed(noStoreJsonResponse(projectionPayload(deps, audience)))
  } catch (error) {
    if (error instanceof OmniMobileWorkroomApprovalUnsafe) {
      return Effect.succeed(
        jsonResponse(
          {
            error: 'mobile_workroom_approval_projection_unsafe',
            reason: error.reason,
          },
          { status: 422 },
        ),
      )
    }

    return Effect.succeed(
      jsonResponse(
        { error: 'mobile_workroom_approval_projection_failed' },
        { status: 500 },
      ),
    )
  }
}
