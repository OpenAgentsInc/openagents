#!/usr/bin/env bun
/**
 * Operator-gated live NIP-90 kind-5050 compute buy smoke (issue #4641).
 *
 * Runs the production buy-mode dispatcher core (`startBuyModeCampaign`,
 * `dispatchBuyModeJob`, `settleBuyModeResult` from
 * `../src/buy-mode-dispatcher`) against the production D1 database through
 * `wrangler d1 execute --remote`, publishes the signed kind-5050 job request
 * to the live scoped market relay, waits for the provider's kind-6050 result,
 * pays the provider's BOLT 11 invoice from the operator-approved payer
 * wallet, confirms provider-side settlement by receiver balance movement,
 * and records the settled receipt row that the deployed
 * `GET /api/public/nip90-market/receipts/{receiptRef}` route serves.
 *
 * This is an operator tool: it requires an explicit operator approval ref,
 * spend caps, and locally funded MDK wallet homes. It never prints raw
 * invoices, mnemonics, preimages, payment hashes, tokens, or wallet paths
 * in its public-safe summary.
 *
 * Usage:
 *   bun scripts/nip90-compute-live-buy-smoke.ts campaign
 *   bun scripts/nip90-compute-live-buy-smoke.ts job
 *
 * Required env (campaign):
 *   NIP90_SMOKE_APPROVAL_REF        operator approval ref
 *   NIP90_SMOKE_CAMPAIGN_ID         campaign id (stable across jobs)
 *   NIP90_SMOKE_PER_JOB_CAP_MSATS   per-job cap
 *   NIP90_SMOKE_DAILY_CAP_MSATS     daily cap
 *
 * Required env (job):
 *   NIP90_SMOKE_CAMPAIGN_ID         campaign id from `campaign`
 *   NIP90_SMOKE_AMOUNT_MSATS        job amount (must match provider price)
 *   NIP90_SMOKE_PROVIDER_PUBKEY     provider nostr pubkey (hex)
 *   NIP90_SMOKE_PROMPT              job prompt text
 *   NIP90_SMOKE_PAYER_WALLET_HOME   funded payer MDK wallet home dir
 *   NIP90_SMOKE_PAYER_WALLET_PORT   payer MDK daemon port
 *   NIP90_SMOKE_PROVIDER_WALLET_HOME provider MDK wallet home dir
 *   NIP90_SMOKE_PROVIDER_WALLET_PORT provider MDK daemon port
 */

import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

import { finalizeEvent, generateSecretKey } from 'nostr-effect/pure'

import {
  DefaultBuyModeRelayUrl,
  type BuyModeCampaignRecord,
  type BuyModeDispatcherStore,
  type BuyModeJobRecord,
  type BuyModePaymentBridge,
  type BuyModeRelayPublisher,
  dispatchBuyModeJob,
  publicDigestRef,
  settleBuyModeResult,
  sha256Hex,
  startBuyModeCampaign,
} from '../src/buy-mode-dispatcher'

const workerDir = join(dirname(fileURLToPath(import.meta.url)), '..')
const d1Name = Bun.env.NIP90_SMOKE_D1_NAME ?? 'openagents-autopilot'
const relayUrl = Bun.env.NIP90_SMOKE_RELAY_URL ?? DefaultBuyModeRelayUrl

const unsafeSummaryPattern =
  /(lnbc|lntb|lnbcrt|lno1|mnemonic|preimage|payment[_-]?hash|Bearer\s+|oa_agent_|\/Users\/|\/home\/)/i

const requiredEnv = (name: string): string => {
  const value = Bun.env[name]?.trim()
  if (value === undefined || value === '') {
    throw new Error(`${name} is required`)
  }
  return value
}

const requiredIntEnv = (name: string): number => {
  const value = Number(requiredEnv(name))
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${name} must be a positive integer`)
  }
  return value
}

const sqlString = (value: string | null): string => {
  if (value === null) return 'NULL'
  if (value.includes('\u0000')) throw new Error('NUL byte in SQL string value')
  return `'${value.replaceAll("'", "''")}'`
}

