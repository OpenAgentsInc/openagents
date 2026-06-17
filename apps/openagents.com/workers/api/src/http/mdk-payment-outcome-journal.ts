import { currentIsoTimestamp } from '../runtime-primitives'

export type MdkPaymentOutcomeStatus = 'succeeded' | 'failed'

export type DurableMdkPaymentOutcome = Readonly<{
  observedAt: string
  reasonRef: string | null
  status: MdkPaymentOutcomeStatus
}>

const PAYMENT_OUTCOME_KEY_PREFIX = 'mdk-payment-outcome:'

const terminalStatuses = new Set<string>(['failed', 'succeeded'])

const safePublicReasonRef = (value: unknown): string | null => {
  if (typeof value !== 'string') {
    return null
  }

  const trimmed = value.trim()

  return /^reason\.public\.[a-z0-9_.]+$/u.test(trimmed) ? trimmed : null
}

export const mdkPaymentOutcomeStorageKey = (paymentId: string): string =>
  `${PAYMENT_OUTCOME_KEY_PREFIX}${paymentId}`

export const mdkPaymentIdFromStatusPath = (pathname: string): string | null => {
  const match = /^\/payments\/([^/]+)$/u.exec(pathname)

  if (match === null) {
    return null
  }

  const encodedPaymentId = match[1]

  if (encodedPaymentId === undefined) {
    return null
  }

  try {
    const paymentId = decodeURIComponent(encodedPaymentId).trim()

    return paymentId === '' ? null : paymentId
  } catch {
    return null
  }
}

export const mdkTerminalOutcomeFromPayload = (
  payload: unknown,
  observedAt: string,
): DurableMdkPaymentOutcome | null => {
  if (typeof payload !== 'object' || payload === null) {
    return null
  }

  const record = payload as Record<string, unknown>
  const status = record.status

  if (typeof status !== 'string' || !terminalStatuses.has(status)) {
    return null
  }

  return {
    observedAt,
    reasonRef: safePublicReasonRef(record.reasonRef),
    status: status as MdkPaymentOutcomeStatus,
  }
}

export const mdkPaymentIdFromPayload = (payload: unknown): string | null => {
  if (typeof payload !== 'object' || payload === null) {
    return null
  }

  const paymentId = (payload as Record<string, unknown>).paymentId

  return typeof paymentId === 'string' && paymentId.trim() !== ''
    ? paymentId.trim()
    : null
}

export const durableMdkPaymentOutcomeResponse = (
  paymentId: string,
  outcome: DurableMdkPaymentOutcome,
): Response =>
  new Response(
    JSON.stringify({
      journaled: true,
      paymentId,
      reason: outcome.reasonRef,
      reasonRef: outcome.reasonRef,
      status: outcome.status,
    }),
    {
      headers: {
        'cache-control': 'no-store',
        'content-type': 'application/json',
      },
      status: 200,
    },
  )

// Parse a container payment Response and journal its terminal outcome via the
// provided storage writer. Lives in the /http/ response-shaping layer so the
// MDK Durable Object keeps no Response-typed surface of its own.
export const journalMdkResponseOutcome = async (
  response: Response,
  writeOutcome: (
    paymentId: string,
    outcome: DurableMdkPaymentOutcome,
  ) => Promise<void>,
): Promise<void> => {
  const payload = await response
    .clone()
    .json()
    .catch(() => null)
  const paymentId = mdkPaymentIdFromPayload(payload)

  if (paymentId === null) {
    return
  }

  const outcome = mdkTerminalOutcomeFromPayload(payload, currentIsoTimestamp())

  if (outcome !== null) {
    await writeOutcome(paymentId, outcome)
  }
}
