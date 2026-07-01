import { PublicActivityTimelineEnvelope } from '@openagentsinc/public-activity-timeline'
import {
  Array as Arr,
  Clock,
  Effect,
  Match as M,
  Option,
  Schema as S,
} from 'effect'
import { Command, Dom } from 'foldkit'
import { load, pushUrl } from 'foldkit/navigation'
import { evo } from 'foldkit/struct'

import {
  clearSessionFromStore,
  sessionStoreLayer,
} from '../../commands/session-store'
import { recordFromUnknown } from '../../json-boundary'
import { homeRouter, khalaRouter, tassadarRouter } from '../../route'
import { FlowModel } from '../autopilot-onboarding/flow'
import { HUD_THREAD_END_SELECTOR } from '../autopilot-onboarding/page'
import {
  OnboardingSessionResponse,
  type StoredInFlight,
  type StoredOnboardingSession,
  clearStoredSession,
  readStoredSession,
  storedSessionFromParts,
  writeStoredSession,
} from '../autopilot-onboarding/persistence'
import { onboardingVerticalForSegment } from '../autopilot-onboarding/vertical-overlay'
import {
  KHALA_CHAT_COMPOSER_TEXTAREA_SELECTOR,
  KHALA_CHAT_LATEST_TURN_SELECTOR,
  KHALA_CHAT_THREAD_END_SELECTOR,
} from '../khala-chat/page'
import {
  BlobRef as AtifBlobRef,
  Trajectory as AtifTrajectory,
} from '../trace/atif'
import { SAMPLE_TRACE_UUID } from '../trace/sample'
import {
  setGymConcurrency,
  setGymFanoutMode,
  setGymMaxTokens,
  setGymModuleComposition,
  setGymReasoningEffort,
  setGymSamplesPerCell,
  setGymTemperature,
  setGymToolSet,
  setGymTransport,
  toggleGymCoordinator,
  toggleGymLane,
  toggleGymSequenceShape,
} from './gym/flow'
import {
  GymRunProgressPublicProjection,
  GymRunProgressResponse,
} from './gym/runProgress'
import {
  GYM_RUN_PROGRESS_SCOPE,
  applyGymRunProgressPatch,
  gymRunProgressAfterSnapshot,
  gymRunProgressStreamAfterCursorGap,
  gymRunProgressStreamClosed,
  gymRunProgressStreamFailed,
  gymRunProgressStreamOpen,
} from './gym/runProgressFeed'
import {
  KHALA_TOKENS_SERVED_SCOPE,
  applyKhalaTokensServedPatch,
  khalaTokensServedAfterScalarSeed,
  khalaTokensServedAfterSnapshot,
  khalaTokensServedStreamAfterCursorGap,
  khalaTokensServedStreamClosed,
  khalaTokensServedStreamFailed,
  khalaTokensServedStreamOpen,
  khalaTokensServedStreamSnapshotSettled,
} from './khala-tokens-served-feed'
import {
  CompletedAutopilotOnboardingCreditKickoff,
  CompletedCopyAgentInstructions,
  CompletedCopyShareLink,
  CompletedFocusKhalaChatComposer,
  CompletedLandingLogout,
  CompletedNavigateToKhala,
  CompletedNavigateToLanding,
  CompletedNavigateToTassadar,
  CompletedPersistAutopilotOnboarding,
  CompletedScrollAutopilotOnboardingThread,
  CompletedScrollKhalaChatThread,
  FailedLoadGymRunProgressSnapshot,
  FailedLoadKhalaTokensServedSnapshot,
  FailedLoadMirrorCodeRuns,
  FailedLoadPublicActivityTimeline,
  FailedLoadPublicAdjutantActivity,
  FailedLoadPublicAgentGoal,
  FailedLoadPublicArtanisReport,
  FailedLoadPublicForumLaunchStatus,
  FailedLoadPublicForumTipLeaderboards,
  FailedLoadPublicGymRunProgress,
  FailedLoadPublicKhalaTokensServed,
  FailedLoadPublicKhalaTokensServedChannelMix,
  FailedLoadPublicKhalaTokensServedHistory,
  FailedLoadPublicKhalaTokensServedModelMix,
  FailedLoadPublicProductPromises,
  FailedLoadPublicPromiseTransitions,
  FailedLoadPublicPylonStats,
  FailedLoadPublicTrainingRuns,
  FailedLoadSettledFeedSnapshot,
  FailedLoadShareProjection,
  FailedLoadTrace,
  FailedReconcileAutopilotOnboardingSession,
  LoadedStoredAutopilotOnboarding,
  Message,
  SucceededLoadGymRunProgressSnapshot,
  SucceededLoadKhalaTokensServedSnapshot,
  SucceededLoadMirrorCodeRuns,
  SucceededLoadPublicActivityTimeline,
  SucceededLoadPublicAdjutantActivity,
  SucceededLoadPublicAgentGoal,
  SucceededLoadPublicArtanisReport,
  SucceededLoadPublicForumLaunchStatus,
  SucceededLoadPublicForumTipLeaderboards,
  SucceededLoadPublicGymRunProgress,
  SucceededLoadPublicKhalaTokensServed,
  SucceededLoadPublicKhalaTokensServedChannelMix,
  SucceededLoadPublicKhalaTokensServedHistory,
  SucceededLoadPublicKhalaTokensServedModelMix,
  SucceededLoadPublicProductPromises,
  SucceededLoadPublicPromiseTransitions,
  SucceededLoadPublicPylonStats,
  SucceededLoadPublicTrainingRuns,
  SucceededLoadSettledFeedSnapshot,
  SucceededLoadShareProjection,
  SucceededLoadTrace,
  SucceededReconcileAutopilotOnboardingSession,
} from './message'
import { MirrorCodeRunsResponse } from './mirrorcode/runs'
import {
  FailedMirrorCodeRuns,
  FailedPublicActivityTimeline,
  FailedPublicAdjutantActivity,
  FailedPublicAgent,
  FailedPublicArtanisReport,
  FailedPublicForumLaunchStatus,
  FailedPublicForumTipLeaderboards,
  FailedPublicGymRunProgress,
  FailedPublicKhalaTokensServed,
  FailedPublicKhalaTokensServedChannelMix,
  FailedPublicKhalaTokensServedHistory,
  FailedPublicKhalaTokensServedModelMix,
  FailedPublicProductPromises,
  FailedPublicPromiseTransitions,
  FailedPublicPylonStats,
  FailedPublicTrainingRuns,
  FailedShareProjection,
  FailedTrace,
  LoadedMirrorCodeRuns,
  LoadedPublicActivityTimeline,
  LoadedPublicAdjutantActivity,
  LoadedPublicAgent,
  LoadedPublicArtanisReport,
  LoadedPublicForumLaunchStatus,
  LoadedPublicForumTipLeaderboards,
  LoadedPublicGymRunProgress,
  LoadedPublicKhalaTokensServedHistory,
  LoadedPublicKhalaTokensServedChannelMix,
  LoadedPublicKhalaTokensServedModelMix,
  LoadedPublicProductPromises,
  LoadedPublicPromiseTransitions,
  LoadedPublicPylonStats,
  LoadedPublicTrainingRuns,
  LoadedShareProjection,
  LoadedTrace,
  Model,
  NotFoundTrace,
  PublicAdjutantActivity,
  PublicAgentGoalResponse,
  PublicArtanisReport,
  PublicForumLaunchStatus,
  PublicForumTipLeaderboards,
  PublicKhalaTokensServed,
  PublicKhalaTokensServedChannelMix,
  PublicKhalaTokensServedHistory,
  PublicKhalaTokensServedModelMix,
  PublicProductPromises,
  PublicPromiseTransitions,
  PublicPylonStats,
  PublicTrainingRunResponse,
  PublicTrainingRunsResponse,
  ShareProjectionResponse,
} from './model'
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

