#!/usr/bin/env node

import { runKhalaProductionSmoke } from './khala-production-smoke.mjs'

const defaultBaseUrl = 'https://openagents.com'
const defaultApiBasePath = '/api/v1'
const defaultCounterPath = '/api/public/khala-tokens-served'
const khalaModel = 'openagents/khala'
const glmWorkerId = 'hydralisk-vllm-glm-5p2-reap-504b'
const glmServedModel = 'openagents/glm-5.2-reap-504b'
const legacyGlmReplicaId = 'primary'
const defaultPrompt =
  'OpenAgents Khala GLM REAP smoke: answer with exactly READY and no other words.'

const requiredArmingFields = [
  'HYDRALISK_GLM_52_REAP_504B_ENABLED',
  'HYDRALISK_GLM_52_REAP_504B_BASE_URL',
  'HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN',
  'HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF',
  'HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF',
]

const forbiddenPublicModelIds = [
  glmServedModel,
  'glm-5.2-reap-504b',
  'openagents/glm-5p2-reap-504b',
  '0xsero/glm-5.2-504b',
  '0xsero/glm-5.2-reap-504b',
]

const publicSafeRef = /^[a-z0-9][a-z0-9._:-]{1,199}$/iu
const replicaIdPattern = /^[a-z0-9][a-z0-9-]{0,63}$/u

const trimBaseUrl = baseUrl =>
  String(baseUrl || defaultBaseUrl).replace(/\/+$/u, '')

const absoluteUrl = (baseUrl, pathOrUrl) =>
  new URL(String(pathOrUrl || ''), baseUrl).toString()

const truthy = value =>
  ['1', 'true', 'yes', 'on'].includes(
    String(value || '')
      .trim()
      .toLowerCase(),
  )

