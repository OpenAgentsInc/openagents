#!/usr/bin/env node

// Owned-infra visibility freshness smoke (issue #5435).
//
// This is a Node script for local/owned schedulers. It must not be wired into
// GitHub Actions as the primary monitor. The check reads public projections,
// reports stale source-lag rows and broken routes with specific refs, and exits
// nonzero unless --warn-only is used for manual evidence collection.
import { readFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const scriptPath = fileURLToPath(import.meta.url)
const repoRoot = resolve(dirname(scriptPath), '../../..')

const defaultBaseUrl = 'https://openagents.com'
const defaultTimelinePath = '/api/public/activity-timeline?limit=50'
const defaultSsePath = '/api/public/activity-timeline/stream?limit=1'
const defaultReplayClipPath = '/api/public/replay-clips'
const defaultGeneratedFrom = '2026-06-18T00:00:00.000Z'
const defaultGeneratedTo = '2026-06-19T00:00:00.000Z'
const publicRouteChecks = [
  {
    accept: 'application/json',
    name: 'activity_timeline_route',
    path: '/api/public/activity-timeline?limit=5',
  },
  {
    accept: 'application/json',
    name: 'generated_proof_replay_route',
    path: ({ generatedFrom, generatedLimit, generatedTo }) =>
      `/api/public/proof-replays?mode=activity-timeline&from=${encodeURIComponent(generatedFrom)}&to=${encodeURIComponent(generatedTo)}&limit=${encodeURIComponent(String(generatedLimit))}`,
  },
  {
    accept: 'application/json',
    name: 'pylon_stats_route',
    path: '/api/public/pylon-stats',
  },
  {
    accept: 'application/json',
    name: 'tassadar_run_summary_route',
    path: '/api/public/tassadar-run-summary',
  },
  {
    accept: 'application/json',
    name: 'first_settlement_replay_route',
    path: '/api/public/tassadar-replays/first-real-settlement',
  },
  {
    accept: 'application/json',
    name: 'render_queue_route',
    path: defaultReplayClipPath,
  },
]

const usage = () => `Usage:
  node apps/openagents.com/scripts/visibility-freshness-smoke.mjs [options]

Options:
  --base-url <url>                       Public origin. Default: ${defaultBaseUrl}
  --timeline-path <path>                 Timeline route. Default: ${defaultTimelinePath}
  --sse-path <path>                      SSE route. Default: ${defaultSsePath}
  --replay-clip-path <path>              Render queue route. Default: ${defaultReplayClipPath}
  --generated-from <iso>                 Generated replay lower bound. Default: ${defaultGeneratedFrom}
  --generated-to <iso>                   Generated replay upper bound. Default: ${defaultGeneratedTo}
  --generated-limit <n>                  Generated replay route limit. Default: 5
  --max-generated-age-seconds <n>        Live projection response freshness. Default: 120
  --max-render-queue-age-seconds <n>     Queued/rendering job age bound. Default: 3600
  --timeout-ms <n>                       Per-request timeout. Default: 15000
  --r2-manifest-url <url>                Explicit replay clip manifest to probe.
  --source-lag-mode <fail|warn|ignore>   Default: fail
  --warn-only                            Print failures but exit 0.
  --help                                 Show this message.

This check is for owned local/CI/Container schedulers, not GitHub Actions.
`

const numericArgNames = new Set([
  'generatedLimit',
  'maxGeneratedAgeSeconds',
  'maxRenderQueueAgeSeconds',
  'timeoutMs',
])

export const parseArgs = argv => {
  const options = {
    baseUrl: process.env.OPENAGENTS_BASE_URL || defaultBaseUrl,
    generatedFrom: defaultGeneratedFrom,
    generatedLimit: 5,
    generatedTo: defaultGeneratedTo,
    maxGeneratedAgeSeconds: 120,
    maxRenderQueueAgeSeconds: 3600,
    replayClipPath: defaultReplayClipPath,
    r2ManifestUrls: [],
    sourceLagMode: 'fail',
    ssePath: defaultSsePath,
    timeoutMs: 15_000,
    timelinePath: defaultTimelinePath,
    warnOnly: false,
  }

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]
    if (value === '--help' || value === '-h') {
      options.help = true
      continue
    }
    if (value === '--warn-only') {
      options.warnOnly = true
      continue
    }
    if (!value.startsWith('--')) {
      throw new Error(`Unknown positional argument: ${value}`)
    }

    const key = value
      .slice(2)
      .replace(/-([a-z])/g, (_, char) => char.toUpperCase())
    const next = argv[index + 1]
    if (next === undefined || next.startsWith('--')) {
      throw new Error(`Missing value for ${value}`)
    }
    index += 1

    if (key === 'r2ManifestUrl') {
      options.r2ManifestUrls.push(next)
    } else {
      options[key] = numericArgNames.has(key) ? Number(next) : next
    }
  }

  if (!['fail', 'warn', 'ignore'].includes(options.sourceLagMode)) {
    throw new Error('--source-lag-mode must be fail, warn, or ignore')
  }
  for (const key of numericArgNames) {
    if (!Number.isFinite(options[key]) || options[key] < 0) {
      throw new Error(`--${key.replace(/[A-Z]/g, c => `-${c.toLowerCase()}`)} must be a non-negative number`)
    }
  }

  return options
}

