#!/usr/bin/env node
/**
 * OB-2 (#8559) Apollo segment wave runner.
 *
 * Fixture / dry-run tier (default): builds ≥100 synthetic public-safe
 * prospects and runs them through the real D1 store in memory to prove
 * first-pass ingest, suppression, idempotent re-wave, and subjectRef
 * dedupe across a second wave with fresh pipelineRefs.
 *
 * Dry-run executes via Vitest (same node:sqlite D1 double as Worker API
 * tests). print-fixture / live use Node strip-types on the pure fixture.
 *
 * Usage:
 *   bun run apollo-wave:dry-run -- --count 100
 *   bun run apollo-wave:print-fixture -- --segment agencies_seo
 *   bun run apollo-wave:live -- --body ./wave.local.json
 *
 * Env (live only):
 *   OPENAGENTS_ADMIN_API_TOKEN (required)
 *   OPENAGENTS_BASE_URL (default https://openagents.com)
 */

import { spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  apolloWaveIngestBodyFromFixture,
  buildOb2ApolloWaveFixture,
  OB2_APOLLO_WAVE_SEGMENTS,
  OB2_LIVE_WAVE_SEGMENT_PAIR,
  type Ob2ApolloWaveSegmentKey,
} from '../src/business-pipeline-apollo-wave-fixture.ts'

const here = dirname(fileURLToPath(import.meta.url))
const packageRoot = join(here, '..')

const usage = `Usage:
  bun run apollo-wave:dry-run -- [--count 100]
  bun run apollo-wave:print-fixture -- --segment agencies_seo [--count 100] [--wave-id fixture]
  bun run apollo-wave:live -- --body ./wave.local.json
  bun run apollo-wave:live -- --segment agencies_seo --wave-id 20260709a --count 100 --allow-synthetic

Segments: ${Object.keys(OB2_APOLLO_WAVE_SEGMENTS).join(', ')}
Live exit-gate pair: ${OB2_LIVE_WAVE_SEGMENT_PAIR.join(' + ')}

Env (live):
  OPENAGENTS_ADMIN_API_TOKEN is required.
  OPENAGENTS_BASE_URL defaults to https://openagents.com.`

const isSegmentKey = (value: string): value is Ob2ApolloWaveSegmentKey =>
  Object.prototype.hasOwnProperty.call(OB2_APOLLO_WAVE_SEGMENTS, value)

const parseFlags = (
  argv: ReadonlyArray<string>,
): ReadonlyMap<string, string> => {
  const flags = new Map<string, string>()
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index]
    if (flag === undefined || !flag.startsWith('--')) {
      throw new Error(usage)
    }
    const key = flag.slice(2)
    const next = argv[index + 1]
    if (next === undefined || next.startsWith('--')) {
      flags.set(key, 'true')
      continue
    }
    flags.set(key, next)
    index += 1
  }
  return flags
}

const parseCount = (flags: ReadonlyMap<string, string>): number => {
  const raw = flags.get('count')
  if (raw === undefined) return 100
  const count = Number(raw)
  if (!Number.isInteger(count) || count < 1 || count > 500) {
    throw new Error('--count must be an integer 1-500')
  }
  return count
}

const requireSegment = (
  flags: ReadonlyMap<string, string>,
): Ob2ApolloWaveSegmentKey => {
  const segment = flags.get('segment')
  if (segment === undefined || !isSegmentKey(segment)) {
    throw new Error(
      `--segment must be one of: ${Object.keys(OB2_APOLLO_WAVE_SEGMENTS).join(', ')}`,
    )
  }
  return segment
}

const runDryRun = (flags: ReadonlyMap<string, string>): void => {
  const count = String(parseCount(flags))
  const vitestBin = join(packageRoot, 'node_modules', '.bin', 'vitest')
  const result = spawnSync(
    vitestBin,
    [
      'run',
      'src/business-pipeline-apollo-wave-dry-run.cli.test.ts',
      '--reporter=verbose',
    ],
    {
      cwd: packageRoot,
      env: {
        ...process.env,
        OB2_APOLLO_WAVE_DRY_RUN_COUNT: count,
        OB2_APOLLO_WAVE_DRY_RUN_PRINT: '1',
      },
      stdio: 'inherit',
    },
  )
  if (result.error) {
    throw result.error
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}

const runPrintFixture = (flags: ReadonlyMap<string, string>): void => {
  const fixture = buildOb2ApolloWaveFixture({
    count: parseCount(flags),
    distinctPipelineRefs: flags.get('distinct-pipeline-refs') === 'true',
    segmentKey: requireSegment(flags),
    waveId: flags.get('wave-id') ?? 'fixture',
  })
  console.log(JSON.stringify(apolloWaveIngestBodyFromFixture(fixture), null, 2))
}

const postLiveWave = async (body: unknown): Promise<unknown> => {
  const token = process.env.OPENAGENTS_ADMIN_API_TOKEN
  if (token === undefined || token.trim() === '') {
    throw new Error('OPENAGENTS_ADMIN_API_TOKEN is required for live mode.')
  }
  const baseUrl = process.env.OPENAGENTS_BASE_URL ?? 'https://openagents.com'
  const response = await fetch(
    new URL('/api/operator/business/pipeline/apollo-waves', baseUrl),
    {
      body: JSON.stringify(body),
      headers: {
        authorization: `Bearer ${token}`,
        'content-type': 'application/json',
      },
      method: 'POST',
    },
  )
  const payload = await response.json().catch(() => ({}))
  if (!response.ok) {
    throw new Error(
      `live wave failed (${response.status}): ${JSON.stringify(payload)}`,
    )
  }
  return payload
}

const runLive = async (flags: ReadonlyMap<string, string>): Promise<void> => {
  const bodyPath = flags.get('body')
  if (bodyPath !== undefined) {
    const body = JSON.parse(readFileSync(bodyPath, 'utf8')) as unknown
    const payload = await postLiveWave(body)
    console.log(JSON.stringify(payload, null, 2))
    return
  }

  const fixture = buildOb2ApolloWaveFixture({
    count: parseCount(flags),
    distinctPipelineRefs: flags.get('distinct-pipeline-refs') === 'true',
    segmentKey: requireSegment(flags),
    waveId:
      flags.get('wave-id') ??
      new Date().toISOString().slice(0, 10).replaceAll('-', ''),
  })

  if (flags.get('allow-synthetic') !== 'true') {
    throw new Error(
      [
        'Live mode without --body refuses synthetic fixture posts by default.',
        'Either:',
        '  1) Pass --body ./wave.local.json built from Apollo MCP public-safe subjectRefs, or',
        '  2) Pass --allow-synthetic to post the synthetic fixture (staging / proof only).',
        '',
        'See docs/fable/2026-07-09-ob2-apollo-segment-wave-operator-runbook.md',
      ].join('\n'),
    )
  }

  const payload = await postLiveWave(apolloWaveIngestBodyFromFixture(fixture))
  console.log(JSON.stringify(payload, null, 2))
}

const main = async (): Promise<void> => {
  const [command, ...rest] = process.argv.slice(2)
  if (
    command !== 'dry-run' &&
    command !== 'print-fixture' &&
    command !== 'live'
  ) {
    throw new Error(usage)
  }
  const flags = parseFlags(rest)

  if (command === 'dry-run') {
    runDryRun(flags)
    return
  }
  if (command === 'print-fixture') {
    runPrintFixture(flags)
    return
  }
  await runLive(flags)
}

main().catch(error => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exit(1)
})
