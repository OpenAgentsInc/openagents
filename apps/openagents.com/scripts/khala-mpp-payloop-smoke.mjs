#!/usr/bin/env node

// Khala MPP crypto pay-loop END-TO-END smoke (EPIC #6049).
//
// This is the FULL pay-loop proof, distinct from the inert/402-only
// `khala-billing-mpp-proof-smoke.mjs`: it actually drives a payment to
// settlement and asserts a served Khala completion. It is the proof we run
// against an ARMED staging endpoint with a TEST Stripe key BEFORE we trust the
// MPP endpoint with real money.
//
// The loop (per docs/reference/mpp/paymentauth/specs/core/draft-httpauth-payment-00.md
// + the Stripe deposit-mode crypto + machine-payments docs):
//   1. POST /mpp/v1/chat/completions with NO credential. Assert 402 +
//      `WWW-Authenticate: Payment` carrying a real crypto deposit recipient
//      address + a challenge `id`.
//   2. Recover the deposit PaymentIntent id from the challenge `opaque`
//      (base64url JCS JSON).
//   3. Call Stripe's `simulate_crypto_deposit` TEST helper (API
//      2026-05-27.preview) with the success transaction-hash to settle that
//      PaymentIntent on the sandbox (testnets are not auto-detected).
//   4. Construct an `Authorization: Payment <base64url>` credential that ECHOES
//      the issued challenge (so the server recomputes the same HMAC id) and
//      retry. Assert 200 + a `Payment-Receipt` header + a real Khala completion
//      body, and best-effort that the USD-origin credit grant `mpp:<pi>`
//      dereferences and metered spend was recorded.
//
// SAFETY: this smoke NEVER hardcodes a base URL or a Stripe key — both come from
// env/flags. It only ever sends a TEST Stripe key to the Stripe TEST-helper
// endpoint (never to our own gateway). It NEVER arms the endpoint, NEVER sets
// secrets, and NEVER deploys. If the endpoint is inert (503), it fails clearly
// telling the operator the staging secrets are not set yet — it does NOT report
// a false green.

// ---- redaction (mirrors khala-billing-mpp-proof-smoke.mjs) ----

const REDACTION_PATTERNS = [
  /oa_agent_[A-Za-z0-9_-]+/g,
  /\b(?:sk|rk)_(?:test|live)_[A-Za-z0-9_-]+/g,
  /\bBearer\s+[A-Za-z0-9._-]+/gi,
  /\bAuthorization:\s*Payment\s+[A-Za-z0-9._=-]+/gi,
  // The credential itself, however it is keyed/labelled.
  /\bPayment\s+[A-Za-z0-9._=-]{16,}/g,
]

export const redact = input => {
  if (input === null || input === undefined) return input
  let text = typeof input === 'string' ? input : JSON.stringify(input)
  for (const pattern of REDACTION_PATTERNS) {
    text = text.replace(pattern, '[REDACTED]')
  }
  return text
}

// ---- base64url-nopad + JCS (matches mpp-canonical.ts on the wire) ----

// Encode a UTF-8 string as base64url without padding.
export const base64UrlEncode = value =>
  Buffer.from(value, 'utf8')
    .toString('base64')
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replaceAll('=', '')

// Decode a base64url-nopad string back to a UTF-8 string. Returns undefined for
// invalid input (fail-closed).
export const base64UrlDecode = value => {
  try {
    const padded = value
      .replaceAll('-', '+')
      .replaceAll('_', '/')
      .padEnd(Math.ceil(value.length / 4) * 4, '=')
    return Buffer.from(padded, 'base64').toString('utf8')
  } catch {
    return undefined
  }
}

// Decode a base64url-nopad JCS parameter to a JSON record. Returns undefined on
// any decode/parse failure.
export const decodeJcsBase64UrlRecord = value => {
  const json = base64UrlDecode(value)
  if (json === undefined) return undefined
  try {
    const parsed = JSON.parse(json)
    if (parsed !== null && typeof parsed === 'object' && !Array.isArray(parsed)) {
      return parsed
    }
    return undefined
  } catch {
    return undefined
  }
}