const trimBaseUrl = baseUrl => String(baseUrl || defaultBaseUrl).replace(/\/+$/, '')

export const absoluteUrl = (baseUrl, pathOrUrl) =>
  new URL(pathOrUrl, `${trimBaseUrl(baseUrl)}/`).toString()

const passed = (name, details = {}) => ({
  details,
  name,
  passed: true,
  severity: 'info',
})

const failed = (name, details = {}, severity = 'error') => ({
  details,
  name,
  passed: false,
  severity,
})

const warning = (name, details = {}) => failed(name, details, 'warning')

const okStatus = response => response.status >= 200 && response.status < 300

const responseContentType = response =>
  response.headers?.get?.('content-type') ?? ''

const fetchWithTimeout = async (fetchImpl, url, init, timeoutMs) => {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), timeoutMs)
  try {
    return await fetchImpl(url, { ...init, signal: controller.signal })
  } finally {
    clearTimeout(timer)
  }
}

const requestText = async ({
  accept,
  baseUrl,
  fetchImpl,
  path,
  timeoutMs,
}) => {
  const url = absoluteUrl(baseUrl, path)
  const response = await fetchWithTimeout(
    fetchImpl,
    url,
    { headers: { accept } },
    timeoutMs,
  )
  const body = await response.text().catch(() => '')
  return { body, response, url }
}

const requestJson = async input => {
  const result = await requestText({ ...input, accept: 'application/json' })
  return {
    ...result,
    body: JSON.parse(result.body || 'null'),
  }
}

const isoAgeSeconds = (iso, now = new Date()) => {
  const ms = Date.parse(String(iso))
  if (!Number.isFinite(ms)) return null
  return Math.max(0, Math.floor((now.getTime() - ms) / 1000))
}

const array = value => (Array.isArray(value) ? value : [])

const sourceRefsFrom = value => [
  ...array(value?.sourceRefs),
  ...array(value?.blockerRefs),
  ...array(value?.caveatRefs),
]

export const checkTimelineFreshness = (
  timeline,
  {
    maxGeneratedAgeSeconds = 120,
    now = new Date(),
    url = 'timeline',
  } = {},
) => {
  const checks = []
  checks.push(
    timeline?.schemaVersion === 'openagents.public_activity_timeline.v1'
      ? passed('timeline_schema_version', {
          schemaVersion: timeline?.schemaVersion,
          url,
        })
      : failed('timeline_schema_version', {
          schemaVersion: timeline?.schemaVersion ?? null,
          url,
        }),
  )

  const generatedAgeSeconds = isoAgeSeconds(timeline?.generatedAt, now)
  checks.push(
    generatedAgeSeconds !== null &&
      generatedAgeSeconds <= maxGeneratedAgeSeconds
      ? passed('timeline_generated_at_fresh', {
          generatedAgeSeconds,
          generatedAt: timeline?.generatedAt,
          maxGeneratedAgeSeconds,
          url,
        })
      : failed('timeline_generated_at_fresh', {
          generatedAgeSeconds,
          generatedAt: timeline?.generatedAt ?? null,
          maxGeneratedAgeSeconds,
          url,
        }),
  )

  const staleness = timeline?.staleness
  checks.push(
    staleness?.contractVersion === 'projection_staleness.v1' &&
      typeof staleness?.composition === 'string' &&
      typeof staleness?.maxStalenessSeconds === 'number'
      ? passed('timeline_staleness_contract', { staleness, url })
      : failed('timeline_staleness_contract', {
          staleness: staleness ?? null,
          url,
        }),
  )

  checks.push(
    Array.isArray(timeline?.events)
      ? passed('timeline_events_array', {
          eventCount: timeline.events.length,
          url,
        })
      : failed('timeline_events_array', { url }),
  )

  checks.push(
    Array.isArray(timeline?.sourceLag)
      ? passed('timeline_source_lag_array', {
          sourceLagCount: timeline.sourceLag.length,
          url,
        })
      : failed('timeline_source_lag_array', { url }),
  )

  return checks
}

