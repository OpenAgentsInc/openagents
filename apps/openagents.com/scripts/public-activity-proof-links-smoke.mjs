#!/usr/bin/env node

const defaultBaseUrl = 'https://openagents.com'
const defaultTimelinePath = '/api/public/activity-timeline?limit=100'

export const parseArgs = argv => {
  const options = {
    baseUrl: process.env.OPENAGENTS_BASE_URL || defaultBaseUrl,
    limit: 8,
    timelinePath: defaultTimelinePath,
  }

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]
    if (value === '--base-url' || value === '--baseUrl') {
      options.baseUrl = argv[++index] || options.baseUrl
    } else if (value === '--limit') {
      options.limit = Number(argv[++index] || options.limit)
    } else if (value === '--timeline-path') {
      options.timelinePath = argv[++index] || options.timelinePath
    } else if (value === '--help' || value === '-h') {
      options.help = true
    } else {
      throw new Error(`Unknown argument: ${value}`)
    }
  }

  if (!Number.isFinite(options.limit) || options.limit < 0) {
    throw new Error('--limit must be a non-negative number.')
  }

  return options
}

export const usage = () => `Usage:
  node scripts/public-activity-proof-links-smoke.mjs
  node scripts/public-activity-proof-links-smoke.mjs --base-url http://localhost:5173

Options:
  --base-url <url>       OpenAgents origin. Defaults to https://openagents.com.
  --limit <count>        Max proof URLs to fetch. Defaults to 8.
  --timeline-path <url>  Timeline path. Defaults to ${defaultTimelinePath}.
`

const assert = (condition, message) => {
  if (!condition) throw new Error(message)
}

const trimBaseUrl = baseUrl =>
  String(baseUrl || defaultBaseUrl).replace(/\/+$/, '')

const absoluteUrl = (baseUrl, pathOrUrl) => new URL(pathOrUrl, baseUrl)

const okStatus = response => response.status >= 200 && response.status < 300

const requestJson = async (fetchImpl, baseUrl, path) => {
  const url = absoluteUrl(baseUrl, path)
  const response = await fetchImpl(url.toString(), {
    headers: { accept: 'application/json' },
  })
  const body = await response.json().catch(() => null)
  return { body, response, url: url.toString() }
}

export const publicActivityProofUrlForRef = (ref, event = {}) => {
  const value = String(ref || '').trim()
  if (value.length === 0 || value.includes('{') || value.includes('}')) {
    return null
  }
  if (value.startsWith('route:')) return value.slice('route:'.length)
  if (value.startsWith('/') || value.startsWith('https://')) return value
  if (value.startsWith('receipt.forum.')) {
    return `/api/forum/receipts/${encodeURIComponent(value)}`
  }
  if (
    value.startsWith('receipt.nexus.') ||
    value.startsWith('receipt.nexus_') ||
    value.startsWith('receipt.nexus-pylon.') ||
    value.startsWith('receipt.public.')
  ) {
    return `/api/public/nexus-pylon/receipts/${encodeURIComponent(value)}`
  }
  if (value.startsWith('training.verification.challenge.')) {
    return `/api/public/training/verification-challenges/${encodeURIComponent(value)}`
  }
  if (
    value.startsWith('training.window.') ||
    value.startsWith('trace.public.')
  ) {
    const runRef = typeof event.runRef === 'string' ? event.runRef.trim() : ''
    return runRef.length === 0
      ? null
      : `/api/public/training/runs/${encodeURIComponent(runRef)}?focusRef=${encodeURIComponent(value)}`
  }
  if (value.startsWith('run.')) {
    return `/api/public/training/runs/${encodeURIComponent(value)}`
  }
  if (value.startsWith('pylon.') || value.startsWith('pylon_')) {
    return '/api/public/pylon-stats'
  }
  if (value.startsWith('forum.')) return '/forum'
  if (value.startsWith('artanis.')) return '/api/public/artanis/admin-ticks'
  if (value.includes('capacity'))
    return '/api/public/pylon-capacity-funnel/history'
  if (value.includes('product-promises') || value.includes('product_promise')) {
    return '/api/public/product-promises'
  }
  return null
}

const unique = values => [...new Set(values.filter(Boolean))]

const proofUrlsFor = (event, lag) =>
  unique(
    [
      ...(Array.isArray(event?.sourceRefs) ? event.sourceRefs : []),
      ...(Array.isArray(event?.refs) ? event.refs : []),
      ...(Array.isArray(event?.blockerRefs) ? event.blockerRefs : []),
      ...(Array.isArray(event?.caveatRefs) ? event.caveatRefs : []),
      ...(Array.isArray(lag?.sourceRefs) ? lag.sourceRefs : []),
      ...(Array.isArray(lag?.blockerRefs) ? lag.blockerRefs : []),
      ...(Array.isArray(lag?.caveatRefs) ? lag.caveatRefs : []),
    ].map(ref => publicActivityProofUrlForRef(ref, event)),
  )

const sameOriginUrl = (baseUrl, url) => {
  const origin = new URL(baseUrl)
  const candidate = absoluteUrl(baseUrl, url)
  return candidate.origin === origin.origin ? candidate.toString() : null
}

export const runPublicActivityProofLinksSmoke = async ({
  baseUrl = defaultBaseUrl,
  fetchImpl = globalThis.fetch,
  limit = 8,
  timelinePath = defaultTimelinePath,
} = {}) => {
  assert(typeof fetchImpl === 'function', 'A fetch implementation is required.')
  const origin = trimBaseUrl(baseUrl)
  const checks = []
  const timelineResult = await requestJson(fetchImpl, origin, timelinePath)
  assert(
    okStatus(timelineResult.response),
    'activity_timeline_endpoint_200 failed',
  )

  const timeline = timelineResult.body
  assert(
    timeline?.schemaVersion === 'openagents.public_activity_timeline.v1',
    'activity_timeline_schema_version failed',
  )

  const events = Array.isArray(timeline?.events) ? timeline.events : []
  const sourceLag = Array.isArray(timeline?.sourceLag) ? timeline.sourceLag : []
  const lagBySource = new Map(sourceLag.map(lag => [lag.sourceKind, lag]))
  const urls = unique(
    events.flatMap(event =>
      proofUrlsFor(event, lagBySource.get(event.sourceKind))
        .map(url => sameOriginUrl(origin, url))
        .filter(Boolean),
    ),
  ).slice(0, limit)

  checks.push({
    details: { count: urls.length, timeline: timelineResult.url },
    name: 'proof_urls_discovered',
    passed: true,
  })

  const linked = await Promise.all(
    urls.map(async url => {
      const response = await fetchImpl(url, {
        headers: { accept: 'application/json' },
      })
      const passed = okStatus(response)
      checks.push({
        details: { status: response.status, url },
        name: 'proof_url_200',
        passed,
      })
      assert(passed, `proof_url_200 failed for ${url}`)
      return { status: response.status, url }
    }),
  )

  return {
    baseUrl: origin,
    checks,
    linked,
    ok: checks.every(check => check.passed),
    timeline: {
      eventCount: events.length,
      proofUrlCount: urls.length,
      sourceLagCount: sourceLag.length,
    },
  }
}

const main = async () => {
  const options = parseArgs(process.argv.slice(2))
  if (options.help) {
    console.log(usage())
    return
  }

  const output = await runPublicActivityProofLinksSmoke(options)
  console.log(JSON.stringify(output, null, 2))
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(error => {
    console.error(error instanceof Error ? error.message : String(error))
    process.exitCode = 1
  })
}
