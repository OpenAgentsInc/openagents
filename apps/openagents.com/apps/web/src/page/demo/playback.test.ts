import { describe, expect, test } from 'vitest'

import { demoCues, demoOrderCues, remainingDemoCues } from './playback'

describe('demo playback schedule', () => {
  test('covers the deterministic 15 second journey in order', () => {
    expect(demoCues.map(cue => cue.name)).toEqual([
      'LoadedProjectRoom',
      'FilledComposer',
      'SubmittedPrompt',
      'ReceivedRunEvents',
      'LoadedRunContext',
      'OpenedThread',
      'CompletedRun',
      'ReturnedToProjectRoom',
      'OpenedTeamFiles',
      'OpenedFileDetail',
      'CompletedPlayback',
    ])
    expect(demoCues[0]?.atMs).toBe(0)
    expect(demoCues.at(-1)?.atMs).toBe(15000)
  })

  test('covers the deterministic customer order journey in order', () => {
    expect(demoOrderCues.map(cue => cue.name)).toEqual([
      'LoadedOrderRepositories',
      'SelectedOrderRepository',
      'FilledOrderGoal',
      'SubmittedOrderGoal',
      'ConfirmedPublicWork',
      'LoadedSubmittedOrder',
      'AdvancedOrderScoping',
      'AdvancedOrderQueued',
      'AdvancedOrderRunning',
      'CompletedPlayback',
    ])
    expect(demoOrderCues[0]?.atMs).toBe(0)
    expect(demoOrderCues.at(-1)?.atMs).toBe(15000)
  })

  test('resumes customer order playback after the current cue', () => {
    expect(
      remainingDemoCues('demo:customer-order', 1800).map(cue => cue.name),
    ).toEqual([
      'FilledOrderGoal',
      'SubmittedOrderGoal',
      'ConfirmedPublicWork',
      'LoadedSubmittedOrder',
      'AdvancedOrderScoping',
      'AdvancedOrderQueued',
      'AdvancedOrderRunning',
      'CompletedPlayback',
    ])
  })
})
