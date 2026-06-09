import { Effect, Match as M, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import {
  CompletedCopyShareLink,
  FailedLoadPublicAdjutantActivity,
  FailedLoadPublicAgentGoal,
  FailedLoadPublicArtanisReport,
  FailedLoadPublicPylonStats,
  FailedLoadShareProjection,
  Message,
  SucceededLoadPublicAdjutantActivity,
  SucceededLoadPublicAgentGoal,
  SucceededLoadPublicArtanisReport,
  SucceededLoadPublicPylonStats,
  SucceededLoadShareProjection,
} from './message'
import {
  FailedPublicAdjutantActivity,
  FailedPublicAgent,
  FailedPublicArtanisReport,
  FailedPublicPylonStats,
  FailedShareProjection,
  LoadedPublicAdjutantActivity,
  LoadedPublicAgent,
  LoadedPublicArtanisReport,
  LoadedPublicPylonStats,
  LoadedShareProjection,
  Model,
  PublicAdjutantActivity,
  PublicAgentGoalResponse,
  PublicArtanisReport,
  PublicPylonStats,
  ShareProjectionResponse,
} from './model'

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
    : model.route._tag === 'Home'
      ? [LoadPublicPylonStats()]
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
    }),
  )
