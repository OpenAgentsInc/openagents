#!/usr/bin/env node

const defaultBaseUrl = 'https://openagents.com'
const defaultApiBasePath = '/v1'
const defaultModel = 'openagents/khala'
const defaultPrompt =
  'OpenAgents Khala production smoke: answer with exactly READY and no other words.'
const defaultExpectedSupplyLane = 'fireworks'
const defaultExpectedWorker = 'fireworks'
const defaultExpectedServedModelContains = 'deepseek-v4-flash'
const forbiddenInfrastructureTerms = [
  'deepseek',
  'fireworks',
  'gpt-oss',
  'hydralisk',
  'provider',
  'vllm',
]
const defaultForbiddenPublicModelIds = [
  'accounts/fireworks/models/deepseek-v4-flash',
  'deepseek-ai/deepseek-v4-flash',
  'deepseek-v4-flash',
  'fireworks/deepseek-v4-flash',
  'gpt-oss-120b',
  'gpt-oss-20b',
  'openai/gpt-oss-120b',
  'openai/gpt-oss-20b',
  'openagents/khala-code',
  'openagents/khala-mini',
  'openagents/khala-oss-20b',
]

const trimBaseUrl = baseUrl =>
  String(baseUrl || defaultBaseUrl).replace(/\/+$/, '')

const absoluteUrl = (baseUrl, pathOrUrl) =>
  new URL(String(pathOrUrl || ''), baseUrl).toString()

const normalizeApiBasePath = value => {
  const raw = String(value || defaultApiBasePath).trim()
  if (raw === '') {
    return defaultApiBasePath
  }
  const withLeadingSlash = raw.startsWith('/') ? raw : `/${raw}`
  return withLeadingSlash.replace(/\/+$/u, '') || defaultApiBasePath
}

const apiPath = (basePath, suffix) =>
  `${normalizeApiBasePath(basePath)}/${String(suffix || '').replace(/^\/+/u, '')}`

const truthy = value =>
  ['1', 'true', 'yes', 'on'].includes(
    String(value || '')
      .trim()
      .toLowerCase(),
  )

const assert = (condition, message) => {
  if (!condition) {
    throw new Error(message)
  }
}

const okStatus = response => response.status >= 200 && response.status < 300

const readJson = async response => {
  const text = await response.text()
  try {
    return text.length === 0 ? null : JSON.parse(text)
  } catch {
    return { rawText: text }
  }
}

const requestJson = async (fetchImpl, baseUrl, path, init = {}) => {
  const response = await fetchImpl(absoluteUrl(baseUrl, path), {
    ...init,
    headers: {
      accept: 'application/json',
      ...(init.headers || {}),
    },
  })
  return { body: await readJson(response), response }
}

const redact = value => {
  if (typeof value !== 'string') {
    return value
  }
  return value
    .replace(/Bearer\s+[A-Za-z0-9._~+/=-]+/gu, 'Bearer <redacted>')
    .replace(/oa_agent_[A-Za-z0-9._~+/=-]+/gu, 'oa_agent_<redacted>')
    .replace(/sk-[A-Za-z0-9]{8,}/gu, 'sk-<redacted>')
}

const csv = value =>
  String(value || '')
    .split(',')
    .map(item => item.trim())
    .filter(Boolean)

const modelIdsFrom = body =>
  Array.isArray(body?.data)
    ? body.data.map(model => model?.id).filter(id => typeof id === 'string')
    : []

const normalize = value =>
  String(value || '')
    .trim()
    .toLowerCase()

const includesInsensitive = (value, needle) =>
  normalize(value).includes(normalize(needle))

const forbiddenCatalogIdsFrom = (modelIds, forbiddenIds) => {
  const forbidden = new Set(forbiddenIds.map(normalize))
  return modelIds.filter(modelId => forbidden.has(normalize(modelId)))
}

