import { Schema as S } from 'effect'

import type {
  AutopilotFallbackLeaseIntentProjection,
} from './autopilot-work-fallback-lease-adapter'
import type {
  AutopilotPylonAssignmentIntentProjection,
} from './autopilot-work-pylon-assignment-synthesizer'
import {
  OpenAgentsAutopilotRunnerKind,
  OpenAgentsAutopilotSettlementMode,
  OpenAgentsAutopilotTaskKind,
} from './autopilot-work-request'
import type {
  AutopilotWorkFundingProjection,
  AutopilotWorkTaskRecordProjection,
} from './autopilot-work-routes'
import {
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

export const OpenAgentsAutopilotCodingAssignmentSchemaVersion = S.Literal(
  'openagents.autopilot_coding_assignment.v1',
)
export type OpenAgentsAutopilotCodingAssignmentSchemaVersion =
  typeof OpenAgentsAutopilotCodingAssignmentSchemaVersion.Type

export const OpenAgentsAutopilotCodingAssignmentToolKind = S.Literals([
  'browser',
  'edit',
  'file',
  'git',
  'mcp',
  'search',
  'shell',
  'test_runner',
])
export type OpenAgentsAutopilotCodingAssignmentToolKind =
  typeof OpenAgentsAutopilotCodingAssignmentToolKind.Type

export const OpenAgentsAutopilotCodingAssignmentPaymentMode = S.Literals([
  'buyer_funded',
  'unpaid_smoke',
])
export type OpenAgentsAutopilotCodingAssignmentPaymentMode =
  typeof OpenAgentsAutopilotCodingAssignmentPaymentMode.Type

export class OpenAgentsAutopilotCodingObjective extends S.Class<OpenAgentsAutopilotCodingObjective>(
  'OpenAgentsAutopilotCodingObjective',
)({
  mode: S.Literal('ref_only'),
  objectiveRef: S.String,
  publicSummary: S.String,
  sourceTaskRef: S.String,
}) {}

export class OpenAgentsAutopilotCodingRepositoryRef extends S.Class<OpenAgentsAutopilotCodingRepositoryRef>(
  'OpenAgentsAutopilotCodingRepositoryRef',
)({
  branch: S.String,
  commitSha: S.optionalKey(S.String),
  fullName: S.String,
  provider: S.Literal('github'),
  visibility: S.Literal('public'),
}) {}

export class OpenAgentsAutopilotCodingVerificationCommand extends S.Class<OpenAgentsAutopilotCodingVerificationCommand>(
  'OpenAgentsAutopilotCodingVerificationCommand',
)({
  args: S.Array(S.String),
  commandRef: S.String,
}) {}

export class OpenAgentsAutopilotCodingGitCheckoutWorkspace extends S.Class<OpenAgentsAutopilotCodingGitCheckoutWorkspace>(
  'OpenAgentsAutopilotCodingGitCheckoutWorkspace',
)({
  kind: S.Literal('git_checkout'),
  repository: OpenAgentsAutopilotCodingRepositoryRef,
  verificationCommand: OpenAgentsAutopilotCodingVerificationCommand,
}) {}

export class OpenAgentsAutopilotCodingAuthorityRefs extends S.Class<OpenAgentsAutopilotCodingAuthorityRefs>(
  'OpenAgentsAutopilotCodingAuthorityRefs',
)({
  branchWriteAuthorityRefs: S.Array(S.String),
  deployAuthorityRefs: S.Array(S.String),
  pullRequestAuthorityRefs: S.Array(S.String),
  readAuthorityRefs: S.Array(S.String),
  spendAuthorityRefs: S.Array(S.String),
  writeAuthorityRefs: S.Array(S.String),
}) {}

export class OpenAgentsAutopilotCodingBudget extends S.Class<OpenAgentsAutopilotCodingBudget>(
  'OpenAgentsAutopilotCodingBudget',
)({
  buyerFundingState: S.Literals(['funded', 'not_required', 'payment_required']),
  maxSpendCents: S.Number,
  paymentChallengeRef: S.NullOr(S.String),
  paymentMode: OpenAgentsAutopilotCodingAssignmentPaymentMode,
  quoteRef: S.String,
  settlementMode: OpenAgentsAutopilotSettlementMode,
  spendCapRefs: S.Array(S.String),
  timeoutSeconds: S.Number,
  workerPayoutAuthority: S.Literal(false),
}) {}

export class OpenAgentsAutopilotCodingTracePolicy extends S.Class<OpenAgentsAutopilotCodingTracePolicy>(
  'OpenAgentsAutopilotCodingTracePolicy',
)({
  operatorEvidenceAllowed: S.Boolean,
  publicTraceAllowed: S.Boolean,
  rawPromptAllowed: S.Literal(false),
  rawProviderPayloadAllowed: S.Literal(false),
  rawRunnerLogAllowed: S.Literal(false),
  rawSourceArchiveAllowed: S.Literal(false),
}) {}

export class OpenAgentsAutopilotCodingCloseoutSchema extends S.Class<OpenAgentsAutopilotCodingCloseoutSchema>(
  'OpenAgentsAutopilotCodingCloseoutSchema',
)({
  acceptedWorkAuthority: S.Literal(false),
  closeoutPathRefs: S.Array(S.String),
  diffOrSummaryRequired: S.Boolean,
  resultExpectationRefs: S.Array(S.String),
  testsOrBlockerRequired: S.Boolean,
}) {}

export class OpenAgentsAutopilotCodingClaudeAgentTask extends S.Class<OpenAgentsAutopilotCodingClaudeAgentTask>(
  'OpenAgentsAutopilotCodingClaudeAgentTask',
)({
  agentKind: S.Literal('claude_agent_sdk'),
  allowedToolKinds: S.Array(OpenAgentsAutopilotCodingAssignmentToolKind),
  maxTurns: S.Number,
  schema: S.Literal('openagents.pylon.claude_agent_task.v0.3'),
  timeoutSeconds: S.Number,
}) {}

export class OpenAgentsAutopilotCodingCodexTask extends S.Class<OpenAgentsAutopilotCodingCodexTask>(
  'OpenAgentsAutopilotCodingCodexTask',
)({
  agentKind: S.Literal('codex_sdk'),
  schema: S.Literal('openagents.pylon.codex_agent_task.v0.3'),
  timeoutSeconds: S.Number,
}) {}

export class OpenAgentsAutopilotCodingAssignmentPayload extends S.Class<OpenAgentsAutopilotCodingAssignmentPayload>(
  'OpenAgentsAutopilotCodingAssignmentPayload',
)({
  acceptanceCriteriaRefs: S.Array(S.String),
  allowedToolKinds: S.Array(OpenAgentsAutopilotCodingAssignmentToolKind),
  assignmentRef: S.String,
  authRefs: S.Array(S.String),
  authorities: OpenAgentsAutopilotCodingAuthorityRefs,
  budget: OpenAgentsAutopilotCodingBudget,
  closeoutSchema: OpenAgentsAutopilotCodingCloseoutSchema,
  claudeAgent: S.optionalKey(OpenAgentsAutopilotCodingClaudeAgentTask),
  codex: S.optionalKey(OpenAgentsAutopilotCodingCodexTask),
  laneRef: S.String,
  objective: OpenAgentsAutopilotCodingObjective,
  publicSafe: S.Literal(true),
  repository: S.NullOr(OpenAgentsAutopilotCodingRepositoryRef),
  requiredCapabilityRefs: S.Array(S.String),
  rollbackRefs: S.Array(S.String),
  runnerKind: OpenAgentsAutopilotRunnerKind,
  runnerRef: S.String,
  schema: OpenAgentsAutopilotCodingAssignmentSchemaVersion,
  sourceRefs: S.Array(S.String),
  taskKind: OpenAgentsAutopilotTaskKind,
  taskRef: S.String,
  tracePolicy: OpenAgentsAutopilotCodingTracePolicy,
  workOrderRef: S.String,
  workspace: S.optionalKey(OpenAgentsAutopilotCodingGitCheckoutWorkspace),
}) {}

export class OpenAgentsAutopilotCodingAssignmentUnsafe extends S.TaggedErrorClass<OpenAgentsAutopilotCodingAssignmentUnsafe>()(
  'OpenAgentsAutopilotCodingAssignmentUnsafe',
  {
    reason: S.String,
  },
) {}

type CodingAssignmentSourceWork = Readonly<{
  fallbackLeaseIntents: ReadonlyArray<AutopilotFallbackLeaseIntentProjection>
  funding: AutopilotWorkFundingProjection
  paymentChallengeRef: string | null
  pylonAssignmentIntents: ReadonlyArray<AutopilotPylonAssignmentIntentProjection>
  quote: Readonly<{
    maxSpendCents: number
    settlementMode: typeof OpenAgentsAutopilotSettlementMode.Type
  }>
  tasks: ReadonlyArray<AutopilotWorkTaskRecordProjection>
  workOrderRef: string
}>

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const githubFullNamePattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/
const gitCommitShaPattern = /^[a-f0-9]{40}$/i
const placeholderCommitShaPattern = /^(0{40}|1{40})$/i
const verificationCommandArgPattern = /^[A-Za-z0-9_./:=@+-]{1,120}$/
const unsafeKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|checkout|cookie|customer[_-]?(email|name)|email[_-]?(address|body)|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(auth|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|tool[_-]?log|webhook)|secret|source[_-]?archive|token|wallet)/i
const unsafeValuePattern =
  /(@|\/Users\/|\/home\/|\.mdk-wallet|access[_-]?token|auth\.json|bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github\.com\/[^:/]+\/private|github[_-]?pat_[a-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|private[_-]?repo|provider[_-]?(account|grant|payload|token)|raw[_-]?(auth|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|tool[_-]?log|webhook)|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet[_-]?(home|material|mnemonic|path|private|secret|state)|webhook[_-]?secret)/i
const safeFalsePolicyKeys = new Set([
  'rawPromptAllowed',
  'rawProviderPayloadAllowed',
  'rawRunnerLogAllowed',
  'rawSourceArchiveAllowed',
])

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const scanForUnsafeValue = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
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
      unsafeKeyPattern.test(key) &&
      !(safeFalsePolicyKeys.has(key) && item === false)
        ? [...path, key].join('.')
        : scanForUnsafeValue(item, [...path, key])
    )
    .find((unsafePath): unsafePath is string => unsafePath !== undefined)
}