const sqlNumber = (value: number): string => {
  if (!Number.isFinite(value)) throw new Error('non-finite SQL number value')
  return String(value)
}

const runRemoteD1 = async (
  sql: string,
): Promise<ReadonlyArray<Record<string, unknown>>> => {
  const proc = Bun.spawn(
    [
      'bunx',
      'wrangler',
      'd1',
      'execute',
      d1Name,
      '--remote',
      '--json',
      '--command',
      sql,
    ],
    { cwd: workerDir, stdout: 'pipe', stderr: 'pipe' },
  )
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (exitCode !== 0) {
    throw new Error(`wrangler d1 execute failed: ${stderr.slice(0, 400)}`)
  }
  const parsed = JSON.parse(stdout) as ReadonlyArray<{
    results?: ReadonlyArray<Record<string, unknown>>
    success?: boolean
  }>
  for (const statement of parsed) {
    if (statement.success !== true) {
      throw new Error('remote D1 statement reported failure')
    }
  }
  return parsed[0]?.results ?? []
}

const rowToCampaign = (row: Record<string, unknown>): BuyModeCampaignRecord => ({
  campaignId: String(row.campaign_id),
  createdAt: String(row.created_at),
  dailyCapMsats: Number(row.daily_cap_msats),
  dayKey: String(row.day_key),
  idempotencyKeyHash: String(row.idempotency_key_hash),
  lastAlertRef: row.last_alert_ref === null ? null : String(row.last_alert_ref),
  operatorUserId: String(row.operator_user_id),
  perJobCapMsats: Number(row.per_job_cap_msats),
  relayUrl: String(row.relay_url),
  spendEnabled: Number(row.spend_enabled) === 1,
  spentTodayMsats: Number(row.spent_today_msats),
  state: String(row.state) as BuyModeCampaignRecord['state'],
  updatedAt: String(row.updated_at),
})

const rowToJob = (row: Record<string, unknown>): BuyModeJobRecord => ({
  amountMsats: Number(row.amount_msats),
  bolt11Ref: row.bolt11_ref === null ? null : String(row.bolt11_ref),
  campaignId: String(row.campaign_id),
  contentDigestRef:
    row.content_digest_ref === null ? null : String(row.content_digest_ref),
  createdAt: String(row.created_at),
  idempotencyKeyHash: String(row.idempotency_key_hash),
  jobId: String(row.job_id),
  providerPubkey:
    row.provider_pubkey === null ? null : String(row.provider_pubkey),
  receiptRef: row.receipt_ref === null ? null : String(row.receipt_ref),
  requestEventId: String(row.request_event_id),
  resultEventId:
    row.result_event_id === null ? null : String(row.result_event_id),
  state: String(row.state) as BuyModeJobRecord['state'],
  updatedAt: String(row.updated_at),
})

