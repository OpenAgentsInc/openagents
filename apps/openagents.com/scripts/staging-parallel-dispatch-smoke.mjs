#!/usr/bin/env node

import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'

export const DEFAULT_STAGING_BASE_URL =
  'https://openagents-staging.openagents.workers.dev'
export const DEFAULT_COUNT = 5
export const DUPLICATE_ACTIVE_ASSIGNMENT_REF =
  'blocker.public.pylon_dispatch.duplicate_active_assignment'

const truthy = value =>
  ['1', 'true', 'yes', 'on'].includes(
    String(value || '')
      .trim()
      .toLowerCase(),
  )

const redact = value =>
  String(value || '')
    .replace(/Bearer\s+[A-Za-z0-9._~+/=-]+/gu, 'Bearer <redacted>')
    .replace(/oa_agent_[A-Za-z0-9._~+/=-]+/gu, 'oa_agent_<redacted>')
    .replace(/sk-[A-Za-z0-9]{8,}/gu, 'sk-<redacted>')

const parsePositiveInteger = (value, fallback, label) => {
  if (value === undefined || value === null || value === '') return fallback
  const parsed = Number(value)
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive integer`)
  }
  return parsed
}

export const assertStagingHost = baseUrl => {
  const url = new URL(baseUrl)
  if (url.hostname !== 'openagents-staging.openagents.workers.dev') {
    throw new Error(
      `staging parallel-dispatch smoke is staging-only; refused ${url.hostname}`,
    )
  }
  return url.toString().replace(/\/+$/u, '')
}

export const parseArgs = (argv = process.argv.slice(2), env = process.env) => {
  const options = {
    baseUrl: env.OPENAGENTS_STAGING_BASE_URL || DEFAULT_STAGING_BASE_URL,
    count: parsePositiveInteger(
      env.OPENAGENTS_STAGING_PARALLEL_DISPATCH_COUNT,
      DEFAULT_COUNT,
      'OPENAGENTS_STAGING_PARALLEL_DISPATCH_COUNT',
    ),
    json: truthy(env.OPENAGENTS_STAGING_PARALLEL_DISPATCH_JSON),
    pylonCommand: env.PYLON || 'bun ../pylon/src/index.ts',
  }

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    const readValue = () => {
      const value = argv[index + 1]
      if (value === undefined) throw new Error(`${arg} requires a value`)
      index += 1
      return value
    }
    if (arg === '--base-url') options.baseUrl = readValue()
    else if (arg === '--count') {
      options.count = parsePositiveInteger(readValue(), DEFAULT_COUNT, '--count')
    } else if (arg === '--pylon') options.pylonCommand = readValue()
    else if (arg === '--json') options.json = true
    else if (arg === '--help' || arg === '-h') {
      options.help = true
    } else {
      throw new Error(`unknown argument: ${arg}`)
    }
  }

  return {
    ...options,
    baseUrl: assertStagingHost(options.baseUrl),
  }
}

const flattenRefs = value => {
  if (Array.isArray(value)) return value.flatMap(flattenRefs)
  if (value && typeof value === 'object') {
    return Object.values(value).flatMap(flattenRefs)
  }
  return typeof value === 'string' ? [value] : []
}

export const parseSpawnJson = stdout => {
  const text = String(stdout || '').trim()
  if (text === '') throw new Error('pylon khala spawn produced no JSON output')
  try {
    return JSON.parse(text)
  } catch {
    const start = text.indexOf('{')
    const end = text.lastIndexOf('}')
    if (start >= 0 && end > start) {
      return JSON.parse(text.slice(start, end + 1))
    }
    throw new Error('pylon khala spawn output was not valid JSON')
  }
}

export const validateSpawnResult = (result, expectedCount = DEFAULT_COUNT) => {
  const refs = flattenRefs(result)
  const blockers = Array.isArray(result?.blockerRefs) ? result.blockerRefs : []
  const results = Array.isArray(result?.results) ? result.results : []
  const distinctAccountHashes = new Set(
    results
      .map(slot => slot?.accountRefHash)
      .filter(hash => typeof hash === 'string' && hash.length > 0),
  ).size
  const acceptedCount = Number(result?.aggregate?.acceptedCount ?? 0)
  const readyAccounts = Number(result?.plan?.readyCodexAccountCount ?? 0)
  const maxParallel = Number(result?.plan?.maxParallel ?? 0)
  const duplicateRefSeen = refs.includes(DUPLICATE_ACTIVE_ASSIGNMENT_REF)
  const failures = [
    ...(duplicateRefSeen
      ? [
          `${DUPLICATE_ACTIVE_ASSIGNMENT_REF} observed; parallel dispatch regressed`,
        ]
      : []),
    ...(result?.ok !== true ? ['spawn result ok=false'] : []),
    ...(blockers.length > 0 ? [`spawn blockers present: ${blockers.join(', ')}`] : []),
    ...(results.length !== expectedCount
      ? [`expected ${expectedCount} assignment results, got ${results.length}`]
      : []),
    ...(acceptedCount !== expectedCount
      ? [`expected ${expectedCount} accepted assignments, got ${acceptedCount}`]
      : []),
    ...(readyAccounts < expectedCount
      ? [`expected at least ${expectedCount} ready Codex accounts, got ${readyAccounts}`]
      : []),
    ...(distinctAccountHashes < expectedCount
      ? [
          `expected ${expectedCount} distinct Codex account hashes, got ${distinctAccountHashes}`,
        ]
      : []),
    ...(maxParallel < expectedCount
      ? [`expected advertised parallel capacity ${expectedCount}, got ${maxParallel}`]
      : []),
  ]
  return {
    ok: failures.length === 0,
    failures,
    summary: {
      acceptedCount,
      assignmentRefs: result?.aggregate?.assignmentRefs ?? [],
      distinctAccountHashes,
      durableRequestIds: result?.aggregate?.durableRequestIds ?? [],
      maxParallel,
      readyCodexAccountCount: readyAccounts,
      resultCount: results.length,
      totalVerifiedTokens: result?.aggregate?.totalVerifiedTokens ?? 0,
    },
  }
}

const splitCommand = command =>
  String(command || '')
    .match(/(?:[^\s"']+|"[^"]*"|'[^']*')+/gu)
    ?.map(part => part.replace(/^(['"])(.*)\1$/u, '$2')) ?? []

const runCommand = ({ args, command, cwd, env }) =>
  new Promise(resolve => {
    const [bin, ...prefixArgs] = splitCommand(command)
    if (!bin) {
      resolve({ code: 1, stderr: 'empty pylon command', stdout: '' })
      return
    }
    const child = spawn(bin, [...prefixArgs, ...args], {
      cwd,
      env,
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    let stdout = ''
    let stderr = ''
    child.stdout.on('data', chunk => {
      stdout += chunk.toString()
    })
    child.stderr.on('data', chunk => {
      stderr += chunk.toString()
    })
    child.on('error', error => {
      resolve({ code: 1, stderr: error.message, stdout })
    })
    child.on('close', code => {
      resolve({ code: code ?? 1, stderr, stdout })
    })
  })

export const buildSpawnArgs = options => [
  'khala',
  'spawn',
  '--count',
  String(options.count),
  '--max-parallel',
  String(options.count),
  '--objective',
  'Run the staging pre-deploy public-safe dummy Codex fixture task.',
  '--fixture',
  '--execute',
  '--base-url',
  options.baseUrl,
  '--json',
]

export const runStagingParallelDispatchSmoke = async ({
  cwd = fileURLToPath(new URL('..', import.meta.url)),
  env = process.env,
  options,
  run = runCommand,
} = {}) => {
  const parsedOptions = options ?? parseArgs([], env)
  const output = await run({
    args: buildSpawnArgs(parsedOptions),
    command: parsedOptions.pylonCommand,
    cwd,
    env: {
      ...env,
      PYLON_OPENAGENTS_BASE_URL: parsedOptions.baseUrl,
    },
  })
  if (output.code !== 0) {
    throw new Error(
      `pylon khala spawn failed (${output.code}): ${redact(output.stderr || output.stdout)}`,
    )
  }
  const result = parseSpawnJson(output.stdout)
  const verdict = validateSpawnResult(result, parsedOptions.count)
  if (!verdict.ok) {
    throw new Error(
      `staging parallel-dispatch smoke failed: ${verdict.failures.join('; ')}`,
    )
  }
  return {
    ok: true,
    baseUrl: parsedOptions.baseUrl,
    command: 'pylon khala spawn',
    count: parsedOptions.count,
    summary: verdict.summary,
  }
}

const HELP = `Staging parallel-dispatch smoke gate

Deploy gate for issue #6409. Runs five concurrent fixture-backed Khala coding
delegations through caller-owned Pylon/Codex capacity against the isolated
Cloudflare staging Worker, then fails closed on duplicate_active_assignment or
any incomplete assignment.

Usage:
  bun scripts/staging-parallel-dispatch-smoke.mjs [--base-url <staging-url>] [--count 5] [--pylon <command>] [--json]

Environment:
  OPENAGENTS_AGENT_TOKEN                      required by pylon khala spawn
  OPENAGENTS_STAGING_BASE_URL                 defaults to ${DEFAULT_STAGING_BASE_URL}
  OPENAGENTS_STAGING_PARALLEL_DISPATCH_COUNT  defaults to ${DEFAULT_COUNT}
  PYLON                                       defaults to "bun ../pylon/src/index.ts"
`

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    const options = parseArgs()
    if (options.help) {
      process.stdout.write(HELP)
      process.exit(0)
    }
    const result = await runStagingParallelDispatchSmoke({ options })
    if (options.json) {
      process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
    } else {
      process.stdout.write(
        [
          'staging parallel-dispatch smoke: PASS',
          `baseUrl=${result.baseUrl}`,
          `accepted=${result.summary.acceptedCount}/${result.count}`,
          `tokens=${result.summary.totalVerifiedTokens}`,
        ].join('\n') + '\n',
      )
    }
  } catch (error) {
    process.stderr.write(
      `staging parallel-dispatch smoke: FAIL: ${redact(error?.message ?? String(error))}\n`,
    )
    process.exit(1)
  }
}
