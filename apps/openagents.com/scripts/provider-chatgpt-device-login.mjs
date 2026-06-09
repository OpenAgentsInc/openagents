#!/usr/bin/env node

const args = process.argv.slice(2)
const command = args[0]

const valueAfter = name => {
  const index = args.indexOf(name)

  return index === -1 ? undefined : args[index + 1]
}

const hasFlag = name => args.includes(name)

const numberAfter = name => {
  const value = valueAfter(name)
  const number = value === undefined ? undefined : Number(value)

  return Number.isFinite(number) ? number : undefined
}

const usage = () => {
  console.error(`Usage:
  node scripts/provider-chatgpt-device-login.mjs start --label "account 1" --create-new [--email you@example.com]
  node scripts/provider-chatgpt-device-login.mjs poll <attempt-id>
  node scripts/provider-chatgpt-device-login.mjs sanity <provider-account-ref>
  node scripts/provider-chatgpt-device-login.mjs sanity --all [--parallel 5] [--email you@example.com]
  node scripts/provider-chatgpt-device-login.mjs lease --action <requested-action> [--runId run_...] [--assignmentId assignment_...] [--email you@example.com]
  node scripts/provider-chatgpt-device-login.mjs explain-lease [--email you@example.com]
  node scripts/provider-chatgpt-device-login.mjs dashboard [--email you@example.com]
  node scripts/provider-chatgpt-device-login.mjs leases [--email you@example.com]
  node scripts/provider-chatgpt-device-login.mjs touch-lease --leaseRef <lease-ref> [--ttlSeconds 900]
  node scripts/provider-chatgpt-device-login.mjs release-lease --leaseRef <lease-ref> [--status released|succeeded|failed]
  node scripts/provider-chatgpt-device-login.mjs failover --previousLeaseRef <lease-ref> --failureClass <class> --action <requested-action> [--attemptNumber 1] [--maxAttempts 3] [--email you@example.com]
  node scripts/provider-chatgpt-device-login.mjs failover-history [--runId run_...] [--assignmentId assignment_...] [--orderId order_...] [--email you@example.com]

Environment:
  OPENAGENTS_ADMIN_API_TOKEN required
  OPENAGENTS_BASE_URL defaults to https://openagents.com`)
}

const token = process.env.OPENAGENTS_ADMIN_API_TOKEN

if (token === undefined || token.trim() === '') {
  console.error('Missing OPENAGENTS_ADMIN_API_TOKEN.')
  process.exit(2)
}

const baseUrl = process.env.OPENAGENTS_BASE_URL ?? 'https://openagents.com'

const requestJson = async (path, init) => {
  const response = await fetch(new URL(path, baseUrl), {
    ...init,
    headers: {
      Authorization: `Bearer ${token}`,
      accept: 'application/json',
      ...(init.body === undefined
        ? {}
        : { 'content-type': 'application/json' }),
      ...(init.headers ?? {}),
    },
  })
  const payload = await response.json().catch(() => ({
    error: 'invalid_json_response',
    status: response.status,
  }))

  return { ok: response.ok, payload, status: response.status }
}

const printJsonOrExit = (ok, payload) => {
  console.log(JSON.stringify(payload, null, 2))
  process.exit(ok ? 0 : 1)
}

const targetSelector = () => ({
  ...(valueAfter('--userId') === undefined
    ? {}
    : { userId: valueAfter('--userId') }),
  ...(valueAfter('--email') === undefined
    ? {}
    : { email: valueAfter('--email') }),
  ...(valueAfter('--githubLogin') === undefined
    ? {}
    : { githubLogin: valueAfter('--githubLogin') }),
  ...(valueAfter('--login') === undefined
    ? {}
    : { login: valueAfter('--login') }),
})