const parseSseFrames = text =>
  String(text)
    .split(/\n\n/u)
    .flatMap(chunk =>
      chunk
        .split(/\n/u)
        .filter(line => line.startsWith('data: '))
        .map(line => line.slice('data: '.length).trim()),
    )
    .filter(data => data.length > 0 && data !== '[DONE]')
    .map(data => JSON.parse(data))

const contentFromCompletion = body =>
  body?.choices?.[0]?.message?.content === undefined
    ? ''
    : String(body.choices[0].message.content)

const contentFromSseFrames = frames =>
  frames
    .map(frame => frame?.choices?.[0]?.delta?.content)
    .filter(value => typeof value === 'string')
    .join('')

const infrastructureLeaks = text => {
  const lower = String(text || '').toLowerCase()
  return forbiddenInfrastructureTerms.filter(term => lower.includes(term))
}

const terminalOpenAgentsFrom = frames => {
  for (let index = frames.length - 1; index >= 0; index -= 1) {
    if (typeof frames[index]?.openagents === 'object') {
      return frames[index].openagents
    }
  }
  return null
}

const receiptPathFrom = openagents => {
  if (
    typeof openagents?.receipt_url === 'string' &&
    openagents.receipt_url.length > 0
  ) {
    return openagents.receipt_url
  }
  if (
    typeof openagents?.telemetry?.detailRef === 'string' &&
    openagents.telemetry.detailRef.length > 0
  ) {
    return openagents.telemetry.detailRef
  }
  if (
    typeof openagents?.receipt === 'string' &&
    openagents.receipt.length > 0
  ) {
    return `/api/public/inference/receipts/${encodeURIComponent(openagents.receipt)}`
  }
  return null
}

const operatorCreditReceiptRefPrefix = 'receipt.inference.operator_credit.'

const receiptRefFromReceiptPath = receiptPath => {
  if (typeof receiptPath !== 'string' || receiptPath.length === 0) {
    return null
  }
  const publicReceiptPrefix = '/api/public/inference/receipts/'
  if (receiptPath.startsWith(publicReceiptPrefix)) {
    return decodeURIComponent(receiptPath.slice(publicReceiptPrefix.length))
  }
  try {
    const url = new URL(receiptPath)
    if (url.pathname.startsWith(publicReceiptPrefix)) {
      return decodeURIComponent(url.pathname.slice(publicReceiptPrefix.length))
    }
  } catch {
    // Relative paths are common in the Worker disclosure block.
  }
  return receiptPath.startsWith('receipt.inference.') ? receiptPath : null
}

const operatorCreditReceiptRefFromPath = receiptPath => {
  const receiptRef = receiptRefFromReceiptPath(receiptPath)
  return receiptRef?.startsWith(operatorCreditReceiptRefPrefix)
    ? receiptRef
    : null
}

const summarizeGatewayReadiness = readiness => ({
  hiddenModelCount: readiness?.hiddenModelCount,
  lanes: Array.isArray(readiness?.lanes)
    ? readiness.lanes.map(lane => ({
        armed: lane?.armed,
        hiddenModelCount: lane?.hiddenModelCount,
        lane: lane?.lane,
        servableModelCount: lane?.servableModelCount,
      }))
    : undefined,
  reasonRefs: Array.isArray(readiness?.reasonRefs)
    ? readiness.reasonRefs.filter(ref => typeof ref === 'string')
    : undefined,
  servableModelCount: readiness?.servableModelCount,
  status: readiness?.status,
  totalModelCount: readiness?.totalModelCount,
})

const routingSummaryFrom = openagents => {
  const routing = openagents?.routing
  if (typeof routing !== 'object' || routing === null) {
    return undefined
  }
  return {
    fallback_reason: routing.fallback_reason,
    glm_saturation_policy: routing.glm_saturation_policy,
    provider_health_score: routing.provider_health_score,
    queue_wait_ms: routing.queue_wait_ms,
    region: routing.region,
    replica_busy_reason: routing.replica_busy_reason,
    replica_fallback_reason: routing.replica_fallback_reason,
    replica_health_score: routing.replica_health_score,
    replica_region: routing.replica_region,
    selected_replica_id: routing.selected_replica_id,
    selected_replica_ref: routing.selected_replica_ref,
  }
}

