#!/usr/bin/env node

import { pathToFileURL } from 'node:url'

const DEFAULT_BASE_URL = 'https://openagents.com'
const DEFAULT_MODEL = 'openagents/khala'

const DEFAULT_FORBIDDEN_PUBLIC_MODEL_IDS = [
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
  'openagents/khala-pro',
]

const DEFAULT_FORBIDDEN_PUBLIC_MODEL_PATTERNS = [
  'accounts/',
  'deepseek',
  'fireworks',
  'glm',
  'gpt-oss',
  'hydralisk',
  'khala-code',
  'khala-mini',
  'khala-oss',
  'khala-pro',
  'vllm',
]

const csv = value =>
  String(value || '')
    .split(',')
    .map(item => item.trim())
    .filter(Boolean)

const trimBaseUrl = value =>
  String(value || DEFAULT_BASE_URL).replace(/\/+$/, '')

const normalize = value =>
  String(value || '')
    .trim()
    .toLowerCase()

export const redactSecrets = value =>
  String(value)
    .replace(/Bearer\s+[A-Za-z0-9._~+/=-]+/gu, 'Bearer <redacted>')
    .replace(/oa_agent_[A-Za-z0-9._~+/=-]+/gu, 'oa_agent_<redacted>')
    .replace(/sk-[A-Za-z0-9]{8,}/gu, 'sk-<redacted>')
    .replace(/gh[pousr]_[A-Za-z0-9_]+/gu, 'gh_<redacted>')
    .replace(/(OPENAGENTS_[A-Z0-9_]*TOKEN=)[^\s]+/gu, '$1<redacted>')

export const safeOutput = value =>
  JSON.parse(redactSecrets(JSON.stringify(value, null, 2)))

export const usage = () => `Usage:
  node scripts/khala-production-readiness-monitor.mjs [options]

Options:
  --base-url <url>                       OpenAgents origin. Defaults to ${DEFAULT_BASE_URL}.
  --model <model>                        Expected single public model. Defaults to ${DEFAULT_MODEL}.
  --forbid-public-model <model>          Add an exact public model id that must not appear.
  --forbid-public-model-pattern <text>   Add a case-insensitive leak pattern for public model ids.
  --help                                 Show this message.

This monitor performs only public no-spend reads:
  GET /v1/gateway/readiness
  GET /v1/models

It never calls /v1/chat/completions, never sends a bearer token, never mutates
state, and never performs paid inference. Use khala-production-smoke.mjs with
--approve-live-spend for the separate paid receipt-dereference proof.
`

export const parseArgs = (argv, env = process.env) => {
  const options = {
    baseUrl: env.OPENAGENTS_BASE_URL || DEFAULT_BASE_URL,
    forbiddenPublicModelIds: [
      ...DEFAULT_FORBIDDEN_PUBLIC_MODEL_IDS,
      ...csv(env.OPENAGENTS_KHALA_FORBIDDEN_PUBLIC_MODEL_IDS),
    ],
    forbiddenPublicModelPatterns: [
      ...DEFAULT_FORBIDDEN_PUBLIC_MODEL_PATTERNS,
      ...csv(env.OPENAGENTS_KHALA_FORBIDDEN_PUBLIC_MODEL_PATTERNS),
    ],
    model: env.OPENAGENTS_KHALA_MONITOR_MODEL || DEFAULT_MODEL,
  }

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]
    if (value === '--help' || value === '-h') {
      options.help = true
    } else if (value === '--base-url' || value === '--baseUrl') {
      options.baseUrl = argv[++index] || options.baseUrl
    } else if (value === '--model') {
      options.model = argv[++index] || options.model
    } else if (value === '--forbid-public-model') {
      const forbidden = argv[++index]
      if (forbidden) {
        options.forbiddenPublicModelIds.push(forbidden)
      }
    } else if (value === '--forbid-public-model-pattern') {
      const forbidden = argv[++index]
      if (forbidden) {
        options.forbiddenPublicModelPatterns.push(forbidden)
      }
    } else {
      throw new Error(`Unknown argument: ${value}`)
    }
  }

  return options
}

const readJson = async response => {
  const text = await response.text()
  try {
    return text.length === 0 ? null : JSON.parse(text)
  } catch {
    return { rawText: text }
  }
}

