#!/usr/bin/env node

import { pathToFileURL } from 'node:url'

const DEFAULT_BASE_URL = 'https://openagents.com'
const STATUS_TOPIC_ID = '88888888-4001-4001-8001-888888888888'

const expectedD1Tables = [
  'artanis_approval_gates',
  'artanis_forum_publication_intents',
  'artanis_health_snapshots',
  'artanis_loop_records',
  'artanis_loop_ticks',
  'artanis_nexus_pylon_adapter_dispatches',
  'artanis_runtime_snapshots',
  'artanis_work_routing_proposals',
]

const expectedReportFields = [
  'autonomousLoop',
  'forumRewardSmoke',
  'healthSummary',
  'productionLaunchGate',
  'pylonLaunchCommunication',
]

export const usage = () => `Usage:
  node scripts/artanis-production-readiness.mjs [options]

Options:
  --base-url <url>                 OpenAgents origin. Defaults to https://openagents.com.
  --d1-tables <csv>                Read-only D1 table names from an operator query.
  --source-commit <ref>            Public-safe source commit ref, for example commit.public.autopilot_omega.abc123.
  --production-smoke-ref <ref>     Public-safe retained production-equivalent smoke ref.
  --scheduled-runner <state>       enabled, disabled, or unknown. Defaults to unknown.
  --latest-pylon-release-tag <tag> Observed latest Pylon release tag.
  --pylon-v02-release-tag <tag>    Observed v0.2 release tag, if present.
  --pylon-v02-release-assets <n>   Observed v0.2 release asset count. Defaults to 0.
  --json                           Print JSON. This is the default.
  --help                           Show this help.

This script performs only public HTTP reads plus caller-supplied read-only
signals. It does not call Wrangler, mutate D1, deploy, post to Forum, dispatch
Pylon work, spend bitcoin, or change scheduler state.`

const valueFlags = new Set([
  'base-url',
  'baseUrl',
  'd1-tables',
  'd1Tables',
  'latest-pylon-release-tag',
  'latestPylonReleaseTag',
  'production-smoke-ref',
  'productionSmokeRef',
  'pylon-v02-release-assets',
  'pylonV02ReleaseAssets',
  'pylon-v02-release-tag',
  'pylonV02ReleaseTag',
  'scheduled-runner',
  'scheduledRunner',
  'source-commit',
  'sourceCommit',
])

const booleanFlags = new Set(['help', 'h', 'json'])

const canonicalFlagName = name =>
  ({
    baseUrl: 'base-url',
    d1Tables: 'd1-tables',
    h: 'help',
    latestPylonReleaseTag: 'latest-pylon-release-tag',
    productionSmokeRef: 'production-smoke-ref',
    pylonV02ReleaseAssets: 'pylon-v02-release-assets',
    pylonV02ReleaseTag: 'pylon-v02-release-tag',
    scheduledRunner: 'scheduled-runner',
    sourceCommit: 'source-commit',
  })[name] || name

export const parseReadinessArgs = argv => {
  const flags = new Map()

  for (let index = 0; index < argv.length; index += 1) {
    const raw = argv[index]

    if (!raw.startsWith('--')) {
      throw new Error(`Unexpected positional argument: ${raw}`)
    }

    const name = canonicalFlagName(raw.slice(2))

    if (booleanFlags.has(name)) {
      flags.set(name, true)
      continue
    }

    if (!valueFlags.has(name)) {
      throw new Error(`Unknown option: ${raw}`)
    }

    const value = argv[index + 1]

    if (value === undefined || value.startsWith('--')) {
      throw new Error(`Missing value for ${raw}`)
    }

    flags.set(name, value)
    index += 1
  }

  return { flags }
}

const flagText = (flags, name) => {
  const value = flags.get(name)

  return typeof value === 'string' ? value.trim() : undefined
}

