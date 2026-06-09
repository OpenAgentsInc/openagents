import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OPENAGENTS_PROBE_CONFORMANCE_FIXTURES,
  OpenAgentsProbeContractUnsafe,
  OpenAgentsProbeRunProjection,
  OpenAgentsProbeRunRecord,
  openAgentsProbeRunHasRequiredTerminalEvidence,
  openAgentsProbeRunIsTerminal,
  openAgentsProbeRunProjectionHasPrivateMaterial,
  openAgentsProbeRunRequiresRetainedFailure,
  projectOpenAgentsProbeRun,
} from './probe-coding-runtime-contract'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T02:15:00.000Z'

const fixture = (index: number): OpenAgentsProbeRunRecord =>
  S.decodeUnknownSync(OpenAgentsProbeRunRecord)(
    OPENAGENTS_PROBE_CONFORMANCE_FIXTURES[index],
  )

describe('OpenAgents Probe coding-runtime contract', () => {
  test('decodes conformance fixtures for Rust Probe adapters to mirror', () => {
    expect(OPENAGENTS_PROBE_CONFORMANCE_FIXTURES.map(run => run.id)).toEqual([
      'probe_run.fixture.success',
      'probe_run.fixture.retained_failure',
    ])

    for (const run of OPENAGENTS_PROBE_CONFORMANCE_FIXTURES) {
      expect(S.decodeUnknownSync(OpenAgentsProbeRunRecord)(run)).toEqual(run)
    }
  })

  test('projects successful runs with safe turns, tool calls, artifacts, tests, and closeout receipts', () => {
    const run = fixture(0)
    const projection = projectOpenAgentsProbeRun(run, 'public', nowIso)

    expect(S.decodeUnknownSync(OpenAgentsProbeRunProjection)(projection))
      .toEqual(projection)
    expect(openAgentsProbeRunIsTerminal(run.status)).toBe(true)
    expect(openAgentsProbeRunHasRequiredTerminalEvidence(run)).toBe(true)
    expect(projection.status).toBe('succeeded')
    expect(projection.terminal).toBe(true)
    expect(projection.toolCalls[0]?.diagnosticRefs).toEqual([])
    expect(projection.turnEvents[0]?.toolCallRefs).toEqual([
      'tool_call.probe.test_runner.1',
    ])
    expect(projection.createdAtDisplay).toBe('14 minutes ago')
    expect(projection.updatedAtDisplay).toBe('10 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(openAgentsProbeRunProjectionHasPrivateMaterial(projection)).toBe(
      false,
    )
  })

  test('models retained failures and required retained-failure evidence', () => {
    const run = fixture(1)
    const projection = projectOpenAgentsProbeRun(run, 'operator', nowIso)

    expect(openAgentsProbeRunRequiresRetainedFailure('failed')).toBe(true)
    expect(openAgentsProbeRunRequiresRetainedFailure('timed_out')).toBe(true)
    expect(openAgentsProbeRunRequiresRetainedFailure(run.status)).toBe(false)
    expect(openAgentsProbeRunHasRequiredTerminalEvidence(run)).toBe(true)
    expect(projection.status).toBe('retained_failure')
    expect(projection.failureRefs).toEqual(['failure.probe.timeout_summary'])
    expect(projection.retainedFailureRefs).toEqual([
      'retained_failure.probe.timeout_1',
    ])
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('requires terminal evidence for success and failure states', () => {
    const successWithoutReceipt = {
      ...fixture(0),
      closeoutReceiptRefs: [],
    }
    const failedWithoutRetainedFailure = {
      ...fixture(1),
      retainedFailureRefs: [],
      status: 'failed' as const,
    }

    expect(openAgentsProbeRunHasRequiredTerminalEvidence(successWithoutReceipt))
      .toBe(false)
    expect(openAgentsProbeRunHasRequiredTerminalEvidence(
      failedWithoutRetainedFailure,
    )).toBe(false)
  })

  test('rejects raw logs, provider payloads, credentials, private repos, wallet/payment material, and timestamps', () => {
    for (const fixtureValue of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'tool log', value: 'raw_tool_log.full' },
      { label: 'provider payload', value: 'raw_provider_payload.full' },
    ]) {
      expect(() =>
        projectOpenAgentsProbeRun(
          {
            ...fixture(0),
            toolCalls: [
              {
                ...fixture(0).toolCalls[0]!,
                diagnosticRefs: [fixtureValue.value],
              },
            ],
          },
          'operator',
          nowIso,
        ),
      ).toThrow(OpenAgentsProbeContractUnsafe)
    }
  })
})