const selectedReplicaRefFrom = openagents => {
  const ref = openagents?.routing?.selected_replica_ref
  return typeof ref === 'string' && ref.length > 0 ? ref : null
}

const summarizeOpenAgents = openagents => ({
  lane: openagents?.lane,
  receipt: openagents?.receipt,
  receipt_url: openagents?.receipt_url,
  requested_model: openagents?.requested_model,
  routing: routingSummaryFrom(openagents),
  served_model: openagents?.served_model,
  supply_lane: openagents?.supply_lane,
  telemetry_detail_ref: openagents?.telemetry?.detailRef,
  worker: openagents?.worker,
})

const receiptProjectionFrom = body =>
  typeof body?.receipt === 'object' && body.receipt !== null
    ? body.receipt
    : body

const receiptModelEvidenceFrom = receipt =>
  receipt?.modelEvidence ??
  receipt?.model_evidence ??
  receipt?.inference ??
  null

const receiptBackingFrom = receipt => {
  const evidence = receiptModelEvidenceFrom(receipt)
  return {
    requested_model: evidence?.requested_model ?? evidence?.requestedModel,
    served_model: evidence?.served_model ?? evidence?.servedModel,
    supply_lane: evidence?.supply_lane ?? evidence?.supplyLane,
    worker: evidence?.worker,
  }
}

const receiptTotalTokensFrom = receipt => {
  const evidence = receiptModelEvidenceFrom(receipt)
  return evidence?.total_tokens ?? evidence?.totalTokens
}

const telemetryTotalTokensFrom = openagents =>
  openagents?.telemetry?.totalTokens ?? openagents?.telemetry?.total_tokens

const receiptRedactionLeaks = value => {
  const serialized = JSON.stringify(value || {})
  const patterns = [
    /Bearer\s+[A-Za-z0-9._~+/=-]+/u,
    /oa_agent_[A-Za-z0-9._~+/=-]+/u,
    /sk-[A-Za-z0-9]{8,}/u,
    /api[_-]?key/iu,
    /access[_-]?token/iu,
    /fireworks[_-]?api[_-]?key/iu,
    /provider[_-]?(payload|secret|token)/iu,
    /raw[\s_-]?prompt/iu,
  ]
  return patterns
    .filter(pattern => pattern.test(serialized))
    .map(pattern => String(pattern))
}

const summarizeReceipt = (receipt, url) => ({
  kind: receipt?.kind,
  ledgerState: receipt?.ledgerState,
  modelEvidence: receiptModelEvidenceFrom(receipt),
  receiptRef: receipt?.receiptRef,
  schemaVersion: receipt?.schemaVersion,
  url,
})

const summarizeOperatorCreditReceipt = (openagents, receiptRef, url) => ({
  kind: 'operator_credit',
  ledgerState: 'zero_debit_operator_exempt',
  modelEvidence: {
    requested_model: openagents?.requested_model,
    served_model: openagents?.served_model,
    supply_lane: openagents?.supply_lane,
    total_tokens: telemetryTotalTokensFrom(openagents),
    worker: openagents?.worker,
  },
  receiptRef,
  url,
  zeroDebit: true,
})