const numberFlag = (flags, name, fallback) => {
  const value = flagText(flags, name)

  if (value === undefined || value === '') {
    return fallback
  }

  const parsed = Number(value)

  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(`--${name} must be a non-negative integer.`)
  }

  return parsed
}

export const redactSecrets = text =>
  text
    .replace(/Bearer\s+[A-Za-z0-9._-]+/g, 'Bearer <redacted>')
    .replace(/oa_agent_[A-Za-z0-9._-]+/g, 'oa_agent_<redacted>')
    .replace(/sk-[A-Za-z0-9._-]+/g, 'sk-<redacted>')
    .replace(/gh[pousr]_[A-Za-z0-9_]+/g, 'gh_<redacted>')
    .replace(/(OPENAGENTS_ADMIN_API_TOKEN=)[^\s]+/g, '$1<redacted>')

export const commaList = value =>
  value === undefined || value.trim() === ''
    ? []
    : value.split(',').map(item => item.trim()).filter(Boolean)

const jsonRequest = async (fetchFn, baseUrl, path) => {
  try {
    const response = await fetchFn(new URL(path, baseUrl))

    if (!response.ok) {
      return { ok: false, status: response.status, value: null }
    }

    return { ok: true, status: response.status, value: await response.json() }
  } catch (error) {
    return {
      ok: false,
      status: 0,
      value: null,
      error: error instanceof Error ? error.message : String(error),
    }
  }
}

const textRequest = async (fetchFn, baseUrl, path) => {
  try {
    const response = await fetchFn(new URL(path, baseUrl))

    if (!response.ok) {
      return { ok: false, status: response.status, value: '' }
    }

    return { ok: true, status: response.status, value: await response.text() }
  } catch (error) {
    return {
      ok: false,
      status: 0,
      value: '',
      error: error instanceof Error ? error.message : String(error),
    }
  }
}

const scheduledRunnerState = value => {
  if (value === 'enabled') {
    return true
  }

  if (value === 'disabled') {
    return false
  }

  if (value === undefined || value === 'unknown') {
    return null
  }

  throw new Error('--scheduled-runner must be enabled, disabled, or unknown.')
}

export const buildObservationFromPublicReads = async (
  parsed,
  env = process.env,
  fetchFn = globalThis.fetch,
) => {
  const baseUrl = flagText(parsed.flags, 'base-url') || DEFAULT_BASE_URL
  const [
    report,
    pylonStats,
    statusTopic,
    artanisPage,
  ] = await Promise.all([
    jsonRequest(fetchFn, baseUrl, '/api/public/artanis/report'),
    jsonRequest(fetchFn, baseUrl, '/api/public/pylon-stats'),
    jsonRequest(fetchFn, baseUrl, `/api/forum/topics/${STATUS_TOPIC_ID}`),
    textRequest(fetchFn, baseUrl, '/artanis'),
  ])
  const d1TablesFlag = flagText(parsed.flags, 'd1-tables')
  const sourceCommitRef =
    flagText(parsed.flags, 'source-commit') ||
    env.OPENAGENTS_ARTANIS_SOURCE_COMMIT_REF ||
    null
  const productionSmokeRef =
    flagText(parsed.flags, 'production-smoke-ref') ||
    env.OPENAGENTS_ARTANIS_PRODUCTION_SMOKE_REF ||
    null
  const latestPylonReleaseTag =
    flagText(parsed.flags, 'latest-pylon-release-tag') ||
    env.OPENAGENTS_LATEST_PYLON_RELEASE_TAG ||
    null
  const pylonV02ReleaseTag =
    flagText(parsed.flags, 'pylon-v02-release-tag') ||
    env.OPENAGENTS_PYLON_V02_RELEASE_TAG ||
    null

  return {
    artanisPageReachable: artanisPage.ok,
    d1TableNames:
      d1TablesFlag === undefined ? null : commaList(d1TablesFlag),
    latestPylonReleaseTag,
    productionSmokeRef,
    publicReportFields:
      report.ok && report.value !== null && typeof report.value === 'object'
        ? Object.keys(report.value)
        : [],
    pylonStatsStatus: pylonStats.ok
      ? pylonStats.value?.status === 'unavailable'
        ? 'unavailable'
        : 'fresh'
      : 'unavailable',
    pylonV02ReleaseAssetCount: numberFlag(
      parsed.flags,
      'pylon-v02-release-assets',
      0,
    ),
    pylonV02ReleaseTag,
    scheduledRunnerEnabled: scheduledRunnerState(
      flagText(parsed.flags, 'scheduled-runner'),
    ),
    sourceCommitRef,
    statusTopicPostCount:
      statusTopic.ok && Array.isArray(statusTopic.value?.posts)
        ? statusTopic.value.posts.length
        : null,
  }
}

