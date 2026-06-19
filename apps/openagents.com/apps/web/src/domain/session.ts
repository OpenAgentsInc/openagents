import {
  PublicProviderAccount,
  PublicProviderConnectionAttempt,
} from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

export const Session = S.Struct({
  userId: S.String,
  email: S.String,
  name: S.String,
  login: S.optionalKey(S.String),
  avatarUrl: S.optionalKey(S.String),
  provider: S.optionalKey(S.String),
  githubId: S.optionalKey(S.String),
})

export type Session = typeof Session.Type

export const TeamMember = S.Struct({
  userId: S.String,
  name: S.String,
  email: S.NullOr(S.String),
  avatarUrl: S.NullOr(S.String),
  githubUsername: S.NullOr(S.String),
  githubId: S.NullOr(S.String),
  role: S.String,
  status: S.String,
  joinedAt: S.NullOr(S.String),
})
export type TeamMember = typeof TeamMember.Type

export const TeamProjectAgent = S.Struct({
  id: S.String,
  name: S.String,
  status: S.String,
  scope: S.String,
  runtime: S.String,
  backend: S.String,
  repository: S.String,
  focus: S.String,
})
export type TeamProjectAgent = typeof TeamProjectAgent.Type

export const TeamProject = S.Struct({
  id: S.String,
  teamId: S.String,
  name: S.String,
  slug: S.NullOr(S.String),
  description: S.String,
  status: S.String,
  agent: S.optionalKey(TeamProjectAgent),
})
export type TeamProject = typeof TeamProject.Type

export const Team = S.Struct({
  id: S.String,
  name: S.String,
  slug: S.NullOr(S.String),
  role: S.String,
  members: S.Array(TeamMember),
  projects: S.optionalKey(S.Array(TeamProject)),
})
export type Team = typeof Team.Type

export const TokenUsageTotals = S.Struct({
  inputTokens: S.Number,
  outputTokens: S.Number,
  reasoningTokens: S.Number,
  cacheReadTokens: S.Number,
  cacheWrite5mTokens: S.Number,
  cacheWrite1hTokens: S.Number,
  totalTokens: S.Number,
  usageEvents: S.Number,
})
export type TokenUsageTotals = typeof TokenUsageTotals.Type

export const TokenLeaderboardTeam = S.Struct({
  inputTokens: S.Number,
  outputTokens: S.Number,
  reasoningTokens: S.Number,
  cacheReadTokens: S.Number,
  cacheWrite5mTokens: S.Number,
  cacheWrite1hTokens: S.Number,
  totalTokens: S.Number,
  usageEvents: S.Number,
  teamId: S.String,
  teamName: S.String,
  teamSlug: S.NullOr(S.String),
})
export type TokenLeaderboardTeam = typeof TokenLeaderboardTeam.Type

export const TokenLeaderboardUser = S.Struct({
  inputTokens: S.Number,
  outputTokens: S.Number,
  reasoningTokens: S.Number,
  cacheReadTokens: S.Number,
  cacheWrite5mTokens: S.Number,
  cacheWrite1hTokens: S.Number,
  totalTokens: S.Number,
  usageEvents: S.Number,
  userId: S.String,
  displayName: S.String,
  email: S.NullOr(S.String),
  avatarUrl: S.NullOr(S.String),
  githubUsername: S.NullOr(S.String),
})
export type TokenLeaderboardUser = typeof TokenLeaderboardUser.Type

export const TokenUsageRunSummary = S.Struct({
  inputTokens: S.Number,
  outputTokens: S.Number,
  reasoningTokens: S.Number,
  cacheReadTokens: S.Number,
  cacheWrite5mTokens: S.Number,
  cacheWrite1hTokens: S.Number,
  totalTokens: S.Number,
  usageEvents: S.Number,
  runId: S.String,
  title: S.String,
  repository: S.String,
  status: S.String,
  runnerId: S.String,
  updatedAt: S.String,
})
export type TokenUsageRunSummary = typeof TokenUsageRunSummary.Type

export const TokenLeaderboards = S.Struct({
  generatedAt: S.String,
  global: TokenUsageTotals,
  currentUser: TokenUsageTotals,
  teams: S.Array(TokenLeaderboardTeam),
  users: S.Array(TokenLeaderboardUser),
  currentUserTeams: S.Array(TokenLeaderboardTeam),
  missingUsageSignals: S.Number,
  recentRuns: S.Array(TokenUsageRunSummary),
})
export type TokenLeaderboards = typeof TokenLeaderboards.Type

