import { CursorGap, SyncPatch } from '@openagentsinc/sync-schema'
import { Schema as S } from 'effect'
import { m } from 'foldkit/message'

import {
  OnboardingStep,
  PublicAdjutantActivity,
  PublicAgentGoalResponse,
  PublicArtanisReport,
  PublicForumLaunchStatus,
  PublicForumTipLeaderboards,
  PublicProductPromises,
  PublicPromiseTransitions,
  PublicPylonStats,
  PublicTrainingRunsResponse,
  ShareProjectionResponse,
} from './model'

// MESSAGE

export const ClickedCopyShareLink = m('ClickedCopyShareLink', {
  url: S.String,
})
export const ClickedEnterKhala = m('ClickedEnterKhala')
export const CompletedNavigateToKhala = m('CompletedNavigateToKhala')
export const ClickedExitKhala = m('ClickedExitKhala')
export const ClickedEnterTassadar = m('ClickedEnterTassadar')
export const CompletedNavigateToTassadar = m('CompletedNavigateToTassadar')
export const ClickedCopyAgentInstructions = m('ClickedCopyAgentInstructions', {
  text: S.String,
})
export const CompletedCopyAgentInstructions = m(
  'CompletedCopyAgentInstructions',
)
export const CompletedNavigateToLanding = m('CompletedNavigateToLanding')
export const ClickedOnboardingStep = m('ClickedOnboardingStep', {
  step: OnboardingStep,
})
export const SelectedOnboardingRepository = m('SelectedOnboardingRepository', {
  repository: S.String,
})
export const SkippedOnboardingRepository = m('SkippedOnboardingRepository')
export const UpdatedOnboardingFundingAmount = m(
  'UpdatedOnboardingFundingAmount',
  { value: S.String },
)
export const ToggledOnboardingCoupon = m('ToggledOnboardingCoupon')
export const UpdatedOnboardingCouponCode = m('UpdatedOnboardingCouponCode', {
  value: S.String,
})
export const SucceededLoadPublicAgentGoal = m('SucceededLoadPublicAgentGoal', {
  agentRef: S.String,
  response: PublicAgentGoalResponse,
})
export const FailedLoadPublicAgentGoal = m('FailedLoadPublicAgentGoal', {
  agentRef: S.String,
  error: S.String,
})
export const SucceededLoadPublicArtanisReport = m(
  'SucceededLoadPublicArtanisReport',
  {
    report: PublicArtanisReport,
  },
)
export const FailedLoadPublicArtanisReport = m(
  'FailedLoadPublicArtanisReport',
  {
    error: S.String,
  },
)
export const SucceededLoadPublicAdjutantActivity = m(
  'SucceededLoadPublicAdjutantActivity',
  {
    activity: PublicAdjutantActivity,
  },
)
export const FailedLoadPublicAdjutantActivity = m(
  'FailedLoadPublicAdjutantActivity',
  {
    error: S.String,
  },
)
export const SucceededLoadPublicPylonStats = m(
  'SucceededLoadPublicPylonStats',
  {
    stats: PublicPylonStats,
  },
)
export const FailedLoadPublicPylonStats = m('FailedLoadPublicPylonStats', {
  error: S.String,
})
export const SucceededLoadPublicForumLaunchStatus = m(
  'SucceededLoadPublicForumLaunchStatus',
  {
    status: PublicForumLaunchStatus,
  },
)
export const FailedLoadPublicForumLaunchStatus = m(
  'FailedLoadPublicForumLaunchStatus',
  {
    error: S.String,
  },
)
export const SucceededLoadPublicForumTipLeaderboards = m(
  'SucceededLoadPublicForumTipLeaderboards',
  {
    leaderboards: PublicForumTipLeaderboards,
  },
)
export const FailedLoadPublicForumTipLeaderboards = m(
  'FailedLoadPublicForumTipLeaderboards',
  {
    error: S.String,
  },
)
export const SucceededLoadPublicProductPromises = m(
  'SucceededLoadPublicProductPromises',
  {
    promises: PublicProductPromises,
  },
)
export const FailedLoadPublicProductPromises = m(
  'FailedLoadPublicProductPromises',
  {
    error: S.String,
  },
)
export const SucceededLoadPublicPromiseTransitions = m(
  'SucceededLoadPublicPromiseTransitions',
  {
    transitions: PublicPromiseTransitions,
  },
)
export const FailedLoadPublicPromiseTransitions = m(
  'FailedLoadPublicPromiseTransitions',
  {
    error: S.String,
  },
)
export const SucceededLoadPublicTrainingRuns = m(
  'SucceededLoadPublicTrainingRuns',
  {
    response: PublicTrainingRunsResponse,
    selectedRunId: S.NullOr(S.String),
  },
)
export const FailedLoadPublicTrainingRuns = m('FailedLoadPublicTrainingRuns', {
  error: S.String,
  runId: S.NullOr(S.String),
})
export const SucceededLoadShareProjection = m('SucceededLoadShareProjection', {
  response: ShareProjectionResponse,
  shareId: S.String,
})
export const FailedLoadShareProjection = m('FailedLoadShareProjection', {
  error: S.String,
  shareId: S.String,
  status: S.Int,
})
export const CompletedCopyShareLink = m('CompletedCopyShareLink', {
  url: S.String,
})
// Live settled feed stream (openagents #5311).
export const SucceededLoadSettledFeedSnapshot = m(
  'SucceededLoadSettledFeedSnapshot',
  {
    cursor: S.Number,
    summary: S.NullOr(
      S.Struct({
        totalSettledCount: S.Number,
        totalSettledSats: S.Number,
      }),
    ),
  },
)
export const FailedLoadSettledFeedSnapshot = m(
  'FailedLoadSettledFeedSnapshot',
  {
    error: S.String,
  },
)
export const OpenedSettledFeedStream = m('OpenedSettledFeedStream')
export const ClosedSettledFeedStream = m('ClosedSettledFeedStream')
export const FailedSettledFeedStream = m('FailedSettledFeedStream', {
  error: S.String,
})
export const ReceivedSettledFeedPatch = m('ReceivedSettledFeedPatch', {
  patch: SyncPatch,
})
export const ReceivedSettledFeedCursorGap = m('ReceivedSettledFeedCursorGap', {
  gap: CursorGap,
})
export const Message = S.Union([
  ClickedCopyShareLink,
  ClickedEnterKhala,
  CompletedNavigateToKhala,
  ClickedExitKhala,
  ClickedEnterTassadar,
  CompletedNavigateToTassadar,
  ClickedCopyAgentInstructions,
  CompletedCopyAgentInstructions,
  CompletedNavigateToLanding,
  ClickedOnboardingStep,
  SelectedOnboardingRepository,
  SkippedOnboardingRepository,
  UpdatedOnboardingFundingAmount,
  ToggledOnboardingCoupon,
  UpdatedOnboardingCouponCode,
  SucceededLoadPublicAgentGoal,
  FailedLoadPublicAgentGoal,
  SucceededLoadPublicArtanisReport,
  FailedLoadPublicArtanisReport,
  SucceededLoadPublicAdjutantActivity,
  FailedLoadPublicAdjutantActivity,
  SucceededLoadPublicPylonStats,
  FailedLoadPublicPylonStats,
  SucceededLoadPublicForumLaunchStatus,
  FailedLoadPublicForumLaunchStatus,
  SucceededLoadPublicForumTipLeaderboards,
  FailedLoadPublicForumTipLeaderboards,
  SucceededLoadPublicProductPromises,
  FailedLoadPublicProductPromises,
  SucceededLoadPublicPromiseTransitions,
  FailedLoadPublicPromiseTransitions,
  SucceededLoadPublicTrainingRuns,
  FailedLoadPublicTrainingRuns,
  SucceededLoadShareProjection,
  FailedLoadShareProjection,
  CompletedCopyShareLink,
  SucceededLoadSettledFeedSnapshot,
  FailedLoadSettledFeedSnapshot,
  OpenedSettledFeedStream,
  ClosedSettledFeedStream,
  FailedSettledFeedStream,
  ReceivedSettledFeedPatch,
  ReceivedSettledFeedCursorGap,
])
export type Message = typeof Message.Type
