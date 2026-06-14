import { Effect, Schema as S } from 'effect'

import {
  type OmniWorkroomRecord,
  type OmniWorkroomVisibility,
} from './omni-workrooms'
import {
  buildOmniWorkroomSurfaceProjection,
} from './omni-workroom-surface-projections'
import { parseJsonRecord, parseJsonStringArray } from './json-boundary'
import { type TenantRef } from './tenant-custom-hostnames'
import { readActiveTeamMembershipRole } from './team-repository'
import {
  type OmniDataClassification,
  type OmniTrustTier,
} from './omni-data-classification'
import {
  type OmniWorkroomStatus,
} from './omni-workrooms'
import { type OmniAcceptedOutcomeWorkKind } from './omni-accepted-outcome-contracts'

// WS-I tenant client scoped workroom views (#4991)
//
// GOAL: produce a CUSTOMER-scoped projection of a workroom for a signed-in
// client browsing a branded tenant subdomain, enforcing client separation via
// the existing team / project / visibility model. This module is pure
// authorization + projection composition over already-existing pieces:
//
//   - `resolveTenantByHostname` (tenant-custom-hostnames.ts) yields the
//     `TenantRef` (teamId) for the branded host. We accept that resolved tenant
//     as an input; we do NOT re-resolve the host here.
//   - `readActiveTeamMembershipRole` (team-repository.ts) is the authorization
//     primitive: a client is authorized for a tenant only if they hold an
//     ACTIVE membership in that tenant's team.
//   - The workroom must BELONG to that tenant. The only tenant linkage on an
//     omni_workroom today is its `site_id`, and `site_projects.team_id` carries
//     the owning team. So a workroom is in-tenant iff its site's team_id equals
//     the resolved tenant's teamId. A workroom with no site (or a site with no
//     team) cannot be proven to belong to the branded tenant and is denied.
//   - We never widen visibility: only `customer`, `team`, or `public`
//     visibility workrooms are exposed to a client, and the projection is
//     always the `customer` surface (the narrowest customer-visible tier). A
//     `private` workroom is denied even to an authorized member, preserving the
//     visibility model. `private` fields therefore never leak.
//
// NO NEW MIGRATION: this module only reads existing tables
// (omni_workrooms, site_projects, team_memberships).

// The set of workroom visibility tiers a signed-in client may ever see. Note
// this is strictly narrower than what we then project: we still render only the
// `customer` surface, never the broader `team` surface, even for `team`
// visibility, because tenant clients are external customers, not team members
// of the OpenAgents-internal sense. This keeps the customer/team separation.
const CLIENT_VISIBLE_VISIBILITIES: ReadonlySet<OmniWorkroomVisibility> =
  new Set<OmniWorkroomVisibility>(['customer', 'team', 'public'])

export const TenantClientWorkroomViewDenialReason = S.Literals([
  'not_authorized_for_tenant',
  'workroom_not_found',
  'workroom_not_in_tenant',
  'workroom_not_client_visible',
])
export type TenantClientWorkroomViewDenialReason =
  typeof TenantClientWorkroomViewDenialReason.Type

export class TenantClientWorkroomViewDenied extends S.TaggedErrorClass<TenantClientWorkroomViewDenied>()(
  'TenantClientWorkroomViewDenied',
  { reason: TenantClientWorkroomViewDenialReason },
) {}

export class TenantClientWorkroomViewStorageError extends S.TaggedErrorClass<TenantClientWorkroomViewStorageError>()(
  'TenantClientWorkroomViewStorageError',
  { operation: S.String, reason: S.String },
) {}

export type TenantClientWorkroomViewError =
  | TenantClientWorkroomViewDenied
  | TenantClientWorkroomViewStorageError

export type TenantClientWorkroomViewInput = Readonly<{
  tenant: TenantRef
  clientUserId: string
  workroomId: string
}>

// The scoped result handed back to the route. We expose only the customer
// surface projection plus the public-safe ids the client used to ask for it.
export type TenantClientWorkroomView = Readonly<{
  surface: 'customer'
  teamId: string
  workroomId: string
  projection: ReturnType<typeof buildOmniWorkroomSurfaceProjection>
}>

type WorkroomTenantRow = Readonly<{
  accepted_outcome_contract_id: string | null
  archived_at: string | null
  artifact_refs_json: string
  assignment_id: string | null
  blocker_refs_json: string
  classification_caveat_ref: string
  created_at: string
  customer_intent_ref: string
  data_classification: OmniDataClassification
  email_refs_json: string
  id: string
  idempotency_key: string
  metadata_json: string
  public_receipt_ref: string
  receipt_refs_json: string
  site_id: string | null
  site_team_id: string | null
  software_order_id: string
  source_refs_json: string
  status: OmniWorkroomStatus
  task_packet_ref: string | null
  trust_tier: OmniTrustTier
  updated_at: string
  visibility: OmniWorkroomVisibility
  work_kind: OmniAcceptedOutcomeWorkKind
}>