const makeRemoteD1Store = (): BuyModeDispatcherStore => ({
  latestCampaign: async () => {
    const rows = await runRemoteD1(
      'SELECT * FROM buy_mode_campaigns ORDER BY updated_at DESC LIMIT 1',
    )
    return rows[0] === undefined ? null : rowToCampaign(rows[0])
  },
  readCampaign: async campaignId => {
    const rows = await runRemoteD1(
      `SELECT * FROM buy_mode_campaigns WHERE campaign_id = ${sqlString(campaignId)}`,
    )
    return rows[0] === undefined ? null : rowToCampaign(rows[0])
  },
  readJobByIdempotencyKeyHash: async idempotencyKeyHash => {
    const rows = await runRemoteD1(
      `SELECT * FROM buy_mode_jobs WHERE idempotency_key_hash = ${sqlString(idempotencyKeyHash)}`,
    )
    return rows[0] === undefined ? null : rowToJob(rows[0])
  },
  readJobByRequestEventId: async requestEventId => {
    const rows = await runRemoteD1(
      `SELECT * FROM buy_mode_jobs WHERE request_event_id = ${sqlString(requestEventId)}`,
    )
    return rows[0] === undefined ? null : rowToJob(rows[0])
  },
  readSettlementByResultEventId: async resultEventId => {
    const rows = await runRemoteD1(
      `SELECT * FROM buy_mode_jobs WHERE result_event_id = ${sqlString(resultEventId)}`,
    )
    return rows[0] === undefined ? null : rowToJob(rows[0])
  },
  recordAlertAndHalt: async (campaign, alert) => {
    await runRemoteD1(
      `UPDATE buy_mode_campaigns SET state = 'halted', last_alert_ref = ${sqlString(alert.reasonRef)}, updated_at = ${sqlString(alert.createdAt)} WHERE campaign_id = ${sqlString(campaign.campaignId)};
       INSERT INTO buy_mode_alerts (alert_id, campaign_id, reason_ref, created_at) VALUES (${sqlString(alert.alertId)}, ${sqlString(alert.campaignId)}, ${sqlString(alert.reasonRef)}, ${sqlString(alert.createdAt)});`,
    )
  },
  recordDispatch: async (campaign, job) => {
    await runRemoteD1(
      `UPDATE buy_mode_campaigns SET updated_at = ${sqlString(job.updatedAt)} WHERE campaign_id = ${sqlString(campaign.campaignId)};
       INSERT INTO buy_mode_jobs (job_id, campaign_id, idempotency_key_hash, request_event_id, result_event_id, provider_pubkey, amount_msats, state, receipt_ref, bolt11_ref, content_digest_ref, created_at, updated_at)
       VALUES (${sqlString(job.jobId)}, ${sqlString(job.campaignId)}, ${sqlString(job.idempotencyKeyHash)}, ${sqlString(job.requestEventId)}, ${sqlString(job.resultEventId)}, ${sqlString(job.providerPubkey)}, ${sqlNumber(job.amountMsats)}, ${sqlString(job.state)}, ${sqlString(job.receiptRef)}, ${sqlString(job.bolt11Ref)}, ${sqlString(job.contentDigestRef)}, ${sqlString(job.createdAt)}, ${sqlString(job.updatedAt)});`,
    )
  },
  recordSettlement: async (campaign, job) => {
    await runRemoteD1(
      `UPDATE buy_mode_campaigns SET spent_today_msats = ${sqlNumber(campaign.spentTodayMsats)}, updated_at = ${sqlString(campaign.updatedAt)} WHERE campaign_id = ${sqlString(campaign.campaignId)};
       UPDATE buy_mode_jobs SET result_event_id = ${sqlString(job.resultEventId)}, provider_pubkey = ${sqlString(job.providerPubkey)}, amount_msats = ${sqlNumber(job.amountMsats)}, state = ${sqlString(job.state)}, receipt_ref = ${sqlString(job.receiptRef)}, bolt11_ref = ${sqlString(job.bolt11Ref)}, content_digest_ref = ${sqlString(job.contentDigestRef)}, updated_at = ${sqlString(job.updatedAt)} WHERE job_id = ${sqlString(job.jobId)};`,
    )
  },
  startCampaign: async campaign => {
    await runRemoteD1(
      `INSERT INTO buy_mode_campaigns (campaign_id, idempotency_key_hash, state, spend_enabled, per_job_cap_msats, daily_cap_msats, spent_today_msats, day_key, operator_user_id, relay_url, last_alert_ref, created_at, updated_at)
       VALUES (${sqlString(campaign.campaignId)}, ${sqlString(campaign.idempotencyKeyHash)}, ${sqlString(campaign.state)}, ${campaign.spendEnabled ? 1 : 0}, ${sqlNumber(campaign.perJobCapMsats)}, ${sqlNumber(campaign.dailyCapMsats)}, ${sqlNumber(campaign.spentTodayMsats)}, ${sqlString(campaign.dayKey)}, ${sqlString(campaign.operatorUserId)}, ${sqlString(campaign.relayUrl)}, ${sqlString(campaign.lastAlertRef)}, ${sqlString(campaign.createdAt)}, ${sqlString(campaign.updatedAt)});`,
    )
  },
  stopCampaign: async (campaign, stoppedAt) => {
    await runRemoteD1(
      `UPDATE buy_mode_campaigns SET state = 'disabled', updated_at = ${sqlString(stoppedAt)} WHERE campaign_id = ${sqlString(campaign.campaignId)}`,
    )
  },
})