export const BillingLedgerSource = S.Literals([
  'trial_grant',
  'coupon',
  'credit_card_placeholder',
  'stripe_checkout',
  'stripe_auto_top_up',
  'container_usage',
  'codex_usage',
  'manual_adjustment',
])
export type BillingLedgerSource = typeof BillingLedgerSource.Type

export const BillingLedgerEntry = S.Struct({
  id: S.String,
  source: BillingLedgerSource,
  description: S.String,
  amountCents: S.Number,
  amountFormatted: S.String,
  quantity: S.NullOr(S.Number),
  unit: S.NullOr(S.String),
  createdAt: S.String,
})
export type BillingLedgerEntry = typeof BillingLedgerEntry.Type

export const BillingActiveRun = S.Struct({
  id: S.String,
  title: S.String,
  status: S.String,
  accruedSeconds: S.Number,
  estimatedDebitCents: S.Number,
  estimatedDebitFormatted: S.String,
  startedAt: S.NullOr(S.String),
})
export type BillingActiveRun = typeof BillingActiveRun.Type

export const BillingSavedPaymentMethod = S.Struct({
  brand: S.NullOr(S.String),
  expMonth: S.NullOr(S.Number),
  expYear: S.NullOr(S.Number),
  last4: S.NullOr(S.String),
  status: S.Literals(['active', 'detached', 'failed', 'requires_action']),
  stripePaymentMethodId: S.String,
  updatedAt: S.String,
})
export type BillingSavedPaymentMethod = typeof BillingSavedPaymentMethod.Type

export const BillingAutoTopUpPolicy = S.Struct({
  amountCents: S.Number,
  amountFormatted: S.String,
  enabled: S.Boolean,
  monthlyCapCents: S.Number,
  monthlyCapFormatted: S.String,
  pauseReason: S.NullOr(S.String),
  spentThisMonthCents: S.Number,
  spentThisMonthFormatted: S.String,
  status: S.Literals(['active', 'disabled', 'paused']),
  thresholdCents: S.Number,
  thresholdFormatted: S.String,
  updatedAt: S.String,
})
export type BillingAutoTopUpPolicy = typeof BillingAutoTopUpPolicy.Type

export const BillingAutoTopUpEvent = S.Struct({
  amountCents: S.Number,
  amountFormatted: S.String,
  createdAt: S.String,
  id: S.String,
  reason: S.NullOr(S.String),
  status: S.Literals([
    'cap_reached',
    'declined',
    'requires_payment_method',
    'skipped',
    'succeeded',
  ]),
})
export type BillingAutoTopUpEvent = typeof BillingAutoTopUpEvent.Type

export const BillingAutoTopUpState = S.Struct({
  events: S.Array(BillingAutoTopUpEvent),
  policy: BillingAutoTopUpPolicy,
  savedPaymentMethod: S.NullOr(BillingSavedPaymentMethod),
})
export type BillingAutoTopUpState = typeof BillingAutoTopUpState.Type

// One purchasable credit package, projected from the server-configured Stripe
// catalog. `id` is the real catalog id the billing page POSTs to
// `/api/billing/checkout`, so the UI never sends an id the server cannot honor.
export const BillingCreditPackage = S.Struct({
  id: S.String,
  label: S.String,
  amountCents: S.Number,
  amountFormatted: S.String,
  currency: S.Literal('USD'),
})
export type BillingCreditPackage = typeof BillingCreditPackage.Type

export const BillingSummary = S.Struct({
  currency: S.Literal('USD'),
  status: S.Literals(['active', 'suspended']),
  balanceCents: S.Number,
  balanceFormatted: S.String,
  minimumRunCreditCents: S.Number,
  minimumRunCreditFormatted: S.String,
  rates: S.Struct({
    containerCentsPerMinute: S.Number,
    codexCentsPerThousandTokens: S.Number,
  }),
  // The purchasable catalog the billing page renders buy buttons from. Every
  // server producer of a summary attaches this (empty when card checkout is
  // not configured), so it is a required field rather than an optional.
  packages: S.Array(BillingCreditPackage),
  recentEntries: S.Array(BillingLedgerEntry),
  activeRuns: S.Array(BillingActiveRun),
  autoTopUp: BillingAutoTopUpState,
})
export type BillingSummary = typeof BillingSummary.Type

