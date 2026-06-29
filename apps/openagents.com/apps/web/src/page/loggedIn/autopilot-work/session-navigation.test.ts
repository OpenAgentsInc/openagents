import { describe, expect, test } from 'vitest'

import {
  type ForgeSessionNavigationInput,
  projectForgeSessionNavigation,
  projectForgeSessionSummaryReceipt,
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
    expect(actions?.resume).toMatchObject({
      action: 'resume',
      authorityRefs: [],
      availability: 'unavailable',
      blockerRefs: [
        'forge-session-navigation-blocker:pylon.session.work_1:resume-control-verb-unavailable',
      ],
      policyRefs: [],
      requestRef: 'forge-session-control-request:pylon.session.work_1:resume',
    })
    expect(actions?.fork.availability).toBe('unavailable')
    expect(actions?.rewind.availability).toBe('unavailable')
    expect(actions?.cancel.availability).toBe('unavailable')
  })

  test('enables advertised session controls with authority and policy refs', () => {
    const view = projectForgeSessionNavigation(
      baseInput({
        localPylonSessions: [
          {
            controlAuthorityRefs: ['authority.public.session-control.bridge'],
            controlPolicyRefs: ['policy.public.resume-fork.allowed'],
            sessionRef: 'pylon.session.work_1',
            state: 'running',
            supportedControlActions: ['resume', 'fork'],
          },
        ],
      }),
    )
    const actions = view.items[0]?.actions

    expect(actions?.resume).toEqual({
      action: 'resume',
      authorityRefs: ['authority.public.session-control.bridge'],
      availability: 'available',
      blockerRefs: [],
      policyRefs: ['policy.public.resume-fork.allowed'],
      requestRef: 'forge-session-control-request:pylon.session.work_1:resume',
    })
    expect(actions?.fork.availability).toBe('available')
    expect(actions?.rewind.availability).toBe('unavailable')
    expect(actions?.cancel.availability).toBe('unavailable')
  })

  test('blocks supported controls without authority or policy refs', () => {
    const view = projectForgeSessionNavigation(
      baseInput({
        localPylonSessions: [
          {
            sessionRef: 'pylon.session.work_1',
            state: 'running',
            supportedControlActions: ['rewind'],
          },
        ],
      }),
    )
    const rewind = view.items[0]?.actions.rewind

    expect(rewind?.availability).toBe('blocked')
    expect(rewind?.blockerRefs).toEqual([
      'forge-session-navigation-blocker:pylon.session.work_1:rewind-missing-authority-ref',
      'forge-session-navigation-blocker:pylon.session.work_1:rewind-missing-policy-ref',
    ])
  })

  test('keeps stale supported controls disabled with a stale-session blocker', () => {
    const view = projectForgeSessionNavigation(
      baseInput({
        localPylonSessions: [
          {
            controlAuthorityRefs: ['authority.public.session-control.bridge'],
            controlFreshness: 'stale',
            controlPolicyRefs: ['policy.public.resume.allowed'],
            sessionRef: 'pylon.session.work_1',
            state: 'running',
            supportedControlActions: ['resume'],
          },
        ],
      }),
    )
    const resume = view.items[0]?.actions.resume

    expect(resume?.availability).toBe('stale')
    expect(resume?.blockerRefs).toEqual([
      'forge-session-navigation-blocker:pylon.session.work_1:resume-stale-session-ref',
    ])
  })

  test('projects public-safe session control receipts', () => {
    const view = projectForgeSessionNavigation(
      baseInput({
        controlReceipts: [
          {
            action: 'resume',
            actorRef: 'actor.public.raynor',
            generatedAt: '2026-06-16T17:06:00.000Z',
            outcome: 'queued',
            provenanceRefs: ['bridge.public.session-control'],
            publicSafe: true,
            receiptRef: 'receipt.public.resume.1',
            requestRef: 'forge-session-control-request:pylon.session.work_1:resume',
            sessionRef: 'pylon.session.work_1',
          },
        ],
        localPylonSessions: [
          {
            sessionRef: 'pylon.session.work_1',
            state: 'running',
          },
        ],
      }),
    )

    expect(view.controlReceipts).toEqual([
      {
        action: 'resume',
        actorRef: 'actor.public.raynor',
        blockerRefs: [],
        generatedAt: '2026-06-16T17:06:00.000Z',
        outcome: 'queued',
        provenanceRefs: ['bridge.public.session-control'],
        publicSafe: true,
        receiptRef: 'receipt.public.resume.1',
        requestRef: 'forge-session-control-request:pylon.session.work_1:resume',
        sessionRef: 'pylon.session.work_1',
      },
    ])
  })

  test('omits private material from session control receipts', () => {
    const view = projectForgeSessionNavigation(
      baseInput({
        controlReceipts: [
          {
            action: 'resume',
            actorRef: 'actor.public.safe',
            generatedAt: '2026-06-16T17:06:00.000Z',
            outcome: 'blocked',
            provenanceRefs: ['raw transcript /Users/christopher/private.jsonl'],
            publicSafe: true,
            receiptRef: 'receipt.public.resume.1',
            requestRef: 'forge-session-control-request:pylon.session.work_1:resume',
            sessionRef: 'pylon.session.work_1',
          },
          {
            action: 'fork',
            actorRef: 'actor.private.should-not-render',
            generatedAt: '2026-06-16T17:07:00.000Z',
            outcome: 'queued',
            publicSafe: false,
            receiptRef: 'receipt.private.fork.1',
            requestRef: 'forge-session-control-request:pylon.session.work_1:fork',
            sessionRef: 'pylon.session.work_1',
          },
        ],
        localPylonSessions: [
          {
            sessionRef: 'pylon.session.work_1',
            state: 'running',
          },
        ],
      }),
    )
    const payload = JSON.stringify(view)

    expect(view.controlReceipts).toHaveLength(1)
    expect(view.controlReceipts[0]?.provenanceRefs).toEqual([])
    expect(view.omittedUnsafeRefCount).toBe(2)
    expect(view.blockerRefs).toContain(
      'forge-session-navigation-blocker:work_1:unsafe-session-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw transcript')
    expect(payload).not.toContain('actor.private.should-not-render')
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

  test('projects a public-safe session summary receipt with counts and blockers', () => {
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
        codexSessions: [
          {
            artifactRefs: ['artifact.public.codex.summary'],
            eventRefs: ['event.public.codex.1'],
            observedAt: '2026-06-16T16:04:00.000Z',
            sessionRef: 'codex.session.work_1',
            state: 'queued',
          },
        ],
        localPylonSessions: [
          {
            checkpointRefs: ['checkpoint.public.pylon.1'],
            observedAt: '2026-06-16T16:05:00.000Z',
            sessionRef: 'pylon.session.work_1',
            state: 'running',
          },
        ],
      }),
    )
    const receipt = projectForgeSessionSummaryReceipt({
      exportedAt: '2026-06-16T17:05:00.000Z',
      view,
    })

    expect(receipt).toMatchObject({
      authority: {
        cancelControlAuthority: false,
        forkControlAuthority: false,
        resumeControlAuthority: false,
        rewindControlAuthority: false,
        runtimeExecutionProof: false,
      },
      countsBySource: {
        bridge: 1,
        claude: 0,
        codex: 1,
        pylon: 1,
      },
      countsByState: {
        cancelled: 0,
        completed: 1,
        failed: 0,
        queued: 1,
        running: 1,
        unknown: 0,
      },
      itemCount: 3,
      provenance: 'refs_only_session_summary',
      publicSafe: true,
      receiptKind: 'forge_session_summary_export.v1',
      status: 'active',
      workOrderRef: 'work_1',
    })
    expect(receipt.receiptRef).toBe(
      'forge.session_summary_export.work_1.2026-06-16T17_05_00.000Z',
    )
    expect(receipt.safeSessionRefs).toEqual([
      'pylon.session.work_1',
      'codex.session.work_1',
      'bridge.session.work_1',
    ])
    expect(receipt.safeEvidenceRefs).toEqual([
      'checkpoint.public.pylon.1',
      'artifact.public.codex.summary',
      'event.public.codex.1',
      'bridge.public.work_1.remote',
    ])
    expect(receipt.controlBlockerRefs).toContain(
      'forge-session-navigation-blocker:pylon.session.work_1:resume-control-verb-unavailable',
    )
  })

  test('omits unsafe material from exported session summary receipts', () => {
    const view = projectForgeSessionNavigation(
      baseInput({
        codexSessions: [
          {
            artifactRefs: [
              'artifact.public.codex.safe',
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
    const receipt = projectForgeSessionSummaryReceipt({
      exportedAt: '2026-06-16T17:05:00.000Z',
      view,
    })
    const payload = JSON.stringify(receipt)

    expect(receipt.safeSessionRefs).toEqual(['codex.session.safe'])
    expect(receipt.safeEvidenceRefs).toEqual(['artifact.public.codex.safe'])
    expect(receipt.omittedUnsafeRefCount).toBe(3)
    expect(receipt.blockerRefs).toContain(
      'forge-session-navigation-blocker:work_1:unsafe-session-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('diff --git')
    expect(payload).not.toContain('raw transcript')
  })

  test('falls back from unsafe titles before UI or receipt projection can cite them', () => {
    const view = projectForgeSessionNavigation(
      baseInput({
        localPylonSessions: [
          {
            sessionRef: 'pylon.session.work_1',
            state: 'running',
            title: 'raw transcript /Users/christopher/private.jsonl',
          },
        ],
      }),
    )
    const receipt = projectForgeSessionSummaryReceipt({
      exportedAt: '2026-06-16T17:05:00.000Z',
      view,
    })
    const payload = JSON.stringify({ receipt, view })

    expect(view.items[0]?.title).toBe('Pylon session')
    expect(view.omittedUnsafeRefCount).toBe(1)
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw transcript')
  })
})
