import { describe, expect, test } from 'vitest'

import {
  consumePendingReferralForUser,
  linkPendingReferralToAgentClaim,
  linkPendingReferralToOrder,
} from './site-referral-attribution-consumption'

type AttributionRow = Readonly<{
  archived_at: string | null
  capture_path: 'human' | 'agent'
  claimed_user_id: string | null
  created_at: string
  expires_at: string
  first_verified_at: string | null
  id: string
  policy_state: 'pending' | 'claimed' | 'expired'
  public_invite_ref: string | null
  public_source_ref: string
  referral_invite_id: string | null
  referral_source_id: string
  target: 'home' | 'order' | 'agent_claim'
  updated_at: string
}>

type UserAttributionRow = Readonly<{
  archived_at: string | null
  referral_attribution_id: string
  user_id: string
}>

type OrderAttributionRow = Readonly<{
  archived_at: string | null
  referral_attribution_id: string
  software_order_id: string
  user_id: string
}>

type AgentAttributionRow = Readonly<{
  agent_user_id: string
  archived_at: string | null
  owner_user_id: string | null
  referral_attribution_id: string
}>

class Store {
  agentAttributions: Array<AgentAttributionRow> = []
  attributions: Array<AttributionRow> = []
  failBatchAfterStatements: number | null = null
  orderAttributions: Array<OrderAttributionRow> = []
  userAttributions: Array<UserAttributionRow> = []
}

