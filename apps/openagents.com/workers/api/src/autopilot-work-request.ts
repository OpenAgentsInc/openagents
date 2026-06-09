import { Schema as S } from 'effect'

import {
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

export const OpenAgentsAutopilotWorkRequestSchemaVersion = S.Literal(
  'openagents.autopilot_work_request.v1',
)
export type OpenAgentsAutopilotWorkRequestSchemaVersion =
  typeof OpenAgentsAutopilotWorkRequestSchemaVersion.Type

export const OpenAgentsAutopilotWorkRequestIntent = S.Literal(
  'delegate_to_autopilot',
)
export type OpenAgentsAutopilotWorkRequestIntent =
  typeof OpenAgentsAutopilotWorkRequestIntent.Type

export const OpenAgentsAutopilotWorkRequestMode = S.Literals([
  'free_slice',
  'free_slice_or_paid_quote',
  'free_slice_or_paid_quote_or_l402',
  'mdk_checkout',
  'l402',
])
export type OpenAgentsAutopilotWorkRequestMode =
  typeof OpenAgentsAutopilotWorkRequestMode.Type

export const OpenAgentsAutopilotCallerKind = S.Literals([
  'browser_session',
  'registered_agent',
  'pylon',
  'anonymous_paid_agent',
])
export type OpenAgentsAutopilotCallerKind =
  typeof OpenAgentsAutopilotCallerKind.Type

export const OpenAgentsAutopilotTaskKind = S.Literals([
  'benchmark_or_gepa',
  'code_change',
  'repo_change',
  'research_and_patch',
  'site_adjustment',
  'site_generation',
  'test_repair',
])
export type OpenAgentsAutopilotTaskKind =
  typeof OpenAgentsAutopilotTaskKind.Type

export const OpenAgentsAutopilotRepositoryProvider = S.Literals([
  'github',
])
export type OpenAgentsAutopilotRepositoryProvider =
  typeof OpenAgentsAutopilotRepositoryProvider.Type

export const OpenAgentsAutopilotRepositoryVisibility = S.Literals([
  'internal',
  'private',
  'public',
])
export type OpenAgentsAutopilotRepositoryVisibility =
  typeof OpenAgentsAutopilotRepositoryVisibility.Type

export const OpenAgentsAutopilotAccessRequestKind = S.Literals([
  'customer_review',
  'github_account_link',
  'github_branch_write',
  'github_pull_request',
  'github_repo_read',
  'github_repo_write',
  'operator_review',
  'privacy_tier_confirmation',
  'pylon_enrollment',
  'repository_selection',
  'secret_broker',
  'site_deploy_review',
])
export type OpenAgentsAutopilotAccessRequestKind =
  typeof OpenAgentsAutopilotAccessRequestKind.Type

export const OpenAgentsAutopilotForumReportingMode = S.Literals([
  'campaign_topic',
  'operator_approved_only',
  'private',
  'public_safe_summary',
])
export type OpenAgentsAutopilotForumReportingMode =
  typeof OpenAgentsAutopilotForumReportingMode.Type

export const OpenAgentsAutopilotPrivacyTier = S.Literals([
  'cloud_allowed',
  'customer_local_pylon',
  'local_only',
  'maple_ai',
  'openagents_shc',
  'public_beta',
  'tee',
])
export type OpenAgentsAutopilotPrivacyTier =
  typeof OpenAgentsAutopilotPrivacyTier.Type

export const OpenAgentsAutopilotRunnerKind = S.Literals([
  'cloud_sandbox',
  'gcloud_credit',
  'maple_ai',
  'openagents_shc',
  'pylon_network',
  'requester_pylon',
  'shc',
  'tee',
])
export type OpenAgentsAutopilotRunnerKind =
  typeof OpenAgentsAutopilotRunnerKind.Type

export const OpenAgentsAutopilotBuyerPaymentMode = S.Literals([
  'free_slice',
  'free_slice_or_mdk_checkout_or_l402',
  'l402',
  'mdk_checkout',
  'paid_quote_required',
])
export type OpenAgentsAutopilotBuyerPaymentMode =
  typeof OpenAgentsAutopilotBuyerPaymentMode.Type

export const OpenAgentsAutopilotSettlementMode = S.Literals([
  'no_worker_payout',
  'no_worker_payout_until_accepted_work',
  'payable_after_accepted_work',
])
export type OpenAgentsAutopilotSettlementMode =
  typeof OpenAgentsAutopilotSettlementMode.Type

export const OpenAgentsAutopilotWorkState = S.Literals([
  'accepted_free_slice',
  'access_required',
  'blocked',
  'delivered',
  'invalid',
  'payment_required',
  'paid_ready',
  'queued_or_running',
])
export type OpenAgentsAutopilotWorkState =
  typeof OpenAgentsAutopilotWorkState.Type

export class OpenAgentsAutopilotCaller extends S.Class<OpenAgentsAutopilotCaller>(
  'OpenAgentsAutopilotCaller',
)({
  agentId: S.optionalKey(S.String),
  agentWalletRef: S.optionalKey(S.String),
  kind: OpenAgentsAutopilotCallerKind,
  ownerRef: S.optionalKey(S.String),
  pylonId: S.optionalKey(S.String),
}) {}

export class OpenAgentsAutopilotRepositoryRef extends S.Class<OpenAgentsAutopilotRepositoryRef>(
  'OpenAgentsAutopilotRepositoryRef',
)({
  branch: S.String,
  fullName: S.String,
  provider: OpenAgentsAutopilotRepositoryProvider,
  visibility: OpenAgentsAutopilotRepositoryVisibility,
}) {}

export class OpenAgentsAutopilotAccessRequest extends S.Class<OpenAgentsAutopilotAccessRequest>(
  'OpenAgentsAutopilotAccessRequest',
)({
  kind: OpenAgentsAutopilotAccessRequestKind,
  reasonRef: S.String,
}) {}

export class OpenAgentsAutopilotForumReportingPolicy extends S.Class<OpenAgentsAutopilotForumReportingPolicy>(
  'OpenAgentsAutopilotForumReportingPolicy',
)({
  mode: OpenAgentsAutopilotForumReportingMode,
  targetForumRef: S.optionalKey(S.String),
}) {}

export class OpenAgentsAutopilotTaskRequest extends S.Class<OpenAgentsAutopilotTaskRequest>(
  'OpenAgentsAutopilotTaskRequest',
)({
  acceptanceCriteriaRefs: S.Array(S.String),
  accessRequests: S.Array(OpenAgentsAutopilotAccessRequest),
  forumReporting: OpenAgentsAutopilotForumReportingPolicy,
  kind: OpenAgentsAutopilotTaskKind,
  objective: S.String,
  repository: S.optionalKey(OpenAgentsAutopilotRepositoryRef),
  taskRef: S.String,
}) {}

export class OpenAgentsAutopilotPlacementPolicy extends S.Class<OpenAgentsAutopilotPlacementPolicy>(
  'OpenAgentsAutopilotPlacementPolicy',
)({
  allowedRunnerKinds: S.Array(OpenAgentsAutopilotRunnerKind),
  disallowedRunnerKinds: S.Array(OpenAgentsAutopilotRunnerKind),
  localOnlyAllowed: S.Boolean,
  preferredRunnerKinds: S.Array(OpenAgentsAutopilotRunnerKind),
  privacyTier: OpenAgentsAutopilotPrivacyTier,
  publicTraceAllowed: S.Boolean,
  requiresSecretBroker: S.Boolean,
}) {}

export class OpenAgentsAutopilotPaymentPolicy extends S.Class<OpenAgentsAutopilotPaymentPolicy>(
  'OpenAgentsAutopilotPaymentPolicy',
)({
  buyerPaymentMode: OpenAgentsAutopilotBuyerPaymentMode,
  maxSpendCents: S.Number,
  quoteRef: S.NullOr(S.String),
  quotedAmountCents: S.NullOr(S.Number),
  settlementMode: OpenAgentsAutopilotSettlementMode,
}) {}

export class OpenAgentsAutopilotWorkRequest extends S.Class<OpenAgentsAutopilotWorkRequest>(
  'OpenAgentsAutopilotWorkRequest',
)({
  caller: OpenAgentsAutopilotCaller,
  clientRequestRef: S.String,
  intent: OpenAgentsAutopilotWorkRequestIntent,
  mode: OpenAgentsAutopilotWorkRequestMode,
  paymentPolicy: OpenAgentsAutopilotPaymentPolicy,
  placementPolicy: OpenAgentsAutopilotPlacementPolicy,
  schema: OpenAgentsAutopilotWorkRequestSchemaVersion,
  tasks: S.Array(OpenAgentsAutopilotTaskRequest),
}) {}

export class OpenAgentsAutopilotWorkResponseFixture extends S.Class<OpenAgentsAutopilotWorkResponseFixture>(
  'OpenAgentsAutopilotWorkResponseFixture',
)({
  accessRequestRefs: S.Array(S.String),
  eventStreamRef: S.String,
  paymentChallengeRef: S.NullOr(S.String),
  state: OpenAgentsAutopilotWorkState,
  statusUrlRef: S.String,
  taskRefs: S.Array(S.String),
  workOrderRef: S.String,
}) {}

export class OpenAgentsAutopilotWorkRequestUnsafe extends S.TaggedErrorClass<OpenAgentsAutopilotWorkRequestUnsafe>()(
  'OpenAgentsAutopilotWorkRequestUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const githubFullNamePattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/
const unsafeKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|checkout[_-]?id|cookie|customer[_-]?(email|name)|email[_-]?(address|body)|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(auth|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|tool[_-]?log|webhook)|secret[_-]?(material|value)|source[_-]?archive|token|webhook[_-]?secret)/i
const unsafeValuePattern =
  /(@|\/Users\/|\/home\/|\.mdk-wallet|access[_-]?token|auth\.json|bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github\.com\/[^:/]+\/private|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|private[_-]?repo|provider[_-]?(account|grant|payload|token)|raw[_-]?(auth|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|tool[_-]?log|webhook)|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet[_-]?(home|material|mnemonic|path|private|secret|state)|webhook[_-]?secret)/i

const normalizedUniqueStrings = (
  values: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(values.map(value => value.trim()).filter(value => value !== ''))]

const scanForUnsafeValue = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    if (
      path.at(-1) === 'kind' &&
      path.some(segment => segment === 'accessRequests')
    ) {
      return undefined
    }

    return unsafeValuePattern.test(value)
      ? path.join('.') || '<root>'
      : undefined
  }

  if (Array.isArray(value)) {
    return value
      .map((item, index) => scanForUnsafeValue(item, [...path, String(index)]))
      .find((unsafePath): unsafePath is string => unsafePath !== undefined)
  }

  if (value === null || typeof value !== 'object') {
    return undefined
  }

  return Object.entries(value)
    .map(([key, item]) =>
      unsafeKeyPattern.test(key)
        ? [...path, key].join('.')
        : scanForUnsafeValue(item, [...path, key])
    )
    .find((unsafePath): unsafePath is string => unsafePath !== undefined)
}

