import { ServerMessage, SyncPatch } from '@openagentsinc/sync-schema'
import { Duration, Effect, Exit, Queue, Schema as S, Stream } from 'effect'
import { Subscription } from 'foldkit'

import { GotDemoMessage, GotLoggedInMessage, type Message } from './message'
import type { Model } from './model'
import { Demo } from './model'
import {
  ClosedSyncStream,
  FailedSyncStream,
  OpenedSyncStream,
  ReceivedSyncCursorGap,
  ReceivedSyncPatch,
  RequestedPollAutopilotRun,
} from './page/loggedIn/message'
import {
  syncAgentRunScope,
  syncTeamScope,
  teamRouteRef,
} from './page/loggedIn/model'
import { authorizedThreadRouteScope } from './page/loggedIn/thread-route'
import { chatRunIsBusy } from './page/loggedIn/update'
import { loggedInWorkroomAllowed } from './product-policy'
import type { LoggedInRoute } from './route'

const inactiveAutopilotRunPoll = {
  isActive: false,
  runId: '',
}

type AutopilotRunPollDependencies = typeof inactiveAutopilotRunPoll

const SyncStreamTarget = S.Struct({
  cursor: S.Number,
  scope: S.String,
  streamHref: S.String,
})

type SyncStreamTarget = typeof SyncStreamTarget.Type

const inactiveSyncStreams: {
  readonly isActive: boolean
  readonly scopeKey: string
  readonly targets: ReadonlyArray<SyncStreamTarget>
} = {
  isActive: false,
  scopeKey: '',
  targets: [],
}

const inactiveDemoPlayback = {
  cursorMs: 0,
  isActive: false,
  key: '',
}

const inactiveDemoKeyboard = {
  isActive: false,
  key: '',
}

const inactiveDemoClock = {
  isActive: false,
  key: '',
}

type SyncStreamDependencies = typeof inactiveSyncStreams

const isChatRoute = (route: LoggedInRoute): boolean =>
  route._tag === 'Chat' ||
  route._tag === 'TeamChat' ||
  route._tag === 'TeamProjectChat' ||
  route._tag === 'Thread'

export const autopilotRunPollDependenciesForModel = (
  model: Model,
): AutopilotRunPollDependencies => {
  if (
    model._tag !== 'LoggedIn' ||
    !isChatRoute(model.route) ||
    model.chatRun._tag !== 'Active' ||
    !chatRunIsBusy(model)
  ) {
    return inactiveAutopilotRunPoll
  }

  return {
    isActive: true,
    runId: model.chatRun.metadata.runId,
  }
}

const syncStreamHref = (scope: string, cursor: number): string => {
  const [kind, ...idParts] = scope.split(':')
  const id = idParts.join(':')

  return `/api/sync/${kind}/${encodeURIComponent(id)}/stream?cursor=${cursor}`
}

const uniqueScopes = (scopes: ReadonlyArray<string>): ReadonlyArray<string> => [
  ...new Set(scopes),
]

const teamSyncScopeForRoute = (
  model: Extract<Model, { _tag: 'LoggedIn' }>,
): string | undefined => {
  if (
    model.route._tag !== 'TeamChat' &&
    model.route._tag !== 'TeamProjectChat' &&
    model.route._tag !== 'TeamFiles' &&
    model.route._tag !== 'TeamFile'
  ) {
    return undefined
  }

  const { teamRef } = model.route
  const teamId = model.auth.teams.find(
    team => teamRouteRef(team) === teamRef,
  )?.id

  return teamId === undefined ? undefined : syncTeamScope(teamId)
}

const syncScopesForModel = (model: Extract<Model, { _tag: 'LoggedIn' }>) => {
  const routeTeamScope = teamSyncScopeForRoute(model)

  return uniqueScopes([
    model.sync.workspaceScope,
    ...(routeTeamScope === undefined ? [] : [routeTeamScope]),
    ...(() => {
      const maybeScope = authorizedThreadRouteScope(model.threadRoute)

      return maybeScope === undefined ? [] : [maybeScope]
    })(),
    ...(model.chatRun._tag === 'Active'
      ? [syncAgentRunScope(model.chatRun.metadata.runId)]
      : []),
  ])
}