const storageError = (
  operation: string,
  error: unknown,
): TenantClientWorkroomViewStorageError =>
  new TenantClientWorkroomViewStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const recordFromRow = (row: WorkroomTenantRow): OmniWorkroomRecord => ({
  acceptedOutcomeContractId: row.accepted_outcome_contract_id,
  archivedAt: row.archived_at,
  artifactRefs: parseJsonStringArray(row.artifact_refs_json),
  assignmentId: row.assignment_id,
  blockerRefs: parseJsonStringArray(row.blocker_refs_json),
  classificationCaveatRef: row.classification_caveat_ref,
  createdAt: row.created_at,
  customerIntentRef: row.customer_intent_ref,
  dataClassification: row.data_classification,
  emailRefs: parseJsonStringArray(row.email_refs_json),
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  publicReceiptRef: row.public_receipt_ref,
  receiptRefs: parseJsonStringArray(row.receipt_refs_json),
  siteId: row.site_id,
  softwareOrderId: row.software_order_id,
  sourceRefs: parseJsonStringArray(row.source_refs_json),
  status: row.status,
  taskPacketRef: row.task_packet_ref,
  trustTier: row.trust_tier,
  updatedAt: row.updated_at,
  visibility: row.visibility,
  workKind: row.work_kind,
})

// Read the workroom together with its site's owning team_id in a single query
// so the tenant-membership check is consistent. We deliberately LEFT JOIN the
// site so a workroom with no site (site_id NULL) still returns a row with a
// NULL site_team_id, which we then deny as "not in tenant".
const readWorkroomWithTenant = (
  db: D1Database,
  workroomId: string,
): Effect.Effect<
  WorkroomTenantRow | null,
  TenantClientWorkroomViewStorageError
> =>
  Effect.tryPromise({
    catch: error => storageError('tenantClientViews.readWorkroom', error),
    try: () =>
      db
        .prepare(
          `SELECT w.accepted_outcome_contract_id,
                  w.archived_at,
                  w.artifact_refs_json,
                  w.assignment_id,
                  w.blocker_refs_json,
                  w.classification_caveat_ref,
                  w.created_at,
                  w.customer_intent_ref,
                  w.data_classification,
                  w.email_refs_json,
                  w.id,
                  w.idempotency_key,
                  w.metadata_json,
                  w.public_receipt_ref,
                  w.receipt_refs_json,
                  w.site_id,
                  s.team_id AS site_team_id,
                  w.software_order_id,
                  w.source_refs_json,
                  w.status,
                  w.task_packet_ref,
                  w.trust_tier,
                  w.updated_at,
                  w.visibility,
                  w.work_kind
             FROM omni_workrooms w
             LEFT JOIN site_projects s
               ON s.id = w.site_id
              AND s.archived_at IS NULL
            WHERE w.id = ?
              AND w.archived_at IS NULL
            LIMIT 1`,
        )
        .bind(workroomId)
        .first<WorkroomTenantRow>(),
  })

const isClientAuthorizedForTenant = (
  db: D1Database,
  tenant: TenantRef,
  clientUserId: string,
): Effect.Effect<boolean, TenantClientWorkroomViewStorageError> =>
  Effect.tryPromise({
    catch: error => storageError('tenantClientViews.membership', error),
    try: () => readActiveTeamMembershipRole(db, tenant.teamId, clientUserId),
  }).pipe(Effect.map(role => role !== undefined))

// Pure composition: authorize the client for the tenant, confirm the workroom
// belongs to that tenant, enforce the visibility floor, then build the customer
// surface projection. Every failure is a typed denial; private material is
// never composed into the result.
export const tenantClientWorkroomView = (
  db: D1Database,
  input: TenantClientWorkroomViewInput,
): Effect.Effect<TenantClientWorkroomView, TenantClientWorkroomViewError> =>
  Effect.gen(function* () {
    const authorized = yield* isClientAuthorizedForTenant(
      db,
      input.tenant,
      input.clientUserId,
    )

    if (!authorized) {
      return yield* new TenantClientWorkroomViewDenied({
        reason: 'not_authorized_for_tenant',
      })
    }

    const row = yield* readWorkroomWithTenant(db, input.workroomId)

    if (row === null) {
      return yield* new TenantClientWorkroomViewDenied({
        reason: 'workroom_not_found',
      })
    }

    // The workroom must belong to the resolved tenant. The only proof of
    // belonging is its site's team_id. Cross-tenant or site-less workrooms are
    // denied so a client on tenant A can never read tenant B's workroom.
    if (row.site_team_id === null || row.site_team_id !== input.tenant.teamId) {
      return yield* new TenantClientWorkroomViewDenied({
        reason: 'workroom_not_in_tenant',
      })
    }

    if (!CLIENT_VISIBLE_VISIBILITIES.has(row.visibility)) {
      return yield* new TenantClientWorkroomViewDenied({
        reason: 'workroom_not_client_visible',
      })
    }

    const record = recordFromRow(row)
    const projection = buildOmniWorkroomSurfaceProjection({
      surface: 'customer',
      workroom: record,
    })

    return {
      surface: 'customer' as const,
      teamId: input.tenant.teamId,
      workroomId: record.id,
      projection,
    }
  })