if (command === 'start') {
  const body = {
    ...targetSelector(),
    ...(valueAfter('--label') === undefined
      ? {}
      : { accountLabel: valueAfter('--label') }),
    ...(hasFlag('--create-new') ? { createNew: true } : {}),
    ...(valueAfter('--providerAccountRef') === undefined
      ? {}
      : { providerAccountRef: valueAfter('--providerAccountRef') }),
  }
  const { ok, payload, status } = await requestJson(
    '/api/operator/provider-accounts/chatgpt-codex/device-login/start',
    {
      body: JSON.stringify(body),
      method: 'POST',
    },
  )

  if (hasFlag('--json')) {
    printJsonOrExit(ok, payload)
  }

  if (!ok) {
    console.error(`Device login start failed: ${payload.error ?? status}`)
    if (payload.message !== undefined) {
      console.error(payload.message)
    }
    process.exit(1)
  }

  console.log('ChatGPT/Codex device login started')
  console.log(
    `Target user: ${payload.targetUser?.email ?? payload.targetUser?.userId}`,
  )
  console.log(
    `Account label: ${payload.accountLabel ?? body.accountLabel ?? 'Unlabeled'}`,
  )
  console.log(`Attempt ID: ${payload.attemptId}`)
  console.log(`Provider account ref: ${payload.providerAccountRef}`)
  console.log(`Verification URL: ${payload.verificationUrl}`)
  console.log(`User code: ${payload.userCode}`)
  console.log(`Expires at: ${payload.expiresAt}`)
  console.log(`Next poll: ${payload.nextPollCommand}`)
  process.exit(0)
}

if (command === 'poll') {
  const attemptId = args[1]

  if (attemptId === undefined || attemptId.trim() === '') {
    usage()
    process.exit(2)
  }

  const { ok, payload, status } = await requestJson(
    `/api/operator/provider-accounts/chatgpt-codex/device-login/${encodeURIComponent(attemptId)}`,
    { method: 'GET' },
  )

  if (hasFlag('--json')) {
    printJsonOrExit(ok, payload)
  }

  if (!ok) {
    console.error(`Device login poll failed: ${payload.error ?? status}`)
    if (payload.message !== undefined) {
      console.error(payload.message)
    }
    process.exit(1)
  }

  console.log(`Device login status: ${payload.status}`)
  console.log(`Attempt ID: ${payload.attemptId}`)
  console.log(`Provider account ref: ${payload.providerAccountRef}`)
  console.log(`Account label: ${payload.accountLabel ?? 'Unlabeled'}`)
  console.log(`Account status: ${payload.providerAccountStatus}`)
  console.log(`Account health: ${payload.providerAccountHealth}`)

  if (payload.failureReason !== null && payload.failureReason !== undefined) {
    console.log(`Failure reason: ${payload.failureReason}`)
  }

  process.exit(['denied', 'expired', 'failed'].includes(payload.status) ? 1 : 0)
}

if (command === 'sanity') {
  const providerAccountRef = args[1]
  const parallel = numberAfter('--parallel')
  const body = {
    ...targetSelector(),
    ...(parallel === undefined ? {} : { parallel }),
    ...(hasFlag('--all')
      ? { all: true }
      : { providerAccountRef: providerAccountRef }),
  }

  if (
    body.all !== true &&
    (body.providerAccountRef === undefined || body.providerAccountRef === '')
  ) {
    usage()
    process.exit(2)
  }

  const { ok, payload, status } = await requestJson(
    '/api/operator/provider-accounts/chatgpt-codex/sanity',
    {
      body: JSON.stringify(body),
      method: 'POST',
    },
  )

  if (hasFlag('--json')) {
    printJsonOrExit(ok, payload)
  }

  if (!ok) {
    console.error(`Sanity check failed: ${payload.error ?? status}`)
    if (payload.message !== undefined) {
      console.error(payload.message)
    }
    process.exit(1)
  }

  console.log(
    `ChatGPT/Codex sanity checks: ${payload.summary?.healthy ?? 0}/${payload.summary?.total ?? 0} healthy, ${payload.summary?.collisionCount ?? 0} collisions`,
  )

  if (payload.probeRunId !== null && payload.probeRunId !== undefined) {
    console.log(
      `Parallel probe: ${payload.probeRunId} at concurrency ${payload.parallel ?? parallel ?? 1}`,
    )
  }

  if (Array.isArray(payload.checks)) {
    for (const check of payload.checks) {
      console.log(
        `- ${check.classification} ${check.providerAccountRef} (${check.accountLabel ?? 'Unlabeled'}): ${check.summary} [probe=${check.probeId ?? 'none'} lease=${check.leaseId ?? 'none'} collision=${check.collisionClass ?? 'none'}]`,
      )
    }
  }

  process.exit(
    payload.summary?.requiresAttention === 0 &&
      (payload.summary?.collisionCount ?? 0) === 0
      ? 0
      : 1,
  )
}

