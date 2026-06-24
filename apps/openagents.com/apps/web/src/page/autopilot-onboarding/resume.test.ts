import { describe, expect, test } from 'vitest'

import {
  ClickedAutopilotOnboardingStartOver,
  ClosedAutopilotOnboardingResumeStream,
  FailedAutopilotOnboardingResume,
  FailedReconcileAutopilotOnboardingSession,
  LoadedStoredAutopilotOnboarding,
  ReceivedAutopilotOnboardingResumeReply,
  ReceivedAutopilotOnboardingStreamHandshake,
  SubmittedAutopilotOnboardingTurn,
  SucceededReconcileAutopilotOnboardingSession,
  SucceededResumeAutopilotOnboardingTurn,
  UpdatedAutopilotOnboardingComposer,
} from '../loggedOut/message'
import { type Model, init } from '../loggedOut/model'
import { initialCommands, update } from '../loggedOut/update'
import { AutopilotRoute } from '../../route'
import type { StoredOnboardingSession } from './persistence'
import type { OnboardingSessionResponse } from './persistence'

const flowOf = (model: Model) => model.autopilotOnboarding

const commandNames = (commands: ReadonlyArray<{ name: string }>) =>
  commands.map(command => command.name)

const storedSession = (
  overrides: Partial<StoredOnboardingSession> = {},
): StoredOnboardingSession => ({
  sessionId: 'ob_resume_1',
  vertical: null,
  status: 'interviewing',
  transcript: [
    { role: 'user', content: 'I run a law office' },
    { role: 'assistant', content: 'Got it — what do you need first?' },
  ],
  outputSpec: { business: 'Law office' },
  inFlight: null,
  updatedAt: 1_700_000_000_000,
  ...overrides,
})

const sessionResponse = (
  overrides: Partial<OnboardingSessionResponse> = {},
): OnboardingSessionResponse => ({
  sessionId: 'ob_resume_1',
  status: 'interviewing',
  turnCount: 2,
  transcript: [
    { role: 'user', content: 'I run a law office' },
    { role: 'assistant', content: 'Got it — what do you need first?' },
  ],
  outputSpec: { business: 'Law office' },
  ...overrides,
})

describe('autopilot onboarding — mount rehydration', () => {
  test('initialCommands rehydrates on the /autopilot route', () => {
    const model = init(AutopilotRoute())
    expect(commandNames(initialCommands(model))).toEqual([
      'RehydrateAutopilotOnboarding',
    ])
  })

  test('restoring a saved session shows the transcript immediately and reconciles', () => {
    const base = init(AutopilotRoute())
    const [restored, commands] = update(
      base,
      LoadedStoredAutopilotOnboarding({ session: storedSession() }),
    )

    const flow = flowOf(restored)
    expect(flow.sessionId).toBe('ob_resume_1')
    expect(flow.transcript).toEqual([
      { role: 'user', content: 'I run a law office' },
      { role: 'assistant', content: 'Got it — what do you need first?' },
    ])
    expect(flow.outputSpec).toEqual({ business: 'Law office' })
    expect(flow.status).toBe('idle')
    // No resume when nothing was mid-stream.
    expect(flow.inFlight).toBeNull()
    expect(commandNames(commands)).toContain(
      'ReconcileAutopilotOnboardingSession',
    )
  })

  test('restoring an in-flight session primes the resume bubble + cursor', () => {
    const base = init(AutopilotRoute())
    const [restored] = update(
      base,
      LoadedStoredAutopilotOnboarding({
        session: storedSession({
          inFlight: {
            streamId: 'onboarding:ob_resume_1:2',
            turnIndex: 2,
            replySoFar: 'Here is the plan so far',
            lastOffset: '64',
          },
        }),
      }),
    )

    const flow = flowOf(restored)
    expect(flow.status).toBe('streaming')
    expect(flow.streamingReply).toBe('Here is the plan so far')
    // The restored cursor carries `resuming: true` so the resume subscription
    // reopens the durable read for this turn at the saved offset.
    expect(flow.inFlight).toEqual({
      streamId: 'onboarding:ob_resume_1:2',
      turnIndex: 2,
      lastOffset: '64',
      resuming: true,
    })
  })
})

