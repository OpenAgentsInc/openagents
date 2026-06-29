import { describe, expect, test } from 'vitest'

import {
  type AgentCredentialLookup,
  type AgentCredentialRecord,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  type AgentReissueSelector,
  type AgentReissueTarget,
  createProgrammaticAgentRegistration,
  sha256Hex,
} from './agent-registration'
import {
  type Env,
  handleAdminReissueAgentToken,
  handleProgrammaticAgentRegistration,
} from './index'

class MemoryAgentRegistrationStore implements AgentRegistrationStore {
  readonly registrations: Array<AgentRegistrationRecord> = []
  readonly addedCredentials: Array<AgentCredentialRecord> = []

  createAgentRegistration(record: AgentRegistrationRecord): Promise<void> {
    this.registrations.push(record)

    return Promise.resolve()
  }

  findAgentByTokenHash(
    _tokenHash: string,
    _now: string,
  ): Promise<AgentCredentialLookup | undefined> {
    return Promise.resolve(undefined)
  }

  touchAgentCredential(
    _credentialId: string,
    _lastUsedAt: string,
  ): Promise<void> {
    return Promise.resolve()
  }

  updateAgentDisplayName(): Promise<number> {
    return Promise.resolve(0)
  }

  findAgentForReissue(
    selector: AgentReissueSelector,
  ): Promise<AgentReissueTarget | undefined> {
    const registration = this.registrations.find(item =>
      'slug' in selector
        ? item.profile.slug === selector.slug
        : item.identity.providerSubject === selector.externalId,
    )

    return Promise.resolve(
      registration === undefined
        ? undefined
        : {
            userId: registration.user.id,
            slug: registration.profile.slug,
            displayName: registration.user.displayName,
          },
    )
  }

  addAgentCredential(record: AgentCredentialRecord): Promise<void> {
    this.addedCredentials.push(record)

    return Promise.resolve()
  }
}

type TipRecipientWalletRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  spark_address: string | null
  bolt12_offer: string | null
  lightning_address: string | null
  caveat_refs_json: string
  claim_policy_refs_json: string
  created_at: string
  custody_policy_refs_json: string
  disabled_at: string | null
  id: string
  payout_target_approval_ref: string | null
  provider_class: 'external_lightning' | 'hosted_mdk' | 'mdk_agent_wallet'
  public_projection_json: string
  readiness_refs_json: string
  receive_capability_ref: string
  source_ref: string
  state: 'ready' | 'disabled' | 'blocked'
  updated_at: string
  wallet_ref: string
}>

class TipRecipientWalletStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly store: TipRecipientWalletD1,
    private readonly query: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>) {
    this.values = values

    return this
  }

  first<T>() {
    if (this.query.includes('FROM forum_tip_recipient_wallets')) {
      const actorRef = String(this.values[0])

      return Promise.resolve(
        (this.store.tipRecipientWallets.find(
          row => row.actor_ref === actorRef && row.archived_at === null,
        ) ?? null) as T | null,
      )
    }

    return Promise.resolve(null as T | null)
  }

  run<T>() {
    if (this.query.includes('INSERT INTO forum_tip_recipient_wallets')) {
      const actorRef = String(this.values[1])
      const row: TipRecipientWalletRow = {
        actor_ref: actorRef,
        archived_at: null,
        spark_address: this.values[5] === null ? null : String(this.values[5]),
        bolt12_offer: this.values[6] === null ? null : String(this.values[6]),
        lightning_address:
          this.values[7] === null ? null : String(this.values[7]),
        caveat_refs_json: String(this.values[10]),
        claim_policy_refs_json: String(this.values[12]),
        created_at: String(this.values[16]),
        custody_policy_refs_json: String(this.values[11]),
        disabled_at: this.values[18] === null ? null : String(this.values[18]),
        id: String(this.values[0]),
        payout_target_approval_ref:
          this.values[8] === null ? null : String(this.values[8]),
        provider_class: this.values[2] as TipRecipientWalletRow['provider_class'],
        public_projection_json: String(this.values[15]),
        readiness_refs_json: String(this.values[9]),
        receive_capability_ref: String(this.values[4]),
        source_ref: String(this.values[13]),
        state: this.values[14] as TipRecipientWalletRow['state'],
        updated_at: String(this.values[17]),
        wallet_ref: String(this.values[3]),
      }
      const existingIndex = this.store.tipRecipientWallets.findIndex(
        existing => existing.actor_ref === actorRef,
      )

      if (existingIndex === -1) {
        this.store.tipRecipientWallets.push(row)
      } else {
        this.store.tipRecipientWallets[existingIndex] = row
      }
    }

    return Promise.resolve({ success: true } as T)
  }
}

class TipRecipientWalletD1 {
  readonly tipRecipientWallets: Array<TipRecipientWalletRow> = []

  prepare(query: string) {
    return new TipRecipientWalletStatement(this, query)
  }
}

const SPARK_ADDRESS =
  'spark1pgssyuuuhnrrdjswal5c3s3rafw9w3y5dd4cjy3duxlf7hjzkp0rqx6dj6mrhu'

