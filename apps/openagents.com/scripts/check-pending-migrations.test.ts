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