const assertSafeSerializedRequest = (
  request: OpenAgentsAutopilotWorkRequest,
): void => {
  const unsafePath = scanForUnsafeValue(request)

  if (
    unsafePath !== undefined ||
    openAgentsSerializedValueContainsUnsafeFixture(request)
  ) {
    throw new OpenAgentsAutopilotWorkRequestUnsafe({
      reason:
        'Autopilot work request contains private repo, secret, provider, payment, wallet, raw prompt, raw source, or runner material.',
    })
  }
}

const assertSafeRef = (label: string, value: string): void => {
  if (!safeRefPattern.test(value) || unsafeValuePattern.test(value)) {
    throw new OpenAgentsAutopilotWorkRequestUnsafe({
      reason: `${label} must be a stable public ref.`,
    })
  }
}

const assertNonEmptySafeRefs = (
  label: string,
  values: ReadonlyArray<string>,
): void => {
  if (values.length === 0) {
    throw new OpenAgentsAutopilotWorkRequestUnsafe({
      reason: `${label} must contain at least one ref.`,
    })
  }

  normalizedUniqueStrings(values).forEach(value => assertSafeRef(label, value))
}

const assertSafeOptionalRef = (
  label: string,
  value: string | undefined,
): void => {
  if (value !== undefined) {
    assertSafeRef(label, value)
  }
}