const assertSafeRef = (label: string, value: string): void => {
  if (!safeRefPattern.test(value) || unsafeValuePattern.test(value)) {
    throw new OpenAgentsAutopilotCodingAssignmentUnsafe({
      reason: `${label} must be a stable public ref.`,
    })
  }
}

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  uniqueRefs(refs).forEach(ref => assertSafeRef(label, ref))
}

const assertSafeRepository = (
  repository: OpenAgentsAutopilotCodingRepositoryRef | null,
): void => {
  if (repository === null) {
    return
  }

  if (!githubFullNamePattern.test(repository.fullName)) {
    throw new OpenAgentsAutopilotCodingAssignmentUnsafe({
      reason: 'Repository fullName must be owner/repo without URLs or secrets.',
    })
  }

  assertSafeRef('repository branch', repository.branch)
  if (
    repository.commitSha !== undefined &&
    (!gitCommitShaPattern.test(repository.commitSha) ||
      placeholderCommitShaPattern.test(repository.commitSha))
  ) {
    throw new OpenAgentsAutopilotCodingAssignmentUnsafe({
      reason:
        'Repository commitSha must be a real pinned 40-character commit SHA, not a placeholder.',
    })
  }
}

const assertSafeVerificationCommand = (
  command: OpenAgentsAutopilotCodingVerificationCommand,
): void => {
  assertSafeRef('verification commandRef', command.commandRef)

  if (command.args.length === 0) {
    throw new OpenAgentsAutopilotCodingAssignmentUnsafe({
      reason: 'Verification command must contain at least one argv token.',
    })
  }

  command.args.forEach(arg => {
    if (
      !verificationCommandArgPattern.test(arg) ||
      arg.includes('..') ||
      arg.startsWith('/')
    ) {
      throw new OpenAgentsAutopilotCodingAssignmentUnsafe({
        reason:
          'Verification command args must be bounded argv tokens without absolute paths, traversal, shell, or secret material.',
      })
    }
    assertSafeRef('verification command arg', arg)
  })
}

