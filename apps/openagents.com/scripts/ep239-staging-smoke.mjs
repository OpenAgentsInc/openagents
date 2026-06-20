#!/usr/bin/env node

// Episode 239 "Let's Make Money" — push-button staging funded-loop smoke.
//
// Verifies the whole Ep239 revenue loop end-to-end against the ISOLATED
// `openagents-staging` Worker with TEST data and prints a PASS/FAIL/SKIP
// receipt-bearing report. The owner (or a forum verifier like Orrery) runs it
// with one command. It is the repeatable Phase-1 evidence the eventual green
// flip will cite.
//
// Runbook: docs/launch/2026-06-19-ep239-staging-test-plan.md
//
// HARD RULES (enforced by this script):
//   - Hits ONLY the staging Worker. Production is never touched.
//   - NEVER prints a token/secret. The staging admin token is read from env
//     (OPENAGENTS_ADMIN_API_TOKEN) only; if unset, the funded legs are marked
//     SKIPPED (not FAILED) with the exact owner command to unblock them.
//   - Honest SKIPs over fake passes. It does NOT flip any promise green and
//     never touches the promise registry.
//   - All refs (chatcmpl ids, receipt refs, ftjob/sbx ids) are printed so each
//     leg is dereferenceable. Any token-shaped value is redacted in output.
//
// Usage:
//   bun apps/openagents.com/scripts/ep239-staging-smoke.mjs
//   node apps/openagents.com/scripts/ep239-staging-smoke.mjs
//   # optional overrides:
//   #   --base-url <url>       (default openagents-staging.openagents.workers.dev)
//   #   --json                 (emit a machine-readable JSON report to stdout tail)
//   #   --require-complete     (exit non-zero unless the #5520 gate is complete)
//   #   --stripe-checkout-session-id <cs_test_...>
//   #   --stripe-checkout-receipt-ref <receipt.billing.stripe_checkout.cs_test_...>
//   #   --referral-payout-receipt-ref <receipt.site_referral_payout...>
//   #   --help

const DEFAULT_BASE = 'https://openagents-staging.openagents.workers.dev'
const PROD_HOSTS = new Set([
  'openagents.com',
  'www.openagents.com',
  'auth.openagents.com',
])

const ADMIN_ENV = 'OPENAGENTS_ADMIN_API_TOKEN'
const STRIPE_CHECKOUT_SESSION_ENV =
  'OPENAGENTS_STAGING_STRIPE_CHECKOUT_SESSION_ID'
const STRIPE_CHECKOUT_RECEIPT_ENV =
  'OPENAGENTS_STAGING_STRIPE_CHECKOUT_RECEIPT_REF'
const REFERRAL_PAYOUT_RECEIPT_ENV =
  'OPENAGENTS_STAGING_REFERRAL_PAYOUT_RECEIPT_REF'

const EP239_PROMISE_IDS = [
  'sites.referral_bitcoin_stream.v1',
  'referral.refer_once_earn_forever.v1',
  'payments.autopilot_credits_purchase.v1',
  'inference.gateway_credits_business.v1',
  'payments.accepted_outcome_economics.v1',
  'cloud.fine_tuning_service.v1',
  'cloud.sandbox_compute_service.v1',
  'markets.open_protocol_markets.v1',
  'marketplace.compose_and_list_products.v1',
  'marketplace.monetize_any_layer_with_referral.v1',
  'autopilot.all_in_one_business_system.v1',
]

// ---------------------------------------------------------------------------
// arg parsing
// ---------------------------------------------------------------------------

export const parseArgs = argv => {
  const options = {
    baseUrl: process.env.OPENAGENTS_STAGING_BASE_URL || DEFAULT_BASE,
    json: false,
    requireComplete: false,
    help: false,
    referralPayoutReceiptRef: process.env[REFERRAL_PAYOUT_RECEIPT_ENV],
    stripeCheckoutSessionId: process.env[STRIPE_CHECKOUT_SESSION_ENV],
    stripeCheckoutReceiptRef: process.env[STRIPE_CHECKOUT_RECEIPT_ENV],
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
    } else if (value === '--referral-payout-receipt-ref') {
      options.referralPayoutReceiptRef =
        argv[++i] || options.referralPayoutReceiptRef
    } else if (value === '--help' || value === '-h') {
      options.help = true
    } else {
      throw new Error(`Unknown argument: ${value}`)
    }
  }
  return options
}

const HELP = `Episode 239 staging funded-loop smoke harness

Runs each leg of the Ep239 revenue loop against the isolated staging Worker and
prints a PASS/FAIL/SKIP receipt-bearing report.

Usage:
  bun apps/openagents.com/scripts/ep239-staging-smoke.mjs [--base-url <url>] [--json]
  bun apps/openagents.com/scripts/ep239-staging-smoke.mjs --require-complete
  bun apps/openagents.com/scripts/ep239-staging-smoke.mjs --stripe-checkout-session-id cs_test_...
  bun apps/openagents.com/scripts/ep239-staging-smoke.mjs --referral-payout-receipt-ref receipt.site_referral_payout.staging_test...

Environment:
  ${ADMIN_ENV}   staging operator admin token (read from env only, never printed).
                            If unset, the funded-grant + metered-spend legs are
                            SKIPPED (not failed) with the exact owner command.
  ${STRIPE_CHECKOUT_SESSION_ENV}
                            fulfilled Stripe TEST Checkout Session id from the
                            browser test card flow.
  ${STRIPE_CHECKOUT_RECEIPT_ENV}
                            public receipt ref
                            receipt.billing.stripe_checkout.<cs_test_...>.
  ${REFERRAL_PAYOUT_RECEIPT_ENV}
                            settled referral payout receipt ref from the
                            staging/test rail:
                            receipt.site_referral_payout.<...>.

This harness NEVER touches production and NEVER flips a product promise green.

By default, unresolved owner-gated legs are reported as SKIP and the process
exits zero if there are no FAILs. With --require-complete, the process exits
non-zero unless every named #5520 Phase-1 gate is PROVEN.`

