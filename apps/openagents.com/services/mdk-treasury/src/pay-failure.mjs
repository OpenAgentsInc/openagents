import { createHash } from 'node:crypto'

const publicReasonPrefix = 'reason.public.treasury_payout'

const includesAny = (text, values) => values.some(value => text.includes(value))

const safeDiagnosticSegment = value => {
  if (typeof value !== 'string') {
    return null
  }

  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_.:-]+/gu, '_')
    .replace(/^_+|_+$/gu, '')
    .slice(0, 80)

  return normalized === '' ? null : normalized
}

const errorDetail = error => (error instanceof Error ? error.message : String(error))

const errorCode = error => {
  if (typeof error !== 'object' || error === null) {
    return null
  }

  const record = error
  const cause =
    typeof record.cause === 'object' && record.cause !== null
      ? record.cause
      : null

  return (
    safeDiagnosticSegment(record.code) ??
    safeDiagnosticSegment(record.errorCode) ??
    safeDiagnosticSegment(cause?.code) ??
    safeDiagnosticSegment(cause?.errorCode)
  )
}

const messageFingerprint = detail => {
  const normalized = String(detail ?? '').trim()

  return normalized === ''
    ? null
    : createHash('sha256').update(normalized).digest('hex')
}

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
  const detail = errorDetail(error)
  const reasonRef = treasuryPayoutFailureReasonRef(detail)

  return {
    reasonClass: reasonClassFromRef(reasonRef),
    reasonRef,
  }
}

export const treasuryPayoutFailureDiagnostics = error => {
  const detail = errorDetail(error)
  const classified = classifyTreasuryPayoutFailure(error)

  return {
    ...classified,
    errorCode: errorCode(error),
    errorName:
      error instanceof Error ? safeDiagnosticSegment(error.name) : null,
    messageFingerprint: messageFingerprint(detail),
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
