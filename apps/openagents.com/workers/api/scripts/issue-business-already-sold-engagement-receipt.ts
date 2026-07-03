import {
  buildBusinessAlreadySoldEngagementPaymentReceipt,
  type BusinessAlreadySoldDemandProvenance,
  type BusinessAlreadySoldEngagementKind,
  type BusinessAlreadySoldPaymentCurrency,
  type BusinessAlreadySoldVerticalDescriptor,
} from '../src/business-already-sold-engagement-receipt'

const args = process.argv.slice(2)

const flag = (name: string): string | null => {
  const index = args.indexOf(name)
  return index >= 0 && index + 1 < args.length ? args[index + 1] : null
}

const requireFlag = (name: string): string => {
  const value = flag(name)
  if (value === null) {
    console.error(`Error: ${name} is required.`)
    process.exit(1)
  }
  return value
}

const csv = (value: string): ReadonlyArray<string> =>
  value
    .split(',')
    .map(part => part.trim())
    .filter(part => part.length > 0)

const main = () => {
  try {
    const receipt = buildBusinessAlreadySoldEngagementPaymentReceipt({
      engagementRef: requireFlag('--engagement-ref'),
      buyerRef: requireFlag('--buyer-ref'),
      buyerPaidRef: requireFlag('--buyer-paid-ref'),
      engagementKind: requireFlag(
        '--engagement-kind',
      ) as BusinessAlreadySoldEngagementKind,
      verticalDescriptor: requireFlag(
        '--vertical',
      ) as BusinessAlreadySoldVerticalDescriptor,
      amountMinorUnits: Number(requireFlag('--amount-minor-units')),
      currency: requireFlag('--currency') as BusinessAlreadySoldPaymentCurrency,
      paidAt: requireFlag('--paid-at'),
      recordedAt: flag('--recorded-at') ?? undefined,
      demandProvenance: requireFlag(
        '--demand-provenance',
      ) as BusinessAlreadySoldDemandProvenance,
      privacyReview: {
        reviewed: true,
        reviewedAt: requireFlag('--privacy-reviewed-at'),
        reviewerRef: requireFlag('--privacy-reviewer-ref'),
        decisionRef: requireFlag('--privacy-decision-ref'),
      },
      sourceRefs: csv(requireFlag('--source-refs')),
      caveatRefs:
        flag('--caveat-refs') === null
          ? undefined
          : csv(requireFlag('--caveat-refs')),
    })

    console.log(JSON.stringify(receipt, null, 2))
  } catch (error) {
    console.error(error)
    process.exit(1)
  }
}

main()