type RelayMessage = ReadonlyArray<unknown>

const openRelaySocket = (url: string): Promise<WebSocket> =>
  new Promise((resolve, reject) => {
    const ws = new WebSocket(url)
    const timer = setTimeout(
      () => reject(new Error('relay socket open timed out')),
      15_000,
    )
    ws.onopen = () => {
      clearTimeout(timer)
      resolve(ws)
    }
    ws.onerror = () => {
      clearTimeout(timer)
      reject(new Error('relay socket failed to open'))
    }
  })

const makeLiveRelayPublisher = (
  buyerSecretKey: Uint8Array,
): BuyModeRelayPublisher => ({
  publishJobRequest: async input => {
    const template = input.requestEvent as {
      content: string
      created_at: number
      kind: number
      tags: ReadonlyArray<ReadonlyArray<string>>
    }
    const signed = finalizeEvent(
      {
        content: template.content,
        created_at: template.created_at,
        kind: template.kind,
        tags: template.tags.map(tag => [...tag]),
      },
      buyerSecretKey,
    )
    const ws = await openRelaySocket(input.relayUrl)
    const accepted = await new Promise<boolean>((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error('relay publish timed out')),
        20_000,
      )
      ws.onmessage = message => {
        const parsed = JSON.parse(String(message.data)) as RelayMessage
        if (parsed[0] === 'OK' && parsed[1] === signed.id) {
          clearTimeout(timer)
          resolve(parsed[2] === true)
        }
      }
      ws.send(JSON.stringify(['EVENT', signed]))
    })
    ws.close()
    return {
      accepted,
      relayRef: `relay.public.market.${await sha256Hex(input.relayUrl)}`.slice(
        0,
        64,
      ),
      requestEventId: signed.id,
    }
  },
})

type ProviderResultEvent = Readonly<{
  amountMsats: number
  bolt11: string
  content: string
  feedbackStatuses: ReadonlyArray<string>
  resultEventId: string
}>

const waitForProviderResult = async (
  requestEventId: string,
  timeoutMs: number,
): Promise<ProviderResultEvent> => {
  const ws = await openRelaySocket(relayUrl)
  const feedbackStatuses: string[] = []
  try {
    return await new Promise<ProviderResultEvent>((resolve, reject) => {
      const timer = setTimeout(
        () =>
          reject(
            new Error(
              `provider result timed out (feedback: ${feedbackStatuses.join(',') || 'none'})`,
            ),
          ),
        timeoutMs,
      )
      ws.onmessage = message => {
        const parsed = JSON.parse(String(message.data)) as RelayMessage
        if (parsed[0] !== 'EVENT' || parsed[1] !== 'buy-smoke') return
        const event = parsed[2] as {
          content: string
          id: string
          kind: number
          tags: ReadonlyArray<ReadonlyArray<string>>
        }
        if (event.kind === 7000) {
          const status = event.tags.find(tag => tag[0] === 'status')?.[1]
          if (status !== undefined) feedbackStatuses.push(status)
          return
        }
        if (event.kind !== 6050) return
        const amountTag = event.tags.find(tag => tag[0] === 'amount')
        const amountMsats = Number(amountTag?.[1])
        const bolt11 = amountTag?.[2]
        if (
          !Number.isInteger(amountMsats) ||
          amountMsats <= 0 ||
          bolt11 === undefined ||
          !bolt11.startsWith('lnbc')
        ) {
          clearTimeout(timer)
          reject(new Error('result event missing valid amount/bolt11 tag'))
          return
        }
        clearTimeout(timer)
        resolve({
          amountMsats,
          bolt11,
          content: event.content,
          feedbackStatuses,
          resultEventId: event.id,
        })
      }
      ws.send(
        JSON.stringify([
          'REQ',
          'buy-smoke',
          { '#e': [requestEventId], kinds: [6050, 7000] },
        ]),
      )
    })
  } finally {
    ws.close()
  }
}

type WalletTarget = Readonly<{ home: string; port: string }>

