import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  MaxStandbyHeartbeatStalenessMs,
  StandbyDispatchBlocker,
  TrainingStandbyDispatch,
  evaluateStandbyDispatch,
  evaluateUntrustedStandbyDispatch,
} from './training-standby-dispatch'

const promotableDispatch: TrainingStandbyDispatch = {
  standbyContributorRef: 'training.run.r1.standby.pylon.0003',
  runRef: 'training.run.r1',
  qualified: true,
  bannedForRound: false,
  bootstrapSealVerified: true,
  bootstrapSealWindowRef: 'training.run.r1.window.0007',
  liveSealedWindowRef: 'training.run.r1.window.0007',
  liveVacancyCount: 1,
  lastHeartbeatAgeMs: 5_000,
}

describe('standby dispatch evaluator', () => {
  test('promotes a qualified, bootstrapped, live, unbanned standby into a vacancy', () => {
    const gate = evaluateStandbyDispatch(promotableDispatch)
    expect(gate.promotable).toBe(true)
    expect(gate.decision).toBe('promote_standby')
    expect(gate.reasons).toEqual([])
    expect(gate.blockerRef).toBe(StandbyDispatchBlocker)
  })

  test('holds when the standby never passed qualification', () => {
    const gate = evaluateStandbyDispatch({
      ...promotableDispatch,
      qualified: false,
    })
    expect(gate.promotable).toBe(false)
    expect(gate.decision).toBe('hold_standby')
    expect(gate.reasons).toContain('standby_not_qualified')
  })

  test('holds when the standby is banned for the current round', () => {
    const gate = evaluateStandbyDispatch({
      ...promotableDispatch,
      bannedForRound: true,
    })
    expect(gate.promotable).toBe(false)
    expect(gate.reasons).toContain('standby_banned_for_round')
  })

  test('holds when bootstrap from the durable seal is not yet verified', () => {
    const gate = evaluateStandbyDispatch({
      ...promotableDispatch,
      bootstrapSealVerified: false,
    })
    expect(gate.promotable).toBe(false)
    expect(gate.reasons).toContain('bootstrap_seal_not_verified')
  })

  test('holds when the standby bootstrapped from a window other than the live seal', () => {
    const gate = evaluateStandbyDispatch({
      ...promotableDispatch,
      bootstrapSealWindowRef: 'training.run.r1.window.0006',
    })
    expect(gate.promotable).toBe(false)
    expect(gate.reasons).toContain('bootstrap_seal_window_mismatch')
  })

  test('holds when there is no live vacancy to promote into', () => {
    const gate = evaluateStandbyDispatch({
      ...promotableDispatch,
      liveVacancyCount: 0,
    })
    expect(gate.promotable).toBe(false)
    expect(gate.reasons).toContain('no_live_vacancy')
  })

  test('holds when the standby heartbeat is stale', () => {
    const gate = evaluateStandbyDispatch({
      ...promotableDispatch,
      lastHeartbeatAgeMs: MaxStandbyHeartbeatStalenessMs + 1,
    })
    expect(gate.promotable).toBe(false)
    expect(gate.reasons).toContain('standby_heartbeat_stale')
  })

  test('a malformed descriptor fails toward hold, never toward promotion', () => {
    const gate = evaluateUntrustedStandbyDispatch({
      standbyContributorRef: 'training.run.r1.standby.pylon.0003',
    })
    expect(gate.promotable).toBe(false)
    expect(gate.decision).toBe('hold_standby')
    expect(gate.reasons).toEqual(['dispatch_descriptor_malformed'])
  })

  test('a well-formed untrusted descriptor decodes and evaluates', () => {
    const gate = evaluateUntrustedStandbyDispatch(promotableDispatch)
    expect(gate.promotable).toBe(true)
    expect(S.decodeUnknownSync(TrainingStandbyDispatch)(promotableDispatch)).toEqual(
      promotableDispatch,
    )
  })
})
