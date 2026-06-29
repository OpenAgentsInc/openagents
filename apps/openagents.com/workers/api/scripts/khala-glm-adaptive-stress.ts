#!/usr/bin/env bun
import { mkdir, mkdtemp, readFile, rm, unlink } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  GLM_REAP_SERVED_MODEL,
  buildGlmLiveAdaptiveStressArtifact,
  classifyGlmLiveAdaptiveStressFailure,
  decideGlmLiveAdaptiveStressConcurrency,
  decideGlmLiveAdaptiveStressWindowBreaker,
  glmLiveAdaptiveStressHeaders,
  type GlmLiveAdaptiveStressObservation,
  type GlmLiveAdaptiveStressWindow,
} from '../src/inference/benchmark/live-adaptive-stress-runner'
import {
  currentEpochMillis,
  currentIsoTimestamp,
  epochMillisToIsoTimestamp,
} from '../src/runtime-primitives'

const args = process.argv.slice(2)

const option = (name: string): string | undefined => {
  const index = args.indexOf(name)
  if (index < 0) return undefined
  const value = args[index + 1]
  return value === undefined || value.startsWith('--') ? undefined : value
}

const flag = (name: string): boolean => args.includes(name)

const numberOption = (name: string, fallback: number): number => {
  const raw = option(name)
  const parsed = Number(raw)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
}

const timestampForRunId = (): string =>
  currentIsoTimestamp().replace(/[:-]/gu, '').replace(/\.\d{3}Z$/u, 'Z')

const help = (): string => [
  'Usage: bun run scripts/khala-glm-adaptive-stress.ts [options]',
  '',
  'Public-gateway #6317 GLM adaptive stress runner using curl-shaped requests.',
  'Requires OPENAGENTS_AGENT_TOKEN or KHALA_AGENT_TOKEN unless --dry-run is set.',
  '',
  'Options:',
  '  --run-id <id>                 Stable demand client/run id.',
  '  --base-url <url>              Default: https://openagents.com',
  '  --model <model>               Default: openagents/khala',
  '  --duration-ms <number>        Launch window, default 300000.',
  '  --window-ms <number>          Adaptive decision window, default 60000.',
  '  --initial-concurrency <n>     Default 2.',
  '  --min-concurrency <n>         Default 2.',
  '  --max-concurrency <n>         Default 6.',
  '  --max-tokens <n>              Default 512.',
  '  --timeout-seconds <n>         Per request curl timeout, default 90.',
  '  --output-dir <dir>            Default: a tmp dir named after the run id.',
  '  --dry-run                     Write an empty public-safe artifact and exit.',
  '  --summary                     Print compact summary JSON.',
  '  --help',
  '',
].join('\n')

if (flag('--help')) {
  process.stdout.write(help())
  process.exit(0)
}

const runId = option('--run-id') ?? `issue6317-adaptive-${timestampForRunId()}`
const baseUrl = (option('--base-url') ?? 'https://openagents.com').replace(
  /\/+$/u,
  '',
)
const model = option('--model') ?? 'openagents/khala'
const durationMs = numberOption('--duration-ms', 300_000)
const windowMs = numberOption('--window-ms', 60_000)
const initialConcurrency = numberOption('--initial-concurrency', 2)
const minConcurrency = numberOption('--min-concurrency', 2)
const maxConcurrency = numberOption('--max-concurrency', 6)
const maxTokens = numberOption('--max-tokens', 512)
const timeoutSeconds = numberOption('--timeout-seconds', 90)
const outputDir = option('--output-dir') ?? join(tmpdir(), runId)
const token = Bun.env.OPENAGENTS_AGENT_TOKEN ?? Bun.env.KHALA_AGENT_TOKEN

const stressPrompt = [
  'OpenAgents public-safe GLM saturation fixture.',
  'Write a dense technical operations note about scheduler backoff, replica',
  'admission, exact token accounting, and public-safe telemetry. Use numbered',
  'sections, concrete but fictional values, and continue until the response',
  'naturally reaches the requested budget. Do not include secrets, URLs,',
  'credentials, private prompts, or customer data.',
].join(' ')