if (command === 'lease') {
  const requestedAction = valueAfter('--action')

  if (requestedAction === undefined || requestedAction.trim() === '') {
    usage()
    process.exit(2)
  }

  const body = {
    ...targetSelector(),
    requestedAction,
    ...(valueAfter('--runId') === undefined
      ? {}
      : { runId: valueAfter('--runId') }),
    ...(valueAfter('--assignmentId') === undefined
      ? {}
      : { assignmentId: valueAfter('--assignmentId') }),
    ...(valueAfter('--orderId') === undefined
      ? {}
      : { orderId: valueAfter('--orderId') }),
    ...(numberAfter('--ttlSeconds') === undefined
      ? {}
      : { ttlSeconds: numberAfter('--ttlSeconds') }),
  }
  const { ok, payload, status } = await requestJson(
    '/api/operator/provider-accounts/chatgpt-codex/leases',
    {
      body: JSON.stringify(body),
      method: 'POST',
    },
  )

  if (hasFlag('--json')) {
    printJsonOrExit(ok, payload)
  }

  if (!ok) {
    console.error(`Lease acquisition failed: ${payload.error ?? status}`)
    if (payload.reason !== undefined) {
      console.error(payload.reason)
    }
    if (payload.message !== undefined) {
      console.error(payload.message)
    }
    process.exit(1)
  }

  console.log('ChatGPT/Codex account lease acquired')
  console.log(`Lease ref: ${payload.leaseRef}`)
  console.log(`Provider account ref: ${payload.providerAccountRef}`)
  console.log(`Account label: ${payload.accountLabel ?? 'Unlabeled'}`)
  console.log(`Requested action: ${payload.requestedAction}`)
  console.log(`Policy: ${payload.selectedByPolicyVersion}`)
  console.log(`Reason: ${payload.selectionReason}`)
  console.log(`Started at: ${payload.startedAt}`)
  console.log(`Expires at: ${payload.expiresAt}`)
  process.exit(0)
}

if (command === 'failover') {
  const previousLeaseRef = valueAfter('--previousLeaseRef')
  const failureClass = valueAfter('--failureClass')
  const requestedAction = valueAfter('--action')

  if (
    previousLeaseRef === undefined ||
    failureClass === undefined ||
    requestedAction === undefined
  ) {
    usage()
    process.exit(2)
  }

  const body = {
    ...targetSelector(),
    failureClass,
    previousLeaseRef,
    requestedAction,
    ...(numberAfter('--attemptNumber') === undefined
      ? {}
      : { attemptNumber: numberAfter('--attemptNumber') }),
    ...(numberAfter('--maxAttempts') === undefined
      ? {}
      : { maxAttempts: numberAfter('--maxAttempts') }),
    ...(valueAfter('--runId') === undefined
      ? {}
      : { runId: valueAfter('--runId') }),
    ...(valueAfter('--assignmentId') === undefined
      ? {}
      : { assignmentId: valueAfter('--assignmentId') }),
    ...(valueAfter('--orderId') === undefined
      ? {}
      : { orderId: valueAfter('--orderId') }),
  }
  const { ok, payload, status } = await requestJson(
    '/api/operator/provider-accounts/chatgpt-codex/leases/failover',
    {
      body: JSON.stringify(body),
      method: 'POST',
    },
  )

  if (hasFlag('--json')) {
    printJsonOrExit(ok, payload)
  }

  if (!ok && payload.outcome !== 'blocked') {
    console.error(`Failover failed: ${payload.error ?? status}`)
    if (payload.reason !== undefined) {
      console.error(payload.reason)
    }
    if (payload.message !== undefined) {
      console.error(payload.message)
    }
    process.exit(1)
  }

  console.log(`Failover outcome: ${payload.outcome}`)
  console.log(`Receipt ID: ${payload.receiptId}`)
  console.log(`Failure class: ${payload.failureClass}`)
  console.log(`Account state action: ${payload.accountStateAction}`)
  console.log(`Previous account ref: ${payload.previousProviderAccountRef}`)
  if (payload.nextLease !== null && payload.nextLease !== undefined) {
    console.log(`Next lease ref: ${payload.nextLease.leaseRef}`)
    console.log(`Next account ref: ${payload.nextLease.providerAccountRef}`)
  }
  console.log(`Customer-safe status: ${payload.customerSafeStatus}`)
  process.exit(payload.outcome === 'retrying' ? 0 : 1)
}

