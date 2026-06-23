import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type BuyModeAlertRecord,
  type BuyModeCampaignRecord,
  type BuyModeDispatcherStore,
  type BuyModeJobRecord,
  type BuyModePaymentBridge,
  type BuyModeRelayJobRequest,
  type BuyModeRelayPublisher,
} from './buy-mode-dispatcher'
import {
  type BuyModeEvalBridge,
  makeOperatorBuyModeRoutes,
} from './operator-buy-mode-routes'

const nowIso = '2026-06-10T08:00:00.000Z'

class MemoryBuyModeStore implements BuyModeDispatcherStore {
  readonly alerts: BuyModeAlertRecord[] = []
  readonly campaigns = new Map<string, BuyModeCampaignRecord>()
  readonly jobs = new Map<string, BuyModeJobRecord>()

  latestCampaign = async () =>
    [...this.campaigns.values()].sort((left, right) =>
      right.updatedAt.localeCompare(left.updatedAt)
    )[0] ?? null

  readCampaign = async (campaignId: string) =>
    this.campaigns.get(campaignId) ?? null

  readJobByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    [...this.jobs.values()].find(job =>
      job.idempotencyKeyHash === idempotencyKeyHash
    ) ?? null

  readJobByRequestEventId = async (requestEventId: string) =>
    [...this.jobs.values()].find(job => job.requestEventId === requestEventId) ??
    null

  readSettlementByResultEventId = async (resultEventId: string) =>
    [...this.jobs.values()].find(job => job.resultEventId === resultEventId) ??
    null

  recordAlertAndHalt = async (
    campaign: BuyModeCampaignRecord,
    alert: BuyModeAlertRecord,
  ) => {
    this.alerts.push(alert)
    this.campaigns.set(campaign.campaignId, {
      ...campaign,
      lastAlertRef: alert.reasonRef,
      state: 'halted',
      updatedAt: alert.createdAt,
    })
  }

  recordDispatch = async (
    campaign: BuyModeCampaignRecord,
    job: BuyModeJobRecord,
  ) => {
    this.campaigns.set(campaign.campaignId, {
      ...campaign,
      updatedAt: job.updatedAt,
    })
    this.jobs.set(job.jobId, job)
  }

  recordSettlement = async (
    campaign: BuyModeCampaignRecord,
    job: BuyModeJobRecord,
  ) => {
    this.campaigns.set(campaign.campaignId, campaign)
    this.jobs.set(job.jobId, job)
  }

  startCampaign = async (campaign: BuyModeCampaignRecord) => {
    this.campaigns.set(campaign.campaignId, campaign)
  }

  stopCampaign = async (
    campaign: BuyModeCampaignRecord,
    stoppedAt: string,
  ) => {
    this.campaigns.set(campaign.campaignId, {
      ...campaign,
      state: 'disabled',
      updatedAt: stoppedAt,
    })
  }
}

class FakeRelayPublisher implements BuyModeRelayPublisher {
  readonly requests: BuyModeRelayJobRequest[] = []

  publishJobRequest = async (input: BuyModeRelayJobRequest) => {
    this.requests.push(input)

    return {
      accepted: true,
      relayRef: 'relay.public.buy_mode.test',
      requestEventId: `event.buy_mode.${this.requests.length}`,
    }
  }
}

class FakePaymentBridge implements BuyModePaymentBridge {
  readonly requests: Array<{ amountMsats: number; bolt11: string }> = []

  payBolt11 = async (input: { amountMsats: number; bolt11: string }) => {
    this.requests.push(input)

    return {
      receiptRef: `receipt.public.buy_mode.${this.requests.length}`,
      settlementRef: `settlement.public.buy_mode.${this.requests.length}`,
    }
  }
}

class FakeEvalBridge implements BuyModeEvalBridge {
  readonly requests: Parameters<BuyModeEvalBridge['dispatchEval']>[0][] = []

  dispatchEval = async (
    input: Parameters<BuyModeEvalBridge['dispatchEval']>[0],
  ) => {
    this.requests.push(input)

    return {
      settledMsats: input.job.amountMsats,
      verdict: {
        class: 'exact_trace_replay' as const,
        passed: true,
      },
    }
  }
}

const makeRoutes = (
  store: MemoryBuyModeStore,
  relay = new FakeRelayPublisher(),
  paymentBridge = new FakePaymentBridge(),
  evalBridge: FakeEvalBridge | undefined = undefined,
) => ({
  evalBridge,
  paymentBridge,
  relay,
  routes: makeOperatorBuyModeRoutes<{ store: MemoryBuyModeStore }>({
    currentIsoTimestamp: () => nowIso,
    makeEvalBridge: () => evalBridge,
    makePaymentBridge: () => paymentBridge,
    makeRelayPublisher: () => relay,
    makeStore: env => env.store,
    makeUuid: () => 'fixed',
    requireAdminApiToken: request =>
      Promise.resolve(request.headers.get('authorization') === 'Bearer admin'),
  }),
})