type CurlMetrics = Readonly<{
  httpCode: number
  timeStartTransfer: number
  timeTotal: number
}>

const parseCurlMetrics = (stdout: string): CurlMetrics => {
  try {
    const parsed = JSON.parse(stdout.trim()) as Partial<CurlMetrics>
    return {
      httpCode:
        typeof parsed.httpCode === 'number' && Number.isFinite(parsed.httpCode)
          ? parsed.httpCode
          : 0,
      timeStartTransfer:
        typeof parsed.timeStartTransfer === 'number' &&
        Number.isFinite(parsed.timeStartTransfer)
          ? parsed.timeStartTransfer
          : 0,
      timeTotal:
        typeof parsed.timeTotal === 'number' && Number.isFinite(parsed.timeTotal)
          ? parsed.timeTotal
          : 0,
    }
  } catch {
    return { httpCode: 0, timeStartTransfer: 0, timeTotal: 0 }
  }
}

const responseObject = async (
  bodyPath: string,
): Promise<Record<string, unknown>> => {
  try {
    const raw = await readFile(bodyPath, 'utf8')
    const parsed = JSON.parse(raw)
    return parsed !== null && typeof parsed === 'object'
      ? (parsed as Record<string, unknown>)
      : {}
  } catch {
    return {}
  }
}

const recordValue = (
  value: unknown,
): Record<string, unknown> =>
  value !== null && typeof value === 'object'
    ? (value as Record<string, unknown>)
    : {}

const numberField = (record: Record<string, unknown>, name: string): number => {
  const value = record[name]
  return typeof value === 'number' && Number.isFinite(value) ? value : 0
}

const stringField = (
  record: Record<string, unknown>,
  name: string,
): string | null => {
  const value = record[name]
  return typeof value === 'string' && value.trim() !== '' ? value : null
}

const runCurlRequest = async (
  requestIndex: number,
  privateDir: string,
): Promise<GlmLiveAdaptiveStressObservation> => {
  const requestRef = `${runId}-${String(requestIndex).padStart(6, '0')}`
  const bodyPath = join(privateDir, `${requestRef}.body.json`)
  const requestBody = JSON.stringify({
    max_tokens: maxTokens,
    messages: [{ content: stressPrompt, role: 'user' }],
    model,
    stream: false,
    temperature: 0.2,
  })
  const publicHeaders = glmLiveAdaptiveStressHeaders(runId, requestRef)
  const curlArgs = [
    '--http1.1',
    '--silent',
    '--show-error',
    '--max-time',
    String(timeoutSeconds),
    '--request',
    'POST',
    `${baseUrl}/api/v1/chat/completions`,
    '--header',
    'accept: application/json',
    '--header',
    'content-type: application/json',
    '--header',
    'user-agent: Mozilla/5.0 AppleWebKit/537.36 Chrome/126 Safari/537.36',
    '--header',
    `authorization: Bearer ${token}`,
    '--header',
    `x-openagents-demand-kind: ${publicHeaders['x-openagents-demand-kind']}`,
    '--header',
    `x-openagents-demand-source: ${publicHeaders['x-openagents-demand-source']}`,
    '--header',
    `x-openagents-client: ${publicHeaders['x-openagents-client']}`,
    '--header',
    `x-openagents-request-ref: ${publicHeaders['x-openagents-request-ref']}`,
    '--data-binary',
    '@-',
    '--output',
    bodyPath,
    '--write-out',
    '{"httpCode":%{http_code},"timeStartTransfer":%{time_starttransfer},"timeTotal":%{time_total}}',
  ]
  const startedAt = currentEpochMillis()
  const proc = Bun.spawn(['curl', ...curlArgs], {
    stderr: 'pipe',
    stdin: 'pipe',
    stdout: 'pipe',
  })
  proc.stdin.write(requestBody)
  proc.stdin.end()
  const [stdout, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    proc.exited,
    new Response(proc.stderr).text(),
  ])
  const metrics = parseCurlMetrics(stdout)
  const body = await responseObject(bodyPath)
  await unlink(bodyPath).catch(() => undefined)

  const usage = recordValue(body.usage)
  const openagents = recordValue(body.openagents)
  const error = recordValue(body.error)
  const servedModel = stringField(openagents, 'served_model')
  const worker = stringField(openagents, 'worker')
  const httpStatus = metrics.httpCode > 0 ? metrics.httpCode : null
  const errorCode = stringField(error, 'code')
  const failureKind =
    httpStatus !== null && httpStatus >= 200 && httpStatus < 300
      ? null
      : classifyGlmLiveAdaptiveStressFailure(httpStatus, exitCode)
  const status =
    httpStatus !== null && httpStatus >= 200 && httpStatus < 300
      ? 'ok'
      : httpStatus === 429 && errorCode === 'internal_stress_yielded'
        ? 'preempted_for_external'
        : 'failed'

  return {
    failureKind,
    httpStatus,
    inputTokens: numberField(usage, 'prompt_tokens'),
    outputTokens: numberField(usage, 'completion_tokens'),
    provider: worker,
    requestRef,
    servedModel,
    status,
    totalTokens: numberField(usage, 'total_tokens'),
    ttftMs:
      metrics.timeStartTransfer > 0
        ? Math.round(metrics.timeStartTransfer * 1000)
        : null,
    usageTruth: numberField(usage, 'total_tokens') > 0 ? 'exact' : 'missing',
    wallClockMs:
      metrics.timeTotal > 0
        ? Math.round(metrics.timeTotal * 1000)
        : currentEpochMillis() - startedAt,
    worker,
  }
}