const positiveInt = (value, fallback) => {
  const parsed = Number(value)
  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback
}

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
    .replace(/https?:\/\/10\.[^\s"']+/gu, '<private-rfc1918-url>')
}

const isPresent = value => typeof value === 'string' && value.trim() !== ''

const isPublicSafeRef = value =>
  typeof value === 'string' &&
  value.trim() !== '' &&
  value === value.trim() &&
  publicSafeRef.test(value) &&
  !value.includes('://') &&
  !value.toLowerCase().startsWith('sk-')

const normalizeReplicaId = value =>
  String(value || '')
    .trim()
    .toLowerCase()

const isReplicaId = value => replicaIdPattern.test(normalizeReplicaId(value))

const replicaEnvToken = replicaId =>
  normalizeReplicaId(replicaId).replace(/-/gu, '_').toUpperCase()

const glmReplicaEnvKey = (replicaId, suffix) =>
  `HYDRALISK_GLM_52_REAP_504B_${replicaEnvToken(replicaId)}_${suffix}`

const envStringValue = (env, key) => {
  const value = env?.[key]
  return typeof value === 'string' ? value : undefined
}

const glmReplicaEnvValue = (env, replicaId, suffix, legacyKey, hasPool) => {
  const key = glmReplicaEnvKey(replicaId, suffix)
  const namedValue = envStringValue(env, key)
  if (
    !hasPool &&
    normalizeReplicaId(replicaId) === legacyGlmReplicaId &&
    !isPresent(namedValue)
  ) {
    return { key: legacyKey, value: envStringValue(env, legacyKey) }
  }
  return { key, value: namedValue }
}

const parseGlmReplicaIds = env => {
  const rawValue = env.HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS?.trim()
  if (rawValue === undefined || rawValue === '') {
    return {
      blockerRefs: [],
      explicitPool: false,
      replicaIds: [legacyGlmReplicaId],
    }
  }

  const blockerRefs = []
  const replicaIds = []
  const seen = new Set()
  for (const raw of rawValue.split(',')) {
    const replicaId = normalizeReplicaId(raw)
    if (!isReplicaId(replicaId)) {
      blockerRefs.push('HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS')
      continue
    }
    if (seen.has(replicaId)) {
      blockerRefs.push('HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS')
      continue
    }
    seen.add(replicaId)
    replicaIds.push(replicaId)
  }

  if (replicaIds.length === 0) {
    blockerRefs.push('HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS')
  }

  return { blockerRefs, explicitPool: true, replicaIds }
}

const isEnabledFlag = value => {
  const normalized = String(value || '')
    .trim()
    .toLowerCase()
  return (
    normalized === 'true' ||
    normalized === '1' ||
    normalized === 'yes' ||
    normalized === 'on'
  )
}

const replicaRefFor = replicaId =>
  `replica.hydralisk.glm_52_reap_504b.${normalizeReplicaId(replicaId)}`

const namedRequiredFieldsForReplica = replicaId => [
  glmReplicaEnvKey(replicaId, 'ENABLED'),
  glmReplicaEnvKey(replicaId, 'BASE_URL'),
  glmReplicaEnvKey(replicaId, 'BEARER_TOKEN'),
  glmReplicaEnvKey(replicaId, 'PREFLIGHT_REF'),
  glmReplicaEnvKey(replicaId, 'RECEIPT_REF'),
]

const resolveGlmReplicaSmokeArming = (env, replicaId, hasPool) => {
  const enabled = glmReplicaEnvValue(
    env,
    replicaId,
    'ENABLED',
    'HYDRALISK_GLM_52_REAP_504B_ENABLED',
    hasPool,
  )
  const baseUrl = glmReplicaEnvValue(
    env,
    replicaId,
    'BASE_URL',
    'HYDRALISK_GLM_52_REAP_504B_BASE_URL',
    hasPool,
  )
  const bearerToken = glmReplicaEnvValue(
    env,
    replicaId,
    'BEARER_TOKEN',
    'HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN',
    hasPool,
  )
  const preflightRef = glmReplicaEnvValue(
    env,
    replicaId,
    'PREFLIGHT_REF',
    'HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF',
    hasPool,
  )
  const receiptRef = glmReplicaEnvValue(
    env,
    replicaId,
    'RECEIPT_REF',
    'HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF',
    hasPool,
  )
  const profileRef = glmReplicaEnvValue(
    env,
    replicaId,
    'PROFILE_REF',
    'HYDRALISK_GLM_52_REAP_504B_PROFILE_REF',
    hasPool,
  )
  const costProfileRef = glmReplicaEnvValue(
    env,
    replicaId,
    'COST_PROFILE_REF',
    'HYDRALISK_GLM_52_REAP_504B_COST_PROFILE_REF',
    hasPool,
  )
  const benchmarkReserved = glmReplicaEnvValue(
    env,
    replicaId,
    'BENCHMARK_RESERVED',
    'HYDRALISK_GLM_52_REAP_504B_BENCHMARK_RESERVED',
    hasPool,
  )
  const draining = glmReplicaEnvValue(
    env,
    replicaId,
    'DRAINING',
    'HYDRALISK_GLM_52_REAP_504B_DRAINING',
    hasPool,
  )

  const blockerRefs = []
  if (enabled.value?.trim() !== 'ready') {
    blockerRefs.push(enabled.key)
  }
  if (!isPresent(baseUrl.value)) {
    blockerRefs.push(baseUrl.key)
  }
  if (!isPresent(bearerToken.value)) {
    blockerRefs.push(bearerToken.key)
  }
  if (!isPublicSafeRef(preflightRef.value)) {
    blockerRefs.push(preflightRef.key)
  }
  if (!isPublicSafeRef(receiptRef.value)) {
    blockerRefs.push(receiptRef.key)
  }
  if (isPresent(profileRef.value) && !isPublicSafeRef(profileRef.value)) {
    blockerRefs.push(profileRef.key)
  }
  if (
    isPresent(costProfileRef.value) &&
    !isPublicSafeRef(costProfileRef.value)
  ) {
    blockerRefs.push(costProfileRef.key)
  }

  const armed = blockerRefs.length === 0
  const reserved = isEnabledFlag(benchmarkReserved.value)
  const isDraining = isEnabledFlag(draining.value)
  const eligible = armed && !reserved && !isDraining
  return {
    armed,
    benchmarkReserved: reserved,
    blockerRefs,
    costProfileRef: isPublicSafeRef(costProfileRef.value)
      ? costProfileRef.value
      : undefined,
    draining: isDraining,
    eligible,
    evidenceRefs: [preflightRef.value, receiptRef.value].filter(
      isPublicSafeRef,
    ),
    profileRef: isPublicSafeRef(profileRef.value)
      ? profileRef.value
      : undefined,
    replicaId,
    replicaRef: replicaRefFor(replicaId),
    requiredFields: hasPool
      ? namedRequiredFieldsForReplica(replicaId)
      : requiredArmingFields,
  }
}

const isFireworksKhalaBacking = value => {
  const normalized = String(value || '')
    .trim()
    .toLowerCase()
  return (
    normalized === 'deepseek-v4-flash' ||
    normalized === 'fireworks/deepseek-v4-flash' ||
    normalized === 'accounts/fireworks/models/deepseek-v4-flash'
  )
}

export const resolveGlmReapSmokeArming = (env = process.env) => {
  const parsed = parseGlmReplicaIds(env)
  const replicas = parsed.replicaIds.map(replicaId =>
    resolveGlmReplicaSmokeArming(env, replicaId, parsed.explicitPool),
  )
  const eligibleReplicaRefs = replicas
    .filter(replica => replica.eligible)
    .map(replica => replica.replicaRef)
  const excludedReplicaRefs = replicas
    .filter(
      replica =>
        replica.armed && (replica.benchmarkReserved || replica.draining),
    )
    .map(replica => replica.replicaRef)
  const blockerRefs = [
    ...parsed.blockerRefs,
    ...replicas.flatMap(replica => replica.blockerRefs),
  ]
  if (eligibleReplicaRefs.length === 0) {
    blockerRefs.push('HYDRALISK_GLM_52_REAP_504B_NO_ELIGIBLE_REPLICA')
  }
  if (isFireworksKhalaBacking(env.KHALA_BACKING_MODEL)) {
    blockerRefs.push('KHALA_BACKING_MODEL')
  }

  return {
    armed: blockerRefs.length === 0,
    blockerRefs,
    eligibleReplicaRefs,
    excludedReplicaRefs,
    poolMode: parsed.explicitPool ? 'named_pool' : 'legacy_single',
    replicas,
    requiredFields: parsed.explicitPool
      ? [
          'HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS',
          ...replicas.flatMap(replica => replica.requiredFields),
        ]
      : requiredArmingFields,
  }
}

export const parseArgs = (argv, env = process.env) => {
  const options = {
    approveLiveSpend:
      truthy(env.OPENAGENTS_KHALA_GLM_REAP_SMOKE_APPROVE_LIVE_SPEND) ||
      truthy(env.OPENAGENTS_KHALA_SMOKE_APPROVE_LIVE_SPEND),
    apiBasePath:
      env.OPENAGENTS_KHALA_GLM_REAP_API_BASE_PATH || defaultApiBasePath,
    baseUrl: env.OPENAGENTS_BASE_URL || defaultBaseUrl,
    counterPath:
      env.OPENAGENTS_KHALA_GLM_REAP_COUNTER_PATH || defaultCounterPath,
    counterPollMs: positiveInt(
      env.OPENAGENTS_KHALA_GLM_REAP_COUNTER_POLL_MS,
      2_500,
    ),
    counterPolls: positiveInt(env.OPENAGENTS_KHALA_GLM_REAP_COUNTER_POLLS, 8),
    env,
    failWhenUnarmed: truthy(
      env.OPENAGENTS_KHALA_GLM_REAP_SMOKE_FAIL_WHEN_UNARMED,
    ),
    expectedReplicaId: env.OPENAGENTS_KHALA_GLM_REAP_EXPECTED_REPLICA_ID || '',
    expectOperatorExemptZeroDebit:
      truthy(env.OPENAGENTS_KHALA_GLM_REAP_OPERATOR_EXEMPT_ZERO_DEBIT) ||
      truthy(env.OPENAGENTS_KHALA_SMOKE_OPERATOR_EXEMPT_ZERO_DEBIT),
    prompt: env.OPENAGENTS_KHALA_GLM_REAP_SMOKE_PROMPT || defaultPrompt,
    readinessOnly: false,
    token: env.OPENAGENTS_AGENT_TOKEN || '',
  }

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]
    if (value === '--base-url' || value === '--baseUrl') {
      options.baseUrl = argv[++index] || options.baseUrl
    } else if (value === '--api-base-path' || value === '--apiBasePath') {
      options.apiBasePath = argv[++index] || options.apiBasePath
    } else if (value === '--counter-path') {
      options.counterPath = argv[++index] || options.counterPath
    } else if (value === '--counter-polls') {
      options.counterPolls = positiveInt(argv[++index], options.counterPolls)
    } else if (value === '--counter-poll-ms') {
      options.counterPollMs = positiveInt(argv[++index], options.counterPollMs)
    } else if (value === '--prompt') {
      options.prompt = argv[++index] || options.prompt
    } else if (value === '--expected-replica-id') {
      options.expectedReplicaId = argv[++index] || options.expectedReplicaId
    } else if (value === '--token') {
      options.token = argv[++index] || options.token
    } else if (value === '--approve-live-spend') {
      options.approveLiveSpend = true
    } else if (value === '--operator-exempt-zero-debit') {
      options.expectOperatorExemptZeroDebit = true
    } else if (value === '--readiness-only') {
      options.readinessOnly = true
    } else if (value === '--fail-when-unarmed') {
      options.failWhenUnarmed = true
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
  HYDRALISK_GLM_52_REAP_504B_ENABLED=ready \\
  HYDRALISK_GLM_52_REAP_504B_BASE_URL=<secret> \\
  HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN=<secret> \\
  HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF=<public-ref> \\
  HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF=<public-ref> \\
    node scripts/khala-glm-reap-production-smoke.mjs --approve-live-spend

Named pool:
  HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS=primary,second \\
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_ENABLED=ready \\
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_BENCHMARK_RESERVED=true \\
  HYDRALISK_GLM_52_REAP_504B_SECOND_ENABLED=ready \\
    node scripts/khala-glm-reap-production-smoke.mjs \\
      --approve-live-spend --expected-replica-id second

Options:
  --base-url <url>          OpenAgents origin. Defaults to https://openagents.com.
  --api-base-path <path>    API base path. Defaults to /api/v1.
  --counter-path <path>     Public counter path. Defaults to /api/public/khala-tokens-served.
  --counter-polls <n>       Counter retry count. Defaults to 8.
  --counter-poll-ms <ms>    Counter retry interval. Defaults to 2500.
  --expected-replica-id <id>
                            Require the selected public-safe GLM replica ref.
  --prompt <text>           Prompt used for nonstreaming and streaming calls.
  --token <token>           Agent bearer token. Defaults to OPENAGENTS_AGENT_TOKEN.
  --approve-live-spend      Required before authenticated completion calls.
  --operator-exempt-zero-debit
                            Operator-exempt token mode: a missing billable
                            receipt ref is skipped only when the response
                            exposes the zero-debit exemption path.
  --readiness-only          Check arming, readiness, and public catalog without spending.
  --fail-when-unarmed       Return failure instead of skipped when arming env is absent.

When the GLM arming env is absent, this smoke prints skipped and exits 0. It
never prints bearer tokens, Hydralisk URLs, prompts, or completions.
`

const tokensServedFrom = body => {
  const value = Number(body?.tokensServed)
  if (!Number.isFinite(value)) {
    throw new Error('tokens_served_counter_invalid failed')
  }
  return value
}

const readTokensServed = async (fetchImpl, origin, counterPath) => {
  const result = await requestJson(fetchImpl, origin, counterPath)
  assert(okStatus(result.response), 'tokens_served_counter_200 failed')
  return tokensServedFrom(result.body)
}

const modelEvidenceFrom = receipt => receipt?.modelEvidence ?? null

const totalTokensFromReceipt = receipt => {
  const evidence = modelEvidenceFrom(receipt)
  const value = Number(evidence?.total_tokens ?? evidence?.totalTokens ?? 0)
  return Number.isFinite(value) ? value : 0
}

const sleepMs = ms => new Promise(resolve => setTimeout(resolve, ms))

const pollCounterDelta = async ({
  before,
  counterPath,
  fetchImpl,
  origin,
  requiredDelta,
  sleep = sleepMs,
  counterPollMs,
  counterPolls,
}) => {
  let after = before
  for (let attempt = 0; attempt < counterPolls; attempt += 1) {
    after = await readTokensServed(fetchImpl, origin, counterPath)
    if (after - before >= requiredDelta) {
      return { after, attempts: attempt + 1, delta: after - before }
    }
    if (attempt < counterPolls - 1) {
      await sleep(counterPollMs)
    }
  }
  return { after, attempts: counterPolls, delta: after - before }
}

const poolSummaryFromArming = arming => ({
  eligibleReplicaRefs: arming.eligibleReplicaRefs,
  excludedReplicaRefs: arming.excludedReplicaRefs,
  mode: arming.poolMode,
  replicas: arming.replicas.map(replica => ({
    armed: replica.armed,
    benchmarkReserved: replica.benchmarkReserved,
    draining: replica.draining,
    eligible: replica.eligible,
    evidenceRefs: replica.evidenceRefs,
    replicaId: replica.replicaId,
    replicaRef: replica.replicaRef,
    ...(replica.profileRef === undefined
      ? {}
      : { profileRef: replica.profileRef }),
    ...(replica.costProfileRef === undefined
      ? {}
      : { costProfileRef: replica.costProfileRef }),
  })),
})

const resolveReplicaRoutingExpectation = (arming, expectedReplicaId) => {
  const expectedReplica = normalizeReplicaId(expectedReplicaId)
  if (expectedReplica !== '') {
    if (!isReplicaId(expectedReplica)) {
      throw new Error(
        `expected_glm_replica_id_public_safe failed: ${expectedReplicaId}`,
      )
    }
    const expectedSelectedReplicaRef = replicaRefFor(expectedReplica)
    assert(
      arming.eligibleReplicaRefs.includes(expectedSelectedReplicaRef),
      `expected_glm_replica_eligible failed: ${expectedReplica}`,
    )
    return {
      allowedSelectedReplicaRefs: [],
      expectedSelectedReplicaRef,
      forbiddenSelectedReplicaRefs: arming.excludedReplicaRefs,
      requireSelectedReplicaRef: true,
    }
  }

  return {
    allowedSelectedReplicaRefs: arming.eligibleReplicaRefs,
    expectedSelectedReplicaRef:
      arming.eligibleReplicaRefs.length === 1
        ? arming.eligibleReplicaRefs[0]
        : '',
    forbiddenSelectedReplicaRefs: arming.excludedReplicaRefs,
    requireSelectedReplicaRef: true,
  }
}

const hydraliskLaneReadinessFrom = readiness =>
  Array.isArray(readiness?.lanes)
    ? readiness.lanes.find(lane => lane?.lane === 'hydralisk')
    : undefined

const assertDeployedGlmReapReadinessEvidence = readiness => {
  const hydraliskLane = hydraliskLaneReadinessFrom(readiness)
  assert(
    hydraliskLane?.armed === true &&
      Number(hydraliskLane?.servableModelCount || 0) > 0,
    'deployed_glm_reap_readiness_public_evidence_present failed',
  )
}

export const runKhalaGlmReapProductionSmoke = async ({
  approveLiveSpend = false,
  apiBasePath = defaultApiBasePath,
  baseUrl = defaultBaseUrl,
  counterPath = defaultCounterPath,
  counterPollMs = 2_500,
  counterPolls = 8,
  env = process.env,
  expectedReplicaId = '',
  expectOperatorExemptZeroDebit = false,
  failWhenUnarmed = false,
  fetchImpl = globalThis.fetch,
  prompt = defaultPrompt,
  readinessOnly = false,
  sleep = sleepMs,
  token = '',
} = {}) => {
  assert(typeof fetchImpl === 'function', 'A fetch implementation is required.')

  const arming = resolveGlmReapSmokeArming(env)
  if (!arming.armed) {
    if (readinessOnly) {
      const origin = trimBaseUrl(baseUrl)
      const khalaSmoke = await runKhalaProductionSmoke({
        apiBasePath,
        baseUrl: origin,
        fetchImpl,
        forbiddenPublicModelIds,
        model: khalaModel,
        readinessOnly: true,
      })
      assertDeployedGlmReapReadinessEvidence(khalaSmoke.readiness)
      return {
        arming: {
          armed: false,
          blockerRefs: arming.blockerRefs,
          evidenceSource: 'deployed_public_readiness_catalog',
        },
        ...khalaSmoke,
        counter: null,
        pool: poolSummaryFromArming(arming),
        reason: 'local_glm_reap_lane_not_armed_read_deployed_public_evidence',
        skipped: false,
      }
    }

    const skipped = {
      arming: {
        armed: false,
        blockerRefs: arming.blockerRefs,
        pool: poolSummaryFromArming(arming),
      },
      ok: true,
      reason: 'glm_reap_lane_not_armed',
      skipped: true,
    }
    if (failWhenUnarmed) {
      throw new Error(
        `glm_reap_lane_not_armed failed: ${arming.blockerRefs.join(',')}`,
      )
    }
    return skipped
  }

  const origin = trimBaseUrl(baseUrl)
  const before = readinessOnly
    ? null
    : await readTokensServed(fetchImpl, origin, counterPath)
  const replicaExpectation = resolveReplicaRoutingExpectation(
    arming,
    expectedReplicaId,
  )
  const khalaSmoke = await runKhalaProductionSmoke({
    allowedSelectedReplicaRefs: replicaExpectation.allowedSelectedReplicaRefs,
    approveLiveSpend,
    apiBasePath,
    baseUrl: origin,
    expectedServedModelContains: glmServedModel,
    expectedSelectedReplicaRef: replicaExpectation.expectedSelectedReplicaRef,
    expectedSupplyLane: 'hydralisk',
    expectedWorker: glmWorkerId,
    expectOperatorExemptZeroDebit,
    fetchImpl,
    forbiddenSelectedReplicaRefs:
      replicaExpectation.forbiddenSelectedReplicaRefs,
    forbiddenPublicModelIds,
    model: khalaModel,
    prompt,
    readinessOnly,
    requireSelectedReplicaRef: replicaExpectation.requireSelectedReplicaRef,
    token,
  })

  if (readinessOnly) {
    return {
      arming: { armed: true, blockerRefs: [] },
      ...khalaSmoke,
      counter: null,
      pool: poolSummaryFromArming(arming),
      skipped: false,
    }
  }

  const nonstreamTokens = Number(khalaSmoke.nonstream?.totalTokens ?? 0)
  const streamTokens = totalTokensFromReceipt(khalaSmoke.stream?.receipt)
  const servedTokens = nonstreamTokens + streamTokens
  const requiredDelta = Math.max(1, Math.floor(servedTokens * 0.75))
  const counter = await pollCounterDelta({
    before,
    counterPath,
    counterPollMs,
    counterPolls,
    fetchImpl,
    origin,
    requiredDelta,
    sleep,
  })
  assert(
    counter.delta >= requiredDelta,
    `tokens_served_counter_delta_matches_glm_usage failed: delta=${counter.delta} required=${requiredDelta}`,
  )

  return {
    arming: { armed: true, blockerRefs: [] },
    ...khalaSmoke,
    counter: {
      after: counter.after,
      attempts: counter.attempts,
      before,
      delta: counter.delta,
      requiredDelta,
      servedTokens,
    },
    pool: poolSummaryFromArming(arming),
    skipped: false,
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const options = parseArgs(process.argv.slice(2))
  if (options.help) {
    console.log(usage())
  } else {
    runKhalaGlmReapProductionSmoke(options)
      .then(output => {
        console.log(
          JSON.stringify(
            output,
            (key, value) => {
              if (key.toLowerCase().includes('token')) {
                return '<redacted>'
              }
              if (key.toLowerCase().includes('baseurl')) {
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
