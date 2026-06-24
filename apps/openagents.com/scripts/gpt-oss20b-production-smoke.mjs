#!/usr/bin/env node

const defaultBaseUrl = 'https://openagents.com'
const defaultModel = 'openai/gpt-oss-20b'
const defaultPrompt =
  'OpenAgents production smoke: answer with exactly READY and no other words.'
const forbiddenInfrastructureTerms = ['hydralisk', 'vllm', 'provider']

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

const modelIdsFrom = body =>
  Array.isArray(body?.data)
    ? body.data.map(model => model?.id).filter(id => typeof id === 'string')
    : []

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

const summarizeOpenAgents = openagents => ({
  lane: openagents?.lane,
  requested_model: openagents?.requested_model,
  served_model: openagents?.served_model,
  supply_lane: openagents?.supply_lane,
  worker: openagents?.worker,
})

export const parseArgs = (argv, env = process.env) => {
  const options = {
    approveLiveSpend: truthy(env.GPT_OSS20B_SMOKE_APPROVE_LIVE_SPEND),
    baseUrl: env.OPENAGENTS_BASE_URL || defaultBaseUrl,
    model: env.OPENAGENTS_GPT_OSS20B_SMOKE_MODEL || defaultModel,
    prompt: env.OPENAGENTS_GPT_OSS20B_SMOKE_PROMPT || defaultPrompt,
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
    node scripts/gpt-oss20b-production-smoke.mjs --approve-live-spend

Options:
  --base-url <url>           OpenAgents origin. Defaults to https://openagents.com.
  --model <model>            Defaults to openai/gpt-oss-20b.
  --prompt <text>            Prompt used for both nonstreaming and streaming calls.
  --token <token>            Agent bearer token. Defaults to OPENAGENTS_AGENT_TOKEN.
  --approve-live-spend       Required before authenticated completion calls.
  --readiness-only           Check readiness + model catalog without spending.

This smoke verifies /v1/gateway/readiness, /v1/models, nonstreaming chat,
streaming chat, usage/disclosure blocks, and a simple infrastructure-leak guard.
It never prints bearer tokens or raw completion text.
`

export const runGptOss20bProductionSmoke = async ({
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
  const check = (name, passed, details = {}) => {
    checks.push({ details, name, passed: Boolean(passed) })
    assert(passed, `${name} failed`)
  }

  const readiness = await requestJson(
    fetchImpl,
    origin,
    '/v1/gateway/readiness',
  )
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

  const models = await requestJson(fetchImpl, origin, '/v1/models')
  const modelIds = modelIdsFrom(models.body)
  check('models_endpoint_200', okStatus(models.response), {
    status: models.response.status,
  })
  check('models_lists_gpt_oss20b', modelIds.includes(model), {
    model,
    modelCount: modelIds.length,
  })

  if (readinessOnly) {
    return {
      checks,
      model,
      ok: true,
      readiness: {
        servableModelCount: readiness.body?.servableModelCount,
        status: readiness.body?.status,
      },
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
  const chatBody = {
    messages: [{ content: prompt, role: 'user' }],
    model,
  }

  const completion = await requestJson(
    fetchImpl,
    origin,
    '/v1/chat/completions',
    {
      body: JSON.stringify({ ...chatBody, stream: false }),
      headers: authHeaders,
      method: 'POST',
    },
  )
  check('nonstream_completion_200', okStatus(completion.response), {
    status: completion.response.status,
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
    'nonstream_hydralisk_disclosure_present',
    completion.body?.openagents?.requested_model === model &&
      completion.body?.openagents?.supply_lane === 'hydralisk',
    summarizeOpenAgents(completion.body?.openagents),
  )

  const streamResponse = await fetchImpl(
    absoluteUrl(origin, '/v1/chat/completions'),
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
    'stream_infrastructure_guard_clean',
    infrastructureLeaks(streamedContent).length === 0,
  )
  check(
    'stream_hydralisk_disclosure_present',
    terminalOpenAgents?.requested_model === model &&
      terminalOpenAgents?.supply_lane === 'hydralisk',
    summarizeOpenAgents(terminalOpenAgents),
  )

  return {
    checks,
    model,
    nonstream: {
      openagents: summarizeOpenAgents(completion.body?.openagents),
      responseId: completion.body?.id,
      totalTokens: completion.body?.usage?.total_tokens,
    },
    ok: true,
    readiness: {
      servableModelCount: readiness.body?.servableModelCount,
      status: readiness.body?.status,
    },
    stream: {
      frameCount: frames.length,
      openagents: summarizeOpenAgents(terminalOpenAgents),
    },
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const options = parseArgs(process.argv.slice(2))
  if (options.help) {
    console.log(usage())
  } else {
    runGptOss20bProductionSmoke(options)
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
