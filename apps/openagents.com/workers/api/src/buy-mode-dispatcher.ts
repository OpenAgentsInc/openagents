import {
  KIND_JOB_TEXT_GENERATION,
  createJobRequestEvent,
  jobInput,
  makeJobRequest,
} from '@openagentsinc/nip90'

export type BuyModeDispatcherState = 'disabled' | 'enabled' | 'halted'
export type BuyModeJobState =
  | 'issued'
  | 'settled'
  | 'settlement_blocked'
  | 'settlement_failed'

export type BuyModeCampaignRecord = Readonly<{
  campaignId: string
  createdAt: string
  dailyCapMsats: number
  dayKey: string
  idempotencyKeyHash: string
  lastAlertRef: string | null
  operatorUserId: string
  perJobCapMsats: number
  relayUrl: string
  spendEnabled: boolean
  spentTodayMsats: number
  state: BuyModeDispatcherState
  updatedAt: string
}>

export type BuyModeJobRecord = Readonly<{
  amountMsats: number
  bolt11Ref: string | null
  campaignId: string
  contentDigestRef: string | null
  createdAt: string
  idempotencyKeyHash: string
  jobId: string
  providerPubkey: string | null
  receiptRef: string | null
  requestEventId: string
  resultEventId: string | null
  state: BuyModeJobState
  updatedAt: string
}>

export type BuyModeAlertRecord = Readonly<{
  alertId: string
  campaignId: string
  createdAt: string
  reasonRef: string
}>

export type BuyModeDispatcherStore = Readonly<{
  latestCampaign: () => Promise<BuyModeCampaignRecord | null>
  readCampaign: (campaignId: string) => Promise<BuyModeCampaignRecord | null>
  readJobByIdempotencyKeyHash: (
    idempotencyKeyHash: string,
  ) => Promise<BuyModeJobRecord | null>
  readJobByRequestEventId: (
    requestEventId: string,
  ) => Promise<BuyModeJobRecord | null>
  readSettlementByResultEventId: (
    resultEventId: string,
  ) => Promise<BuyModeJobRecord | null>
  recordAlertAndHalt: (
    campaign: BuyModeCampaignRecord,
    alert: BuyModeAlertRecord,
  ) => Promise<void>
  recordDispatch: (
    campaign: BuyModeCampaignRecord,
    job: BuyModeJobRecord,
  ) => Promise<void>
  recordSettlement: (
    campaign: BuyModeCampaignRecord,
    job: BuyModeJobRecord,
  ) => Promise<void>
  startCampaign: (campaign: BuyModeCampaignRecord) => Promise<void>
  stopCampaign: (
    campaign: BuyModeCampaignRecord,
    stoppedAt: string,
  ) => Promise<void>
}>

export type BuyModeRelayPublisher = Readonly<{
  publishJobRequest: (
    input: BuyModeRelayJobRequest,
  ) => Promise<BuyModeRelayPublishReceipt>
}>

export type BuyModeRelayJobRequest = Readonly<{
  campaignId: string
  content: string
  providerPubkeys: ReadonlyArray<string>
  requestEvent: unknown
  relayUrl: string
}>

export type BuyModeRelayPublishReceipt = Readonly<{
  accepted: boolean
  relayRef: string
  requestEventId: string
}>

export type BuyModePaymentBridge = Readonly<{
  payBolt11: (
    input: BuyModePaymentBridgeRequest,
  ) => Promise<BuyModePaymentBridgeReceipt>
}>

export type BuyModePaymentBridgeRequest = Readonly<{
  amountMsats: number
  bolt11: string
  idempotencyRef: string
  providerPubkey: string
  resultEventId: string
}>

export type BuyModePaymentBridgeReceipt = Readonly<{
  receiptRef: string
  settlementRef: string
}>

export type BuyModeStartInput = Readonly<{
  campaignId: string
  dailyCapMsats: number
  idempotencyKeyHash: string
  nowIso: string
  operatorUserId: string
  perJobCapMsats: number
  relayUrl: string
  spendEnabled: boolean
}>

