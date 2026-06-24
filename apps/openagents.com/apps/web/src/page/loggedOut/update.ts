import { Clock, Effect, Match as M, Option, Schema as S } from 'effect'
import { Command, Dom } from 'foldkit'
import { load, pushUrl } from 'foldkit/navigation'
import { evo } from 'foldkit/struct'

import {
  CompletedCopyAgentInstructions,
  CompletedCopyShareLink,
  CompletedNavigateToKhala,
  CompletedNavigateToLanding,
  CompletedNavigateToTassadar,
  CompletedAutopilotOnboardingCreditKickoff,
  CompletedPersistAutopilotOnboarding,
  CompletedScrollAutopilotOnboardingThread,
  FailedReconcileAutopilotOnboardingSession,
  LoadedStoredAutopilotOnboarding,
  SucceededReconcileAutopilotOnboardingSession,
  FailedLoadPublicAdjutantActivity,
  FailedLoadPublicAgentGoal,
  FailedLoadPublicArtanisReport,
  FailedLoadPublicForumLaunchStatus,
  FailedLoadPublicForumTipLeaderboards,
  FailedLoadPublicProductPromises,
  FailedLoadPublicPromiseTransitions,
  FailedLoadPublicPylonStats,
  FailedLoadPublicTrainingRuns,
  FailedLoadSettledFeedSnapshot,
  FailedLoadShareProjection,
  Message,
  SucceededLoadPublicAdjutantActivity,
  SucceededLoadPublicAgentGoal,
  SucceededLoadPublicArtanisReport,
  SucceededLoadPublicForumLaunchStatus,
  SucceededLoadPublicForumTipLeaderboards,
  SucceededLoadPublicProductPromises,
  SucceededLoadPublicPromiseTransitions,
  SucceededLoadPublicPylonStats,
  SucceededLoadPublicTrainingRuns,
  SucceededLoadSettledFeedSnapshot,
  SucceededLoadShareProjection,
} from './message'
import {
  FailedPublicAdjutantActivity,
  FailedPublicAgent,
  FailedPublicArtanisReport,
  FailedPublicForumLaunchStatus,
  FailedPublicForumTipLeaderboards,
  FailedPublicProductPromises,
  FailedPublicPromiseTransitions,
  FailedPublicPylonStats,
  FailedPublicTrainingRuns,
  FailedShareProjection,
  LoadedPublicAdjutantActivity,
  LoadedPublicAgent,
  LoadedPublicArtanisReport,
  LoadedPublicForumLaunchStatus,
  LoadedPublicForumTipLeaderboards,
  LoadedPublicProductPromises,
  LoadedPublicPromiseTransitions,
  LoadedPublicPylonStats,
  LoadedPublicTrainingRuns,
  LoadedShareProjection,
  Model,
  PublicAdjutantActivity,
  PublicAgentGoalResponse,
  PublicArtanisReport,
  PublicForumLaunchStatus,
  PublicForumTipLeaderboards,
  PublicProductPromises,
  PublicPromiseTransitions,
  PublicPylonStats,
  PublicTrainingRunResponse,
  PublicTrainingRunsResponse,
  ShareProjectionResponse,
} from './model'
import { HUD_THREAD_END_SELECTOR } from '../autopilot-onboarding/page'
import { onboardingVerticalForSegment } from '../autopilot-onboarding/vertical-overlay'
import {
  OnboardingSessionResponse,
  type StoredInFlight,
  type StoredOnboardingSession,
  clearStoredSession,
  readStoredSession,
  storedSessionFromParts,
  writeStoredSession,
} from '../autopilot-onboarding/persistence'
import { FlowModel } from '../autopilot-onboarding/flow'
import { recordFromUnknown } from '../../json-boundary'
import { homeRouter, khalaRouter, tassadarRouter } from '../../route'
import {
  SETTLED_FEED_SCOPE,
  applySettledFeedPatch,
  settledFeedAfterCursorGap,
  settledFeedAfterSnapshot,
  settledFeedClosed,
  settledFeedFailed,
  settledFeedOpen,
} from './settled-feed'

type UpdateReturn = readonly [Model, ReadonlyArray<Command.Command<Message>>]
const withUpdateReturn = M.withReturnType<UpdateReturn>()

class PublicAgentGoalLoadError extends S.TaggedErrorClass<PublicAgentGoalLoadError>()(
  'PublicAgentGoalLoadError',
  { error: S.Defect },
) {}

class PublicPylonStatsLoadError extends S.TaggedErrorClass<PublicPylonStatsLoadError>()(
  'PublicPylonStatsLoadError',
  { error: S.Defect },
) {}

class PublicForumLaunchStatusLoadError extends S.TaggedErrorClass<PublicForumLaunchStatusLoadError>()(
  'PublicForumLaunchStatusLoadError',
  { error: S.Defect },
) {}

class PublicForumTipLeaderboardsLoadError extends S.TaggedErrorClass<PublicForumTipLeaderboardsLoadError>()(
  'PublicForumTipLeaderboardsLoadError',
  { error: S.Defect },
) {}

class PublicProductPromisesLoadError extends S.TaggedErrorClass<PublicProductPromisesLoadError>()(
  'PublicProductPromisesLoadError',
  { error: S.Defect },
) {}

class PublicPromiseTransitionsLoadError extends S.TaggedErrorClass<PublicPromiseTransitionsLoadError>()(
  'PublicPromiseTransitionsLoadError',
  { error: S.Defect },
) {}

class PublicTrainingRunsLoadError extends S.TaggedErrorClass<PublicTrainingRunsLoadError>()(
  'PublicTrainingRunsLoadError',
  { error: S.Defect },
) {}

class PublicArtanisReportLoadError extends S.TaggedErrorClass<PublicArtanisReportLoadError>()(
  'PublicArtanisReportLoadError',
  { error: S.Defect },
) {}

class PublicAdjutantActivityLoadError extends S.TaggedErrorClass<PublicAdjutantActivityLoadError>()(
  'PublicAdjutantActivityLoadError',
  { error: S.Defect },
) {}

class ShareProjectionLoadError extends S.TaggedErrorClass<ShareProjectionLoadError>()(
  'ShareProjectionLoadError',
  {
    error: S.Defect,
    status: S.Int,
  },
) {}

class ShareLinkCopyError extends S.TaggedErrorClass<ShareLinkCopyError>()(
  'ShareLinkCopyError',
  {
    error: S.Defect,
  },
) {}

