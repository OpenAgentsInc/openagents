#!/usr/bin/env node

export const parseArgs = argv => {
  const options = {
    baseUrl: process.env.OPENAGENTS_BASE_URL || 'https://openagents.com',
    register: false,
    title: process.env.OPENAGENTS_FORUM_TIP_WALLET_SMOKE_TITLE || '',
  }

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]

    if (value === '--register') {
      options.register = true
    } else if (value === '--base-url' || value === '--baseUrl') {
      options.baseUrl = argv[++index] || options.baseUrl
    } else if (value === '--title') {
      options.title = argv[++index] || options.title
    } else if (value === '--help' || value === '-h') {
      options.help = true
    } else {
      throw new Error(`Unknown argument: ${value}`)
    }
  }

  return options
}

export const usage = () => `Usage:
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum-tip-wallet-smoke.mjs
  node scripts/forum-tip-wallet-smoke.mjs --register

Options:
  --base-url <url>  OpenAgents origin. Defaults to https://openagents.com.
  --register       Self-register a temporary smoke agent when no token is provided.
  --title <title>  Override the generated unlisted verification topic title.
`

const tokenPattern = /oa_agent_[A-Za-z0-9._~+/-]+/g
const forbiddenOutputPattern =
  /(mnemonic|payment_hash|paymentHash|preimage|lnbc|lntb|lno1|OPENAGENTS_AGENT_TOKEN|Bearer\s+[A-Za-z0-9._~+/-]+)/i

export const redact = text =>
  String(text)
    .replace(/Bearer\s+[A-Za-z0-9._~+/-]+/g, 'Bearer <redacted>')
    .replace(tokenPattern, 'oa_agent_<redacted>')

export const jsonRequest = async (baseUrl, path, init = {}) => {
  const response = await fetch(new URL(path, baseUrl), {
    ...init,
    headers: {
      accept: 'application/json',
      ...(init.body === undefined
        ? {}
        : { 'content-type': 'application/json' }),
      ...(init.headers || {}),
    },
  })
  const body = await response.json().catch(() => ({}))

  if (!response.ok) {
    const reason = body.reason || body.error || response.statusText
    throw new Error(
      `${init.method || 'GET'} ${path} failed: ${response.status} ${redact(
        reason,
      )}`,
    )
  }

  return body
}

const uniqueKey = prefix =>
  `${prefix}-${new Date().toISOString().replace(/[^0-9]/g, '')}-${crypto.randomUUID()}`

const authHeaders = token => ({
  authorization: `Bearer ${token}`,
})

const assert = (condition, message) => {
  if (!condition) {
    throw new Error(message)
  }
}

export const registerAgent = async baseUrl => {
  const suffix = crypto.randomUUID().slice(0, 8)
  const registration = await jsonRequest(baseUrl, '/api/agents/register', {
    body: JSON.stringify({
      displayName: 'Forum Tip Wallet Smoke Agent',
      externalId: `forum-tip-wallet-smoke-${suffix}`,
      metadata: {
        purpose: 'forum_tip_wallet_smoke',
        publicSafe: true,
      },
      slug: `forum-tip-wallet-smoke-${suffix}`,
    }),
    method: 'POST',
  })
  const token = registration?.credential?.token

  if (typeof token !== 'string' || !token.startsWith('oa_agent_')) {
    throw new Error('Registration did not return a usable agent token.')
  }

  return token
}

export const buildWalletClaimBody = suffix => ({
  caveatRefs: [
    `caveat.public.forum_tip_recipient.${suffix}.settlement_pending`,
  ],
  claimPolicyRefs: [
    `policy.public.forum_tip_recipient.${suffix}.agent_self_claimed`,
  ],
  custodyPolicyRefs: [
    `policy.public.forum_tip_recipient.${suffix}.self_custody`,
  ],
  payoutTargetApprovalRef: null,
  providerClass: 'mdk_agent_wallet',
  readinessRefs: [
    `readiness.public.forum_tip_recipient.${suffix}.mdk_daemon_available`,
    `readiness.public.forum_tip_recipient.${suffix}.setup_present`,
    `readiness.public.forum_tip_recipient.${suffix}.receive_ready`,
  ],
  receiveCapabilityRef: `receive_capability.public.forum_tip_recipient.${suffix}.redacted`,
  sourceRef: `source.public.forum_tip_recipient.${suffix}.agent_self_claim`,
  walletRef: `wallet.public.forum_tip_recipient.${suffix}.redacted`,
})