const verifyReceiptProof = async ({
  backingExpectation,
  check,
  fetchImpl,
  label,
  openagents,
  origin,
}) => {
  const receiptPath = receiptPathFrom(openagents)
  check(`${label}_receipt_ref_present`, receiptPath !== null, { receiptPath })

  const operatorCreditReceiptRef = operatorCreditReceiptRefFromPath(receiptPath)
  if (operatorCreditReceiptRef !== null) {
    check(
      `${label}_operator_credit_zero_debit`,
      backingMatches(openagents, backingExpectation) &&
        Number(telemetryTotalTokensFrom(openagents) || 0) > 0,
      {
        receiptRef: operatorCreditReceiptRef,
        totalTokens: telemetryTotalTokensFrom(openagents),
      },
    )
    return summarizeOperatorCreditReceipt(
      openagents,
      operatorCreditReceiptRef,
      absoluteUrl(origin, receiptPath),
    )
  }

  const receiptResult = await requestJson(fetchImpl, origin, receiptPath)
  check(`${label}_receipt_endpoint_200`, okStatus(receiptResult.response), {
    status: receiptResult.response.status,
  })

  const receipt = receiptProjectionFrom(receiptResult.body)
  check(
    `${label}_receipt_schema_present`,
    receipt?.schemaVersion === 'openagents.inference.receipt.v1',
    {
      schemaVersion: receipt?.schemaVersion,
    },
  )
  check(
    `${label}_receipt_backing_evidence_present`,
    backingMatches(receiptBackingFrom(receipt), backingExpectation),
    receiptBackingFrom(receipt),
  )
  check(
    `${label}_receipt_usage_present`,
    Number(receiptTotalTokensFrom(receipt) || 0) > 0,
    { totalTokens: receiptTotalTokensFrom(receipt) },
  )
  const redactionLeaks = receiptRedactionLeaks(receiptResult.body)
  check(`${label}_receipt_redaction_guard_clean`, redactionLeaks.length === 0, {
    redactionLeaks,
  })

  return summarizeReceipt(receipt, absoluteUrl(origin, receiptPath))
}

const backingMatches = (
  openagents,
  {
    expectedServedModelContains = defaultExpectedServedModelContains,
    expectedSupplyLane = defaultExpectedSupplyLane,
    expectedWorker = defaultExpectedWorker,
    model = defaultModel,
  } = {},
) => {
  if (openagents?.requested_model !== model) {
    return false
  }
  if (expectedSupplyLane && openagents?.supply_lane !== expectedSupplyLane) {
    return false
  }
  if (expectedWorker && openagents?.worker !== expectedWorker) {
    return false
  }
  if (
    expectedServedModelContains &&
    !includesInsensitive(openagents?.served_model, expectedServedModelContains)
  ) {
    return false
  }
  return true
}

const replicaRoutingExpectation = ({
  allowedSelectedReplicaRefs = [],
  expectedSelectedReplicaRef = '',
  forbiddenSelectedReplicaRefs = [],
  requireSelectedReplicaRef = false,
} = {}) => ({
  allowedSelectedReplicaRefs: [...allowedSelectedReplicaRefs].filter(Boolean),
  expectedSelectedReplicaRef: String(expectedSelectedReplicaRef || ''),
  forbiddenSelectedReplicaRefs: [...forbiddenSelectedReplicaRefs].filter(
    Boolean,
  ),
  requireSelectedReplicaRef: Boolean(requireSelectedReplicaRef),
})

const hasReplicaRoutingExpectation = expectation =>
  expectation.requireSelectedReplicaRef ||
  expectation.expectedSelectedReplicaRef !== '' ||
  expectation.allowedSelectedReplicaRefs.length > 0 ||
  expectation.forbiddenSelectedReplicaRefs.length > 0

const replicaRoutingMatches = (openagents, expectation) => {
  const selectedReplicaRef = selectedReplicaRefFrom(openagents)
  if (expectation.requireSelectedReplicaRef && selectedReplicaRef === null) {
    return false
  }
  if (
    expectation.expectedSelectedReplicaRef !== '' &&
    selectedReplicaRef !== expectation.expectedSelectedReplicaRef
  ) {
    return false
  }
  if (
    expectation.allowedSelectedReplicaRefs.length > 0 &&
    !expectation.allowedSelectedReplicaRefs.includes(selectedReplicaRef)
  ) {
    return false
  }
  if (
    selectedReplicaRef !== null &&
    expectation.forbiddenSelectedReplicaRefs.includes(selectedReplicaRef)
  ) {
    return false
  }
  return true
}