if (command === 'failover-history') {
  const body = {
    ...targetSelector(),
    ...(valueAfter('--runId') === undefined
      ? {}
      : { runId: valueAfter('--runId') }),
    ...(valueAfter('--assignmentId') === undefined
      ? {}
      : { assignmentId: valueAfter('--assignmentId') }),
    ...(valueAfter('--orderId') === undefined
      ? {}
      : { orderId: valueAfter('--orderId') }),
    ...(numberAfter('--limit') === undefined
      ? {}
      : { limit: numberAfter('--limit') }),
  }
  const { ok, payload, status } = await requestJson(
    '/api/operator/provider-accounts/chatgpt-codex/leases/failover-history',
    {
      body: JSON.stringify(body),
      method: 'POST',
    },
  )

  if (hasFlag('--json')) {
    printJsonOrExit(ok, payload)
  }

  if (!ok) {
    console.error(`Failover history failed: ${payload.error ?? status}`)
    process.exit(1)
  }

  console.log(`ChatGPT/Codex failover receipts: ${payload.total ?? 0}`)
  for (const receipt of payload.receipts ?? []) {
    console.log(
      `- ${receipt.createdAt} ${receipt.receiptId} ${receipt.outcome} ${receipt.failureClass} previous=${receipt.previousProviderAccountRef ?? 'none'} next=${receipt.nextProviderAccountRef ?? 'none'} summary="${receipt.customerSafeSummary ?? receipt.customerSafeStatus}"`,
    )
  }
  process.exit(0)
}

if (command === 'leases') {
  const { ok, payload, status } = await requestJson(
    '/api/operator/provider-accounts/chatgpt-codex/leases/active',
    {
      body: JSON.stringify(targetSelector()),
      method: 'POST',
    },
  )

  if (hasFlag('--json')) {
    printJsonOrExit(ok, payload)
  }

  if (!ok) {
    console.error(`Active leases failed: ${payload.error ?? status}`)
    process.exit(1)
  }

  console.log(`Active ChatGPT/Codex leases: ${payload.total ?? 0}`)
  for (const lease of payload.leases ?? []) {
    console.log(
      `- ${lease.leaseRef} ${lease.providerAccountRef} ${lease.requestedAction} expires=${lease.expiresAt}`,
    )
  }
  process.exit(0)
}

if (command === 'explain-lease') {
  const { ok, payload, status } = await requestJson(
    '/api/operator/provider-accounts/chatgpt-codex/leases/explain',
    {
      body: JSON.stringify(targetSelector()),
      method: 'POST',
    },
  )

  if (hasFlag('--json')) {
    printJsonOrExit(ok, payload)
  }

  if (!ok) {
    console.error(`Lease explain failed: ${payload.error ?? status}`)
    process.exit(1)
  }

  console.log(`Lease selector status: ${payload.status}`)
  console.log(`Provider account ref: ${payload.providerAccountRef ?? 'none'}`)
  console.log(`Account label: ${payload.accountLabel ?? 'none'}`)
  console.log(`Policy: ${payload.selectedByPolicyVersion}`)
  console.log(`Reason: ${payload.selectionReason}`)
  process.exit(payload.status === 'selected' ? 0 : 1)
}

