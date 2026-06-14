import { Option } from 'effect'
import { describe, expect, test } from 'vitest'

import { Demo2Route, DemoOrderRoute, DemoRoute } from '../../route'
import {
  AdvancedDemoCue,
  ClickedNextDemoStep,
  ClickedPreviousDemoStep,
  PressedDemoSpacebar,
  SelectedTrainingSceneNode,
  TickedDemoPlayback,
} from './message'
import { type DemoCueName, init } from './model'
import { update } from './update'

const sendCue = (model: ReturnType<typeof init>, name: DemoCueName) =>
  update(
    model,
    AdvancedDemoCue({
      cue: { name, atMs: 0 },
    }),
  )

describe('demo update', () => {
  test('applies playback cues through the nested logged-in model without commands', () => {
    const [loaded, loadedCommands] = sendCue(
      init(Demo2Route()),
      'LoadedProjectRoom',
    )
    const [filled, filledCommands] = sendCue(loaded, 'FilledComposer')

    expect(loadedCommands).toHaveLength(0)
    expect(filled.loggedIn.chatComposerValue).toBe(
      '@autopilot prepare the Pylon release briefing from the attached plan',
    )
    expect(filledCommands).toHaveLength(0)

    const [submitted, submittedCommands] = sendCue(filled, 'SubmittedPrompt')

    expect(submitted.loggedIn.chatComposerValue).toBe('')
    expect(submitted.loggedIn.chatRun._tag).toBe('Active')
    expect(submittedCommands).toHaveLength(0)

    const [thread, threadCommands] = sendCue(submitted, 'OpenedThread')

    expect(thread.loggedIn.route).toEqual({
      _tag: 'Thread',
      threadId: 'pylon-release-demo',
    })
    expect(threadCommands).toHaveLength(0)

    const [files, filesCommands] = sendCue(thread, 'OpenedFileDetail')

    expect(files.loggedIn.route).toEqual({
      _tag: 'TeamFile',
      teamRef: 'openagents-core-team',
      fileId: 'file_pylon_release_plan',
    })
    expect(
      files.loggedIn.threadFileDetailsById.file_pylon_release_plan?.references,
    ).toHaveLength(2)
    expect(filesCommands).toHaveLength(0)
  })

  test('applies customer order cues through the nested logged-in model', () => {
    const [loaded] = sendCue(init(DemoOrderRoute()), 'LoadedOrderRepositories')
    const [selected] = sendCue(loaded, 'SelectedOrderRepository')
    const [filled] = sendCue(selected, 'FilledOrderGoal')
    const [submitted] = sendCue(filled, 'SubmittedOrderGoal')
    const [confirmed] = sendCue(submitted, 'ConfirmedPublicWork')
    const [scoping] = sendCue(confirmed, 'AdvancedOrderScoping')
    const [queued] = sendCue(scoping, 'AdvancedOrderQueued')
    const [running, commands] = sendCue(queued, 'AdvancedOrderRunning')

    expect(loaded.mode).toBe('order')
    expect(loaded.loggedIn.onboarding.repositories._tag).toBe(
      'OnboardingRepositoriesLoaded',
    )
    expect(selected.loggedIn.auth.onboarding.step).toBe('goal')
    expect(filled.loggedIn.onboarding.goalValue).toContain(
      'Stripe credits checkout',
    )
    expect(submitted.loggedIn.auth.onboarding.step).toBe('billing')
    expect(confirmed.loggedIn.route).toEqual({ _tag: 'Order' })
    expect(running.loggedIn.customerOrder).toMatchObject({
      _tag: 'CustomerOrderLoaded',
      order: {
        id: 'software_order_beta_shopify_checkout',
        status: 'agent_running',
        visibility: 'public',
        providerAccountRequired: false,
      },
    })
    expect(commands).toHaveLength(0)
  })

  test('toggles playback with the spacebar message', () => {
    const [paused, pauseCommands] = update(
      init(DemoOrderRoute()),
      PressedDemoSpacebar(),
    )
    const [resumed, resumeCommands] = update(paused, PressedDemoSpacebar())

    expect(paused.playback).toBe('paused')
    expect(resumed.playback).toBe('playing')
    expect(pauseCommands).toHaveLength(0)
    expect(resumeCommands).toHaveLength(0)
  })

  test('selects training scene nodes without activating playback', () => {
    const [selected, commands] = update(
      init(DemoRoute()),
      SelectedTrainingSceneNode({ nodeId: 'freivalds' }),
    )
    const [spacebar] = update(selected, PressedDemoSpacebar())
    const [tick] = update(spacebar, TickedDemoPlayback({ deltaMs: 100 }))

    expect(selected.mode).toBe('training')
    expect(selected.playback).toBe('complete')
    expect(selected.maybeSelectedTrainingSceneNodeId).toEqual(
      Option.some('freivalds'),
    )
    expect(tick.elapsedMs).toBe(0)
    expect(tick.playback).toBe('complete')
    expect(commands).toHaveLength(0)
  })

  test('ticks visible playback time independently from cue timestamps', () => {
    const [tick1, tick1Commands] = update(
      init(DemoOrderRoute()),
      TickedDemoPlayback({ deltaMs: 100 }),
    )
    const [tick2, tick2Commands] = update(
      tick1,
      TickedDemoPlayback({ deltaMs: 100 }),
    )
    const [paused] = update(tick2, PressedDemoSpacebar())
    const [stillPaused] = update(paused, TickedDemoPlayback({ deltaMs: 100 }))

    expect(tick1.elapsedMs).toBe(100)
    expect(tick2.elapsedMs).toBe(200)
    expect(stillPaused.elapsedMs).toBe(200)
    expect(tick1Commands).toHaveLength(0)
    expect(tick2Commands).toHaveLength(0)
  })

  test('resumes after the first zero-millisecond cue without rewinding', () => {
    const [started] = sendCue(init(DemoOrderRoute()), 'LoadedOrderRepositories')
    const [paused] = update(started, PressedDemoSpacebar())
    const [resumed, commands] = update(paused, PressedDemoSpacebar())

    expect(started.cueIndex).toBe(0)
    expect(paused.cueIndex).toBe(0)
    expect(resumed.cueIndex).toBe(0)
    expect(resumed.playback).toBe('playing')
    expect(commands).toHaveLength(0)
  })

  test('navigates customer order demo steps manually', () => {
    const [loaded, loadedCommands] = update(
      init(DemoOrderRoute()),
      ClickedNextDemoStep(),
    )
    const [selected, selectedCommands] = update(loaded, ClickedNextDemoStep())
    const [back, backCommands] = update(selected, ClickedPreviousDemoStep())

    expect(loaded.cueIndex).toBe(0)
    expect(loaded.elapsedMs).toBe(0)
    expect(loaded.playback).toBe('paused')
    expect(selected.cueIndex).toBe(1800)
    expect(selected.loggedIn.auth.onboarding.step).toBe('goal')
    expect(back.cueIndex).toBe(0)
    expect(back.elapsedMs).toBe(0)
    expect(back.loggedIn.auth.onboarding.step).toBe('repository')
    expect(loadedCommands).toHaveLength(0)
    expect(selectedCommands).toHaveLength(0)
    expect(backCommands).toHaveLength(0)
  })
})
