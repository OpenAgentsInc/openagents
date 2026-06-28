import { readFileSync } from 'node:fs'
import { describe, expect, test } from 'vitest'

import {
  DEFAULT_DATABASE,
  decidePendingMigrations,
  parseMigrationsList,
} from './check-pending-migrations.mjs'

describe('parseMigrationsList', () => {
  test('returns [] for the "No migrations to apply!" sentinel', () => {
    expect(parseMigrationsList('No migrations to apply!')).toEqual([])
    expect(
      parseMigrationsList(
        'Migrations have been applied!\nNo migrations to apply!\n',
      ),
    ).toEqual([])
  })

  test('extracts pending migration filenames from a wrangler table', () => {
    // The exact shape that masked the 2026-06-25 outage: 0234 pending.
    const raw = [
      '🌀 Loading...',
      '┌───────────────────────────────┐',
      '│ Name                          │',
      '├───────────────────────────────┤',
      '│ 0234_pylon_openauth_links.sql │',
      '└───────────────────────────────┘',
    ].join('\n')
    expect(parseMigrationsList(raw)).toEqual(['0234_pylon_openauth_links.sql'])
  })

  test('extracts multiple pending migrations and de-dupes', () => {
    const raw = [
      'Migrations to be applied:',
      '0234_pylon_openauth_links.sql',
      '0235_some_followup.sql',
      '0234_pylon_openauth_links.sql',
    ].join('\n')
    expect(parseMigrationsList(raw)).toEqual([
      '0234_pylon_openauth_links.sql',
      '0235_some_followup.sql',
    ])
  })

  test('ignores non-migration noise lines', () => {
    const raw = 'Fetching... \nDone in 1.2s\nNo migrations to apply!'
    expect(parseMigrationsList(raw)).toEqual([])
  })
})

describe('decidePendingMigrations', () => {
  test('zero pending => ok, exit 0', () => {
    const d = decidePendingMigrations([])
    expect(d.ok).toBe(true)
    expect(d.exitCode).toBe(0)
    expect(d.message).toContain(DEFAULT_DATABASE)
  })

  test('any pending => not ok, exit 1, names the files + the remediation', () => {
    const d = decidePendingMigrations(['0234_pylon_openauth_links.sql'])
    expect(d.ok).toBe(false)
    expect(d.exitCode).toBe(1)
    expect(d.message).toContain('0234_pylon_openauth_links.sql')
    expect(d.message).toContain('must NOT ship ahead of its schema')
    expect(d.message).toContain('wrangler d1 migrations apply')
  })
})

describe('deploy:safe package command', () => {
  const apiPackage = JSON.parse(
    readFileSync(new URL('../workers/api/package.json', import.meta.url), 'utf8'),
  )
  const deploySafe = apiPackage.scripts['deploy:safe']

  test('keeps migration-first zero-pending build upload ordering', () => {
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

  test('disables Wrangler container rollout probing on the final upload', () => {
    expect(deploySafe).toContain(
      'wrangler deploy --containers-rollout=none --assets ../../apps/web/dist',
    )
  })

  test('disables Wrangler container rollout probing on the staging upload', () => {
    expect(deploySafe).toContain(
      'wrangler deploy --env staging --containers-rollout=none --assets ../../apps/web/dist',
    )
  })
})