export const parseArgs = (argv, env = process.env) => {
  const options = {
    approveLiveSpend:
      truthy(env.OPENAGENTS_KHALA_SMOKE_APPROVE_LIVE_SPEND) ||
      truthy(env.KHALA_SMOKE_APPROVE_LIVE_SPEND),
    apiBasePath: env.OPENAGENTS_API_BASE_PATH || defaultApiBasePath,
    baseUrl: env.OPENAGENTS_BASE_URL || defaultBaseUrl,
    expectedServedModelContains:
      env.OPENAGENTS_KHALA_EXPECTED_SERVED_MODEL_CONTAINS ||
      defaultExpectedServedModelContains,
    expectedSupplyLane:
      env.OPENAGENTS_KHALA_EXPECTED_SUPPLY_LANE || defaultExpectedSupplyLane,
    expectedWorker:
      env.OPENAGENTS_KHALA_EXPECTED_WORKER || defaultExpectedWorker,
    forbiddenPublicModelIds: [
      ...defaultForbiddenPublicModelIds,
      ...csv(env.OPENAGENTS_KHALA_FORBIDDEN_PUBLIC_MODEL_IDS),
    ],
    model: env.OPENAGENTS_KHALA_SMOKE_MODEL || defaultModel,
    prompt: env.OPENAGENTS_KHALA_SMOKE_PROMPT || defaultPrompt,
    readinessOnly: false,
    token: env.OPENAGENTS_AGENT_TOKEN || '',
  }

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]
    if (value === '--base-url' || value === '--baseUrl') {
      options.baseUrl = argv[++index] || options.baseUrl
    } else if (value === '--api-base-path' || value === '--apiBasePath') {
      options.apiBasePath = argv[++index] || options.apiBasePath
    } else if (value === '--model') {
      options.model = argv[++index] || options.model
    } else if (value === '--prompt') {
      options.prompt = argv[++index] || options.prompt
    } else if (value === '--token') {
      options.token = argv[++index] || options.token
    } else if (value === '--expected-supply-lane') {
      options.expectedSupplyLane = argv[++index] || options.expectedSupplyLane
    } else if (value === '--expected-worker') {
      options.expectedWorker = argv[++index] || options.expectedWorker
    } else if (value === '--expected-served-model-contains') {
      options.expectedServedModelContains =
        argv[++index] || options.expectedServedModelContains
    } else if (value === '--forbid-public-model') {
      const forbidden = argv[++index]
      if (forbidden) {
        options.forbiddenPublicModelIds.push(forbidden)
      }
    } else if (value === '--approve-live-spend') {
      options.approveLiveSpend = true
    } else if (value === '--readiness-only') {
      options.readinessOnly = true
    } else if (value === '--help' || value === '-h') {
      options.help = true
    } else {
      throw new Error(`Unknown argument: ${value}`)
    }
  }

  return options
}

export const usage = () => `Usage:
  OPENAGENTS_AGENT_TOKEN=oa_agent_... \\
    node scripts/khala-production-smoke.mjs --approve-live-spend

Options:
  --base-url <url>                         OpenAgents origin. Defaults to https://openagents.com.
  --api-base-path <path>                   API base path. Defaults to /v1.
  --model <model>                          Defaults to openagents/khala.
  --prompt <text>                          Prompt used for both nonstreaming and streaming calls.
  --token <token>                          Agent bearer token. Defaults to OPENAGENTS_AGENT_TOKEN.
  --expected-supply-lane <lane>            Defaults to fireworks.
  --expected-worker <worker>               Defaults to fireworks.
  --expected-served-model-contains <text>  Defaults to deepseek-v4-flash.
  --forbid-public-model <model>            Add a model id that must not appear in /v1/models.
  --approve-live-spend                     Required before authenticated completion calls.
  --readiness-only                         Check readiness + public model catalog without spending.

This smoke verifies /v1/gateway/readiness, /v1/models, nonstreaming chat,
streaming chat, public model stability, usage, receipt/backing evidence, and
a simple infrastructure-leak guard. It never prints bearer tokens or raw
completion text.
`

