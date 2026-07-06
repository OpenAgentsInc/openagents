import { describe, expect, test } from 'vitest'

import type { IdentityDb } from './identity-db'
import {
  readOperatorTargetByIdentity,
  readOperatorTargetByUserId,
  readOperatorTargetUser,
} from './operator-targets'

type QueryBinding = Readonly<{
  query: string
  values: ReadonlyArray<unknown>
}>

const jsonFixture = <T>(value: unknown): T => JSON.parse(JSON.stringify(value))

// CFG-4 Domain 2 (#8519): operator target resolution reads the Postgres
// identity handle now — the scripted store records each query + params the
// same way the old scripted D1 recorded `.bind()` values.
const makeScriptedIdentityDb = (row: unknown | null) => {
  const bindings: Array<QueryBinding> = []
  const identityDb: IdentityDb = {
    batch: () => Promise.resolve(),
    query: (query, params = []) => {
      bindings.push({ query, values: params })

      return Promise.resolve(
        row === null ? [] : [jsonFixture<Record<string, unknown>>(row)],
      )
    },
  }

  return { bindings, identityDb }
}

const targetRow = {
  display_name: 'Christopher David',
  github_username: 'AtlantisPleb',
  primary_email: 'chris@openagents.com',
  user_id: 'github:14167547',
}

describe('operator target repository helpers', () => {
  test('reads targets by user id', async () => {
    const { bindings, identityDb } = makeScriptedIdentityDb(targetRow)

    await expect(
      readOperatorTargetByUserId(identityDb, 'github:14167547'),
    ).resolves.toEqual({
      displayName: 'Christopher David',
      email: 'chris@openagents.com',
      githubUsername: 'AtlantisPleb',
      userId: 'github:14167547',
    })
    expect(bindings[0]?.values).toEqual(['github:14167547'])
  })

  test('normalizes identity selectors', async () => {
    const { bindings, identityDb } = makeScriptedIdentityDb(targetRow)

    await readOperatorTargetByIdentity(identityDb, '@AtlantisPleb')

    expect(bindings[0]?.values).toEqual([
      'atlantispleb',
      'atlantispleb',
      'atlantispleb',
    ])
  })

  test('falls back to the configured default identity', async () => {
    const { bindings, identityDb } = makeScriptedIdentityDb(null)

    await expect(
      readOperatorTargetUser(identityDb, {}, 'chris@openagents.com'),
    ).resolves.toBeUndefined()
    expect(bindings[0]?.values).toEqual([
      'chris@openagents.com',
      'chris@openagents.com',
      'chris@openagents.com',
    ])
  })
})
