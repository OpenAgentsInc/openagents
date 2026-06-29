// Omni client-delivery business-object projection endpoint
// (promise workrooms.omni_client_delivery_workrooms.v1, yellow; DE-9 / EPIC #5532).
//
// Disabled by default. The route is wired into the live Worker but reads from an
// injected store that the Worker leaves EMPTY unless the surface flag is
// explicitly armed (OMNI_CLIENT_DELIVERY_PROJECTION_ENABLED). When armed it
// projects the EXISTING source-authorized business-object delivery seam
// (buildOmniBusinessObjectDeliveryPlan) over the live client-delivery workroom
// surface: per-write approval-gated decisions plus the integration gate verdict
// and applied business-object projections when the owner-gated delivery config
// reaches enabled_ready. It never sends, settles, spends, mutates a connector,
// notifies, launches a runner, or upgrades a public claim.
//
// This clears ONLY the missing read-only delivery projection blocker. The
// live-integration / owner-sign-off / closeout-receipt blockers stay
// owner-gated, so workrooms.omni_client_delivery_workrooms.v1 stays yellow and
// no green flip is claimed here (the flip is owner-signed per
// proof.claim_upgrade_receipts.v1).

import { badRequest } from '@openagentsinc/sync-worker'
import { Schema as S } from 'effect'

import type { OmniProjectionAudience } from './omni-data-classification'
import {
  OmniBusinessObjectDeliveryPlan,
  buildOmniBusinessObjectDeliveryPlan,
} from './omni-workroom-business-object-delivery'
import type { OmniBusinessObjectDeliveryConfig } from './omni-workroom-business-object-delivery'
import type {
  OmniBusinessObjectWriteRecord,
  OmniSourceAuthorityBinding,
} from './omni-source-authorized-business-objects'
import type { OmniWorkroomRecord } from './omni-workrooms'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

export const OmniClientDeliveryProjectionEndpoint =
  '/api/public/omni/client-delivery-projection'

export const OMNI_CLIENT_DELIVERY_PROJECTION_PROMISE_ID =
  'workrooms.omni_client_delivery_workrooms.v1'

/**
 * Staleness contract for the projection: built fresh from the injected store on
 * every request, so it is `live_at_read` (maxStaleness 0).
 */
export const OmniClientDeliveryProjectionStaleness:
  PublicProjectionStalenessContract = liveAtReadStaleness([
    'omni_client_delivery_workroom_changed',
    'omni_business_object_write_proposed',
    'omni_source_authority_binding_changed',
  ])

export const OMNI_CLIENT_DELIVERY_PROJECTION_BLOCKERS = {
  cleared: 'blocker.product_promises.omni_client_delivery_projection_missing',
  remaining: [
    'blocker.business_object_delivery.integration_inert_disabled',
    'blocker.business_object_delivery.owner_sign_off_missing',
    'blocker.business_object_delivery.closeout_receipt_missing',
  ],
} as const

/**
 * A single live client-delivery workroom plus the source-authority bindings and
 * proposed business-object writes the delivery seam reasons over. The store
 * yields these; the projection runs the existing pure plan builder per entry.
 */
export type OmniClientDeliveryProjectionWorkroom = Readonly<{
  bindings: ReadonlyArray<OmniSourceAuthorityBinding>
  config?: OmniBusinessObjectDeliveryConfig | undefined
  workroom: OmniWorkroomRecord
  writes: ReadonlyArray<OmniBusinessObjectWriteRecord>
}>

export type OmniClientDeliveryProjectionStore = Readonly<{
  listWorkrooms: () => ReadonlyArray<OmniClientDeliveryProjectionWorkroom>
}>

export const emptyOmniClientDeliveryProjectionStore:
  OmniClientDeliveryProjectionStore = {
    listWorkrooms: () => [],
  }

export const makeInMemoryOmniClientDeliveryProjectionStore = (
  workrooms: ReadonlyArray<OmniClientDeliveryProjectionWorkroom>,
): OmniClientDeliveryProjectionStore => ({
  listWorkrooms: () => [...workrooms],
})

export const isOmniClientDeliveryProjectionEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