// ---------------------------------------------------------------------------
// redaction — defense in depth so no token can leak into the report
// ---------------------------------------------------------------------------

// Anything token/secret shaped is scrubbed from any string we print.
const REDACTION_PATTERNS = [
  /oa_agent_[A-Za-z0-9_-]+/g, // agent tokens
  /\bsk-[A-Za-z0-9_-]{8,}/g, // stripe / openai-style secrets
  /\bBearer\s+[A-Za-z0-9._-]+/gi, // auth headers
]

export const redact = input => {
  if (input === null || input === undefined) return input
  let text = typeof input === 'string' ? input : JSON.stringify(input)
  for (const pattern of REDACTION_PATTERNS) {
    text = text.replace(pattern, '[REDACTED]')
  }
  return text
}

// Print only a short, non-secret fingerprint of a value so a reader can confirm
// "a token was present" without the value ever hitting output.
export const presenceTag = value =>
  typeof value === 'string' && value.length > 0
    ? `present(len=${value.length})`
    : 'absent'

// ---------------------------------------------------------------------------
// http helpers
// ---------------------------------------------------------------------------

export const assertStagingHost = baseUrl => {
  let host
  try {
    host = new URL(baseUrl).host
  } catch {
    throw new Error(`Invalid --base-url: ${baseUrl}`)
  }
  if (PROD_HOSTS.has(host)) {
    throw new Error(
      `Refusing to run against production host "${host}". ` +
        `This harness is staging-only; use the staging Worker URL.`,
    )
  }
  return host
}

const requestJson = async (baseUrl, path, init = {}) => {
  const url = `${baseUrl}${path}`
  const started = Date.now()
  let response
  try {
    response = await fetch(url, init)
  } catch (error) {
    return {
      ok: false,
      transportError: redact(error?.message ?? String(error)),
      status: 0,
      ms: Date.now() - started,
      body: undefined,
      path,
    }
  }
  const text = await response.text()
  let body
  try {
    body = text === '' ? undefined : JSON.parse(text)
  } catch {
    body = { _nonJsonBody: redact(text).slice(0, 400) }
  }
  return {
    ok: response.ok,
    status: response.status,
    ms: Date.now() - started,
    body,
    path,
  }
}

const unique = prefix =>
  `${prefix}-${Date.now()}-${Math.floor(Math.random() * 1e6)}`

const readPublicInferenceReceipt = async (base, receiptRef, expectedKind) => {
  const result = await requestJson(
    base,
    `/api/public/inference/receipts/${encodeURIComponent(receiptRef)}`,
    { method: 'GET' },
  )
  const receipt = result.body?.receipt
  const ok =
    result.status === 200 &&
    receipt?.receiptRef === receiptRef &&
    receipt?.kind === expectedKind &&
    receipt?.ledgerState === 'paid' &&
    receipt?.schemaVersion === 'openagents.inference.receipt.v1'

  return { ok, result }
}

export const stripeCheckoutReceiptRefForSession = sessionId =>
  `receipt.billing.stripe_checkout.${sessionId}`

const readPublicStripeCheckoutReceipt = async (base, receiptRef) => {
  const result = await requestJson(
    base,
    `/api/public/billing/stripe-checkout-receipts/${encodeURIComponent(
      receiptRef,
    )}`,
    { method: 'GET' },
  )
  const receipt = result.body?.receipt
  const resolution = receipt?.resolution
  const ok =
    result.status === 200 &&
    receipt?.receiptRef === receiptRef &&
    receipt?.schemaVersion ===
      'openagents.billing.stripe_checkout_receipt.v1' &&
    resolution?.status === 'ok' &&
    resolution?.sessionMode === 'test' &&
    resolution?.paymentState === 'paid' &&
    resolution?.fulfillmentState === 'fulfilled' &&
    resolution?.creditLedgerState === 'credited'

  return { ok, result }
}

const readPublicSiteReferralPayoutReceipt = async (base, receiptRef) => {
  const result = await requestJson(
    base,
    `/api/public/site-referral-payout-receipts/${encodeURIComponent(
      receiptRef,
    )}`,
    { method: 'GET' },
  )
  const receipt = result.body?.receipt
  const resolution = receipt?.resolution
  const ok =
    result.status === 200 &&
    receipt?.receiptRef === receiptRef &&
    receipt?.schemaVersion ===
      'openagents.site_referral_payout_receipt.v1' &&
    receipt?.attributionLinked === true &&
    typeof receipt?.qualifyingEventKind === 'string' &&
    receipt.qualifyingEventKind.length > 0 &&
    resolution?.status === 'ok' &&
    resolution?.state === 'settled'

  return { ok, result }
}

