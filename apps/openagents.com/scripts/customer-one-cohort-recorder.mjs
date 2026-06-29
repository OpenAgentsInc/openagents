#!/usr/bin/env node

import { readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

export const DEFAULT_BASE_URL = 'https://openagents.com'
export const PUBLIC_COHORT_PATH = '/api/public/customer-one-cohort'
export const OPERATOR_COHORT_ROWS_PATH =
  '/api/operator/customer-one-cohort/rows'

const valueFlags = new Set([
  'base-url',
  'baseUrl',
  'row-file',
  'rowFile',
  'row-json',
  'rowJson',
])
const booleanFlags = new Set(['help', 'h', 'json'])
const adminCommands = new Set(['list', 'upsert'])
const cohortRowStates = new Set([
  'candidate',
  'invited',
  'workspace_seeded',
  'first_run_started',
  'delivery_reviewed',
  'loop_completed',
  'blocked',
  'deferred',
])
const templatePlaceholderMarkers = [
  /\breplace(?:[-_. ]?me)?\b/i,
  /\bplaceholder\b/i,
  /\btodo\b/i,
  /\btbd\b/i,
]

const unsafeRowMarkers = [
  /raw[_ -]prompt/i,
  /raw[_ -]shell/i,
  /shell[_ -]log/i,
  /stack[_ -]trace/i,
  /private[_ -]repo/i,
  /private[_ -]content/i,
  /provider[_ -]payload/i,
  /invoice/i,
  /payment[_ -]hash/i,
  /preimage/i,
  /mnemonic/i,
  /bearer/i,
  /oauth/i,
  /api[_ -]?key/i,
  /(?:^|[\s"':])\/Users\//,
  /(?:^|[\s"':])(?:git|ssh|https?):\/\//i,
  /git@/i,
  /[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/i,
]

export const usage = () => `Usage:
  node scripts/customer-one-cohort-recorder.mjs public [--json]
  node scripts/customer-one-cohort-recorder.mjs audit [--json]
  node scripts/customer-one-cohort-recorder.mjs check --row-file row.json [--json]
  node scripts/customer-one-cohort-recorder.mjs check --row-json '{"teamCohortRef":"cohort.team.alpha.v1",...}' [--json]
  node scripts/customer-one-cohort-recorder.mjs list [--json]
  node scripts/customer-one-cohort-recorder.mjs upsert --row-file row.json [--json]
  node scripts/customer-one-cohort-recorder.mjs upsert --row-json '{"teamCohortRef":"cohort.team.alpha.v1",...}' [--json]

Options:
  --base-url <url>     Override OPENAGENTS_BASE_URL. Defaults to ${DEFAULT_BASE_URL}.
  --row-file <path>    Read one cohort source row from a JSON file.
  --row-json <json>    Read one cohort source row from inline JSON. Prefer --row-file for real rows.
  --json               Print the raw JSON response.
  --help               Show this help.

Environment:
  OPENAGENTS_BASE_URL          Optional base URL.
  OPENAGENTS_ADMIN_API_TOKEN   Required for list and upsert.

This recorder posts public-safe refs only. The audit command reads only the
public projection and fails until that projection proves the D3 completion
gate. The check and upsert commands refuse obvious raw prompts, private paths,
URLs, emails, bearer/API tokens, wallet/payment material, provider payload
markers, unresolved template placeholders, and completed rows missing
completion/privacy review refs. It does not create fake cohort rows and does
not complete #5098 by itself.`

const canonicalFlagName = name =>
  ({
    baseUrl: 'base-url',
    h: 'help',
    rowFile: 'row-file',
    rowJson: 'row-json',
  })[name] || name

const flagValue = (parsed, name) => {
  const value = parsed.flags.get(name)

  return typeof value === 'string' && value.trim() !== ''
    ? value.trim()
    : undefined
}

const hasFlag = (parsed, name) => parsed.flags.get(name) === true

export const parseRecorderArgs = argv => {
  const command = argv[0]
  const flags = new Map()
  const rest = argv.slice(1)

  for (let index = 0; index < rest.length; index += 1) {
    const raw = rest[index]

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

    const value = rest[index + 1]

    if (value === undefined || value.startsWith('--')) {
      throw new Error(`Missing value for ${raw}`)
    }

    flags.set(name, value)
    index += 1
  }

  return { command, flags }
}

const escapeRegExp = value => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

export const redactSecrets = (text, extraValues = []) => {
  const redacted = String(text)
    .replace(/Bearer\s+[A-Za-z0-9._~+/-]+/g, 'Bearer <redacted>')
    .replace(
      /OPENAGENTS_ADMIN_API_TOKEN=[^\s]+/g,
      'OPENAGENTS_ADMIN_API_TOKEN=<redacted>',
    )
    .replace(/oa_agent_[A-Za-z0-9._-]+/g, 'oa_agent_<redacted>')
    .replace(/oa_admin_[A-Za-z0-9._-]+/g, 'oa_admin_<redacted>')

  return extraValues
    .filter(value => typeof value === 'string' && value.trim().length > 2)
    .reduce(
      (current, value) =>
        current.replace(
          new RegExp(escapeRegExp(value.trim()), 'g'),
          '<redacted:value>',
        ),
      redacted,
    )
}

const parseJsonObject = text => {
  const parsed = JSON.parse(text)

  if (parsed === null || Array.isArray(parsed) || typeof parsed !== 'object') {
    throw new Error('Cohort row JSON must be an object.')
  }

  return parsed
}

export const assertPublicSafeRow = row => {
  const text = JSON.stringify(row)
  const matched = unsafeRowMarkers.find(pattern => pattern.test(text))

  if (matched !== undefined) {
    throw new Error(
      `Cohort row contains unsafe private material marker: ${matched}`,
    )
  }

  return row
}

const isRecord = value =>
  value !== null && !Array.isArray(value) && typeof value === 'object'

const hasNonEmptyString = (row, key) =>
  typeof row[key] === 'string' && row[key].trim() !== ''

const collectStringLeaves = (value, path = '$') => {
  if (typeof value === 'string') {
    return [{ path, value }]
  }

  if (Array.isArray(value)) {
    return value.flatMap((item, index) =>
      collectStringLeaves(item, `${path}[${index}]`),
    )
  }

  if (isRecord(value)) {
    return Object.entries(value).flatMap(([key, child]) =>
      collectStringLeaves(child, `${path}.${key}`),
    )
  }

  return []
}

export const validateCohortRowPacket = row => {
  assertPublicSafeRow(row)

  const errors = []

  if (!hasNonEmptyString(row, 'teamCohortRef')) {
    errors.push('teamCohortRef is required.')
  }

  if (!hasNonEmptyString(row, 'state')) {
    errors.push('state is required.')
  } else if (!cohortRowStates.has(row.state)) {
    errors.push(
      `state must be one of: ${Array.from(cohortRowStates).join(', ')}.`,
    )
  }

  if (!hasNonEmptyString(row, 'updatedAt')) {
    errors.push('updatedAt is required.')
  } else if (Number.isNaN(Date.parse(row.updatedAt))) {
    errors.push('updatedAt must be an ISO timestamp.')
  }

  if (row.blockerRefs !== undefined && !Array.isArray(row.blockerRefs)) {
    errors.push('blockerRefs must be an array when present.')
  }

  if (row.caveatRefs !== undefined && !Array.isArray(row.caveatRefs)) {
    errors.push('caveatRefs must be an array when present.')
  }

  if (row.state === 'loop_completed') {
    if (!hasNonEmptyString(row, 'completionBundleRef')) {
      errors.push('loop_completed rows require completionBundleRef.')
    }

    if (!hasNonEmptyString(row, 'privacyReviewRef')) {
      errors.push('loop_completed rows require privacyReviewRef.')
    }
  }

  const placeholders = collectStringLeaves(row).filter(({ value }) =>
    templatePlaceholderMarkers.some(pattern => pattern.test(value)),
  )

  if (placeholders.length > 0) {
    errors.push(
      `unresolved template placeholders: ${placeholders
        .map(({ path }) => path)
        .join(', ')}.`,
    )
  }

  if (errors.length > 0) {
    throw new Error(`Invalid cohort row packet:\n- ${errors.join('\n- ')}`)
  }

  return row
}

export const readRowInput = (
  parsed,
  readTextFile = path => readFileSync(path, 'utf8'),
) => {
  const rowFile = flagValue(parsed, 'row-file')
  const rowJson = flagValue(parsed, 'row-json')

  if (rowFile !== undefined && rowJson !== undefined) {
    throw new Error('Use either --row-file or --row-json, not both.')
  }

  if (rowFile === undefined && rowJson === undefined) {
    throw new Error('Missing --row-file or --row-json for row packet.')
  }

  return validateCohortRowPacket(
    parseJsonObject(rowJson ?? readTextFile(rowFile)),
  )
}

const adminTokenFor = (command, env) => {
  const token = env.OPENAGENTS_ADMIN_API_TOKEN

  if (!adminCommands.has(command)) {
    return undefined
  }

  if (typeof token !== 'string' || token.trim() === '') {
    throw new Error(`Missing OPENAGENTS_ADMIN_API_TOKEN for ${command}.`)
  }

  return token.trim()
}

export const baseUrlFor = (parsed, env) =>
  flagValue(parsed, 'base-url') ?? env.OPENAGENTS_BASE_URL ?? DEFAULT_BASE_URL

export const buildRecorderRequest = (
  parsed,
  env,
  readTextFile = path => readFileSync(path, 'utf8'),
) => {
  const token = adminTokenFor(parsed.command, env)

  if (parsed.command === 'public' || parsed.command === 'audit') {
    return {
      body: undefined,
      headers: { accept: 'application/json' },
      method: 'GET',
      path: PUBLIC_COHORT_PATH,
    }
  }

  if (parsed.command === 'list') {
    return {
      body: undefined,
      headers: {
        Authorization: `Bearer ${token}`,
        accept: 'application/json',
      },
      method: 'GET',
      path: OPERATOR_COHORT_ROWS_PATH,
    }
  }

  if (parsed.command === 'upsert') {
    return {
      body: readRowInput(parsed, readTextFile),
      headers: {
        Authorization: `Bearer ${token}`,
        accept: 'application/json',
        'content-type': 'application/json',
      },
      method: 'POST',
      path: OPERATOR_COHORT_ROWS_PATH,
    }
  }

  throw new Error(`Unknown command: ${parsed.command ?? '<missing>'}`)
}

export const requestJson = async (request, options = {}) => {
  const fetchImpl = options.fetchImpl ?? globalThis.fetch
  const response = await fetchImpl(new URL(request.path, options.baseUrl), {
    body: request.body === undefined ? undefined : JSON.stringify(request.body),
    headers: request.headers,
    method: request.method,
  })
  const payload = await response.json().catch(() => ({
    error: 'invalid_json_response',
    status: response.status,
  }))

  return { ok: response.ok, payload, status: response.status }
}

const completedCount = payload =>
  typeof payload?.counts?.loop_completed === 'number'
    ? payload.counts.loop_completed
    : 0

const requiredCompletionCount = payload =>
  typeof payload?.target?.minimumCompletedTeams === 'number'
    ? Math.max(3, payload.target.minimumCompletedTeams)
    : 3

const countedCompletionRows = payload =>
  Array.isArray(payload?.rows)
    ? payload.rows.filter(row => row?.countsTowardD3Completion === true).length
    : 0

export const auditCohortProjection = payload => {
  const requiredCompleted = requiredCompletionCount(payload)
  const completedTeams = completedCount(payload)
  const countedRows = countedCompletionRows(payload)
  const gateState = payload?.gate?.state ?? 'unknown'
  const reasonRefs = Array.isArray(payload?.gate?.reasonRefs)
    ? payload.gate.reasonRefs
    : []
  const blockerRefs = Array.isArray(payload?.blockerRefs)
    ? payload.blockerRefs
    : []
  const rowCount = Array.isArray(payload?.rows) ? payload.rows.length : 0
  const blockers = [
    ...(gateState === 'ready'
      ? []
      : [`customer-one-cohort-audit:gate-${gateState}`]),
    ...(completedTeams >= requiredCompleted
      ? []
      : ['customer-one-cohort-audit:insufficient-completed-count']),
    ...(countedRows >= requiredCompleted
      ? []
      : ['customer-one-cohort-audit:insufficient-counted-rows']),
    ...reasonRefs,
    ...blockerRefs,
  ]

  return {
    blockerRefs: [...new Set(blockers)],
    completedTeams,
    countedRows,
    gateState,
    ok: blockers.length === 0,
    requiredCompleted,
    rowCount,
  }
}

export const humanSummary = (command, result) => {
  const payload = result.payload

  if (command === 'audit') {
    return [
      `Customer #1 cohort audit: ${payload?.ok === true ? 'ready' : 'blocked'}`,
      `Completed teams: ${payload?.completedTeams ?? 0}/${payload?.requiredCompleted ?? 3}`,
      `Counted completion rows: ${payload?.countedRows ?? 0}/${payload?.requiredCompleted ?? 3}`,
      `Gate: ${payload?.gateState ?? 'unknown'}`,
      `Rows: ${payload?.rowCount ?? 0}`,
    ].join('\n')
  }

  if (command === 'check') {
    return [
      `Customer #1 cohort row packet valid: ${payload?.row?.teamCohortRef ?? 'unknown'}`,
      `State: ${payload?.row?.state ?? 'unknown'}`,
      `Counts toward D3: ${payload?.countsTowardD3 === true ? 'yes' : 'no'}`,
    ].join('\n')
  }

  if (command === 'public') {
    return [
      `Customer #1 cohort gate: ${payload?.gate?.state ?? 'unknown'}`,
      `Completed teams: ${completedCount(payload)}`,
      `Rows: ${Array.isArray(payload?.rows) ? payload.rows.length : 0}`,
    ].join('\n')
  }

  if (command === 'list') {
    return [
      `Customer #1 private rows: ${Array.isArray(payload?.rows) ? payload.rows.length : 0}`,
      `Generated at: ${payload?.generatedAt ?? 'unknown'}`,
    ].join('\n')
  }

  return [
    `Customer #1 cohort row stored: ${payload?.row?.teamCohortRef ?? 'unknown'}`,
    `State: ${payload?.row?.state ?? 'unknown'}`,
  ].join('\n')
}

export const runRecorder = async (argv, env = process.env, io = {}) => {
  const parsed = parseRecorderArgs(argv)
  const stdout = io.stdout ?? process.stdout
  const stderr = io.stderr ?? process.stderr

  if (parsed.command === undefined || hasFlag(parsed, 'help')) {
    stdout.write(`${usage()}\n`)
    return parsed.command === undefined ? 2 : 0
  }

  if (parsed.command === 'check') {
    const row = readRowInput(parsed, io.readTextFile)
    const payload = {
      countsTowardD3: row.state === 'loop_completed',
      row,
    }

    if (hasFlag(parsed, 'json')) {
      stdout.write(`${JSON.stringify(payload, null, 2)}\n`)
    } else {
      stdout.write(`${humanSummary(parsed.command, { payload })}\n`)
    }

    return 0
  }

  const request = buildRecorderRequest(parsed, env, io.readTextFile)
  const result = await requestJson(request, {
    baseUrl: baseUrlFor(parsed, env),
    fetchImpl: io.fetchImpl,
  })

  if (parsed.command === 'audit') {
    const payload = auditCohortProjection(result.payload)

    if (hasFlag(parsed, 'json')) {
      stdout.write(`${JSON.stringify(payload, null, 2)}\n`)
    } else {
      stdout.write(`${humanSummary(parsed.command, { payload })}\n`)
    }

    if (!result.ok) {
      stderr.write(
        `${redactSecrets(
          `Customer #1 cohort audit failed: ${result.status} ${JSON.stringify(result.payload)}`,
          [env.OPENAGENTS_ADMIN_API_TOKEN],
        )}\n`,
      )
      return 1
    }

    if (!payload.ok) {
      stderr.write(
        `Customer #1 cohort audit blocked: ${payload.blockerRefs.join(', ')}\n`,
      )
      return 1
    }

    return 0
  }

  if (hasFlag(parsed, 'json')) {
    stdout.write(`${JSON.stringify(result.payload, null, 2)}\n`)
  } else {
    stdout.write(`${humanSummary(parsed.command, result)}\n`)
  }

  if (!result.ok) {
    stderr.write(
      `${redactSecrets(
        `Customer #1 cohort ${parsed.command} failed: ${result.status} ${JSON.stringify(result.payload)}`,
        [env.OPENAGENTS_ADMIN_API_TOKEN],
      )}\n`,
    )
    return 1
  }

  return 0
}

const isCli =
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href

if (isCli) {
  runRecorder(process.argv.slice(2))
    .then(exitCode => {
      process.exitCode = exitCode
    })
    .catch(error => {
      console.error(
        redactSecrets(error instanceof Error ? error.message : String(error), [
          process.env.OPENAGENTS_ADMIN_API_TOKEN,
        ]),
      )
      process.exitCode = 1
    })
}
