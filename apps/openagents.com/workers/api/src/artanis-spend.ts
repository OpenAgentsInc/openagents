import { artanisMindComplete } from './artanis-mind'
import {
  artanisAuthorityDb,
  mirrorArtanisRows,
  type ArtanisDatabase,
} from './artanis-domain-store'
import { parseJsonWithSchema } from './json-boundary'
import { randomUuid } from './runtime-primitives'
import {
  type TreasuryPayoutExecution,
  type TreasuryRouteDependencies,
  executeTreasuryPayout,
} from './treasury-routes'
import { Schema as S } from 'effect'

// Artanis treasury spend under the standing-cap envelope (issue #4703).
// The owner grants a bounded, revocable standing approval (per-payout +
// per-day caps). The MIND decides whether and how much to pay a
// concrete payable item; spends within the envelope auto-satisfy their
// approval requirement WITH the grant ref cited on the decision row and
// the public evidence. Anything outside the caps is recorded as
// blocked_over_cap and waits for explicit approval - never silently
// paid, never silently dropped. Destinations come only from registered
// public-safe sources (the recipient's tip-recipient wallet claim).

const SpendVerdict = S.Struct({
  amountSat: S.Number,
  pay: S.Boolean,
  rationale: S.String,
})

export type StandingGrant = Readonly<{
  grantRef: string
  perPayoutCapSat: number
  perDayCapSat: number
}>

export const readActiveStandingGrant = async (
  db: ArtanisDatabase,
): Promise<StandingGrant | null> => {
  const row = (await artanisAuthorityDb(db)
    .prepare(
      `SELECT grant_ref, per_payout_cap_sat, per_day_cap_sat
         FROM artanis_standing_spend_grants
        WHERE active = 1 AND revoked_at IS NULL
        ORDER BY created_at DESC LIMIT 1`,
    )
    .first()) as
    | { grant_ref: string; per_payout_cap_sat: number; per_day_cap_sat: number }
    | null

  return row === null
    ? null
    : {
        grantRef: row.grant_ref,
        perDayCapSat: row.per_day_cap_sat,
        perPayoutCapSat: row.per_payout_cap_sat,
      }
}

export const spentTodaySat = async (
  db: ArtanisDatabase,
  nowIso: string,
): Promise<number> => {
  const row = (await artanisAuthorityDb(db)
    .prepare(
      `SELECT COALESCE(SUM(paid_amount_sat), 0) AS spent
         FROM artanis_spend_decisions
        WHERE state = 'paid' AND created_at >= ?`,
    )
    .bind(`${nowIso.slice(0, 10)}T00:00:00.000Z`)
    .first()) as { spent: number } | null

  return Number(row?.spent ?? 0)
}

export type SpendCandidate = Readonly<{
  recipientRef: string
  destinationSourceRef: string
  destination: string
  context: string
  suggestedMaxSat: number
}>

export type ArtanisSpendOutcome = Readonly<{
  decided: boolean
  state: 'paid' | 'refused' | 'blocked_over_cap' | 'declined' | 'skipped'
  decisionId: string | null
  paidAmountSat: number | null
  paymentRef: string | null
  grantRef: string | null
  reason: string | null
}>