await mkdir(outputDir, { recursive: true })
const privateDir = await mkdtemp(join(tmpdir(), `${runId}.private.`))

if (!flag('--dry-run') && (token === undefined || token.trim() === '')) {
  process.stderr.write(
    '[glm-adaptive-stress] missing OPENAGENTS_AGENT_TOKEN or KHALA_AGENT_TOKEN\n',
  )
  process.exit(2)
}

const startedAt = currentEpochMillis()
const deadline = startedAt + durationMs
let requestIndex = 0
let currentConcurrency = initialConcurrency
let consecutiveCleanWindows = 0
let nextWindowAt = startedAt + windowMs
const observations: Array<GlmLiveAdaptiveStressObservation> = []
let windowObservations: Array<GlmLiveAdaptiveStressObservation> = []
const windows: Array<GlmLiveAdaptiveStressWindow> = []
const inFlight = new Set<Promise<void>>()
let windowBreakerTripped = false

const sleep = (ms: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, ms))

const launch = (): void => {
  requestIndex += 1
  const promise = runCurlRequest(requestIndex, privateDir)
    .then(observation => {
      observations.push(observation)
      windowObservations.push(observation)
      process.stderr.write(
        [
          `[glm-adaptive-stress] request=${observation.requestRef}`,
          `status=${observation.status}`,
          `http=${observation.httpStatus ?? 'none'}`,
          `tokens=${observation.totalTokens}`,
          `servedModel=${observation.servedModel ?? 'none'}`,
        ].join(' ') + '\n',
      )
      if (!windowBreakerTripped) {
        const breaker = decideGlmLiveAdaptiveStressWindowBreaker({
          currentConcurrency,
          observations: windowObservations,
        })
        if (breaker.tripped) {
          windowBreakerTripped = true
          process.stderr.write(
            [
              '[glm-adaptive-stress] window-breaker=tripped',
              `observed=${breaker.observedCount}`,
              `failed=${breaker.failedCount}`,
              `preempted=${breaker.preemptedCount}`,
              `overload=${breaker.overloadFailureCount}`,
              `errorRate=${breaker.observedErrorRate ?? 'none'}`,
              `reasons=${breaker.reasonRefs.join(',')}`,
            ].join(' ') + '\n',
          )
        }
      }
    })
    .finally(() => {
      inFlight.delete(promise)
    })
  inFlight.add(promise)
}

