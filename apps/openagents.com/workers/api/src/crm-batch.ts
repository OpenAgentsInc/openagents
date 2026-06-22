/**
 * Dual-channel batch send orchestration (epic #5980, sub-issue #5988) — the
 * ~150-contact "Sprint A" run, on our own infra.
 *
 * Operator picks the segment + channel (Gmail-vs-Resend split = two batch calls,
 * one per channel). Each batch:
 *   - dry_run (DEFAULT): compose + the shared eligibility gate only, NO writes —
 *     so we see who would send / who's suppressed before any blast.
 *   - live: dispatch each contact through the unified `dispatchCrmSend`, which
 *     sends via Resend or queues a gmail_gws row for the local executor, with
 *     suppression enforced and every attempt in the ledger.
 *
 * Wave pacing (Gmail/Resend daily limits) is the caller's job: the script sends
 * one wave per call with a delay. This module is the per-wave engine + a pure
 * planner, both unit-tested.
 */
import {
  composeCrmEmailForContact,
  CrmEmailError,
  type CrmSendChannel,
} from './crm-email'
import { type CrmDispatchDeps, type CrmSendOutcome, dispatchCrmSend } from './crm-send'
import { type CrmRuntime, defaultCrmRuntime } from './crm-store'
import { readEmailSendEligibility } from './email-preferences'

export type CrmBatchDisposition =
  | 'sent'
  | 'queued'
  | 'dry_run'
  | 'would_send'
  | 'suppressed'
  | 'failed'

export type CrmBatchItemResult = Readonly<{
  contactId: string
  disposition: CrmBatchDisposition
  detail?: string
}>

export type CrmBatchSummary = Readonly<{
  channel: CrmSendChannel
  dryRun: boolean
  total: number
  counts: Readonly<Record<CrmBatchDisposition, number>>
  items: ReadonlyArray<CrmBatchItemResult>
}>

const emptyCounts = (): Record<CrmBatchDisposition, number> => ({
  dry_run: 0,
  failed: 0,
  queued: 0,
  sent: 0,
  suppressed: 0,
  would_send: 0,
})

/** Chunk contact ids into waves of at most `waveSize` (pure; for the caller). */
export const planCrmBatchWaves = (
  contactIds: ReadonlyArray<string>,
  waveSize: number,
): ReadonlyArray<ReadonlyArray<string>> => {
  const size = !Number.isFinite(waveSize) || waveSize <= 0 ? 25 : Math.floor(waveSize)
  const waves: Array<Array<string>> = []
  for (let i = 0; i < contactIds.length; i += size) {
    waves.push(contactIds.slice(i, i + size))
  }
  return waves
}

/** Map a unified send outcome to a batch disposition (pure). */
export const dispositionForOutcome = (outcome: CrmSendOutcome): CrmBatchDisposition => {
  if (outcome.channel === 'resend') {
    switch (outcome.result.kind) {
      case 'sent':
        return 'sent'
      case 'suppressed':
        return 'suppressed'
      case 'failed':
        return 'failed'
      default:
        return 'dry_run' // dry_run | not_configured
    }
  }
  return outcome.kind === 'suppressed' ? 'suppressed' : 'queued'
}

export type RunCrmBatchInput = Readonly<{
  tenantRef: string
  contactIds: ReadonlyArray<string>
  channel: CrmSendChannel
  templateSlug: string
  dryRun: boolean
  sendReason?: string | null
}>

export const runCrmBatch = async (
  db: D1Database,
  deps: CrmDispatchDeps,
  input: RunCrmBatchInput,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<CrmBatchSummary> => {
  const counts = emptyCounts()
  const items: Array<CrmBatchItemResult> = []

  for (const contactId of input.contactIds) {
    try {
      if (input.dryRun) {
        // Compose + gate only; no writes.
        const composed = await composeCrmEmailForContact(db, {
          contactId,
          templateSlug: input.templateSlug,
          tenantRef: input.tenantRef,
        })
        const eligibility = await readEmailSendEligibility(db, {
          category: 'marketing',
          email: composed.toEmail,
        })
        const disposition: CrmBatchDisposition = eligibility.allowed ? 'would_send' : 'suppressed'
        counts[disposition] += 1
        items.push({ contactId, detail: eligibility.allowed ? composed.toEmail : eligibility.reason, disposition })
        continue
      }

      const outcome = await dispatchCrmSend(
        db,
        deps,
        {
          channel: input.channel,
          contactId,
          sendReason: input.sendReason ?? 'crm_batch_outreach',
          templateSlug: input.templateSlug,
          tenantRef: input.tenantRef,
        },
        runtime,
      )
      const disposition = dispositionForOutcome(outcome)
      counts[disposition] += 1
      items.push({ contactId, disposition })
    } catch (error) {
      // A missing contact/template for one row must not abort the wave.
      counts.failed += 1
      items.push({
        contactId,
        detail: error instanceof CrmEmailError ? error.reason : String(error),
        disposition: 'failed',
      })
    }
  }

  return {
    channel: input.channel,
    counts,
    dryRun: input.dryRun,
    items,
    total: input.contactIds.length,
  }
}