export const checkSourceLag = (
  timeline,
  { mode = 'fail', url = 'timeline' } = {},
) => {
  if (mode === 'ignore') {
    return [passed('source_lag_ignored', { url })]
  }

  const severity = mode === 'warn' ? 'warning' : 'error'
  const rows = array(timeline?.sourceLag)
  if (rows.length === 0) {
    return [
      failed(
        'source_lag_present',
        { reason: 'timeline did not include sourceLag rows', url },
        severity,
      ),
    ]
  }

  return rows.map(row => {
    const details = {
      blockerRefs: array(row?.blockerRefs),
      caveatRefs: array(row?.caveatRefs),
      lagSeconds: row?.lagSeconds ?? null,
      latestSourceEventAt: row?.latestSourceEventAt ?? null,
      maxStalenessSeconds: row?.maxStalenessSeconds ?? null,
      observedAt: row?.observedAt ?? null,
      sourceKind: row?.sourceKind ?? 'unknown',
      sourceRefs: array(row?.sourceRefs),
      status: row?.status ?? null,
      url,
    }
    return row?.status === 'current'
      ? passed(`source_lag_current:${details.sourceKind}`, details)
      : failed(`source_lag_stale:${details.sourceKind}`, details, severity)
  })
}

export const checkSseHealth = async ({
  baseUrl,
  fetchImpl,
  ssePath,
  timeoutMs,
}) => {
  try {
    const result = await requestText({
      accept: 'text/event-stream',
      baseUrl,
      fetchImpl,
      path: ssePath,
      timeoutMs,
    })
    const type = responseContentType(result.response)
    const details = {
      bodyPreview: result.body.slice(0, 240),
      contentType: type,
      status: result.response.status,
      url: result.url,
    }
    if (!okStatus(result.response)) {
      return [failed('sse_route_200', details)]
    }
    if (!/text\/event-stream/i.test(type)) {
      return [failed('sse_content_type', details)]
    }
    if (!/event:\s*activity_timeline_meta/.test(result.body)) {
      return [failed('sse_meta_frame_present', details)]
    }
    return [
      passed('sse_route_200', details),
      passed('sse_content_type', details),
      passed('sse_meta_frame_present', details),
    ]
  } catch (error) {
    return [
      failed('sse_fetch_error', {
        message: error instanceof Error ? error.message : String(error),
        path: ssePath,
      }),
    ]
  }
}

export const checkPublicRoutes = async ({
  baseUrl,
  fetchImpl,
  generatedFrom,
  generatedLimit,
  generatedTo,
  replayClipPath,
  timeoutMs,
}) => {
  const checks = []
  for (const route of publicRouteChecks) {
    const path =
      route.name === 'render_queue_route'
        ? replayClipPath
        : typeof route.path === 'function'
          ? route.path({ generatedFrom, generatedLimit, generatedTo })
          : route.path
    try {
      const result = await requestText({
        accept: route.accept,
        baseUrl,
        fetchImpl,
        path,
        timeoutMs,
      })
      const details = {
        contentType: responseContentType(result.response),
        status: result.response.status,
        url: result.url,
      }
      checks.push(
        okStatus(result.response)
          ? passed(route.name, details)
          : failed(route.name, {
              ...details,
              bodyPreview: result.body.slice(0, 160),
            }),
      )
    } catch (error) {
      checks.push(
        failed(route.name, {
          message: error instanceof Error ? error.message : String(error),
          path,
        }),
      )
    }
  }
  return checks
}

const jobAgeSeconds = (job, now) =>
  isoAgeSeconds(job?.updatedAt ?? job?.createdAt, now)