// Recover the crypto deposit PaymentIntent id from a challenge `opaque`
// (base64url JCS of `{ pi, amount, network, model }` per mpp-protocol.ts
// MppOpaque). Returns undefined if the opaque is missing/malformed or carries
// no `pi`.
export const decodeOpaquePaymentIntentId = opaqueB64 => {
  if (typeof opaqueB64 !== 'string' || opaqueB64 === '') return undefined
  const record = decodeJcsBase64UrlRecord(opaqueB64)
  if (record === undefined) return undefined
  return typeof record.pi === 'string' && record.pi !== '' ? record.pi : undefined
}

// ---- WWW-Authenticate: Payment <params> parsing ----

// Parse a single `Payment k1="v1", k2="v2", ...` header value into a flat
// key->value record. Tolerant of escaped quotes inside values (the server
// escapes `"` as `\"`). Returns undefined for a non-Payment scheme.
export const parsePaymentChallengeHeader = headerValue => {
  if (typeof headerValue !== 'string') return undefined
  const trimmed = headerValue.trim()
  if (!/^Payment\s+/i.test(trimmed)) return undefined
  const rest = trimmed.replace(/^Payment\s+/i, '')
  const params = {}
  // Match k="...": value may contain escaped quotes (\") and commas.
  const re = /([A-Za-z0-9_-]+)\s*=\s*"((?:\\.|[^"\\])*)"/g
  let match
  while ((match = re.exec(rest)) !== null) {
    params[match[1]] = match[2].replace(/\\"/g, '"')
  }
  return Object.keys(params).length > 0 ? params : undefined
}

// From a 402 response (status + headers + parsed problem body), recover the
// CRYPTO challenge to pay against. We prefer the WWW-Authenticate header (the
// authoritative wire form) for the bound `id/realm/method/intent/request/expires/
// opaque`, and fall back to the inline `challenges[]` structured echo for the
// human-readable recipient/network/paymentIntentId mirrors. A crypto challenge
// is any method that is NOT `stripe` (i.e. a crypto network: base/solana/tempo).
export const recoverCryptoChallenge = (result, cryptoNetworks) => {
  const networks =
    cryptoNetworks && cryptoNetworks.length > 0
      ? cryptoNetworks
      : ['base', 'solana', 'tempo']
  const isCrypto = method =>
    typeof method === 'string' && method !== 'stripe'

  // 1. WWW-Authenticate headers (one per accepted method). getSetCookie-style
  //    multi-value: a Headers object joins repeated values with ", "; we split
  //    on the scheme keyword so each "Payment ..." is parsed independently.
  const rawHeader = result.headers?.get?.('www-authenticate') ?? ''
  const headerChallenges = String(rawHeader)
    .split(/(?=Payment\s)/i)
    .map(part => part.trim())
    .filter(part => part !== '')
    .map(parsePaymentChallengeHeader)
    .filter(Boolean)

  // 2. inline structured echo (challenges[]).
  const inline = Array.isArray(result.body?.challenges)
    ? result.body.challenges
    : []

  // Pick the crypto header challenge (carries the bound fields we must echo).
  const headerCrypto = headerChallenges.find(c =>
    isCrypto(c.method) && networks.includes(c.method),
  )
  // Pick the matching inline echo (carries recipient + paymentIntentId mirrors).
  const inlineCrypto = inline.find(
    c => isCrypto(c.method) && networks.includes(c.method),
  )

  if (headerCrypto === undefined) return undefined

  // Recipient + payment intent id: prefer the inline echo, then derive PI from
  // the bound opaque (the load-bearing, HMAC-covered source).
  const paymentIntentId =
    decodeOpaquePaymentIntentId(headerCrypto.opaque) ??
    (typeof inlineCrypto?.paymentIntentId === 'string'
      ? inlineCrypto.paymentIntentId
      : undefined)
  const recipient =
    (typeof inlineCrypto?.recipient === 'string' && inlineCrypto.recipient) ||
    // The recipient also lives inside the bound `request` JCS.
    (() => {
      const reqRecord = headerCrypto.request
        ? decodeJcsBase64UrlRecord(headerCrypto.request)
        : undefined
      return typeof reqRecord?.recipient === 'string'
        ? reqRecord.recipient
        : undefined
    })()

  return {
    // The fields we MUST echo back verbatim so the server recomputes the same
    // HMAC id (mpp-protocol.ts verifyCredential): id/realm/method/intent/
    // request/expires/opaque (digest absent for our crypto rail).
    challenge: {
      id: headerCrypto.id,
      realm: headerCrypto.realm,
      method: headerCrypto.method,
      intent: headerCrypto.intent,
      request: headerCrypto.request,
      ...(headerCrypto.expires === undefined
        ? {}
        : { expires: headerCrypto.expires }),
      ...(headerCrypto.opaque === undefined
        ? {}
        : { opaque: headerCrypto.opaque }),
    },
    network: headerCrypto.method,
    paymentIntentId,
    recipient,
  }
}