const assertSafeNullableRef = (
  label: string,
  value: string | null,
): void => {
  if (value !== null) {
    assertSafeRef(label, value)
  }
}

const assertSafeRepository = (
  repository: OpenAgentsAutopilotRepositoryRef | undefined,
): void => {
  if (repository === undefined) {
    return
  }

  if (repository.visibility !== 'public') {
    throw new OpenAgentsAutopilotWorkRequestUnsafe({
      reason:
        'Autopilot work request v1 only accepts public repositories until private access and secret broker states are modeled.',
    })
  }

  if (!githubFullNamePattern.test(repository.fullName)) {
    throw new OpenAgentsAutopilotWorkRequestUnsafe({
      reason: 'Repository fullName must be owner/repo without URLs or secrets.',
    })
  }

  assertSafeRef('repository branch', repository.branch)
}

const assertTask = (task: OpenAgentsAutopilotTaskRequest): void => {
  assertSafeRef('taskRef', task.taskRef)
  assertSafeRepository(task.repository)
  assertNonEmptySafeRefs(
    'acceptanceCriteriaRefs',
    task.acceptanceCriteriaRefs,
  )
  task.accessRequests.forEach(accessRequest => {
    assertSafeRef('access request reasonRef', accessRequest.reasonRef)
  })
  assertSafeOptionalRef(
    'forum reporting targetForumRef',
    task.forumReporting.targetForumRef,
  )

  if (task.objective.trim().length < 8 || unsafeValuePattern.test(task.objective)) {
    throw new OpenAgentsAutopilotWorkRequestUnsafe({
      reason:
        'Task objective must be a bounded public-safe summary, not a raw prompt or secret-shaped value.',
    })
  }
}