export type BuyModeDispatchInput = Readonly<{
  amountMsats: number
  campaignId?: string | undefined
  content: string
  idempotencyKeyHash: string
  jobId: string
  nowIso: string
  providerPubkeys: ReadonlyArray<string>
}>

export type BuyModeSettleInput = Readonly<{
  amountMsats: number
  bolt11: string
  content: string
  idempotencyKeyHash: string
  nowIso: string
  providerPubkey: string
  requestEventId: string
  resultEventId: string
}>

export type BuyModeDispatcherResult =
  | Readonly<{ campaign: BuyModeCampaignRecord; kind: 'started' }>
  | Readonly<{ campaign: BuyModeCampaignRecord; kind: 'stopped' }>
  | Readonly<{
      alert: BuyModeAlertRecord
      campaign: BuyModeCampaignRecord
      kind: 'halted'
    }>
  | Readonly<{
      campaign: BuyModeCampaignRecord
      job: BuyModeJobRecord
      kind: 'dispatched'
      relayReceipt: BuyModeRelayPublishReceipt
    }>
  | Readonly<{
      campaign: BuyModeCampaignRecord
      job: BuyModeJobRecord
      kind: 'settled'
      paymentReceipt: BuyModePaymentBridgeReceipt
    }>
  | Readonly<{ job: BuyModeJobRecord; kind: 'idempotent_replay' }>
  | Readonly<{ kind: 'blocked'; reasonRef: string }>

const textEncoder = new TextEncoder()
const unsafePublicMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|bearer\s+|cookie|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(invoice|payment|payload|prompt|runner|state)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

export const DefaultBuyModeRelayUrl =
  'wss://relay.openagents.com'

export const buyModeDayKey = (iso: string): string => iso.slice(0, 10)

export const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest('SHA-256', textEncoder.encode(value))

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

export const publicDigestRef = async (
  prefix: string,
  value: string,
): Promise<string> => `${prefix}.${(await sha256Hex(value)).slice(0, 32)}`

export const isPublicSafeBuyModeProjection = (value: unknown): boolean =>
  !unsafePublicMaterialPattern.test(JSON.stringify(value))

const positiveIntegerBlocker = (label: string, value: number): string | null => {
  if (!Number.isInteger(value) || value <= 0) {
    return `blocker.buy_mode.invalid_${label}`
  }

  return null
}

const publicSafetyBlocker = (label: string, value: unknown): string | null =>
  isPublicSafeBuyModeProjection(value)
    ? null
    : `blocker.buy_mode.public_projection_${label}_unsafe`

const jobRequestTemplate = (
  input: Readonly<{
    amountMsats: number
    content: string
    providerPubkeys: ReadonlyArray<string>
  }>,
): unknown =>
  createJobRequestEvent(
    makeJobRequest({
      bid: input.amountMsats,
      inputs: [jobInput.text(input.content)],
      kind: KIND_JOB_TEXT_GENERATION,
      output: 'text/plain',
      serviceProviders: [...input.providerPubkeys],
    }),
  )

const haltedCampaign = (
  campaign: BuyModeCampaignRecord,
  alert: BuyModeAlertRecord,
): BuyModeCampaignRecord => ({
  ...campaign,
  lastAlertRef: alert.reasonRef,
  state: 'halted',
  updatedAt: alert.createdAt,
})

