const publicReasonPrefix = 'reason.public.treasury_payout'

const includesAny = (text, values) => values.some(value => text.includes(value))

export const treasuryPayoutFailureReasonRef = detail => {
  const normalized = String(detail ?? '')
    .trim()
    .toLowerCase()

  if (normalized === '') {
    return `${publicReasonPrefix}.failed`
  }

  if (
    includesAny(normalized, [
      'treasury_insufficient_spendable_balance',
      'insufficient_spendable_balance',
      'maxsendable',
    ]) ||
    (normalized.includes('insufficient') && normalized.includes('balance'))
  ) {
    return `${publicReasonPrefix}.insufficient_spendable_balance`
  }

  if (includesAny(normalized, ['self_pay', 'self pay'])) {
    return `${publicReasonPrefix}.self_pay_refused`
  }

  if (includesAny(normalized, ['timeout', 'timed out', 'deadline'])) {
    return `${publicReasonPrefix}.timeout`
  }

  if (
    includesAny(normalized, [
      'liquidity',
      'capacity',
      'channel',
      'temporary_channel_failure',
      'temporary channel failure',
    ])
  ) {
    return `${publicReasonPrefix}.liquidity`
  }

  if (includesAny(normalized, ['no_route', 'no route', 'route', 'pathfind'])) {
    return `${publicReasonPrefix}.no_route`
  }

  if (
    includesAny(normalized, [
      'amount_out_of_range',
      'amount out of range',
      'bolt11',
      'expired',
      'invoice',
      'payment request',
    ])
  ) {
    return `${publicReasonPrefix}.invoice_rejected`
  }

  return `${publicReasonPrefix}.failed`
}

export const reasonClassFromRef = reasonRef =>
  String(reasonRef).startsWith(`${publicReasonPrefix}.`)
    ? String(reasonRef).slice(`${publicReasonPrefix}.`.length)
    : 'failed'

export const classifyTreasuryPayoutFailure = error => {
  const detail = error instanceof Error ? error.message : String(error)
  const reasonRef = treasuryPayoutFailureReasonRef(detail)

  return {
    reasonClass: reasonClassFromRef(reasonRef),
    reasonRef,
  }
}

export const paymentDestinationKind = destination => {
  const value = String(destination ?? '')
    .trim()
    .toLowerCase()

  if (value.startsWith('lnbc') || value.startsWith('lntb')) {
    return 'bolt11'
  }

  if (value.startsWith('lno')) {
    return 'bolt12'
  }

  if (value.startsWith('lnurl')) {
    return 'lnurl'
  }

  if (/^[^@\s]+@[^@\s]+\.[^@\s]+$/u.test(value)) {
    return 'lightning_address'
  }

  return 'unknown'
}
