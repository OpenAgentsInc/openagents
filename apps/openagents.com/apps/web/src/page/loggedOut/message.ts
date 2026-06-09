import { Schema as S } from 'effect'
import { m } from 'foldkit/message'

import {
  OnboardingStep,
  PublicAdjutantActivity,
  PublicAgentGoalResponse,
  PublicArtanisReport,
  PublicForumLaunchStatus,
  PublicForumTipLeaderboards,
  PublicPylonStats,
  ShareProjectionResponse,
} from './model'

// MESSAGE

export const ClickedCopyShareLink = m('ClickedCopyShareLink', {
  url: S.String,
})
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
export const Message = S.Union([
  ClickedCopyShareLink,
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
  SucceededLoadShareProjection,
  FailedLoadShareProjection,
  CompletedCopyShareLink,
])
export type Message = typeof Message.Type