export const runKhalaProductionSmoke = async ({
  allowedSelectedReplicaRefs = [],
  approveLiveSpend = false,
  apiBasePath = defaultApiBasePath,
  baseUrl = defaultBaseUrl,
  expectedServedModelContains = defaultExpectedServedModelContains,
  expectedSelectedReplicaRef = '',
  expectedSupplyLane = defaultExpectedSupplyLane,
  expectedWorker = defaultExpectedWorker,
  fetchImpl = globalThis.fetch,
  forbiddenSelectedReplicaRefs = [],
  forbiddenPublicModelIds = defaultForbiddenPublicModelIds,
  model = defaultModel,
  prompt = defaultPrompt,
  readinessOnly = false,
  requireSelectedReplicaRef = false,
  token = '',
} = {}) => {
  assert(typeof fetchImpl === 'function', 'A fetch implementation is required.')

  const origin = trimBaseUrl(baseUrl)
  const readinessPath = apiPath(apiBasePath, 'gateway/readiness')
  const modelsPath = apiPath(apiBasePath, 'models')
  const chatCompletionsPath = apiPath(apiBasePath, 'chat/completions')
  const checks = []
  const check = (name, passed, details = {}) => {
    checks.push({ details, name, passed: Boolean(passed) })
    assert(passed, `${name} failed`)
  }

  const readiness = await requestJson(fetchImpl, origin, readinessPath)
  check('readiness_endpoint_200', okStatus(readiness.response), {
    status: readiness.response.status,
  })
  check(
    'readiness_has_servable_model',
    Number(readiness.body?.servableModelCount || 0) > 0,
    {
      servableModelCount: readiness.body?.servableModelCount,
      status: readiness.body?.status,
    },
  )

  const models = await requestJson(fetchImpl, origin, modelsPath)
  const modelIds = modelIdsFrom(models.body)
  const forbiddenPublicIds = forbiddenCatalogIdsFrom(
    modelIds,
    forbiddenPublicModelIds,
  )
  check('models_endpoint_200', okStatus(models.response), {
    status: models.response.status,
  })
  check('models_lists_public_khala', modelIds.includes(model), {
    model,
    modelCount: modelIds.length,
  })
  check('models_public_surface_closed', forbiddenPublicIds.length === 0, {
    forbiddenPublicIds,
  })

  if (readinessOnly) {
    return {
      checks,
      model,
      ok: true,
      readiness: summarizeGatewayReadiness(readiness.body),
    }
  }

  assert(
    token.trim() !== '',
    'Missing OPENAGENTS_AGENT_TOKEN or --token for authenticated smoke.',
  )
  assert(
    approveLiveSpend,
    'Refusing authenticated completion smoke without --approve-live-spend.',
  )

  const authHeaders = {
    authorization: `Bearer ${token}`,
    'content-type': 'application/json',
  }
  const backingExpectation = {
    expectedServedModelContains,
    expectedSupplyLane,
    expectedWorker,
    model,
  }
  const replicaExpectation = replicaRoutingExpectation({
    allowedSelectedReplicaRefs,
    expectedSelectedReplicaRef,
    forbiddenSelectedReplicaRefs,
    requireSelectedReplicaRef,
  })
  const shouldCheckReplicaRouting =
    hasReplicaRoutingExpectation(replicaExpectation)
  const chatBody = {
    messages: [{ content: prompt, role: 'user' }],
    model,
  }

  const completion = await requestJson(fetchImpl, origin, chatCompletionsPath, {
    body: JSON.stringify({ ...chatBody, stream: false }),
    headers: authHeaders,
    method: 'POST',
  })
  check('nonstream_completion_200', okStatus(completion.response), {
    status: completion.response.status,
  })
  check('nonstream_public_model_preserved', completion.body?.model === model, {
    responseModel: completion.body?.model,
  })
  const completionContent = contentFromCompletion(completion.body)
  check(
    'nonstream_infrastructure_guard_clean',
    infrastructureLeaks(completionContent).length === 0,
  )
  check(
    'nonstream_usage_present',
    Number(completion.body?.usage?.total_tokens || 0) > 0,
    {
      totalTokens: completion.body?.usage?.total_tokens,
    },
  )
  check(
    'nonstream_backing_disclosure_present',
    backingMatches(completion.body?.openagents, backingExpectation),
    summarizeOpenAgents(completion.body?.openagents),
  )
  if (shouldCheckReplicaRouting) {
    check(
      'nonstream_replica_routing_present',
      replicaRoutingMatches(completion.body?.openagents, replicaExpectation),
      {
        expectation: replicaExpectation,
        openagents: summarizeOpenAgents(completion.body?.openagents),
      },
    )
  }
  const nonstreamReceipt = await verifyReceiptProof({
    backingExpectation,
    check,
    fetchImpl,
    label: 'nonstream',
    openagents: completion.body?.openagents,
    origin,
  })

  const streamResponse = await fetchImpl(
    absoluteUrl(origin, chatCompletionsPath),
    {
      body: JSON.stringify({ ...chatBody, stream: true }),
      headers: {
        accept: 'text/event-stream',
        ...authHeaders,
      },
      method: 'POST',
    },
  )
  const streamText = await streamResponse.text()
  check('stream_completion_200', okStatus(streamResponse), {
    status: streamResponse.status,
  })
  check('stream_done_seen', streamText.trimEnd().endsWith('data: [DONE]'))
  const frames = parseSseFrames(streamText)
  const streamedContent = contentFromSseFrames(frames)
  const terminalOpenAgents = terminalOpenAgentsFrom(frames)
  check('stream_frames_present', frames.length > 0, { frames: frames.length })
  check(
    'stream_public_model_preserved',
    terminalOpenAgents?.requested_model === model,
    summarizeOpenAgents(terminalOpenAgents),
  )
  check(
    'stream_infrastructure_guard_clean',
    infrastructureLeaks(streamedContent).length === 0,
  )
  check(
    'stream_backing_disclosure_present',
    backingMatches(terminalOpenAgents, backingExpectation),
    summarizeOpenAgents(terminalOpenAgents),
  )
  if (shouldCheckReplicaRouting) {
    check(
      'stream_replica_routing_present',
      replicaRoutingMatches(terminalOpenAgents, replicaExpectation),
      {
        expectation: replicaExpectation,
        openagents: summarizeOpenAgents(terminalOpenAgents),
      },
    )
  }
  const streamReceipt = await verifyReceiptProof({
    backingExpectation,
    check,
    fetchImpl,
    label: 'stream',
    openagents: terminalOpenAgents,
    origin,
  })

  return {
    checks,
    model,
    nonstream: {
      openagents: summarizeOpenAgents(completion.body?.openagents),
      receipt: nonstreamReceipt,
      responseId: completion.body?.id,
      totalTokens: completion.body?.usage?.total_tokens,
    },
    ok: true,
    readiness: summarizeGatewayReadiness(readiness.body),
    stream: {
      frameCount: frames.length,
      openagents: summarizeOpenAgents(terminalOpenAgents),
      receipt: streamReceipt,
    },
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const options = parseArgs(process.argv.slice(2))
  if (options.help) {
    console.log(usage())
  } else {
    runKhalaProductionSmoke(options)
      .then(output => {
        console.log(
          JSON.stringify(
            output,
            (key, value) => {
              if (key.toLowerCase().includes('token')) {
                return '<redacted>'
              }
              return redact(value)
            },
            2,
          ),
        )
      })
      .catch(error => {
        console.error(
          redact(error instanceof Error ? error.message : String(error)),
        )
        process.exitCode = 1
      })
  }
}
