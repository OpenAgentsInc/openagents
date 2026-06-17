import { MAINNET_MDK_NODE_OPTIONS } from '@moneydevkit/core'
import { MdkNode } from '@moneydevkit/lightning-js'

import {
  paymentDestinationKind,
  treasuryPayoutFailureDiagnostics,
} from './pay-failure.mjs'

const port = Number(process.env.PORT ?? '8080')
const MAX_WAIT_SECS = 50
const SERVICE_TOKEN_HEADER = 'x-tips-buffer-service-token'

const secret = name => {
  const value = process.env[name]?.trim()

  return value === undefined || value === '' ? undefined : value
}

const configuredFlags = () => ({
  accessTokenConfigured: secret('MDK_TIPS_BUFFER_ACCESS_TOKEN') !== undefined,
  mnemonicConfigured: secret('MDK_TIPS_BUFFER_MNEMONIC') !== undefined,
  serviceTokenConfigured: secret('MDK_TIPS_BUFFER_SERVICE_TOKEN') !== undefined,
})

const json = (status, body) =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      'content-type': 'application/json',
    },
  })

// One node per container instance. Started lazily on the first call that
// needs it; the container binding keys a single durable instance, so this is
// the only writer against the tips-buffer VSS state.
let runningNode = null
// Outbound completion has no webhook in lightning-js; outcomes are drained
// from the node event queue into this map and served by paymentId.
const paymentOutcomes = new Map()
// Inbound receipts (donations) drained from the same event queue, keyed by
// payment hash. Held for container lifetime only; callers must tolerate a
// pending answer after a container restart.
const receivedPayments = new Map()

const nodeUnavailableReason = () => {
  const flags = configuredFlags()

  if (!flags.mnemonicConfigured || !flags.accessTokenConfigured) {
    return 'tips_buffer_unconfigured'
  }

  return null
}

const getRunningNode = () => {
  if (runningNode !== null) {
    return runningNode
  }

  const node = new MdkNode({
    ...MAINNET_MDK_NODE_OPTIONS,
    mdkApiKey: secret('MDK_TIPS_BUFFER_ACCESS_TOKEN'),
    mnemonic: secret('MDK_TIPS_BUFFER_MNEMONIC'),
  })

  node.start()
  runningNode = node

  return runningNode
}

const drainPaymentEvents = node => {
  while (true) {
    const event = node.nextEvent()

    if (event === null) {
      return
    }

    // PaymentEventType is a const numeric enum in lightning-js:
    // Claimable=0, Received=1, Failed=2, Sent=3.
    const isSent = event.eventType === 3 || event.eventType === 'Sent'
    const isFailed = event.eventType === 2 || event.eventType === 'Failed'
    const isReceived = event.eventType === 1 || event.eventType === 'Received'

    if ((isSent || isFailed) && typeof event.paymentId === 'string') {
      paymentOutcomes.set(event.paymentId, {
        reason: event.reason ?? null,
        status: isSent ? 'succeeded' : 'failed',
      })
    }

    if (isReceived && typeof event.paymentHash === 'string') {
      receivedPayments.set(event.paymentHash.toLowerCase(), {
        amountSat:
          typeof event.amountMsat === 'number'
            ? Math.floor(event.amountMsat / 1000)
            : null,
      })
    }

    node.ackEvent()
  }
}

const requireServiceToken = request => {
  const expected = secret('MDK_TIPS_BUFFER_SERVICE_TOKEN')

  if (expected === undefined) {
    return json(503, { error: 'tips_buffer_service_token_unconfigured' })
  }

  if (request.headers.get(SERVICE_TOKEN_HEADER) !== expected) {
    return json(403, { error: 'tips_buffer_service_token_invalid' })
  }

  return null
}

const paySnapshot = (node, destination = undefined) => {
  const estimate = node.getMaxSendable()
  const destinationEstimate =
    destination === undefined ? estimate : node.getMaxSendable(destination)

  return {
    balanceSat: node.getBalanceWhileRunning(),
    feeBudgetMsat: estimate === null ? null : estimate.feeBudgetMsat,
    maxSendableSat:
      estimate === null ? null : Math.floor(estimate.amountMsat / 1000),
    destinationFeeBudgetMsat:
      destinationEstimate === null ? null : destinationEstimate.feeBudgetMsat,
    destinationMaxSendableSat:
      destinationEstimate === null
        ? null
        : Math.floor(destinationEstimate.amountMsat / 1000),
  }
}

const safePaySnapshot = (node, destination) => {
  try {
    return paySnapshot(node, destination)
  } catch {
    return {
      balanceSat: null,
      destinationFeeBudgetMsat: null,
      destinationMaxSendableSat: null,
      feeBudgetMsat: null,
      maxSendableSat: null,
    }
  }
}