export const LoadPublicAgentGoal = Command.define(
  'LoadPublicAgentGoal',
  { agentId: S.String, agentRef: S.String },
  SucceededLoadPublicAgentGoal,
  FailedLoadPublicAgentGoal,
)(({ agentId, agentRef }) =>
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(
          `/api/public/agents/${encodeURIComponent(agentId)}/current-goal`,
          {
            cache: 'no-store',
            headers: { accept: 'application/json' },
          },
        ),
      catch: error => new PublicAgentGoalLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicAgentGoalLoadError({
        error: `Public agent goal returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new PublicAgentGoalLoadError({ error }),
    })
    const decoded = yield* S.decodeUnknownEffect(PublicAgentGoalResponse)(
      payload,
    )

    return SucceededLoadPublicAgentGoal({ agentRef, response: decoded })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicAgentGoal({
          agentRef,
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

export const LoadPublicAdjutantActivity = Command.define(
  'LoadPublicAdjutantActivity',
  SucceededLoadPublicAdjutantActivity,
  FailedLoadPublicAdjutantActivity,
)(
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch('/api/public/adjutant/activity', {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new PublicAdjutantActivityLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicAdjutantActivityLoadError({
        error: `Public Autopilot activity returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new PublicAdjutantActivityLoadError({ error }),
    })
    const activity = yield* S.decodeUnknownEffect(PublicAdjutantActivity)(
      payload,
    )

    return SucceededLoadPublicAdjutantActivity({ activity })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicAdjutantActivity({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

export const LoadPublicArtanisReport = Command.define(
  'LoadPublicArtanisReport',
  SucceededLoadPublicArtanisReport,
  FailedLoadPublicArtanisReport,
)(
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch('/api/public/artanis/report', {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new PublicArtanisReportLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicArtanisReportLoadError({
        error: `Public Artanis report returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new PublicArtanisReportLoadError({ error }),
    })
    const report = yield* S.decodeUnknownEffect(PublicArtanisReport)(payload)

    return SucceededLoadPublicArtanisReport({ report })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicArtanisReport({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

export const LoadPublicPylonStats = Command.define(
  'LoadPublicPylonStats',
  SucceededLoadPublicPylonStats,
  FailedLoadPublicPylonStats,
)(
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch('/api/public/pylon-stats', {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new PublicPylonStatsLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicPylonStatsLoadError({
        error: `Public pylon stats returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new PublicPylonStatsLoadError({ error }),
    })
    const decoded = yield* S.decodeUnknownEffect(PublicPylonStats)(payload)

    return SucceededLoadPublicPylonStats({ stats: decoded })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicPylonStats({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

class SettledFeedSnapshotLoadError extends S.TaggedErrorClass<SettledFeedSnapshotLoadError>()(
  'SettledFeedSnapshotLoadError',
  {
    error: S.Defect,
  },
) {}

const SettledFeedSnapshotPayload = S.Struct({
  collections: S.Record(S.String, S.Record(S.String, S.Unknown)),
  cursor: S.Number,
})

const SettledFeedSnapshotSummary = S.Struct({
  totalSettledCount: S.Number,
  totalSettledSats: S.Number,
})

// Non-realtime cold read of the public settled-feed scope. Seeds the running
// totals + cursor before the WebSocket attaches, and is the graceful fallback
// when the socket is unavailable (the homepage still renders these totals).
export const LoadSettledFeedSnapshot = Command.define(
  'LoadSettledFeedSnapshot',
  SucceededLoadSettledFeedSnapshot,
  FailedLoadSettledFeedSnapshot,
)(
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(`/api/sync/${SETTLED_FEED_SCOPE.replace(':', '/')}/snapshot`, {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new SettledFeedSnapshotLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new SettledFeedSnapshotLoadError({
        error: `Settled feed snapshot returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new SettledFeedSnapshotLoadError({ error }),
    })
    const decoded =
      yield* S.decodeUnknownEffect(SettledFeedSnapshotPayload)(payload)
    const rawSummary = decoded.collections['settled_summary']?.['summary']
    const summary = rawSummary === undefined
      ? null
      : yield* S.decodeUnknownEffect(SettledFeedSnapshotSummary)(
          rawSummary,
        ).pipe(Effect.orElseSucceed(() => null))

    return SucceededLoadSettledFeedSnapshot({
      cursor: decoded.cursor,
      summary,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadSettledFeedSnapshot({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

export const LoadPublicForumLaunchStatus = Command.define(
  'LoadPublicForumLaunchStatus',
  SucceededLoadPublicForumLaunchStatus,
  FailedLoadPublicForumLaunchStatus,
)(
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch('/api/forum/launch-status', {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new PublicForumLaunchStatusLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicForumLaunchStatusLoadError({
        error: `Forum launch status returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new PublicForumLaunchStatusLoadError({ error }),
    })
    const decoded = yield* S.decodeUnknownEffect(PublicForumLaunchStatus)(
      payload,
    )

    return SucceededLoadPublicForumLaunchStatus({ status: decoded })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicForumLaunchStatus({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

export const LoadPublicForumTipLeaderboards = Command.define(
  'LoadPublicForumTipLeaderboards',
  SucceededLoadPublicForumTipLeaderboards,
  FailedLoadPublicForumTipLeaderboards,
)(
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch('/api/forum/tip-leaderboards?limit=10', {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new PublicForumTipLeaderboardsLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicForumTipLeaderboardsLoadError({
        error: `Forum tip leaderboards returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new PublicForumTipLeaderboardsLoadError({ error }),
    })
    const decoded = yield* S.decodeUnknownEffect(PublicForumTipLeaderboards)(
      payload,
    )

    return SucceededLoadPublicForumTipLeaderboards({
      leaderboards: decoded,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicForumTipLeaderboards({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

export const LoadPublicProductPromises = Command.define(
  'LoadPublicProductPromises',
  SucceededLoadPublicProductPromises,
  FailedLoadPublicProductPromises,
)(
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch('/api/public/product-promises', {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new PublicProductPromisesLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicProductPromisesLoadError({
        error: `Product promises returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new PublicProductPromisesLoadError({ error }),
    })
    const decoded = yield* S.decodeUnknownEffect(PublicProductPromises)(payload)

    return SucceededLoadPublicProductPromises({ promises: decoded })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicProductPromises({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

export const LoadPublicPromiseTransitions = Command.define(
  'LoadPublicPromiseTransitions',
  SucceededLoadPublicPromiseTransitions,
  FailedLoadPublicPromiseTransitions,
)(
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch('/api/public/product-promises/transitions', {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new PublicPromiseTransitionsLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicPromiseTransitionsLoadError({
        error: `Promise transitions returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new PublicPromiseTransitionsLoadError({ error }),
    })
    const decoded =
      yield* S.decodeUnknownEffect(PublicPromiseTransitions)(payload)

    return SucceededLoadPublicPromiseTransitions({ transitions: decoded })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicPromiseTransitions({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

export const LoadPublicTrainingRuns = Command.define(
  'LoadPublicTrainingRuns',
  { runId: S.NullOr(S.String) },
  SucceededLoadPublicTrainingRuns,
  FailedLoadPublicTrainingRuns,
)(({ runId }) =>
  Effect.gen(function* () {
    const path =
      runId === null
        ? '/api/training/runs'
        : `/api/training/runs/${encodeURIComponent(runId)}`
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(path, {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new PublicTrainingRunsLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicTrainingRunsLoadError({
        error: `Public training runs returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new PublicTrainingRunsLoadError({ error }),
    })
    const decoded =
      runId === null
        ? yield* S.decodeUnknownEffect(PublicTrainingRunsResponse)(payload)
        : yield* S.decodeUnknownEffect(PublicTrainingRunResponse)(payload).pipe(
            Effect.map(detail => ({
              runs: [detail.run],
              summaries: [detail.summary],
            })),
          )

    return SucceededLoadPublicTrainingRuns({
      response: decoded,
      selectedRunId: runId,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicTrainingRuns({
          runId,
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

export const LoadShareProjection = Command.define(
  'LoadShareProjection',
  { shareId: S.String },
  SucceededLoadShareProjection,
  FailedLoadShareProjection,
)(({ shareId }) =>
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(`/api/share/${encodeURIComponent(shareId)}/v1/data`, {
          cache: 'no-store',
          credentials: 'include',
          headers: { accept: 'application/json' },
        }),
      catch: error => new ShareProjectionLoadError({ error, status: 0 }),
    })

    if (!response.ok) {
      const payload = yield* Effect.tryPromise({
        try: () => response.json(),
        catch: () => ({ error: 'share_unavailable' }),
      })
      const record = recordFromUnknown(payload) ?? {}
      const error =
        typeof record.error === 'string'
          ? record.error
          : `Share returned HTTP ${response.status}.`

      return yield* new ShareProjectionLoadError({
        error,
        status: response.status,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new ShareProjectionLoadError({ error, status: 0 }),
    })
    const decoded = yield* S.decodeUnknownEffect(ShareProjectionResponse)(
      payload,
    )

    return SucceededLoadShareProjection({ response: decoded, shareId })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadShareProjection({
          shareId,
          error:
            error instanceof ShareProjectionLoadError
              ? String(error.error)
              : error instanceof Error
                ? error.message
                : String(error),
          status: error instanceof ShareProjectionLoadError ? error.status : 0,
        }),
      ),
    ),
  ),
)

export const CopyShareLink = Command.define(
  'CopyShareLink',
  { url: S.String },
  CompletedCopyShareLink,
)(({ url }) =>
  Effect.tryPromise({
    try: () => navigator.clipboard.writeText(url),
    catch: error => new ShareLinkCopyError({ error }),
  }).pipe(
    Effect.as(CompletedCopyShareLink({ url })),
    Effect.catch(() => Effect.succeed(CompletedCopyShareLink({ url }))),
  ),
)

// The exact, pasteable instruction block the "Copy Agent Instructions" button
// writes to the clipboard. Composed from the live agent contract
// (https://openagents.com/AGENTS.md): point the agent at the contract, then the
// one concrete first step the contract specifies — register an agent — so the
// pasted text is self-sufficient and grounded, not aspirational.
export const TASSADAR_AGENT_INSTRUCTIONS = [
  'Read https://openagents.com/AGENTS.md and join the OpenAgents Tassadar training run.',
  '',
  'First step — register an agent (no auth required):',
  '',
  'curl -X POST https://openagents.com/api/agents/register \\',
  '  -H "Content-Type: application/json" \\',
  "  -d '{\"displayName\": \"YOUR_AGENT_NAME\", \"slug\": \"your-agent-name\"}'",
  '',
  'Then install Pylon and join the run:',
  '  npx @openagentsinc/pylon',
  '  pylon training status --base-url https://openagents.com',
  '  pylon training preflight --base-url https://openagents.com',
  '  pylon training claim --base-url https://openagents.com --lease-seconds 300',
  '',
  'Accepted work is paid in Bitcoin over Lightning, with public receipts.',
].join('\n')

const CopyAgentInstructionsError = ShareLinkCopyError

export const CopyAgentInstructions = Command.define(
  'CopyAgentInstructions',
  { text: S.String },
  CompletedCopyAgentInstructions,
)(({ text }) =>
  Effect.tryPromise({
    try: () => navigator.clipboard.writeText(text),
    catch: error => new CopyAgentInstructionsError({ error }),
  }).pipe(
    Effect.as(CompletedCopyAgentInstructions()),
    Effect.catch(() => Effect.succeed(CompletedCopyAgentInstructions())),
  ),
)

export const NavigateToKhala = Command.define(
  'NavigateToKhala',
  CompletedNavigateToKhala,
)(pushUrl(khalaRouter()).pipe(Effect.as(CompletedNavigateToKhala())))

export const NavigateToTassadar = Command.define(
  'NavigateToTassadar',
  CompletedNavigateToTassadar,
)(pushUrl(tassadarRouter()).pipe(Effect.as(CompletedNavigateToTassadar())))

export const NavigateToLanding = Command.define(
  'NavigateToLanding',
  CompletedNavigateToLanding,
  // Landing IS home now (root `/`), so the back button on /khala and
  // /tassadar pushes the root and flies the camera home.
)(pushUrl(homeRouter()).pipe(Effect.as(CompletedNavigateToLanding())))

// Generate a session id for a new onboarding conversation. Uses secure browser
// randomness (the same primitive the checkout idempotency key uses); no
// Math.random. Exported so the streaming subscription mints the first-turn
// session id at the stream boundary (off the pure update path).
export const newOnboardingSessionId = (): string => {
  const bytes = new Uint8Array(12)
  if (
    globalThis.crypto !== undefined &&
    typeof globalThis.crypto.getRandomValues === 'function'
  ) {
    globalThis.crypto.getRandomValues(bytes)
    const segment = Array.from(bytes, byte =>
      byte.toString(16).padStart(2, '0'),
    ).join('')
    return `ob_${segment}`
  }

  throw new Error('Secure browser randomness is required to start onboarding.')
}

// The /autopilot onboarding turn now STREAMS (issue #6123 UI follow-up): a
// `pendingTurn` set on submit is picked up by the `autopilotOnboardingStream`
// subscription (`subscriptions.ts`), which opens the SSE stream
// (`POST /api/autopilot/onboarding/{sessionId}/turn` with
// `Accept: text/event-stream`) and dispatches `Opened…` / `Received…Delta` /
// `Succeeded…` / `Failed…` messages as deltas land. The submit handler stays a
// pure model transition (no fetch on the command path).

// Scroll the onboarding thread's bottom sentinel into view after a turn is sent
// or finishes. Fire-and-forget; the native scroll anchoring keeps it pinned
// while content grows, and this jumps to the bottom when the user sends.
export const ScrollAutopilotOnboardingThreadToEnd = Command.define(
  'ScrollAutopilotOnboardingThreadToEnd',
  CompletedScrollAutopilotOnboardingThread,
)(
  Dom.scrollIntoViewAfterPaint(HUD_THREAD_END_SELECTOR, {
    block: 'end',
  }).pipe(
    Effect.as(CompletedScrollAutopilotOnboardingThread()),
    Effect.catch(() =>
      Effect.succeed(CompletedScrollAutopilotOnboardingThread()),
    ),
  ),
)

// The credit_kickoff stub. A logged-out visitor cannot top up directly (the
// `/api/billing/checkout` entry needs an authenticated session); the honest v1
// stub routes them into the existing funded path via GitHub login, exactly like
// the legacy onboarding CTA. The payment->workspace->promise backend is
// explicitly deferred (#6129). `load` is a full-page navigation, so the
// completion message is a benign no-op that is effectively never observed.
const ONBOARDING_GITHUB_LOGIN_HREF = '/login/github'

export const OpenAutopilotCreditKickoff = Command.define(
  'OpenAutopilotCreditKickoff',
  CompletedAutopilotOnboardingCreditKickoff,
)(
  load(ONBOARDING_GITHUB_LOGIN_HREF).pipe(
    Effect.as(CompletedAutopilotOnboardingCreditKickoff()),
  ),
)

// BROWSER PERSISTENCE + DURABLE RESUME (#6154 tier 4) --------------------------

// Derive the stored localStorage record from the live flow model. Only the
// conversation/spec the user entered plus the resume cursor is persisted; the
// transcript is capped inside the persistence helper. Returns `null` when there
// is nothing worth persisting yet (no session id minted).
const storedSessionFromFlow = (
  flow: FlowModel,
  updatedAt: number,
): StoredOnboardingSession | null => {
  if (flow.sessionId === null) {
    return null
  }

  const inFlight: StoredInFlight | null =
    flow.inFlight === null
      ? null
      : {
          streamId: flow.inFlight.streamId,
          turnIndex: flow.inFlight.turnIndex,
          replySoFar: flow.streamingReply ?? '',
          lastOffset: flow.inFlight.lastOffset,
        }

  const status =
    flow.status === 'idle' && flow.turnCount > 0
      ? ('interviewing' as const)
      : null

  return storedSessionFromParts({
    sessionId: flow.sessionId,
    vertical: flow.vertical,
    status,
    transcript: flow.transcript,
    outputSpec: flow.outputSpec,
    inFlight,
    updatedAt,
  })
}

// Stop resuming an in-flight turn and fall back to the reconciled transcript
// (no stuck half-bubble), persisting the cleared cursor. Shared by the resume
// stream's terminal-without-done (closed) and 404 (failed) outcomes — both end
// the resume the same way. Ignores a turn that is no longer the resuming one.
const endResume = (
  model: Model,
  turnIndex: number,
): UpdateReturn => {
  const flow = model.autopilotOnboarding
  if (
    flow.inFlight === null ||
    !flow.inFlight.resuming ||
    flow.inFlight.turnIndex !== turnIndex
  ) {
    return [model, []]
  }
  const nextModel = evo(model, {
    autopilotOnboarding: current =>
      evo(current, {
        status: () => 'idle',
        streamingReply: () => null,
        inFlight: () => null,
      }),
  })
  return [
    nextModel,
    [PersistAutopilotOnboarding({ flow: nextModel.autopilotOnboarding })],
  ]
}

// Persist the current flow to localStorage (fire-and-forget). Reads the clock
// through Effect's `Clock` (no `Date.now`) for the `updatedAt` stamp; the write
// itself is the safe, guarded helper.
export const PersistAutopilotOnboarding = Command.define(
  'PersistAutopilotOnboarding',
  { flow: FlowModel },
  CompletedPersistAutopilotOnboarding,
)(({ flow }) =>
  Clock.currentTimeMillis.pipe(
    Effect.map(now => storedSessionFromFlow(flow, now)),
    Effect.tap(session =>
      Effect.sync(() => {
        if (session !== null) {
          writeStoredSession(session)
        }
      }),
    ),
    Effect.as(CompletedPersistAutopilotOnboarding()),
  ),
)

// Clear the stored onboarding session (start over / expired / 404).
export const ClearAutopilotOnboardingStorage = Command.define(
  'ClearAutopilotOnboardingStorage',
  CompletedPersistAutopilotOnboarding,
)(
  Effect.sync(() => {
    clearStoredSession()
  }).pipe(Effect.as(CompletedPersistAutopilotOnboarding())),
)

// On mount of `/autopilot` (and `/autopilot/{vertical}`), read the stored
// session. If present, dispatch it so the page restores the transcript
// immediately (no blank flash) and then reconciles with the server. Absent =>
// a benign completion (clean fresh start). The localStorage read is the safe,
// guarded helper; a corrupt blob is treated as absent.
export const RehydrateAutopilotOnboarding = Command.define(
  'RehydrateAutopilotOnboarding',
  LoadedStoredAutopilotOnboarding,
  CompletedPersistAutopilotOnboarding,
)(
  Effect.sync(() => readStoredSession()).pipe(
    Effect.map(maybeSession =>
      Option.match(maybeSession, {
        onNone: () => CompletedPersistAutopilotOnboarding(),
        onSome: session => LoadedStoredAutopilotOnboarding({ session }),
      }),
    ),
  ),
)

class OnboardingSessionReconcileError extends S.TaggedErrorClass<OnboardingSessionReconcileError>()(
  'OnboardingSessionReconcileError',
  { error: S.Defect, status: S.Int },
) {}

// Reconcile the rehydrated session with the authoritative server record. A 200
// adopts the server transcript/status/outputSpec (covers a turn that completed
// while the tab was gone); a 404 clears localStorage and starts fresh. A
// transient network error reports status 0 (the page keeps the local transcript
// and does not clear).
export const ReconcileAutopilotOnboardingSession = Command.define(
  'ReconcileAutopilotOnboardingSession',
  { sessionId: S.String },
  SucceededReconcileAutopilotOnboardingSession,
  FailedReconcileAutopilotOnboardingSession,
)(({ sessionId }) =>
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(`/api/autopilot/onboarding/${encodeURIComponent(sessionId)}`, {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new OnboardingSessionReconcileError({ error, status: 0 }),
    })

    if (!response.ok) {
      return yield* new OnboardingSessionReconcileError({
        error: `Onboarding session returned HTTP ${response.status}.`,
        status: response.status,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new OnboardingSessionReconcileError({ error, status: 0 }),
    })
    const decoded =
      yield* S.decodeUnknownEffect(OnboardingSessionResponse)(payload)

    return SucceededReconcileAutopilotOnboardingSession({ response: decoded })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedReconcileAutopilotOnboardingSession({
          status:
            error instanceof OnboardingSessionReconcileError
              ? error.status
              : 0,
        }),
      ),
    ),
  ),
)

const publicAgentIdForRef = (agentRef: string): string => {
  const knownAgentIds: Readonly<Record<string, string>> = {
    adjutant: 'agent_adjutant',
    artanis: 'agent_artanis',
  }

  return knownAgentIds[agentRef] ?? agentRef
}

export const initialCommands = (
  model: Model,
): ReadonlyArray<Command.Command<Message>> =>
  model.route._tag === 'Autopilot' || model.route._tag === 'AutopilotVertical'
    ? [RehydrateAutopilotOnboarding()]
    : model.route._tag === 'Share'
    ? [LoadShareProjection({ shareId: model.route.shareId })]
    : model.route._tag === 'Home' ||
        model.route._tag === 'Stats' ||
        model.route._tag === 'PublicStatsArchive'
      ? [
          LoadPublicPylonStats(),
          LoadPublicForumLaunchStatus(),
          LoadPublicForumTipLeaderboards(),
          LoadSettledFeedSnapshot(),
        ]
      : model.route._tag === 'ProductPromises'
        ? [LoadPublicProductPromises(), LoadPublicPromiseTransitions()]
        : model.route._tag === 'PublicTrainingRuns'
          ? [LoadPublicTrainingRuns({ runId: null })]
          : model.route._tag === 'PublicTrainingRun'
            ? [LoadPublicTrainingRuns({ runId: model.route.runId })]
            : model.route._tag === 'PublicAgent'
              ? model.route.agentRef === 'artanis'
                ? [
                    LoadPublicAgentGoal({
                      agentId: publicAgentIdForRef(model.route.agentRef),
                      agentRef: model.route.agentRef,
                    }),
                    LoadPublicArtanisReport(),
                    LoadPublicPylonStats(),
                  ]
                : model.route.agentRef === 'adjutant'
                  ? [
                      LoadPublicAgentGoal({
                        agentId: publicAgentIdForRef(model.route.agentRef),
                        agentRef: model.route.agentRef,
                      }),
                      LoadPublicAdjutantActivity(),
                    ]
                  : [
                      LoadPublicAgentGoal({
                        agentId: publicAgentIdForRef(model.route.agentRef),
                        agentRef: model.route.agentRef,
                      }),
                    ]
              : []

const fundingAmountFromInput = (value: string): number => {
  const parsed = Number.parseInt(value, 10)

  if (!Number.isFinite(parsed)) {
    return 5
  }

  return Math.min(500, Math.max(5, parsed))
}

export const update = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tagsExhaustive({
      ClickedCopyShareLink: ({ url }) => [model, [CopyShareLink({ url })]],
      ClickedEnterKhala: () => [model, [NavigateToKhala()]],
      CompletedNavigateToKhala: () => [model, []],
      ClickedExitKhala: () => [model, [NavigateToLanding()]],
      ClickedEnterTassadar: () => [model, [NavigateToTassadar()]],
      CompletedNavigateToTassadar: () => [model, []],
      ClickedCopyAgentInstructions: ({ text }) => [
        model,
        [CopyAgentInstructions({ text })],
      ],
      CompletedCopyAgentInstructions: () => [
        evo(model, { copiedAgentInstructions: () => true }),
        [],
      ],
      CompletedNavigateToLanding: () => [model, []],
      ClickedOnboardingStep: ({ step }) => [
        evo(model, {
          onboarding: onboarding => evo(onboarding, { step: () => step }),
        }),
        [],
      ],
      SelectedOnboardingRepository: ({ repository }) => [
        evo(model, {
          onboarding: onboarding =>
            evo(onboarding, {
              selectedRepository: () => repository,
              step: () => 'funding',
            }),
        }),
        [],
      ],
      SkippedOnboardingRepository: () => [
        evo(model, {
          onboarding: onboarding =>
            evo(onboarding, {
              selectedRepository: () => '',
              step: () => 'funding',
            }),
        }),
        [],
      ],
      UpdatedOnboardingFundingAmount: ({ value }) => [
        evo(model, {
          onboarding: onboarding =>
            evo(onboarding, {
              fundingAmount: () => fundingAmountFromInput(value),
              step: () => 'funding',
            }),
        }),
        [],
      ],
      ToggledOnboardingCoupon: () => [
        evo(model, {
          onboarding: onboarding =>
            evo(onboarding, {
              isCouponOpen: isOpen => !isOpen,
              step: () => 'funding',
            }),
        }),
        [],
      ],
      UpdatedOnboardingCouponCode: ({ value }) => [
        evo(model, {
          onboarding: onboarding =>
            evo(onboarding, {
              couponCode: () => value,
              isCouponOpen: () => true,
              step: () => 'funding',
            }),
        }),
        [],
      ],
      SucceededLoadPublicAgentGoal: ({ agentRef, response }) => [
        evo(model, {
          publicAgent: () => LoadedPublicAgent({ agentRef, response }),
        }),
        [],
      ],
      FailedLoadPublicAgentGoal: ({ agentRef, error }) => [
        evo(model, {
          publicAgent: () => FailedPublicAgent({ agentRef, error }),
        }),
        [],
      ],
      SucceededLoadPublicArtanisReport: ({ report }) => [
        evo(model, {
          publicArtanisReport: () => LoadedPublicArtanisReport({ report }),
        }),
        [],
      ],
      FailedLoadPublicArtanisReport: ({ error }) => [
        evo(model, {
          publicArtanisReport: () => FailedPublicArtanisReport({ error }),
        }),
        [],
      ],
      SucceededLoadPublicAdjutantActivity: ({ activity }) => [
        evo(model, {
          publicAdjutantActivity: () =>
            LoadedPublicAdjutantActivity({ activity }),
        }),
        [],
      ],
      FailedLoadPublicAdjutantActivity: ({ error }) => [
        evo(model, {
          publicAdjutantActivity: () => FailedPublicAdjutantActivity({ error }),
        }),
        [],
      ],
      SucceededLoadPublicPylonStats: ({ stats }) => [
        evo(model, {
          publicPylonStats: () => LoadedPublicPylonStats({ stats }),
        }),
        [],
      ],
      FailedLoadPublicPylonStats: ({ error }) => [
        evo(model, {
          publicPylonStats: () => FailedPublicPylonStats({ error }),
        }),
        [],
      ],
      SucceededLoadPublicForumLaunchStatus: ({ status }) => [
        evo(model, {
          publicForumLaunchStatus: () =>
            LoadedPublicForumLaunchStatus({ status }),
        }),
        [],
      ],
      FailedLoadPublicForumLaunchStatus: ({ error }) => [
        evo(model, {
          publicForumLaunchStatus: () =>
            FailedPublicForumLaunchStatus({ error }),
        }),
        [],
      ],
      SucceededLoadPublicForumTipLeaderboards: ({ leaderboards }) => [
        evo(model, {
          publicForumTipLeaderboards: () =>
            LoadedPublicForumTipLeaderboards({ leaderboards }),
        }),
        [],
      ],
      FailedLoadPublicForumTipLeaderboards: ({ error }) => [
        evo(model, {
          publicForumTipLeaderboards: () =>
            FailedPublicForumTipLeaderboards({ error }),
        }),
        [],
      ],
      SucceededLoadPublicProductPromises: ({ promises }) => [
        evo(model, {
          publicProductPromises: () =>
            LoadedPublicProductPromises({ promises }),
        }),
        [],
      ],
      FailedLoadPublicProductPromises: ({ error }) => [
        evo(model, {
          publicProductPromises: () => FailedPublicProductPromises({ error }),
        }),
        [],
      ],
      SucceededLoadPublicPromiseTransitions: ({ transitions }) => [
        evo(model, {
          publicPromiseTransitions: () =>
            LoadedPublicPromiseTransitions({ transitions }),
        }),
        [],
      ],
      FailedLoadPublicPromiseTransitions: ({ error }) => [
        evo(model, {
          publicPromiseTransitions: () =>
            FailedPublicPromiseTransitions({ error }),
        }),
        [],
      ],
      SucceededLoadPublicTrainingRuns: ({ response, selectedRunId }) => [
        evo(model, {
          publicTrainingRuns: () =>
            LoadedPublicTrainingRuns({ response, selectedRunId }),
        }),
        [],
      ],
      FailedLoadPublicTrainingRuns: ({ error, runId }) => [
        evo(model, {
          publicTrainingRuns: () => FailedPublicTrainingRuns({ error, runId }),
        }),
        [],
      ],
      SucceededLoadShareProjection: ({ response }) => [
        evo(model, {
          shareProjection: () =>
            LoadedShareProjection({ projection: response.projection }),
        }),
        [],
      ],
      FailedLoadShareProjection: ({ error, shareId, status }) => [
        evo(model, {
          shareProjection: () =>
            FailedShareProjection({ error, shareId, status }),
        }),
        [],
      ],
      CompletedCopyShareLink: () => [model, []],
      SucceededLoadSettledFeedSnapshot: ({ cursor, summary }) => [
        evo(model, {
          settledFeed: feed => settledFeedAfterSnapshot(feed, { cursor, summary }),
        }),
        [],
      ],
      FailedLoadSettledFeedSnapshot: () => [model, []],
      OpenedSettledFeedStream: () => [
        evo(model, { settledFeed: settledFeedOpen }),
        [],
      ],
      ClosedSettledFeedStream: () => [
        evo(model, { settledFeed: settledFeedClosed }),
        [],
      ],
      FailedSettledFeedStream: () => [
        evo(model, { settledFeed: settledFeedFailed }),
        [],
      ],
      ReceivedSettledFeedPatch: ({ patch }) => [
        evo(model, {
          settledFeed: feed => applySettledFeedPatch(feed, patch),
        }),
        [],
      ],
      ReceivedSettledFeedCursorGap: ({ gap }) => [
        evo(model, {
          settledFeed: feed => settledFeedAfterCursorGap(feed, gap),
        }),
        [],
      ],
      UpdatedAutopilotOnboardingComposer: ({ value }) => [
        evo(model, {
          autopilotOnboarding: flow =>
            evo(flow, { composerDraft: () => value }),
        }),
        [],
      ],
      SubmittedAutopilotOnboardingTurn: () => {
        const flow = model.autopilotOnboarding
        const userText = flow.composerDraft.trim()

        // Guard the empty/in-flight submit on the pure path so a double-enter or
        // an empty composer never fires a turn (the composer also disables the
        // control, but the model stays authoritative).
        if (
          userText === '' ||
          flow.status === 'submitting' ||
          flow.status === 'streaming'
        ) {
          return [model, []]
        }

        // A deterministic per-turn id: the transcript length AFTER appending this
        // user turn is unique and monotonic within the session, so the streaming
        // subscription opens the SSE stream exactly once for this turn (no
        // Math.random; deterministic for captures/tests).
        const turnId = `${flow.sessionId ?? 'new'}:${flow.transcript.length + 1}`

        // The user's session id may still be null (minted at the stream
        // boundary for the first turn); persist on the handshake once it lands.
        const nextModel = evo(model, {
          autopilotOnboarding: current =>
            evo(current, {
              status: () => 'submitting',
              errorReason: () => null,
              composerDraft: () => '',
              streamingReply: () => null,
              transcript: transcript => [
                ...transcript,
                { role: 'user', content: userText },
              ],
              // Thread only the bounded vertical selector; the server owns
              // all prompt guidance for the selected vertical.
              pendingTurn: () => ({
                id: turnId,
                sessionId: current.sessionId,
                userText,
                vertical: onboardingVerticalForSegment(current.vertical),
              }),
            }),
        })

        return [
          nextModel,
          // Jump the thread to the just-sent message; the subscription opens the
          // stream and deltas land into the streaming bubble. Persist the user
          // turn so a reload before the reply lands still shows it.
          [
            ScrollAutopilotOnboardingThreadToEnd(),
            PersistAutopilotOnboarding({
              flow: nextModel.autopilotOnboarding,
            }),
          ],
        ]
      },
      OpenedAutopilotOnboardingStream: ({ turnId }) => {
        const flow = model.autopilotOnboarding
        // Ignore a stream-open for a turn that is no longer pending (a stale
        // subscription tail after the turn already resolved).
        if (flow.pendingTurn === null || flow.pendingTurn.id !== turnId) {
          return [model, []]
        }
        return [
          evo(model, {
            autopilotOnboarding: current =>
              evo(current, {
                status: () => 'streaming',
                streamingReply: () => '',
              }),
          }),
          [],
        ]
      },
      ReceivedAutopilotOnboardingStreamHandshake: ({
        turnId,
        streamId,
        sessionId,
        turnIndex,
      }) => {
        const flow = model.autopilotOnboarding
        if (flow.pendingTurn === null || flow.pendingTurn.id !== turnId) {
          return [model, []]
        }
        // The first turn mints its session id at the stream boundary; adopt the
        // handshake's session id so persistence keys the right session and a
        // mid-stream reload can resume even on the very first turn. The handshake
        // also fixes the durable cursor.
        const nextModel = evo(model, {
          autopilotOnboarding: current =>
            evo(current, {
              sessionId: () => sessionId,
              status: () => 'streaming',
              inFlight: () => ({
                streamId,
                turnIndex,
                lastOffset: null,
                resuming: false,
              }),
            }),
        })
        return [
          nextModel,
          [PersistAutopilotOnboarding({ flow: nextModel.autopilotOnboarding })],
        ]
      },
      ReceivedAutopilotOnboardingDelta: ({ turnId, text }) => {
        const flow = model.autopilotOnboarding
        if (flow.pendingTurn === null || flow.pendingTurn.id !== turnId) {
          return [model, []]
        }
        const nextModel = evo(model, {
          autopilotOnboarding: current =>
            evo(current, {
              status: () => 'streaming',
              streamingReply: reply => (reply ?? '') + text,
            }),
        })
        return [
          nextModel,
          // Keep the newest tokens in view as they land (native scroll anchoring
          // only pins when already at the bottom, so this never fights a user who
          // scrolled up). Persist the partial reply so a mid-stream reload
          // resumes from where it left off.
          [
            ScrollAutopilotOnboardingThreadToEnd(),
            PersistAutopilotOnboarding({
              flow: nextModel.autopilotOnboarding,
            }),
          ],
        ]
      },
      SucceededAutopilotOnboardingTurn: ({ response }) => {
        const nextModel = evo(model, {
          autopilotOnboarding: flow =>
            evo(flow, {
              sessionId: () => response.sessionId,
              status: () => 'idle',
              errorReason: () => null,
              turnCount: () => response.turnCount,
              outputSpec: () => response.outputSpec,
              streamingReply: () => null,
              pendingTurn: () => null,
              inFlight: () => null,
              transcript: transcript => [
                ...transcript,
                { role: 'assistant', content: response.reply },
              ],
            }),
        })
        return [
          nextModel,
          [
            ScrollAutopilotOnboardingThreadToEnd(),
            PersistAutopilotOnboarding({
              flow: nextModel.autopilotOnboarding,
            }),
          ],
        ]
      },
      FailedAutopilotOnboardingTurn: ({ reason }) => {
        const nextModel = evo(model, {
          autopilotOnboarding: flow =>
            evo(flow, {
              status: () => 'error',
              errorReason: () => (reason === '' ? null : reason),
              streamingReply: () => null,
              pendingTurn: () => null,
              inFlight: () => null,
            }),
        })
        // Persist with the cleared inFlight so a reload does not phantom-resume a
        // turn that already failed.
        return [
          nextModel,
          [PersistAutopilotOnboarding({ flow: nextModel.autopilotOnboarding })],
        ]
      },
      LoadedStoredAutopilotOnboarding: ({ session }) => {
        // Restore the saved transcript/spec/cursor IMMEDIATELY (no blank flash),
        // then reconcile with the server. If a turn was mid-stream, prime the
        // resume bubble + cursor (with `resuming` set) so the resume subscription
        // reopens the durable read.
        const wasInFlight = session.inFlight ?? null
        const nextModel = evo(model, {
          autopilotOnboarding: flow =>
            evo(flow, {
              sessionId: () => session.sessionId,
              vertical: () => session.vertical ?? flow.vertical,
              status: () => (wasInFlight === null ? 'idle' : 'streaming'),
              errorReason: () => null,
              transcript: () => session.transcript,
              outputSpec: () => session.outputSpec ?? {},
              streamingReply: () =>
                wasInFlight === null ? null : wasInFlight.replySoFar,
              pendingTurn: () => null,
              inFlight: () =>
                wasInFlight === null
                  ? null
                  : {
                      streamId: wasInFlight.streamId,
                      turnIndex: wasInFlight.turnIndex,
                      lastOffset: wasInFlight.lastOffset ?? null,
                      resuming: true,
                    },
            }),
        })
        return [
          nextModel,
          [
            ReconcileAutopilotOnboardingSession({
              sessionId: session.sessionId,
            }),
            ScrollAutopilotOnboardingThreadToEnd(),
          ],
        ]
      },
      SucceededReconcileAutopilotOnboardingSession: ({ response }) => {
        const flow = model.autopilotOnboarding
        // Only adopt the reconcile for the session we are showing.
        if (flow.sessionId !== response.sessionId) {
          return [model, []]
        }
        // Adopt the authoritative transcript/status/outputSpec/turnCount. If a
        // resume is in flight, keep the streaming bubble + resume cursor intact
        // (the resume read owns the in-flight tail); otherwise the server
        // transcript is the full truth.
        const resuming = flow.inFlight !== null && flow.inFlight.resuming
        const nextModel = evo(model, {
          autopilotOnboarding: current =>
            evo(current, {
              transcript: () => response.transcript,
              outputSpec: () => response.outputSpec,
              turnCount: () => response.turnCount,
              status: existing =>
                resuming ? existing : existing === 'error' ? existing : 'idle',
            }),
        })
        return [
          nextModel,
          resuming
            ? []
            : [
                PersistAutopilotOnboarding({
                  flow: nextModel.autopilotOnboarding,
                }),
              ],
        ]
      },
      FailedReconcileAutopilotOnboardingSession: ({ status }) => {
        // A transient network error (status 0) keeps the locally restored
        // transcript untouched.
        if (status !== 404) {
          return [model, []]
        }
        const flow = model.autopilotOnboarding
        // A 404 means the server has no PERSISTED session row for this id. That
        // is NOT always "stale/expired" — the FIRST turn's row is only written
        // when that turn FINALIZES (after the stream drains). A refresh that
        // lands mid-first-turn (or before the durable/D1 write is visible) sees
        // a 404 even though the conversation is real and resumable from the
        // durable log. Clearing here is what silently wiped the whole
        // conversation on reload (the live bug). So: if a turn was mid-stream
        // (a resuming in-flight cursor) OR we have a locally restored
        // transcript, KEEP it — the durable resume read owns the in-flight tail
        // and the local transcript is the user's real conversation; a later
        // reconcile (after the turn finalizes server-side) will adopt the
        // authoritative row. Only a 404 with NOTHING in flight and an EMPTY
        // transcript is a genuinely dead pointer worth clearing.
        const hasInFlight = flow.inFlight !== null
        const hasTranscript = flow.transcript.length > 0
        if (hasInFlight || hasTranscript) {
          return [model, []]
        }
        const nextModel = evo(model, {
          autopilotOnboarding: current =>
            evo(current, {
              sessionId: () => null,
              status: () => 'idle',
              errorReason: () => null,
              transcript: () => [],
              outputSpec: () => ({}),
              streamingReply: () => null,
              pendingTurn: () => null,
              inFlight: () => null,
              turnCount: () => 0,
            }),
        })
        return [nextModel, [ClearAutopilotOnboardingStorage()]]
      },
      ReceivedAutopilotOnboardingResumeReply: ({
        turnIndex,
        reply,
        nextOffset,
      }) => {
        const flow = model.autopilotOnboarding
        if (
          flow.inFlight === null ||
          !flow.inFlight.resuming ||
          flow.inFlight.turnIndex !== turnIndex
        ) {
          return [model, []]
        }
        // REPLACE the bubble with the accumulated reply (the replay re-streams
        // the whole turn from the offset; never append-duplicate). Advance the
        // persisted offset from the `stream-next-offset` header so a second
        // reload resumes further along.
        const nextModel = evo(model, {
          autopilotOnboarding: current =>
            evo(current, {
              status: () => 'streaming',
              streamingReply: () => reply,
              inFlight: held =>
                held === null ? held : { ...held, lastOffset: nextOffset },
            }),
        })
        return [
          nextModel,
          [
            ScrollAutopilotOnboardingThreadToEnd(),
            PersistAutopilotOnboarding({
              flow: nextModel.autopilotOnboarding,
            }),
          ],
        ]
      },
      SucceededResumeAutopilotOnboardingTurn: ({ response }) => {
        const flow = model.autopilotOnboarding
        if (flow.sessionId !== response.sessionId) {
          return [model, []]
        }
        // The resumed turn completed: commit the final assistant message and
        // clear the resume/in-flight state. The transcript already holds the
        // user turn (from the reconcile / restore); append the assistant reply
        // only if the last turn is not already this assistant reply (the
        // reconcile may have adopted a completed transcript).
        const lastTurn = flow.transcript[flow.transcript.length - 1]
        const alreadyCommitted =
          lastTurn !== undefined &&
          lastTurn.role === 'assistant' &&
          lastTurn.content === response.reply
        const nextModel = evo(model, {
          autopilotOnboarding: current =>
            evo(current, {
              status: () => 'idle',
              errorReason: () => null,
              turnCount: () => response.turnCount,
              outputSpec: () => response.outputSpec,
              streamingReply: () => null,
              inFlight: () => null,
              transcript: transcript =>
                alreadyCommitted
                  ? transcript
                  : [
                      ...transcript,
                      { role: 'assistant', content: response.reply },
                    ],
            }),
        })
        return [
          nextModel,
          [
            ScrollAutopilotOnboardingThreadToEnd(),
            PersistAutopilotOnboarding({
              flow: nextModel.autopilotOnboarding,
            }),
          ],
        ]
      },
      ClosedAutopilotOnboardingResumeStream: ({ turnIndex }) =>
        // The durable log ended without a `done` frame. Stop resuming and fall
        // back to the reconciled transcript (which the reconcile already
        // adopted) so no stuck half-bubble remains.
        endResume(model, turnIndex),
      FailedAutopilotOnboardingResume: ({ turnIndex }) =>
        // The resume read 404'd (durable log gone / TTL expired): fall back to
        // the reconciled transcript without leaving a stuck half-bubble.
        endResume(model, turnIndex),
      ClickedAutopilotOnboardingStartOver: () => {
        // Drop the in-memory flow + the stored session (preserve the route's
        // vertical so the page stays on the same onboarding lane).
        const nextModel = evo(model, {
          autopilotOnboarding: flow =>
            evo(flow, {
              sessionId: () => null,
              composerDraft: () => '',
              status: () => 'idle',
              errorReason: () => null,
              transcript: () => [],
              outputSpec: () => ({}),
              streamingReply: () => null,
              pendingTurn: () => null,
              inFlight: () => null,
              turnCount: () => 0,
            }),
        })
        return [nextModel, [ClearAutopilotOnboardingStorage()]]
      },
      CompletedPersistAutopilotOnboarding: () => [model, []],
      ClickedAutopilotOnboardingCreditKickoff: () => [
        model,
        [OpenAutopilotCreditKickoff()],
      ],
      CompletedAutopilotOnboardingCreditKickoff: () => [model, []],
      CompletedScrollAutopilotOnboardingThread: () => [model, []],
    }),
  )
