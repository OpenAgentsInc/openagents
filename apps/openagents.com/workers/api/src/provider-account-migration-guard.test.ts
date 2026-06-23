import { readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, test } from 'vitest'

// This guard lived in the provider-account-schema package's test before the
// security-contract consolidation moved that contract to the top-level
// `@openagentsinc/provider-account-schema`. The contract is shared and
// app-agnostic; this migration guard is openagents.com-specific (it asserts
// the openagents.com Worker migrations never introduce raw credential column
// names), so it belongs here next to the migrations it checks.
describe('provider account migrations', () => {
  test('do not introduce raw credential column names', () => {
    const migrationDir = fileURLToPath(new URL('../migrations', import.meta.url))
    const forbidden = [
      'access_token',
      'refresh_token',
      'id_token',
      'code_verifier',
      'device_code',
      'auth_json',
    ]
    const providerMigrations = readdirSync(migrationDir)
      .filter(fileName => fileName.endsWith('.sql'))
      .filter(fileName =>
        readFileSync(join(migrationDir, fileName), 'utf8').includes(
          'provider_account',
        ),
      )

    expect(providerMigrations).toContain('0009_provider_accounts.sql')

    for (const fileName of providerMigrations) {
      const sql = readFileSync(join(migrationDir, fileName), 'utf8')

      for (const forbiddenName of forbidden) {
        expect(sql, `${fileName} contains ${forbiddenName}`).not.toMatch(
          new RegExp(`\\b${forbiddenName}\\b`, 'i'),
        )
      }
    }
  })
})