export const syncStreamDependenciesForModel = (
  model: Model,
): SyncStreamDependencies => {
  if (model._tag !== 'LoggedIn') {
    return inactiveSyncStreams
  }

  if (!loggedInWorkroomAllowed(model.auth)) {
    return inactiveSyncStreams
  }

  const targets = syncScopesForModel(model).map(scope => {
    const cursor = model.sync.cursors[scope] ?? 0

    return {
      cursor,
      scope,
      streamHref: syncStreamHref(scope, cursor),
    }
  })

  return {
    isActive: true,
    scopeKey: targets.map(target => target.scope).join('|'),
    targets,
  }
}

export const workspaceSyncDependenciesForModel = (model: Model) => {
  const dependencies = syncStreamDependenciesForModel(model)
  const target = dependencies.targets[0]

  return target === undefined
    ? {
        cursor: 0,
        isActive: false,
        scope: '',
        streamHref: '',
      }
    : {
        cursor: target.cursor,
        isActive: dependencies.isActive,
        scope: target.scope,
        streamHref: target.streamHref,
      }
}

const webSocketUrl = (href: string): string => {
  const url = new URL(href, window.location.href)
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'

  return url.toString()
}

const ServerMessageFromJson = S.fromJsonString(ServerMessage)

export const syncMessageFromPayload = (payload: string) =>
  S.decodeUnknownExit(ServerMessageFromJson)(payload).pipe(exit =>
    Exit.isSuccess(exit)
      ? exit.value instanceof SyncPatch
        ? ReceivedSyncPatch({ patch: exit.value })
        : ReceivedSyncCursorGap({ gap: exit.value })
      : FailedSyncStream({
          error: 'Sync stream message could not be decoded.',
          scope: '',
        }),
  )

const scopedSyncMessageFromPayload = (scope: string, payload: string) => {
  const message = syncMessageFromPayload(payload)

  return message._tag === 'FailedSyncStream'
    ? FailedSyncStream({ error: message.error, scope })
    : message
}

const syncTargetStream = (target: SyncStreamTarget): Stream.Stream<Message> => {
  const { scope, streamHref } = target
  return Stream.callback<Message>(queue =>
    Effect.acquireRelease(
      Effect.sync(() => {
        const socket = new WebSocket(webSocketUrl(streamHref))
        const resource = { released: false, socket }
        socket.addEventListener('open', () => {
          if (resource.released) {
            socket.close()
            return
          }

          Queue.offerUnsafe(
            queue,
            GotLoggedInMessage({ message: OpenedSyncStream({ scope }) }),
          )
        })
        socket.addEventListener('message', event => {
          Queue.offerUnsafe(
            queue,
            GotLoggedInMessage({
              message: scopedSyncMessageFromPayload(scope, String(event.data)),
            }),
          )
        })
        socket.addEventListener('close', () => {
          if (resource.released) {
            return
          }

          Queue.offerUnsafe(
            queue,
            GotLoggedInMessage({ message: ClosedSyncStream({ scope }) }),
          )
        })
        socket.addEventListener('error', () => {
          if (resource.released) {
            return
          }

          Queue.offerUnsafe(
            queue,
            GotLoggedInMessage({
              message: FailedSyncStream({
                error: 'Sync stream connection failed.',
                scope,
              }),
            }),
          )
        })

        return resource
      }),
      resource =>
        Effect.sync(() => {
          resource.released = true

          if (resource.socket.readyState === WebSocket.OPEN) {
            resource.socket.close()
          }
        }),
    ).pipe(Effect.flatMap(() => Effect.never)),
  )
}

const syncStreams = (
  dependencies: SyncStreamDependencies,
): Stream.Stream<Message> => {
  if (!dependencies.isActive || dependencies.targets.length === 0) {
    return Stream.empty
  }

  return Stream.mergeAll({ concurrency: 'unbounded' })(
    dependencies.targets.map(syncTargetStream),
  )
}

export const demoKeyboardDependenciesForModel = (model: Model) =>
  model._tag === 'Demo' && model.playback !== 'complete'
    ? { isActive: true, key: model.routeKey }
    : inactiveDemoKeyboard

export const demoPlaybackDependenciesForModel = (model: Model) =>
  model._tag === 'Demo' && model.playback === 'playing'
    ? {
        cursorMs: model.cueIndex,
        isActive: true,
        key: model.routeKey,
      }
    : inactiveDemoPlayback