export const checkRenderQueueHealth = (body, {
  maxRenderQueueAgeSeconds = 3600,
  now = new Date(),
  url = 'render-queue',
} = {}) => {
  const checks = []
  const staleness = body?.staleness
  checks.push(
    staleness?.contractVersion === 'projection_staleness.v1'
      ? passed('render_queue_staleness_contract', { staleness, url })
      : failed('render_queue_staleness_contract', {
          staleness: staleness ?? null,
          url,
        }),
  )

  const jobs = array(body?.jobs)
  checks.push(passed('render_queue_jobs_listed', { jobCount: jobs.length, url }))
  for (const job of jobs) {
    const age = jobAgeSeconds(job, now)
    const active = job?.status === 'queued' || job?.status === 'rendering'
    const details = {
      ageSeconds: age,
      jobRef: job?.jobRef ?? null,
      status: job?.status ?? null,
      updatedAt: job?.updatedAt ?? null,
      url,
    }
    checks.push(
      active && (age === null || age > maxRenderQueueAgeSeconds)
        ? failed(`render_queue_job_stale:${job?.jobRef ?? 'unknown'}`, {
            ...details,
            maxRenderQueueAgeSeconds,
          })
        : passed(`render_queue_job_current:${job?.jobRef ?? 'none'}`, details),
    )
  }
  return checks
}