export const startBuyModeCampaign = async (
  store: BuyModeDispatcherStore,
  input: BuyModeStartInput,
): Promise<BuyModeDispatcherResult> => {
  const perJobCapBlocker = positiveIntegerBlocker(
    'per_job_cap_msats',
    input.perJobCapMsats,
  )

  if (perJobCapBlocker !== null) {
    return { kind: 'blocked', reasonRef: perJobCapBlocker }
  }

  const dailyCapBlocker = positiveIntegerBlocker(
    'daily_cap_msats',
    input.dailyCapMsats,
  )

  if (dailyCapBlocker !== null) {
    return { kind: 'blocked', reasonRef: dailyCapBlocker }
  }

  if (input.perJobCapMsats > input.dailyCapMsats) {
    return {
      kind: 'blocked',
      reasonRef: 'blocker.buy_mode.per_job_cap_exceeds_daily_cap',
    }
  }

  const campaign: BuyModeCampaignRecord = {
    campaignId: input.campaignId,
    createdAt: input.nowIso,
    dailyCapMsats: input.dailyCapMsats,
    dayKey: buyModeDayKey(input.nowIso),
    idempotencyKeyHash: input.idempotencyKeyHash,
    lastAlertRef: null,
    operatorUserId: input.operatorUserId,
    perJobCapMsats: input.perJobCapMsats,
    relayUrl: input.relayUrl,
    spendEnabled: input.spendEnabled,
    spentTodayMsats: 0,
    state: 'enabled',
    updatedAt: input.nowIso,
  }
  const publicSafetyReasonRef = publicSafetyBlocker('campaign', campaign)

  if (publicSafetyReasonRef !== null) {
    return { kind: 'blocked', reasonRef: publicSafetyReasonRef }
  }

  await store.startCampaign(campaign)

  return { campaign, kind: 'started' }
}

export const stopBuyModeCampaign = async (
  store: BuyModeDispatcherStore,
  nowIso: string,
): Promise<BuyModeDispatcherResult> => {
  const campaign = await store.latestCampaign()

  if (campaign === null) {
    return { kind: 'blocked', reasonRef: 'blocker.buy_mode.no_campaign' }
  }

  const stopped = { ...campaign, state: 'disabled' as const, updatedAt: nowIso }
  await store.stopCampaign(stopped, nowIso)

  return { campaign: stopped, kind: 'stopped' }
}

export const dispatchBuyModeJob = async (
  store: BuyModeDispatcherStore,
  relay: BuyModeRelayPublisher,
  input: BuyModeDispatchInput,
): Promise<BuyModeDispatcherResult> => {
  const existing = await store.readJobByIdempotencyKeyHash(input.idempotencyKeyHash)

  if (existing !== null) {
    return { job: existing, kind: 'idempotent_replay' }
  }

  const campaign = input.campaignId === undefined
    ? await store.latestCampaign()
    : await store.readCampaign(input.campaignId)

  if (campaign === null) {
    return { kind: 'blocked', reasonRef: 'blocker.buy_mode.no_campaign' }
  }

  if (campaign.state !== 'enabled') {
    return { kind: 'blocked', reasonRef: `blocker.buy_mode.${campaign.state}` }
  }

  const amountBlocker = positiveIntegerBlocker('amount_msats', input.amountMsats)

  if (amountBlocker !== null) {
    return { kind: 'blocked', reasonRef: amountBlocker }
  }

  if (input.amountMsats > campaign.perJobCapMsats) {
    const alert: BuyModeAlertRecord = {
      alertId: `buy_mode_alert_${input.jobId}`,
      campaignId: campaign.campaignId,
      createdAt: input.nowIso,
      reasonRef: 'alert.buy_mode.per_job_cap_breach',
    }
    await store.recordAlertAndHalt(campaign, alert)

    return { alert, campaign: haltedCampaign(campaign, alert), kind: 'halted' }
  }

  if (campaign.spentTodayMsats + input.amountMsats > campaign.dailyCapMsats) {
    const alert: BuyModeAlertRecord = {
      alertId: `buy_mode_alert_${input.jobId}`,
      campaignId: campaign.campaignId,
      createdAt: input.nowIso,
      reasonRef: 'alert.buy_mode.daily_cap_breach',
    }
    await store.recordAlertAndHalt(campaign, alert)

    return { alert, campaign: haltedCampaign(campaign, alert), kind: 'halted' }
  }

  const requestEvent = jobRequestTemplate(input)
  const relayReceipt = await relay.publishJobRequest({
    campaignId: campaign.campaignId,
    content: input.content,
    providerPubkeys: input.providerPubkeys,
    relayUrl: campaign.relayUrl,
    requestEvent,
  })

  if (!relayReceipt.accepted) {
    const relayReason = relayReceipt.relayRef.match(
      /^relay\.public\.([a-z0-9_]+)\./,
    )?.[1]

    return {
      kind: 'blocked',
      reasonRef: relayReason === undefined
        ? 'blocker.buy_mode.relay_rejected'
        : `blocker.buy_mode.${relayReason}`,
    }
  }

  const job: BuyModeJobRecord = {
    amountMsats: input.amountMsats,
    bolt11Ref: null,
    campaignId: campaign.campaignId,
    contentDigestRef: await publicDigestRef('digest.buy_mode.prompt', input.content),
    createdAt: input.nowIso,
    idempotencyKeyHash: input.idempotencyKeyHash,
    jobId: input.jobId,
    providerPubkey: input.providerPubkeys[0] ?? null,
    receiptRef: null,
    requestEventId: relayReceipt.requestEventId,
    resultEventId: null,
    state: 'issued',
    updatedAt: input.nowIso,
  }
  const publicSafetyReasonRef = publicSafetyBlocker('dispatch_job', job)

  if (publicSafetyReasonRef !== null) {
    return { kind: 'blocked', reasonRef: publicSafetyReasonRef }
  }

  await store.recordDispatch(campaign, job)

  return { campaign, job, kind: 'dispatched', relayReceipt }
}