export const assertOpenAgentsAutopilotCodingAssignmentPayloadSafe = (
  payload: OpenAgentsAutopilotCodingAssignmentPayload,
): void => {
  const unsafePath = scanForUnsafeValue(payload)

  if (
    unsafePath !== undefined ||
    openAgentsSerializedValueContainsUnsafeFixture(payload)
  ) {
    throw new OpenAgentsAutopilotCodingAssignmentUnsafe({
      reason:
        `Autopilot coding assignment contains private repo data, raw prompts, local paths, provider payloads, payment material, wallet material, source archives, or secrets${unsafePath === undefined ? '.' : ` at ${unsafePath}.`}`,
    })
  }

  assertSafeRef('assignmentRef', payload.assignmentRef)
  assertSafeRef('workOrderRef', payload.workOrderRef)
  assertSafeRef('taskRef', payload.taskRef)
  assertSafeRef('runnerRef', payload.runnerRef)
  assertSafeRef('laneRef', payload.laneRef)
  assertSafeRepository(payload.repository)
  assertSafeRefs('acceptanceCriteriaRefs', payload.acceptanceCriteriaRefs)
  assertSafeRefs('allowed auth refs', payload.authRefs)
  assertSafeRefs('required capability refs', payload.requiredCapabilityRefs)
  assertSafeRefs('rollback refs', payload.rollbackRefs)
  assertSafeRefs('source refs', payload.sourceRefs)
  assertSafeRefs('branch authority refs', payload.authorities.branchWriteAuthorityRefs)
  assertSafeRefs('deploy authority refs', payload.authorities.deployAuthorityRefs)
  assertSafeRefs('pull request authority refs', payload.authorities.pullRequestAuthorityRefs)
  assertSafeRefs('read authority refs', payload.authorities.readAuthorityRefs)
  assertSafeRefs('spend authority refs', payload.authorities.spendAuthorityRefs)
  assertSafeRefs('write authority refs', payload.authorities.writeAuthorityRefs)
  assertSafeRefs('spend cap refs', payload.budget.spendCapRefs)
  assertSafeRefs('closeout path refs', payload.closeoutSchema.closeoutPathRefs)
  assertSafeRefs(
    'result expectation refs',
    payload.closeoutSchema.resultExpectationRefs,
  )
  if (payload.workspace !== undefined) {
    assertSafeRepository(payload.workspace.repository)
    assertSafeVerificationCommand(payload.workspace.verificationCommand)
  }
}

