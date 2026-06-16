import { describe, expect, test } from 'vitest'

import {
  type ForgeSessionNavigationInput,
  projectForgeSessionNavigation,
} from './session-navigation'

const baseInput = (
  overrides: Partial<ForgeSessionNavigationInput> = {},
): ForgeSessionNavigationInput => ({
  generatedAt: '2026-06-16T17:00:00.000Z',
  workOrderRef: 'work_1',
  ...overrides,
})

describe('Forge session navigation projection', () => {
  test('joins Pylon, Codex, Claude, and bridge session summaries', () => {
    const view = projectForgeSessionNavigation(
      baseInput({
        bridgeSessions: [
          {
            bridgeRefs: ['bridge.public.work_1.remote'],
            observedAt: '2026-06-16T16:02:00.000Z',
            sessionRef: 'bridge.session.work_1',
            state: 'completed',
          },
        ],
        claudeSessions: [
          {
            artifactRefs: ['artifact.public.claude.summary'],
            observedAt: '2026-06-16T16:03:00.000Z',
            sessionRef: 'claude.session.work_1',
            state: 'failed',
          },
        ],
        codexSessions: [
          {
            checkpointRefs: ['checkpoint.public.codex.1'],
            eventRefs: ['event.public.codex.1'],
            observedAt: '2026-06-16T16:04:00.000Z',
            sessionRef: 'codex.session.work_1',
            state: 'queued',
          },
        ],
        localPylonSessions: [
          {
            artifactRefs: ['artifact.public.pylon.summary'],
            checkpointRefs: ['checkpoint.public.pylon.1'],
            eventRefs: ['event.public.pylon.1'],
            observedAt: '2026-06-16T16:05:00.000Z',
            sessionRef: 'pylon.session.work_1',
            state: 'running',
          },
        ],
      }),
    )

    expect(view.status).toBe('active')
    expect(view.blockerRefs).toEqual([])
    expect(view.items.map(item => item.source)).toEqual([
      'pylon',
      'codex',
      'claude',
      'bridge',
    ])
    expect(view.items[0]).toMatchObject({
      artifactRefs: ['artifact.public.pylon.summary'],
      checkpointRefs: ['checkpoint.public.pylon.1'],
      eventRefs: ['event.public.pylon.1'],
      sessionRef: 'pylon.session.work_1',
      state: 'running',
      title: 'Pylon session',
    })
  })

  test('keeps all session controls explicitly unavailable', () => {
    const view = projectForgeSessionNavigation(
      baseInput({
        localPylonSessions: [
          {
            checkpointRefs: ['checkpoint.public.pylon.1'],
            sessionRef: 'pylon.session.work_1',
            state: 'completed',
          },
        ],
      }),
    )
    const actions = view.items[0]?.actions

    expect(view.status).toBe('complete')
    expect(actions?.resume).toEqual({
      action: 'resume',
      availability: 'unavailable',
      blockerRefs: [
        'forge-session-navigation-blocker:pylon.session.work_1:resume-control-verb-unavailable',
      ],
    })
    expect(actions?.fork.availability).toBe('unavailable')
    expect(actions?.rewind.availability).toBe('unavailable')
    expect(actions?.cancel.availability).toBe('unavailable')
  })

  test('reports explicit blockers when no session summaries are available', () => {
    const view = projectForgeSessionNavigation(baseInput())

    expect(view).toMatchObject({
      blockerRefs: [
        'forge-session-navigation-blocker:work_1:no-session-summaries',
      ],
      items: [],
      status: 'empty',
    })
  })

  test('omits unsafe session refs and private material before projection', () => {
    const view = projectForgeSessionNavigation(
      baseInput({
        codexSessions: [
          {
            artifactRefs: [
              'artifact.public.codex.summary',
              'raw transcript /Users/christopher/private.jsonl',
            ],
            eventRefs: ['diff --git a/private.ts b/private.ts'],
            sessionRef: '/Users/christopher/.codex/session.jsonl',
            state: 'running',
          },
          {
            artifactRefs: ['artifact.public.codex.safe'],
            sessionRef: 'codex.session.safe',
            state: 'running',
          },
        ],
      }),
    )
    const payload = JSON.stringify(view)

    expect(view.status).toBe('active')
    expect(view.items.map(item => item.sessionRef)).toEqual([
      'codex.session.safe',
    ])
    expect(view.omittedUnsafeRefCount).toBe(3)
    expect(view.blockerRefs).toContain(
      'forge-session-navigation-blocker:work_1:unsafe-session-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('diff --git')
    expect(payload).not.toContain('raw transcript')
  })

  test('orders running, queued, attention, completed, then unknown sessions', () => {
    const view = projectForgeSessionNavigation(
      baseInput({
        bridgeSessions: [
          {
            observedAt: '2026-06-16T16:01:00.000Z',
            sessionRef: 'bridge.session.unknown',
          },
        ],
        claudeSessions: [
          {
            observedAt: '2026-06-16T16:04:00.000Z',
            sessionRef: 'claude.session.failed',
            state: 'failed',
          },
        ],
        codexSessions: [
          {
            observedAt: '2026-06-16T16:03:00.000Z',
            sessionRef: 'codex.session.queued',
            state: 'queued',
          },
          {
            observedAt: '2026-06-16T16:02:00.000Z',
            sessionRef: 'codex.session.completed',
            state: 'completed',
          },
        ],
        localPylonSessions: [
          {
            observedAt: '2026-06-16T16:05:00.000Z',
            sessionRef: 'pylon.session.running',
            state: 'running',
          },
        ],
      }),
    )

    expect(view.items.map(item => item.sessionRef)).toEqual([
      'pylon.session.running',
      'codex.session.queued',
      'claude.session.failed',
      'codex.session.completed',
      'bridge.session.unknown',
    ])
  })
})
