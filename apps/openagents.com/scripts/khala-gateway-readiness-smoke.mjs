#!/usr/bin/env node

const defaultBaseUrl = 'https://openagents.com'
const defaultModel = 'openagents/khala-mini'
const defaultPrompt =
  'Khala launch smoke: answer with one short sentence confirming the gateway is serving.'

const truthy = value =>
  ['1', 'true', 'yes', 'on'].includes(
    String(value || '')
      .trim()
      .toLowerCase(),
  )

const trimBaseUrl = baseUrl =>
  String(baseUrl || defaultBaseUrl).replace(/\/+$/, '')

const absoluteUrl = (baseUrl, pathOrUrl) =>
  new URL(String(pathOrUrl || ''), baseUrl).toString()

const assert = (condition, message) => {
  if (!condition) {
    throw new Error(message)
  }
}

const okStatus = response => response.status >= 200 && response.status < 300

const addCheck = (checks, name, passed, details = {}) => {
  checks.push({ details, name, passed: Boolean(passed) })
}

const assertCheck = (checks, name, condition, details = {}) => {
  addCheck(checks, name, condition, details)
  assert(condition, `${name} failed`)
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
  const url = absoluteUrl(baseUrl, path)
  const response = await fetchImpl(url, {
    ...init,
    headers: {
      accept: 'application/json',
      ...(init.headers || {}),
    },
  })
  const body = await readJson(response)
  return { body, response, url }
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

export const parseArgs = (argv, env = process.env) => {
  const options = {
    approveLiveSpend: truthy(env.KHALA_GATEWAY_SMOKE_APPROVE_LIVE_SPEND),
    baseUrl: env.OPENAGENTS_BASE_URL || defaultBaseUrl,
    model: env.OPENAGENTS_KHALA_SMOKE_MODEL || defaultModel,
    prompt: env.OPENAGENTS_KHALA_SMOKE_PROMPT || defaultPrompt,
    readinessOnly: false,
    token: env.OPENAGENTS_AGENT_TOKEN || '',
  }

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]

    if (value === '--base-url' || value === '--baseUrl') {
      options.baseUrl = argv[++index] || options.baseUrl
    } else if (value === '--model') {
      options.model = argv[++index] || options.model
    } else if (value === '--prompt') {
      options.prompt = argv[++index] || options.prompt
    } else if (value === '--token') {
      options.token = argv[++index] || options.token
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
    node scripts/khala-gateway-readiness-smoke.mjs --approve-live-spend

Options:
  --base-url <url>           OpenAgents origin. Defaults to https://openagents.com.
  --model <model>            Khala model to smoke. Defaults to openagents/khala-mini.
  --prompt <text>            Prompt for the authenticated completion.
  --token <token>            Agent bearer token. Defaults to OPENAGENTS_AGENT_TOKEN.
  --approve-live-spend       Required before the authenticated completion call.
  --readiness-only           Check readiness + model catalog without spending.

This smoke verifies /v1/gateway/readiness, /v1/models, an authenticated
/v1/chat/completions call, the OpenAgents receipt block, and the dereferenceable
receipt URL. It never prints bearer tokens.
`

const modelIdsFrom = body =>
  Array.isArray(body?.data)
    ? body.data.map(model => model?.id).filter(id => typeof id === 'string')
    : []

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
  return null
}

export const runKhalaGatewayReadinessSmoke = async ({
  approveLiveSpend = false,
  baseUrl = defaultBaseUrl,
  fetchImpl = globalThis.fetch,
  model = defaultModel,
  prompt = defaultPrompt,
  readinessOnly = false,
  token = '',
} = {}) => {
  assert(typeof fetchImpl === 'function', 'A fetch implementation is required.')

  const origin = trimBaseUrl(baseUrl)
  const checks = []

  const readinessResult = await requestJson(
    fetchImpl,
    origin,
    '/v1/gateway/readiness',
  )
  const readiness = readinessResult.body
  assertCheck(
    checks,
    'readiness_endpoint_200',
    okStatus(readinessResult.response),
    {
      status: readinessResult.response.status,
      url: readinessResult.url,
    },
  )
  assertCheck(
    checks,
    'readiness_has_servable_model',
    Number(readiness?.servableModelCount || 0) > 0,
    {
      servableModelCount: readiness?.servableModelCount,
      status: readiness?.status,
    },
  )

  const modelsResult = await requestJson(fetchImpl, origin, '/v1/models')
  const modelIds = modelIdsFrom(modelsResult.body)
  assertCheck(checks, 'models_endpoint_200', okStatus(modelsResult.response), {
    status: modelsResult.response.status,
    url: modelsResult.url,
  })
  assertCheck(
    checks,
    'models_lists_requested_khala_model',
    modelIds.includes(model),
    {
      model,
      modelCount: modelIds.length,
    },
  )

  if (readinessOnly) {
    return {
      checks,
      completion: null,
      model,
      ok: true,
      readiness: {
        servableModelCount: readiness?.servableModelCount,
        status: readiness?.status,
      },
      receipt: null,
    }
  }

  assert(
    token.trim() !== '',
    'Missing OPENAGENTS_AGENT_TOKEN or --token for authenticated completion smoke.',
  )
  assert(
    approveLiveSpend,
    'Refusing authenticated completion smoke without --approve-live-spend.',
  )

  const completionResult = await requestJson(
    fetchImpl,
    origin,
    '/v1/chat/completions',
    {
      body: JSON.stringify({
        messages: [{ content: prompt, role: 'user' }],
        model,
        stream: false,
      }),
      headers: {
        authorization: `Bearer ${token}`,
        'content-type': 'application/json',
      },
      method: 'POST',
    },
  )
  const completion = completionResult.body
  assertCheck(
    checks,
    'completion_endpoint_200',
    okStatus(completionResult.response),
    {
      status: completionResult.response.status,
      url: completionResult.url,
    },
  )
  assertCheck(
    checks,
    'completion_has_openagents_block',
    typeof completion?.openagents === 'object' &&
      completion.openagents !== null,
    { keys: Object.keys(completion?.openagents || {}) },
  )
  assertCheck(
    checks,
    'completion_echoes_requested_model',
    completion?.openagents?.requested_model === model,
    {
      requestedModel: completion?.openagents?.requested_model,
      smokeModel: model,
    },
  )

  const receiptPath = receiptPathFrom(completion.openagents)
  assertCheck(
    checks,
    'completion_has_dereferenceable_receipt_ref',
    receiptPath !== null,
    { receiptPath },
  )

  const receiptResult = await requestJson(fetchImpl, origin, receiptPath)
  assertCheck(
    checks,
    'receipt_endpoint_200',
    okStatus(receiptResult.response),
    {
      status: receiptResult.response.status,
      url: receiptResult.url,
    },
  )

  return {
    checks,
    completion: {
      openagents: completion.openagents,
      responseId: completion.id,
    },
    model,
    ok: true,
    readiness: {
      servableModelCount: readiness?.servableModelCount,
      status: readiness?.status,
    },
    receipt: {
      body: receiptResult.body,
      url: receiptResult.url,
    },
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const options = parseArgs(process.argv.slice(2))
  if (options.help) {
    console.log(usage())
  } else {
    runKhalaGatewayReadinessSmoke(options)
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