export const runArtanisSpendDecision = async (
  database: ArtanisDatabase,
  deps: Readonly<{
    geminiApiKey: string | null
    gatewayToken?: string | undefined
    treasury: TreasuryRouteDependencies
    candidate: SpendCandidate
    nowIso: string
  }>,
): Promise<ArtanisSpendOutcome> => {
  // The authoritative D1 handle; decision inserts mirror to Postgres
  // through the KS-8.6 seam (fail-soft). Spend decisions reference the
  // treasury by payment_ref ID only — no cross-store joins (KS-8.8).
  const db = artanisAuthorityDb(database)
  const skipped = (reason: string): ArtanisSpendOutcome => ({
    decided: false,
    decisionId: null,
    grantRef: null,
    paidAmountSat: null,
    paymentRef: null,
    reason,
    state: 'skipped',
  })

  if (deps.geminiApiKey === null || deps.geminiApiKey === '') {
    return skipped('mind_unconfigured')
  }

  const grant = await readActiveStandingGrant(database)
  if (grant === null) {
    return skipped('no_active_standing_grant')
  }

  const alreadySpentSat = await spentTodaySat(database, deps.nowIso)
  const dayBudgetLeftSat = grant.perDayCapSat - alreadySpentSat
  if (dayBudgetLeftSat <= 0) {
    return skipped('per_day_cap_exhausted')
  }

  // The mind decides - amount and whether to pay at all.
  const mindResult = await artanisMindComplete({
    apiKey: deps.geminiApiKey,
    ...(deps.gatewayToken === undefined || deps.gatewayToken === ''
      ? {}
      : { gatewayToken: deps.gatewayToken }),
    prompt: [
      `Payable item under consideration: ${deps.candidate.context}`,
      `Recipient: ${deps.candidate.recipientRef} (registered destination: ${deps.candidate.destinationSourceRef})`,
      `Constraints: per-payout cap ${grant.perPayoutCapSat} sats; remaining today ${dayBudgetLeftSat} sats; suggested maximum for this item ${deps.candidate.suggestedMaxSat} sats.`,
      'Decide whether to pay and how much. Output STRICT JSON only: {"pay":true|false,"amountSat":<integer>,"rationale":"<one sentence>"}',
    ].join('\n'),
    system:
      'You are Artanis, the Nexus administrator, deciding a bounded treasury spend under a standing owner grant. You are conservative, you never exceed the stated caps, and you output strict JSON only.',
  })

  if ('error' in mindResult) {
    return skipped('mind_unavailable')
  }

  let verdict: typeof SpendVerdict.Type | null = null
  try {
    const cleaned = mindResult.text
      .replace(/^```(?:json)?\s*/m, '')
      .replace(/```\s*$/m, '')
      .trim()
    verdict = parseJsonWithSchema(SpendVerdict, cleaned)
  } catch {
    verdict = null
  }

  if (verdict === null) {
    return skipped('schema_invalid_mind_output')
  }

  if (!verdict.pay || verdict.amountSat <= 0) {
    return {
      decided: true,
      decisionId: null,
      grantRef: grant.grantRef,
      paidAmountSat: null,
      paymentRef: null,
      reason: verdict.rationale.slice(0, 200),
      state: 'declined',
    }
  }

  const amountSat = Math.floor(verdict.amountSat)
  const decisionId = randomUuid()

  // The envelope holds regardless of what the mind asked for.
  if (
    amountSat > grant.perPayoutCapSat ||
    amountSat > dayBudgetLeftSat ||
    amountSat > deps.candidate.suggestedMaxSat
  ) {
    await db
      .prepare(
        `INSERT INTO artanis_spend_decisions
         (id, grant_ref, state, intended_amount_sat, destination_source_ref,
          recipient_ref, rationale, created_at, updated_at)
         VALUES (?, ?, 'blocked_over_cap', ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        decisionId,
        grant.grantRef,
        amountSat,
        deps.candidate.destinationSourceRef,
        deps.candidate.recipientRef,
        verdict.rationale.slice(0, 300),
        deps.nowIso,
        deps.nowIso,
      )
      .run()
    await mirrorArtanisRows(database, 'artanis_spend_decisions', 'id', [
      decisionId,
    ])
    return {
      decided: true,
      decisionId,
      grantRef: grant.grantRef,
      paidAmountSat: null,
      paymentRef: null,
      reason: 'blocked_over_cap',
      state: 'blocked_over_cap',
    }
  }

  const execution: TreasuryPayoutExecution = await executeTreasuryPayout(
    deps.treasury,
    {
      amountSat,
      destination: deps.candidate.destination,
    },
  )

  const paid = execution.kind === 'paid'
  await db
    .prepare(
      `INSERT INTO artanis_spend_decisions
       (id, grant_ref, state, intended_amount_sat, paid_amount_sat,
        destination_source_ref, recipient_ref, rationale, payment_ref,
        policy_applied, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      decisionId,
      grant.grantRef,
      paid ? 'paid' : 'refused',
      amountSat,
      paid ? execution.paidAmountSat : null,
      deps.candidate.destinationSourceRef,
      deps.candidate.recipientRef,
      verdict.rationale.slice(0, 300),
      paid ? execution.paymentRef : null,
      execution.policyApplied,
      deps.nowIso,
      deps.nowIso,
    )
    .run()
  await mirrorArtanisRows(database, 'artanis_spend_decisions', 'id', [
    decisionId,
  ])

  return {
    decided: true,
    decisionId,
    grantRef: grant.grantRef,
    paidAmountSat: paid ? execution.paidAmountSat : null,
    paymentRef: paid ? execution.paymentRef : null,
    reason: paid ? null : execution.kind === 'refused' ? execution.reason : null,
    state: paid ? 'paid' : 'refused',
  }
}
