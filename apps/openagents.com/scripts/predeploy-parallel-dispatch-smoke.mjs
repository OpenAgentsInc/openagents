#!/usr/bin/env node

const defaultBaseUrl = 'https://openagents-staging.openagents.workers.dev'
const defaultParallelism = 5

const trimBaseUrl = baseUrl =>
  String(baseUrl || defaultBaseUrl).replace(/\/+$/, '')

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

const readJson = async response => {
  const text = await response.text()
  try {
    return text.length === 0 ? null : JSON.parse(text)
  } catch {
    return { rawText: text }
  }
}

const safePayloadSummary = payload => {
  if (payload === null || typeof payload !== 'object') {
    return payload
  }
  return {
    assignmentRef: payload.assignment?.assignmentRef ?? payload.assignmentRef,
    blockerRefs: payload.dispatchGate?.blockerRefs ?? payload.blockerRefs,
    error: payload.error,
    evidenceRefs: payload.evidenceRefs,
    reason: payload.reason,
    requestedPylonRef: payload.requestedPylonRef,
  }
}

const requestJson = async (fetchImpl, baseUrl, path, init = {}) => {
  const url = new URL(path, baseUrl).toString()
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

const requireOk = async promise => {
  const result = await promise
  if (result.response.status >= 200 && result.response.status < 300) {
    return result
  }
  throw new Error(
    JSON.stringify(
      {
        body: safePayloadSummary(result.body),
        status: result.response.status,
        url: result.url,
      },
      null,
      2,
    ),
  )
}

const stableAccountKey = index => `6409${String(index).padStart(20, '0')}`

const defaultStagingOrigin = trimBaseUrl(defaultBaseUrl)

const slugPart = value =>
  String(value)
    .toLowerCase()
    .replace(/[^a-z0-9-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48) || 'run'

const registerStagingSmokeAgentToken = async (fetchImpl, baseUrl, runRef) => {
  const registration = await requireOk(
    requestJson(fetchImpl, baseUrl, '/api/agents/register', {
      body: JSON.stringify({
        displayName: 'Predeploy parallel dispatch smoke',
        externalId: `predeploy.parallel.dispatch.${runRef}`,
        metadata: {
          authority: 'staging_predeploy_parallel_dispatch_smoke',
          runRef,
        },
        slug: `predeploy-smoke-${slugPart(runRef)}`.slice(0, 80),
      }),
      headers: {
        'content-type': 'application/json',
      },
      method: 'POST',
    }),
  )
  const token = registration.body?.credential?.token
  assert(
    typeof token === 'string' && token.trim() !== '',
    'Staging smoke agent registration did not return a token.',
  )
  return token
}

export const parseArgs = (argv, env = process.env) => {
  const options = {
    approveStagingMutation: truthy(
      env.OPENAGENTS_PARALLEL_DISPATCH_SMOKE_APPROVE_STAGING_MUTATION,
    ),
    autoRegisterAgentToken: !truthy(
      env.OPENAGENTS_PARALLEL_DISPATCH_SMOKE_DISABLE_AUTO_REGISTER,
    ),
    baseUrl: env.OPENAGENTS_PARALLEL_DISPATCH_SMOKE_BASE_URL || defaultBaseUrl,
    parallelism: Number.parseInt(
      env.OPENAGENTS_PARALLEL_DISPATCH_SMOKE_PARALLELISM ||
        String(defaultParallelism),
      10,
    ),
    pylonRef:
      env.OPENAGENTS_PARALLEL_DISPATCH_SMOKE_PYLON_REF ||
      `pylon.predeploy.parallel_dispatch.${Date.now()}`,
    runRef:
      env.OPENAGENTS_PARALLEL_DISPATCH_SMOKE_RUN_REF ||
      `issue6409_${Date.now()}`,
    token: env.OPENAGENTS_AGENT_TOKEN || '',
  }

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]
    if (value === '--base-url' || value === '--baseUrl') {
      options.baseUrl = argv[++index] || options.baseUrl
    } else if (value === '--parallelism') {
      options.parallelism = Number.parseInt(argv[++index] || '', 10)
    } else if (value === '--pylon-ref') {
      options.pylonRef = argv[++index] || options.pylonRef
    } else if (value === '--run-ref') {
      options.runRef = argv[++index] || options.runRef
    } else if (value === '--token') {
      options.token = argv[++index] || options.token
    } else if (value === '--approve-staging-mutation') {
      options.approveStagingMutation = true
    } else if (value === '--no-auto-register-agent-token') {
      options.autoRegisterAgentToken = false
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
    node scripts/predeploy-parallel-dispatch-smoke.mjs --approve-staging-mutation

Options:
  --base-url <url>                 Staging origin. Defaults to ${defaultBaseUrl}.
  --parallelism <n>                Concurrent dummy Codex tasks. Defaults to 5.
  --pylon-ref <ref>                Staging-only dummy Pylon ref.
  --run-ref <ref>                  Public-safe run suffix for idempotency refs.
  --token <token>                  Agent bearer token. Defaults to OPENAGENTS_AGENT_TOKEN.
  --approve-staging-mutation       Required. This creates staging Pylon/assignment rows.
  --no-auto-register-agent-token   Do not self-register the staging smoke agent.

By default, the smoke self-registers a throwaway agent token when using the
default staging origin, so a production OPENAGENTS_AGENT_TOKEN cannot poison the
staging gate. Set OPENAGENTS_PARALLEL_DISPATCH_SMOKE_DISABLE_AUTO_REGISTER=1 or
pass --no-auto-register-agent-token to force the provided token. The smoke
registers a staging dummy Pylon, advertises one Codex slot for each dummy account
hash, then dispatches the no-spend dummy assignments concurrently. Any
duplicate_active_assignment response fails the deploy gate.
`

export const buildRegisterBody = pylonRef => ({
  capabilityRefs: ['capability.pylon.local_codex'],
  clientProtocolVersion: '0.3.0',
  clientVersion: 'pylon-v0.3.0',
  displayName: 'Predeploy parallel dispatch smoke',
  providerNip90LaneRefs: [],
  pylonRef,
  resourceMode: 'background_20',
  statusRefs: ['status.public.predeploy_parallel_dispatch_smoke'],
  walletRef: 'wallet.public.predeploy_parallel_dispatch_smoke.no_spend',
})

export const buildHeartbeatBody = parallelism => {
  const accountCapacityRefs = Array.from(
    { length: parallelism },
    (_, index) => [
      `capacity.coding.codex.account.${stableAccountKey(index)}.ready=1`,
      `capacity.coding.codex.account.${stableAccountKey(index)}.available=1`,
    ],
  ).flat()
  const accountLoadRefs = Array.from(
    { length: parallelism },
    (_, index) => `load.coding.codex.account.${stableAccountKey(index)}.busy=0`,
  )

  return {
    capabilityRefs: ['capability.pylon.local_codex'],
    capacityRefs: [
      `capacity.coding.codex.ready=${parallelism}`,
      `capacity.coding.codex.available=${parallelism}`,
      ...accountCapacityRefs,
    ],
    clientProtocolVersion: '0.3.0',
    clientVersion: 'pylon-v0.3.0',
    healthRefs: ['health.public.predeploy_parallel_dispatch_smoke.ok'],
    loadRefs: [
      'load.coding.codex.busy=0',
      'load.coding.codex.queued=0',
      ...accountLoadRefs,
    ],
    providerNip90LaneRefs: [],
    resourceMode: 'background_20',
    status: 'online',
    walletReady: true,
  }
}

export const buildAssignmentBody = ({ index, pylonRef, runRef }) => {
  const accountRefHash = `account.pylon.codex.${stableAccountKey(index)}`
  return {
    acceptanceCriteriaRefs: [
      'acceptance.public.issue6409.parallel_dispatch_dummy_task_created',
    ],
    assignmentRef: `assignment.public.issue6409.${runRef}.${index}`,
    campaignPaused: false,
    campaignPolicyRefs: ['policy.public.issue6409.predeploy_parallel_smoke'],
    campaignRef: 'campaign.public.issue6409.predeploy_parallel_dispatch_smoke',
    closeoutPathRefs: ['closeout.public.issue6409.staging_no_spend_smoke'],
    codingAssignment: {
      codex: {
        accountRefHash,
        agentKind: 'codex_sdk',
        fixtureRef: 'fixture.public.issue6409.parallel_dispatch_dummy',
        schema: 'openagents.pylon.codex_agent_task.v0.3',
        timeoutSeconds: 60,
      },
      kind: 'codex_agent_task',
      objective: {
        objectiveRef: 'goal.public.issue6409.parallel_dispatch_dummy',
      },
      requiredCapabilityRefs: ['capability.pylon.local_codex'],
    },
    forumAutoPublishAllowed: false,
    idempotencyRefs: [`idempotency.public.issue6409.${runRef}.${index}`],
    jobKind: 'codex_agent_task',
    leaseSeconds: 600,
    noDuplicateAssignmentRefs: ['dedupe.public.pylon_assignment.active_lease'],
    noForumAutoPublishRefs: ['policy.public.no_forum_auto_publish'],
    operatorPauseRefs: ['pause.public.issue6409.predeploy_parallel_smoke'],
    paymentMode: 'unpaid_smoke',
    pylonRef,
    requiredCapabilityRefs: ['capability.pylon.local_codex'],
    resultExpectationRefs: [
      'result.public.issue6409.no_duplicate_active_assignment_regression',
    ],
    rollbackRefs: ['rollback.public.issue6409.cancel_staging_dummy_assignments'],
    selectionPolicyRefs: ['selection.public.issue6409.staging_dummy_pylon'],
    spendCapRefs: [],
    taskRefs: [`task.public.issue6409.parallel_dispatch_dummy.${index}`],
  }
}

export const runPredeployParallelDispatchSmoke = async ({
  approveStagingMutation = false,
  autoRegisterAgentToken = true,
  baseUrl = defaultBaseUrl,
  fetchImpl = globalThis.fetch,
  parallelism = defaultParallelism,
  pylonRef,
  runRef,
  token = '',
} = {}) => {
  assert(typeof fetchImpl === 'function', 'A fetch implementation is required.')
  assert(
    approveStagingMutation,
    'Refusing staging mutation smoke without --approve-staging-mutation.',
  )
  assert(
    token.trim() !== '' || autoRegisterAgentToken,
    'Missing OPENAGENTS_AGENT_TOKEN or --token for staging dispatch smoke.',
  )
  assert(
    Number.isInteger(parallelism) && parallelism >= 5 && parallelism <= 20,
    'parallelism must be an integer between 5 and 20.',
  )
  assert(
    typeof pylonRef === 'string' && pylonRef.trim() !== '',
    'pylonRef is required.',
  )
  assert(
    typeof runRef === 'string' && runRef.trim() !== '',
    'runRef is required.',
  )

  const origin = trimBaseUrl(baseUrl)
  const resolvedToken =
    autoRegisterAgentToken && origin === defaultStagingOrigin
      ? await registerStagingSmokeAgentToken(fetchImpl, origin, runRef)
      : token
  assert(
    resolvedToken.trim() !== '',
    'Missing OPENAGENTS_AGENT_TOKEN or --token for staging dispatch smoke.',
  )
  const authHeaders = {
    authorization: `Bearer ${resolvedToken}`,
    'content-type': 'application/json',
  }

  await requireOk(
    requestJson(fetchImpl, origin, '/api/pylons/register', {
      body: JSON.stringify(buildRegisterBody(pylonRef)),
      headers: {
        ...authHeaders,
        'Idempotency-Key': `issue6409-register-${runRef}`,
      },
      method: 'POST',
    }),
  )

  await requireOk(
    requestJson(fetchImpl, origin, `/api/pylons/${pylonRef}/heartbeat`, {
      body: JSON.stringify(buildHeartbeatBody(parallelism)),
      headers: {
        ...authHeaders,
        'Idempotency-Key': `issue6409-heartbeat-${runRef}`,
      },
      method: 'POST',
    }),
  )

  const assignmentResults = await Promise.all(
    Array.from({ length: parallelism }, (_, index) =>
      requireOk(
        requestJson(fetchImpl, origin, '/api/operator/pylons/assignments', {
          body: JSON.stringify(buildAssignmentBody({ index, pylonRef, runRef })),
          headers: {
            ...authHeaders,
            'Idempotency-Key': `issue6409-assignment-${runRef}-${index}`,
          },
          method: 'POST',
        }),
      ).then(result => ({
        assignmentRef: result.body?.assignment?.assignmentRef ?? null,
        index,
        status: result.response.status,
      })),
    ),
  )

  return {
    assignmentResults,
    baseUrl: origin,
    ok: true,
    parallelism,
    pylonRef,
    runRef,
  }
}

const main = async () => {
  const options = parseArgs(process.argv.slice(2))
  if (options.help) {
    console.log(usage())
    return
  }
  const result = await runPredeployParallelDispatchSmoke(options)
  console.log(JSON.stringify(result, null, 2))
}

if (import.meta.main) {
  main().catch(error => {
    console.error(`✘ predeploy parallel dispatch smoke failed: ${error.message}`)
    process.exit(1)
  })
}
