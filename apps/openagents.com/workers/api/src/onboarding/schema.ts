import { Schema as S } from 'effect'

export const OnboardingStep = S.Literals([
  'repository',
  'goal',
  'billing',
  'complete',
])
export type OnboardingStep = typeof OnboardingStep.Type

export const OnboardingGitHubRepository = S.Struct({
  id: S.String,
  provider: S.Literal('github'),
  owner: S.String,
  name: S.String,
  fullName: S.String,
  private: S.Boolean,
  defaultBranch: S.String,
  htmlUrl: S.String,
  description: S.NullOr(S.String),
})
export type OnboardingGitHubRepository = typeof OnboardingGitHubRepository.Type

export const OnboardingRepositoryUnselected = S.TaggedStruct(
  'RepositoryUnselected',
  {},
)
export const OnboardingRepositorySkipped = S.TaggedStruct('RepositorySkipped', {
  skippedAt: S.String,
})
export const OnboardingRepositorySelected = S.TaggedStruct(
  'RepositorySelected',
  {
    repository: OnboardingGitHubRepository,
    selectedAt: S.String,
  },
)
export const OnboardingRepositorySelection = S.Union([
  OnboardingRepositoryUnselected,
  OnboardingRepositorySkipped,
  OnboardingRepositorySelected,
])
export type OnboardingRepositorySelection =
  typeof OnboardingRepositorySelection.Type

export const OnboardingBillingPending = S.TaggedStruct('BillingPending', {})
export const OnboardingBillingSkipped = S.TaggedStruct('BillingSkipped', {
  skippedAt: S.String,
})
export const OnboardingBillingState = S.Union([
  OnboardingBillingPending,
  OnboardingBillingSkipped,
])
export type OnboardingBillingState = typeof OnboardingBillingState.Type

export const OnboardingStatus = S.Struct({
  step: OnboardingStep,
  repository: OnboardingRepositorySelection,
  billing: OnboardingBillingState,
  goal: S.NullOr(S.String),
  completedAt: S.NullOr(S.String),
  updatedAt: S.String,
})
export type OnboardingStatus = typeof OnboardingStatus.Type

export const OnboardingStatusResponse = S.Struct({
  onboarding: OnboardingStatus,
})
export type OnboardingStatusResponse = typeof OnboardingStatusResponse.Type

export const OnboardingRepositoriesResponse = S.Struct({
  repositories: S.Array(OnboardingGitHubRepository),
  tokenStatus: S.Literals(['available', 'missing']),
})
export type OnboardingRepositoriesResponse =
  typeof OnboardingRepositoriesResponse.Type

export const SelectOnboardingRepositoryByIdRequest = S.Struct({
  repositoryId: S.String,
})

export const SelectOnboardingRepositoryByNameRequest = S.Struct({
  owner: S.String,
  name: S.String,
})

export const SelectOnboardingRepositoryRequest = S.Union([
  SelectOnboardingRepositoryByIdRequest,
  SelectOnboardingRepositoryByNameRequest,
])
export type SelectOnboardingRepositoryRequest =
  typeof SelectOnboardingRepositoryRequest.Type

export const SubmitOnboardingGoalRequest = S.Struct({
  goal: S.String,
})
export type SubmitOnboardingGoalRequest =
  typeof SubmitOnboardingGoalRequest.Type
