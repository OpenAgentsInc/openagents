import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { beforeEach, describe, expect, test } from 'vitest'

import type { AgentRegistrationStore } from './agent-registration'
import { makeForumRoutes } from './forum-routes'
import type {
  ForumWorkRequestAcceptanceRelayPublishInput,
  ForumWorkRequestAcceptanceRelayPublishReceipt,
  ForumWorkRequestRelayPublisher,
} from './forum-work-requests'

// ---------------------------------------------------------------------------
// Minimal real-SQL D1 adapter backed by node:sqlite. We exercise the real
// negotiation + escrow code paths (offer POST, acceptance + relay publish,
// result POST, validator-pass release) against genuine SQL so exactly-once
// and idempotency guarantees are real, not modeled.
// ---------------------------------------------------------------------------

type Row = Record<string, unknown>

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }

  async first<T = Row>(): Promise<T | null> {
    const row = this.db.prepare(this.sql).get(...(this.bound as never[]))
    return (row ?? null) as T | null
  }

  async all<T = Row>(): Promise<{ results: T[] }> {
    const results = this.db
      .prepare(this.sql)
      .all(...(this.bound as never[])) as T[]
    return { results }
  }

  async run<T = Row>(): Promise<{ success: true; results: T[] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true, results: [] }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }

  async batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<Array<{ success: true }>> {
    this.db.exec('BEGIN')
    try {
      for (const statement of statements) {
        await statement.run()
      }
      this.db.exec('COMMIT')
    } catch (error) {
      this.db.exec('ROLLBACK')
      throw error
    }
    return statements.map(() => ({ success: true as const }))
  }
}

// Schema subset needed by the negotiation + escrow paths. FK REFERENCES to
// forum_topics/forum_posts are dropped because this isolated harness does not
// seed those tables; the negotiation/escrow logic under test does not depend
// on them.
const SCHEMA = `
CREATE TABLE agent_balances (
  actor_ref TEXT PRIMARY KEY,
  balance_msat INTEGER NOT NULL DEFAULT 0 CHECK (balance_msat >= 0),
  held_msat INTEGER NOT NULL DEFAULT 0 CHECK (held_msat >= 0),
  usd_credit_msat INTEGER NOT NULL DEFAULT 0 CHECK (usd_credit_msat >= 0),
  sweep_enabled INTEGER NOT NULL DEFAULT 1,
  sweep_threshold_sat INTEGER NOT NULL DEFAULT 210,
  send_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  receive_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TRIGGER agent_balances_available_insert
BEFORE INSERT ON agent_balances
WHEN NEW.balance_msat < NEW.held_msat
BEGIN
  SELECT RAISE(ABORT, 'agent_balance_available_nonnegative');
END;

CREATE TRIGGER agent_balances_available_update
BEFORE UPDATE OF balance_msat, held_msat ON agent_balances
WHEN NEW.balance_msat < NEW.held_msat
BEGIN
  SELECT RAISE(ABORT, 'agent_balance_available_nonnegative');
END;

CREATE TABLE forum_work_requests (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  topic_id TEXT NOT NULL,
  first_post_id TEXT NOT NULL,
  requester_actor_ref TEXT NOT NULL,
  title TEXT NOT NULL,
  objective_ref TEXT NOT NULL,
  verification_command_ref TEXT NOT NULL,
  repository_refs_json TEXT NOT NULL DEFAULT '[]',
  required_capability_refs_json TEXT NOT NULL DEFAULT '[]',
  budget_sats INTEGER NOT NULL,
  budget_msats INTEGER NOT NULL,
  deadline_ref TEXT NOT NULL,
  relay_url TEXT NOT NULL,
  job_event_id TEXT NOT NULL UNIQUE,
  job_event_kind INTEGER NOT NULL,
  job_result_kind INTEGER NOT NULL,
  state TEXT NOT NULL DEFAULT 'open',
  quote_count INTEGER NOT NULL DEFAULT 0,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_work_request_relay_links (
  id TEXT PRIMARY KEY NOT NULL,
  work_request_id TEXT NOT NULL,
  topic_id TEXT NOT NULL,
  job_event_id TEXT NOT NULL,
  job_event_kind INTEGER NOT NULL,
  relay_url TEXT NOT NULL,
  relay_ref TEXT NOT NULL,
  bridge_actor_ref TEXT NOT NULL,
  event_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_work_request_offers (
  id TEXT PRIMARY KEY NOT NULL,
  work_request_id TEXT NOT NULL,
  quote_ref TEXT NOT NULL UNIQUE,
  provider_actor_ref TEXT NOT NULL,
  provider_pubkey TEXT,
  amount_sats INTEGER NOT NULL CHECK (amount_sats > 0),
  amount_msats INTEGER NOT NULL CHECK (amount_msats > 0),
  capability_refs_json TEXT NOT NULL DEFAULT '[]',
  relay_event_ref TEXT,
  state TEXT NOT NULL DEFAULT 'offered',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_work_request_acceptances (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  work_request_id TEXT NOT NULL UNIQUE,
  offer_id TEXT NOT NULL,
  quote_ref TEXT NOT NULL,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT NOT NULL,
  amount_msats INTEGER NOT NULL,
  escrow_id TEXT NOT NULL UNIQUE,
  reserve_receipt_ref TEXT NOT NULL UNIQUE,
  acceptance_event_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_work_request_results (
  id TEXT PRIMARY KEY NOT NULL,
  work_request_id TEXT NOT NULL,
  offer_id TEXT NOT NULL,
  quote_ref TEXT NOT NULL UNIQUE,
  provider_actor_ref TEXT NOT NULL,
  result_event_ref TEXT NOT NULL,
  verification_command_ref TEXT NOT NULL,
  artifact_refs_json TEXT NOT NULL DEFAULT '[]',
  closeout_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE labor_escrows (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  work_request_id TEXT NOT NULL UNIQUE,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  state TEXT NOT NULL,
  funding_source TEXT NOT NULL DEFAULT 'ledger_balance',
  job_event_id TEXT NOT NULL,
  acceptance_event_ref TEXT,
  reserve_receipt_ref TEXT NOT NULL UNIQUE,
  release_receipt_ref TEXT UNIQUE,
  refund_receipt_ref TEXT UNIQUE,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  released_at TEXT,
  refunded_at TEXT,
  archived_at TEXT
);

CREATE TABLE labor_escrow_receipts (
  id TEXT PRIMARY KEY NOT NULL,
  escrow_id TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  transition_kind TEXT NOT NULL,
  work_request_id TEXT NOT NULL,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT,
  amount_msat INTEGER NOT NULL,
  receipt_ref TEXT NOT NULL UNIQUE,
  evidence_ref TEXT,
  state_after TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_labor_escrow_receipts_once
  ON labor_escrow_receipts(escrow_id, transition_kind);
`

