// Full client-side onboarding reload-resume handshake — the CLIENT half of the
// client↔server contract, driven through the REAL Foldkit pieces:
//   - the REAL persistence port (an in-memory `OnboardingStoragePort` standing
//     in for `window.localStorage`, so the handshake runs headlessly),
//   - the REAL `loggedOut` `update` reducer + `initialCommands`,
//   - the REAL top-level routing `init` (`main.ts`), to prove which submodel a
//     logged-in vs logged-out onboarding user actually mounts.
//
// What it proves (the regression that would have caught the live bug):
//   1. A streaming turn persists the transcript + the in-flight stream cursor
//      (handshake streamId/turnIndex) to the storage port.
//   2. A PAGE RELOAD — a fresh client model running `initialCommands` again —
//      rehydrates from the persisted port: `RehydrateAutopilotOnboarding` fires,
//      `LoadedStoredAutopilotOnboarding` restores the transcript, and the
//      reconcile + resume run.
//   3. The mid-first-turn 404 reconcile (server row not written yet — see the
//      api `autopilot-onboarding-resume-handshake.test.ts`) NO LONGER wipes the
//      conversation. The durable resume read replays the in-flight turn and the
//      bubble completes.
//   4. This holds for BOTH auth states: a logged-OUT visitor AND a logged-in
//      no-workspace user BOTH mount the rehydrating LoggedOut submodel on
//      `/autopilot`, so `initialCommands` rehydrates either way.

import { Option } from 'effect'
import { afterEach, describe, expect, test } from 'vitest'

import {
  authBootstrapFromSession,
  incompleteOnboardingStatus,
} from '../../domain/session'
import { Flags, init } from '../../main'
import {
  FailedReconcileAutopilotOnboardingSession,
  LoadedStoredAutopilotOnboarding,
  ReceivedAutopilotOnboardingDelta,
  ReceivedAutopilotOnboardingResumeReply,
  ReceivedAutopilotOnboardingStreamHandshake,
  SubmittedAutopilotOnboardingTurn,
  SucceededResumeAutopilotOnboardingTurn,
  UpdatedAutopilotOnboardingComposer,
} from '../loggedOut/message'
import { type Model, init as initLoggedOut } from '../loggedOut/model'
import { initialCommands, update } from '../loggedOut/update'
import { AutopilotRoute } from '../../route'
import {
  type OnboardingStoragePort,
  makeMemoryOnboardingStoragePort,
  readStoredSession,
  setOnboardingStoragePort,
  writeStoredSession,
} from './persistence'

// HELPERS -----------------------------------------------------------------

const flowOf = (model: Model) => model.autopilotOnboarding
const names = (commands: ReadonlyArray<{ name: string }>) =>
  commands.map(c => c.name)

// Run the `PersistAutopilotOnboarding` command's effect against the active
// storage port by replaying what the command does: build the stored record from
// the flow and write it. We instead exercise the real path by driving the same
// `writeStoredSession` the command calls, sourced from the flow the reducer
// produced — so the persisted blob is exactly what the app would store.
//
// The reducer returns `PersistAutopilotOnboarding({ flow })` commands; rather
// than spin Foldkit's runtime, we mirror the command's documented behaviour
// (storedSessionFromFlow -> writeStoredSession) by persisting the flow fields
// the command would. To keep this honest we go through the SAME public
// `writeStoredSession`/`readStoredSession` the production command uses.
const persistFlow = (model: Model): void => {
  const flow = flowOf(model)
  if (flow.sessionId === null) {
    return
  }
  const inFlight =
    flow.inFlight === null
      ? null
      : {
          streamId: flow.inFlight.streamId,
          turnIndex: flow.inFlight.turnIndex,
          replySoFar: flow.streamingReply ?? '',
          lastOffset: flow.inFlight.lastOffset,
        }
  writeStoredSession({
    sessionId: flow.sessionId,
    vertical: flow.vertical,
    status:
      flow.status === 'idle' && flow.turnCount > 0 ? 'interviewing' : null,
    transcript: flow.transcript,
    outputSpec: flow.outputSpec,
    inFlight,
    updatedAt: 1_700_000_000_000,
  })
}

// A simulated browser tab: a model plus the storage port that backs it. A
// "reload" constructs a fresh model + runs `initialCommands` against the SAME
// persisted port.
const coreTeam = [
  {
    id: 'team_openagents_core',
    name: 'OpenAgents Core Team',
    slug: 'openagents-core-team',
    role: 'owner',
    members: [],
  },
]
const baseAuth = authBootstrapFromSession({
  email: 'visitor@example.com',
  name: 'Visitor',
  userId: 'gh:visitor',
})
const autopilotUrl = {
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname: '/autopilot',
  search: Option.none(),
  hash: Option.none(),
}