export type OmniClientDeliveryProjectionDeps = Readonly<{
  enabled: boolean
  nowIso: () => string
  store?: OmniClientDeliveryProjectionStore
}>

const projectionAudienceValues: ReadonlyArray<OmniProjectionAudience> = [
  'public',
  'customer',
  'agent',
  'team',
  'operator',
  'private',
]

const isProjectionAudience = (
  value: string,
): value is OmniProjectionAudience =>
  projectionAudienceValues.includes(value as OmniProjectionAudience)

const audienceFromRequest = (
  request: Request,
): OmniProjectionAudience | null => {
  const audience = new URL(request.url).searchParams.get('audience')

  if (audience === null) {
    return 'public'
  }

  return isProjectionAudience(audience) ? audience : null
}

const resolveStore = (
  deps: OmniClientDeliveryProjectionDeps,
): OmniClientDeliveryProjectionStore =>
  deps.enabled && deps.store !== undefined
    ? deps.store
    : emptyOmniClientDeliveryProjectionStore

const projectionPayload = (
  deps: OmniClientDeliveryProjectionDeps,
  audience: OmniProjectionAudience,
): Record<string, unknown> => {
  const store = resolveStore(deps)
  const nowIso = deps.nowIso()

  const plans = store.listWorkrooms().map(entry =>
    S.encodeSync(OmniBusinessObjectDeliveryPlan)(
      buildOmniBusinessObjectDeliveryPlan({
        audience,
        bindings: entry.bindings,
        config: entry.config,
        nowIso,
        workroom: entry.workroom,
        writes: entry.writes,
      }),
    ),
  )

  const proposedCount = plans.reduce(
    (count, plan) => count + plan.proposedCount,
    0,
  )
  const applyableCount = plans.reduce(
    (count, plan) => count + plan.applyableCount,
    0,
  )
  const effectsApplied = plans.some(plan => plan.effectsApplied)

  return {
    promiseId: OMNI_CLIENT_DELIVERY_PROJECTION_PROMISE_ID,
    promiseState: 'yellow' as const,
    enabled: deps.enabled,
    inert: !deps.enabled,
    projectionAvailable: true as const,
    // Public-projection staleness contract (epic #4751). Composed live from the
    // injected store at read, so maxStaleness is 0.
    generatedAt: nowIso,
    maxStalenessSeconds: OmniClientDeliveryProjectionStaleness.maxStalenessSeconds,
    staleness: OmniClientDeliveryProjectionStaleness,
    audience,
    // The delivery seam only applies owner-gated business-object projections.
    // It still cannot mutate connectors, send notifications, move money, run
    // workers, or upgrade public claims.
    effectsApplied,
    writeMutationAllowed: effectsApplied,
    connectorWriteAllowed: false as const,
    notificationMutationAllowed: false as const,
    paymentMutationAllowed: false as const,
    settlementAllowed: false as const,
    runnerLaunchAllowed: false as const,
    publicClaimUpgradeAllowed: false as const,
    blockerCleared: OMNI_CLIENT_DELIVERY_PROJECTION_BLOCKERS.cleared,
    remainingBlockers: OMNI_CLIENT_DELIVERY_PROJECTION_BLOCKERS.remaining,
    workroomCount: plans.length,
    proposedWriteCount: proposedCount,
    applyableWriteCount: applyableCount,
    plans,
    note:
      'Omni client-delivery business-object projection plans approval-gated ' +
      'writes via the existing delivery seam and applies only when the ' +
      'owner-gated config carries source refs, approval, owner sign-off, and ' +
      'closeout receipts. Connector writes, notifications, settlement, runner ' +
      'launch, and public-claim upgrades remain disabled.',
  }
}

/**
 * GET the omni client-delivery business-object projection. Read-only,
 * no-store JSON. GET only.
 */
export const handleOmniClientDeliveryProjectionApi = (
  request: Request,
  deps: OmniClientDeliveryProjectionDeps,
): Response => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const audience = audienceFromRequest(request)
  if (audience === null) {
    return badRequest(
      'Invalid omni client-delivery projection request: audience must be one ' +
        'of ' +
        projectionAudienceValues.join(', ') +
        '.',
    )
  }

  return noStoreJsonResponse(projectionPayload(deps, audience))
}