const assertPublicSmokeOutput = output => {
  const text = JSON.stringify(output)

  if (forbiddenOutputPattern.test(text)) {
    throw new Error('Smoke output contains private payment or token material.')
  }
}

export const runForumTipWalletSmoke = async ({
  baseUrl,
  register,
  title,
  token,
}) => {
  const agentToken = token || (register ? await registerAgent(baseUrl) : null)

  if (!agentToken) {
    throw new Error(
      'Missing OPENAGENTS_AGENT_TOKEN. Pass --register to self-register a temporary smoke agent.',
    )
  }

  const agent = await jsonRequest(baseUrl, '/api/agents/me', {
    headers: authHeaders(agentToken),
  })
  assert(agent?.authenticated === true, 'Agent auth sanity check failed.')

  const suffix = `smoke_${crypto.randomUUID().replace(/-/g, '').slice(0, 12)}`
  const claim = await jsonRequest(
    baseUrl,
    '/api/forum/tip-recipient-wallets/claims',
    {
      body: JSON.stringify(buildWalletClaimBody(suffix)),
      headers: {
        ...authHeaders(agentToken),
        'idempotency-key': uniqueKey('forum-tip-wallet-claim'),
      },
      method: 'POST',
    },
  )
  const readiness = claim?.tipRecipientReadiness
  assert(
    readiness?.state === 'ready',
    'Tip recipient readiness did not reach ready state.',
  )
  assert(
    readiness?.tippingAvailable === true,
    'Tip recipient projection is not tip-ready.',
  )

  const topicTitle =
    title || `Forum tip wallet smoke ${new Date().toISOString()}`
  const topic = await jsonRequest(baseUrl, '/api/forum/forums/void/topics', {
    body: JSON.stringify({
      bodyText: `Public-safe tip wallet smoke verification for ${topicTitle}.`,
      requestedSlug: `forum-tip-wallet-smoke-${crypto.randomUUID().slice(0, 8)}`,
      title: topicTitle,
    }),
    headers: {
      ...authHeaders(agentToken),
      'idempotency-key': uniqueKey('forum-tip-wallet-topic'),
    },
    method: 'POST',
  })
  const topicId = topic?.topic?.topicId
  const firstPostId = topic?.firstPost?.postId

  assert(typeof topicId === 'string', 'Topic creation did not return topicId.')
  assert(
    typeof firstPostId === 'string',
    'Topic creation did not return first post id.',
  )
  assert(
    topic?.firstPost?.tipRecipientReadiness?.tippingAvailable === true,
    'Created post did not project tip recipient readiness.',
  )

  const detail = await jsonRequest(baseUrl, `/api/forum/topics/${topicId}`)
  const firstPost = Array.isArray(detail.posts)
    ? detail.posts.find(post => post.postId === firstPostId)
    : null
  assert(
    firstPost?.tipRecipientReadiness?.tippingAvailable === true,
    'Readable topic detail did not preserve tip recipient readiness.',
  )

  const output = {
    actorRef: readiness.actorRef,
    baseUrl,
    blockerRef: readiness.blockerRef ?? null,
    caveatRefs: readiness.caveatRefs ?? [],
    providerClass: readiness.providerClass,
    readinessRefs: readiness.readinessRefs ?? [],
    state: readiness.state,
    tippingAvailable: readiness.tippingAvailable === true,
    topicId,
    topicUrl: `${baseUrl}/forum/t/${topicId}`,
  }

  assertPublicSmokeOutput(output)

  return output
}

const main = async () => {
  const options = parseArgs(process.argv.slice(2))

  if (options.help) {
    console.log(usage())
    return
  }

  const output = await runForumTipWalletSmoke({
    baseUrl: options.baseUrl.replace(/\/+$/, ''),
    register: options.register,
    title: options.title,
    token: process.env.OPENAGENTS_AGENT_TOKEN,
  })

  console.log(JSON.stringify(output, null, 2))
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(error => {
    console.error(redact(error instanceof Error ? error.message : String(error)))
    process.exitCode = 1
  })
}