describe('programmatic agent registration route', () => {
  test('public registration returns an immediately usable agent token', async () => {
    const store = new MemoryAgentRegistrationStore()
    const response = await handleProgrammaticAgentRegistration(
      new Request('https://openagents.com/api/agents/register', {
        body: JSON.stringify({
          displayName: 'Self Service Forum Agent',
          externalId: 'self-service-forum-agent-1',
          metadata: { purpose: 'forum_posting' },
          slug: 'self-service-forum-agent',
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      {} as Env,
      store,
    )
    const body = (await response.json()) as {
      credential: { token: string; tokenPrefix: string }
      user: { status: string }
    }
    const record = store.registrations[0]

    expect(response.status).toBe(201)
    expect(body.user.status).toBe('active')
    expect(body.credential.token).toMatch(/^oa_agent_/)
    expect(record?.credential.status).toBe('active')
    expect(record?.credential.tokenPrefix).toBe(body.credential.tokenPrefix)
    expect(record?.credential.tokenHash).toBe(
      await sha256Hex(body.credential.token),
    )
  })

  test('auto-claims native Spark as the default tip-recipient rail', async () => {
    const store = new MemoryAgentRegistrationStore()
    const db = new TipRecipientWalletD1()
    const legacyBolt12Offer =
      'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j'
    const response = await handleProgrammaticAgentRegistration(
      new Request('https://openagents.com/api/agents/register', {
        body: JSON.stringify({
          displayName: 'Spark Default Forum Agent',
          externalId: 'spark-default-forum-agent-1',
          metadata: { purpose: 'forum_posting' },
          slug: 'spark-default-forum-agent',
          sparkAddress: SPARK_ADDRESS,
          bolt12Offer: legacyBolt12Offer,
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      { OPENAGENTS_DB: db as unknown as D1Database } as Env,
      store,
    )
    const body = (await response.json()) as {
      user: { id: string; status: string }
    }
    const row = db.tipRecipientWallets[0]
    const projection = JSON.parse(row?.public_projection_json ?? '{}') as {
      directPayment?: { kind?: string; sparkAddress?: string }
      readinessRefs?: ReadonlyArray<string>
    }

    expect(response.status).toBe(201)
    expect(body.user.status).toBe('active')
    expect(row?.actor_ref).toBe(`agent:${body.user.id}`)
    expect(row?.spark_address).toBe(SPARK_ADDRESS)
    expect(row?.bolt12_offer).toBe(legacyBolt12Offer)
    expect(row?.provider_class).toBe('mdk_agent_wallet')
    expect(projection.directPayment).toMatchObject({
      kind: 'spark_address',
      sparkAddress: SPARK_ADDRESS,
    })
    expect(projection.readinessRefs).toEqual([
      'readiness.public.spark_address.offline_receive_ready',
      'readiness.public.spark_primary.agent_balance',
    ])
  })
})

const reissueRequest = (body: unknown): Request =>
  new Request('https://openagents.com/api/admin/agents/reissue-token', {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

const seedAgent = async (
  store: MemoryAgentRegistrationStore,
  input: Readonly<{ displayName: string; slug?: string; externalId?: string }>,
) =>
  createProgrammaticAgentRegistration(store, input, {
    makeToken: () => `oa_agent_${input.slug ?? input.externalId}_original`,
  })

describe('admin agent token reissue route (#6370)', () => {
  test('admin reissue mints a fresh token for an existing slug, same entity', async () => {
    const store = new MemoryAgentRegistrationStore()
    const original = await seedAgent(store, {
      displayName: 'Artanis',
      externalId: 'artanis-1',
      slug: 'artanis',
    })

    const response = await handleAdminReissueAgentToken(
      reissueRequest({ slug: 'artanis' }),
      {} as Env,
      {} as ExecutionContext,
      { agentRegistrationStore: store, authorize: async () => true },
    )
    const body = (await response.json()) as {
      token: string
      tokenPrefix: string
      slug: string | null
      actorRef: string
    }

    expect(response.status).toBe(201)
    expect(body.token).toMatch(/^oa_agent_/)
    expect(body.token).not.toBe(original.credential.token)
    expect(body.slug).toBe('artanis')
    expect(body.actorRef).toBe(`agent:${original.user.id}`)

    // The fresh credential is stored as a hash bound to the same agent user.
    const added = store.addedCredentials[0]
    expect(added?.userId).toBe(original.user.id)
    expect(added?.status).toBe('active')
    expect(added?.tokenPrefix).toBe(body.tokenPrefix)
    expect(added?.tokenHash).toBe(await sha256Hex(body.token))
    expect(added?.tokenHash).not.toContain('oa_agent_')
  })

  test('non-admin callers are refused', async () => {
    const store = new MemoryAgentRegistrationStore()
    await seedAgent(store, { displayName: 'Artanis', slug: 'artanis' })

    const response = await handleAdminReissueAgentToken(
      reissueRequest({ slug: 'artanis' }),
      {} as Env,
      {} as ExecutionContext,
      { agentRegistrationStore: store, authorize: async () => false },
    )

    expect(response.status).toBe(403)
    expect(store.addedCredentials).toHaveLength(0)
  })

  test('unknown slug returns 404', async () => {
    const store = new MemoryAgentRegistrationStore()

    const response = await handleAdminReissueAgentToken(
      reissueRequest({ slug: 'no-such-agent' }),
      {} as Env,
      {} as ExecutionContext,
      { agentRegistrationStore: store, authorize: async () => true },
    )
    const body = (await response.json()) as { error?: string }

    expect(response.status).toBe(404)
    expect(body.error).toBe('agent_not_found')
    expect(store.addedCredentials).toHaveLength(0)
  })

  test('missing slug and externalId is a bad request', async () => {
    const store = new MemoryAgentRegistrationStore()

    const response = await handleAdminReissueAgentToken(
      reissueRequest({}),
      {} as Env,
      {} as ExecutionContext,
      { agentRegistrationStore: store, authorize: async () => true },
    )

    expect(response.status).toBe(400)
  })
})