export const demoClockDependenciesForModel = (model: Model) =>
  model._tag === 'Demo' && model.playback === 'playing'
    ? { isActive: true, key: model.routeKey }
    : inactiveDemoClock

const spacebarTargetIsEditable = (target: EventTarget | null): boolean => {
  if (!(target instanceof HTMLElement)) {
    return false
  }

  const tagName = target.tagName.toLowerCase()

  return (
    tagName === 'input' ||
    tagName === 'textarea' ||
    tagName === 'select' ||
    target.isContentEditable
  )
}

const demoKeyboardStream = (dependencies: {
  readonly isActive: boolean
}): Stream.Stream<Message> => {
  if (!dependencies.isActive) {
    return Stream.empty
  }

  return Stream.callback<Message>(queue =>
    Effect.acquireRelease(
      Effect.sync(() => {
        const onKeyDown = (event: KeyboardEvent) => {
          if (
            (event.code !== 'Space' && event.key !== ' ') ||
            spacebarTargetIsEditable(event.target)
          ) {
            return
          }

          event.preventDefault()
          Queue.offerUnsafe(
            queue,
            GotDemoMessage({ message: Demo.PressedDemoSpacebar() }),
          )
        }

        window.addEventListener('keydown', onKeyDown)

        return onKeyDown
      }),
      onKeyDown =>
        Effect.sync(() => {
          window.removeEventListener('keydown', onKeyDown)
        }),
    ).pipe(Effect.flatMap(() => Effect.never)),
  )
}

export const subscriptions = Subscription.make<Model, Message>()(entry => ({
  autopilotRunPoll: entry(
    {
      isActive: S.Boolean,
      runId: S.String,
    },
    {
      modelToDependencies: autopilotRunPollDependenciesForModel,
      dependenciesToStream: ({ isActive, runId }) =>
        Stream.when(
          Stream.tick(Duration.seconds(2)).pipe(
            Stream.map(() =>
              GotLoggedInMessage({
                message: RequestedPollAutopilotRun({ runId }),
              }),
            ),
          ),
          Effect.sync(() => isActive),
        ),
    },
  ),
  workspaceSync: entry(
    {
      isActive: S.Boolean,
      scopeKey: S.String,
      targets: S.Array(SyncStreamTarget),
    },
    {
      modelToDependencies: syncStreamDependenciesForModel,
      keepAliveEquivalence: (left, right) =>
        left.isActive === right.isActive && left.scopeKey === right.scopeKey,
      dependenciesToStream: syncStreams,
    },
  ),
  demoPlayback: entry(
    {
      isActive: S.Boolean,
      cursorMs: S.Number,
      key: S.String,
    },
    {
      modelToDependencies: demoPlaybackDependenciesForModel,
      keepAliveEquivalence: (left, right) =>
        left.isActive === right.isActive &&
        left.key === right.key &&
        left.cursorMs === right.cursorMs,
      dependenciesToStream: ({
        cursorMs,
        isActive,
        key,
      }: {
        cursorMs: number
        isActive: boolean
        key: string
      }) =>
        Stream.when(
          Demo.demoPlaybackStream(key, cursorMs).pipe(
            Stream.map(message => GotDemoMessage({ message })),
          ),
          Effect.succeed(isActive),
        ),
    },
  ),
  demoKeyboard: entry(
    {
      isActive: S.Boolean,
      key: S.String,
    },
    {
      modelToDependencies: demoKeyboardDependenciesForModel,
      keepAliveEquivalence: (left, right) =>
        left.isActive === right.isActive && left.key === right.key,
      dependenciesToStream: demoKeyboardStream,
    },
  ),
  demoClock: entry(
    {
      isActive: S.Boolean,
      key: S.String,
    },
    {
      modelToDependencies: demoClockDependenciesForModel,
      keepAliveEquivalence: (left, right) =>
        left.isActive === right.isActive && left.key === right.key,
      dependenciesToStream: ({ isActive }: { isActive: boolean }) =>
        Stream.when(
          Stream.tick(Duration.millis(100)).pipe(
            Stream.map(() =>
              GotDemoMessage({
                message: Demo.TickedDemoPlayback({ deltaMs: 100 }),
              }),
            ),
          ),
          Effect.succeed(isActive),
        ),
    },
  ),
}))