const PUBLIC_PROJECTION_JSON = JSON.stringify({
  classificationCaveatRef: 'classification.public_forum_projection',
  customerSafe: true,
  dataClassification: 'public',
  excludedPrivateRefs: [],
  publicSafe: true,
  redactionPolicyRef: 'redaction.forum.public.v1',
  safeArtifactRefs: ['artifact.forum.work_request.wr_live_1'],
  safeReceiptRefs: [],
  trustTier: 'reviewed',
})

const NOW = '2026-06-13T18:00:00.000Z'
const REQUESTER = 'agent:requester-live'
const PROVIDER_PUBKEY = '2'.repeat(64)
const JOB_EVENT_ID = 'a'.repeat(64)
const WORK_REQUEST_ID = 'wr_live_1'

const seedWorkRequest = (db: DatabaseSync): void => {
  db.prepare(
    `INSERT INTO agent_balances (actor_ref, balance_msat, held_msat, created_at, updated_at)
     VALUES (?, ?, 0, ?, ?)`,
  ).run(REQUESTER, 5_000_000, NOW, NOW)

  db.prepare(
    `INSERT INTO forum_work_requests (
       id, idempotency_key, topic_id, first_post_id, requester_actor_ref,
       title, objective_ref, verification_command_ref, repository_refs_json,
       required_capability_refs_json, budget_sats, budget_msats, deadline_ref,
       relay_url, job_event_id, job_event_kind, job_result_kind, state,
       quote_count, public_projection_json, created_at, updated_at
     )
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 5934, 6934, 'open',
             0, ?, ?, ?)`,
  ).run(
    WORK_REQUEST_ID,
    'wr-live-idem',
    'topic_live_1',
    'post_live_1',
    REQUESTER,
    'Live negotiated job',
    'objective.public.live.fix',
    'command.public.pylon.labor.bun_test',
    JSON.stringify(['repo.public.github.OpenAgentsInc.openagents']),
    JSON.stringify(['capability.pylon.local_claude_agent']),
    2_000,
    2_000_000,
    'deadline.public.live.20260613',
    'wss://relay.openagents.com',
    JOB_EVENT_ID,
    PUBLIC_PROJECTION_JSON,
    NOW,
    NOW,
  )
}

