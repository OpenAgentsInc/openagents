#!/usr/bin/env node

import { runKhalaProductionSmoke } from './khala-production-smoke.mjs'

const defaultBaseUrl = 'https://openagents.com'
const defaultApiBasePath = '/api/v1'
const defaultCounterPath = '/api/public/khala-tokens-served'
const khalaModel = 'openagents/khala'
const glmWorkerId = 'hydralisk-vllm-glm-5p2-reap-504b'
const glmServedModel = 'openagents/glm-5.2-reap-504b'
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

const isPresent = value =>
  typeof value === 'string' && value.trim() !== ''

const isPublicSafeRef = value =>
  typeof value === 'string' &&
  value.trim() !== '' &&
  value === value.trim() &&
  publicSafeRef.test(value) &&
  !value.includes('://') &&
  !value.toLowerCase().startsWith('sk-')

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
  const blockerRefs = []
  if (env.HYDRALISK_GLM_52_REAP_504B_ENABLED?.trim() !== 'ready') {
    blockerRefs.push('HYDRALISK_GLM_52_REAP_504B_ENABLED')
  }
  if (!isPresent(env.HYDRALISK_GLM_52_REAP_504B_BASE_URL)) {
    blockerRefs.push('HYDRALISK_GLM_52_REAP_504B_BASE_URL')
  }
  if (!isPresent(env.HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN)) {
    blockerRefs.push('HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN')
  }
  if (!isPublicSafeRef(env.HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF)) {
    blockerRefs.push('HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF')
  }
  if (!isPublicSafeRef(env.HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF)) {
    blockerRefs.push('HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF')
  }
  if (isFireworksKhalaBacking(env.KHALA_BACKING_MODEL)) {
    blockerRefs.push('KHALA_BACKING_MODEL')
  }

  return {
    armed: blockerRefs.length === 0,
    blockerRefs,
    requiredFields: requiredArmingFields,
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
    counterPolls: positiveInt(
      env.OPENAGENTS_KHALA_GLM_REAP_COUNTER_POLLS,
      8,
    ),
    env,
    failWhenUnarmed: truthy(
      env.OPENAGENTS_KHALA_GLM_REAP_SMOKE_FAIL_WHEN_UNARMED,
    ),
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
    } else if (value === '--token') {
      options.token = argv[++index] || options.token
    } else if (value === '--approve-live-spend') {
      options.approveLiveSpend = true
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

Options:
  --base-url <url>          OpenAgents origin. Defaults to https://openagents.com.
  --api-base-path <path>    API base path. Defaults to /api/v1.
  --counter-path <path>     Public counter path. Defaults to /api/public/khala-tokens-served.
  --counter-polls <n>       Counter retry count. Defaults to 8.
  --counter-poll-ms <ms>    Counter retry interval. Defaults to 2500.
  --prompt <text>           Prompt used for nonstreaming and streaming calls.
  --token <token>           Agent bearer token. Defaults to OPENAGENTS_AGENT_TOKEN.
  --approve-live-spend      Required before authenticated completion calls.
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

export const runKhalaGlmReapProductionSmoke = async ({
  approveLiveSpend = false,
  apiBasePath = defaultApiBasePath,
  baseUrl = defaultBaseUrl,
  counterPath = defaultCounterPath,
  counterPollMs = 2_500,
  counterPolls = 8,
  env = process.env,
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
    const skipped = {
      arming: {
        armed: false,
        blockerRefs: arming.blockerRefs,
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
  const khalaSmoke = await runKhalaProductionSmoke({
    approveLiveSpend,
    apiBasePath,
    baseUrl: origin,
    expectedServedModelContains: glmServedModel,
    expectedSupplyLane: 'hydralisk',
    expectedWorker: glmWorkerId,
    fetchImpl,
    forbiddenPublicModelIds,
    model: khalaModel,
    prompt,
    readinessOnly,
    token,
  })

  if (readinessOnly) {
    return {
      arming: { armed: true, blockerRefs: [] },
      ...khalaSmoke,
      counter: null,
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
