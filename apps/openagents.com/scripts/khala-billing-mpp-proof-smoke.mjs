#!/usr/bin/env node

// Khala launch P0-2 billing/MPP proof smoke.
//
// This smoke is production-safe by default. It never sends a Stripe key, never
// sends an agent token, never attempts a paid MPP credential, and never flips a
// product promise. It proves the launch-safe states:
//   - MPP is either inert (503, no charge) or armed as a 402 Payment challenge.
//   - Optional card/test-credit receipts resolve only when supplied.
//   - `--require-complete` fails until the optional receipt-first evidence is
//     present, keeping the launch gate honest.

const DEFAULT_BASE = 'https://openagents.com'

const REDACTION_PATTERNS = [
  /oa_agent_[A-Za-z0-9_-]+/g,
  /\b(?:sk|rk)_(?:test|live)_[A-Za-z0-9_-]+/g,
  /\bBearer\s+[A-Za-z0-9._-]+/gi,
  /\bAuthorization:\s*Payment\s+[A-Za-z0-9._=-]+/gi,
]

export const redact = input => {
  if (input === null || input === undefined) return input
  let text = typeof input === 'string' ? input : JSON.stringify(input)
  for (const pattern of REDACTION_PATTERNS) {
    text = text.replace(pattern, '[REDACTED]')
  }
  return text
}

export const stripeCheckoutReceiptRefForSession = sessionId =>
  `receipt.billing.stripe_checkout.${sessionId}`

export const cardCreditSpendReceiptRefForSession = sessionId =>
  `receipt.inference.card_credit_spend.${sessionId}`

export const parseArgs = argv => {
  const options = {
    baseUrl: process.env.KHALA_BILLING_SMOKE_BASE_URL || DEFAULT_BASE,
    cardCreditSpendReceiptRef:
      process.env.KHALA_BILLING_CARD_CREDIT_SPEND_RECEIPT_REF,
    cardCreditSpendSessionId:
      process.env.KHALA_BILLING_CARD_CREDIT_SPEND_SESSION_ID,
    help: false,
    json: false,
    requireComplete: false,
    stripeCheckoutReceiptRef:
      process.env.KHALA_BILLING_STRIPE_CHECKOUT_RECEIPT_REF,
    stripeCheckoutSessionId:
      process.env.KHALA_BILLING_STRIPE_CHECKOUT_SESSION_ID,
  }

  for (let i = 0; i < argv.length; i += 1) {
    const value = argv[i]
    if (value === '--base-url' || value === '--baseUrl') {
      options.baseUrl = argv[++i] || options.baseUrl
    } else if (value === '--json') {
      options.json = true
    } else if (value === '--require-complete') {
      options.requireComplete = true
    } else if (value === '--stripe-checkout-session-id') {
      options.stripeCheckoutSessionId =
        argv[++i] || options.stripeCheckoutSessionId
    } else if (value === '--stripe-checkout-receipt-ref') {
      options.stripeCheckoutReceiptRef =
        argv[++i] || options.stripeCheckoutReceiptRef
    } else if (value === '--card-credit-spend-session-id') {
      options.cardCreditSpendSessionId =
        argv[++i] || options.cardCreditSpendSessionId
    } else if (value === '--card-credit-spend-receipt-ref') {
      options.cardCreditSpendReceiptRef =
        argv[++i] || options.cardCreditSpendReceiptRef
    } else if (value === '--help' || value === '-h') {
      options.help = true
    } else {
      throw new Error(`Unknown argument: ${value}`)
    }
  }

  return options
}

const HELP = `Khala billing/MPP proof smoke

Usage:
  bun run smoke:khala:billing-mpp-proof
  bun run smoke:khala:billing-mpp-proof -- --json
  bun run smoke:khala:billing-mpp-proof -- --require-complete \\
    --stripe-checkout-session-id cs_test_... \\
    --card-credit-spend-session-id cs_test_...

Options:
  --base-url <url>                         default: https://openagents.com
  --stripe-checkout-session-id <cs_...>    derives receipt.billing.stripe_checkout.<id>
  --stripe-checkout-receipt-ref <ref>      explicit public checkout receipt ref
  --card-credit-spend-session-id <cs_...>  derives receipt.inference.card_credit_spend.<id>
  --card-credit-spend-receipt-ref <ref>    explicit public composite receipt ref
  --require-complete                       fail unless optional receipt proofs pass
  --json                                  print final JSON report

This smoke never sends payment credentials or secrets.`

const nowIso = () => new Date().toISOString()

const urlFor = (base, path) => new URL(path, base).toString()

const requestJson = async (base, path, init = {}) => {
  const started = Date.now()
  const response = await fetch(urlFor(base, path), {
    ...init,
    headers: {
      accept: 'application/json',
      ...(init.headers ?? {}),
    },
  })
  let body = null
  try {
    body = await response.json()
  } catch {
    body = null
  }
  return {
    body,
    headers: response.headers,
    ms: Date.now() - started,
    status: response.status,
  }
}

export const classifyMppUnauthenticatedResponse = result => {
  const wwwAuthenticate = result.headers.get('www-authenticate') ?? ''
  if (result.status === 503 && result.body?.error === 'mpp_not_configured') {
    return {
      ok: true,
      status: 'inert',
      detail:
        'MPP endpoint is configured fail-safe inert and returned no challenge.',
    }
  }
  if (
    result.status === 402 &&
    wwwAuthenticate.toLowerCase().includes('payment ')
  ) {
    return {
      ok: true,
      status: 'armed_402',
      detail: 'MPP endpoint is armed and returned a Payment challenge.',
    }
  }
  return {
    ok: false,
    status: 'unexpected',
    detail: 'Expected fail-safe 503 inert or 402 Payment challenge.',
  }
}

