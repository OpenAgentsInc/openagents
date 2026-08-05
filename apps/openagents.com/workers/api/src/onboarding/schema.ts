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

// Mobile-bearer repo list/select API (MM-B1, issue #8471). Paginated —
// distinct from the cookie-gated `OnboardingRepositoriesResponse` above,
// which returns the whole first page unpaginated for the web onboarding
// wizard.
export const MobileRepositoryListResponse = S.Struct({
  repositories: S.Array(OnboardingGitHubRepository),
  page: S.Number,
  perPage: S.Number,
  hasNextPage: S.Boolean,
})
export type MobileRepositoryListResponse =
  typeof MobileRepositoryListResponse.Type

export const MobileRepositoryDetailResponse = S.Struct({
  repository: OnboardingGitHubRepository,
})
export type MobileRepositoryDetailResponse =
  typeof MobileRepositoryDetailResponse.Type

const GitHubOwner = S.String.check(S.isMinLength(1), S.isMaxLength(100))
const GitHubRepositoryName = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(100),
)
const GitHubRef = S.String.check(S.isMinLength(1), S.isMaxLength(255))
const GitHubPath = S.String.check(S.isMaxLength(1_024))

const GitHubRepositoryTarget = {
  owner: GitHubOwner,
  name: GitHubRepositoryName,
}

export const MobileGitHubToolRequest = S.Union([
  S.Struct({
    action: S.Literal('get_contents'),
    ...GitHubRepositoryTarget,
    path: S.optional(GitHubPath),
    ref: S.optional(GitHubRef),
  }),
  S.Struct({
    action: S.Literal('list_issues'),
    ...GitHubRepositoryTarget,
    state: S.optional(S.Literals(['open', 'closed', 'all'])),
    limit: S.optional(
      S.Number.check(S.isInt(), S.isBetween({ minimum: 1, maximum: 100 })),
    ),
  }),
  S.Struct({
    action: S.Literal('create_issue'),
    ...GitHubRepositoryTarget,
    title: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
    body: S.optional(S.String.check(S.isMaxLength(65_536))),
  }),
  S.Struct({
    action: S.Literal('create_branch'),
    ...GitHubRepositoryTarget,
    branch: GitHubRef,
    fromRef: GitHubRef,
  }),
  S.Struct({
    action: S.Literal('upsert_file'),
    ...GitHubRepositoryTarget,
    path: S.String.check(S.isMinLength(1), S.isMaxLength(1_024)),
    content: S.String.check(S.isMaxLength(1_000_000)),
    message: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
    branch: GitHubRef,
    sha: S.optional(S.String.check(S.isMinLength(1), S.isMaxLength(64))),
  }),
  S.Struct({
    action: S.Literal('create_pull_request'),
    ...GitHubRepositoryTarget,
    title: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
    body: S.optional(S.String.check(S.isMaxLength(65_536))),
    head: GitHubRef,
    base: GitHubRef,
    draft: S.optional(S.Boolean),
  }),
])
export type MobileGitHubToolRequest = typeof MobileGitHubToolRequest.Type

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