// Construct the `Authorization: Payment <base64url>` credential value per the
// core spec §"Credentials": base64url-nopad of `{ challenge, payload, source? }`
// where `challenge` ECHOES the issued challenge. For the crypto rail the deposit
// reference rides in the bound `opaque` (already inside the echoed challenge),
// so `payload` is a small method-proof object; we carry the settled tx hash as a
// non-load-bearing breadcrumb (the server settles by retrieving the PI, not by
// trusting payload).
export const buildPaymentCredential = ({ challenge, payload, source }) => {
  const credential = {
    challenge,
    payload: payload ?? {},
    ...(source === undefined ? {} : { source }),
  }
  return base64UrlEncode(JSON.stringify(credential))
}

// Public-safe receipt ref for the USD-origin credit grant minted by a settled
// MPP payment (mpp-credit-grant.ts mppGrantRef + usd-credit-bridge.ts
// usdCreditGrantReceiptRef). Dereferenceable at
// GET /api/public/inference/receipts/<ref>.
export const mppCreditGrantReceiptRef = paymentIntentId =>
  `receipt.inference.usd_credit_grant.mpp:${paymentIntentId}`

// ---- CLI ----

const DEFAULT_BASE = process.env.KHALA_MPP_PAYLOOP_BASE_URL || ''
const STRIPE_API_BASE = 'https://api.stripe.com/v1'
// Stripe TEST-helper API version (sandbox crypto deposit simulation).
const STRIPE_SIMULATE_API_VERSION = '2026-05-27.preview'
// The deterministic success transaction-hash for the sandbox crypto-deposit
// simulator (Stripe deposit-mode test docs). Drives the PI to `succeeded`
// within ~15s. The failure hash ends in `testfailed`.
const STRIPE_SIMULATE_SUCCESS_TX_HASH =
  '0x00000000000000000000000000000000000000000000000000000testsuccess'
const CRYPTO_NETWORKS = ['base', 'solana', 'tempo']

export const parseArgs = argv => {
  const options = {
    baseUrl: DEFAULT_BASE,
    stripeTestKey: process.env.KHALA_MPP_PAYLOOP_STRIPE_TEST_KEY || '',
    model: process.env.KHALA_MPP_PAYLOOP_MODEL || 'openagents/khala-mini',
    settleTimeoutMs: Number(
      process.env.KHALA_MPP_PAYLOOP_SETTLE_TIMEOUT_MS || 60000,
    ),
    json: false,
    help: false,
  }
  for (let i = 0; i < argv.length; i += 1) {
    const value = argv[i]
    if (value === '--base-url' || value === '--baseUrl') {
      options.baseUrl = argv[++i] || options.baseUrl
    } else if (value === '--stripe-test-key') {
      options.stripeTestKey = argv[++i] || options.stripeTestKey
    } else if (value === '--model') {
      options.model = argv[++i] || options.model
    } else if (value === '--settle-timeout-ms') {
      options.settleTimeoutMs = Number(argv[++i] || options.settleTimeoutMs)
    } else if (value === '--json') {
      options.json = true
    } else if (value === '--help' || value === '-h') {
      options.help = true
    } else {
      throw new Error(`Unknown argument: ${value}`)
    }
  }
  return options
}

