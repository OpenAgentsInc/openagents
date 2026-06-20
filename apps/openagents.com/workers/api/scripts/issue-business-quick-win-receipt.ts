/**
 * Operator tool to issue a deterministically generated business quick-win receipt.
 * 
 * Closes blocker.product_promises.business_first_paid_quick_win_receipt_missing
 * by providing the required operator loop to generate the required receipt 
 * object (intake -> scoped -> delivered -> accepted -> paid) before public
 * product packaging is finished.
 * 
 * Usage:
 *   bun run scripts/issue-business-quick-win-receipt.ts \
 *     --signup-id <id> \
 *     --offering-id <promiseId> \
 *     --summary "..." \
 *     [--scoped-ref <ref>] \
 *     [--delivered-ref <ref>] \
 *     [--accepted-ref <ref>] \
 *     [--buyer-paid-ref <ref>] \
 *     [--provider-settled-ref <ref>] \
 *     [--assert-paid]
 */

import {
  buildBusinessQuickWinReceipt,
  assertFirstPaidQuickWinReceipt,
  type BusinessQuickWinReceiptInput,
} from '../src/business-quick-win-receipt'

const args = process.argv.slice(2)
const flag = (name: string): string | null => {
  const index = args.indexOf(name)
  return index >= 0 && index + 1 < args.length ? args[index + 1] : null
}
const hasFlag = (name: string): boolean => args.includes(name)

const main = () => {
  const signupId = flag('--signup-id')
  const offeringPromiseId = flag('--offering-id')
  const quickWinSummary = flag('--summary')

  if (!signupId || !offeringPromiseId || !quickWinSummary) {
    console.error(
      'Error: --signup-id, --offering-id, and --summary are required.',
    )
    process.exit(1)
  }

  const input: BusinessQuickWinReceiptInput = {
    signupId,
    offeringPromiseId,
    quickWinSummary,
    quickWinScopedRef: flag('--scoped-ref'),
    deliveredEvidenceRef: flag('--delivered-ref'),
    outcomeAcceptedRef: flag('--accepted-ref'),
    buyerPaidRef: flag('--buyer-paid-ref'),
    providerSettledRef: flag('--provider-settled-ref'),
  }

  try {
    const receipt = buildBusinessQuickWinReceipt(input)

    if (hasFlag('--assert-paid')) {
      assertFirstPaidQuickWinReceipt(receipt)
      console.error('Paid quick-win receipt assertions passed.')
    }

    console.log(JSON.stringify(receipt, null, 2))
  } catch (error) {
    console.error(error)
    process.exit(1)
  }
}

main()
