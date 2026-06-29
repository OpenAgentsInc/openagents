import { describe, expect, test } from 'vitest'

import { normalizeOmniRunnerEventPayload } from './runner-event'

describe('Omni runner event compatibility normalizer', () => {
  test('normalizes complete runner event payloads', () => {
    expect(
      normalizeOmniRunnerEventPayload(
        {
          artifactRefs: ['artifact_1', 42, 'artifact_2'],
          externalEventId: 'runner:event:1',
          sequence: 7,
          source: 'shc',
          status: 'running',
          summary: 'Runner produced output.',
          type: 'runner.log',
          value: 'stdout: hello',
        },
        3,
      ),
    ).toEqual({
      artifactRefs: ['artifact_1', 'artifact_2'],
      externalEventId: 'runner:event:1',
      payload: {
        artifactRefs: ['artifact_1', 42, 'artifact_2'],
        externalEventId: 'runner:event:1',
        sequence: 7,
        source: 'shc',
        status: 'running',
        summary: 'Runner produced output.',
        type: 'runner.log',
        value: 'stdout: hello',
      },
      sequence: 7,
      source: 'shc',
      status: 'running',
      summary: 'Runner produced output.',
      type: 'runner.log',
    })
  })

  test('fills stable defaults for partial payloads', () => {
    expect(normalizeOmniRunnerEventPayload({ sequence: '8' }, 4)).toEqual({
      artifactRefs: [],
      payload: { sequence: '8' },
      sequence: 4,
      source: 'runner',
      summary: 'Runner event received.',
      type: 'runner.event',
    })
  })

  test('preserves SHC snake case event ids and artifacts', () => {
    expect(
      normalizeOmniRunnerEventPayload(
        {
          artifact_refs: ['artifact_1'],
          external_event_id: 'runner:event:2',
          sequence: 9,
          source: 'codex',
          summary: 'stdout JSON event captured.',
          type: 'tool_use',
        },
        3,
      ),
    ).toEqual({
      artifactRefs: ['artifact_1'],
      externalEventId: 'runner:event:2',
      payload: {
        artifact_refs: ['artifact_1'],
        external_event_id: 'runner:event:2',
        sequence: 9,
        source: 'codex',
        summary: 'stdout JSON event captured.',
        type: 'tool_use',
      },
      sequence: 9,
      source: 'codex',
      summary: 'stdout JSON event captured.',
      type: 'tool_use',
    })
  })

  test('rejects non-record event payloads', () => {
    expect(normalizeOmniRunnerEventPayload(null, 1)).toBeUndefined()
    expect(normalizeOmniRunnerEventPayload([], 1)).toBeUndefined()
    expect(normalizeOmniRunnerEventPayload('event', 1)).toBeUndefined()
  })
})