export const decodeOpenAgentsAutopilotCodingAssignmentPayload = (
  value: unknown,
): OpenAgentsAutopilotCodingAssignmentPayload => {
  const payload = S.decodeUnknownSync(
    OpenAgentsAutopilotCodingAssignmentPayload,
  )(value)

  assertOpenAgentsAutopilotCodingAssignmentPayloadSafe(payload)

  return payload
}

const allowedToolKindsForTask = (
  task: AutopilotWorkTaskRecordProjection,
): ReadonlyArray<OpenAgentsAutopilotCodingAssignmentToolKind> => {
  switch (task.kind) {
    case 'benchmark_or_gepa':
    case 'research_and_patch':
      return ['file', 'git', 'search', 'shell', 'test_runner']
    case 'code_change':
    case 'repo_change':
    case 'test_repair':
      return ['edit', 'file', 'git', 'shell', 'test_runner']
    case 'site_adjustment':
    case 'site_generation':
      return ['browser', 'edit', 'file', 'git', 'shell', 'test_runner']
  }
}

const repositoryForTask = (
  task: AutopilotWorkTaskRecordProjection,
): OpenAgentsAutopilotCodingRepositoryRef | null => {
  const repository = task.repository ?? null
  const checkout = task.checkout ?? null

  if (repository === null) {
    return null
  }

  if (repository.visibility !== 'public') {
    throw new OpenAgentsAutopilotCodingAssignmentUnsafe({
      reason:
        'Normalized Autopilot coding assignments require public repo refs until private access and secret broker lanes are modeled.',
    })
  }

  return new OpenAgentsAutopilotCodingRepositoryRef({
    branch: repository.branch,
    ...(checkout === null
      ? {}
      : { commitSha: checkout.commitSha }),
    fullName: repository.fullName,
    provider: repository.provider,
    visibility: 'public',
  })
}