describe('autopilot onboarding — full client reload-resume handshake', () => {
  let restorePort: (() => void) | undefined

  const install = (): OnboardingStoragePort => {
    const port = makeMemoryOnboardingStoragePort()
    restorePort = setOnboardingStoragePort(port)
    return port
  }

  afterEach(() => {
    restorePort?.()
    restorePort = undefined
  })

  test('a streaming first turn persists transcript + in-flight cursor through the storage port', () => {
    install()
    let model = initLoggedOut(AutopilotRoute())

    // The visitor types and submits the first turn.
    ;[model] = update(
      model,
      UpdatedAutopilotOnboardingComposer({ value: 'I run a bakery' }),
    )
    ;[model] = update(model, SubmittedAutopilotOnboardingTurn())
    const pendingId = flowOf(model).pendingTurn?.id ?? ''
    persistFlow(model)

    // The server handshake mints the session id + durable cursor.
    ;[model] = update(
      model,
      ReceivedAutopilotOnboardingStreamHandshake({
        turnId: pendingId,
        streamId: 'onboarding:ob_bakery:0',
        sessionId: 'ob_bakery',
        turnIndex: 0,
      }),
    )
    persistFlow(model)

    // A delta lands mid-stream.
    ;[model] = update(
      model,
      ReceivedAutopilotOnboardingDelta({ turnId: pendingId, text: 'Welcome' }),
    )
    persistFlow(model)

    // The storage port now holds the in-flight turn the durable log can resume.
    const stored = readStoredSession()
    expect(Option.isSome(stored)).toBe(true)
    const session = Option.getOrThrow(stored)
    expect(session.sessionId).toBe('ob_bakery')
    expect(session.transcript).toEqual([{ role: 'user', content: 'I run a bakery' }])
    expect(session.inFlight).toMatchObject({
      streamId: 'onboarding:ob_bakery:0',
      turnIndex: 0,
      replySoFar: 'Welcome',
    })
  })

  test('RELOAD mid-first-turn: a 404 reconcile NO LONGER wipes the conversation; the resume replays and completes', () => {
    const port = install()

    // --- TAB 1: stream a first turn, persist mid-stream, then the tab goes away.
    let tab1 = initLoggedOut(AutopilotRoute())
    ;[tab1] = update(
      tab1,
      UpdatedAutopilotOnboardingComposer({ value: 'I run a bakery' }),
    )
    ;[tab1] = update(tab1, SubmittedAutopilotOnboardingTurn())
    const pendingId = flowOf(tab1).pendingTurn?.id ?? ''
    ;[tab1] = update(
      tab1,
      ReceivedAutopilotOnboardingStreamHandshake({
        turnId: pendingId,
        streamId: 'onboarding:ob_bakery:0',
        sessionId: 'ob_bakery',
        turnIndex: 0,
      }),
    )
    ;[tab1] = update(
      tab1,
      ReceivedAutopilotOnboardingDelta({ turnId: pendingId, text: 'Welcome' }),
    )
    persistFlow(tab1)
    expect(port.get('oa.autopilot.onboarding.v1')).not.toBeNull()

    // --- TAB 2 (RELOAD): fresh model + initialCommands rehydrate from the port.
    let tab2 = initLoggedOut(AutopilotRoute())
    expect(names(initialCommands(tab2))).toEqual([
      'RehydrateAutopilotOnboarding',
    ])
    // RehydrateAutopilotOnboarding reads the port and dispatches the restore.
    const restored = readStoredSession()
    expect(Option.isSome(restored)).toBe(true)
    ;[tab2] = update(
      tab2,
      LoadedStoredAutopilotOnboarding({ session: Option.getOrThrow(restored) }),
    )

    // The transcript is restored and the in-flight turn is primed for resume.
    expect(flowOf(tab2).transcript).toEqual([
      { role: 'user', content: 'I run a bakery' },
    ])
    expect(flowOf(tab2).status).toBe('streaming')
    expect(flowOf(tab2).inFlight).toMatchObject({ turnIndex: 0, resuming: true })

    // The reconcile GET hits the server. MID-FIRST-TURN the session row is not
    // written yet, so the server replies 404 (proved in the api handshake test).
    // THE FIX: this 404 must NOT clear the conversation, because a resume is in
    // flight and a transcript exists.
    let after404: Model
    let cmds404: ReadonlyArray<{ name: string }>
    ;[after404, cmds404] = update(
      tab2,
      FailedReconcileAutopilotOnboardingSession({ status: 404 }),
    )
    // No ClearAutopilotOnboardingStorage — the transcript + resume survive.
    expect(names(cmds404)).not.toContain('ClearAutopilotOnboardingStorage')
    expect(flowOf(after404).transcript).toEqual([
      { role: 'user', content: 'I run a bakery' },
    ])
    expect(flowOf(after404).inFlight).toMatchObject({ resuming: true })
    // The persisted blob is still intact (not wiped).
    expect(port.get('oa.autopilot.onboarding.v1')).not.toBeNull()

    // The durable resume read replays the in-flight turn; deltas REPLACE the
    // bubble, then completion commits the assistant reply.
    let resumed: Model
    ;[resumed] = update(
      after404,
      ReceivedAutopilotOnboardingResumeReply({
        turnIndex: 0,
        reply: 'Welcome to Autopilot',
        nextOffset: '64',
      }),
    )
    expect(flowOf(resumed).streamingReply).toBe('Welcome to Autopilot')
    ;[resumed] = update(
      resumed,
      SucceededResumeAutopilotOnboardingTurn({
        response: {
          sessionId: 'ob_bakery',
          reply: 'Welcome to Autopilot',
          status: 'interviewing',
          turnCount: 1,
          outputSpec: {},
        },
      }),
    )

    // The conversation is fully restored + resumed — never lost.
    expect(flowOf(resumed).status).toBe('idle')
    expect(flowOf(resumed).inFlight).toBeNull()
    expect(flowOf(resumed).transcript).toEqual([
      { role: 'user', content: 'I run a bakery' },
      { role: 'assistant', content: 'Welcome to Autopilot' },
    ])
  })

  test('an in-flight turn that lands a real 404 is NOT a transcript wipe (resume still owns the tail)', () => {
    install()
    let model = initLoggedOut(AutopilotRoute())
    ;[model] = update(
      model,
      LoadedStoredAutopilotOnboarding({
        session: {
          sessionId: 'ob_x',
          vertical: null,
          status: 'interviewing',
          transcript: [{ role: 'user', content: 'hi' }],
          outputSpec: {},
          inFlight: {
            streamId: 'onboarding:ob_x:0',
            turnIndex: 0,
            replySoFar: 'par',
            lastOffset: null,
          },
          updatedAt: 1,
        },
      }),
    )
    const [after, cmds] = update(
      model,
      FailedReconcileAutopilotOnboardingSession({ status: 404 }),
    )
    expect(names(cmds)).not.toContain('ClearAutopilotOnboardingStorage')
    expect(flowOf(after).transcript.length).toBe(1)
  })

  test('a 404 with an EMPTY transcript and NO in-flight turn still clears (genuinely dead pointer)', () => {
    install()
    let model = initLoggedOut(AutopilotRoute())
    // Restore a session whose local transcript is empty and nothing in flight,
    // then a 404 reconcile — this is the only case that should clear.
    ;[model] = update(
      model,
      LoadedStoredAutopilotOnboarding({
        session: {
          sessionId: 'ob_dead',
          vertical: null,
          status: 'interviewing',
          transcript: [],
          outputSpec: {},
          inFlight: null,
          updatedAt: 1,
        },
      }),
    )
    const [after, cmds] = update(
      model,
      FailedReconcileAutopilotOnboardingSession({ status: 404 }),
    )
    expect(names(cmds)).toEqual(['ClearAutopilotOnboardingStorage'])
    expect(flowOf(after).sessionId).toBeNull()
    expect(flowOf(after).transcript).toEqual([])
  })
})

