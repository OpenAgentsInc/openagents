import { describe, expect, test } from 'vitest'

import {
  authorizesManagedFleetUnitDispatch,
  newestManagedFleetProviderAccountsFirst,
  selectExactManagedFleetProviderAccount,
} from './fleet-managed-dispatch-authority'

const runRef = 'fleet_run.sarah.0123456789abcdef0123'
const workUnitRef = 'unit.fc4.managed_cloud'

describe('managed FleetRun dispatch authority', () => {
  test('advertises the most recently refreshed eligible provider first', () => {
    expect(newestManagedFleetProviderAccountsFirst([
      { providerAccountRef: 'provider.stale', updatedAt: '2026-07-12T23:18:21.166Z' },
      { providerAccountRef: 'provider.fresh', updatedAt: '2026-07-13T08:27:45.933Z' },
    ]).map(account => account.providerAccountRef)).toEqual([
      'provider.fresh',
      'provider.stale',
    ])
  })

  test('selects only the exact pre-claimed provider and never substitutes another healthy account', async () => {
    const firstHash = `account.pylon.codex.${'a'.repeat(24)}`
    const claimedHash = `account.pylon.codex.${'b'.repeat(24)}`
    const accounts = [
      { providerAccountRef: 'provider.first' },
      { providerAccountRef: 'provider.claimed' },
    ]
    const selected = await selectExactManagedFleetProviderAccount(
      accounts,
      claimedHash,
      async account =>
        account.providerAccountRef === 'provider.claimed'
          ? claimedHash
          : firstHash,
    )
    expect(selected?.account.providerAccountRef).toBe('provider.claimed')
    expect(
      await selectExactManagedFleetProviderAccount(
        accounts,
        `account.pylon.codex.${'c'.repeat(24)}`,
        async account => account.providerAccountRef === 'provider.claimed' ? claimedHash : firstHash,
      ),
    ).toBeUndefined()
  })

  test('fails closed when two provider rows resolve to the same claimed hash', async () => {
    const duplicateHash = `account.pylon.codex.${'d'.repeat(24)}`
    expect(
      await selectExactManagedFleetProviderAccount(
        [{ id: 1 }, { id: 2 }],
        duplicateHash,
        async () => duplicateHash,
      ),
    ).toBeUndefined()
  })

  test('accepts an exact per-unit claim under a separately accepted run lease', () => {
    expect(
      authorizesManagedFleetUnitDispatch({
        acceptedRunLease: true,
        runStatus: 'claimed_by_pylon',
        runRef,
        workUnitRef,
        unitClaimRef: `${runRef}.claim.${workUnitRef}.1783917001735.1`,
      }),
    ).toBe(true)
  })

  test('never substitutes the whole-run intake claim for the per-unit claim', () => {
    expect(
      authorizesManagedFleetUnitDispatch({
        acceptedRunLease: true,
        runStatus: 'claimed_by_pylon',
        runRef,
        workUnitRef,
        unitClaimRef: 'claim.sarah_fleet_run.1f0264736ea8a0e77e22e84d',
      }),
    ).toBe(false)
  })

  test('rejects a different unit or an unaccepted run lease', () => {
    const unitClaimRef = `${runRef}.claim.${workUnitRef}.1783917001735.1`
    expect(
      authorizesManagedFleetUnitDispatch({
        acceptedRunLease: true,
        runStatus: 'running',
        runRef,
        workUnitRef: 'unit.fc4.owner_local',
        unitClaimRef,
      }),
    ).toBe(false)
    expect(
      authorizesManagedFleetUnitDispatch({
        acceptedRunLease: false,
        runStatus: 'running',
        runRef,
        workUnitRef,
        unitClaimRef,
      }),
    ).toBe(false)
  })
})