const requestJson = async (fetchImpl, baseUrl, path) => {
  try {
    const response = await fetchImpl(new URL(path, baseUrl).toString(), {
      headers: { accept: 'application/json' },
      method: 'GET',
    })
    return {
      body: await readJson(response),
      error: null,
      status: response.status,
    }
  } catch (error) {
    return {
      body: null,
      error: error instanceof Error ? error.message : String(error),
      status: 0,
    }
  }
}

const okStatus = status => status >= 200 && status < 300

const modelIdsFrom = body =>
  Array.isArray(body?.data)
    ? body.data.map(model => model?.id).filter(id => typeof id === 'string')
    : []

const publicModelLeaks = ({
  forbiddenIds,
  forbiddenPatterns,
  modelIds,
}) => {
  const forbidden = new Set(forbiddenIds.map(normalize))
  const patterns = forbiddenPatterns.map(normalize)

  return modelIds.flatMap(modelId => {
    const normalized = normalize(modelId)
    const reasons = [
      ...(forbidden.has(normalized) ? ['exact_forbidden_id'] : []),
      ...patterns
        .filter(pattern => pattern.length > 0 && normalized.includes(pattern))
        .map(pattern => `pattern:${pattern}`),
    ]
    return reasons.length === 0 ? [] : [{ modelId, reasons }]
  })
}

const check = (name, passed, details = {}) => ({
  details,
  name,
  passed: Boolean(passed),
})

export const runKhalaProductionReadinessMonitor = async ({
  baseUrl = DEFAULT_BASE_URL,
  fetchImpl = globalThis.fetch,
  forbiddenPublicModelIds = DEFAULT_FORBIDDEN_PUBLIC_MODEL_IDS,
  forbiddenPublicModelPatterns = DEFAULT_FORBIDDEN_PUBLIC_MODEL_PATTERNS,
  model = DEFAULT_MODEL,
} = {}) => {
  if (typeof fetchImpl !== 'function') {
    throw new Error('A fetch implementation is required.')
  }

  const origin = trimBaseUrl(baseUrl)
  const [readiness, models] = await Promise.all([
    requestJson(fetchImpl, origin, '/v1/gateway/readiness'),
    requestJson(fetchImpl, origin, '/v1/models'),
  ])
  const modelIds = modelIdsFrom(models.body)
  const leaks = publicModelLeaks({
    forbiddenIds: forbiddenPublicModelIds,
    forbiddenPatterns: forbiddenPublicModelPatterns,
    modelIds,
  })
  const checks = [
    check('readiness_endpoint_200', okStatus(readiness.status), {
      error: readiness.error,
      status: readiness.status,
    }),
    check('readiness_status_ready', readiness.body?.status === 'ready', {
      status: readiness.body?.status,
    }),
    check(
      'readiness_has_servable_model',
      Number(readiness.body?.servableModelCount || 0) > 0,
      {
        servableModelCount: readiness.body?.servableModelCount,
      },
    ),
    check('models_endpoint_200', okStatus(models.status), {
      error: models.error,
      status: models.status,
    }),
    check(
      'models_public_surface_exactly_khala',
      modelIds.length === 1 && modelIds[0] === model,
      {
        expectedModel: model,
        modelCount: modelIds.length,
        modelIds,
      },
    ),
    check('models_public_leak_guard_clean', leaks.length === 0, {
      leaks,
    }),
  ]

  return {
    authority: {
      bearerTokenAllowed: false,
      chatCompletionAllowed: false,
      inferenceSpendAllowed: false,
      mutationAllowed: false,
    },
    baseUrl: origin,
    catalog: {
      leaks,
      modelCount: modelIds.length,
      modelIds,
    },
    checks,
    generatedAt: new Date().toISOString(),
    model,
    ok: checks.every(item => item.passed),
    readiness: {
      servableModelCount: readiness.body?.servableModelCount,
      status: readiness.body?.status,
    },
  }
}

export const main = async argv => {
  const options = parseArgs(argv)
  if (options.help) {
    console.log(usage())
    return 0
  }

  const output = await runKhalaProductionReadinessMonitor(options)
  console.log(JSON.stringify(safeOutput(output), null, 2))
  return output.ok ? 0 : 1
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2))
    .then(exitCode => {
      process.exitCode = exitCode
    })
    .catch(error => {
      console.error(redactSecrets(error instanceof Error ? error.message : String(error)))
      process.exitCode = 1
    })
}