const check = (id, passed, unavailable = false) => ({
  id,
  status: unavailable ? 'unavailable' : passed ? 'passed' : 'blocked',
})

export const summarizeObservation = observation => {
  const missingD1Tables = observation.d1TableNames === null
    ? expectedD1Tables
    : expectedD1Tables.filter(table => !observation.d1TableNames.includes(table))
  const missingReportFields = expectedReportFields.filter(
    field => !observation.publicReportFields.includes(field),
  )
  const releaseReady =
    observation.pylonV02ReleaseTag === 'pylon-v0.2.0' &&
    observation.pylonV02ReleaseAssetCount > 0
  const smokeReady = observation.productionSmokeRef !== null
  const schedulerReady =
    observation.scheduledRunnerEnabled === true && smokeReady
  const checks = [
    check('source_commit', observation.sourceCommitRef !== null),
    check(
      'public_report_fields',
      missingReportFields.length === 0,
    ),
    check('artanis_page', observation.artanisPageReachable),
    check(
      'd1_persistence',
      missingD1Tables.length === 0,
      observation.d1TableNames === null,
    ),
    check(
      'forum_status_topic',
      observation.statusTopicPostCount !== null &&
        observation.statusTopicPostCount > 0,
      observation.statusTopicPostCount === null,
    ),
    {
      id: 'pylon_stats',
      status: observation.pylonStatsStatus === 'fresh'
        ? 'passed'
        : observation.pylonStatsStatus,
    },
    check('pylon_v02_release', releaseReady),
    check('production_e2e_smoke', smokeReady),
    check(
      'scheduled_runner_state',
      schedulerReady,
      observation.scheduledRunnerEnabled === null,
    ),
  ]
  const blockers = [
    ...missingD1Tables.map(table => `missing_d1_table:${table}`),
    ...missingReportFields.map(field => `missing_report_field:${field}`),
    ...(releaseReady ? [] : ['pylon_v0_2_release_not_shipped']),
    ...(smokeReady ? [] : ['production_smoke_missing']),
    ...(schedulerReady ? [] : ['scheduler_not_ready']),
  ]

  return {
    authority: {
      d1MutationAllowed: false,
      deploymentAllowed: false,
      forumMutationAllowed: false,
      gitHubReleaseMutationAllowed: false,
      pylonDispatchAllowed: false,
      schedulerMutationAllowed: false,
      walletSpendAllowed: false,
    },
    blockers,
    checks,
    missingD1Tables,
    missingReportFields,
    state: checks.every(item => item.status === 'passed') ? 'ready' : 'blocked',
  }
}

export const safeOutput = value =>
  JSON.parse(redactSecrets(JSON.stringify(value, null, 2)))

export const main = async argv => {
  const parsed = parseReadinessArgs(argv)

  if (parsed.flags.has('help')) {
    console.log(usage())
    return
  }

  const observation = await buildObservationFromPublicReads(parsed)
  const summary = summarizeObservation(observation)

  console.log(JSON.stringify(safeOutput({
    observation,
    summary,
  }), null, 2))
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2)).catch(error => {
    console.error(redactSecrets(error instanceof Error ? error.message : String(error)))
    process.exitCode = 1
  })
}
