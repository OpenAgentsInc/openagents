// Autopilot composed-run REAL-BUSINESS-RECEIPT readiness DIGEST — the pure,
// deterministic markdown rendering of the readiness report, i.e. the literal
// human-readable artifact a reviewer reads (EPIC #5510, child #5519; promises
// cloud.primitives_suite.v1, cloud.agent_cloud_one_stop_revshare.v1,
// autopilot.all_in_one_business_system.v1 — all planned).
//
// The readiness report (autopilot-composed-run-receipt-readiness.ts) describes
// itself as "the ONE reviewer-facing artifact", but it emits a STRUCTURED object
// (JSON shape) — not the markdown a human reviewer actually reads. This module
// closes that last gap: it renders one readiness report into a stable, ordered,
// public-safe markdown digest, so the reviewer (or a future armed run's writeup)
// has a single document showing, per criterion, whether it currently holds, why,
// and — when it does not — exactly which dereferenceable artifact is still owed
// and which seam / proof primitive governs it.
//
// SCOPE / HONESTY: this is PURE PRESENTATION. It reads an already-built readiness
// report, introduces NO new pass/fail rule, and renders ONLY what the report
// already carries (refs + booleans + prose). It moves no money, settles no
// charge, writes no receipt, records no owner sign-off, FLIPS NO promise state,
// and DROPS NO blocker. The digest's verdict mirrors the report's `clearsBlocker`
// exactly; a `CLEARS` verdict is a RENDERED REPORT, not an action. Public-safe:
// the readiness report itself carries no amounts, idempotency keys, or payment
// destinations, so neither does this digest.

import type { RealBusinessReceiptReadinessReport } from './autopilot-composed-run-receipt-readiness'

export const AUTOPILOT_COMPOSED_RUN_RECEIPT_READINESS_DIGEST_SCHEMA =
  'openagents.autopilot_composed_run_receipt_readiness_digest.v1' as const

// The marker a satisfied / unsatisfied criterion renders with. ASCII only, so the
// digest is diff- and terminal-safe everywhere.
const SATISFIED_MARK = '[x]' as const
const UNSATISFIED_MARK = '[ ]' as const

/**
 * Render ONE readiness report into a deterministic, public-safe markdown digest.
 * PURE: reads only the report's existing fields (refs + booleans + prose), adds no
 * new pass/fail rule, and decides nothing irreversible. The digest's verdict
 * mirrors `report.clearsBlocker`; rendering it flips no promise and drops no
 * blocker. The output is stable for a given report (no timestamps, no randomness),
 * so it is safe to snapshot in a test or commit to a runbook.
 */
export const renderRealBusinessReceiptReadinessDigest = (
  report: RealBusinessReceiptReadinessReport,
): string => {
  const verdict = report.clearsBlocker
    ? 'CLEARS (all criteria satisfied)'
    : 'DOES NOT CLEAR (criteria outstanding)'

  const lines: string[] = [
    `# Autopilot composed-run real-business-receipt readiness`,
    ``,
    `- Run: \`${report.runId}\``,
    `- Blocker: \`${report.blockerRef}\``,
    `- Verdict: ${verdict}`,
    `- Criteria satisfied: ${report.satisfiedCount}/${report.totalCount}`,
    `- Receipt posture: inert=${String(report.receipt.inert)}, billed=${String(report.receipt.billed)}, settled=${String(report.receipt.settled)}`,
    ``,
    `## Receipt context`,
    ``,
    `- Balance: \`${report.receipt.balanceRef}\` (${report.receipt.balanceAsset})`,
    `- Envelope: \`${report.receipt.envelopeRef}\``,
    `- Referral state: ${report.receipt.referralState}`,
    `- Components (${report.receipt.componentReceiptRefs.length}):`,
  ]

  for (const component of report.receipt.componentReceiptRefs) {
    lines.push(
      `  - ${component.primitive}: surface \`${component.surfaceReceiptRef}\` -> settlement \`${component.settlementReceiptRef}\``,
    )
  }

  lines.push(``, `## Criteria`, ``)
  for (const line of report.lines) {
    const mark = line.satisfied ? SATISFIED_MARK : UNSATISFIED_MARK
    lines.push(`- ${mark} ${line.criterionId}: ${line.detail}`)
    if (!line.satisfied) {
      lines.push(
        `  - owed: ${line.requiredArtifact}`,
        `  - governed by: \`${line.governingRef}\``,
      )
    }
  }

  lines.push(``, `## Outstanding artifacts`, ``)
  if (report.outstandingArtifacts.length === 0) {
    lines.push(`- none — every criterion is satisfied`)
  } else {
    for (const artifact of report.outstandingArtifacts) {
      lines.push(
        `- ${artifact.criterionId}: ${artifact.requiredArtifact} (governed by \`${artifact.governingRef}\`)`,
      )
    }
  }

  if (report.unclearedBlockerRefs.length > 0) {
    lines.push(``, `## Uncleared blockers`, ``)
    for (const ref of report.unclearedBlockerRefs) {
      lines.push(`- \`${ref}\``)
    }
  }

  // Trailing newline so the digest concatenates cleanly into a larger document.
  return lines.join('\n') + '\n'
}
