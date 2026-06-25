#!/usr/bin/env node

import { setTimeout as sleep } from 'node:timers/promises'

import { runKhalaProductionSmoke } from './khala-production-smoke.mjs'

const defaultBaseUrl = 'https://openagents.com'
const defaultApiPrefix = '/api/v1'
const defaultModel = 'openagents/khala'
const defaultPrompt =
  'OpenAgents Khala GLM REAP smoke: answer with exactly READY and no other words.'
const expectedSupplyLane = 'hydralisk'
const expectedWorker = 'hydralisk-vllm-glm-5p2-reap-504b'
const expectedServedModelContains = 'glm-5.2-reap-504b'
const rawGlmModelId = 'openagents/glm-5.2-reap-504b'
const defaultForbiddenPublicModelIds = [
  rawGlmModelId,
  'glm-5.2-reap-504b',
  'openagents/khala-code',
  'openagents/khala-mini',
  'openai/gpt-oss-120b',
  'openai/gpt-oss-20b',
]
const defaultServingProfileRefs = [
  'glm-reap-504b-g4-tp4-minp-rp105',
  'glm-reap-504b-g4-tp4-mtp2-rp105',
  'glm-reap-504b-g4-dual-tp4-minp-rp105',
]
const publicSafeRefPattern = /^[a-z0-9][a-z0-9._:-]{1,199}$/iu

const trimBaseUrl = baseUrl =>
  String(baseUrl || defaultBaseUrl).replace(/\/+$/, '')

const absoluteUrl = (baseUrl, pathOrUrl) =>
  new URL(String(pathOrUrl || ''), baseUrl).toString()

const truthy = value =>
  ['1', 'true', 'yes', 'on'].includes(
    String(value || '')
      .trim()
      .toLowerCase(),
  )

const csv = value =>
  String(value || '')
    .split(',')
    .map(item => item.trim())
    .filter(Boolean)

const isPresent = value => typeof value === 'string' && value.trim() !== ''

const isPublicSafeRef = value => {
  if (typeof value !== 'string' || value.trim() === '') {
    return false
  }
  const trimmed = value.trim()
  return (
    trimmed === value &&
    publicSafeRefPattern.test(trimmed) &&
    !trimmed.includes('://') &&
    !trimmed.toLowerCase().startsWith('sk-')
  )
}

const assert = (condition, message) => {
  if (!condition) {
    throw new Error(message)
  }
}

const okStatus = response => response.status >= 200 && response.status < 300

const redact = value => {
  if (typeof value !== 'string') {
    return value
  }
  return value
    .replace(/Bearer\s+[A-Za-z0-9._~+/=-]+/gu, 'Bearer <redacted>')
    .replace(/oa_agent_[A-Za-z0-9._~+/=-]+/gu, 'oa_agent_<redacted>')
    .replace(/sk-[A-Za-z0-9]{8,}/gu, 'sk-<redacted>')
}

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

const numberFrom = value => {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : null
}

const publicTokensServedFrom = body =>
  numberFrom(body?.tokensServed ?? body?.tokens_served)

const receiptTokensFrom = receiptSummary =>
  numberFrom(
    receiptSummary?.modelEvidence?.total_tokens ??
      receiptSummary?.modelEvidence?.totalTokens,
  )

export const resolveGlmReapArming = (env = process.env) => {
  const blockerRefs = []
  const evidenceRefs = []
  const evidence = [
    [
      'blocker.hydralisk_glm_52_reap_504b.preflight_ref_missing',
      env.HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF,
    ],
    [
      'blocker.hydralisk_glm_52_reap_504b.receipt_ref_missing',
      env.HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF,
    ],
  ]

  if (env.HYDRALISK_GLM_52_REAP_504B_ENABLED?.trim() !== 'ready') {
    blockerRefs.push('blocker.hydralisk_glm_52_reap_504b.route_not_ready')
  }
  if (!isPresent(env.HYDRALISK_GLM_52_REAP_504B_BASE_URL)) {
    blockerRefs.push('blocker.hydralisk_glm_52_reap_504b.base_url_missing')
  }
  if (!isPresent(env.HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN)) {
    blockerRefs.push('blocker.hydralisk_glm_52_reap_504b.bearer_missing')
  }

  for (const [blockerRef, value] of evidence) {
    if (isPublicSafeRef(value)) {
      evidenceRefs.push(value)
    } else {
      blockerRefs.push(blockerRef)
    }
  }

  return {
    armed: blockerRefs.length === 0,
    blockerRefs,
    evidenceRefs,
  }
}