const assertPaymentPolicy = (
  paymentPolicy: OpenAgentsAutopilotPaymentPolicy,
): void => {
  if (paymentPolicy.maxSpendCents < 0) {
    throw new OpenAgentsAutopilotWorkRequestUnsafe({
      reason: 'maxSpendCents must be non-negative.',
    })
  }

  if (
    paymentPolicy.quotedAmountCents !== null &&
    paymentPolicy.quotedAmountCents < 0
  ) {
    throw new OpenAgentsAutopilotWorkRequestUnsafe({
      reason: 'quotedAmountCents must be non-negative when present.',
    })
  }

  assertSafeNullableRef('quoteRef', paymentPolicy.quoteRef)
}

const assertPlacementPolicy = (
  placementPolicy: OpenAgentsAutopilotPlacementPolicy,
): void => {
  if (placementPolicy.allowedRunnerKinds.length === 0) {
    throw new OpenAgentsAutopilotWorkRequestUnsafe({
      reason: 'allowedRunnerKinds must contain at least one runner kind.',
    })
  }

  if (
    placementPolicy.publicTraceAllowed &&
    (
      placementPolicy.privacyTier === 'local_only' ||
      placementPolicy.privacyTier === 'tee' ||
      placementPolicy.privacyTier === 'maple_ai'
    )
  ) {
    throw new OpenAgentsAutopilotWorkRequestUnsafe({
      reason:
        'Private or premium privacy tiers must not allow public traces in the v1 request contract.',
    })
  }
}

export const assertOpenAgentsAutopilotWorkRequest = (
  request: OpenAgentsAutopilotWorkRequest,
): void => {
  assertSafeSerializedRequest(request)
  assertSafeRef('clientRequestRef', request.clientRequestRef)
  assertSafeOptionalRef('caller agentId', request.caller.agentId)
  assertSafeOptionalRef('caller agentWalletRef', request.caller.agentWalletRef)
  assertSafeOptionalRef('caller ownerRef', request.caller.ownerRef)
  assertSafeOptionalRef('caller pylonId', request.caller.pylonId)

  if (request.tasks.length === 0) {
    throw new OpenAgentsAutopilotWorkRequestUnsafe({
      reason: 'Autopilot work request must include at least one typed task.',
    })
  }

  request.tasks.forEach(assertTask)
  assertPlacementPolicy(request.placementPolicy)
  assertPaymentPolicy(request.paymentPolicy)
}

export const decodeOpenAgentsAutopilotWorkRequest = (
  value: unknown,
): OpenAgentsAutopilotWorkRequest => {
  const request = S.decodeUnknownSync(OpenAgentsAutopilotWorkRequest)(value)
  assertOpenAgentsAutopilotWorkRequest(request)

  return request
}

