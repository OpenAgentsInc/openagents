import { describe, expect, test } from 'vitest'

import {
  buildComposedRunPlan,
  type ComposedRunComponentInput,
  type ComposedRunPlan,
} from './autopilot-composed-run'
import {
  composeRunExecution,
  type ComposedRunExecution,
  type ComposedRunReferralInput,
} from './autopilot-composed-run-execution'
import {
  buildComposedRunReceipt,
  type ComposedRunReceipt,
} from './autopilot-composed-run-receipt'
import type { RealBusinessReceiptEvidence } from './autopilot-composed-run-receipt-gate'
import {
  buildRealBusinessReceiptReadinessReport,
  inertReadinessReport,
} from './autopilot-composed-run-receipt-readiness'
import { renderRealBusinessReceiptReadinessDigest } from './autopilot-composed-run-receipt-readiness-digest'
import type { ReferredPrincipal } from './referral-cross-category-accrual'

// A D1 stub that THROWS on any IO: the digest path stays strictly inert.
const explodingDb = new Proxy(
  {},
  {
    get() {
      throw new Error(
        'composed-run receipt readiness digest must not touch the database (INERT)',
      )
    },
  },
) as unknown as D1Database

const principal: ReferredPrincipal = { kind: 'agent', userId: 'agent-payer' }

const components: ReadonlyArray<ComposedRunComponentInput> = [
  {
    primitive: 'inference',
    capabilityRef: 'promise:inference.gateway_credits_business.v1',
    componentRunId: 'req-1',
  },
  {
    primitive: 'fine_tuning',
    capabilityRef: 'promise:cloud.fine_tuning_service.v1',
    componentRunId: 'ft-1',
  },
]

const referral: ComposedRunReferralInput = {
  eventId: 'evt-1',
  sellerRef: 'agent:raynor',
  referrerRef: 'agent:kerrigan',
  referralBps: 500,
  principal,
}

const buildReceipt = async (): Promise<ComposedRunReceipt> => {
  const planResult = buildComposedRunPlan({
    runId: 'run-1',
    businessRef: 'agent:raynor',
    title: 'All-in-one run',
    summary: 'inference + fine-tuning on one balance',
    balance: { balanceRef: 'balance:agent:raynor', asset: 'credit' },
    components,
    createdAt: '2026-06-20T00:00:00.000Z',
  })
  if (!planResult.ok) {
    throw new Error(planResult.error.reason)
  }
  const plan: ComposedRunPlan = planResult.plan

  const execResult = await composeRunExecution(explodingDb, {
    plan,
    accountRef: 'account:raynor',
    components: [
      { primitive: 'inference', componentRunId: 'req-1', chargeMsat: 1200 },
      { primitive: 'fine_tuning', componentRunId: 'ft-1', chargeMsat: 3400 },
    ],
    referral,
  })
  if (!execResult.ok) {
    throw new Error(execResult.error.reason)
  }
  const execution: ComposedRunExecution = execResult.execution

  const receiptResult = buildComposedRunReceipt({ plan, execution })
  if (!receiptResult.ok) {
    throw new Error(receiptResult.error.reason)
  }
  return receiptResult.receipt
}

const armedEvidence = (
  receipt: ComposedRunReceipt,
): RealBusinessReceiptEvidence => ({
  receipt,
  componentsBilled: true,
  revenueApplies: true,
  revshareSettled: true,
  ownerSignoffReceiptRef: 'receipt.promise_transition.autopilot.run-1',
  demandProvenance: 'external_market',
})

describe('autopilot composed-run real-business-receipt readiness DIGEST (#5519)', () => {
  test('the inert digest renders the honest DOES NOT CLEAR verdict and names the unmet criteria', async () => {
    const receipt = await buildReceipt()
    const report = inertReadinessReport(receipt)
    const digest = renderRealBusinessReceiptReadinessDigest(report)

    expect(digest).toContain('# Autopilot composed-run real-business-receipt readiness')
    expect(digest).toContain('Verdict: DOES NOT CLEAR')
    expect(digest).toContain(`Criteria satisfied: ${report.satisfiedCount}/${report.totalCount}`)
    // Honest posture is rendered.
    expect(digest).toContain('billed=false')
    expect(digest).toContain('settled=false')
    expect(digest).toContain('inert=true')
    // Every unsatisfied criterion is rendered with the unchecked marker.
    for (const line of report.lines) {
      if (!line.satisfied) {
        expect(digest).toContain(`[ ] ${line.criterionId}`)
      }
    }
    // Every outstanding artifact is named with its governing ref.
    for (const artifact of report.outstandingArtifacts) {
      expect(digest).toContain(artifact.requiredArtifact)
      expect(digest).toContain(artifact.governingRef)
    }
  })

  test('the armed digest renders CLEARS with no outstanding artifacts', async () => {
    const receipt = await buildReceipt()
    const report = buildRealBusinessReceiptReadinessReport(armedEvidence(receipt))
    const digest = renderRealBusinessReceiptReadinessDigest(report)

    expect(digest).toContain('Verdict: CLEARS')
    expect(digest).toContain(`Criteria satisfied: ${report.totalCount}/${report.totalCount}`)
    expect(digest).toContain('none — every criterion is satisfied')
    // Every criterion renders satisfied; none render the unchecked marker.
    expect(digest).not.toContain('[ ]')
    for (const line of report.lines) {
      expect(digest).toContain(`[x] ${line.criterionId}`)
    }
  })

  test('the digest is public-safe — no per-component amounts leak', async () => {
    const receipt = await buildReceipt()
    const digest = renderRealBusinessReceiptReadinessDigest(
      buildRealBusinessReceiptReadinessReport(armedEvidence(receipt)),
    )

    expect(digest).not.toContain('1200')
    expect(digest).not.toContain('3400')
    expect(digest).not.toContain('chargeMsat')
    // But the public-safe refs ARE present.
    expect(digest).toContain('balance:agent:raynor')
  })

  test('the digest is deterministic — identical reports render byte-identical text', async () => {
    const receipt = await buildReceipt()
    const report = inertReadinessReport(receipt)
    const first = renderRealBusinessReceiptReadinessDigest(report)
    const second = renderRealBusinessReceiptReadinessDigest(report)

    expect(first).toBe(second)
    // Stable trailing newline so it concatenates cleanly into a larger document.
    expect(first.endsWith('\n')).toBe(true)
    // No trailing whitespace on any rendered line.
    for (const line of first.split('\n')) {
      expect(line).toBe(line.replace(/\s+$/, ''))
    }
  })
})
