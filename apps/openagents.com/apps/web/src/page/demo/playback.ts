import { Context, Duration, Effect, Layer, Schedule, Stream } from 'effect'

import { AdvancedDemoCue, type Message } from './message'
import { DemoCue } from './model'

export const demoCues: ReadonlyArray<DemoCue> = [
  DemoCue.make({ name: 'LoadedProjectRoom', atMs: 0 }),
  DemoCue.make({ name: 'FilledComposer', atMs: 1000 }),
  DemoCue.make({ name: 'SubmittedPrompt', atMs: 1800 }),
  DemoCue.make({ name: 'ReceivedRunEvents', atMs: 3000 }),
  DemoCue.make({ name: 'LoadedRunContext', atMs: 5000 }),
  DemoCue.make({ name: 'OpenedThread', atMs: 6500 }),
  DemoCue.make({ name: 'CompletedRun', atMs: 8500 }),
  DemoCue.make({ name: 'ReturnedToProjectRoom', atMs: 10500 }),
  DemoCue.make({ name: 'OpenedTeamFiles', atMs: 12000 }),
  DemoCue.make({ name: 'OpenedFileDetail', atMs: 13500 }),
  DemoCue.make({ name: 'CompletedPlayback', atMs: 15000 }),
]

export const demoOrderCues: ReadonlyArray<DemoCue> = [
  DemoCue.make({ name: 'LoadedOrderRepositories', atMs: 0 }),
  DemoCue.make({ name: 'SelectedOrderRepository', atMs: 1800 }),
  DemoCue.make({ name: 'FilledOrderGoal', atMs: 3600 }),
  DemoCue.make({ name: 'SubmittedOrderGoal', atMs: 5600 }),
  DemoCue.make({ name: 'ConfirmedPublicWork', atMs: 7600 }),
  DemoCue.make({ name: 'LoadedSubmittedOrder', atMs: 9000 }),
  DemoCue.make({ name: 'AdvancedOrderScoping', atMs: 10800 }),
  DemoCue.make({ name: 'AdvancedOrderQueued', atMs: 12600 }),
  DemoCue.make({ name: 'AdvancedOrderRunning', atMs: 14200 }),
  DemoCue.make({ name: 'CompletedPlayback', atMs: 15000 }),
]

export const cuesForDemoKey = (key: string): ReadonlyArray<DemoCue> =>
  key === 'demo:customer-order' ? demoOrderCues : demoCues

const firstDemoCue = (key: string): DemoCue =>
  key === 'demo:customer-order'
    ? DemoCue.make({ name: 'LoadedOrderRepositories', atMs: 0 })
    : DemoCue.make({ name: 'LoadedProjectRoom', atMs: 0 })

const finalDemoCue = (): DemoCue =>
  DemoCue.make({ name: 'CompletedPlayback', atMs: 15000 })

export const previousDemoCue = (key: string, cursorMs: number): DemoCue =>
  cuesForDemoKey(key).reduce<DemoCue>(
    (previous, cue) => (cue.atMs < cursorMs ? cue : previous),
    firstDemoCue(key),
  )

export const nextDemoCue = (key: string, cursorMs: number): DemoCue =>
  cuesForDemoKey(key).find(cue => cue.atMs > cursorMs) ?? finalDemoCue()

export const remainingDemoCues = (
  key: string,
  cursorMs: number,
): ReadonlyArray<DemoCue> =>
  cuesForDemoKey(key).filter(cue => cue.atMs > cursorMs)

export class DemoPlaybackService extends Context.Service<
  DemoPlaybackService,
  {
    readonly cues: (key: string) => Effect.Effect<ReadonlyArray<DemoCue>>
    readonly stream: (key: string, cursorMs: number) => Stream.Stream<Message>
  }
>()('DemoPlaybackService') {}

export const DemoPlaybackLive = Layer.succeed(DemoPlaybackService, {
  cues: Effect.fn('DemoPlaybackService.cues')(key =>
    Effect.succeed(cuesForDemoKey(key)),
  ),
  stream: (key, cursorMs) =>
    Stream.fromIterable(remainingDemoCues(key, cursorMs)).pipe(
      Stream.schedule(Schedule.spaced(Duration.millis(1500))),
      Stream.map(cue => AdvancedDemoCue({ cue })),
    ),
})

export const demoPlaybackStream = (
  key: string,
  cursorMs: number,
): Stream.Stream<Message> =>
  Stream.unwrap(
    Effect.map(DemoPlaybackService, service =>
      service.stream(key, cursorMs),
    ).pipe(Effect.provide(DemoPlaybackLive)),
  )