// Registered-agent auth store: accepts the test bearer token and maps it to a
// stable agent actor whose actorRef is `agent:<userId>`. The requester routes
// (acceptance, release) check the resolved actorRef against the work request's
// requesterActorRef.
const liveAgentStore = (agentUserId: string): AgentRegistrationStore =>
  ({
    createAgentRegistration: () => Promise.resolve(),
    findAgentByTokenHash: () =>
      Promise.resolve({
        credentialId: 'credential-live',
        profileMetadataJson: '{}',
        tokenPrefix: 'oa_agent_',
        user: {
          avatarUrl: null,
          createdAt: NOW,
          displayName: 'Live Agent',
          id: agentUserId,
          kind: 'agent' as const,
          primaryEmail: 'agent@example.com',
          status: 'active' as const,
          updatedAt: NOW,
        },
      }),
    touchAgentCredential: () => Promise.resolve(),
  }) as unknown as AgentRegistrationStore

type CapturedAcceptance = ForumWorkRequestAcceptanceRelayPublishInput

const acceptanceCapturingPublisher = (
  captured: Array<CapturedAcceptance>,
): ForumWorkRequestRelayPublisher => ({
  publishAcceptance: async (
    input,
  ): Promise<ForumWorkRequestAcceptanceRelayPublishReceipt> => {
    captured.push(input)
    return {
      accepted: true,
      acceptanceEventId: 'c'.repeat(64),
      event: { kind: 7000 },
      relayRef: 'relay.public.market.live',
      relayUrl: input.relayUrl,
    }
  },
  publishWorkRequest: async () => {
    throw new Error('publishWorkRequest is not used in negotiation tests')
  },
})

const makeHarness = (
  options: Readonly<{
    agentUserId?: string
    captured?: Array<CapturedAcceptance>
    idSequence?: () => string
  }> = {},
) => {
  const db = new DatabaseSync(':memory:')
  db.exec(SCHEMA)
  seedWorkRequest(db)
  const d1 = new SqliteD1(db) as unknown as D1Database
  const captured = options.captured ?? []

  let counter = 0
  const makeId =
    options.idSequence ??
    (() => {
      counter += 1
      return `id_${counter}`
    })

  const route = async (
    path: string,
    init: Readonly<{
      authToken?: string | undefined
      body?: unknown
      idempotencyKey?: string
      method?: string
    }> = {},
  ): Promise<Response> => {
    const headers: Record<string, string> = {}
    if (init.body !== undefined) {
      headers['content-type'] = 'application/json'
    }
    if (init.authToken !== undefined) {
      headers.authorization = `Bearer ${init.authToken}`
    }
    if (init.idempotencyKey !== undefined) {
      headers['idempotency-key'] = init.idempotencyKey
    }

    const request = new Request(`https://openagents.com${path}`, {
      ...(init.body === undefined
        ? {}
        : { body: JSON.stringify(init.body) }),
      headers,
      method: init.method ?? 'GET',
    })

    const effect = makeForumRoutes({
      agentStore: liveAgentStore(options.agentUserId ?? 'requester-live'),
      forumWorkRequestRelayPublisher: acceptanceCapturingPublisher(captured),
      makeId,
      nowIso: () => NOW,
    }).routeForumRequest(request, d1)

    if (effect === undefined) {
      throw new Error(`Forum route not matched for ${path}`)
    }

    return Effect.runPromise(effect)
  }

  return { captured, db, route }
}