const manifestRefsFromRenderQueue = body =>
  array(body?.jobs)
    .filter(job => job?.status === 'succeeded')
    .map(job => job?.manifestRef)
    .filter(ref => typeof ref === 'string' && /^https:\/\//i.test(ref))

const fetchClipArtifact = async ({ fetchImpl, timeoutMs, url }) => {
  const head = await fetchWithTimeout(
    fetchImpl,
    url,
    { method: 'HEAD' },
    timeoutMs,
  ).catch(() => null)
  if (head !== null && okStatus(head)) {
    return { method: 'HEAD', response: head }
  }
  const get = await fetchWithTimeout(
    fetchImpl,
    url,
    { headers: { range: 'bytes=0-0' } },
    timeoutMs,
  )
  return { method: 'GET', response: get }
}

export const checkR2ClipAvailability = async ({
  fetchImpl,
  manifestUrls,
  renderQueueBody,
  timeoutMs,
}) => {
  const refs = [
    ...manifestUrls,
    ...manifestRefsFromRenderQueue(renderQueueBody),
  ]
  if (refs.length === 0) {
    return [
      failed('r2_clip_manifest_available', {
        reason:
          'no explicit --r2-manifest-url and no succeeded replay clip jobs with manifestRef',
      }),
    ]
  }

  const checks = []
  for (const manifestUrl of refs) {
    try {
      const manifestResponse = await fetchWithTimeout(
        fetchImpl,
        manifestUrl,
        { headers: { accept: 'application/json' } },
        timeoutMs,
      )
      const manifestText = await manifestResponse.text()
      const manifest = JSON.parse(manifestText || 'null')
      const manifestDetails = {
        status: manifestResponse.status,
        url: manifestUrl,
      }
      checks.push(
        okStatus(manifestResponse)
          ? passed('r2_clip_manifest_available', manifestDetails)
          : failed('r2_clip_manifest_available', {
              ...manifestDetails,
              bodyPreview: manifestText.slice(0, 160),
            }),
      )

      const artifacts = array(manifest?.artifacts)
      checks.push(
        artifacts.length > 0
          ? passed('r2_clip_manifest_has_artifacts', {
              artifactCount: artifacts.length,
              url: manifestUrl,
            })
          : failed('r2_clip_manifest_has_artifacts', { url: manifestUrl }),
      )
      for (const artifact of artifacts) {
        const storageUrl = artifact?.storageUrl
        if (typeof storageUrl !== 'string' || !/^https:\/\//i.test(storageUrl)) {
          checks.push(
            failed('r2_clip_artifact_url_public_https', {
              storageUrl: storageUrl ?? null,
              url: manifestUrl,
            }),
          )
          continue
        }
        const artifactResult = await fetchClipArtifact({
          fetchImpl,
          timeoutMs,
          url: storageUrl,
        })
        checks.push(
          okStatus(artifactResult.response)
            ? passed('r2_clip_artifact_available', {
                method: artifactResult.method,
                status: artifactResult.response.status,
                storageUrl,
              })
            : failed('r2_clip_artifact_available', {
                method: artifactResult.method,
                status: artifactResult.response.status,
                storageUrl,
              }),
        )
      }
    } catch (error) {
      checks.push(
        failed('r2_clip_manifest_probe_error', {
          message: error instanceof Error ? error.message : String(error),
          url: manifestUrl,
        }),
      )
    }
  }
  return checks
}

const reportStatus = checks =>
  checks.some(check => !check.passed && check.severity === 'error')
    ? 'failed'
    : checks.some(check => !check.passed)
      ? 'warning'
      : 'passed'

export const summarizeReport = checks => ({
  errorCount: checks.filter(
    check => !check.passed && check.severity === 'error',
  ).length,
  failed: checks
    .filter(check => !check.passed)
    .map(check => ({
      details: check.details,
      name: check.name,
      severity: check.severity,
    })),
  passedCount: checks.filter(check => check.passed).length,
  status: reportStatus(checks),
  warningCount: checks.filter(
    check => !check.passed && check.severity === 'warning',
  ).length,
})

export const runVisibilityFreshnessSmoke = async ({
  baseUrl = defaultBaseUrl,
  fetchImpl = globalThis.fetch,
  generatedFrom = defaultGeneratedFrom,
  generatedLimit = 5,
  generatedTo = defaultGeneratedTo,
  maxGeneratedAgeSeconds = 120,
  maxRenderQueueAgeSeconds = 3600,
  now = new Date(),
  replayClipPath = defaultReplayClipPath,
  r2ManifestUrls = [],
  sourceLagMode = 'fail',
  ssePath = defaultSsePath,
  timeoutMs = 15_000,
  timelinePath = defaultTimelinePath,
} = {}) => {
  const origin = trimBaseUrl(baseUrl)
  const checks = []

  const routeChecks = await checkPublicRoutes({
    baseUrl: origin,
    fetchImpl,
    generatedFrom,
    generatedLimit,
    generatedTo,
    replayClipPath,
    timeoutMs,
  })
  checks.push(...routeChecks)

  let timelineBody = null
  let timelineUrl = absoluteUrl(origin, timelinePath)
  try {
    const timelineResult = await requestJson({
      baseUrl: origin,
      fetchImpl,
      path: timelinePath,
      timeoutMs,
    })
    timelineBody = timelineResult.body
    timelineUrl = timelineResult.url
    checks.push(
      okStatus(timelineResult.response)
        ? passed('timeline_route_200', {
            status: timelineResult.response.status,
            url: timelineUrl,
          })
        : failed('timeline_route_200', {
            status: timelineResult.response.status,
            url: timelineUrl,
          }),
    )
    checks.push(
      ...checkTimelineFreshness(timelineBody, {
        maxGeneratedAgeSeconds,
        now,
        url: timelineUrl,
      }),
      ...checkSourceLag(timelineBody, {
        mode: sourceLagMode,
        url: timelineUrl,
      }),
    )
  } catch (error) {
    checks.push(
      failed('timeline_fetch_error', {
        message: error instanceof Error ? error.message : String(error),
        url: timelineUrl,
      }),
    )
  }

  checks.push(
    ...(await checkSseHealth({
      baseUrl: origin,
      fetchImpl,
      ssePath,
      timeoutMs,
    })),
  )

  let renderQueueBody = null
  const renderQueueUrl = absoluteUrl(origin, replayClipPath)
  try {
    const renderQueue = await requestJson({
      baseUrl: origin,
      fetchImpl,
      path: replayClipPath,
      timeoutMs,
    })
    renderQueueBody = renderQueue.body
    if (okStatus(renderQueue.response)) {
      checks.push(
        ...checkRenderQueueHealth(renderQueueBody, {
          maxRenderQueueAgeSeconds,
          now,
          url: renderQueue.url,
        }),
      )
    }
  } catch (error) {
    checks.push(
      failed('render_queue_fetch_error', {
        message: error instanceof Error ? error.message : String(error),
        url: renderQueueUrl,
      }),
    )
  }

  checks.push(
    ...(await checkR2ClipAvailability({
      fetchImpl,
      manifestUrls: r2ManifestUrls,
      renderQueueBody,
      timeoutMs,
    })),
  )

  return {
    authority: 'observation_projection_retrieval_only',
    baseUrl: origin,
    checks,
    generatedAt: new Date().toISOString(),
    schemaVersion: 'openagents.visibility_freshness_smoke.v1',
    summary: summarizeReport(checks),
  }
}

const main = async () => {
  const options = parseArgs(process.argv.slice(2))
  if (options.help) {
    console.log(usage())
    return
  }
  const report = await runVisibilityFreshnessSmoke(options)
  console.log(JSON.stringify(report, null, 2))
  if (report.summary.errorCount > 0 && !options.warnOnly) {
    process.exitCode = 1
  }
}

if (process.argv[1] === scriptPath) {
  main().catch(error => {
    console.error(error instanceof Error ? error.message : String(error))
    process.exitCode = 1
  })
}