const balanceDeltaSat = (beforeSnapshot, afterSnapshot) =>
  typeof beforeSnapshot.balanceSat === 'number' &&
  typeof afterSnapshot.balanceSat === 'number'
    ? afterSnapshot.balanceSat - beforeSnapshot.balanceSat
    : null

const preflightCoverageSat = (snapshot, amountSat) =>
  typeof snapshot.destinationMaxSendableSat === 'number'
    ? snapshot.destinationMaxSendableSat - amountSat
    : null

const preflightRouteAvailable = (snapshot, amountSat) =>
  typeof snapshot.destinationMaxSendableSat === 'number'
    ? snapshot.destinationMaxSendableSat >= amountSat
    : null

const balanceResponse = node => {
  const snapshot = paySnapshot(node)

  return json(200, {
    balanceSat: snapshot.balanceSat,
    feeBudgetMsat: snapshot.feeBudgetMsat,
    maxSendableSat: snapshot.maxSendableSat,
  })
}

// Fresh-receiver BOLT12 pays have failed where BOLT11 JIT invoices worked
// (2026-06-10 Tassadar PoC receiver), so funding exposes both rails.
const offerResponse = node => {
  const invoice = node.getVariableAmountJitInvoiceWhileRunning(
    'OpenAgents tips buffer funding',
    3600,
  )

  return json(200, {
    bolt11Invoice: invoice.bolt11,
    bolt11ExpiresAt: invoice.expiresAt,
    bolt12Offer: node.getVariableAmountBolt12OfferWhileRunning(
      'OpenAgents tips buffer funding',
    ),
    nodeId: node.getNodeId(),
  })
}

