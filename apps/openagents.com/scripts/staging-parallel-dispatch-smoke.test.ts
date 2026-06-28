import { readFileSync } from 'node:fs'
import { describe, expect, test } from 'vitest'

import {
  DEFAULT_STAGING_BASE_URL,
  DUPLICATE_ACTIVE_ASSIGNMENT_REF,
  assertStagingHost,
  buildSpawnArgs,
  parseArgs,
  parseSpawnJson,
  runStagingParallelDispatchSmoke,
  validateSpawnResult,
} from './staging-parallel-dispatch-smoke.mjs'

const spawnResult = (patch: Record<string, unknown> = {}) => ({
  aggregate: {
    acceptedCount: 5,
    assignmentRefs: [
      'assignment.public.staging.1',
      'assignment.public.staging.2',
      'assignment.public.staging.3',
      'assignment.public.staging.4',
      'assignment.public.staging.5',
    ],
    durableRequestIds: [
      'chatcmpl_staging_1',
      'chatcmpl_staging_2',
      'chatcmpl_staging_3',
      'chatcmpl_staging_4',
      'chatcmpl_staging_5',
    ],
    totalVerifiedTokens: 5000,
  },
  blockerRefs: [],
  ok: true,
  plan: {
    maxParallel: 5,
    readyCodexAccountCount: 5,
  },
  results: Array.from({ length: 5 }, (_, index) => ({
    accountRefHash: `accthash_${index + 1}`,
    blockerRefs: [],
    ok: true,
    slotIndex: index,
  })),
  ...patch,
})

describe('staging parallel-dispatch smoke', () => {
  test('is staging-only and refuses production hosts', () => {
    expect(assertStagingHost(DEFAULT_STAGING_BASE_URL)).toBe(
      DEFAULT_STAGING_BASE_URL,
    )
    expect(() => assertStagingHost('https://openagents.com')).toThrow(
      /staging-only/,
    )
  })

  test('builds the five-way fixture-backed pylon khala spawn command', () => {
    const options = parseArgs(['--count', '5'], {})
    expect(buildSpawnArgs(options)).toEqual([
      'khala',
      'spawn',
      '--count',
      '5',
      '--max-parallel',
      '5',
      '--objective',
      'Run the staging pre-deploy public-safe dummy Codex fixture task.',
      '--fixture',
      '--execute',
      '--base-url',
      DEFAULT_STAGING_BASE_URL,
      '--json',
    ])
  })

  test('validates a complete five-assignment run', () => {
    const verdict = validateSpawnResult(spawnResult(), 5)
    expect(verdict.ok).toBe(true)
    expect(verdict.summary).toMatchObject({
      acceptedCount: 5,
      maxParallel: 5,
      readyCodexAccountCount: 5,
      distinctAccountHashes: 5,
      resultCount: 5,
    })
  })

  test('fails closed on duplicate_active_assignment evidence anywhere in the result', () => {
    const verdict = validateSpawnResult(
      spawnResult({
        blockerRefs: [DUPLICATE_ACTIVE_ASSIGNMENT_REF],
        ok: false,
      }),
      5,
    )
    expect(verdict.ok).toBe(false)
    expect(verdict.failures.join('\n')).toContain(
      DUPLICATE_ACTIVE_ASSIGNMENT_REF,
    )
  })

  test('fails when the planner silently lowers concurrency below five accounts', () => {
    const verdict = validateSpawnResult(
      spawnResult({
        aggregate: { acceptedCount: 4, totalVerifiedTokens: 4000 },
        plan: { maxParallel: 4, readyCodexAccountCount: 4 },
        results: Array.from({ length: 4 }, (_, index) => ({
          accountRefHash: `accthash_${index + 1}`,
          blockerRefs: [],
          ok: true,
          slotIndex: index,
        })),
      }),
      5,
    )
    expect(verdict.ok).toBe(false)
    expect(verdict.failures).toEqual(
      expect.arrayContaining([
        'expected 5 assignment results, got 4',
        'expected 5 accepted assignments, got 4',
        'expected at least 5 ready Codex accounts, got 4',
        'expected 5 distinct Codex account hashes, got 4',
        'expected advertised parallel capacity 5, got 4',
      ]),
    )
  })

  test('fails when five assignments recycle fewer than five accounts', () => {
    const result = spawnResult({
      results: Array.from({ length: 5 }, (_, index) => ({
        accountRefHash: `accthash_${(index % 2) + 1}`,
        blockerRefs: [],
        ok: true,
        slotIndex: index,
      })),
    })
    const verdict = validateSpawnResult(result, 5)

    expect(verdict.ok).toBe(false)
    expect(verdict.failures).toContain(
      'expected 5 distinct Codex account hashes, got 2',
    )
  })

  test('parses JSON even when pylon emits lifecycle lines around the final object', () => {
    expect(
      parseSpawnJson(`ignored stderr mirror\n${JSON.stringify(spawnResult())}\n`),
    ).toMatchObject({ ok: true })
  })

  test('runs pylon spawn and returns a public-safe summary', async () => {
    const calls: unknown[] = []
    const result = await runStagingParallelDispatchSmoke({
      env: { OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_test' },
      options: parseArgs(['--count', '5', '--pylon', 'pylon'], {}),
      run: async input => {
        calls.push(input)
        return {
          code: 0,
          stderr: '',
          stdout: JSON.stringify(spawnResult()),
        }
      },
    })

    expect(result).toMatchObject({
      baseUrl: DEFAULT_STAGING_BASE_URL,
      count: 5,
      ok: true,
      summary: {
        acceptedCount: 5,
        totalVerifiedTokens: 5000,
      },
    })
    expect(calls).toHaveLength(1)
  })
})

describe('deploy:safe package command', () => {
  const apiPackage = JSON.parse(
    readFileSync(new URL('../workers/api/package.json', import.meta.url), 'utf8'),
  )
  const deploySafe = apiPackage.scripts['deploy:safe']

  test('deploys staging and passes parallel-dispatch smoke before production upload', () => {
    const expectedOrder = [
      'cd ../.. && bun run check:deploy-from-main',
      '&& bun run check:deploy &&',
      '&& cd workers/api && wrangler d1 migrations apply openagents-autopilot-staging --env staging --remote',
      '&& cd ../.. && bun run build:web',
      '&& cd workers/api && wrangler deploy --env staging --containers-rollout=none --assets ../../apps/web/dist',
      '&& cd ../.. && bun run smoke:khala:staging-parallel-dispatch',
      '&& cd workers/api && wrangler d1 migrations apply openagents-autopilot --remote',
      '&& cd ../.. && bun run check:pending-migrations',
      '&& cd workers/api && wrangler deploy --containers-rollout=none --assets ../../apps/web/dist',
    ]

    expectedOrder.reduce((previousIndex, step) => {
      const index = deploySafe.indexOf(step)
      expect(index).toBeGreaterThan(previousIndex)
      return index
    }, -1)
  })
})
