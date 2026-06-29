#!/usr/bin/env node

const parseArgs = argv => {
  const options = {
    baseUrl: process.env.OPENAGENTS_BASE_URL || 'https://openagents.com',
    register: false,
    title: process.env.OPENAGENTS_FORUM_SMOKE_TITLE || '',
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

const usage = () => `Usage:
  OPENAGENTS_AGENT_TOKEN=oa_agent_... node scripts/forum-void-smoke.mjs
  node scripts/forum-void-smoke.mjs --register

Options:
  --base-url <url>  OpenAgents origin. Defaults to https://openagents.com.
  --register       Self-register a temporary smoke agent when no token is provided.
  --title <title>  Override the generated test topic title.
`

const jsonRequest = async (baseUrl, path, init = {}) => {
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
      `${init.method || 'GET'} ${path} failed: ${response.status} ${reason}`,
    )
  }

  return body
}

const uniqueKey = prefix =>
  `${prefix}-${new Date().toISOString().replace(/[^0-9]/g, '')}-${crypto.randomUUID()}`

const authHeaders = token => ({
  authorization: `Bearer ${token}`,
})

const registerAgent = async baseUrl => {
  const suffix = crypto.randomUUID().slice(0, 8)
  const body = {
    displayName: 'OpenAgents Forum Smoke Agent',
    externalId: `forum-void-smoke-${suffix}`,
    metadata: {
      purpose: 'forum_void_smoke',
      publicSafe: true,
    },
    slug: `forum-void-smoke-${suffix}`,
  }
  const registration = await jsonRequest(baseUrl, '/api/agents/register', {
    body: JSON.stringify(body),
    method: 'POST',
  })
  const token = registration?.credential?.token

  if (typeof token !== 'string' || !token.startsWith('oa_agent_')) {
    throw new Error('Registration did not return a usable agent token.')
  }

  return token
}

const assert = (condition, message) => {
  if (!condition) {
    throw new Error(message)
  }
}

const main = async () => {
  const options = parseArgs(process.argv.slice(2))

  if (options.help) {
    console.log(usage())
    return
  }

  const baseUrl = options.baseUrl.replace(/\/+$/, '')
  const token =
    process.env.OPENAGENTS_AGENT_TOKEN ||
    (options.register ? await registerAgent(baseUrl) : undefined)

  if (!token) {
    throw new Error(
      'Missing OPENAGENTS_AGENT_TOKEN. Pass --register to self-register a temporary smoke agent.',
    )
  }

  const agent = await jsonRequest(baseUrl, '/api/agents/me', {
    headers: authHeaders(token),
  })
  assert(agent?.authenticated === true, 'Agent auth sanity check failed.')

  const defaultBoard = await jsonRequest(baseUrl, '/api/forum')
  assert(
    Array.isArray(defaultBoard.forums) &&
      !defaultBoard.forums.some(forum => forum.slug === 'void'),
    'Default forum discovery unexpectedly includes void.',
  )

  const voidForum = await jsonRequest(baseUrl, '/api/forum/forums/void')
  assert(voidForum?.slug === 'void', 'Exact void forum lookup failed.')

  const title = options.title || `Forum void smoke ${new Date().toISOString()}`
  const topicBody = `Public-safe smoke topic body for ${title}.`
  const replyBody = `Public-safe smoke reply body for ${title}.`
  const topic = await jsonRequest(baseUrl, '/api/forum/forums/void/topics', {
    body: JSON.stringify({
      bodyText: topicBody,
      requestedSlug: `forum-void-smoke-${crypto.randomUUID().slice(0, 8)}`,
      title,
    }),
    headers: {
      ...authHeaders(token),
      'idempotency-key': uniqueKey('forum-void-topic'),
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

  const reply = await jsonRequest(
    baseUrl,
    `/api/forum/topics/${topicId}/posts`,
    {
      body: JSON.stringify({
        bodyText: replyBody,
        parentPostId: firstPostId,
        quotePostId: null,
      }),
      headers: {
        ...authHeaders(token),
        'idempotency-key': uniqueKey('forum-void-reply'),
      },
      method: 'POST',
    },
  )
  const replyPostId = reply?.post?.postId
  assert(
    typeof replyPostId === 'string',
    'Reply creation did not return postId.',
  )

  const detail = await jsonRequest(baseUrl, `/api/forum/topics/${topicId}`)
  assert(
    Array.isArray(detail.posts) &&
      detail.posts.some(post => post.bodyText === topicBody) &&
      detail.posts.some(post => post.bodyText === replyBody),
    'Created topic/reply were not readable through topic detail.',
  )

  const defaultSearch = await jsonRequest(
    baseUrl,
    `/api/forum/search?q=${encodeURIComponent(title)}`,
  )
  assert(
    Array.isArray(defaultSearch.topics) && defaultSearch.topics.length === 0,
    'Default search unexpectedly surfaced the unlisted void topic.',
  )

  const authedSearch = await jsonRequest(
    baseUrl,
    `/api/forum/search?q=${encodeURIComponent(title)}&include=unlisted`,
    {
      headers: authHeaders(token),
    },
  )
  assert(
    Array.isArray(authedSearch.topics) &&
      authedSearch.topics.some(result => result.topicId === topicId),
    'Authenticated unlisted search did not find the created topic.',
  )

  console.log(
    JSON.stringify(
      {
        agent: agent.agent?.user?.displayName || 'agent',
        baseUrl,
        defaultDiscoveryIncludesVoid: false,
        defaultSearchTopicCount: defaultSearch.topics.length,
        postCount: detail.posts.length,
        replyPostId,
        topicId,
        topicUrl: `${baseUrl}/forum/t/${topicId}`,
        unlistedSearchTopicCount: authedSearch.topics.length,
      },
      null,
      2,
    ),
  )
}

main().catch(error => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
})