const runWalletCommand = async (
  target: WalletTarget,
  args: ReadonlyArray<string>,
  timeoutMs: number,
): Promise<Readonly<{ exitCode: number; stdout: string }>> => {
  const proc = Bun.spawn(
    ['npx', '--yes', '@moneydevkit/agent-wallet@latest', ...args],
    {
      env: {
        ...Bun.env,
        HOME: target.home,
        MDK_WALLET_PORT: target.port,
      },
      stderr: 'pipe',
      stdout: 'pipe',
    },
  )
  const timer = setTimeout(() => proc.kill(), timeoutMs)
  const [stdout, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    proc.exited,
  ])
  clearTimeout(timer)
  return { exitCode, stdout }
}

const walletBalanceSats = async (target: WalletTarget): Promise<number> => {
  const result = await runWalletCommand(target, ['balance'], 60_000)
  if (result.exitCode !== 0) throw new Error('wallet balance command failed')
  const parsed = JSON.parse(result.stdout) as { balance_sats?: number }
  if (!Number.isInteger(parsed.balance_sats)) {
    throw new Error('wallet balance output missing balance_sats')
  }
  return parsed.balance_sats as number
}

type SettlementObservation = {
  classification: string
  payerSpentSats: number
  providerReceivedSats: number
}

const makeLivePaymentBridge = (
  payer: WalletTarget,
  provider: WalletTarget,
  observation: SettlementObservation,
): BuyModePaymentBridge => ({
  payBolt11: async input => {
    const providerBefore = await walletBalanceSats(provider)
    const payerBefore = await walletBalanceSats(payer)
    const send = await runWalletCommand(
      payer,
      ['send', input.bolt11],
      180_000,
    )
    const sendSucceeded = send.exitCode === 0
    let providerAfter = providerBefore
    const deadline = Date.now() + 240_000
    while (Date.now() < deadline) {
      providerAfter = await walletBalanceSats(provider)
      if (providerAfter > providerBefore) break
      await new Promise(resolve => setTimeout(resolve, 5_000))
    }
    if (providerAfter <= providerBefore) {
      throw new Error(
        `provider wallet did not confirm settlement (send exit ${send.exitCode})`,
      )
    }
    const payerAfter = await walletBalanceSats(payer)
    observation.classification = sendSucceeded
      ? 'first_attempt_settled'
      : 'fail_then_pass_settled'
    observation.payerSpentSats = payerBefore - payerAfter
    observation.providerReceivedSats = providerAfter - providerBefore
    const settlementDigest = await publicDigestRef(
      'settlement.nip90_market.compute',
      input.idempotencyRef,
    )
    return {
      receiptRef: await publicDigestRef(
        'receipt.nip90_market.compute',
        input.resultEventId,
      ),
      settlementRef: settlementDigest,
    }
  },
})

const printSummary = (summary: Record<string, unknown>): void => {
  const serialized = JSON.stringify(summary, null, 2)
  if (unsafeSummaryPattern.test(serialized)) {
    throw new Error('summary contains private payment or wallet material')
  }
  process.stdout.write(`${serialized}\n`)
}

const runCampaign = async (): Promise<void> => {
  const approvalRef = requiredEnv('NIP90_SMOKE_APPROVAL_REF')
  const campaignId = requiredEnv('NIP90_SMOKE_CAMPAIGN_ID')
  const store = makeRemoteD1Store()
  const nowIso = new Date().toISOString()
  const result = await startBuyModeCampaign(store, {
    campaignId,
    dailyCapMsats: requiredIntEnv('NIP90_SMOKE_DAILY_CAP_MSATS'),
    idempotencyKeyHash: await sha256Hex(`nip90-buy-smoke-campaign-${campaignId}`),
    nowIso,
    operatorUserId: `operator:${approvalRef}`,
    perJobCapMsats: requiredIntEnv('NIP90_SMOKE_PER_JOB_CAP_MSATS'),
    relayUrl,
    spendEnabled: true,
  })
  printSummary({
    approvalRef,
    kind: result.kind,
    campaignId,
    reasonRef: result.kind === 'blocked' ? result.reasonRef : null,
    schema: 'openagents.nip90_compute_live_buy_smoke.campaign.v1',
  })
  process.exitCode = result.kind === 'started' ? 0 : 2
}

