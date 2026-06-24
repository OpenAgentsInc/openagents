import { ServerMessage, SyncPatch } from '@openagentsinc/sync-schema'
import { Duration, Effect, Exit, Queue, Schema as S, Stream } from 'effect'
import { Subscription } from 'foldkit'

import { parseJsonRecord } from './json-boundary'

import {
  GotDemoMessage,
  GotLoggedInMessage,
  GotLoggedOutMessage,
  type Message,
} from './message'
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
  ClosedAutopilotOnboardingResumeStream,
  ClosedSettledFeedStream,
  FailedAutopilotOnboardingResume,
  FailedAutopilotOnboardingTurn,
  FailedSettledFeedStream,
  OpenedAutopilotOnboardingStream,
  OpenedSettledFeedStream,
  ReceivedAutopilotOnboardingDelta,
  ReceivedAutopilotOnboardingResumeReply,
  ReceivedAutopilotOnboardingStreamHandshake,
  ReceivedSettledFeedCursorGap,
  ReceivedSettledFeedPatch,
  RequestedPollKhalaTokensServed,
  SucceededAutopilotOnboardingTurn,
  SucceededResumeAutopilotOnboardingTurn,
  FailedKhalaChatTurn,
  OpenedKhalaChatStream,
  ReceivedKhalaChatDelta,
  SucceededKhalaChatTurn,
} from './page/loggedOut/message'
import type { Message as LoggedOutMessage } from './page/loggedOut/message'
import {
  type OnboardingStreamEvent,
  parseOnboardingStreamEvent,
} from './page/autopilot-onboarding/flow'
import {
  type KhalaChatStreamEvent,
  KhalaChatTurn,
  parseKhalaChatStreamEvent,
} from './page/khala-chat/flow'
import { newOnboardingSessionId } from './page/loggedOut/update'
import { SETTLED_FEED_SCOPE } from './page/loggedOut/settled-feed'
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

// Live settled feed (openagents #5311). The logged-out homepage / stats
// surfaces subscribe to ONE public, read-only sync room scope so settled totals
// render live as real Bitcoin settlements stream. Reuses the exact same
// WebSocket + cursor-replay plumbing as the logged-in workspace stream; falls
// back to the non-realtime snapshot fetch when the socket is unavailable.
const inactiveSettledFeed: {
  readonly cursor: number
  readonly isActive: boolean
  readonly scope: string
  readonly streamHref: string
} = {
  cursor: 0,
  isActive: false,
  scope: '',
  streamHref: '',
}

type SettledFeedDependencies = typeof inactiveSettledFeed

const settledFeedRouteIsLive = (
  model: Extract<Model, { _tag: 'LoggedOut' }>,
): boolean =>
  model.route._tag === 'Home' ||
  model.route._tag === 'Stats' ||
  model.route._tag === 'PublicStatsArchive'

export const settledFeedDependenciesForModel = (
  model: Model,
): SettledFeedDependencies => {
  if (model._tag !== 'LoggedOut' || !settledFeedRouteIsLive(model)) {
    return inactiveSettledFeed
  }

  const cursor = model.settledFeed.cursor

  return {
    cursor,
    isActive: true,
    scope: SETTLED_FEED_SCOPE,
    streamHref: syncStreamHref(SETTLED_FEED_SCOPE, cursor),
  }
}

// "Khala Tokens Served" homepage counter (#6227). v1 realtime = poll: while the
// logged-out homepage / stats surface is mounted, tick every few seconds and
// re-fetch the public aggregate so the odometer count-up reads live. Reuses the
// exact poll shape as autopilotRunPoll (Stream.tick + Stream.when); no Durable
// Object / WebSocket for this pass (a true-push v2 is a noted follow-up).
const KHALA_TOKENS_SERVED_POLL_INTERVAL_SECONDS = 4

const inactiveKhalaTokensServedPoll = { isActive: false }

type KhalaTokensServedPollDependencies = typeof inactiveKhalaTokensServedPoll

