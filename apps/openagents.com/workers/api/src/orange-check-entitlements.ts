import { Effect } from 'effect'

import type {
  InferenceEntitlementsMirror,
  InferenceEntitlementsNonGateReads,
} from './inference-entitlements-store'

export type OrangeCheckEntitlement = Readonly<{
  actionRef: string | null
  actorRef: string
  agentUserId: string
  createdAt: string
  id: string
  paidAmountCents: number
  receiptRef: string
  state: 'active' | 'revoked'
  updatedAt: string
}>

export type OrangeCheckBadgeProjection = Readonly<{
  active: boolean
  authorityBoundary: string
  badgeRef: string | null
  meaning: string
}>

export const orangeCheckBadgeProjection = (
  entitlement: OrangeCheckEntitlement | null,
): OrangeCheckBadgeProjection => ({
  active: entitlement !== null && entitlement.state === 'active',
  authorityBoundary:
    'An orange check signals economic participation only. It is not identity verification, moderation immunity, settlement authority, or proof the account is safe.',
  badgeRef:
    entitlement !== null && entitlement.state === 'active'
      ? entitlement.receiptRef
      : null,
  meaning:
    'Orange checked accounts are owner-claimed and have a recent Bitcoin-backed OpenAgents participation receipt.',
})

type EntitlementRow = Readonly<{
  action_ref: string | null
  actor_ref: string
  agent_user_id: string
  created_at: string
  id: string
  paid_amount_cents: number
  receipt_ref: string
  state: string
  updated_at: string
}>

const entitlementFromRow = (row: EntitlementRow): OrangeCheckEntitlement => ({
  actionRef: row.action_ref,
  actorRef: row.actor_ref,
  agentUserId: row.agent_user_id,
  createdAt: row.created_at,
  id: row.id,
  paidAmountCents: row.paid_amount_cents,
  receiptRef: row.receipt_ref,
  state: row.state as OrangeCheckEntitlement['state'],
  updatedAt: row.updated_at,
})

export class OrangeCheckStorageError extends Error {}

// KS-8.9 decommission follow-up (#8336): the count is a public stat only
// ("orangeChecksSold" on `/api/forum/launch-status`) — it never gates a
// grant, spend, or admission decision, so it is safe to serve from
// Postgres for real. `nonGateReads`, when present, ALREADY implements its
// own d1/compare/postgres routing (see
// `makeRoutedEntitlementsNonGateReads`) with fail-soft D1 fallback built
// in; absent => byte-identical inline D1 behavior.
export const countActiveOrangeChecks = (
  db: D1Database,
  nonGateReads?: Pick<InferenceEntitlementsNonGateReads, 'activeOrangeCheckCount'> | undefined,
): Effect.Effect<number | null> =>
  Effect.promise(async () => {
    if (nonGateReads !== undefined) {
      return nonGateReads.activeOrangeCheckCount()
    }
    try {
      const row = await db
        .prepare(
          `SELECT COUNT(*) AS orange_count
           FROM orange_check_entitlements
           WHERE state = 'active'`,
        )
        .first<Record<string, unknown>>()

      return row === null ? null : Number(row.orange_count)
    } catch {
      return null
    }
  })

export const grantOrangeCheckEntitlement = (
  db: D1Database,
  input: Readonly<{
    actionRef: string
    actorRef: string
    agentUserId: string
    nowIso: string
    paidAmountCents: number
    receiptRef: string
  }>,
  // KS-8.9 (#8320): fire-safe Postgres dual-write mirror.
  mirror?: InferenceEntitlementsMirror | undefined,
): Effect.Effect<OrangeCheckEntitlement | null> =>
  Effect.promise(async () => {
    try {
      await db
        .prepare(
          `INSERT OR IGNORE INTO orange_check_entitlements (
            id, agent_user_id, actor_ref, state, receipt_ref, action_ref,
            paid_amount_cents, created_at, updated_at
          ) VALUES (?, ?, ?, 'active', ?, ?, ?, ?, ?)`,
        )
        .bind(
          `orange_check_${input.agentUserId}`,
          input.agentUserId,
          input.actorRef,
          input.receiptRef,
          input.actionRef,
          input.paidAmountCents,
          input.nowIso,
          input.nowIso,
        )
        .run()

      mirror?.([
        {
          kind: 'write',
          row: {
            action_ref: input.actionRef,
            actor_ref: input.actorRef,
            agent_user_id: input.agentUserId,
            created_at: input.nowIso,
            id: `orange_check_${input.agentUserId}`,
            paid_amount_cents: input.paidAmountCents,
            receipt_ref: input.receiptRef,
            state: 'active',
            updated_at: input.nowIso,
          },
          table: 'orange_check_entitlements',
        },
      ])

      const row = await db
        .prepare(
          `SELECT *
           FROM orange_check_entitlements
           WHERE actor_ref = ?
           LIMIT 1`,
        )
        .bind(input.actorRef)
        .first<EntitlementRow>()

      return row === null ? null : entitlementFromRow(row)
    } catch {
      return null
    }
  })

// KS-8.9 decommission follow-up (#8336): a public badge-display lookup by
// actor (agent profile pages, post author badges, the Nostr badge export) —
// never a grant/spend/admission decision, so it is safe to serve from
// Postgres for real. Same fail-soft-routed `nonGateReads` contract as
// `countActiveOrangeChecks` above.
export const readActiveOrangeCheckByActorRef = (
  db: D1Database,
  actorRef: string,
  nonGateReads?:
    | Pick<InferenceEntitlementsNonGateReads, 'activeOrangeCheckByActorRef'>
    | undefined,
): Effect.Effect<OrangeCheckEntitlement | null> =>
  Effect.promise(async () => {
    if (nonGateReads !== undefined) {
      const row = await nonGateReads.activeOrangeCheckByActorRef(actorRef)
      return row === null ? null : entitlementFromRow(row)
    }
    try {
      const row = await db
        .prepare(
          `SELECT *
           FROM orange_check_entitlements
           WHERE actor_ref = ?
             AND state = 'active'
           LIMIT 1`,
        )
        .bind(actorRef)
        .first<EntitlementRow>()

      return row === null ? null : entitlementFromRow(row)
    } catch {
      return null
    }
  })