const closeWindow = (completedAtMs: number): void => {
  const decision = decideGlmLiveAdaptiveStressConcurrency({
    cleanWindowIncreaseThreshold: 3,
    consecutiveCleanWindows,
    currentConcurrency,
    maxConcurrency,
    minConcurrency,
    observations: windowObservations,
  })
  windows.push({
    completedAt: epochMillisToIsoTimestamp(completedAtMs),
    decision,
    startedAt: epochMillisToIsoTimestamp(nextWindowAt - windowMs),
  })
  currentConcurrency = decision.nextConcurrency
  consecutiveCleanWindows = decision.nextConsecutiveCleanWindows
  windowBreakerTripped = false
  process.stderr.write(
    [
      `[glm-adaptive-stress] window action=${decision.action}`,
      `current=${decision.currentConcurrency}`,
      `next=${decision.nextConcurrency}`,
      `ok=${decision.okCount}`,
      `failed=${decision.failedCount}`,
      `preempted=${decision.preemptedCount}`,
      `overload=${decision.overloadFailureCount}`,
    ].join(' ') + '\n',
  )
  windowObservations = []
  nextWindowAt = completedAtMs + windowMs
}

if (flag('--dry-run')) {
  const artifact = buildGlmLiveAdaptiveStressArtifact({
    demandClient: runId,
    durationMs: 0,
    finalConcurrency: initialConcurrency,
    generatedAt: currentIsoTimestamp(),
    initialConcurrency,
    maxConcurrency,
    maxTokens,
    minConcurrency,
    model,
    observations,
    runId,
    windowMs,
    windows,
  })
  await Bun.write(
    join(outputDir, `${runId}.public.json`),
    `${JSON.stringify(artifact, null, 2)}\n`,
  )
  process.stdout.write(
    `${JSON.stringify(flag('--summary') ? artifact.summary : artifact, null, 2)}\n`,
  )
  await rm(privateDir, { force: true, recursive: true })
  process.exit(0)
}

while (currentEpochMillis() < deadline || inFlight.size > 0) {
  while (
    currentEpochMillis() < deadline &&
    !windowBreakerTripped &&
    inFlight.size < currentConcurrency
  ) {
    launch()
  }
  const now = currentEpochMillis()
  if (
    now >= nextWindowAt ||
    (windowBreakerTripped && inFlight.size === 0 && windowObservations.length > 0)
  ) {
    closeWindow(now)
  }
  if (inFlight.size === 0) {
    await sleep(250)
  } else {
    await Promise.race([...inFlight, sleep(250)])
  }
}

if (windowObservations.length > 0) {
  closeWindow(currentEpochMillis())
}

const artifact = buildGlmLiveAdaptiveStressArtifact({
  demandClient: runId,
  durationMs: currentEpochMillis() - startedAt,
  finalConcurrency: currentConcurrency,
  generatedAt: currentIsoTimestamp(),
  initialConcurrency,
  maxConcurrency,
  maxTokens,
  minConcurrency,
  model,
  observations,
  runId,
  windowMs,
  windows,
})

await Bun.write(
  join(outputDir, `${runId}.public.json`),
  `${JSON.stringify(artifact, null, 2)}\n`,
)
await rm(privateDir, { force: true, recursive: true })

process.stderr.write(
  [
    `[glm-adaptive-stress] wrotePublicArtifact=${join(
      outputDir,
      `${runId}.public.json`,
    )}`,
    `[glm-adaptive-stress] glmServedModel=${GLM_REAP_SERVED_MODEL}`,
    `[glm-adaptive-stress] totalExactGlmTokens=${artifact.summary.totalTokens}`,
  ].join('\n') + '\n',
)

process.stdout.write(
  `${JSON.stringify(flag('--summary') ? artifact.summary : artifact, null, 2)}\n`,
)
