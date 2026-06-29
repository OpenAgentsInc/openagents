import { Effect } from 'effect'

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

export const countActiveOrangeChecks = (
  db: D1Database,
): Effect.Effect<number | null> =>
  Effect.promise(async () => {
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

export const readActiveOrangeCheckByActorRef = (
  db: D1Database,
  actorRef: string,
): Effect.Effect<OrangeCheckEntitlement | null> =>
  Effect.promise(async () => {
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