// ---------------------------------------------------------------------------
// report model
// ---------------------------------------------------------------------------

const legs = []
const acceptanceGates = []

const record = (name, state, detail) => {
  // state: 'PASS' | 'FAIL' | 'SKIP'
  const entry = { name, state, ...detail }
  legs.push(entry)
  return entry
}

export const buildAcceptanceGateSummary = input => {
  const proven = 'PROVEN'
  const unproven = 'UNPROVEN'
  const gates = [
    {
      id: 'card_to_credit_stripe_test',
      status: input.stripeTestCardCheckoutProven ? proven : unproven,
      evidenceRefs: input.stripeTestCardCheckoutRefs ?? [],
      blockerRefs: input.stripeTestCardCheckoutProven
        ? []
        : [
            'blocker.ep239_phase1.stripe_test_checkout_not_exercised',
            'blocker.ep239_phase1.webhook_credit_landing_not_proven',
          ],
      note: 'Requires Stripe TEST card -> /api/billing/checkout -> webhook -> credit balance. Operator credit grant is useful but not a substitute for this #5520 leg.',
    },
    {
      id: 'operator_grant_to_credit_bridge',
      status: input.operatorGrantProven ? proven : unproven,
      evidenceRefs: input.operatorGrantRefs ?? [],
      blockerRefs: input.operatorGrantProven
        ? []
        : ['blocker.ep239_phase1.operator_credit_grant_unproven'],
      note: 'Proves the admin-grant USD->msat bridge seam only; it does not prove Stripe checkout.',
    },
    {
      id: 'credit_to_metered_spend',
      status: input.meteredSpendProven ? proven : unproven,
      evidenceRefs: input.meteredSpendRefs ?? [],
      blockerRefs: input.meteredSpendProven
        ? []
        : [
            'blocker.ep239_phase1.metered_decrement_missing',
            'blocker.ep239_phase1.charge_receipt_not_dereferenced',
          ],
      note: 'Requires a balance decrement plus a dereferenceable inference charge receipt. A free-taste Gemini completion is not enough.',
    },
    {
      id: 'referral_accrual_and_test_settlement',
      status: input.referralAccrualProven ? proven : unproven,
      evidenceRefs: input.referralAccrualRefs ?? [],
      blockerRefs: input.referralAccrualProven
        ? []
        : [
            'blocker.ep239_phase1.referral_capture_claim_paid_event_unproven',
            'blocker.ep239_phase1.referral_test_payout_settlement_unproven',
          ],
      note: 'Requires create source -> capture -> claim -> paid event -> cross-category eligibility -> staging/test payout settlement. 401/404 gating only proves the surface exists.',
    },
    {
      id: 'new_surfaces_honest_inert',
      status: input.newSurfacesProven ? proven : unproven,
      evidenceRefs: input.newSurfaceRefs ?? [],
      blockerRefs: input.newSurfacesProven
        ? []
        : ['blocker.ep239_phase1.new_surfaces_not_honestly_exercised'],
      note: 'Requires markets, marketplace, fine-tuning, and sandbox surfaces to return honest inert/scaffold bodies rather than 404s or fake paid results.',
    },
    {
      id: 'promise_honesty_held',
      status: input.promiseHonestyProven ? proven : unproven,
      evidenceRefs: input.promiseHonestyRefs ?? [],
      blockerRefs: input.promiseHonestyProven
        ? []
        : ['blocker.ep239_phase1.promise_honesty_not_asserted'],
      note: 'Requires the staging registry/audit projection to show Ep239 promises have not flipped green without the real receipts.',
    },
  ]
  return {
    complete: gates.every(gate => gate.status === proven),
    gates,
  }
}

const recordAcceptanceGates = gates => {
  acceptanceGates.splice(0, acceptanceGates.length, ...gates)
}

// ---------------------------------------------------------------------------
// legs
// ---------------------------------------------------------------------------

// Leg 1: register a fresh staging agent -> token + userId.
const legRegister = async base => {
  const displayName = `Ep239 Smoke ${unique('agent')}`
  const result = await requestJson(base, '/api/agents/register', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ displayName }),
  })

  const token = result.body?.credential?.token
  const userId = result.body?.user?.id

  const okShape =
    result.status === 201 &&
    typeof token === 'string' &&
    token.startsWith('oa_agent_') &&
    typeof userId === 'string'

  const detail = {
    http: result.status,
    ms: result.ms,
    userId: userId ?? null,
    tokenPresence: presenceTag(token),
    transportError: result.transportError,
  }

  if (!okShape) {
    record('1. register fresh staging agent', 'FAIL', {
      ...detail,
      reason: 'expected HTTP 201 with credential.token (oa_agent_*) + user.id',
      body: redact(result.body),
    })
    return undefined
  }

  record('1. register fresh staging agent', 'PASS', detail)
  return { token, userId }
}