export const khalaTokensServedPollDependenciesForModel = (
  model: Model,
): KhalaTokensServedPollDependencies =>
  model._tag === 'LoggedOut' && settledFeedRouteIsLive(model)
    ? { isActive: true }
    : inactiveKhalaTokensServedPoll

const settledFeedStream = (
  dependencies: SettledFeedDependencies,
): Stream.Stream<Message> => {
  if (!dependencies.isActive) {
    return Stream.empty
  }

  const { streamHref } = dependencies

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
            GotLoggedOutMessage({ message: OpenedSettledFeedStream() }),
          )
        })
        socket.addEventListener('message', event => {
          const decoded = syncMessageFromPayload(String(event.data))

          Queue.offerUnsafe(
            queue,
            GotLoggedOutMessage({
              message:
                decoded._tag === 'ReceivedSyncPatch'
                  ? ReceivedSettledFeedPatch({ patch: decoded.patch })
                  : decoded._tag === 'ReceivedSyncCursorGap'
                    ? ReceivedSettledFeedCursorGap({ gap: decoded.gap })
                    : FailedSettledFeedStream({ error: decoded.error }),
            }),
          )
        })
        socket.addEventListener('close', () => {
          if (resource.released) {
            return
          }

          Queue.offerUnsafe(
            queue,
            GotLoggedOutMessage({ message: ClosedSettledFeedStream() }),
          )
        })
        socket.addEventListener('error', () => {
          if (resource.released) {
            return
          }

          Queue.offerUnsafe(
            queue,
            GotLoggedOutMessage({
              message: FailedSettledFeedStream({
                error: 'Settled feed stream connection failed.',
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

// Live /autopilot onboarding turn stream (issue #6123 UI follow-up). When a turn
// is pending (set on submit), open the SSE stream and dispatch prose deltas as
// they arrive, then the terminal success/failure. Keyed by the turn id so the
// stream opens EXACTLY ONCE per turn (the keepAliveEquivalence below holds the
// open stream stable while a turn is in flight, and tears it down when the turn
// resolves and `pendingTurn` clears).
const inactiveOnboardingStream: {
  readonly isActive: boolean
  readonly turnId: string
  readonly sessionId: string
  readonly userText: string
  readonly vertical: 'general' | 'legal'
} = {
  isActive: false,
  turnId: '',
  sessionId: '',
  userText: '',
  vertical: 'general',
}

type OnboardingStreamDependencies = typeof inactiveOnboardingStream

export const onboardingStreamDependenciesForModel = (
  model: Model,
): OnboardingStreamDependencies => {
  if (model._tag !== 'LoggedOut') {
    return inactiveOnboardingStream
  }

  const pending = model.autopilotOnboarding.pendingTurn
  if (pending === null) {
    return inactiveOnboardingStream
  }

  return {
    isActive: true,
    turnId: pending.id,
    // Mint a session id at the stream boundary for the first turn (the server
    // creates the session on first contact; later turns reuse it).
    sessionId: pending.sessionId ?? newOnboardingSessionId(),
    userText: pending.userText,
    vertical: pending.vertical,
  }
}

// Read a fetch SSE body to completion, parsing each `event:`/`data:` block into
// a typed `OnboardingStreamEvent` and invoking `onEvent` for each. Shared by the
// live turn stream and the durable resume read, which differ only in how they
// map parsed events to messages (the live stream also emits a handshake +
// failure; the resume read accumulates + closes). The block-splitting +
// trailing-flush wire is identical, so it lives here once.
const readOnboardingSseEvents = async (
  body: NonNullable<Response['body']>,
  onEvent: (event: OnboardingStreamEvent) => void,
): Promise<void> => {
  const reader = body.pipeThrough(new TextDecoderStream()).getReader()
  let buffer = ''

  const drainBlock = (block: string): void => {
    const lines = block.split(/\r?\n/)
    const eventLine = lines.find(line => line.startsWith('event:'))
    const dataLines = lines
      .filter(line => line.startsWith('data:'))
      .map(line => line.slice('data:'.length).replace(/^ /, ''))

    if (dataLines.length === 0) {
      return
    }

    const eventName =
      eventLine === undefined
        ? 'message'
        : eventLine.slice('event:'.length).replace(/^ /, '').trim()
    const parsed = parseOnboardingStreamEvent(
      eventName,
      parseJsonRecord(dataLines.join('\n')),
    )

    if (parsed !== undefined) {
      onEvent(parsed)
    }
  }

  // Read frames; SSE events are separated by a blank line.
  for (;;) {
    const { done, value } = await reader.read()
    if (done) {
      break
    }
    buffer += value
    let separator = buffer.search(/\r?\n\r?\n/)
    while (separator !== -1) {
      const block = buffer.slice(0, separator)
      buffer = buffer.slice(separator).replace(/^\r?\n\r?\n/, '')
      drainBlock(block)
      separator = buffer.search(/\r?\n\r?\n/)
    }
  }
  // Flush any trailing block without a terminating blank line.
  if (buffer.trim() !== '') {
    drainBlock(buffer)
  }
}

// Live `/autopilot` onboarding turn stream. Opens the SSE POST and maps each
// parsed event to a message. Mirrors the settled-feed `Stream.callback` +
// acquireRelease lifecycle so an unmount aborts the in-flight request cleanly.
const onboardingStream = (
  dependencies: OnboardingStreamDependencies,
): Stream.Stream<Message> => {
  if (!dependencies.isActive) {
    return Stream.empty
  }

  const { turnId, sessionId, userText, vertical } = dependencies

  return Stream.callback<Message>(queue =>
    Effect.acquireRelease(
      Effect.sync(() => {
        const controller = new AbortController()
        const resource = { aborted: false, controller }

        const offer = (message: LoggedOutMessage): void => {
          if (!resource.aborted) {
            Queue.offerUnsafe(queue, GotLoggedOutMessage({ message }))
          }
        }

        const fail = (): void =>
          offer(
            FailedAutopilotOnboardingTurn({
              reason:
                'Autopilot could not respond just now. Try sending that again.',
            }),
          )

        const run = async (): Promise<void> => {
          const response = await fetch(
            `/api/autopilot/onboarding/${encodeURIComponent(sessionId)}/turn`,
            {
              method: 'POST',
              cache: 'no-store',
              signal: controller.signal,
              headers: {
                accept: 'text/event-stream',
                'content-type': 'application/json',
              },
              body: JSON.stringify({
                userText,
                vertical,
              }),
            },
          )

          if (!response.ok || response.body === null) {
            fail()
            return
          }

          offer(OpenedAutopilotOnboardingStream({ turnId }))

          await readOnboardingSseEvents(response.body, event => {
            if (event.kind === 'stream') {
              // The handshake frame carries the durable cursor; persist it so a
              // reload can resume this turn from the durable log.
              offer(
                ReceivedAutopilotOnboardingStreamHandshake({
                  turnId,
                  streamId: event.streamId,
                  sessionId: event.sessionId,
                  turnIndex: event.turnIndex,
                }),
              )
            } else if (event.kind === 'delta') {
              offer(ReceivedAutopilotOnboardingDelta({ turnId, text: event.text }))
            } else if (event.kind === 'done') {
              offer(SucceededAutopilotOnboardingTurn({ response: event.response }))
            } else {
              fail()
            }
          })
        }

        run().catch(() => {
          if (!resource.aborted) {
            fail()
          }
        })

        return resource
      }),
      resource =>
        Effect.sync(() => {
          resource.aborted = true
          resource.controller.abort()
        }),
    ).pipe(Effect.flatMap(() => Effect.never)),
  )
}

// GENERIC /khala CHAT stream (a minimal stateless streaming chat — NOT the
// concierge intake). When a turn is pending (set on submit), POST the whole
// running conversation to `POST /api/khala/chat` and dispatch prose deltas as
// they arrive, then the terminal success/failure. Keyed by the turn id so the
// stream opens EXACTLY ONCE per turn. Stateless: there is no session id, no
// durable resume — the body carries the running message list each turn.
const inactiveKhalaChatStream: {
  readonly isActive: boolean
  readonly turnId: string
  readonly userText: string
  readonly history: ReadonlyArray<KhalaChatTurn>
} = {
  isActive: false,
  turnId: '',
  userText: '',
  history: [],
}

type KhalaChatStreamDependencies = typeof inactiveKhalaChatStream

export const khalaChatStreamDependenciesForModel = (
  model: Model,
): KhalaChatStreamDependencies => {
  if (model._tag !== 'LoggedOut') {
    return inactiveKhalaChatStream
  }
  const pending = model.khalaChat.pendingTurn
  if (pending === null) {
    return inactiveKhalaChatStream
  }
  return {
    isActive: true,
    turnId: pending.id,
    userText: pending.userText,
    history: pending.history,
  }
}

// Read a fetch SSE body to completion, parsing each `event:`/`data:` block into
// a typed `KhalaChatStreamEvent`. Same block-splitting wire as the onboarding
// reader; differs only in the parser (the khala wire has no handshake).
const readKhalaChatSseEvents = async (
  body: NonNullable<Response['body']>,
  onEvent: (event: KhalaChatStreamEvent) => void,
): Promise<void> => {
  const reader = body.pipeThrough(new TextDecoderStream()).getReader()
  let buffer = ''

  const drainBlock = (block: string): void => {
    const lines = block.split(/\r?\n/)
    const eventLine = lines.find(line => line.startsWith('event:'))
    const dataLines = lines
      .filter(line => line.startsWith('data:'))
      .map(line => line.slice('data:'.length).replace(/^ /, ''))

    if (dataLines.length === 0) {
      return
    }

    const eventName =
      eventLine === undefined
        ? 'message'
        : eventLine.slice('event:'.length).replace(/^ /, '').trim()
    const parsed = parseKhalaChatStreamEvent(
      eventName,
      parseJsonRecord(dataLines.join('\n')),
    )

    if (parsed !== undefined) {
      onEvent(parsed)
    }
  }

  for (;;) {
    const { done, value } = await reader.read()
    if (done) {
      break
    }
    buffer += value
    let separator = buffer.search(/\r?\n\r?\n/)
    while (separator !== -1) {
      const block = buffer.slice(0, separator)
      buffer = buffer.slice(separator).replace(/^\r?\n\r?\n/, '')
      drainBlock(block)
      separator = buffer.search(/\r?\n\r?\n/)
    }
  }
  if (buffer.trim() !== '') {
    drainBlock(buffer)
  }
}

const khalaChatStream = (
  dependencies: KhalaChatStreamDependencies,
): Stream.Stream<Message> => {
  if (!dependencies.isActive) {
    return Stream.empty
  }

  const { turnId, userText, history } = dependencies

  return Stream.callback<Message>(queue =>
    Effect.acquireRelease(
      Effect.sync(() => {
        const controller = new AbortController()
        const resource = { aborted: false, controller }

        const offer = (message: LoggedOutMessage): void => {
          if (!resource.aborted) {
            Queue.offerUnsafe(queue, GotLoggedOutMessage({ message }))
          }
        }

        const fail = (): void =>
          offer(
            FailedKhalaChatTurn({
              reason: 'Khala could not respond just now. Try sending that again.',
            }),
          )

        const run = async (): Promise<void> => {
          const response = await fetch('/api/khala/chat', {
            method: 'POST',
            cache: 'no-store',
            signal: controller.signal,
            headers: {
              accept: 'text/event-stream',
              'content-type': 'application/json',
            },
            body: JSON.stringify({
              messages: [...history, { role: 'user', content: userText }],
            }),
          })

          if (!response.ok || response.body === null) {
            fail()
            return
          }

          offer(OpenedKhalaChatStream({ turnId }))

          await readKhalaChatSseEvents(response.body, event => {
            if (event.kind === 'delta') {
              offer(ReceivedKhalaChatDelta({ turnId, text: event.text }))
            } else if (event.kind === 'done') {
              offer(SucceededKhalaChatTurn({ turnId }))
            } else {
              fail()
            }
          })
        }

        run().catch(() => {
          if (!resource.aborted) {
            fail()
          }
        })

        return resource
      }),
      resource =>
        Effect.sync(() => {
          resource.aborted = true
          resource.controller.abort()
        }),
    ).pipe(Effect.flatMap(() => Effect.never)),
  )
}

// Resume read of an in-flight onboarding turn after reload (#6154 tier 4). When
// the rehydrated model carries a `resumeTurn` (a turn was mid-stream when the
// tab went away), open the durable resume read
// (`GET /api/autopilot/onboarding/{sessionId}/turn/{turnIndex}/stream?offset=`),
// which replays the SAME `delta`/`done` wire from the durable log starting at
// `offset`. The replay re-streams the WHOLE in-flight turn from that offset, so
// deltas accumulate into a fresh reply (the update REPLACES the bubble, never
// double-appends a re-replayed prefix). The `stream-next-offset` header advances
// the persisted offset so a second reload resumes further along. A 404 (durable
// log gone / TTL expired) yields a typed failure so the page falls back to the
// reconciled transcript without a stuck half-bubble.
const inactiveOnboardingResume: {
  readonly isActive: boolean
  readonly sessionId: string
  readonly turnIndex: number
  readonly offset: string
} = {
  isActive: false,
  sessionId: '',
  turnIndex: 0,
  offset: '',
}

type OnboardingResumeDependencies = typeof inactiveOnboardingResume

export const onboardingResumeDependenciesForModel = (
  model: Model,
): OnboardingResumeDependencies => {
  if (model._tag !== 'LoggedOut') {
    return inactiveOnboardingResume
  }

  const flow = model.autopilotOnboarding
  const inFlight = flow.inFlight
  if (inFlight === null || !inFlight.resuming || flow.sessionId === null) {
    return inactiveOnboardingResume
  }

  return {
    isActive: true,
    sessionId: flow.sessionId,
    turnIndex: inFlight.turnIndex,
    offset: inFlight.lastOffset ?? '0',
  }
}

const onboardingResumeStream = (
  dependencies: OnboardingResumeDependencies,
): Stream.Stream<Message> => {
  if (!dependencies.isActive) {
    return Stream.empty
  }

  const { sessionId, turnIndex, offset } = dependencies

  return Stream.callback<Message>(queue =>
    Effect.acquireRelease(
      Effect.sync(() => {
        const controller = new AbortController()
        const resource = { aborted: false, controller }

        const offer = (message: LoggedOutMessage): void => {
          if (!resource.aborted) {
            Queue.offerUnsafe(queue, GotLoggedOutMessage({ message }))
          }
        }

        const run = async (): Promise<void> => {
          const response = await fetch(
            `/api/autopilot/onboarding/${encodeURIComponent(sessionId)}/turn/${turnIndex}/stream?offset=${encodeURIComponent(offset)}`,
            {
              method: 'GET',
              cache: 'no-store',
              signal: controller.signal,
              headers: { accept: 'text/event-stream' },
            },
          )

          // 404 / non-OK => durable log gone or unwired: fall back to the
          // reconciled transcript (the completed reply may already be there).
          if (!response.ok || response.body === null) {
            offer(FailedAutopilotOnboardingResume({ turnIndex }))
            return
          }

          const nextOffset = response.headers.get('stream-next-offset')

          // The replay re-streams the whole turn from `offset`; accumulate fresh
          // and REPLACE the bubble so a re-replayed prefix never double-counts.
          let accumulated = ''
          let sawDone = false

          await readOnboardingSseEvents(response.body, event => {
            if (event.kind === 'delta') {
              accumulated += event.text
              offer(
                ReceivedAutopilotOnboardingResumeReply({
                  turnIndex,
                  reply: accumulated,
                  nextOffset,
                }),
              )
            } else if (event.kind === 'done') {
              sawDone = true
              offer(
                SucceededResumeAutopilotOnboardingTurn({
                  response: event.response,
                }),
              )
            }
            // A `stream` handshake or `error` frame in the replay is ignored:
            // the close handler below resolves the terminal state.
          })

          // EOF without a `done` frame: the durable log ended mid-turn (the live
          // producer is still writing, or the turn was abandoned). Mark the
          // resume stream closed so the page stops resuming without losing what
          // streamed; a later load re-checks via reconcile/resume.
          if (!sawDone) {
            offer(ClosedAutopilotOnboardingResumeStream({ turnIndex }))
          }
        }

        run().catch(() => {
          offer(FailedAutopilotOnboardingResume({ turnIndex }))
        })

        return resource
      }),
      resource =>
        Effect.sync(() => {
          resource.aborted = true
          resource.controller.abort()
        }),
    ).pipe(Effect.flatMap(() => Effect.never)),
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
  settledFeed: entry(
    {
      cursor: S.Number,
      isActive: S.Boolean,
      scope: S.String,
      streamHref: S.String,
    },
    {
      modelToDependencies: settledFeedDependenciesForModel,
      keepAliveEquivalence: (left, right) =>
        left.isActive === right.isActive && left.scope === right.scope,
      dependenciesToStream: settledFeedStream,
    },
  ),
  khalaTokensServedPoll: entry(
    {
      isActive: S.Boolean,
    },
    {
      modelToDependencies: khalaTokensServedPollDependenciesForModel,
      dependenciesToStream: ({ isActive }: { isActive: boolean }) =>
        Stream.when(
          Stream.tick(
            Duration.seconds(KHALA_TOKENS_SERVED_POLL_INTERVAL_SECONDS),
          ).pipe(
            Stream.map(() =>
              GotLoggedOutMessage({
                message: RequestedPollKhalaTokensServed(),
              }),
            ),
          ),
          Effect.sync(() => isActive),
        ),
    },
  ),
  autopilotOnboardingStream: entry(
    {
      isActive: S.Boolean,
      turnId: S.String,
      sessionId: S.String,
      userText: S.String,
      vertical: S.Literals(['general', 'legal']),
    },
    {
      modelToDependencies: onboardingStreamDependenciesForModel,
      // Hold the open stream stable for the duration of a single turn; a change
      // of `turnId` (a new turn) tears down the old stream and opens a new one.
      keepAliveEquivalence: (left, right) =>
        left.isActive === right.isActive && left.turnId === right.turnId,
      dependenciesToStream: onboardingStream,
    },
  ),
  autopilotOnboardingResumeStream: entry(
    {
      isActive: S.Boolean,
      sessionId: S.String,
      turnIndex: S.Int,
      offset: S.String,
    },
    {
      modelToDependencies: onboardingResumeDependenciesForModel,
      // Hold the resume read stable for one (turnIndex, offset) attempt; a new
      // offset (a second reload resuming further along) reopens the read.
      keepAliveEquivalence: (left, right) =>
        left.isActive === right.isActive &&
        left.sessionId === right.sessionId &&
        left.turnIndex === right.turnIndex &&
        left.offset === right.offset,
      dependenciesToStream: onboardingResumeStream,
    },
  ),
  khalaChatStream: entry(
    {
      isActive: S.Boolean,
      turnId: S.String,
      userText: S.String,
      history: S.Array(KhalaChatTurn),
    },
    {
      modelToDependencies: khalaChatStreamDependenciesForModel,
      // Hold the open stream stable for the duration of a single turn; a change
      // of `turnId` (a new turn) tears down the old stream and opens a new one.
      keepAliveEquivalence: (left, right) =>
        left.isActive === right.isActive && left.turnId === right.turnId,
      dependenciesToStream: khalaChatStream,
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