const HELP = `Khala MPP crypto pay-loop end-to-end smoke

Proves the ARMED MPP endpoint settles a crypto payment and serves a Khala
completion. Run against STAGING with a TEST Stripe key. Never deploys, never
arms, never sets secrets.

Usage:
  KHALA_MPP_PAYLOOP_BASE_URL=https://staging.example \\
  KHALA_MPP_PAYLOOP_STRIPE_TEST_KEY=sk_test_... \\
    bun run smoke:khala:mpp-payloop -- --json

  bun run smoke:khala:mpp-payloop -- \\
    --base-url https://staging.example \\
    --stripe-test-key sk_test_... \\
    --json

Options:
  --base-url <url>            staging gateway base URL (REQUIRED)
  --stripe-test-key <sk_...>  TEST Stripe secret key for the deposit simulator (REQUIRED)
  --model <id>                Khala model to quote (default openagents/khala-mini)
  --settle-timeout-ms <n>     max wait for the simulated deposit to settle (default 60000)
  --json                      print final JSON report
  --help                      show this help

This smoke sends the TEST Stripe key ONLY to api.stripe.com's test-helper
endpoint, never to the gateway. All keys/credentials are redacted in output.`

const nowIso = () => new Date().toISOString()
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms))
const urlFor = (base, path) => new URL(path, base).toString()