const payResponse = async (request, node) => {
  let body

  try {
    body = await request.json()
  } catch {
    return json(400, { error: 'invalid_json_body' })
  }

  const destination =
    typeof body?.destination === 'string' ? body.destination.trim() : ''
  const amountSat = Number(body?.amountSat)
  const timeoutSecs = Math.min(
    Number.isFinite(Number(body?.timeoutSecs)) && Number(body.timeoutSecs) > 0
      ? Math.floor(Number(body.timeoutSecs))
      : MAX_WAIT_SECS,
    MAX_WAIT_SECS,
  )

  if (destination === '') {
    return json(400, { error: 'destination_required' })
  }

  if (!Number.isInteger(amountSat) || amountSat <= 0) {
    return json(400, { error: 'amount_sat_must_be_positive_integer' })
  }

  if (destination.toLowerCase().includes(node.getNodeId().toLowerCase())) {
    return json(409, { error: 'tips_buffer_self_pay_refused' })
  }

  const destinationKind = paymentDestinationKind(destination)
  const beforeSnapshot = safePaySnapshot(node, destination)
  const preflightMaxSendableSat = beforeSnapshot.destinationMaxSendableSat

  if (
    beforeSnapshot.destinationMaxSendableSat === null ||
    beforeSnapshot.destinationMaxSendableSat < amountSat
  ) {
    return json(409, {
      balanceSatBefore: beforeSnapshot.balanceSat,
      balanceDeltaSat: null,
      destinationKind,
      error: 'tips_buffer_insufficient_spendable_balance',
      failureStage: 'preflight_max_sendable',
      feeBudgetMsatBefore: beforeSnapshot.destinationFeeBudgetMsat,
      maxSendableSat: preflightMaxSendableSat,
      preflightCoverageSat: preflightCoverageSat(beforeSnapshot, amountSat),
      preflightRouteAvailable: preflightRouteAvailable(
        beforeSnapshot,
        amountSat,
      ),
      reasonClass: 'insufficient_spendable_balance',
      reasonRef: 'reason.public.treasury_payout.insufficient_spendable_balance',
    })
  }

  let result

  try {
    result = node.payWhileRunning(destination, amountSat * 1000, timeoutSecs)
  } catch (error) {
    const diagnostics = treasuryPayoutFailureDiagnostics(error)
    const afterSnapshot = safePaySnapshot(node, destination)
    const balanceChanged =
      typeof beforeSnapshot.balanceSat === 'number' &&
      typeof afterSnapshot.balanceSat === 'number'
        ? beforeSnapshot.balanceSat !== afterSnapshot.balanceSat
        : null
    const balanceDelta = balanceDeltaSat(beforeSnapshot, afterSnapshot)
    const routeAvailable = preflightRouteAvailable(beforeSnapshot, amountSat)
    const routeCoverage = preflightCoverageSat(beforeSnapshot, amountSat)

    console.warn({
      amountSat,
      balanceChanged,
      balanceDeltaSat: balanceDelta,
      balanceSatAfter: afterSnapshot.balanceSat,
      balanceSatBefore: beforeSnapshot.balanceSat,
      destinationKind,
      errorCode: diagnostics.errorCode,
      errorName: diagnostics.errorName,
      event: 'tips_buffer_pay_failed',
      failureStage: 'pay_throws',
      feeBudgetMsatAfter: afterSnapshot.destinationFeeBudgetMsat,
      feeBudgetMsatBefore: beforeSnapshot.destinationFeeBudgetMsat,
      messageFingerprint: diagnostics.messageFingerprint,
      paymentIdPresent: false,
      preflightBalanceMaxSendableSat: beforeSnapshot.maxSendableSat,
      preflightCoverageSat: routeCoverage,
      preflightMaxSendableSat,
      preflightRouteAvailable: routeAvailable,
      resultReturned: false,
      reasonClass: diagnostics.reasonClass,
      service: 'openagents-mdk-tips-buffer',
      timeoutSecs,
    })

    return json(502, {
      amountSat,
      balanceChanged,
      balanceDeltaSat: balanceDelta,
      balanceSatAfter: afterSnapshot.balanceSat,
      balanceSatBefore: beforeSnapshot.balanceSat,
      destinationKind,
      errorCode: diagnostics.errorCode,
      errorName: diagnostics.errorName,
      error: 'tips_buffer_pay_failed',
      failureStage: 'pay_throws',
      feeBudgetMsatAfter: afterSnapshot.destinationFeeBudgetMsat,
      feeBudgetMsatBefore: beforeSnapshot.destinationFeeBudgetMsat,
      messageFingerprint: diagnostics.messageFingerprint,
      paymentIdPresent: false,
      preflightBalanceMaxSendableSat: beforeSnapshot.maxSendableSat,
      preflightCoverageSat: routeCoverage,
      preflightMaxSendableSat,
      preflightRouteAvailable: routeAvailable,
      reason: error instanceof Error ? error.message : String(error),
      reasonClass: diagnostics.reasonClass,
      reasonRef: diagnostics.reasonRef,
      resultReturned: false,
      timeoutSecs,
    })
  }

  drainPaymentEvents(node)
  const outcome = paymentOutcomes.get(result.paymentId)
  const afterSnapshot = safePaySnapshot(node, destination)
  const status =
    result.preimage !== undefined && result.preimage !== null
      ? 'succeeded'
      : (outcome?.status ?? 'pending')
  const balanceChanged =
    typeof beforeSnapshot.balanceSat === 'number' &&
    typeof afterSnapshot.balanceSat === 'number'
      ? beforeSnapshot.balanceSat !== afterSnapshot.balanceSat
      : null
  const responsePayload = {
    balanceChanged,
    balanceDeltaSat: balanceDeltaSat(beforeSnapshot, afterSnapshot),
    balanceSatAfter: afterSnapshot.balanceSat,
    balanceSatBefore: beforeSnapshot.balanceSat,
    destinationKind,
    eventOutcomeStatus: outcome?.status ?? null,
    feeBudgetMsatAfter: afterSnapshot.destinationFeeBudgetMsat,
    feeBudgetMsatBefore: beforeSnapshot.destinationFeeBudgetMsat,
    paymentHash: result.paymentHash ?? null,
    paymentHashPresent: typeof result.paymentHash === 'string',
    paymentId: result.paymentId,
    paymentIdPresent: typeof result.paymentId === 'string',
    preflightBalanceMaxSendableSat: beforeSnapshot.maxSendableSat,
    preflightCoverageSat: preflightCoverageSat(beforeSnapshot, amountSat),
    preflightMaxSendableSat,
    preflightRouteAvailable: preflightRouteAvailable(
      beforeSnapshot,
      amountSat,
    ),
    preimage: result.preimage ?? null,
    preimagePresent:
      result.preimage !== undefined && result.preimage !== null,
    resultReturned: true,
    status,
    timeoutSecs,
  }

  console.info({
    amountSat,
    balanceChanged,
    balanceDeltaSat: responsePayload.balanceDeltaSat,
    destinationKind,
    event: 'tips_buffer_pay_completed',
    eventOutcomeStatus: responsePayload.eventOutcomeStatus,
    paymentHashPresent: responsePayload.paymentHashPresent,
    paymentIdPresent: responsePayload.paymentIdPresent,
    preflightBalanceMaxSendableSat: beforeSnapshot.maxSendableSat,
    preflightCoverageSat: responsePayload.preflightCoverageSat,
    preflightMaxSendableSat,
    preflightRouteAvailable: responsePayload.preflightRouteAvailable,
    preimagePresent: responsePayload.preimagePresent,
    service: 'openagents-mdk-tips-buffer',
    status,
    timeoutSecs,
  })

  return json(200, responsePayload)
}