if (command === 'dashboard') {
  const { ok, payload, status } = await requestJson(
    '/api/operator/provider-accounts/chatgpt-codex/fleet-dashboard',
    {
      body: JSON.stringify(targetSelector()),
      method: 'POST',
    },
  )

  if (hasFlag('--json')) {
    printJsonOrExit(ok, payload)
  }

  if (!ok) {
    console.error(`Fleet dashboard failed: ${payload.error ?? status}`)
    process.exit(1)
  }

  console.log('ChatGPT/Codex fleet dashboard')
  console.log(
    `Summary: ${payload.summary?.eligible ?? 0}/${payload.summary?.total ?? 0} eligible, ${payload.summary?.activeLeaseCount ?? 0} active leases, ${payload.summary?.lowCredit ?? 0} low-credit, ${payload.summary?.requiresReauth ?? 0} reauth, ${payload.summary?.cooldown ?? 0} cooldown`,
  )
  console.log(
    `Selector: ${payload.selector?.status ?? 'unknown'} ${payload.selector?.providerAccountRef ?? 'none'} (${payload.selector?.selectionReason ?? 'No selector explanation.'})`,
  )

  for (const account of payload.accounts ?? []) {
    const label =
      account.operatorLabel ?? account.accountLabel ?? account.providerAccountRef
    const reasons =
      Array.isArray(account.eligibilityReasons) &&
      account.eligibilityReasons.length > 0
        ? account.eligibilityReasons.join(',')
        : 'eligible'
    console.log(
      `- ${account.providerAccountRef} "${label}" ${account.status}/${account.health} ${account.eligibility} leases=${account.activeLeaseCount}/${account.leaseLimit} priority=${account.operatorPriority} reasons=${reasons}`,
    )
    console.log(
      `  sanity=${account.lastSanityCheckResult ?? 'none'} at ${account.lastSanityCheckAt ?? 'never'} probe=${account.lastParallelProbeResult ?? 'none'} at ${account.lastParallelProbeAt ?? 'never'}`,
    )
    console.log(
      `  selected=${account.lastSelectedAt ?? 'never'} success=${account.lastSuccessfulLaunchAt ?? 'never'} failure=${account.lastFailedLaunchAt ?? 'never'} recentFailure=${account.recentFailureClass ?? 'none'} cooldown=${account.cooldownUntil ?? 'none'}`,
    )
    if (account.lowCredit || account.reauthRequiredReason !== null) {
      console.log(
        `  attention lowCredit=${account.lowCredit ? 'yes' : 'no'} reauth=${account.reauthRequiredReason ?? 'none'} refill=${account.refillNote ?? 'none'}`,
      )
    }
    if (account.operatorNote !== null && account.operatorNote !== undefined) {
      console.log(`  note=${account.operatorNote}`)
    }
    console.log(`  sanity command: ${account.sanityCommand}`)
    console.log(`  reconnect command: ${account.reconnectCommand}`)
  }

  if (Array.isArray(payload.activeLeases) && payload.activeLeases.length > 0) {
    console.log('Active leases:')
    for (const lease of payload.activeLeases) {
      console.log(
        `- ${lease.leaseRef} ${lease.providerAccountRef} ${lease.requestedAction} assignment=${lease.assignmentId ?? 'none'} expires=${lease.expiresAt}`,
      )
    }
  }

  process.exit(payload.summary?.eligible > 0 ? 0 : 1)
}

if (command === 'touch-lease' || command === 'release-lease') {
  const leaseRef = valueAfter('--leaseRef')

  if (leaseRef === undefined) {
    usage()
    process.exit(2)
  }

  const isRelease = command === 'release-lease'
  const body = {
    leaseRef,
    ...(isRelease
      ? {
          failureClass: valueAfter('--failureClass'),
          status: valueAfter('--status'),
          terminalOutcome: valueAfter('--terminalOutcome'),
        }
      : { ttlSeconds: numberAfter('--ttlSeconds') }),
  }
  const { ok, payload, status } = await requestJson(
    `/api/operator/provider-accounts/chatgpt-codex/leases/${isRelease ? 'release' : 'touch'}`,
    {
      body: JSON.stringify(body),
      method: 'POST',
    },
  )

  if (hasFlag('--json')) {
    printJsonOrExit(ok, payload)
  }

  if (!ok) {
    console.error(
      `Lease ${isRelease ? 'release' : 'touch'} failed: ${payload.error ?? status}`,
    )
    process.exit(1)
  }

  console.log(
    `Lease ${isRelease ? 'released' : 'touched'}: ${payload.leaseRef}`,
  )
  process.exit(0)
}

usage()
process.exit(2)