export const settleBuyModeResult = async (
  store: BuyModeDispatcherStore,
  paymentBridge: BuyModePaymentBridge,
  input: BuyModeSettleInput,
): Promise<BuyModeDispatcherResult> => {
  const duplicate = await store.readSettlementByResultEventId(input.resultEventId)

  if (duplicate !== null) {
    return { job: duplicate, kind: 'idempotent_replay' }
  }

  const job = await store.readJobByRequestEventId(input.requestEventId)

  if (job === null) {
    return { kind: 'blocked', reasonRef: 'blocker.buy_mode.unknown_request' }
  }

  const campaign = await store.readCampaign(job.campaignId)

  if (campaign === null || campaign.state !== 'enabled') {
    return { kind: 'blocked', reasonRef: 'blocker.buy_mode.not_enabled' }
  }

  if (!campaign.spendEnabled) {
    const blocked: BuyModeJobRecord = {
      ...job,
      resultEventId: input.resultEventId,
      state: 'settlement_blocked',
      updatedAt: input.nowIso,
    }
    await store.recordSettlement(campaign, blocked)

    return {
      kind: 'blocked',
      reasonRef: 'blocker.buy_mode.operator_spend_approval_missing',
    }
  }

  if (input.amountMsats !== job.amountMsats || input.amountMsats > campaign.perJobCapMsats) {
    const alert: BuyModeAlertRecord = {
      alertId: `buy_mode_alert_${job.jobId}`,
      campaignId: campaign.campaignId,
      createdAt: input.nowIso,
      reasonRef: 'alert.buy_mode.result_amount_cap_breach',
    }
    await store.recordAlertAndHalt(campaign, alert)

    return { alert, campaign: haltedCampaign(campaign, alert), kind: 'halted' }
  }

  if (campaign.spentTodayMsats + input.amountMsats > campaign.dailyCapMsats) {
    const alert: BuyModeAlertRecord = {
      alertId: `buy_mode_alert_${job.jobId}`,
      campaignId: campaign.campaignId,
      createdAt: input.nowIso,
      reasonRef: 'alert.buy_mode.daily_cap_breach',
    }
    await store.recordAlertAndHalt(campaign, alert)

    return { alert, campaign: haltedCampaign(campaign, alert), kind: 'halted' }
  }

  if (!input.bolt11.startsWith('lnbc')) {
    return { kind: 'blocked', reasonRef: 'blocker.buy_mode.invalid_bolt11' }
  }

  if (input.content.trim() === '') {
    return { kind: 'blocked', reasonRef: 'blocker.buy_mode.empty_result' }
  }

  const paymentReceipt = await paymentBridge.payBolt11({
    amountMsats: input.amountMsats,
    bolt11: input.bolt11,
    idempotencyRef: `buy_mode.settle.${input.resultEventId}`,
    providerPubkey: input.providerPubkey,
    resultEventId: input.resultEventId,
  })
  const settled: BuyModeJobRecord = {
    ...job,
    bolt11Ref: await publicDigestRef('bolt11.buy_mode.redacted', input.bolt11),
    contentDigestRef: await publicDigestRef('digest.buy_mode.result', input.content),
    providerPubkey: input.providerPubkey,
    receiptRef: paymentReceipt.receiptRef,
    resultEventId: input.resultEventId,
    state: 'settled',
    updatedAt: input.nowIso,
  }
  const publicSafetyReasonRef = publicSafetyBlocker('settled_job', settled)

  if (publicSafetyReasonRef !== null) {
    return { kind: 'blocked', reasonRef: publicSafetyReasonRef }
  }

  await store.recordSettlement({
    ...campaign,
    spentTodayMsats: campaign.spentTodayMsats + input.amountMsats,
    updatedAt: input.nowIso,
  }, settled)

  return { campaign, job: settled, kind: 'settled', paymentReceipt }
}