const readStripeCheckoutReceipt = async (base, receiptRef) => {
  const result = await requestJson(
    base,
    `/api/public/billing/stripe-checkout-receipts/${encodeURIComponent(
      receiptRef,
    )}`,
  )
  const receipt = result.body?.receipt
  const resolution = receipt?.resolution
  const ok =
    result.status === 200 &&
    receipt?.receiptRef === receiptRef &&
    receipt?.schemaVersion ===
      'openagents.billing.stripe_checkout_receipt.v1' &&
    resolution?.status === 'ok' &&
    resolution?.paymentState === 'paid' &&
    resolution?.fulfillmentState === 'fulfilled' &&
    resolution?.creditLedgerState === 'credited'

  return { ok, result }
}

const readCardCreditSpendReceipt = async (base, receiptRef) => {
  const result = await requestJson(
    base,
    `/api/public/inference/card-credit-spend-receipts/${encodeURIComponent(
      receiptRef,
    )}`,
  )
  const receipt = result.body?.receipt
  const resolution = receipt?.resolution
  const chainReceipt = resolution?.receipt
  const ok =
    result.status === 200 &&
    receipt?.receiptRef === receiptRef &&
    receipt?.schemaVersion ===
      'openagents.inference.card_credit_spend_receipt.v1' &&
    resolution?.status === 'ok' &&
    Array.isArray(chainReceipt?.chain) &&
    chainReceipt.chain.some(step => step?.step === 'card_to_credit') &&
    chainReceipt.chain.some(step => step?.step === 'credit_to_msat') &&
    chainReceipt.chain.some(step => step?.step === 'msat_to_inference')

  return { ok, result }
}

const makeRecorder = () => {
  const checks = []
  return {
    checks,
    record(name, status, details = {}) {
      checks.push({ details, name, status })
    },
  }
}

export const buildSummary = checks => {
  const failed = checks.filter(check => check.status === 'FAIL')
  const skipped = checks.filter(check => check.status === 'SKIP')
  return {
    complete: failed.length === 0 && skipped.length === 0,
    failed: failed.length,
    skipped: skipped.length,
  }
}

const runSmoke = async options => {
  const { checks, record } = makeRecorder()
  const base = options.baseUrl

  const mpp = await requestJson(base, '/mpp/v1/chat/completions', {
    body: JSON.stringify({
      messages: [{ content: 'quote only', role: 'user' }],
      model: 'openagents/khala-mini',
    }),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })
  const mppClass = classifyMppUnauthenticatedResponse(mpp)
  record('mpp_unauthenticated_safe_state', mppClass.ok ? 'PASS' : 'FAIL', {
    classification: mppClass.status,
    detail: mppClass.detail,
    http: mpp.status,
    mppError: mpp.body?.error ?? null,
  })

  const checkoutReceiptRef =
    options.stripeCheckoutReceiptRef ||
    (options.stripeCheckoutSessionId
      ? stripeCheckoutReceiptRefForSession(options.stripeCheckoutSessionId)
      : undefined)
  if (checkoutReceiptRef === undefined || checkoutReceiptRef.trim() === '') {
    record('stripe_checkout_credit_receipt', 'SKIP', {
      reason:
        'No Stripe checkout receipt supplied. Run a browser/test-card checkout and pass --stripe-checkout-session-id.',
    })
  } else {
    const readback = await readStripeCheckoutReceipt(base, checkoutReceiptRef)
    record('stripe_checkout_credit_receipt', readback.ok ? 'PASS' : 'FAIL', {
      http: readback.result.status,
      receiptRef: checkoutReceiptRef,
      resolutionStatus:
        readback.result.body?.receipt?.resolution?.status ?? null,
      body: readback.ok ? undefined : redact(readback.result.body),
    })
  }

  const spendReceiptRef =
    options.cardCreditSpendReceiptRef ||
    (options.cardCreditSpendSessionId
      ? cardCreditSpendReceiptRefForSession(options.cardCreditSpendSessionId)
      : undefined)
  if (spendReceiptRef === undefined || spendReceiptRef.trim() === '') {
    record('card_credit_to_inference_spend_receipt', 'SKIP', {
      reason:
        'No card-credit-spend receipt supplied. Complete checkout -> USD bridge -> metered spend and pass --card-credit-spend-session-id.',
    })
  } else {
    const readback = await readCardCreditSpendReceipt(base, spendReceiptRef)
    record(
      'card_credit_to_inference_spend_receipt',
      readback.ok ? 'PASS' : 'FAIL',
      {
        http: readback.result.status,
        receiptRef: spendReceiptRef,
        resolutionStatus:
          readback.result.body?.receipt?.resolution?.status ?? null,
        body: readback.ok ? undefined : redact(readback.result.body),
      },
    )
  }

  const summary = buildSummary(checks)
  return {
    baseUrl: base,
    checks,
    generatedAt: nowIso(),
    ok:
      summary.failed === 0 &&
      (!options.requireComplete || summary.skipped === 0),
    requireComplete: options.requireComplete,
    summary,
  }
}

const main = async () => {
  const options = parseArgs(process.argv.slice(2))
  if (options.help) {
    console.log(HELP)
    return
  }

  const report = await runSmoke(options)
  if (options.json) {
    console.log(JSON.stringify(report, null, 2))
  } else {
    for (const check of report.checks) {
      console.log(`${check.status} ${check.name} ${redact(check.details)}`)
    }
    console.log(
      `${report.ok ? 'PASS' : 'FAIL'} khala billing/MPP proof smoke (${report.summary.failed} failed, ${report.summary.skipped} skipped)`,
    )
  }
  if (!report.ok) {
    process.exitCode = 1
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(error => {
    console.error(redact(error?.stack ?? String(error)))
    process.exitCode = 1
  })
}