describe('live NIP-LBR negotiation route plumbing', () => {
  let harness: ReturnType<typeof makeHarness>

  beforeEach(() => {
    harness = makeHarness()
  })

  test('(a) offer POST records, is idempotent on quoteRef, and shows in offers GET', async () => {
    const first = await harness.route(
      `/api/forum/work-requests/${WORK_REQUEST_ID}/offers`,
      {
        authToken: 'oa_agent_live',
        body: {
          amountSats: 1,
          providerActorRef: 'provider.public.pylon.independent',
          providerPubkey: PROVIDER_PUBKEY,
          quoteRef: 'quote.public.live.one',
          relayEventRef: `nostr.event.${'b'.repeat(64)}`,
        },
        method: 'POST',
      },
    )
    expect(first.status).toBe(201)
    const firstBody = (await first.json()) as {
      idempotent: boolean
      offer: { amountSats: number; providerPubkey: string; quoteRef: string }
    }
    expect(firstBody.idempotent).toBe(false)
    expect(firstBody.offer.quoteRef).toBe('quote.public.live.one')
    expect(firstBody.offer.amountSats).toBe(1)
    expect(firstBody.offer.providerPubkey).toBe(PROVIDER_PUBKEY)

    // Idempotent re-submit of the same quoteRef does not duplicate.
    const repeat = await harness.route(
      `/api/forum/work-requests/${WORK_REQUEST_ID}/offers`,
      {
        authToken: 'oa_agent_live',
        body: {
          amountSats: 1,
          providerActorRef: 'provider.public.pylon.independent',
          providerPubkey: PROVIDER_PUBKEY,
          quoteRef: 'quote.public.live.one',
        },
        method: 'POST',
      },
    )
    expect(repeat.status).toBe(200)
    await expect(repeat.json()).resolves.toMatchObject({ idempotent: true })

    const offers = await harness.route(
      `/api/forum/work-requests/${WORK_REQUEST_ID}/offers`,
    )
    const offersBody = (await offers.json()) as {
      offers: ReadonlyArray<{ quoteRef: string }>
    }
    expect(offersBody.offers).toHaveLength(1)
    expect(offersBody.offers[0]?.quoteRef).toBe('quote.public.live.one')
  })

  test('(a) offer POST requires agent auth', async () => {
    const unauth = await harness.route(
      `/api/forum/work-requests/${WORK_REQUEST_ID}/offers`,
      {
        body: {
          amountSats: 1,
          providerActorRef: 'provider.public.pylon.independent',
          quoteRef: 'quote.public.live.unauth',
        },
        method: 'POST',
      },
    )
    expect(unauth.status).toBe(401)
  })

  const submitOffer = async () =>
    harness.route(`/api/forum/work-requests/${WORK_REQUEST_ID}/offers`, {
      authToken: 'oa_agent_live',
      body: {
        amountSats: 1,
        providerActorRef: 'provider.public.pylon.independent',
        providerPubkey: PROVIDER_PUBKEY,
        quoteRef: 'quote.public.live.one',
      },
      method: 'POST',
    })

  const acceptOffer = async (idempotencyKey = 'accept-live-1') =>
    harness.route(`/api/forum/work-requests/${WORK_REQUEST_ID}/acceptances`, {
      authToken: 'oa_agent_live',
      body: { quoteRef: 'quote.public.live.one' },
      idempotencyKey,
      method: 'POST',
    })

  test('(b) acceptance reserves escrow and publishes a relay acceptance carrying the reserve receipt ref + provider pubkey', async () => {
    await submitOffer()
    const accepted = await acceptOffer()
    expect(accepted.status).toBe(201)

    const acceptedBody = (await accepted.json()) as {
      acceptanceRelay: { accepted: boolean; relayRef: string | null }
      escrowState: { reserveReceiptRef: string }
    }
    expect(acceptedBody.acceptanceRelay.accepted).toBe(true)

    expect(harness.captured).toHaveLength(1)
    const published = harness.captured[0]!
    expect(published.providerPubkey).toBe(PROVIDER_PUBKEY)
    expect(published.escrowReceiptRef).toBe(
      acceptedBody.escrowState.reserveReceiptRef,
    )
    expect(published.quoteRef).toBe('quote.public.live.one')
    expect(published.jobEventId).toBe(JOB_EVENT_ID)
    // public-safe: no wallet/payment material in the published acceptance
    expect(JSON.stringify(published)).not.toMatch(
      /lnbc|preimage|payment_hash|mnemonic|secret|xprv|\/Users\//i,
    )

    // Escrow held the requester funds.
    const balance = harness.db
      .prepare('SELECT balance_msat, held_msat FROM agent_balances WHERE actor_ref = ?')
      .get(REQUESTER) as { balance_msat: number; held_msat: number }
    expect(balance.held_msat).toBe(1_000)
    expect(balance.balance_msat).toBe(5_000_000)
  })

  test('(c) result POST records the delivered result against the accepted offer', async () => {
    await submitOffer()
    await acceptOffer()

    const recorded = await harness.route(
      `/api/forum/work-requests/${WORK_REQUEST_ID}/results`,
      {
        authToken: 'oa_agent_live',
        body: {
          artifactRefs: ['artifact.public.live.patch_1'],
          closeoutRef: 'closeout.public.live.1',
          quoteRef: 'quote.public.live.one',
          resultEventRef: `nostr.event.${'d'.repeat(64)}`,
          verificationCommandRef: 'command.public.pylon.labor.bun_test',
        },
        method: 'POST',
      },
    )
    expect(recorded.status).toBe(201)
    await expect(recorded.json()).resolves.toMatchObject({
      idempotent: false,
      result: {
        artifactRefs: ['artifact.public.live.patch_1'],
        quoteRef: 'quote.public.live.one',
        resultEventRef: `nostr.event.${'d'.repeat(64)}`,
      },
    })

    // Idempotent on quoteRef.
    const repeat = await harness.route(
      `/api/forum/work-requests/${WORK_REQUEST_ID}/results`,
      {
        authToken: 'oa_agent_live',
        body: {
          quoteRef: 'quote.public.live.one',
          resultEventRef: `nostr.event.${'d'.repeat(64)}`,
          verificationCommandRef: 'command.public.pylon.labor.bun_test',
        },
        method: 'POST',
      },
    )
    expect(repeat.status).toBe(200)
    await expect(repeat.json()).resolves.toMatchObject({ idempotent: true })
  })

  test('(c) result POST refuses when there is no accepted offer', async () => {
    await submitOffer()
    const refused = await harness.route(
      `/api/forum/work-requests/${WORK_REQUEST_ID}/results`,
      {
        authToken: 'oa_agent_live',
        body: {
          quoteRef: 'quote.public.live.one',
          resultEventRef: `nostr.event.${'d'.repeat(64)}`,
          verificationCommandRef: 'command.public.pylon.labor.bun_test',
        },
        method: 'POST',
      },
    )
    expect(refused.status).toBe(409)
    await expect(refused.json()).resolves.toMatchObject({
      error: 'result_requires_accepted_offer',
    })
  })

  const recordResult = async () =>
    harness.route(`/api/forum/work-requests/${WORK_REQUEST_ID}/results`, {
      authToken: 'oa_agent_live',
      body: {
        artifactRefs: ['artifact.public.live.patch_1'],
        quoteRef: 'quote.public.live.one',
        resultEventRef: `nostr.event.${'d'.repeat(64)}`,
        verificationCommandRef: 'command.public.pylon.labor.bun_test',
      },
      method: 'POST',
    })

  const releaseEscrow = async () =>
    harness.route(`/api/forum/work-requests/${WORK_REQUEST_ID}/release`, {
      authToken: 'oa_agent_live',
      body: {
        quoteRef: 'quote.public.live.one',
        verificationVerdictRef: 'verdict.public.live.validator_passed',
      },
      method: 'POST',
    })

  test('(d) release moves the reserved escrow to the provider balance exactly once and is idempotent', async () => {
    await submitOffer()
    await acceptOffer()
    await recordResult()

    const released = await releaseEscrow()
    expect(released.status).toBe(200)
    const releasedBody = (await released.json()) as {
      escrow: { providerActorRef: string; state: string }
      idempotent: boolean
      released: boolean
    }
    expect(releasedBody.released).toBe(true)
    expect(releasedBody.idempotent).toBe(false)
    expect(releasedBody.escrow.state).toBe('released_to_provider')

    const requester = harness.db
      .prepare('SELECT balance_msat, held_msat FROM agent_balances WHERE actor_ref = ?')
      .get(REQUESTER) as { balance_msat: number; held_msat: number }
    const provider = harness.db
      .prepare('SELECT balance_msat, held_msat FROM agent_balances WHERE actor_ref = ?')
      .get(releasedBody.escrow.providerActorRef) as {
      balance_msat: number
      held_msat: number
    }
    expect(requester).toEqual({ balance_msat: 4_999_000, held_msat: 0 })
    expect(provider).toEqual({ balance_msat: 1_000, held_msat: 0 })

    // Exactly-once: a second release does not move funds again.
    const again = await releaseEscrow()
    expect(again.status).toBe(200)
    await expect(again.json()).resolves.toMatchObject({ idempotent: true })

    const requesterAfter = harness.db
      .prepare('SELECT balance_msat, held_msat FROM agent_balances WHERE actor_ref = ?')
      .get(REQUESTER) as { balance_msat: number; held_msat: number }
    const providerAfter = harness.db
      .prepare('SELECT balance_msat, held_msat FROM agent_balances WHERE actor_ref = ?')
      .get(releasedBody.escrow.providerActorRef) as {
      balance_msat: number
      held_msat: number
    }
    expect(requesterAfter).toEqual({ balance_msat: 4_999_000, held_msat: 0 })
    expect(providerAfter).toEqual({ balance_msat: 1_000, held_msat: 0 })

    const receipts = harness.db
      .prepare(
        'SELECT transition_kind FROM labor_escrow_receipts ORDER BY created_at',
      )
      .all() as Array<{ transition_kind: string }>
    expect(receipts.map(receipt => receipt.transition_kind)).toEqual([
      'reserve',
      'release',
    ])
  })

  test('(d) release refuses before a result is recorded', async () => {
    await submitOffer()
    await acceptOffer()
    const refused = await releaseEscrow()
    expect(refused.status).toBe(409)
    await expect(refused.json()).resolves.toMatchObject({
      error: 'release_requires_recorded_result',
    })
  })
})