describe('autopilot onboarding — reconcile with the server', () => {
  test('a 200 reconcile adopts the authoritative server transcript', () => {
    const base = init(AutopilotRoute())
    const [restored] = update(
      base,
      LoadedStoredAutopilotOnboarding({
        session: storedSession({
          transcript: [{ role: 'user', content: 'I run a law office' }],
        }),
      }),
    )

    const [reconciled, commands] = update(
      restored,
      SucceededReconcileAutopilotOnboardingSession({
        response: sessionResponse({
          // The server has the completed assistant reply the tab missed.
          transcript: [
            { role: 'user', content: 'I run a law office' },
            { role: 'assistant', content: 'Welcome back — here is the plan.' },
          ],
          turnCount: 2,
        }),
      }),
    )

    const flow = flowOf(reconciled)
    expect(flow.transcript).toEqual([
      { role: 'user', content: 'I run a law office' },
      { role: 'assistant', content: 'Welcome back — here is the plan.' },
    ])
    expect(flow.turnCount).toBe(2)
    expect(commandNames(commands)).toContain('PersistAutopilotOnboarding')
  })

  test('a 404 reconcile clears storage and starts fresh', () => {
    const base = init(AutopilotRoute())
    const [restored] = update(
      base,
      LoadedStoredAutopilotOnboarding({ session: storedSession() }),
    )

    const [cleared, commands] = update(
      restored,
      FailedReconcileAutopilotOnboardingSession({ status: 404 }),
    )

    const flow = flowOf(cleared)
    expect(flow.sessionId).toBeNull()
    expect(flow.transcript).toEqual([])
    expect(flow.outputSpec).toEqual({})
    expect(commandNames(commands)).toEqual(['ClearAutopilotOnboardingStorage'])
  })

  test('a transient (status 0) reconcile keeps the local transcript', () => {
    const base = init(AutopilotRoute())
    const [restored] = update(
      base,
      LoadedStoredAutopilotOnboarding({ session: storedSession() }),
    )

    const [kept, commands] = update(
      restored,
      FailedReconcileAutopilotOnboardingSession({ status: 0 }),
    )

    expect(flowOf(kept).transcript.length).toBe(2)
    expect(commandNames(commands)).toEqual([])
  })

  test('a reconcile during an active resume does not clobber the streaming bubble', () => {
    const base = init(AutopilotRoute())
    const [restored] = update(
      base,
      LoadedStoredAutopilotOnboarding({
        session: storedSession({
          inFlight: {
            streamId: 'onboarding:ob_resume_1:2',
            turnIndex: 2,
            replySoFar: 'partial',
            lastOffset: null,
          },
        }),
      }),
    )

    const [reconciled] = update(
      restored,
      SucceededReconcileAutopilotOnboardingSession({
        response: sessionResponse(),
      }),
    )

    const flow = flowOf(reconciled)
    // The resume cursor + streaming bubble survive the reconcile.
    expect(flow.inFlight).toMatchObject({ turnIndex: 2, resuming: true })
    expect(flow.status).toBe('streaming')
    expect(flow.streamingReply).toBe('partial')
  })
})