const paymentStatusResponse = (node, paymentId) => {
  drainPaymentEvents(node)
  const outcome = paymentOutcomes.get(paymentId)

  return json(200, {
    paymentId,
    reason: outcome?.reason ?? null,
    status: outcome?.status ?? 'pending',
  })
}

const handleRequest = async request => {
  const url = new URL(request.url)

  if (request.method === 'GET' && url.pathname === '/healthz') {
    return json(200, {
      ok: true,
      service: 'openagents-mdk-tips-buffer',
      ...configuredFlags(),
    })
  }

  const authFailure = requireServiceToken(request)

  if (authFailure !== null) {
    return authFailure
  }

  const unavailable = nodeUnavailableReason()

  if (unavailable !== null) {
    return json(503, { error: unavailable })
  }

  const node = getRunningNode()

  if (request.method === 'GET' && url.pathname === '/balance') {
    return balanceResponse(node)
  }

  if (request.method === 'GET' && url.pathname === '/offer') {
    return offerResponse(node)
  }

  if (request.method === 'POST' && url.pathname === '/donation-invoice') {
    const invoice = node.getVariableAmountJitInvoiceWhileRunning(
      'OpenAgents tips buffer donation',
      3600,
    )

    return json(200, {
      bolt11: invoice.bolt11,
      expiresAt: invoice.expiresAt,
      paymentHash: invoice.paymentHash,
    })
  }

  const receivedMatch = /^\/received\/([a-f0-9]{64})$/i.exec(url.pathname)

  if (request.method === 'GET' && receivedMatch !== null) {
    drainPaymentEvents(node)
    const entry = receivedPayments.get(receivedMatch[1].toLowerCase())

    return json(200, {
      amountSat: entry?.amountSat ?? null,
      received: entry !== undefined,
    })
  }

  if (request.method === 'POST' && url.pathname === '/pay') {
    return payResponse(request, node)
  }

  const paymentMatch = /^\/payments\/([^/]+)$/.exec(url.pathname)

  if (request.method === 'GET' && paymentMatch !== null) {
    return paymentStatusResponse(node, decodeURIComponent(paymentMatch[1]))
  }

  return json(404, { error: 'not_found' })
}

const Runtime = defineRuntime()
const server = await Runtime.serve({
  fetch: handleRequest,
  port,
})

console.log(`openagents-mdk-tips-buffer listening on ${server.url}`)

function defineRuntime() {
  if (typeof globalThis.Bun !== 'undefined') {
    return globalThis.Bun
  }

  return {
    serve: ({ fetch, port: listenPort }) => nodeServe(fetch, listenPort),
  }
}

async function nodeServe(fetch, listenPort) {
  const http = await import('node:http')
  const server = http.createServer(async (incoming, outgoing) => {
    const protocol = incoming.headers['x-forwarded-proto'] ?? 'http'
    const host = incoming.headers.host ?? `localhost:${listenPort}`
    const url = `${protocol}://${host}${incoming.url ?? '/'}`
    const chunks = []

    for await (const chunk of incoming) {
      chunks.push(chunk)
    }

    const body =
      chunks.length === 0 || incoming.method === 'GET'
        ? undefined
        : Buffer.concat(chunks)
    const headers = new Headers()

    Object.entries(incoming.headers).forEach(([key, value]) => {
      if (Array.isArray(value)) {
        headers.set(key, value.join(', '))
      } else if (typeof value === 'string') {
        headers.set(key, value)
      }
    })

    const request = new Request(url, {
      body,
      headers,
      method: incoming.method,
    })

    try {
      const response = await fetch(request)
      outgoing.writeHead(response.status, Object.fromEntries(response.headers))
      if (response.body) {
        const reader = response.body.getReader()
        while (true) {
          const { done, value } = await reader.read()
          if (done) {
            break
          }
          outgoing.write(value)
        }
      }
      outgoing.end()
    } catch (error) {
      outgoing.writeHead(500, { 'content-type': 'application/json' })
      outgoing.end(
        JSON.stringify({
          error: 'tips_buffer_error',
          message: error instanceof Error ? error.message : String(error),
        }),
      )
    }
  })

  await new Promise(resolve => server.listen(listenPort, resolve))

  return {
    url: new URL(`http://localhost:${listenPort}`),
  }
}