export const makeD1BuyModeDispatcherStore = (
  db: D1Database,
): BuyModeDispatcherStore => {
  const rowToCampaign = (row: any): BuyModeCampaignRecord => ({
    campaignId: row.campaign_id,
    createdAt: row.created_at,
    dailyCapMsats: row.daily_cap_msats,
    dayKey: row.day_key,
    idempotencyKeyHash: row.idempotency_key_hash,
    lastAlertRef: row.last_alert_ref,
    operatorUserId: row.operator_user_id,
    perJobCapMsats: row.per_job_cap_msats,
    relayUrl: row.relay_url,
    spendEnabled: row.spend_enabled === 1,
    spentTodayMsats: row.spent_today_msats,
    state: row.state,
    updatedAt: row.updated_at,
  })
  const rowToJob = (row: any): BuyModeJobRecord => ({
    amountMsats: row.amount_msats,
    bolt11Ref: row.bolt11_ref,
    campaignId: row.campaign_id,
    contentDigestRef: row.content_digest_ref,
    createdAt: row.created_at,
    idempotencyKeyHash: row.idempotency_key_hash,
    jobId: row.job_id,
    providerPubkey: row.provider_pubkey,
    receiptRef: row.receipt_ref,
    requestEventId: row.request_event_id,
    resultEventId: row.result_event_id,
    state: row.state,
    updatedAt: row.updated_at,
  })

  return {
    latestCampaign: async () => {
      const row = await db.prepare(
        `SELECT * FROM buy_mode_campaigns ORDER BY updated_at DESC LIMIT 1`,
      ).first<any>()

      return row === null ? null : rowToCampaign(row)
    },
    readCampaign: async campaignId => {
      const row = await db.prepare(
        `SELECT * FROM buy_mode_campaigns WHERE campaign_id = ?`,
      ).bind(campaignId).first<any>()

      return row === null ? null : rowToCampaign(row)
    },
    readJobByIdempotencyKeyHash: async idempotencyKeyHash => {
      const row = await db.prepare(
        `SELECT * FROM buy_mode_jobs WHERE idempotency_key_hash = ?`,
      ).bind(idempotencyKeyHash).first<any>()

      return row === null ? null : rowToJob(row)
    },
    readJobByRequestEventId: async requestEventId => {
      const row = await db.prepare(
        `SELECT * FROM buy_mode_jobs WHERE request_event_id = ?`,
      ).bind(requestEventId).first<any>()

      return row === null ? null : rowToJob(row)
    },
    readSettlementByResultEventId: async resultEventId => {
      const row = await db.prepare(
        `SELECT * FROM buy_mode_jobs WHERE result_event_id = ?`,
      ).bind(resultEventId).first<any>()

      return row === null ? null : rowToJob(row)
    },
    recordAlertAndHalt: async (campaign, alert) => {
      await db.batch([
        db.prepare(
          `UPDATE buy_mode_campaigns
           SET state = 'halted', last_alert_ref = ?, updated_at = ?
           WHERE campaign_id = ?`,
        ).bind(alert.reasonRef, alert.createdAt, campaign.campaignId),
        db.prepare(
          `INSERT INTO buy_mode_alerts (alert_id, campaign_id, reason_ref, created_at)
           VALUES (?, ?, ?, ?)`,
        ).bind(alert.alertId, alert.campaignId, alert.reasonRef, alert.createdAt),
      ])
    },
    recordDispatch: async (campaign, job) => {
      await db.batch([
        db.prepare(
          `UPDATE buy_mode_campaigns
           SET updated_at = ?
           WHERE campaign_id = ?`,
        ).bind(job.updatedAt, campaign.campaignId),
        db.prepare(
          `INSERT INTO buy_mode_jobs
            (job_id, campaign_id, idempotency_key_hash, request_event_id,
             result_event_id, provider_pubkey, amount_msats, state, receipt_ref,
             bolt11_ref, content_digest_ref, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        ).bind(
          job.jobId,
          job.campaignId,
          job.idempotencyKeyHash,
          job.requestEventId,
          job.resultEventId,
          job.providerPubkey,
          job.amountMsats,
          job.state,
          job.receiptRef,
          job.bolt11Ref,
          job.contentDigestRef,
          job.createdAt,
          job.updatedAt,
        ),
      ])
    },
    recordSettlement: async (campaign, job) => {
      await db.batch([
        db.prepare(
          `UPDATE buy_mode_campaigns
           SET spent_today_msats = ?, updated_at = ?
           WHERE campaign_id = ?`,
        ).bind(campaign.spentTodayMsats, campaign.updatedAt, campaign.campaignId),
        db.prepare(
          `UPDATE buy_mode_jobs
           SET result_event_id = ?, provider_pubkey = ?, amount_msats = ?,
               state = ?, receipt_ref = ?, bolt11_ref = ?,
               content_digest_ref = ?, updated_at = ?
           WHERE job_id = ?`,
        ).bind(
          job.resultEventId,
          job.providerPubkey,
          job.amountMsats,
          job.state,
          job.receiptRef,
          job.bolt11Ref,
          job.contentDigestRef,
          job.updatedAt,
          job.jobId,
        ),
      ])
    },
    startCampaign: async campaign => {
      await db.prepare(
        `INSERT INTO buy_mode_campaigns
          (campaign_id, idempotency_key_hash, state, spend_enabled,
           per_job_cap_msats, daily_cap_msats, spent_today_msats, day_key,
           operator_user_id, relay_url, last_alert_ref, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      ).bind(
        campaign.campaignId,
        campaign.idempotencyKeyHash,
        campaign.state,
        campaign.spendEnabled ? 1 : 0,
        campaign.perJobCapMsats,
        campaign.dailyCapMsats,
        campaign.spentTodayMsats,
        campaign.dayKey,
        campaign.operatorUserId,
        campaign.relayUrl,
        campaign.lastAlertRef,
        campaign.createdAt,
        campaign.updatedAt,
      ).run()
    },
    stopCampaign: async (campaign, stoppedAt) => {
      await db.prepare(
        `UPDATE buy_mode_campaigns
         SET state = 'disabled', updated_at = ?
         WHERE campaign_id = ?`,
      ).bind(stoppedAt, campaign.campaignId).run()
    },
  }
}