describe('autopilot onboarding — durable stream resume', () => {
  const restoredInFlight = (): Model => {
    const base = init(AutopilotRoute())
    const [restored] = update(
      base,
      LoadedStoredAutopilotOnboarding({
        session: storedSession({
          transcript: [{ role: 'user', content: 'I run a law office' }],
          inFlight: {
            streamId: 'onboarding:ob_resume_1:1',
            turnIndex: 1,
            replySoFar: '',
            lastOffset: null,
          },
        }),
      }),
    )
    return restored
  }

  test('the resume subscription is active for a restored in-flight turn', () => {
    // The dependency selector drives the resume read; assert it activates.
    const restored = restoredInFlight()
    const flow = flowOf(restored)
    expect(flow.inFlight).toMatchObject({ turnIndex: 1, resuming: true })
    expect(flow.sessionId).toBe('ob_resume_1')
  })

  test('resume deltas REPLACE the bubble and advance the persisted offset', () => {
    const restored = restoredInFlight()
    const [step1] = update(
      restored,
      ReceivedAutopilotOnboardingResumeReply({
        turnIndex: 1,
        reply: 'Here is',
        nextOffset: '32',
      }),
    )
    const [step2, commands] = update(
      step1,
      ReceivedAutopilotOnboardingResumeReply({
        turnIndex: 1,
        reply: 'Here is the full plan',
        nextOffset: '96',
      }),
    )

    const flow = flowOf(step2)
    // REPLACE, not append — the re-replayed prefix never doubles up.
    expect(flow.streamingReply).toBe('Here is the full plan')
    // The advanced offset lives on the single in-flight cursor.
    expect(flow.inFlight).toMatchObject({
      turnIndex: 1,
      lastOffset: '96',
      resuming: true,
    })
    expect(commandNames(commands)).toContain('PersistAutopilotOnboarding')
  })

  test('resume completion commits the assistant reply and clears resume state', () => {
    const restored = restoredInFlight()
    const [streamed] = update(
      restored,
      ReceivedAutopilotOnboardingResumeReply({
        turnIndex: 1,
        reply: 'Here is the full plan',
        nextOffset: '96',
      }),
    )
    const [done, commands] = update(
      streamed,
      SucceededResumeAutopilotOnboardingTurn({
        response: {
          sessionId: 'ob_resume_1',
          reply: 'Here is the full plan',
          status: 'interviewing',
          turnCount: 1,
          outputSpec: { business: 'Law office' },
        },
      }),
    )

    const flow = flowOf(done)
    expect(flow.status).toBe('idle')
    expect(flow.streamingReply).toBeNull()
    expect(flow.inFlight).toBeNull()
    expect(flow.transcript).toEqual([
      { role: 'user', content: 'I run a law office' },
      { role: 'assistant', content: 'Here is the full plan' },
    ])
    expect(commandNames(commands)).toContain('PersistAutopilotOnboarding')
  })

  test('a 404 resume read falls back without a stuck half-bubble', () => {
    const restored = restoredInFlight()
    const [failed] = update(
      restored,
      FailedAutopilotOnboardingResume({ turnIndex: 1 }),
    )

    const flow = flowOf(failed)
    expect(flow.status).toBe('idle')
    expect(flow.streamingReply).toBeNull()
    expect(flow.inFlight).toBeNull()
  })

  test('a resume stream that ends without done falls back cleanly', () => {
    const restored = restoredInFlight()
    const [closed] = update(
      restored,
      ClosedAutopilotOnboardingResumeStream({ turnIndex: 1 }),
    )

    const flow = flowOf(closed)
    expect(flow.status).toBe('idle')
    expect(flow.streamingReply).toBeNull()
    expect(flow.inFlight).toBeNull()
  })
})

describe('autopilot onboarding — handshake + start over', () => {
  test('the live handshake sets the in-flight cursor and persists', () => {
    const base = init(AutopilotRoute())
    const [typed] = update(
      base,
      UpdatedAutopilotOnboardingComposer({ value: 'Help me draft an NDA' }),
    )
    const [submitted] = update(typed, SubmittedAutopilotOnboardingTurn())
    const pendingId = flowOf(submitted).pendingTurn?.id ?? ''

    const [handshook, commands] = update(
      submitted,
      ReceivedAutopilotOnboardingStreamHandshake({
        turnId: pendingId,
        streamId: 'onboarding:ob_minted:0',
        sessionId: 'ob_minted',
        turnIndex: 0,
      }),
    )

    const flow = flowOf(handshook)
    // The handshake adopts the server-minted session id so a first-turn
    // mid-stream reload can resume.
    expect(flow.sessionId).toBe('ob_minted')
    // A live (non-resume) handshake sets the cursor with `resuming: false`.
    expect(flow.inFlight).toEqual({
      streamId: 'onboarding:ob_minted:0',
      turnIndex: 0,
      lastOffset: null,
      resuming: false,
    })
    expect(commandNames(commands)).toContain('PersistAutopilotOnboarding')
  })

  test('start over resets the flow and clears storage', () => {
    const base = init(AutopilotRoute())
    const [restored] = update(
      base,
      LoadedStoredAutopilotOnboarding({ session: storedSession() }),
    )

    const [fresh, commands] = update(
      restored,
      ClickedAutopilotOnboardingStartOver(),
    )

    const flow = flowOf(fresh)
    expect(flow.sessionId).toBeNull()
    expect(flow.transcript).toEqual([])
    expect(flow.composerDraft).toBe('')
    expect(commandNames(commands)).toEqual(['ClearAutopilotOnboardingStorage'])
  })
})
