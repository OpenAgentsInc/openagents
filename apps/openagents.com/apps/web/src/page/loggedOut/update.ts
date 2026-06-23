import { Effect, Match as M, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { load, pushUrl } from 'foldkit/navigation'
import { evo } from 'foldkit/struct'

import {
  CompletedCopyAgentInstructions,
  CompletedCopyShareLink,
  CompletedNavigateToKhala,
  CompletedNavigateToLanding,
  CompletedNavigateToTassadar,
  CompletedAutopilotOnboardingCreditKickoff,
  FailedAutopilotOnboardingTurn,
  SucceededAutopilotOnboardingTurn,
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
import { OnboardingTurnResponse } from '../autopilot-onboarding/flow'
import { verticalOverlayForSegment } from '../autopilot-onboarding/vertical-overlay'
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

class AutopilotOnboardingTurnError extends S.TaggedErrorClass<AutopilotOnboardingTurnError>()(
  'AutopilotOnboardingTurnError',
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
      const record =
        typeof payload === 'object' && payload !== null
          ? (payload as Record<string, unknown>)
          : {}
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
// Math.random. Lives in the command (effect side) so the pure update path stays
// deterministic.
const newOnboardingSessionId = (): string => {
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

// Drive one /autopilot onboarding turn against the merged Khala program
// (`POST /api/autopilot/onboarding/{sessionId}/turn`, #6126). `sessionId` is the
// existing session or null (a fresh one is minted here). `verticalOverlay` is
// only honored by the server on the first turn that creates the session.
export const SubmitAutopilotOnboardingTurn = Command.define(
  'SubmitAutopilotOnboardingTurn',
  {
    sessionId: S.NullOr(S.String),
    userText: S.String,
    verticalOverlay: S.NullOr(S.String),
  },
  SucceededAutopilotOnboardingTurn,
  FailedAutopilotOnboardingTurn,
)(({ sessionId, userText, verticalOverlay }) =>
  Effect.gen(function* () {
    const resolvedSessionId =
      sessionId ?? (yield* Effect.sync(() => newOnboardingSessionId()))

    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(
          `/api/autopilot/onboarding/${encodeURIComponent(resolvedSessionId)}/turn`,
          {
            method: 'POST',
            cache: 'no-store',
            headers: {
              accept: 'application/json',
              'content-type': 'application/json',
            },
            body: JSON.stringify({
              userText,
              ...(verticalOverlay === null ? {} : { verticalOverlay }),
            }),
          },
        ),
      catch: error => new AutopilotOnboardingTurnError({ error }),
    })

    if (!response.ok) {
      return yield* new AutopilotOnboardingTurnError({
        error: `Onboarding turn returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new AutopilotOnboardingTurnError({ error }),
    })
    const decoded = yield* S.decodeUnknownEffect(OnboardingTurnResponse)(payload)

    return SucceededAutopilotOnboardingTurn({ response: decoded })
  }).pipe(
    Effect.catch(() =>
      Effect.succeed(
        FailedAutopilotOnboardingTurn({
          reason:
            'Autopilot could not respond just now. Try sending that again.',
        }),
      ),
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
  model.route._tag === 'Share'
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
        if (userText === '' || flow.status === 'submitting') {
          return [model, []]
        }

        return [
          evo(model, {
            autopilotOnboarding: current =>
              evo(current, {
                status: () => 'submitting',
                errorReason: () => null,
                composerDraft: () => '',
                transcript: transcript => [
                  ...transcript,
                  { role: 'user', content: userText },
                ],
              }),
          }),
          [
            // Thread the legal SYSTEM-PROMPT OVERLAY (not the raw `legal`
            // segment) to the program's vertical-overlay slot. The server only
            // honors it on the first turn that creates the session; non-legal /
            // absent verticals resolve to null so the generic intake carries no
            // overlay (#6130).
            SubmitAutopilotOnboardingTurn({
              sessionId: flow.sessionId,
              userText,
              verticalOverlay: verticalOverlayForSegment(flow.vertical),
            }),
          ],
        ]
      },
      SucceededAutopilotOnboardingTurn: ({ response }) => [
        evo(model, {
          autopilotOnboarding: flow =>
            evo(flow, {
              sessionId: () => response.sessionId,
              status: () => 'idle',
              errorReason: () => null,
              turnCount: () => response.turnCount,
              outputSpec: () => response.outputSpec,
              transcript: transcript => [
                ...transcript,
                { role: 'assistant', content: response.reply },
              ],
            }),
        }),
        [],
      ],
      FailedAutopilotOnboardingTurn: ({ reason }) => [
        evo(model, {
          autopilotOnboarding: flow =>
            evo(flow, {
              status: () => 'error',
              errorReason: () => (reason === '' ? null : reason),
            }),
        }),
        [],
      ],
      ClickedAutopilotOnboardingCreditKickoff: () => [
        model,
        [OpenAutopilotCreditKickoff()],
      ],
      CompletedAutopilotOnboardingCreditKickoff: () => [model, []],
    }),
  )
