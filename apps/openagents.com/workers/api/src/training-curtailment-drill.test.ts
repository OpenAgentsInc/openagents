import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CurtailmentDrillBlocker,
  MaxCurtailmentAckLatencyMs,
  MaxCurtailmentHaltLatencyMs,
  TrainingCurtailmentDrill,
  evaluateCurtailmentDrill,
  evaluateUntrustedCurtailmentDrill,
} from './training-curtailment-drill'

const passingDrill: TrainingCurtailmentDrill = {
  drillRef: 'training.run.r1.curtailment_drill.0003',
  runRef: 'training.run.r1',
  scheduled: true,
  signalAcknowledged: true,
  ackLatencyMs: 4_200,
  haltCompleted: true,
  haltLatencyMs: 118_000,
  durableCheckpointSealed: true,
  resumeVerified: true,
}

describe('curtailment drill evaluator', () => {
  test('passes a scheduled, in-SLA, durably-sealed, resume-verified drill', () => {
    const gate = evaluateCurtailmentDrill(passingDrill)
    expect(gate.passed).toBe(true)
    expect(gate.decision).toBe('drill_passed')
    expect(gate.reasons).toEqual([])
    expect(gate.blockerRef).toBe(CurtailmentDrillBlocker)
  })

  test('incomplete when the drill was not scheduled', () => {
    const gate = evaluateCurtailmentDrill({ ...passingDrill, scheduled: false })
    expect(gate.passed).toBe(false)
    expect(gate.decision).toBe('drill_incomplete')
    expect(gate.reasons).toContain('drill_not_scheduled')
  })

  test('incomplete when the curtailment signal was never acknowledged', () => {
    const gate = evaluateCurtailmentDrill({
      ...passingDrill,
      signalAcknowledged: false,
    })
    expect(gate.passed).toBe(false)
    expect(gate.reasons).toContain('curtailment_signal_not_acknowledged')
  })

  test('incomplete when acknowledgement blew the ack SLA', () => {
    const gate = evaluateCurtailmentDrill({
      ...passingDrill,
      ackLatencyMs: MaxCurtailmentAckLatencyMs + 1,
    })
    expect(gate.passed).toBe(false)
    expect(gate.reasons).toContain('ack_latency_exceeded')
  })

  test('incomplete when the halt never completed', () => {
    const gate = evaluateCurtailmentDrill({
      ...passingDrill,
      haltCompleted: false,
    })
    expect(gate.passed).toBe(false)
    expect(gate.reasons).toContain('halt_not_completed')
  })

  test('incomplete when the halt blew the load-shed SLA', () => {
    const gate = evaluateCurtailmentDrill({
      ...passingDrill,
      haltLatencyMs: MaxCurtailmentHaltLatencyMs + 1,
    })
    expect(gate.passed).toBe(false)
    expect(gate.reasons).toContain('halt_latency_exceeded')
  })

  test('incomplete when the run halted without a durable checkpoint seal', () => {
    const gate = evaluateCurtailmentDrill({
      ...passingDrill,
      durableCheckpointSealed: false,
    })
    expect(gate.passed).toBe(false)
    expect(gate.reasons).toContain('durable_checkpoint_not_sealed')
  })

  test('incomplete when resume from the sealed checkpoint was not verified', () => {
    const gate = evaluateCurtailmentDrill({
      ...passingDrill,
      resumeVerified: false,
    })
    expect(gate.passed).toBe(false)
    expect(gate.reasons).toContain('resume_not_verified')
  })

  test('a malformed descriptor fails toward incomplete, never toward passed', () => {
    const gate = evaluateUntrustedCurtailmentDrill({ scheduled: true })
    expect(gate.passed).toBe(false)
    expect(gate.decision).toBe('drill_incomplete')
    expect(gate.reasons).toEqual(['drill_descriptor_malformed'])
  })

  test('a well-formed untrusted descriptor decodes and evaluates', () => {
    const gate = evaluateUntrustedCurtailmentDrill(passingDrill)
    expect(gate.passed).toBe(true)
    expect(S.decodeUnknownSync(TrainingCurtailmentDrill)(passingDrill)).toEqual(
      passingDrill,
    )
  })
})
