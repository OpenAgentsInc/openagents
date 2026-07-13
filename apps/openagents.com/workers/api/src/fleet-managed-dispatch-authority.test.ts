import { describe, expect, test } from 'vitest'

import {
  authorizesManagedFleetUnitDispatch,
  managedFleetClaimAccountRefHash,
} from './fleet-managed-dispatch-authority'

const runRef = 'fleet_run.sarah.0123456789abcdef0123'
const workUnitRef = 'unit.fc4.managed_cloud'

describe('managed FleetRun dispatch authority', () => {
  test('keeps the claimed managed broker identity across private provider resolution', () => {
    expect(
      managedFleetClaimAccountRefHash('account.pylon.managed_cloud.broker'),
    ).toBe('account.pylon.codex.de252ca6ca49232a1208fab5')
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