class Statement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly store: Store,
    private readonly query: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM referral_attributions')) {
      const [id] = this.values

      return Promise.resolve(
        (this.store.attributions.find(row => row.id === id) as T | undefined) ??
          null,
      )
    }

    if (this.query.includes('FROM user_referral_attributions')) {
      const [userId] = this.values

      return Promise.resolve(
        (this.store.userAttributions.find(
          row => row.user_id === userId && row.archived_at === null,
        ) as T | undefined) ?? null,
      )
    }

    if (this.query.includes('FROM order_referral_attributions')) {
      const [orderId] = this.values

      return Promise.resolve(
        (this.store.orderAttributions.find(
          row => row.software_order_id === orderId && row.archived_at === null,
        ) as T | undefined) ?? null,
      )
    }

    if (this.query.includes('FROM agent_referral_attributions')) {
      const [agentUserId] = this.values

      return Promise.resolve(
        (this.store.agentAttributions.find(
          row => row.agent_user_id === agentUserId && row.archived_at === null,
        ) as T | undefined) ?? null,
      )
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO user_referral_attributions')) {
      const [userId, referralAttributionId] = this.values

      if (
        this.store.userAttributions.every(row => row.user_id !== userId) &&
        this.store.userAttributions.every(
          row => row.referral_attribution_id !== referralAttributionId,
        )
      ) {
        this.store.userAttributions.push({
          archived_at: null,
          referral_attribution_id: String(referralAttributionId),
          user_id: String(userId),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO order_referral_attributions')) {
      const [orderId, userId, referralAttributionId] = this.values

      if (
        this.store.orderAttributions.every(
          row => row.software_order_id !== orderId,
        )
      ) {
        this.store.orderAttributions.push({
          archived_at: null,
          referral_attribution_id: String(referralAttributionId),
          software_order_id: String(orderId),
          user_id: String(userId),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO agent_referral_attributions')) {
      const [agentUserId, ownerUserId, referralAttributionId] = this.values

      if (
        this.store.agentAttributions.every(
          row => row.agent_user_id !== agentUserId,
        )
      ) {
        this.store.agentAttributions.push({
          agent_user_id: String(agentUserId),
          archived_at: null,
          owner_user_id: ownerUserId === null ? null : String(ownerUserId),
          referral_attribution_id: String(referralAttributionId),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE referral_attributions')) {
      const [userId, firstVerifiedAt, updatedAt, attributionId] = this.values

      this.store.attributions = this.store.attributions.map(row =>
        row.id === attributionId && row.policy_state === 'pending'
          ? {
              ...row,
              claimed_user_id: row.claimed_user_id ?? String(userId),
              first_verified_at:
                row.first_verified_at ?? String(firstVerifiedAt),
              policy_state: 'claimed',
              updated_at: String(updatedAt),
            }
          : row,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.reject(new Error(`Unexpected all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[string[], ...T[]]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<T[]>
  raw<T = unknown[]>(): Promise<[string[], ...T[]] | T[]> {
    return Promise.reject(new Error(`Unexpected raw: ${this.query}`))
  }
}

const db = (store: Store): D1Database =>
  ({
    batch: async statements => {
      const snapshot = {
        agentAttributions: [...store.agentAttributions],
        attributions: [...store.attributions],
        orderAttributions: [...store.orderAttributions],
        userAttributions: [...store.userAttributions],
      }

      try {
        const results: Array<D1Result> = []

        for (const [index, statement] of statements.entries()) {
          if (store.failBatchAfterStatements === index) {
            throw new Error('simulated batch failure')
          }

          results.push(await statement.run())
        }

        return results
      } catch (error) {
        store.agentAttributions = snapshot.agentAttributions
        store.attributions = snapshot.attributions
        store.orderAttributions = snapshot.orderAttributions
        store.userAttributions = snapshot.userAttributions

        throw error
      }
    },
    dump: () => Promise.reject(new Error('D1 dump should not be used')),
    exec: () => Promise.reject(new Error('D1 exec should not be used')),
    prepare: query => new Statement(store, query),
    withSession: () => {
      throw new Error('D1 session should not be used')
    },
  }) as D1Database

const runtime = {
  nowIso: () => '2026-06-05T21:00:00.000Z',
}

const attribution = (
  overrides: Partial<AttributionRow> = {},
): AttributionRow => ({
  archived_at: null,
  capture_path: 'human',
  claimed_user_id: null,
  created_at: '2026-06-05T20:00:00.000Z',
  expires_at: '2026-07-05T20:00:00.000Z',
  first_verified_at: null,
  id: 'referral_attribution_otec',
  policy_state: 'pending',
  public_invite_ref: null,
  public_source_ref: 'site_ref_otec_ben',
  referral_invite_id: null,
  referral_source_id: 'site_referral_source_otec',
  target: 'order',
  updated_at: '2026-06-05T20:00:00.000Z',
  ...overrides,
})

describe('Site referral attribution consumption', () => {
  test('consumes a pending attribution for a user', async () => {
    const store = new Store()
    store.attributions.push(attribution())

    await expect(
      consumePendingReferralForUser(db(store), runtime, {
        pendingAttributionId: 'referral_attribution_otec',
        userId: 'github:1',
      }),
    ).resolves.toEqual({
      _tag: 'consumed',
      attributionId: 'referral_attribution_otec',
    })
    expect(store.userAttributions).toEqual([
      expect.objectContaining({
        referral_attribution_id: 'referral_attribution_otec',
        user_id: 'github:1',
      }),
    ])
    expect(store.attributions[0]).toMatchObject({
      claimed_user_id: 'github:1',
      first_verified_at: '2026-06-05T21:00:00.000Z',
      policy_state: 'claimed',
    })
  })

  test('preserves the first verified user attribution', async () => {
    const store = new Store()
    store.attributions.push(attribution({ id: 'referral_attribution_later' }))
    store.userAttributions.push({
      archived_at: null,
      referral_attribution_id: 'referral_attribution_first',
      user_id: 'github:1',
    })

    await expect(
      consumePendingReferralForUser(db(store), runtime, {
        pendingAttributionId: 'referral_attribution_later',
        userId: 'github:1',
      }),
    ).resolves.toEqual({
      _tag: 'already_verified',
      attributionId: 'referral_attribution_first',
    })
    expect(store.attributions[0]?.policy_state).toBe('pending')
  })

  test('links a consumed attribution to an order', async () => {
    const store = new Store()
    store.attributions.push(attribution())

    await expect(
      linkPendingReferralToOrder(db(store), runtime, {
        orderId: 'software_order_1',
        pendingAttributionId: 'referral_attribution_otec',
        userId: 'github:1',
      }),
    ).resolves.toEqual({
      _tag: 'consumed',
      attributionId: 'referral_attribution_otec',
    })
    expect(store.orderAttributions).toEqual([
      expect.objectContaining({
        referral_attribution_id: 'referral_attribution_otec',
        software_order_id: 'software_order_1',
        user_id: 'github:1',
      }),
    ])
  })

  test('does not consume expired pending attribution', async () => {
    const store = new Store()
    store.attributions.push(
      attribution({ expires_at: '2026-06-05T20:30:00.000Z' }),
    )

    await expect(
      consumePendingReferralForUser(db(store), runtime, {
        pendingAttributionId: 'referral_attribution_otec',
        userId: 'github:1',
      }),
    ).resolves.toEqual({
      _tag: 'expired',
      attributionId: 'referral_attribution_otec',
    })
    expect(store.userAttributions).toEqual([])
    expect(store.attributions[0]?.policy_state).toBe('pending')
  })

  test('does not consume when no pending attribution is present', async () => {
    const store = new Store()

    await expect(
      consumePendingReferralForUser(db(store), runtime, {
        pendingAttributionId: undefined,
        userId: 'github:1',
      }),
    ).resolves.toEqual({ _tag: 'none' })
    expect(store.userAttributions).toEqual([])
    expect(store.attributions).toEqual([])
  })

  test('rolls back consumed attribution when batch fails after first mutation', async () => {
    const store = new Store()
    store.attributions.push(attribution())
    store.failBatchAfterStatements = 1

    await expect(
      consumePendingReferralForUser(db(store), runtime, {
        pendingAttributionId: 'referral_attribution_otec',
        userId: 'github:1',
      }),
    ).rejects.toMatchObject({
      _tag: 'SiteReferralConsumptionStorageError',
      operation: 'siteReferralConsumption.user.batch',
    })
    expect(store.userAttributions).toEqual([])
    expect(store.attributions[0]).toMatchObject({
      claimed_user_id: null,
      first_verified_at: null,
      policy_state: 'pending',
    })
  })

  test('supports future agent claim linkage', async () => {
    const store = new Store()
    store.attributions.push(
      attribution({
        capture_path: 'agent',
        target: 'agent_claim',
      }),
    )

    await expect(
      linkPendingReferralToAgentClaim(db(store), runtime, {
        agentUserId: 'agent:demo',
        ownerUserId: 'github:1',
        pendingAttributionId: 'referral_attribution_otec',
      }),
    ).resolves.toEqual({
      _tag: 'consumed',
      attributionId: 'referral_attribution_otec',
    })
    expect(store.agentAttributions).toEqual([
      expect.objectContaining({
        agent_user_id: 'agent:demo',
        owner_user_id: 'github:1',
        referral_attribution_id: 'referral_attribution_otec',
      }),
    ])
  })
})