const authorityRefsForTask = (
  task: AutopilotWorkTaskRecordProjection,
): OpenAgentsAutopilotCodingAuthorityRefs => {
  const accessKinds = new Set(
    task.accessRequirements.map(requirement => requirement.kind),
  )

  return new OpenAgentsAutopilotCodingAuthorityRefs({
    branchWriteAuthorityRefs:
      accessKinds.has('github_branch_write')
        ? [`authority.${task.taskRef}.branch_write.owner_grant_required`]
        : [],
    deployAuthorityRefs: ['authority.autopilot.deploy.disabled'],
    pullRequestAuthorityRefs:
      accessKinds.has('github_pull_request')
        ? [`authority.${task.taskRef}.pull_request.owner_grant_required`]
        : [],
    readAuthorityRefs:
      task.repository?.visibility === 'public'
        ? [`authority.${task.taskRef}.repo_read.public`]
        : [],
    spendAuthorityRefs: ['authority.autopilot.spend.disabled'],
    writeAuthorityRefs:
      accessKinds.has('github_repo_write') ||
      accessKinds.has('github_branch_write')
        ? [`authority.${task.taskRef}.repo_write.owner_grant_required`]
        : [],
  })
}

const authRefsForTask = (
  task: AutopilotWorkTaskRecordProjection,
): ReadonlyArray<string> =>
  task.accessRequirements.length === 0
    ? ['auth.public_or_grant_satisfied']
    : task.accessRequirements.map(requirement => requirement.ownerActionRef)

const paymentModeForIntent = (
  intent: AutopilotFallbackLeaseIntentProjection | AutopilotPylonAssignmentIntentProjection,
): OpenAgentsAutopilotCodingAssignmentPaymentMode =>
  intent.paymentMode === 'buyer_funded' ||
  intent.paymentMode === 'payable_pending_settlement'
    ? 'buyer_funded'
    : 'unpaid_smoke'