const runJob = async (): Promise<void> => {
  const campaignId = requiredEnv('NIP90_SMOKE_CAMPAIGN_ID')
  const amountMsats = requiredIntEnv('NIP90_SMOKE_AMOUNT_MSATS')
  const providerPubkey = requiredEnv('NIP90_SMOKE_PROVIDER_PUBKEY')
  const prompt = requiredEnv('NIP90_SMOKE_PROMPT')
  const payer: WalletTarget = {
    home: requiredEnv('NIP90_SMOKE_PAYER_WALLET_HOME'),
    port: requiredEnv('NIP90_SMOKE_PAYER_WALLET_PORT'),
  }
  const provider: WalletTarget = {
    home: requiredEnv('NIP90_SMOKE_PROVIDER_WALLET_HOME'),
    port: requiredEnv('NIP90_SMOKE_PROVIDER_WALLET_PORT'),
  }
  const store = makeRemoteD1Store()
  const jobId = `buy_mode_job_${crypto.randomUUID()}`
  const buyerSecretKey = generateSecretKey()

  const dispatch = await dispatchBuyModeJob(
    store,
    makeLiveRelayPublisher(buyerSecretKey),
    {
      amountMsats,
      campaignId,
      content: prompt,
      idempotencyKeyHash: await sha256Hex(`nip90-buy-smoke-${jobId}`),
      jobId,
      nowIso: new Date().toISOString(),
      providerPubkeys: [providerPubkey],
    },
  )
  if (dispatch.kind !== 'dispatched') {
    printSummary({
      kind: dispatch.kind,
      reasonRef: dispatch.kind === 'blocked' ? dispatch.reasonRef : null,
      schema: 'openagents.nip90_compute_live_buy_smoke.job.v1',
      stage: 'dispatch',
    })
    process.exitCode = 2
    return
  }

  const requestEventId = dispatch.relayReceipt.requestEventId
  process.stderr.write(`[buy-smoke] dispatched request ${requestEventId}\n`)
  const result = await waitForProviderResult(requestEventId, 300_000)
  process.stderr.write(
    `[buy-smoke] result ${result.resultEventId} feedback=${result.feedbackStatuses.join(',')}\n`,
  )
  if (result.amountMsats !== amountMsats) {
    throw new Error(
      `provider quoted ${result.amountMsats} msats, dispatch amount was ${amountMsats}`,
    )
  }

  const observation: SettlementObservation = {
    classification: 'unsettled',
    payerSpentSats: 0,
    providerReceivedSats: 0,
  }
  const settle = await settleBuyModeResult(
    store,
    makeLivePaymentBridge(payer, provider, observation),
    {
      amountMsats,
      bolt11: result.bolt11,
      content: result.content,
      idempotencyKeyHash: await sha256Hex(
        `nip90-buy-smoke-settle-${result.resultEventId}`,
      ),
      nowIso: new Date().toISOString(),
      providerPubkey,
      requestEventId,
      resultEventId: result.resultEventId,
    },
  )
  printSummary({
    amountMsats,
    campaignId,
    feedbackStatuses: result.feedbackStatuses,
    jobId,
    kind: settle.kind,
    reasonRef: settle.kind === 'blocked' ? settle.reasonRef : null,
    receiptRef: settle.kind === 'settled' ? settle.job.receiptRef : null,
    requestEventId,
    resultEventId: result.resultEventId,
    schema: 'openagents.nip90_compute_live_buy_smoke.job.v1',
    settlementClassification: observation.classification,
    settlementMovement: {
      payerSpentSats: observation.payerSpentSats,
      providerReceivedSats: observation.providerReceivedSats,
    },
  })
  process.exitCode = settle.kind === 'settled' ? 0 : 2
}

const command = Bun.argv[2]

if (command === 'campaign') {
  await runCampaign()
} else if (command === 'job') {
  await runJob()
} else {
  process.stderr.write(
    'usage: bun scripts/nip90-compute-live-buy-smoke.ts <campaign|job>\n',
  )
  process.exitCode = 1
}