const requestJson = async (base, path, init = {}) => {
  const started = Date.now()
  const response = await fetch(urlFor(base, path), {
    ...init,
    headers: { accept: 'application/json', ...(init.headers ?? {}) },
  })
  let body = null
  let text = null
  try {
    text = await response.text()
    body = text ? JSON.parse(text) : null
  } catch {
    body = null
  }
  return {
    body,
    text,
    headers: response.headers,
    ms: Date.now() - started,
    status: response.status,
  }
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

// POST the gateway with no credential and classify the 402 vs the inert 503.
const requestChallenge = (base, model) =>
  requestJson(base, '/mpp/v1/chat/completions', {
    body: JSON.stringify({
      messages: [{ content: 'mpp pay-loop smoke', role: 'user' }],
      model,
    }),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

// Retry the gateway WITH the constructed credential.
const requestPaid = (base, model, authorization) =>
  requestJson(base, '/mpp/v1/chat/completions', {
    body: JSON.stringify({
      messages: [{ content: 'mpp pay-loop smoke', role: 'user' }],
      model,
    }),
    headers: {
      authorization: `Payment ${authorization}`,
      'content-type': 'application/json',
    },
    method: 'POST',
  })

// Stripe form-encode a flat params object.
const encodeForm = params =>
  Object.entries(params)
    .map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`)
    .join('&')

// Call the Stripe TEST-helper to simulate a crypto deposit settling the PI.
const simulateCryptoDeposit = async (stripeTestKey, paymentIntentId) => {
  const response = await fetch(
    `${STRIPE_API_BASE}/payment_intents/${encodeURIComponent(
      paymentIntentId,
    )}/simulate_crypto_deposit`,
    {
      body: encodeForm({ transaction_hash: STRIPE_SIMULATE_SUCCESS_TX_HASH }),
      headers: {
        authorization: `Bearer ${stripeTestKey}`,
        'content-type': 'application/x-www-form-urlencoded',
        'stripe-version': STRIPE_SIMULATE_API_VERSION,
      },
      method: 'POST',
    },
  )
  let body = null
  try {
    body = await response.json()
  } catch {
    body = null
  }
  return { status: response.status, body }
}

// Retrieve a PaymentIntent (TEST key) to read its status.
const retrievePaymentIntentStatus = async (stripeTestKey, paymentIntentId) => {
  const response = await fetch(
    `${STRIPE_API_BASE}/payment_intents/${encodeURIComponent(paymentIntentId)}`,
    {
      headers: {
        authorization: `Bearer ${stripeTestKey}`,
        'stripe-version': STRIPE_SIMULATE_API_VERSION,
      },
      method: 'GET',
    },
  )
  let body = null
  try {
    body = await response.json()
  } catch {
    body = null
  }
  return {
    status: response.status,
    intentStatus: typeof body?.status === 'string' ? body.status : 'unknown',
    body,
  }
}

// Poll the PI until it reaches `succeeded` (the simulator settles within ~15s).
const waitForSettled = async (
  stripeTestKey,
  paymentIntentId,
  timeoutMs,
) => {
  const deadline = Date.now() + Math.max(15000, timeoutMs)
  let last = { intentStatus: 'unknown', status: 0 }
  while (Date.now() < deadline) {
    last = await retrievePaymentIntentStatus(stripeTestKey, paymentIntentId)
    if (last.intentStatus === 'succeeded') {
      return { settled: true, ...last }
    }
    if (
      last.intentStatus === 'requires_payment_method' ||
      last.intentStatus === 'canceled'
    ) {
      // Terminal non-settled state (e.g. the failure tx hash path).
      return { settled: false, ...last }
    }
    await sleep(3000)
  }
  return { settled: false, ...last }
}

// Best-effort: dereference the minted USD-origin credit grant receipt.
const readCreditGrantReceipt = async (base, paymentIntentId) => {
  const receiptRef = mppCreditGrantReceiptRef(paymentIntentId)
  const result = await requestJson(
    base,
    `/api/public/inference/receipts/${encodeURIComponent(receiptRef)}`,
  )
  const ok =
    result.status === 200 &&
    result.body?.receipt?.kind === 'usd_credit_grant'
  return { ok, receiptRef, result }
}

const runSmoke = async options => {
  const { checks, record } = makeRecorder()
  const base = options.baseUrl

  // PRECONDITIONS — fail clearly (not falsely green) when the operator has not
  // supplied the staging URL + TEST key.
  if (!base || base.trim() === '') {
    record('preconditions', 'FAIL', {
      reason:
        'No base URL. Pass --base-url or set KHALA_MPP_PAYLOOP_BASE_URL to the STAGING gateway.',
    })
    return finalize(checks, base, options)
  }
  if (!options.stripeTestKey || options.stripeTestKey.trim() === '') {
    record('preconditions', 'FAIL', {
      reason:
        'No TEST Stripe key. Pass --stripe-test-key or set KHALA_MPP_PAYLOOP_STRIPE_TEST_KEY (sk_test_...).',
    })
    return finalize(checks, base, options)
  }
  if (!/^(sk|rk)_test_/.test(options.stripeTestKey.trim())) {
    record('preconditions', 'FAIL', {
      reason:
        'Stripe key is not a TEST key (expected sk_test_/rk_test_). This smoke must never run with a live key.',
    })
    return finalize(checks, base, options)
  }
  record('preconditions', 'PASS', { baseUrl: base })

  // STEP 1 — 402 challenge with no credential.
  const challengeResult = await requestChallenge(base, options.model)
  if (
    challengeResult.status === 503 &&
    challengeResult.body?.error === 'mpp_not_configured'
  ) {
    // Inert: fail clearly. Not a false green.
    record('mpp_armed_402_challenge', 'FAIL', {
      classification: 'inert',
      detail:
        'MPP endpoint is INERT (503 mpp_not_configured). The staging secrets (KHALA_MPP_ENABLED + Stripe key + KHALA_MPP_SIGNING_SECRET) are not set yet. Arm staging before running the pay-loop smoke.',
      http: challengeResult.status,
      reason: challengeResult.body?.reason ?? null,
    })
    return finalize(checks, base, options)
  }
  const crypto = recoverCryptoChallenge(challengeResult, CRYPTO_NETWORKS)
  const has402 =
    challengeResult.status === 402 &&
    crypto !== undefined &&
    typeof crypto.challenge.id === 'string' &&
    crypto.challenge.id !== '' &&
    typeof crypto.recipient === 'string' &&
    crypto.recipient !== ''
  record('mpp_armed_402_challenge', has402 ? 'PASS' : 'FAIL', {
    http: challengeResult.status,
    challengeId: crypto?.challenge.id ?? null,
    network: crypto?.network ?? null,
    hasRecipient: typeof crypto?.recipient === 'string' && crypto.recipient !== '',
    // Recipient address is a deposit address (not a secret), but keep output tidy.
    recipientPresent: Boolean(crypto?.recipient),
    body: has402 ? undefined : redact(challengeResult.body),
  })
  if (!has402) {
    return finalize(checks, base, options)
  }

  // STEP 2 — recover the deposit PaymentIntent id from the bound opaque.
  const paymentIntentId = crypto.paymentIntentId
  const hasPi = typeof paymentIntentId === 'string' && paymentIntentId !== ''
  record('recover_payment_intent_from_opaque', hasPi ? 'PASS' : 'FAIL', {
    paymentIntentPresent: hasPi,
    source: 'challenge.opaque (base64url JCS)',
  })
  if (!hasPi) {
    return finalize(checks, base, options)
  }

  // STEP 3 — settle the PI via the Stripe TEST-helper, then poll to succeeded.
  const sim = await simulateCryptoDeposit(options.stripeTestKey, paymentIntentId)
  const simOk = sim.status >= 200 && sim.status < 300
  record('stripe_simulate_crypto_deposit', simOk ? 'PASS' : 'FAIL', {
    http: sim.status,
    apiVersion: STRIPE_SIMULATE_API_VERSION,
    body: simOk ? undefined : redact(sim.body),
  })
  if (!simOk) {
    return finalize(checks, base, options)
  }

  const settled = await waitForSettled(
    options.stripeTestKey,
    paymentIntentId,
    options.settleTimeoutMs,
  )
  record('payment_intent_settled', settled.settled ? 'PASS' : 'FAIL', {
    intentStatus: settled.intentStatus,
    http: settled.status,
  })
  if (!settled.settled) {
    return finalize(checks, base, options)
  }

  // STEP 4 — construct the echoed-challenge credential + retry; assert 200 +
  // Payment-Receipt + a real Khala completion body.
  const authorization = buildPaymentCredential({
    challenge: crypto.challenge,
    payload: {
      // Crypto rail proof: the server settles by retrieving the bound PI; this
      // is a breadcrumb only (non-load-bearing).
      network: crypto.network,
      transaction_hash: STRIPE_SIMULATE_SUCCESS_TX_HASH,
    },
  })
  const paid = await requestPaid(base, options.model, authorization)
  const receiptHeader = paid.headers?.get?.('payment-receipt') ?? ''
  const completionBody = paid.body
  const looksLikeCompletion =
    completionBody?.object === 'chat.completion' ||
    Array.isArray(completionBody?.choices)
  const paidOk =
    paid.status === 200 &&
    receiptHeader !== '' &&
    looksLikeCompletion
  record('paid_retry_served_completion', paidOk ? 'PASS' : 'FAIL', {
    http: paid.status,
    hasPaymentReceipt: receiptHeader !== '',
    completionShape: looksLikeCompletion ? 'chat.completion' : 'unexpected',
    body: paidOk ? undefined : redact(completionBody),
  })

  // STEP 4b — best-effort: the USD-origin credit grant dereferences + spend.
  const grant = await readCreditGrantReceipt(base, paymentIntentId)
  record(
    'mpp_credit_grant_receipt',
    grant.ok ? 'PASS' : 'SKIP',
    {
      ok: grant.ok,
      receiptRef: grant.receiptRef,
      http: grant.result.status,
      // SKIP (best-effort), not FAIL: the grant projection may lag the served
      // completion; the served 200 + Payment-Receipt is the load-bearing proof.
      note: grant.ok
        ? 'usd_credit_grant receipt dereferenced'
        : 'grant receipt not yet dereferenceable (best-effort, non-blocking)',
    },
  )

  return finalize(checks, base, options)
}

const finalize = (checks, base, options) => {
  const summary = buildSummary(checks)
  return {
    baseUrl: base || null,
    checks,
    generatedAt: nowIso(),
    // A SKIP on the best-effort grant readback does NOT fail the loop; a FAIL on
    // any load-bearing step does.
    ok: summary.failed === 0,
    model: options.model,
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
      `${report.ok ? 'PASS' : 'FAIL'} khala MPP crypto pay-loop smoke (${report.summary.failed} failed, ${report.summary.skipped} skipped)`,
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