const codingAssignmentForIntent = (
  input: Readonly<{
    fallbackLaneRef?: string
    funding: AutopilotWorkFundingProjection
    intent: AutopilotFallbackLeaseIntentProjection | AutopilotPylonAssignmentIntentProjection
    paymentChallengeRef: string | null
    quote: CodingAssignmentSourceWork['quote']
    runnerKind: typeof OpenAgentsAutopilotRunnerKind.Type
    runnerRef: string
    task: AutopilotWorkTaskRecordProjection
    workOrderRef: string
  }>,
): OpenAgentsAutopilotCodingAssignmentPayload => {
  const repository = repositoryForTask(input.task)
  const allowedToolKinds = allowedToolKindsForTask(input.task)
  const checkout = input.task.checkout ?? null
  const workspace =
    repository === null || checkout === null
      ? undefined
      : new OpenAgentsAutopilotCodingGitCheckoutWorkspace({
          kind: 'git_checkout',
          repository,
          verificationCommand: new OpenAgentsAutopilotCodingVerificationCommand({
            args: checkout.verificationCommand.args,
            commandRef: checkout.verificationCommand.commandRef,
          }),
        })
  const payload = new OpenAgentsAutopilotCodingAssignmentPayload({
    acceptanceCriteriaRefs: uniqueRefs(input.intent.acceptanceCriteriaRefs),
    allowedToolKinds,
    assignmentRef: input.intent.assignmentRef,
    authRefs: authRefsForTask(input.task),
    authorities: authorityRefsForTask(input.task),
    budget: new OpenAgentsAutopilotCodingBudget({
      buyerFundingState: input.funding.buyerFundingState,
      maxSpendCents: input.quote.maxSpendCents,
      paymentChallengeRef: input.paymentChallengeRef,
      paymentMode: paymentModeForIntent(input.intent),
      quoteRef: input.funding.quoteRef,
      settlementMode: input.quote.settlementMode,
      spendCapRefs: uniqueRefs(input.intent.spendCapRefs),
      timeoutSeconds: 15 * 60,
      workerPayoutAuthority: false,
    }),
    closeoutSchema: new OpenAgentsAutopilotCodingCloseoutSchema({
      acceptedWorkAuthority: false,
      closeoutPathRefs: uniqueRefs(input.intent.closeoutPathRefs),
      diffOrSummaryRequired: true,
      resultExpectationRefs: uniqueRefs(input.intent.resultExpectationRefs),
      testsOrBlockerRequired: true,
    }),
    ...(input.intent.jobKind === 'codex_agent_task'
      ? {
          codex: new OpenAgentsAutopilotCodingCodexTask({
            agentKind: 'codex_sdk',
            schema: 'openagents.pylon.codex_agent_task.v0.3',
            timeoutSeconds: 15 * 60,
          }),
        }
      : {
          claudeAgent: new OpenAgentsAutopilotCodingClaudeAgentTask({
            agentKind: 'claude_agent_sdk',
            allowedToolKinds,
            maxTurns: 24,
            schema: 'openagents.pylon.claude_agent_task.v0.3',
            timeoutSeconds: 15 * 60,
          }),
        }),
    laneRef: input.fallbackLaneRef ?? `lane.${input.runnerKind}`,
    objective: new OpenAgentsAutopilotCodingObjective({
      mode: 'ref_only',
      objectiveRef: `objective.${input.workOrderRef}.${input.task.taskRef}`,
      publicSummary: input.task.objective,
      sourceTaskRef: input.task.taskRef,
    }),
    publicSafe: true,
    repository,
    requiredCapabilityRefs: uniqueRefs(input.intent.requiredCapabilityRefs),
    rollbackRefs: uniqueRefs(input.intent.rollbackRefs),
    runnerKind: input.runnerKind,
    runnerRef: input.runnerRef,
    schema: 'openagents.autopilot_coding_assignment.v1',
    sourceRefs: uniqueRefs([
      input.workOrderRef,
      input.task.taskRef,
      `source.autopilot_work.${input.workOrderRef}`,
      ...input.intent.selectionPolicyRefs,
    ]),
    taskKind: input.task.kind,
    taskRef: input.task.taskRef,
    tracePolicy: new OpenAgentsAutopilotCodingTracePolicy({
      operatorEvidenceAllowed: true,
      publicTraceAllowed: true,
      rawPromptAllowed: false,
      rawProviderPayloadAllowed: false,
      rawRunnerLogAllowed: false,
      rawSourceArchiveAllowed: false,
    }),
    workOrderRef: input.workOrderRef,
    ...(workspace === undefined ? {} : { workspace }),
  })

  assertOpenAgentsAutopilotCodingAssignmentPayloadSafe(payload)

  return payload
}

export const autopilotCodingAssignmentsForWork = (
  work: CodingAssignmentSourceWork,
): ReadonlyArray<OpenAgentsAutopilotCodingAssignmentPayload> => {
  const tasksByRef = new Map(work.tasks.map(task => [task.taskRef, task]))
  const pylonAssignments = work.pylonAssignmentIntents.flatMap(intent => {
    const task = tasksByRef.get(intent.taskRef)

    return task === undefined
      ? []
      : [
          codingAssignmentForIntent({
            funding: work.funding,
            intent,
            paymentChallengeRef: work.paymentChallengeRef,
            quote: work.quote,
            runnerKind: 'requester_pylon',
            runnerRef: intent.pylonRef,
            task,
            workOrderRef: work.workOrderRef,
          }),
        ]
  })
  const fallbackAssignments = work.fallbackLeaseIntents.flatMap(intent => {
    const task = tasksByRef.get(intent.taskRef)

    return task === undefined
      ? []
      : [
          codingAssignmentForIntent({
            fallbackLaneRef: intent.fallbackLaneRef,
            funding: work.funding,
            intent,
            paymentChallengeRef: work.paymentChallengeRef,
            quote: work.quote,
            runnerKind: intent.runnerKind,
            runnerRef: intent.fallbackLaneRef,
            task,
            workOrderRef: work.workOrderRef,
          }),
        ]
  })

  return [...pylonAssignments, ...fallbackAssignments]
}
