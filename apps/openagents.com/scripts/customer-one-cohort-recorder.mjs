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

This recorder posts public-safe refs only. It refuses obvious raw prompts,
private paths, URLs, emails, bearer/API tokens, wallet/payment material, and
provider payload markers before sending an upsert request. It does not create
fake cohort rows and does not complete #5098 by itself.`

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
    throw new Error('Missing --row-file or --row-json for upsert.')
  }

  return assertPublicSafeRow(parseJsonObject(rowJson ?? readTextFile(rowFile)))
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

  if (parsed.command === 'public') {
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

export const humanSummary = (command, result) => {
  const payload = result.payload

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

  const request = buildRecorderRequest(parsed, env, io.readTextFile)
  const result = await requestJson(request, {
    baseUrl: baseUrlFor(parsed, env),
    fetchImpl: io.fetchImpl,
  })

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