export const emptyBillingSummary = (): BillingSummary => ({
  currency: 'USD',
  status: 'active',
  balanceCents: 0,
  balanceFormatted: '$0.00',
  minimumRunCreditCents: 5,
  minimumRunCreditFormatted: '$0.05',
  rates: {
    containerCentsPerMinute: 5,
    codexCentsPerThousandTokens: 2,
  },
  packages: [],
  recentEntries: [],
  activeRuns: [],
  autoTopUp: {
    events: [],
    policy: {
      amountCents: 2500,
      amountFormatted: '$25.00',
      enabled: false,
      monthlyCapCents: 10000,
      monthlyCapFormatted: '$100.00',
      pauseReason: null,
      spentThisMonthCents: 0,
      spentThisMonthFormatted: '$0.00',
      status: 'disabled',
      thresholdCents: 500,
      thresholdFormatted: '$5.00',
      updatedAt: '',
    },
    savedPaymentMethod: null,
  },
})

export const BillingSummaryResponse = S.Struct({
  billing: BillingSummary,
  message: S.optionalKey(S.String),
})
export type BillingSummaryResponse = typeof BillingSummaryResponse.Type

export const BillingCheckoutResponse = S.Struct({
  billing: BillingSummary,
  checkoutUrl: S.String,
  message: S.String,
  packageId: S.String,
  status: S.Literal('checkout_created'),
})
export type BillingCheckoutResponse = typeof BillingCheckoutResponse.Type

export const BillingSetupIntentResponse = S.Struct({
  clientSecret: S.String,
  setupIntentId: S.String,
  status: S.String,
})
export type BillingSetupIntentResponse = typeof BillingSetupIntentResponse.Type

export const ProviderAccountBundle = S.Struct({
  accounts: S.Array(PublicProviderAccount),
  attempts: S.Array(PublicProviderConnectionAttempt),
})
export type ProviderAccountBundle = typeof ProviderAccountBundle.Type

export const emptyProviderAccountBundle = (): ProviderAccountBundle => ({
  accounts: [],
  attempts: [],
})

export const ProviderDeviceLoginStartResponse = S.Struct({
  account: PublicProviderAccount,
  attempt: PublicProviderConnectionAttempt,
  expiresAt: S.String,
  intervalSeconds: S.Number,
  providerAccountRef: S.String,
  verificationUrl: S.String,
  userCode: S.String,
})
export type ProviderDeviceLoginStartResponse =
  typeof ProviderDeviceLoginStartResponse.Type

export const ProviderDeviceLoginStatusResponse = S.Struct({
  account: PublicProviderAccount,
  attempt: PublicProviderConnectionAttempt,
})
export type ProviderDeviceLoginStatusResponse =
  typeof ProviderDeviceLoginStatusResponse.Type

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

export const incompleteOnboardingStatus = (): OnboardingStatus => ({
  step: 'repository',
  repository: { _tag: 'RepositoryUnselected' },
  billing: { _tag: 'BillingPending' },
  goal: null,
  completedAt: null,
  updatedAt: '',
})

export const completedOnboardingStatus = (): OnboardingStatus => ({
  step: 'complete',
  repository: { _tag: 'RepositorySkipped', skippedAt: '' },
  billing: { _tag: 'BillingSkipped', skippedAt: '' },
  goal: null,
  completedAt: '',
  updatedAt: '',
})

export const onboardingIsComplete = (onboarding: OnboardingStatus): boolean =>
  onboarding.step === 'complete' && onboarding.completedAt !== null

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

export const AuthBootstrap = S.Struct({
  session: Session,
  teams: S.Array(Team),
  tokenLeaderboards: S.optionalKey(TokenLeaderboards),
  billing: BillingSummary,
  onboarding: OnboardingStatus,
  providerAccounts: S.optionalKey(ProviderAccountBundle),
  isAdmin: S.Boolean,
})
export type AuthBootstrap = typeof AuthBootstrap.Type

export const authBootstrapFromSession = (session: Session): AuthBootstrap => ({
  session,
  teams: [],
  billing: emptyBillingSummary(),
  onboarding: completedOnboardingStatus(),
  providerAccounts: emptyProviderAccountBundle(),
  isAdmin: false,
})

export const AuthSessionResponse = S.Union([
  S.Struct({
    authenticated: S.Literal(true),
    bootstrap: AuthBootstrap,
  }),
  S.Struct({
    authenticated: S.Literal(false),
  }),
])
export type AuthSessionResponse = typeof AuthSessionResponse.Type

const CORE_TEAM_ID = 'team_openagents_core'
const CORE_TEAM_NAME = 'OpenAgents Core Team'
const CORE_TEAM_SLUG = 'openagents-core-team'

export const authHasCoreTeamAccess = (auth: AuthBootstrap): boolean =>
  auth.teams.some(
    team =>
      team.id === CORE_TEAM_ID ||
      team.slug === CORE_TEAM_SLUG ||
      team.name === CORE_TEAM_NAME,
  )