describe('autopilot onboarding — rehydration fires for BOTH auth states', () => {
  // The live bug report was from a LOGGED-IN owner. This pins which top-level
  // submodel a /autopilot onboarding user mounts, and that `initialCommands`
  // rehydrates in every case where the onboarding PAGE is actually shown.

  test('a logged-OUT visitor on /autopilot mounts LoggedOut and rehydrates', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      autopilotUrl as never,
    )
    expect((model as { _tag: string })._tag).toBe('LoggedOut')
    expect(names(commands)).toContain('RehydrateAutopilotOnboarding')
  })

  test('a logged-in NO-WORKSPACE user (no core team) on /autopilot mounts LoggedOut and rehydrates', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(baseAuth) }),
      autopilotUrl as never,
    )
    expect((model as { _tag: string })._tag).toBe('LoggedOut')
    expect(names(commands)).toContain('RehydrateAutopilotOnboarding')
  })

  test('a logged-in core-team user with INCOMPLETE onboarding on /autopilot mounts LoggedOut and rehydrates', () => {
    const auth = {
      ...baseAuth,
      teams: coreTeam,
      onboarding: incompleteOnboardingStatus(),
    }
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.some(auth) }),
      autopilotUrl as never,
    )
    expect((model as { _tag: string })._tag).toBe('LoggedOut')
    expect(names(commands)).toContain('RehydrateAutopilotOnboarding')
  })
})