export const parseArgs = (argv, env = process.env) => {
  const options = {
    apiPrefix: env.OPENAGENTS_KHALA_GLM_REAP_API_PREFIX || defaultApiPrefix,
    approveLiveSpend: truthy(
      env.OPENAGENTS_KHALA_GLM_REAP_SMOKE_APPROVE_LIVE_SPEND,
    ),
    baseUrl: env.OPENAGENTS_BASE_URL || defaultBaseUrl,
    counterSettleMs: Number(
      env.OPENAGENTS_KHALA_GLM_REAP_COUNTER_SETTLE_MS || 3000,
    ),
    counterToleranceTokens: Number(
      env.OPENAGENTS_KHALA_GLM_REAP_COUNTER_TOLERANCE_TOKENS || 0,
    ),
    forbiddenPublicModelIds: [
      ...defaultForbiddenPublicModelIds,
      ...csv(env.OPENAGENTS_KHALA_GLM_REAP_FORBIDDEN_PUBLIC_MODEL_IDS),
    ],
    model: env.OPENAGENTS_KHALA_GLM_REAP_SMOKE_MODEL || defaultModel,
    prompt: env.OPENAGENTS_KHALA_GLM_REAP_SMOKE_PROMPT || defaultPrompt,
    servingProfileRefs: [
      ...defaultServingProfileRefs,
      ...csv(env.OPENAGENTS_KHALA_GLM_REAP_SERVING_PROFILE_REFS),
    ],
    token:
      env.OPENAGENTS_KHALA_GLM_REAP_SMOKE_TOKEN ||
      env.OPENAGENTS_AGENT_TOKEN ||
      '',
  }

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]
    if (value === '--api-prefix') {
      options.apiPrefix = argv[++index] || options.apiPrefix
    } else if (value === '--base-url' || value === '--baseUrl') {
      options.baseUrl = argv[++index] || options.baseUrl
    } else if (value === '--counter-settle-ms') {
      options.counterSettleMs = Number(argv[++index] || options.counterSettleMs)
    } else if (value === '--counter-tolerance-tokens') {
      options.counterToleranceTokens = Number(
        argv[++index] || options.counterToleranceTokens,
      )
    } else if (value === '--forbid-public-model') {
      const forbidden = argv[++index]
      if (forbidden) {
        options.forbiddenPublicModelIds.push(forbidden)
      }
    } else if (value === '--model') {
      options.model = argv[++index] || options.model
    } else if (value === '--prompt') {
      options.prompt = argv[++index] || options.prompt
    } else if (value === '--serving-profile-ref') {
      const profileRef = argv[++index]
      if (profileRef) {
        options.servingProfileRefs.push(profileRef)
      }
    } else if (value === '--token') {
      options.token = argv[++index] || options.token
    } else if (value === '--approve-live-spend') {
      options.approveLiveSpend = true
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
    node scripts/khala-glm-reap-smoke.mjs --approve-live-spend

Required arming env for a live run:
  HYDRALISK_GLM_52_REAP_504B_ENABLED=ready
  HYDRALISK_GLM_52_REAP_504B_BASE_URL=<secret endpoint, never printed>
  HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN=<secret bearer, never printed>
  HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF=<public-safe ref>
  HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF=<public-safe ref>

Options:
  --api-prefix <path>              Gateway prefix. Defaults to /api/v1.
  --base-url <url>                 OpenAgents origin. Defaults to https://openagents.com.
  --counter-settle-ms <ms>         Wait before reading the public counter after the smoke.
  --counter-tolerance-tokens <n>   Allowed shortfall against receipt token sum. Defaults to 0.
  --forbid-public-model <model>    Add a model id that must not appear in /api/v1/models.
  --model <model>                  Defaults to openagents/khala.
  --prompt <text>                  Prompt used for both completion calls.
  --serving-profile-ref <ref>      Public-safe serving profile ref to include in proof output.
  --token <token>                  Agent bearer token. Defaults to OPENAGENTS_AGENT_TOKEN.
  --approve-live-spend             Required before authenticated completion calls.

When the GLM REAP arming env is absent or unsafe, this exits 0 with state=skipped.
It never prints bearer tokens, raw endpoint URLs, prompts, or completion text.
`

export const runKhalaGlmReapSmoke = async ({
  apiPrefix = defaultApiPrefix,
  approveLiveSpend = false,
  baseUrl = defaultBaseUrl,
  counterSettleMs = 3000,
  counterToleranceTokens = 0,
  env = process.env,
  fetchImpl = globalThis.fetch,
  forbiddenPublicModelIds = defaultForbiddenPublicModelIds,
  model = defaultModel,
  prompt = defaultPrompt,
  servingProfileRefs = defaultServingProfileRefs,
  token = '',
} = {}) => {
  assert(typeof fetchImpl === 'function', 'A fetch implementation is required.')

  const arming = resolveGlmReapArming(env)
  const safeServingProfileRefs = servingProfileRefs.filter(isPublicSafeRef)
  if (!arming.armed) {
    return {
      arming: {
        armed: false,
        blockerRefs: arming.blockerRefs,
        evidenceRefs: arming.evidenceRefs,
        servingProfileRefs: safeServingProfileRefs,
      },
      ok: true,
      state: 'skipped',
    }
  }

  assert(
    safeServingProfileRefs.length > 0,
    'At least one public-safe GLM serving profile ref is required.',
  )

  const origin = trimBaseUrl(baseUrl)
  const checks = []
  const check = (name, passed, details = {}) => {
    checks.push({ details, name, passed: Boolean(passed) })
    assert(passed, `${name} failed`)
  }

  check('arming_ready', arming.armed, {
    evidenceRefs: arming.evidenceRefs,
    servingProfileRefs: safeServingProfileRefs,
  })

  const before = await requestJson(
    fetchImpl,
    origin,
    '/api/public/khala-tokens-served',
  )
  const beforeTokens = publicTokensServedFrom(before.body)
  check('public_tokens_counter_before', okStatus(before.response), {
    status: before.response.status,
  })
  check('public_tokens_counter_before_value', beforeTokens !== null, {
    tokensServed: beforeTokens,
  })

  const smoke = await runKhalaProductionSmoke({
    apiPrefix,
    approveLiveSpend,
    baseUrl: origin,
    expectedServedModelContains,
    expectedSupplyLane,
    expectedWorker,
    fetchImpl,
    forbiddenPublicModelIds,
    model,
    prompt,
    token,
  })
  check('khala_glm_backing_smoke', smoke.ok === true, {
    nonstreamWorker: smoke.nonstream?.openagents?.worker,
    streamWorker: smoke.stream?.openagents?.worker,
  })

  if (Number(counterSettleMs) > 0) {
    await sleep(Number(counterSettleMs))
  }

  const after = await requestJson(
    fetchImpl,
    origin,
    '/api/public/khala-tokens-served',
  )
  const afterTokens = publicTokensServedFrom(after.body)
  check('public_tokens_counter_after', okStatus(after.response), {
    status: after.response.status,
  })
  check('public_tokens_counter_after_value', afterTokens !== null, {
    tokensServed: afterTokens,
  })

  const nonstreamReceiptTokens = receiptTokensFrom(smoke.nonstream?.receipt) ?? 0
  const streamReceiptTokens = receiptTokensFrom(smoke.stream?.receipt) ?? 0
  const expectedCounterDelta = Math.max(
    1,
    nonstreamReceiptTokens + streamReceiptTokens - Number(counterToleranceTokens),
  )
  const counterDelta = afterTokens - beforeTokens
  check('public_tokens_counter_delta', counterDelta >= expectedCounterDelta, {
    afterTokens,
    beforeTokens,
    counterDelta,
    expectedCounterDelta,
    nonstreamReceiptTokens,
    streamReceiptTokens,
  })

  return {
    apiPrefix: smoke.apiPrefix,
    arming: {
      armed: true,
      evidenceRefs: arming.evidenceRefs,
      servingProfileRefs: safeServingProfileRefs,
    },
    checks: [...checks, ...smoke.checks],
    expected: {
      forbiddenPublicModelIds,
      servedModelContains: expectedServedModelContains,
      supplyLane: expectedSupplyLane,
      worker: expectedWorker,
    },
    model,
    nonstream: smoke.nonstream,
    ok: true,
    publicTokensServed: {
      afterTokens,
      beforeTokens,
      counterDelta,
      expectedCounterDelta,
    },
    state: 'ok',
    stream: smoke.stream,
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const options = parseArgs(process.argv.slice(2))
  if (options.help) {
    console.log(usage())
  } else {
    runKhalaGlmReapSmoke(options)
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