export const OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES = [
  {
    caller: {
      agentId: 'oa_agent.docs_agent',
      agentWalletRef: 'wallet_ref.agent.docs_agent',
      kind: 'registered_agent',
      ownerRef: 'owner_ref.openagents_docs',
      pylonId: 'pylon.local.docs_agent',
    },
    clientRequestRef: 'client.example.20260609.001',
    intent: 'delegate_to_autopilot',
    mode: 'free_slice_or_paid_quote_or_l402',
    paymentPolicy: {
      buyerPaymentMode: 'free_slice',
      maxSpendCents: 0,
      quoteRef: null,
      quotedAmountCents: null,
      settlementMode: 'no_worker_payout',
    },
    placementPolicy: {
      allowedRunnerKinds: ['requester_pylon', 'openagents_shc'],
      disallowedRunnerKinds: [],
      localOnlyAllowed: false,
      preferredRunnerKinds: ['requester_pylon'],
      privacyTier: 'public_beta',
      publicTraceAllowed: true,
      requiresSecretBroker: false,
    },
    schema: 'openagents.autopilot_work_request.v1',
    tasks: [
      {
        acceptanceCriteriaRefs: [
          'acceptance.docs.updated',
          'acceptance.tests.contract',
        ],
        accessRequests: [
          {
            kind: 'github_repo_read',
            reasonRef: 'access.github.public_read',
          },
        ],
        forumReporting: {
          mode: 'public_safe_summary',
          targetForumRef: 'forum.product_promises.autopilot_coder',
        },
        kind: 'code_change',
        objective: 'Add public-safe Autopilot coder contract docs.',
        repository: {
          branch: 'main',
          fullName: 'OpenAgentsInc/openagents',
          provider: 'github',
          visibility: 'public',
        },
        taskRef: 'task.autopilot_coder.docs_contract',
      },
    ],
  },
  {
    caller: {
      agentId: 'oa_agent.paid_agent',
      agentWalletRef: 'wallet_ref.agent.paid_agent',
      kind: 'registered_agent',
      ownerRef: 'owner_ref.paid_customer',
    },
    clientRequestRef: 'client.example.20260609.002',
    intent: 'delegate_to_autopilot',
    mode: 'l402',
    paymentPolicy: {
      buyerPaymentMode: 'l402',
      maxSpendCents: 2500,
      quoteRef: 'quote.autopilot_coder.public_patch.1',
      quotedAmountCents: 2500,
      settlementMode: 'no_worker_payout_until_accepted_work',
    },
    placementPolicy: {
      allowedRunnerKinds: ['openagents_shc', 'cloud_sandbox'],
      disallowedRunnerKinds: [],
      localOnlyAllowed: false,
      preferredRunnerKinds: ['openagents_shc'],
      privacyTier: 'openagents_shc',
      publicTraceAllowed: false,
      requiresSecretBroker: false,
    },
    schema: 'openagents.autopilot_work_request.v1',
    tasks: [
      {
        acceptanceCriteriaRefs: [
          'acceptance.patch.applies',
          'acceptance.tests.pass',
        ],
        accessRequests: [],
        forumReporting: {
          mode: 'operator_approved_only',
        },
        kind: 'test_repair',
        objective: 'Repair the failing public test fixture and return a patch.',
        repository: {
          branch: 'main',
          fullName: 'OpenAgentsInc/openagents',
          provider: 'github',
          visibility: 'public',
        },
        taskRef: 'task.autopilot_coder.paid_test_repair',
      },
    ],
  },
] as const

export const OPENAGENTS_AUTOPILOT_WORK_RESPONSE_FIXTURES = [
  {
    accessRequestRefs: [],
    eventStreamRef: 'events.autopilot_work.client_example_20260609_001',
    paymentChallengeRef: null,
    state: 'accepted_free_slice',
    statusUrlRef: 'status.autopilot_work.client_example_20260609_001',
    taskRefs: ['task.autopilot_coder.docs_contract'],
    workOrderRef: 'work_order.autopilot.client_example_20260609_001',
  },
  {
    accessRequestRefs: [],
    eventStreamRef: 'events.autopilot_work.client_example_20260609_002',
    paymentChallengeRef: 'challenge.l402.autopilot_work.public_patch.1',
    state: 'payment_required',
    statusUrlRef: 'status.autopilot_work.client_example_20260609_002',
    taskRefs: ['task.autopilot_coder.paid_test_repair'],
    workOrderRef: 'work_order.autopilot.client_example_20260609_002',
  },
] as const