// Leg 2: free inference call -> 200 + chatcmpl id (balance does NOT decrement).
const legFreeInference = async (base, token) => {
  if (!token) {
    record('2. free inference call', 'SKIP', {
      reason: 'no agent token (registration leg did not pass)',
    })
    return undefined
  }

  const before = await requestJson(base, '/api/agents/me/balance', {
    headers: { authorization: `Bearer ${token}` },
  })
  const beforeMsat = before.body?.balance?.availableMsat ?? null

  const result = await requestJson(base, '/v1/chat/completions', {
    method: 'POST',
    headers: {
      authorization: `Bearer ${token}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      model: 'gemini-3.5-flash',
      messages: [
        {
          role: 'user',
          content: 'Reply with exactly STAGING_FREE_OK and nothing else.',
        },
      ],
      max_tokens: 256,
    }),
  })

  const after = await requestJson(base, '/api/agents/me/balance', {
    headers: { authorization: `Bearer ${token}` },
  })
  const afterMsat = after.body?.balance?.availableMsat ?? null

  const chatcmplId = result.body?.id
  const okShape =
    result.status === 200 &&
    typeof chatcmplId === 'string' &&
    chatcmplId.startsWith('chatcmpl')

  const detail = {
    http: result.status,
    ms: result.ms,
    chatcmplId: chatcmplId ?? null,
    model: result.body?.model ?? null,
    usage: result.body?.usage ?? null,
    availableMsatBefore: beforeMsat,
    availableMsatAfter: afterMsat,
    freePoolHeld: beforeMsat === afterMsat,
  }

  if (!okShape) {
    // The gateway may be flag-off on staging; surface that honestly rather than
    // pretending the loop works.
    record('2. free inference call', 'FAIL', {
      ...detail,
      reason:
        'expected HTTP 200 with a chatcmpl id; if 404/503 the inference ' +
        'gateway flag may be off on staging',
      body: redact(result.body),
    })
    return { chatcmplId: null, afterMsat }
  }

  record('2. free inference call', 'PASS', detail)
  return { chatcmplId, afterMsat }
}

// Leg 2b: optional browser-completed Stripe TEST checkout receipt readback.
const legStripeTestCheckoutCredit = async (base, options) => {
  const receiptRef =
    options.stripeCheckoutReceiptRef ||
    (options.stripeCheckoutSessionId
      ? stripeCheckoutReceiptRefForSession(options.stripeCheckoutSessionId)
      : undefined)

  if (receiptRef === undefined || receiptRef.trim() === '') {
    record('2b. Stripe TEST checkout credit receipt', 'SKIP', {
      reason:
        'no fulfilled Stripe TEST Checkout Session supplied; run the browser ' +
        'test-card flow, then re-run with --stripe-checkout-session-id cs_test_...',
    })
    return undefined
  }

  const readback = await readPublicStripeCheckoutReceipt(base, receiptRef)
  const receipt = readback.result.body?.receipt
  const resolution = receipt?.resolution
  const detail = {
    http: readback.result.status,
    ms: readback.result.ms,
    receiptRef,
    resolutionStatus: resolution?.status ?? null,
    sessionMode: resolution?.sessionMode ?? null,
    receiptReadbackHttp: readback.result.status,
  }

  if (!readback.ok) {
    record('2b. Stripe TEST checkout credit receipt', 'FAIL', {
      ...detail,
      reason:
        'expected a public Stripe checkout receipt with resolution.status ok, ' +
        'sessionMode test, paymentState paid, fulfillmentState fulfilled, and ' +
        'creditLedgerState credited',
      receiptReadbackBody: redact(readback.result.body),
    })
    return {
      proven: false,
      receiptRef,
    }
  }

  record('2b. Stripe TEST checkout credit receipt', 'PASS', detail)
  return {
    proven: true,
    receiptRef,
  }
}

// Leg 3: funded grant via operator credit-grant (admin-token gated; SKIP if unset).
const legFundedGrant = async (base, userId) => {
  const adminToken = process.env[ADMIN_ENV]
  const grantRef = unique('ep239-stg')

  const ownerCommand =
    'wrangler secret put OPENAGENTS_ADMIN_API_TOKEN --env staging   ' +
    `# then re-run with ${ADMIN_ENV} set in env`

  if (!adminToken || adminToken.trim() === '') {
    record('3. funded grant (operator credit)', 'SKIP', {
      reason: `${ADMIN_ENV} not set in env (owner-gated)`,
      ownerAction: ownerCommand,
    })
    return undefined
  }
  if (!userId) {
    record('3. funded grant (operator credit)', 'SKIP', {
      reason: 'no agent userId (registration leg did not pass)',
    })
    return undefined
  }

  const result = await requestJson(
    base,
    '/api/omni/operator/billing/inference-credit',
    {
      method: 'POST',
      headers: {
        authorization: `Bearer ${adminToken}`,
        'content-type': 'application/json',
      },
      body: JSON.stringify({ userId, amountCents: 1000, grantRef }),
    },
  )

  const receiptRef = result.body?.receiptRef
  const okShape =
    result.status === 200 &&
    result.body?.status === 'inference_credit_granted' &&
    typeof receiptRef === 'string'

  const detail = {
    http: result.status,
    ms: result.ms,
    grantRef,
    receiptRef: receiptRef ?? null,
    grantedCents: result.body?.grantedCents ?? null,
    grantedMsat: result.body?.grantedMsat ?? null,
    adminTokenPresence: presenceTag(adminToken),
  }

  if (result.status === 401) {
    // The route is live (deployed) but rejects this token. On staging this means
    // the env admin token does not match the staging secret (e.g. a prod token).
    record('3. funded grant (operator credit)', 'FAIL', {
      ...detail,
      reason:
        '401 unauthorized — route is live but the env admin token is not the ' +
        'staging admin token (prod token is rejected by design)',
      ownerAction: ownerCommand,
    })
    return undefined
  }

  if (!okShape) {
    record('3. funded grant (operator credit)', 'FAIL', {
      ...detail,
      reason:
        'expected HTTP 200 + status inference_credit_granted + receiptRef',
      body: redact(result.body),
    })
    return undefined
  }

  const readback = await readPublicInferenceReceipt(
    base,
    receiptRef,
    'usd_credit_grant',
  )
  if (!readback.ok) {
    record('3. funded grant (operator credit)', 'FAIL', {
      ...detail,
      reason: 'grant receipt was minted but was not publicly dereferenceable',
      receiptReadbackHttp: readback.result.status,
      receiptReadbackBody: redact(readback.result.body),
    })
    return undefined
  }

  record('3. funded grant (operator credit)', 'PASS', {
    ...detail,
    receiptReadbackHttp: readback.result.status,
  })
  return { receiptRef, grantedMsat: result.body?.grantedMsat ?? null }
}

// Leg 4: metered spend -> balance decrement + a receipt.inference.charge.* row.
// Requires a funded balance, so it depends on leg 3 (admin-gated).
const legMeteredSpend = async (base, token, grant) => {
  const adminTokenSet =
    typeof process.env[ADMIN_ENV] === 'string' &&
    process.env[ADMIN_ENV].trim() !== ''

  if (!token) {
    record('4. metered spend (decrement + charge receipt)', 'SKIP', {
      reason: 'no agent token (registration leg did not pass)',
    })
    return
  }
  if (!adminTokenSet || !grant) {
    record('4. metered spend (decrement + charge receipt)', 'SKIP', {
      reason:
        'depends on the funded-grant leg, which is owner-gated (admin token ' +
        'unset or grant did not land)',
      ownerAction:
        'set OPENAGENTS_ADMIN_API_TOKEN (staging) so leg 3 funds the balance',
    })
    return
  }

  const before = await requestJson(base, '/api/agents/me/balance', {
    headers: { authorization: `Bearer ${token}` },
  })
  const beforeMsat = before.body?.balance?.availableMsat ?? null

  // A large Gemini request: once the free taste is exhausted by the funded grant
  // path it meters; the charge is receipt-first and idempotent per chatcmpl id.
  const result = await requestJson(base, '/v1/chat/completions', {
    method: 'POST',
    headers: {
      authorization: `Bearer ${token}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      model: 'gemini-3.5-flash',
      messages: [
        {
          role: 'user',
          content: 'Write a 400-word essay about verification by replay.',
        },
      ],
      max_tokens: 1200,
    }),
  })

  const after = await requestJson(base, '/api/agents/me/balance', {
    headers: { authorization: `Bearer ${token}` },
  })
  const afterMsat = after.body?.balance?.availableMsat ?? null

  const chatcmplId = result.body?.id
  const chargeReceiptRef =
    typeof chatcmplId === 'string'
      ? `receipt.inference.charge.${chatcmplId}`
      : null

  // The decrement is the load-bearing assertion. The free taste is large, so a
  // single grant-funded call may still be eaten by the free pool — report that
  // honestly as an INFO-bearing PASS-conditional rather than a fake decrement.
  const decremented =
    typeof beforeMsat === 'number' &&
    typeof afterMsat === 'number' &&
    afterMsat < beforeMsat

  const detail = {
    http: result.status,
    ms: result.ms,
    chatcmplId: chatcmplId ?? null,
    chargeReceiptRef,
    availableMsatBefore: beforeMsat,
    availableMsatAfter: afterMsat,
    deltaMsat:
      typeof beforeMsat === 'number' && typeof afterMsat === 'number'
        ? afterMsat - beforeMsat
        : null,
  }

  if (result.status !== 200 || typeof chatcmplId !== 'string') {
    record('4. metered spend (decrement + charge receipt)', 'FAIL', {
      ...detail,
      reason: 'expected HTTP 200 + chatcmpl id from the metering request',
      body: redact(result.body),
    })
    return
  }

  if (decremented) {
    const readback =
      typeof chargeReceiptRef === 'string'
        ? await readPublicInferenceReceipt(base, chargeReceiptRef, 'charge')
        : { ok: false, result: { status: 0, body: null } }

    if (!readback.ok) {
      record('4. metered spend (decrement + charge receipt)', 'FAIL', {
        ...detail,
        reason:
          'balance decremented, but the charge receipt was not publicly dereferenceable',
        receiptReadbackHttp: readback.result.status,
        receiptReadbackBody: redact(readback.result.body),
      })
      return undefined
    }

    record('4. metered spend (decrement + charge receipt)', 'PASS', {
      ...detail,
      receiptReadbackHttp: readback.result.status,
    })
    return { chargeReceiptRef, chatcmplId, receiptDereferenced: true }
  }

  // 200 + chatcmpl but no decrement: the free taste/allowance covered it. This is
  // expected per the test plan (taste covers thousands of Gemini Flash calls),
  // so it is an honest SKIP of the *decrement assertion*, not a fake pass.
  record('4. metered spend (decrement + charge receipt)', 'SKIP', {
    ...detail,
    reason:
      'completion succeeded but the free taste/allowance absorbed the charge ' +
      '(no decrement) — per the plan, free taste covers thousands of Gemini ' +
      'Flash calls; exhaust the taste or use a premium allowlisted model to ' +
      'force a metered decrement',
  })
  return undefined
}

// Leg 5: referral attribution capture + cross-category accrual eligibility gate.
// The full accrual chain is browser-session-driven (create source -> capture ->
// claim -> paid event -> dashboard); headlessly we confirm the live (401/404)
// gating + capture-redirect behavior, which is the agent-testable surface today.
const legReferral = async (base, token, options = {}) => {
  // Capture: an UNKNOWN source returns 404 by design (live, not absent).
  const unknownSource = unique('unknown-src')
  const capture = await requestJson(
    base,
    `/r/site/${encodeURIComponent(unknownSource)}?target=order`,
    { method: 'GET', redirect: 'manual' },
  )

  // Dashboard: bare agent token returns 401 (live, browser-session required).
  const dashboard = await requestJson(
    base,
    '/api/inference/referral/dashboard',
    token ? { headers: { authorization: `Bearer ${token}` } } : {},
  )

  const captureLive = capture.status === 404
  const dashboardGated = dashboard.status === 401

  const receiptRef =
    typeof options.referralPayoutReceiptRef === 'string' &&
    options.referralPayoutReceiptRef.trim() !== ''
      ? options.referralPayoutReceiptRef.trim()
      : null

  const receiptReadback =
    receiptRef === null
      ? null
      : await readPublicSiteReferralPayoutReceipt(base, receiptRef)

  const receipt = receiptReadback?.result.body?.receipt
  const resolution = receipt?.resolution
  const receiptProven = Boolean(receiptReadback?.ok)

  const detail = {
    captureHttp: capture.status,
    captureExpected: '404 (unknown source rejected by design)',
    dashboardHttp: dashboard.status,
    dashboardExpected: '401 (browser-session required; live, not 404)',
    payoutReceiptRef: receiptRef,
    payoutReceiptHttp: receiptReadback?.result.status ?? null,
    payoutReceiptResolution: resolution?.status ?? null,
    payoutReceiptState: resolution?.state ?? null,
    payoutSettlementRail: resolution?.settlementRail ?? null,
    qualifyingEventKind: receipt?.qualifyingEventKind ?? null,
    note:
      'full cross-category accrual (create source -> capture -> claim -> paid ' +
      'event -> dashboard) is browser-session-driven and owner-gated; the ' +
      'live gating above is the headless-testable surface unless a settled ' +
      'public referral payout receipt is supplied',
  }

  if (captureLive && dashboardGated) {
    record('5. referral attribution + accrual gating', 'PASS', detail)
    return {
      fullAccrualProven: receiptProven,
      gatingLive: true,
      receiptRef: receiptProven ? receiptRef : null,
    }
  }

  // If the dashboard route is not present (404), that is a real surface gap.
  record('5. referral attribution + accrual gating', 'FAIL', {
    ...detail,
    reason:
      'expected capture 404 (unknown source) AND dashboard 401 (bare token); ' +
      'a 404 on the dashboard would mean the referral surface is missing',
  })
  return { gatingLive: false, fullAccrualProven: false }
}

// Leg 6: new Ep239 surfaces return honest inert/scaffold responses (not 404).
const legNewSurfaces = async (base, token) => {
  const checks = []

  const markets = [
    ['/api/public/markets/open-markets', 'markets.open-markets'],
    ['/api/public/markets/liquidity/skeleton', 'markets.liquidity'],
    ['/api/public/markets/risk/skeleton', 'markets.risk'],
  ]
  for (const [path, label] of markets) {
    const r = await requestJson(base, path, { method: 'GET' })
    checks.push({
      surface: label,
      path,
      http: r.status,
      pass: r.status === 200 && r.body !== undefined,
      detail: r.status === 200 ? 'live read-only projection' : redact(r.body),
    })
  }

  // Marketplace compose-and-list: 200 + inert:true, products:[].
  const marketplace = await requestJson(
    base,
    '/api/public/marketplace/composed-products',
    { method: 'GET' },
  )
  const marketplaceHonest =
    marketplace.status === 200 &&
    marketplace.body?.inert === true &&
    Array.isArray(marketplace.body?.products) &&
    marketplace.body.products.length === 0
  checks.push({
    surface: 'marketplace.composed-products',
    path: '/api/public/marketplace/composed-products',
    http: marketplace.status,
    pass: marketplaceHonest,
    detail: marketplaceHonest
      ? `inert:true promiseState:${marketplace.body?.promiseState ?? '?'} products:[]`
      : redact(marketplace.body),
  })

  if (token) {
    // Fine-tuning scaffold: 200 + status queued, metered:false, receiptRef null.
    const ft = await requestJson(base, '/v1/fine_tuning/jobs', {
      method: 'POST',
      headers: {
        authorization: `Bearer ${token}`,
        'content-type': 'application/json',
      },
      body: JSON.stringify({
        baseModel: 'gemini-3.5-flash',
        datasetRef: 'dataset-ep239',
        suffix: 'smoke',
      }),
    })
    // Route emits OpenAI-shaped fields: `id` (ftjob_*) + snake_case receipt_ref.
    const ftId = ft.body?.id ?? ft.body?.jobId ?? null
    // receipt_ref must be explicitly null (no charge). `??` would mask a null,
    // so read the snake_case field (the route's shape) directly.
    const ftReceiptNull =
      ft.body?.receipt_ref === null || ft.body?.receiptRef === null
    const ftHonest =
      ft.status === 200 &&
      ft.body?.metered === false &&
      ftReceiptNull &&
      typeof ftId === 'string' &&
      ftId.startsWith('ftjob')
    checks.push({
      surface: 'fine-tuning.jobs',
      path: '/v1/fine_tuning/jobs',
      http: ft.status,
      pass: ftHonest,
      ref: ftId,
      detail: ftHonest
        ? `id:${ftId} status:${ft.body?.status ?? '?'} metered:false receipt_ref:null`
        : redact(ft.body),
    })

    // Sandbox scaffold: 200 + status provisioning, metered:false, receiptRef null.
    const sbx = await requestJson(base, '/v1/sandboxes', {
      method: 'POST',
      headers: {
        authorization: `Bearer ${token}`,
        'content-type': 'application/json',
      },
      body: JSON.stringify({ image: 'python:3.12', command: 'echo hi' }),
    })
    const sbxId = sbx.body?.id ?? sbx.body?.sandboxId ?? null
    const sbxReceiptNull =
      sbx.body?.receipt_ref === null || sbx.body?.receiptRef === null
    const sbxHonest =
      sbx.status === 200 &&
      sbx.body?.metered === false &&
      sbxReceiptNull &&
      typeof sbxId === 'string' &&
      sbxId.startsWith('sbx')
    checks.push({
      surface: 'sandboxes',
      path: '/v1/sandboxes',
      http: sbx.status,
      pass: sbxHonest,
      ref: sbxId,
      detail: sbxHonest
        ? `id:${sbxId} status:${sbx.body?.status ?? '?'} metered:false receipt_ref:null`
        : redact(sbx.body),
    })
  } else {
    checks.push({
      surface: 'fine-tuning.jobs / sandboxes',
      pass: false,
      skipped: true,
      detail: 'no agent token (registration leg did not pass)',
    })
  }

  const allPass = checks.every(c => c.pass)
  record(
    '6. new Ep239 surfaces (honest inert/scaffold)',
    allPass ? 'PASS' : 'FAIL',
    {
      checks,
    },
  )
  return { allPass, refs: checks.map(c => c.ref).filter(Boolean) }
}

// Leg 7: promise honesty — the staging registry must not accidentally green any
// Ep239 promise while the money/referral receipts are still unproven.
const legPromiseHonesty = async base => {
  const registry = await requestJson(base, '/api/public/product-promises', {
    method: 'GET',
  })
  const audit = await requestJson(base, '/api/public/product-promises/audit', {
    method: 'GET',
  })

  const promises = Array.isArray(registry.body?.promises)
    ? registry.body.promises
    : []
  const presentTargets = promises.filter(p =>
    EP239_PROMISE_IDS.includes(p.promiseId),
  )
  const greenTargets = presentTargets.filter(p => p.state === 'green')
  const missingTargets = EP239_PROMISE_IDS.filter(
    promiseId => !presentTargets.some(p => p.promiseId === promiseId),
  )
  const auditAvailable = audit.status === 200 && audit.body !== undefined
  const auditEvidenceRef = auditAvailable
    ? `registry:${audit.body?.registryVersion ?? registry.body?.registryVersion ?? 'unknown'}`
    : null

  const detail = {
    registryHttp: registry.status,
    auditHttp: audit.status,
    registryVersion: registry.body?.registryVersion ?? null,
    generatedAt: registry.body?.generatedAt ?? null,
    presentTargets: presentTargets.map(p => ({
      promiseId: p.promiseId,
      state: p.state,
    })),
    missingTargets,
    auditSummary: auditAvailable ? (audit.body?.summary ?? null) : null,
  }

  if (registry.status !== 200 || !Array.isArray(registry.body?.promises)) {
    record('7. Ep239 promise honesty held', 'FAIL', {
      ...detail,
      reason: 'expected public product-promise registry JSON',
      body: redact(registry.body),
    })
    return { proven: false, refs: [] }
  }

  if (greenTargets.length > 0) {
    record('7. Ep239 promise honesty held', 'FAIL', {
      ...detail,
      reason:
        'one or more Ep239 target promises are green on staging; #5520 must not ' +
        'green these without the named money/referral receipts',
      greenTargets: greenTargets.map(p => p.promiseId),
    })
    return { proven: false, refs: [] }
  }

  record('7. Ep239 promise honesty held', 'PASS', {
    ...detail,
    evidenceRef: auditEvidenceRef,
    note:
      missingTargets.length > 0
        ? 'older staging registry is missing newer target records; present Ep239 targets are non-green'
        : 'all tracked Ep239 target promises are present and non-green',
  })
  return { proven: true, refs: [auditEvidenceRef].filter(Boolean) }
}

// ---------------------------------------------------------------------------
// runner
// ---------------------------------------------------------------------------

const STATE_GLYPH = { PASS: 'PASS', FAIL: 'FAIL', SKIP: 'SKIP' }

const printReport = (base, host) => {
  const line = '-'.repeat(72)
  console.log(line)
  console.log('Episode 239 staging funded-loop smoke — receipt-bearing report')
  console.log(`base: ${base}`)
  console.log(`host: ${host} (staging-only; production untouched)`)
  console.log(
    `admin token (${ADMIN_ENV}): ${presenceTag(process.env[ADMIN_ENV])}`,
  )
  console.log(line)

  for (const leg of legs) {
    console.log(`[${STATE_GLYPH[leg.state]}] ${leg.name}`)
    const { name: _n, state: _s, checks, body, ...rest } = leg
    for (const [k, v] of Object.entries(rest)) {
      if (v === undefined || v === null) continue
      console.log(`        ${k}: ${typeof v === 'object' ? redact(v) : v}`)
    }
    if (Array.isArray(checks)) {
      for (const c of checks) {
        const tag = c.skipped ? 'SKIP' : c.pass ? 'ok' : 'BAD'
        console.log(
          `        - [${tag}] ${c.surface}${c.http ? ` (${c.http})` : ''}` +
            `${c.ref ? ` ref:${c.ref}` : ''} ${c.detail ?? ''}`,
        )
      }
    }
    if (body !== undefined) {
      console.log(`        body: ${redact(body)}`)
    }
  }

  console.log(line)
  const counts = legs.reduce(
    (acc, l) => ({ ...acc, [l.state]: (acc[l.state] ?? 0) + 1 }),
    {},
  )
  console.log(
    `summary: PASS=${counts.PASS ?? 0}  FAIL=${counts.FAIL ?? 0}  SKIP=${counts.SKIP ?? 0}`,
  )

  const skips = legs.filter(l => l.state === 'SKIP' && l.ownerAction)
  if (skips.length > 0) {
    console.log(line)
    console.log('Owner action(s) to unblock SKIPped legs:')
    for (const s of skips) {
      console.log(`  - ${s.name}: ${s.ownerAction}`)
    }
  }
  if (acceptanceGates.length > 0) {
    console.log(line)
    const complete = acceptanceGates.every(g => g.status === 'PROVEN')
    console.log(
      `#5520 Phase-1 gate: ${complete ? 'COMPLETE' : 'NOT COMPLETE'} ` +
        '(PROVEN means the named acceptance leg has real evidence)',
    )
    for (const gate of acceptanceGates) {
      console.log(`  - [${gate.status}] ${gate.id}`)
      if (gate.evidenceRefs.length > 0) {
        console.log(`      evidenceRefs: ${gate.evidenceRefs.join(', ')}`)
      }
      if (gate.blockerRefs.length > 0) {
        console.log(`      blockerRefs: ${gate.blockerRefs.join(', ')}`)
      }
      console.log(`      note: ${gate.note}`)
    }
  }
  console.log(line)
}

export const run = async argv => {
  const options = parseArgs(argv)
  if (options.help) {
    console.log(HELP)
    return 0
  }

  const host = assertStagingHost(options.baseUrl)
  const base = options.baseUrl.replace(/\/$/, '')

  const registration = await legRegister(base)
  const token = registration?.token
  const userId = registration?.userId

  const free = await legFreeInference(base, token)
  const stripeCheckout = await legStripeTestCheckoutCredit(base, options)
  const grant = await legFundedGrant(base, userId)
  const meteredSpend = await legMeteredSpend(base, token, grant)
  const referral = await legReferral(base, token, options)
  const newSurfaces = await legNewSurfaces(base, token)
  const promiseHonesty = await legPromiseHonesty(base)

  const phaseGate = buildAcceptanceGateSummary({
    stripeTestCardCheckoutProven: Boolean(stripeCheckout?.proven),
    stripeTestCardCheckoutRefs: stripeCheckout?.receiptRef
      ? [stripeCheckout.receiptRef]
      : [],
    operatorGrantProven: Boolean(grant),
    operatorGrantRefs: grant?.receiptRef ? [grant.receiptRef] : [],
    meteredSpendProven: Boolean(
      meteredSpend?.chargeReceiptRef && meteredSpend?.receiptDereferenced,
    ),
    meteredSpendRefs: meteredSpend?.chargeReceiptRef
      ? [meteredSpend.chargeReceiptRef]
      : [],
    referralAccrualProven: Boolean(referral?.fullAccrualProven),
    referralAccrualRefs: referral?.receiptRef ? [referral.receiptRef] : [],
    newSurfacesProven: Boolean(newSurfaces?.allPass),
    newSurfaceRefs: newSurfaces?.refs ?? [],
    promiseHonestyProven: Boolean(promiseHonesty?.proven),
    promiseHonestyRefs: promiseHonesty?.refs ?? [],
  })
  recordAcceptanceGates(phaseGate.gates)

  printReport(base, host)

  if (options.json) {
    // Machine-readable tail for verifier tooling. Already redacted upstream.
    console.log('JSON_REPORT_BEGIN')
    console.log(redact({ base, host, legs, phaseGate }))
    console.log('JSON_REPORT_END')
  }

  // Exit non-zero ONLY on a real FAIL. SKIP (owner-gated / free-taste-absorbed)
  // is an honest, expected outcome and must not fail the run.
  const hasFail = legs.some(l => l.state === 'FAIL')
  // reference `free` so unused-var linting stays quiet while keeping the value
  // available for future assertions.
  void free
  if (hasFail) return 1
  if (options.requireComplete && !phaseGate.complete) return 3
  return 0
}

const isMain = (() => {
  try {
    return import.meta.url === `file://${process.argv[1]}`
  } catch {
    return false
  }
})()

if (isMain) {
  run(process.argv.slice(2))
    .then(code => process.exit(code))
    .catch(error => {
      console.error(
        `ep239-staging-smoke fatal: ${redact(error?.message ?? String(error))}`,
      )
      process.exit(2)
    })
}