const request = (path: string, body?: unknown, idempotencyKey = 'idem-key-1') =>
  new Request(
    `https://openagents.com${path}`,
    body === undefined
      ? {
          headers: {
            authorization: 'Bearer admin',
            'idempotency-key': idempotencyKey,
          },
          method: 'GET',
        }
      : {
          body: JSON.stringify(body),
          headers: {
            authorization: 'Bearer admin',
            'content-type': 'application/json',
            'idempotency-key': idempotencyKey,
          },
          method: 'POST',
        },
  )

const run = async (
  effect: Effect.Effect<Response> | undefined,
): Promise<Response> => {
  if (effect === undefined) throw new Error('route did not match')

  return Effect.runPromise(effect)
}

describe('operator buy-mode routes', () => {
  test('is disabled by default and requires admin token', async () => {
    const store = new MemoryBuyModeStore()
    const { routes } = makeRoutes(store)
    const anonymous = await run(routes.handleOperatorBuyModeStatusApi(
      new Request('https://openagents.com/api/operator/buy-mode'),
      { store },
    ))
    const authorized = await run(routes.handleOperatorBuyModeStatusApi(
      request('/api/operator/buy-mode'),
      { store },
    ))
    const body = await authorized.json() as Record<string, any>

    expect(anonymous.status).toBe(401)
    expect(authorized.status).toBe(200)
    expect(body.disabledByDefault).toBe(true)
    expect(body.authority.operatorApprovalRequiredForSpend).toBe(true)
  })

  test('starts disabled-spend campaign and dispatches a NIP-90 job only when enabled', async () => {
    const store = new MemoryBuyModeStore()
    const { relay, routes } = makeRoutes(store)
    const blockedDispatch = await run(routes.handleOperatorBuyModeDispatchApi(
      request('/api/operator/buy-mode/dispatch', {
        amountMsats: 1000,
        content: 'public prompt',
        providerPubkeys: ['11'.repeat(32)],
      }),
      { store },
    ))
    const start = await run(routes.handleOperatorBuyModeStartApi(
      request('/api/operator/buy-mode/start', {
        dailyCapMsats: 10_000,
        perJobCapMsats: 2_000,
        spendEnabled: false,
      }, 'start-key-1'),
      { store },
    ))
    const dispatch = await run(routes.handleOperatorBuyModeDispatchApi(
      request('/api/operator/buy-mode/dispatch', {
        amountMsats: 1000,
        content: 'public prompt',
        providerPubkeys: ['11'.repeat(32)],
      }, 'dispatch-key-1'),
      { store },
    ))
    const dispatchBody = await dispatch.json() as Record<string, any>

    expect(blockedDispatch.status).toBe(409)
    expect(start.status).toBe(201)
    expect(dispatch.status).toBe(201)
    expect(dispatchBody.result.kind).toBe('dispatched')
    expect(relay.requests).toHaveLength(1)
    expect(JSON.stringify(dispatchBody)).not.toContain('public prompt')
  })

  test('halts and alerts on cap breach before relay publication', async () => {
    const store = new MemoryBuyModeStore()
    const { relay, routes } = makeRoutes(store)

    await run(routes.handleOperatorBuyModeStartApi(
      request('/api/operator/buy-mode/start', {
        dailyCapMsats: 10_000,
        perJobCapMsats: 2_000,
      }, 'start-key-2'),
      { store },
    ))
    const response = await run(routes.handleOperatorBuyModeDispatchApi(
      request('/api/operator/buy-mode/dispatch', {
        amountMsats: 3_000,
        content: 'public prompt',
        providerPubkeys: ['11'.repeat(32)],
      }, 'dispatch-key-2'),
      { store },
    ))
    const body = await response.json() as Record<string, any>

    expect(response.status).toBe(409)
    expect(body.result.kind).toBe('halted')
    expect(body.result.alert.reasonRef).toBe('alert.buy_mode.per_job_cap_breach')
    expect((await store.latestCampaign())?.state).toBe('halted')
    expect(relay.requests).toHaveLength(0)
  })

  test('exposes Psionic-compatible eval endpoint behind existing dispatch admission', async () => {
    const store = new MemoryBuyModeStore()
    const evalBridge = new FakeEvalBridge()
    const { relay, routes } = makeRoutes(
      store,
      new FakeRelayPublisher(),
      new FakePaymentBridge(),
      evalBridge,
    )

    await run(routes.handleOperatorBuyModeStartApi(
      request('/api/operator/buy-mode/start', {
        dailyCapMsats: 10_000,
        perJobCapMsats: 2_000,
        spendEnabled: true,
      }, 'start-key-eval'),
      { store },
    ))
    const response = await run(routes.handleOperatorBuyModeEvalApi(
      request('/api/operator/buy-mode/eval', {
        amount_msats: 1_250,
        role_index: 2,
        sample_id: 'sample.http',
        worker_id: '11'.repeat(32),
      }),
      { store },
    ))
    const body = await response.json() as Record<string, any>

    expect(response.status).toBe(200)
    expect(body).toEqual({
      settled_msats: 1_250,
      verdict: {
        class: 'exact_trace_replay',
        passed: true,
      },
    })
    expect(relay.requests).toHaveLength(1)
    expect(evalBridge.requests).toHaveLength(1)
    expect(evalBridge.requests[0]?.job).toEqual({
      amountMsats: 1_250,
      roleIndex: 2,
      sampleId: 'sample.http',
      workerId: '11'.repeat(32),
    })
    expect(JSON.stringify(relay.requests[0])).not.toContain('Bearer')
  })

  test('blocks Psionic eval when live eval bridge is not configured', async () => {
    const store = new MemoryBuyModeStore()
    const { relay, routes } = makeRoutes(store)

    await run(routes.handleOperatorBuyModeStartApi(
      request('/api/operator/buy-mode/start', {
        dailyCapMsats: 10_000,
        perJobCapMsats: 2_000,
        spendEnabled: true,
      }, 'start-key-eval-unconfigured'),
      { store },
    ))
    const response = await run(routes.handleOperatorBuyModeEvalApi(
      request('/api/operator/buy-mode/eval', {
        amount_msats: 1_250,
        role_index: 2,
        sample_id: 'sample.http',
        worker_id: '11'.repeat(32),
      }),
      { store },
    ))
    const body = await response.json() as Record<string, any>

    expect(response.status).toBe(409)
    expect(body.result).toEqual({
      kind: 'blocked',
      reasonRef: 'blocker.buy_mode.eval_bridge_unconfigured',
    })
    expect(relay.requests).toHaveLength(1)
  })

  test('Psionic eval cap breaches halt before relay or eval bridge calls', async () => {
    const store = new MemoryBuyModeStore()
    const evalBridge = new FakeEvalBridge()
    const { relay, routes } = makeRoutes(
      store,
      new FakeRelayPublisher(),
      new FakePaymentBridge(),
      evalBridge,
    )

    await run(routes.handleOperatorBuyModeStartApi(
      request('/api/operator/buy-mode/start', {
        dailyCapMsats: 10_000,
        perJobCapMsats: 1_000,
        spendEnabled: true,
      }, 'start-key-eval-cap'),
      { store },
    ))
    const response = await run(routes.handleOperatorBuyModeEvalApi(
      request('/api/operator/buy-mode/eval', {
        amount_msats: 1_250,
        role_index: 2,
        sample_id: 'sample.http',
        worker_id: '11'.repeat(32),
      }),
      { store },
    ))
    const body = await response.json() as Record<string, any>

    expect(response.status).toBe(409)
    expect(body.result.kind).toBe('halted')
    expect(relay.requests).toHaveLength(0)
    expect(evalBridge.requests).toHaveLength(0)
  })

  test('blocks settlement until operator explicitly enables spend', async () => {
    const store = new MemoryBuyModeStore()
    const { paymentBridge, routes } = makeRoutes(store)

    await run(routes.handleOperatorBuyModeStartApi(
      request('/api/operator/buy-mode/start', {
        dailyCapMsats: 10_000,
        perJobCapMsats: 2_000,
        spendEnabled: false,
      }, 'start-key-3'),
      { store },
    ))
    await run(routes.handleOperatorBuyModeDispatchApi(
      request('/api/operator/buy-mode/dispatch', {
        amountMsats: 1000,
        content: 'public prompt',
        providerPubkeys: ['11'.repeat(32)],
      }, 'dispatch-key-3'),
      { store },
    ))
    const response = await run(routes.handleOperatorBuyModeSettleApi(
      request('/api/operator/buy-mode/results/settle', {
        amountMsats: 1000,
        bolt11: 'lnbc10n1testinvoice',
        content: 'valid public result',
        providerPubkey: '11'.repeat(32),
        requestEventId: 'event.buy_mode.1',
        resultEventId: 'result.event.1',
      }, 'settle-key-3'),
      { store },
    ))
    const body = await response.json() as Record<string, any>

    expect(response.status).toBe(409)
    expect(body.result.reasonRef).toBe(
      'blocker.buy_mode.operator_spend_approval_missing',
    )
    expect(paymentBridge.requests).toHaveLength(0)
  })

  test('blocks live settlement when the payment bridge is not configured', async () => {
    const store = new MemoryBuyModeStore()
    const relay = new FakeRelayPublisher()
    const routes = makeOperatorBuyModeRoutes<{ store: MemoryBuyModeStore }>({
      currentIsoTimestamp: () => nowIso,
      makeRelayPublisher: () => relay,
      makeStore: env => env.store,
      makeUuid: () => 'fixed',
      requireAdminApiToken: request =>
        Promise.resolve(request.headers.get('authorization') === 'Bearer admin'),
    })

    await run(routes.handleOperatorBuyModeStartApi(
      request('/api/operator/buy-mode/start', {
        dailyCapMsats: 10_000,
        perJobCapMsats: 2_000,
        spendEnabled: true,
      }, 'start-key-unconfigured'),
      { store },
    ))
    await run(routes.handleOperatorBuyModeDispatchApi(
      request('/api/operator/buy-mode/dispatch', {
        amountMsats: 1000,
        content: 'public prompt',
        providerPubkeys: ['11'.repeat(32)],
      }, 'dispatch-key-unconfigured'),
      { store },
    ))
    const response = await run(routes.handleOperatorBuyModeSettleApi(
      request('/api/operator/buy-mode/results/settle', {
        amountMsats: 1000,
        bolt11: 'lnbc10n1testinvoice',
        content: 'valid public result',
        providerPubkey: '11'.repeat(32),
        requestEventId: 'event.buy_mode.1',
        resultEventId: 'result.event.unconfigured',
      }, 'settle-key-unconfigured'),
      { store },
    ))
    const body = await response.json() as Record<string, any>

    expect(response.status).toBe(409)
    expect(body.result.reasonRef).toBe(
      'blocker.buy_mode.payment_bridge_unconfigured',
    )
    expect((await store.latestCampaign())?.spentTodayMsats).toBe(0)
  })

  test('settles only valid results and blocks duplicate settlement', async () => {
    const store = new MemoryBuyModeStore()
    const { paymentBridge, routes } = makeRoutes(store)

    await run(routes.handleOperatorBuyModeStartApi(
      request('/api/operator/buy-mode/start', {
        dailyCapMsats: 10_000,
        perJobCapMsats: 2_000,
        spendEnabled: true,
      }, 'start-key-4'),
      { store },
    ))
    await run(routes.handleOperatorBuyModeDispatchApi(
      request('/api/operator/buy-mode/dispatch', {
        amountMsats: 1000,
        content: 'public prompt',
        providerPubkeys: ['11'.repeat(32)],
      }, 'dispatch-key-4'),
      { store },
    ))
    const invalid = await run(routes.handleOperatorBuyModeSettleApi(
      request('/api/operator/buy-mode/results/settle', {
        amountMsats: 1000,
        bolt11: 'not-an-invoice',
        content: 'valid public result',
        providerPubkey: '11'.repeat(32),
        requestEventId: 'event.buy_mode.1',
        resultEventId: 'result.event.invalid',
      }, 'settle-key-invalid'),
      { store },
    ))
    const settled = await run(routes.handleOperatorBuyModeSettleApi(
      request('/api/operator/buy-mode/results/settle', {
        amountMsats: 1000,
        bolt11: 'lnbc10n1testinvoice',
        content: 'valid public result',
        providerPubkey: '11'.repeat(32),
        requestEventId: 'event.buy_mode.1',
        resultEventId: 'result.event.valid',
      }, 'settle-key-valid'),
      { store },
    ))
    const replay = await run(routes.handleOperatorBuyModeSettleApi(
      request('/api/operator/buy-mode/results/settle', {
        amountMsats: 1000,
        bolt11: 'lnbc10n1testinvoice',
        content: 'valid public result',
        providerPubkey: '11'.repeat(32),
        requestEventId: 'event.buy_mode.1',
        resultEventId: 'result.event.valid',
      }, 'settle-key-replay'),
      { store },
    ))
    const settledBody = await settled.json() as Record<string, any>
    const replayBody = await replay.json() as Record<string, any>

    expect(invalid.status).toBe(409)
    expect(settled.status).toBe(201)
    expect(replay.status).toBe(200)
    expect(settledBody.result.kind).toBe('settled')
    expect(replayBody.result.kind).toBe('idempotent_replay')
    expect(paymentBridge.requests).toHaveLength(1)
    expect(JSON.stringify(settledBody)).not.toContain('lnbc10n1testinvoice')
    expect((await store.latestCampaign())?.spentTodayMsats).toBe(1000)
  })
})