class PublicKhalaTokensServedLoadError extends S.TaggedErrorClass<PublicKhalaTokensServedLoadError>()(
  'PublicKhalaTokensServedLoadError',
  { error: S.Defect },
) {}

class PublicKhalaTokensServedHistoryLoadError extends S.TaggedErrorClass<PublicKhalaTokensServedHistoryLoadError>()(
  'PublicKhalaTokensServedHistoryLoadError',
  { error: S.Defect },
) {}

class PublicKhalaTokensServedModelMixLoadError extends S.TaggedErrorClass<PublicKhalaTokensServedModelMixLoadError>()(
  'PublicKhalaTokensServedModelMixLoadError',
  { error: S.Defect },
) {}

class PublicKhalaTokensServedChannelMixLoadError extends S.TaggedErrorClass<PublicKhalaTokensServedChannelMixLoadError>()(
  'PublicKhalaTokensServedChannelMixLoadError',
  { error: S.Defect },
) {}

class PublicGymRunProgressLoadError extends S.TaggedErrorClass<PublicGymRunProgressLoadError>()(
  'PublicGymRunProgressLoadError',
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

class PublicActivityTimelineLoadError extends S.TaggedErrorClass<PublicActivityTimelineLoadError>()(
  'PublicActivityTimelineLoadError',
  { error: S.Defect },
) {}

class ShareProjectionLoadError extends S.TaggedErrorClass<ShareProjectionLoadError>()(
  'ShareProjectionLoadError',
  {
    error: S.Defect,
    status: S.Int,
  },
) {}

class TraceLoadError extends S.TaggedErrorClass<TraceLoadError>()(
  'TraceLoadError',
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

// Live fleet-shipping feed (#6534). Reads the same read-only public activity
// timeline the /activity surface reads and renders TODAY's live fleet work on
// /artanis. Bounded to a small recent window so the page stays fast.
export const LoadPublicActivityTimeline = Command.define(
  'LoadPublicActivityTimeline',
  SucceededLoadPublicActivityTimeline,
  FailedLoadPublicActivityTimeline,
)(
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch('/api/public/activity-timeline?limit=60', {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new PublicActivityTimelineLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicActivityTimelineLoadError({
        error: `Public activity timeline returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new PublicActivityTimelineLoadError({ error }),
    })
    const envelope = yield* S.decodeUnknownEffect(
      PublicActivityTimelineEnvelope,
    )(payload)

    return SucceededLoadPublicActivityTimeline({ envelope })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicActivityTimeline({
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

// "Khala Tokens Served" homepage counter (#6227). Cold-reads the public-safe
// aggregate; the poll subscription re-runs this every few seconds so the
// odometer count-up animates between fetched totals. Public read: no-store, no
// auth, aggregate only.
export const LoadPublicKhalaTokensServed = Command.define(
  'LoadPublicKhalaTokensServed',
  SucceededLoadPublicKhalaTokensServed,
  FailedLoadPublicKhalaTokensServed,
)(
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch('/api/public/khala-tokens-served', {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new PublicKhalaTokensServedLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicKhalaTokensServedLoadError({
        error: `Public Khala tokens served returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new PublicKhalaTokensServedLoadError({ error }),
    })
    const decoded = yield* S.decodeUnknownEffect(PublicKhalaTokensServed)(
      payload,
    )

    return SucceededLoadPublicKhalaTokensServed({ served: decoded })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicKhalaTokensServed({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

class KhalaTokensServedSnapshotLoadError extends S.TaggedErrorClass<KhalaTokensServedSnapshotLoadError>()(
  'KhalaTokensServedSnapshotLoadError',
  {
    error: S.Defect,
  },
) {}

const KhalaTokensServedSnapshotPayload = S.Struct({
  collections: S.Record(S.String, S.Record(S.String, S.Unknown)),
  cursor: S.Number,
})

const KhalaTokensServedSnapshotSummary = S.Struct({
  observedAt: S.String,
  tokensServedTotal: S.Number,
})

// "Khala Tokens Served" snapshot seed (#6231 follow-up). ONE read of the public
// tokens-served sync scope returns the AUTHORITATIVE running total (the room's
// `summary` record) + the cursor. The client seeds from this and subscribes
// strictly from that cursor, so events already baked into the seeded total are
// never replayed-and-re-added — no double-count, no backward jump. When the
// summary is absent (a brand-new scope with no served events yet) the scalar
// `LoadPublicKhalaTokensServed` below still seeds the displayed total.
export const LoadKhalaTokensServedSnapshot = Command.define(
  'LoadKhalaTokensServedSnapshot',
  SucceededLoadKhalaTokensServedSnapshot,
  FailedLoadKhalaTokensServedSnapshot,
)(
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(
          `/api/sync/${KHALA_TOKENS_SERVED_SCOPE.replace(':', '/')}/snapshot`,
          {
            cache: 'no-store',
            headers: { accept: 'application/json' },
          },
        ),
      catch: error => new KhalaTokensServedSnapshotLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new KhalaTokensServedSnapshotLoadError({
        error: `Khala tokens served snapshot returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new KhalaTokensServedSnapshotLoadError({ error }),
    })
    const decoded = yield* S.decodeUnknownEffect(
      KhalaTokensServedSnapshotPayload,
    )(payload)
    const rawSummary = decoded.collections['tokens_served_summary']?.['summary']
    const summary =
      rawSummary === undefined
        ? null
        : yield* S.decodeUnknownEffect(KhalaTokensServedSnapshotSummary)(
            rawSummary,
          ).pipe(Effect.orElseSucceed(() => null))

    return SucceededLoadKhalaTokensServedSnapshot({
      cursor: decoded.cursor,
      summary,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadKhalaTokensServedSnapshot({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

// "Khala Tokens Served" history (#6227). Cold-reads the public-safe per-day
// series for the /stats chart; the poll subscription re-runs this every few
// seconds alongside the scalar counter. Public read: no-store, no auth,
// aggregate-only (bare day + sum) series.
export const LoadPublicKhalaTokensServedHistory = Command.define(
  'LoadPublicKhalaTokensServedHistory',
  SucceededLoadPublicKhalaTokensServedHistory,
  FailedLoadPublicKhalaTokensServedHistory,
)(
  Effect.gen(function* () {
    const search = new URLSearchParams({
      bucket: 'day',
      timezone: 'America/Chicago',
      window: '30d',
    })
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(`/api/public/khala-tokens-served/history?${search.toString()}`, {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new PublicKhalaTokensServedHistoryLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicKhalaTokensServedHistoryLoadError({
        error: `Public Khala tokens served history returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new PublicKhalaTokensServedHistoryLoadError({ error }),
    })
    const decoded = yield* S.decodeUnknownEffect(
      PublicKhalaTokensServedHistory,
    )(payload)

    return SucceededLoadPublicKhalaTokensServedHistory({ history: decoded })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicKhalaTokensServedHistory({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

export const LoadPublicKhalaTokensServedModelMix = Command.define(
  'LoadPublicKhalaTokensServedModelMix',
  SucceededLoadPublicKhalaTokensServedModelMix,
  FailedLoadPublicKhalaTokensServedModelMix,
)(
  Effect.gen(function* () {
    const search = new URLSearchParams({ window: '30d' })
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(
          `/api/public/khala-tokens-served/model-mix?${search.toString()}`,
          {
            cache: 'no-store',
            headers: { accept: 'application/json' },
          },
        ),
      catch: error => new PublicKhalaTokensServedModelMixLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicKhalaTokensServedModelMixLoadError({
        error: `Public Khala tokens served model mix returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new PublicKhalaTokensServedModelMixLoadError({ error }),
    })
    const decoded = yield* S.decodeUnknownEffect(
      PublicKhalaTokensServedModelMix,
    )(payload)

    return SucceededLoadPublicKhalaTokensServedModelMix({ mix: decoded })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicKhalaTokensServedModelMix({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

export const LoadPublicKhalaTokensServedChannelMix = Command.define(
  'LoadPublicKhalaTokensServedChannelMix',
  SucceededLoadPublicKhalaTokensServedChannelMix,
  FailedLoadPublicKhalaTokensServedChannelMix,
)(
  Effect.gen(function* () {
    const search = new URLSearchParams({ window: '30d' })
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(
          `/api/public/khala-tokens-served/channel-mix?${search.toString()}`,
          {
            cache: 'no-store',
            headers: { accept: 'application/json' },
          },
        ),
      catch: error =>
        new PublicKhalaTokensServedChannelMixLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicKhalaTokensServedChannelMixLoadError({
        error: `Public tokens served channel mix returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error =>
        new PublicKhalaTokensServedChannelMixLoadError({ error }),
    })
    const decoded = yield* S.decodeUnknownEffect(
      PublicKhalaTokensServedChannelMix,
    )(payload)

    return SucceededLoadPublicKhalaTokensServedChannelMix({ mix: decoded })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicKhalaTokensServedChannelMix({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

// Live Gym / Harbor run-progress follow-along (#6261). Cold-reads the
// public-safe `{ runs: [...] }` projection for the `/gym` follow-along; the poll
// subscription re-runs this on a ~12s cadence. Public read: no-store, no auth,
// already redacted (counts/denominators/refs only). Decodes through the
// `GymRunProgressResponse` envelope (tolerating the staleness/scope/generatedAt
// fields) and surfaces just the runs.
export const LoadPublicGymRunProgress = Command.define(
  'LoadPublicGymRunProgress',
  SucceededLoadPublicGymRunProgress,
  FailedLoadPublicGymRunProgress,
)(
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch('/api/public/gym/run-progress', {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new PublicGymRunProgressLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new PublicGymRunProgressLoadError({
        error: `Public Gym run progress returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new PublicGymRunProgressLoadError({ error }),
    })
    const decoded = yield* S.decodeUnknownEffect(GymRunProgressResponse)(
      payload,
    )

    return SucceededLoadPublicGymRunProgress({ runs: decoded.runs })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadPublicGymRunProgress({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

class MirrorCodeRunsLoadError extends S.TaggedErrorClass<MirrorCodeRunsLoadError>()(
  'MirrorCodeRunsLoadError',
  {
    error: S.Defect,
  },
) {}

// MirrorCode runs (#6378). Cold-reads the public-safe MirrorCode runs
// projection for the `/mirrorcode` page on route entry. Public read: no-store,
// no auth, already redacted. Decodes through `MirrorCodeRunsResponse` (excess
// envelope fields are tolerated) and surfaces the whole response.
export const LoadMirrorCodeRuns = Command.define(
  'LoadMirrorCodeRuns',
  SucceededLoadMirrorCodeRuns,
  FailedLoadMirrorCodeRuns,
)(
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch('/api/gym/mirrorcode/runs', {
          cache: 'no-store',
          headers: { accept: 'application/json' },
        }),
      catch: error => new MirrorCodeRunsLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new MirrorCodeRunsLoadError({
        error: `Public MirrorCode runs returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new MirrorCodeRunsLoadError({ error }),
    })
    const decoded = yield* S.decodeUnknownEffect(MirrorCodeRunsResponse)(
      payload,
    )

    return SucceededLoadMirrorCodeRuns({ response: decoded })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadMirrorCodeRuns({
          error: error instanceof Error ? error.message : String(error),
        }),
      ),
    ),
  ),
)

class GymRunProgressSnapshotLoadError extends S.TaggedErrorClass<GymRunProgressSnapshotLoadError>()(
  'GymRunProgressSnapshotLoadError',
  {
    error: S.Defect,
  },
) {}

const GymRunProgressSnapshotPayload = S.Struct({
  collections: S.Record(S.String, S.Record(S.String, S.Unknown)),
  cursor: S.Number,
})

// Gym run-progress snapshot seed (#6261). ONE read of the public gym
// run-progress sync scope returns each run's latest public-safe projection
// (collapsed by `runRef`) + the cursor. The client seeds the panel from this and
// subscribes strictly from that cursor, so a put already baked into the seed is
// never replayed into a duplicate card. An empty/absent collection seeds an
// honest empty panel (no runs yet) rather than an error.
export const LoadGymRunProgressSnapshot = Command.define(
  'LoadGymRunProgressSnapshot',
  SucceededLoadGymRunProgressSnapshot,
  FailedLoadGymRunProgressSnapshot,
)(
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(
          `/api/sync/${GYM_RUN_PROGRESS_SCOPE.replace(':', '/')}/snapshot`,
          {
            cache: 'no-store',
            headers: { accept: 'application/json' },
          },
        ),
      catch: error => new GymRunProgressSnapshotLoadError({ error }),
    })

    if (!response.ok) {
      return yield* new GymRunProgressSnapshotLoadError({
        error: `Gym run progress snapshot returned HTTP ${response.status}.`,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new GymRunProgressSnapshotLoadError({ error }),
    })
    const decoded = yield* S.decodeUnknownEffect(GymRunProgressSnapshotPayload)(
      payload,
    )
    const collection = decoded.collections['gym_run_progress'] ?? {}
    const maybeRuns = yield* Effect.forEach(Object.values(collection), value =>
      S.decodeUnknownEffect(GymRunProgressPublicProjection)(value).pipe(
        Effect.map(run => Option.some<GymRunProgressPublicProjection>(run)),
        Effect.orElseSucceed(() =>
          Option.none<GymRunProgressPublicProjection>(),
        ),
      ),
    )
    const runs = Arr.getSomes(maybeRuns)

    return SucceededLoadGymRunProgressSnapshot({
      cursor: decoded.cursor,
      runs,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadGymRunProgressSnapshot({
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
    const decoded = yield* S.decodeUnknownEffect(SettledFeedSnapshotPayload)(
      payload,
    )
    const rawSummary = decoded.collections['settled_summary']?.['summary']
    const summary =
      rawSummary === undefined
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
    const decoded = yield* S.decodeUnknownEffect(PublicPromiseTransitions)(
      payload,
    )

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

// Live `/trace/{uuid}` read (issue #6209). Fetches the visibility-gated read API
// and decodes the response's public-safe ATIF projection at `.trace.trajectory`
// — the exact `AtifTrajectory` shape the page already renders. A clean 404
// (trace not found, or owner_only to an anonymous viewer, which the worker
// reports as 404 by design) surfaces with `status: 404` so the page shows the
// honest not-found body; any other failure (network, 5xx, decode) surfaces with
// its status (or 0) and renders the same not-found body.
export const LoadTrace = Command.define(
  'LoadTrace',
  { uuid: S.String },
  SucceededLoadTrace,
  FailedLoadTrace,
)(({ uuid }) =>
  Effect.gen(function* () {
    // Read-scope token (mobile "Open traces in web", #6347): when the owner
    // opens `/trace/{uuid}?token=<oa_agent_…>` from the Khala app, forward the
    // token to the visibility-gated read API so an owner_only trace resolves
    // without a web login. Read-only: the token only grants owner-scoped trace
    // READ. A normal shared link carries no token and is unaffected.
    const maybeToken = new URLSearchParams(window.location.search).get('token')
    const tokenQuery =
      maybeToken !== null && maybeToken.startsWith('oa_agent_')
        ? `?token=${encodeURIComponent(maybeToken)}`
        : ''
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(`/api/traces/${encodeURIComponent(uuid)}${tokenQuery}`, {
          cache: 'no-store',
          credentials: 'include',
          headers: { accept: 'application/json' },
        }),
      catch: error => new TraceLoadError({ error, status: 0 }),
    })

    if (!response.ok) {
      return yield* new TraceLoadError({
        error: `Trace returned HTTP ${response.status}.`,
        status: response.status,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => new TraceLoadError({ error, status: 0 }),
    })
    // The read API wraps the public-safe projection as `{ trace: { ... } }`; the
    // page renders the decoded ATIF trajectory at `.trace.trajectory`.
    const record = recordFromUnknown(payload)
    const traceField =
      record !== undefined && typeof record.trace === 'object'
        ? record.trace
        : undefined
    const traceRecord = recordFromUnknown(traceField)
    if (traceRecord === undefined) {
      return yield* new TraceLoadError({
        error: 'Trace response missing the trace projection.',
        status: 0,
      })
    }
    const trajectory = yield* S.decodeUnknownEffect(AtifTrajectory)(
      traceRecord.trajectory,
    )

    // #6223: the envelope also carries public-safe blob refs (R2 keys for the
    // recording + screenshots). Decode leniently — a trace with none, or an
    // older envelope without the field, still loads (the page omits the media).
    const blobRefs = yield* S.decodeUnknownEffect(S.Array(AtifBlobRef))(
      traceRecord.blobRefs ?? [],
    ).pipe(Effect.orElseSucceed(() => [] as ReadonlyArray<AtifBlobRef>))

    return SucceededLoadTrace(
      blobRefs.length === 0
        ? { uuid, trajectory }
        : { uuid, trajectory, blobRefs },
    )
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadTrace({
          uuid,
          error:
            error instanceof TraceLoadError
              ? String(error.error)
              : error instanceof Error
                ? error.message
                : String(error),
          status: error instanceof TraceLoadError ? error.status : 0,
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
  '  -d \'{"displayName": "YOUR_AGENT_NAME", "slug": "your-agent-name"}\'',
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

// The single logout endpoint, identical to the public header's wire
// (src/update.ts RequestedLoggedOutLogout -> LoadExternal '/auth/logout').
const LOGOUT_HREF = '/auth/logout'

// Log out from the homepage hero's floating avatar menu. Reuses the SAME logout
// behavior the public header uses: clear the cached session, then full-page
// navigate to `/auth/logout` (the server route that clears the cookie). `load`
// is a full-page navigation, so the completion message is effectively never
// observed — it exists only to type the command.
export const LogoutFromLanding = Command.define(
  'LogoutFromLanding',
  CompletedLandingLogout,
)(
  clearSessionFromStore.pipe(
    Effect.provide(sessionStoreLayer),
    Effect.catch(() => Effect.void),
    Effect.andThen(load(LOGOUT_HREF)),
    Effect.as(CompletedLandingLogout()),
  ),
)

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

// Scroll the generic /khala chat thread's bottom sentinel into view after an
// explicit "latest" request. Streaming deltas never call this: the reader keeps
// their place unless they asked to move.
export const ScrollKhalaChatThreadToEnd = Command.define(
  'ScrollKhalaChatThreadToEnd',
  CompletedScrollKhalaChatThread,
)(
  Dom.scrollIntoViewAfterPaint(KHALA_CHAT_THREAD_END_SELECTOR, {
    block: 'end',
  }).pipe(
    Effect.as(CompletedScrollKhalaChatThread()),
    Effect.catch(() => Effect.succeed(CompletedScrollKhalaChatThread())),
  ),
)

// A submitted turn is explicit reader intent. Put that turn near the top of the
// transcript viewport so the answer can stream into the space below it.
export const ScrollKhalaChatLatestTurnIntoView = Command.define(
  'ScrollKhalaChatLatestTurnIntoView',
  CompletedScrollKhalaChatThread,
)(
  Dom.scrollIntoViewAfterPaint(KHALA_CHAT_LATEST_TURN_SELECTOR, {
    block: 'start',
  }).pipe(
    Effect.as(CompletedScrollKhalaChatThread()),
    Effect.catch(() => Effect.succeed(CompletedScrollKhalaChatThread())),
  ),
)

export const FocusKhalaChatComposer = Command.define(
  'FocusKhalaChatComposer',
  CompletedFocusKhalaChatComposer,
)(
  Dom.focus(KHALA_CHAT_COMPOSER_TEXTAREA_SELECTOR, {
    preventScroll: true,
  }).pipe(Effect.ignore, Effect.as(CompletedFocusKhalaChatComposer())),
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
const endResume = (model: Model, turnIndex: number): UpdateReturn => {
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
    const decoded = yield* S.decodeUnknownEffect(OnboardingSessionResponse)(
      payload,
    )

    return SucceededReconcileAutopilotOnboardingSession({ response: decoded })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedReconcileAutopilotOnboardingSession({
          status:
            error instanceof OnboardingSessionReconcileError ? error.status : 0,
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
      : model.route._tag === 'Trace' && model.route.uuid !== SAMPLE_TRACE_UUID
        ? [LoadTrace({ uuid: model.route.uuid })]
        : model.route._tag === 'Home'
          ? [
              LoadPublicPylonStats(),
              LoadKhalaTokensServedSnapshot(),
              LoadPublicKhalaTokensServed(),
              LoadPublicKhalaTokensServedHistory(),
              LoadPublicForumLaunchStatus(),
              LoadPublicForumTipLeaderboards(),
              LoadSettledFeedSnapshot(),
            ]
          : model.route._tag === 'Stats' ||
              model.route._tag === 'PublicStatsArchive'
            ? [
                LoadPublicPylonStats(),
                LoadKhalaTokensServedSnapshot(),
                LoadPublicKhalaTokensServed(),
                LoadPublicKhalaTokensServedHistory(),
                LoadPublicKhalaTokensServedModelMix(),
                LoadPublicKhalaTokensServedChannelMix(),
                LoadPublicForumLaunchStatus(),
                LoadPublicForumTipLeaderboards(),
                LoadSettledFeedSnapshot(),
              ]
            : model.route._tag === 'Khala' || model.route._tag === 'Landing'
              ? // /khala AND the / landing hero both show the live "Khala Tokens
                // Served" total (the landing top-left pill mirrors the /khala
                // counter), so both seed from the SAME snapshot + scalar endpoints and
                // subscribe to the SAME live stream below — no parallel data source.
                [LoadKhalaTokensServedSnapshot(), LoadPublicKhalaTokensServed()]
              : model.route._tag === 'Gym'
                ? [LoadGymRunProgressSnapshot(), LoadPublicGymRunProgress()]
                : model.route._tag === 'MirrorCode'
                  ? [LoadMirrorCodeRuns()]
                  : model.route._tag === 'ProductPromises'
                    ? [
                        LoadPublicProductPromises(),
                        LoadPublicPromiseTransitions(),
                      ]
                    : model.route._tag === 'PublicTrainingRuns'
                      ? [LoadPublicTrainingRuns({ runId: null })]
                      : model.route._tag === 'PublicTrainingRun'
                        ? [LoadPublicTrainingRuns({ runId: model.route.runId })]
                        : model.route._tag === 'PublicAgent'
                          ? model.route.agentRef === 'artanis'
                            ? [
                                LoadPublicAgentGoal({
                                  agentId: publicAgentIdForRef(
                                    model.route.agentRef,
                                  ),
                                  agentRef: model.route.agentRef,
                                }),
                                LoadPublicActivityTimeline(),
                                LoadPublicPylonStats(),
                                LoadPublicKhalaTokensServedHistory(),
                              ]
                            : model.route.agentRef === 'adjutant'
                              ? [
                                  LoadPublicAgentGoal({
                                    agentId: publicAgentIdForRef(
                                      model.route.agentRef,
                                    ),
                                    agentRef: model.route.agentRef,
                                  }),
                                  LoadPublicAdjutantActivity(),
                                ]
                              : [
                                  LoadPublicAgentGoal({
                                    agentId: publicAgentIdForRef(
                                      model.route.agentRef,
                                    ),
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
      RequestedLandingLogout: () => [model, [LogoutFromLanding()]],
      CompletedLandingLogout: () => [model, []],
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
      ToggledGymLane: ({ lane }) => [
        evo(model, { gym: gym => toggleGymLane(gym, lane) }),
        [],
      ],
      UpdatedGymFanoutMode: ({ mode }) => [
        evo(model, { gym: gym => setGymFanoutMode(gym, mode) }),
        [],
      ],
      UpdatedGymConcurrency: ({ value }) => [
        evo(model, { gym: gym => setGymConcurrency(gym, value) }),
        [],
      ],
      UpdatedGymToolSet: ({ tools }) => [
        evo(model, { gym: gym => setGymToolSet(gym, tools) }),
        [],
      ],
      UpdatedGymModuleComposition: ({ mode }) => [
        evo(model, { gym: gym => setGymModuleComposition(gym, mode) }),
        [],
      ],
      ToggledGymCoordinator: ({ candidate }) => [
        evo(model, { gym: gym => toggleGymCoordinator(gym, candidate) }),
        [],
      ],
      UpdatedGymTemperature: ({ value }) => [
        evo(model, { gym: gym => setGymTemperature(gym, value) }),
        [],
      ],
      UpdatedGymReasoningEffort: ({ reasoningEffort }) => [
        evo(model, {
          gym: gym => setGymReasoningEffort(gym, reasoningEffort),
        }),
        [],
      ],
      UpdatedGymMaxTokens: ({ value }) => [
        evo(model, { gym: gym => setGymMaxTokens(gym, value) }),
        [],
      ],
      UpdatedGymTransport: ({ transport }) => [
        evo(model, { gym: gym => setGymTransport(gym, transport) }),
        [],
      ],
      ToggledGymSequenceShape: ({ shape }) => [
        evo(model, { gym: gym => toggleGymSequenceShape(gym, shape) }),
        [],
      ],
      UpdatedGymSamplesPerCell: ({ value }) => [
        evo(model, { gym: gym => setGymSamplesPerCell(gym, value) }),
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
      // Reconcile tick (only fired if the slow fallback poll is active, i.e. the
      // socket is down): re-fetch the scalar SUM. The model is left as-is (no
      // flash to Loading) so the odometer holds its last value until the next
      // total arrives.
      RequestedPollKhalaTokensServed: () => [
        model,
        [LoadPublicKhalaTokensServed()],
      ],
      // Scalar SUM seed / socket-down fallback. MONOTONE: only ever raises the
      // displayed total, never lowers it — so a stale-low cached scalar value can
      // never clobber a higher live (snapshot + stream) total back down. The
      // authoritative running total now flows through the snapshot summary + the
      // per-event `tokensServedTotal`; this scalar path is the seed-before-socket
      // and the graceful fallback when the socket is unavailable.
      SucceededLoadPublicKhalaTokensServed: ({ served }) => [
        evo(model, {
          publicKhalaTokensServed: counter =>
            khalaTokensServedAfterScalarSeed(counter, {
              generatedAt: served.generatedAt,
              tokensServed: served.tokensServed,
            }),
        }),
        [],
      ],
      // Snapshot seed: the room's `summary` record carries the AUTHORITATIVE
      // running total; the snapshot carries the cursor. Seed both, then subscribe
      // strictly from the cursor so events baked into the seed are never replayed.
      SucceededLoadKhalaTokensServedSnapshot: ({ cursor, summary }) => {
        const seeded = khalaTokensServedAfterSnapshot({
          counter: model.publicKhalaTokensServed,
          cursor,
          stream: model.khalaTokensServedStream,
          summary,
        })

        return [
          evo(model, {
            publicKhalaTokensServed: () => seeded.counter,
            khalaTokensServedStream: () => seeded.stream,
          }),
          [],
        ]
      },
      // A snapshot read failure is non-fatal: the scalar seed still establishes
      // the displayed total and every streamed event carries its own
      // authoritative total. Flip `snapshotLoaded` so the stream socket may open
      // (openagents #6324) — otherwise the stream would never connect and the
      // counter would only ever move on the slow scalar reconcile.
      FailedLoadKhalaTokensServedSnapshot: () => [
        evo(model, {
          khalaTokensServedStream: khalaTokensServedStreamSnapshotSettled,
        }),
        [],
      ],
      // Scalar fetch failure: only surface the error if the counter has NOT yet
      // been seeded with an authoritative value. A failed reconcile must never
      // wipe out a good live total.
      FailedLoadPublicKhalaTokensServed: ({ error }) => [
        model.publicKhalaTokensServed._tag === 'PublicKhalaTokensServedLoaded'
          ? model
          : evo(model, {
              publicKhalaTokensServed: () =>
                FailedPublicKhalaTokensServed({ error }),
            }),
        [],
      ],
      // History poll tick: re-fetch the per-day series. The model holds its
      // last loaded series (no flash to Loading) so the chart stays stable
      // between fetches and just updates when the next series arrives.
      RequestedPollKhalaTokensServedHistory: () => [
        model,
        [LoadPublicKhalaTokensServedHistory()],
      ],
      SucceededLoadPublicKhalaTokensServedHistory: ({ history }) => [
        evo(model, {
          publicKhalaTokensServedHistory: () =>
            LoadedPublicKhalaTokensServedHistory({ history }),
        }),
        [],
      ],
      FailedLoadPublicKhalaTokensServedHistory: ({ error }) => [
        evo(model, {
          publicKhalaTokensServedHistory: () =>
            FailedPublicKhalaTokensServedHistory({ error }),
        }),
        [],
      ],
      SelectedKhalaTokensServedHistoryGraphMetric: ({ metric }) => [
        evo(model, {
          publicKhalaTokensServedHistoryGraphMetric: () => metric,
        }),
        [],
      ],
      // Live fleet-shipping feed (#6534). The poll holds the last loaded
      // envelope (no flash to Loading) so the feed stays stable between fetches
      // and just updates when the next live window arrives.
      RequestedPollPublicActivityTimeline: () => [
        model,
        [LoadPublicActivityTimeline()],
      ],
      SucceededLoadPublicActivityTimeline: ({ envelope }) => [
        evo(model, {
          publicActivityTimeline: () =>
            LoadedPublicActivityTimeline({ envelope }),
        }),
        [],
      ],
      FailedLoadPublicActivityTimeline: ({ error }) => [
        evo(model, {
          publicActivityTimeline: () => FailedPublicActivityTimeline({ error }),
        }),
        [],
      ],
      // Model-mix poll tick (#6392): re-fetch the canonical model-family mix on
      // the same /stats refresh cadence as the history chart so the per-family
      // bars track the live counter as tokens stream in. The model holds its
      // last loaded mix (no flash to Loading) so the bars stay stable between
      // fetches and just update when the next aggregate arrives.
      RequestedPollKhalaTokensServedModelMix: () => [
        model,
        [LoadPublicKhalaTokensServedModelMix()],
      ],
      SucceededLoadPublicKhalaTokensServedModelMix: ({ mix }) => [
        evo(model, {
          publicKhalaTokensServedModelMix: () =>
            LoadedPublicKhalaTokensServedModelMix({ mix }),
        }),
        [],
      ],
      // A failed model-mix fetch only surfaces the error if the chart has NOT
      // yet loaded; a transient poll failure must never wipe out a good loaded
      // mix back to "unavailable" (#6392), mirroring the counter's "a failed
      // reconcile must never wipe a good live total" rule.
      FailedLoadPublicKhalaTokensServedModelMix: ({ error }) => [
        model.publicKhalaTokensServedModelMix._tag ===
        'PublicKhalaTokensServedModelMixLoaded'
          ? model
          : evo(model, {
              publicKhalaTokensServedModelMix: () =>
                FailedPublicKhalaTokensServedModelMix({ error }),
            }),
        [],
      ],
      RequestedPollKhalaTokensServedChannelMix: () => [
        model,
        [LoadPublicKhalaTokensServedChannelMix()],
      ],
      SucceededLoadPublicKhalaTokensServedChannelMix: ({ mix }) => [
        evo(model, {
          publicKhalaTokensServedChannelMix: () =>
            LoadedPublicKhalaTokensServedChannelMix({ mix }),
        }),
        [],
      ],
      FailedLoadPublicKhalaTokensServedChannelMix: ({ error }) => [
        model.publicKhalaTokensServedChannelMix._tag ===
        'PublicKhalaTokensServedChannelMixLoaded'
          ? model
          : evo(model, {
              publicKhalaTokensServedChannelMix: () =>
                FailedPublicKhalaTokensServedChannelMix({ error }),
            }),
        [],
      ],
      // Gym run-progress reconcile tick (#6261): the WebSocket push is the
      // primary path, so this is now a SLOW socket-down fallback that re-fetches
      // the authoritative full run set. The model holds its last loaded runs (no
      // flash to Loading) so the panel stays stable between reconciles.
      RequestedPollGymRunProgress: () => [model, [LoadPublicGymRunProgress()]],
      // The cold-read full set is the seed/reconcile. It must NOT clobber a live
      // streamed set: when the stream is open the per-run pushes are authoritative
      // (a stale reconcile snapshot mid-flight would flicker the cards), so the
      // reconcile only applies while the socket is not open.
      SucceededLoadPublicGymRunProgress: ({ runs }) =>
        model.gymRunProgressStream.connection === 'open'
          ? [model, []]
          : [
              evo(model, {
                gymRunProgress: () => LoadedPublicGymRunProgress({ runs }),
              }),
              [],
            ],
      // A failed poll must never wipe out an already-loaded set of live runs:
      // only surface the error if no runs have loaded yet, so a transient
      // network blip never flashes the follow-along back to an empty/error
      // state while runs are live.
      FailedLoadPublicGymRunProgress: ({ error }) => [
        model.gymRunProgress._tag === 'PublicGymRunProgressLoaded'
          ? model
          : evo(model, {
              gymRunProgress: () => FailedPublicGymRunProgress({ error }),
            }),
        [],
      ],
      // One-shot snapshot seed of the run cards + stream cursor (#6261). Subscribe
      // strictly from this cursor so a put baked into the seed is never replayed.
      SucceededLoadGymRunProgressSnapshot: ({ cursor, runs }) => {
        const seeded = gymRunProgressAfterSnapshot({
          counter: model.gymRunProgress,
          cursor,
          runs,
          stream: model.gymRunProgressStream,
        })

        return [
          evo(model, {
            gymRunProgress: () => seeded.counter,
            gymRunProgressStream: () => seeded.stream,
          }),
          [],
        ]
      },
      // A failed snapshot seed leaves the panel as-is (the cold GET still seeds
      // and the slow reconcile + next push self-heal); never flash to an error.
      FailedLoadGymRunProgressSnapshot: () => [model, []],
      // Live gym run-progress delta stream (#6261).
      OpenedGymRunProgressStream: () => [
        evo(model, { gymRunProgressStream: gymRunProgressStreamOpen }),
        [],
      ],
      ClosedGymRunProgressStream: () => [
        evo(model, { gymRunProgressStream: gymRunProgressStreamClosed }),
        [],
      ],
      FailedGymRunProgressStream: () => [
        evo(model, { gymRunProgressStream: gymRunProgressStreamFailed }),
        [],
      ],
      ReceivedGymRunProgressPatch: ({ patch }) => {
        const applied = applyGymRunProgressPatch({
          counter: model.gymRunProgress,
          patch,
          stream: model.gymRunProgressStream,
        })

        return [
          evo(model, {
            gymRunProgress: () => applied.counter,
            gymRunProgressStream: () => applied.stream,
          }),
          [],
        ]
      },
      ReceivedGymRunProgressCursorGap: ({ gap }) => [
        evo(model, {
          gymRunProgressStream: stream =>
            gymRunProgressStreamAfterCursorGap(stream, gap),
        }),
        [],
      ],
      // MirrorCode runs (#6378): one cold read on `/mirrorcode` entry.
      SucceededLoadMirrorCodeRuns: ({ response }) => [
        evo(model, {
          mirrorCodeRuns: () => LoadedMirrorCodeRuns({ response }),
        }),
        [],
      ],
      FailedLoadMirrorCodeRuns: ({ error }) => [
        evo(model, {
          mirrorCodeRuns: () => FailedMirrorCodeRuns({ error }),
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
      // Live `/trace/{uuid}` read (issue #6209). Ignore a response whose uuid no
      // longer matches the current route (the visitor navigated away mid-flight).
      SucceededLoadTrace: ({ uuid, trajectory, blobRefs }) =>
        model.route._tag === 'Trace' && model.route.uuid === uuid
          ? [
              evo(model, {
                trace: () =>
                  blobRefs === undefined
                    ? LoadedTrace({ uuid, trajectory })
                    : LoadedTrace({ uuid, trajectory, blobRefs }),
              }),
              [],
            ]
          : [model, []],
      FailedLoadTrace: ({ uuid, error, status }) =>
        model.route._tag === 'Trace' && model.route.uuid === uuid
          ? [
              evo(model, {
                // A clean 404 (not found / not public) is the honest not-found
                // state; any other failure is a distinct error state that
                // renders the same not-found body.
                trace: () =>
                  status === 404
                    ? NotFoundTrace({ uuid })
                    : FailedTrace({ uuid, error }),
              }),
              [],
            ]
          : [model, []],
      CompletedCopyShareLink: () => [model, []],
      SucceededLoadSettledFeedSnapshot: ({ cursor, summary }) => [
        evo(model, {
          settledFeed: feed =>
            settledFeedAfterSnapshot(feed, { cursor, summary }),
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
      // Live "Khala Tokens Served" delta stream (#6231).
      OpenedKhalaTokensServedStream: () => [
        evo(model, { khalaTokensServedStream: khalaTokensServedStreamOpen }),
        [],
      ],
      ClosedKhalaTokensServedStream: () => [
        evo(model, { khalaTokensServedStream: khalaTokensServedStreamClosed }),
        [],
      ],
      FailedKhalaTokensServedStream: () => [
        evo(model, { khalaTokensServedStream: khalaTokensServedStreamFailed }),
        [],
      ],
      ReceivedKhalaTokensServedPatch: ({ patch }) => {
        const applied = applyKhalaTokensServedPatch({
          counter: model.publicKhalaTokensServed,
          patch,
          stream: model.khalaTokensServedStream,
        })

        return [
          evo(model, {
            publicKhalaTokensServed: () => applied.counter,
            khalaTokensServedStream: () => applied.stream,
          }),
          [],
        ]
      },
      ReceivedKhalaTokensServedCursorGap: ({ gap }) => [
        evo(model, {
          khalaTokensServedStream: stream =>
            khalaTokensServedStreamAfterCursorGap(stream, gap),
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

      // GENERIC /khala CHAT (stateless streaming, NOT the concierge intake). The
      // composer/turn lifecycle mirrors the onboarding stream handlers minus the
      // persistence/resume/output-spec machinery; the subscription reads
      // `khalaChat.pendingTurn` and posts the whole running conversation.
      UpdatedKhalaChatComposer: ({ value }) => [
        evo(model, {
          khalaChat: chat => evo(chat, { composerDraft: () => value }),
        }),
        [],
      ],
      ToggledKhalaChatComposerPreview: () => [
        evo(model, {
          khalaChat: chat =>
            evo(chat, { composerPreview: preview => !preview }),
        }),
        [],
      ],
      ToggledKhalaChatComposerExpanded: () => [
        evo(model, {
          khalaChat: chat =>
            evo(chat, { composerExpanded: expanded => !expanded }),
        }),
        [],
      ],
      SubmittedKhalaChatTurn: () => {
        const chat = model.khalaChat
        const userText = chat.composerDraft.trim()
        if (
          userText === '' ||
          chat.status === 'submitting' ||
          chat.status === 'streaming'
        ) {
          return [model, []]
        }

        // A deterministic per-turn id (the transcript length after appending the
        // user turn) so the streaming subscription opens the stream exactly once.
        const turnId = String(chat.transcript.length + 1)
        // The prior transcript is the stateless history the subscription posts
        // alongside the new user turn.
        const history = chat.transcript

        const nextModel = evo(model, {
          khalaChat: current =>
            evo(current, {
              status: () => 'submitting',
              errorReason: () => null,
              composerDraft: () => '',
              streamingReply: () => null,
              transcript: transcript => [
                ...transcript,
                { role: 'user', content: userText },
              ],
              pendingTurn: () => ({ id: turnId, userText, history }),
            }),
        })

        return [
          nextModel,
          [ScrollKhalaChatLatestTurnIntoView(), FocusKhalaChatComposer()],
        ]
      },
      OpenedKhalaChatStream: ({ turnId }) => {
        const chat = model.khalaChat
        if (chat.pendingTurn === null || chat.pendingTurn.id !== turnId) {
          return [model, []]
        }
        return [
          evo(model, {
            khalaChat: current =>
              evo(current, {
                status: () => 'streaming',
                streamingReply: () => '',
              }),
          }),
          [],
        ]
      },
      ReceivedKhalaChatDelta: ({ turnId, text }) => {
        const chat = model.khalaChat
        if (chat.pendingTurn === null || chat.pendingTurn.id !== turnId) {
          return [model, []]
        }
        return [
          evo(model, {
            khalaChat: current =>
              evo(current, {
                status: () => 'streaming',
                streamingReply: reply => (reply ?? '') + text,
              }),
          }),
          [],
        ]
      },
      SucceededKhalaChatTurn: ({ turnId }) => {
        const chat = model.khalaChat
        if (chat.pendingTurn === null || chat.pendingTurn.id !== turnId) {
          return [model, []]
        }
        const reply = chat.streamingReply ?? ''
        return [
          evo(model, {
            khalaChat: current =>
              evo(current, {
                status: () => 'idle',
                errorReason: () => null,
                streamingReply: () => null,
                pendingTurn: () => null,
                transcript: transcript => [
                  ...transcript,
                  { role: 'assistant', content: reply },
                ],
              }),
          }),
          [],
        ]
      },
      ClickedKhalaChatJumpToLatest: () => [
        model,
        [ScrollKhalaChatThreadToEnd()],
      ],
      FailedKhalaChatTurn: ({ reason }) => [
        evo(model, {
          khalaChat: chat =>
            evo(chat, {
              status: () => 'error',
              errorReason: () => (reason === '' ? null : reason),
              streamingReply: () => null,
              pendingTurn: () => null,
            }),
        }),
        [],
      ],
      CompletedScrollKhalaChatThread: () => [model, []],
      CompletedFocusKhalaChatComposer: () => [model, []],
      OpenedKhalaChatInfo: () => [
        evo(model, {
          khalaChat: chat => evo(chat, { infoOpen: () => true }),
        }),
        [],
      ],
      ClosedKhalaChatInfo: () => [
        evo(model, {
          khalaChat: chat => evo(chat, { infoOpen: () => false }),
        }),
        [],
      ],
    }),
  )
