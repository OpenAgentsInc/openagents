import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  artanisProductionLaunchGateCheckInputFromGepaSmoke,
  exampleArtanisGepaProductionSmokeRecord,
} from './artanis-gepa-production-smoke'
import {
  artanisProductionLaunchGateCheckInputFromGepaScheduledRunnerProof,
  exampleArtanisGepaScheduledRunnerProofRecord,
} from './artanis-gepa-scheduled-runner-proof'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const ArtanisProductionLaunchGateStatus = S.Literals([
  'blocked',
  'not_required',
  'passed',
  'pending',
])
export type ArtanisProductionLaunchGateStatus =
  typeof ArtanisProductionLaunchGateStatus.Type

export const ArtanisProductionLaunchGateCategory = S.Literals([
  'approval_gates',
  'continual_learning_templates',
  'forum_delivery',
  'forum_listener',
  'marketplace_intake',
  'nexus_pylon_adapter',
  'operator_console',
  'payment_reward_boundary',
  'persistence',
  'production_e2e_smoke',
  'public_report_projection',
  'rollback_runbook',
  'scheduled_runner',
])
export type ArtanisProductionLaunchGateCategory =
  typeof ArtanisProductionLaunchGateCategory.Type

export const ArtanisLaunchRunbookCommandKind = S.Literals([
  'check',
  'disable',
  'enable',
  'pause',
  'recover',
  'revoke',
])
export type ArtanisLaunchRunbookCommandKind =
  typeof ArtanisLaunchRunbookCommandKind.Type

export const ArtanisLaunchRollbackKind = S.Literals([
  'dispatch',
  'payment_reward',
  'publication',
  'public_claim',
])
export type ArtanisLaunchRollbackKind = typeof ArtanisLaunchRollbackKind.Type

export class ArtanisProductionLaunchGateCheck extends S.Class<ArtanisProductionLaunchGateCheck>(
  'ArtanisProductionLaunchGateCheck',
)({
  category: ArtanisProductionLaunchGateCategory,
  checkRef: S.String,
  description: S.String,
  issueRefs: S.Array(S.String),
  requiredForAutonomousClaim: S.Boolean,
  routeRefs: S.Array(S.String),
  status: ArtanisProductionLaunchGateStatus,
  testRefs: S.Array(S.String),
}) {}

export class ArtanisLaunchRunbookCommand extends S.Class<ArtanisLaunchRunbookCommand>(
  'ArtanisLaunchRunbookCommand',
)({
  command: S.String,
  description: S.String,
  kind: ArtanisLaunchRunbookCommandKind,
  requiresEnvRefs: S.Array(S.String),
  runFromRef: S.String,
}) {}

export class ArtanisLaunchRollbackStep extends S.Class<ArtanisLaunchRollbackStep>(
  'ArtanisLaunchRollbackStep',
)({
  commandRefs: S.Array(S.String),
  description: S.String,
  kind: ArtanisLaunchRollbackKind,
  rollbackRef: S.String,
}) {}

export class ArtanisProductionVerificationTarget extends S.Class<ArtanisProductionVerificationTarget>(
  'ArtanisProductionVerificationTarget',
)({
  expectedSignal: S.String,
  targetRef: S.String,
}) {}

export class ArtanisProductionLaunchGateRecord extends S.Class<ArtanisProductionLaunchGateRecord>(
  'ArtanisProductionLaunchGateRecord',
)({
  agentRef: S.String,
  checks: S.Array(ArtanisProductionLaunchGateCheck),
  environmentRef: S.String,
  gateRef: S.String,
  publicBlockedClaimPhrases: S.Array(S.String),
  publicSafeClaimPhrases: S.Array(S.String),
  rollbackSteps: S.Array(ArtanisLaunchRollbackStep),
  runbookCommands: S.Array(ArtanisLaunchRunbookCommand),
  updatedAtIso: S.String,
  verificationTargets: S.Array(ArtanisProductionVerificationTarget),
}) {}

export class ArtanisProductionLaunchGateProjection extends S.Class<ArtanisProductionLaunchGateProjection>(
  'ArtanisProductionLaunchGateProjection',
)({
  agentRef: S.String,
  blockerRefs: S.Array(S.String),
  canClaimBoundedStatusProjection: S.Boolean,
  canClaimContinuouslyRunning: S.Boolean,
  checkCount: S.Number,
  checkRefs: S.Array(S.String),
  dispatchAuthorityAllowed: S.Boolean,
  docsRefs: S.Array(S.String),
  enableCommandRefs: S.Array(S.String),
  environmentRef: S.String,
  failedOrPendingRequiredCount: S.Number,
  forumAutoPublishAllowed: S.Boolean,
  gateRef: S.String,
  providerMutationAuthorityAllowed: S.Boolean,
  publicBlockedClaimPhrases: S.Array(S.String),
  publicSafeClaimPhrases: S.Array(S.String),
  requiredIssueRefs: S.Array(S.String),
  rollbackRefs: S.Array(S.String),
  routeRefs: S.Array(S.String),
  runbookCommandRefs: S.Array(S.String),
  settlementAuthorityAllowed: S.Boolean,
  state: S.Literals(['blocked', 'ready']),
  stateLabel: S.String,
  testRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  verificationTargetRefs: S.Array(S.String),
  walletSpendAuthorityAllowed: S.Boolean,
}) {}

export class ArtanisProductionLaunchGateUnsafe extends S.TaggedErrorClass<ArtanisProductionLaunchGateUnsafe>()(
  'ArtanisProductionLaunchGateUnsafe',
  {
    reason: S.String,
  },
) {}

const requiredCategories: ReadonlyArray<ArtanisProductionLaunchGateCategory> = [
  'approval_gates',
  'continual_learning_templates',
  'forum_delivery',
  'forum_listener',
  'marketplace_intake',
  'nexus_pylon_adapter',
  'operator_console',
  'payment_reward_boundary',
  'persistence',
  'production_e2e_smoke',
  'public_report_projection',
  'rollback_runbook',
  'scheduled_runner',
]

const requiredVerificationTargetRefs = [
  'route:/artanis',
  'route:/api/public/artanis/report',
  'route:/api/public/pylon-stats',
  'route:/api/operator/artanis/console',
  'route:/api/operator/artanis/approval-gates',
  'route:/autopilot',
  'topic.public.forum.artanis.status',
  'signal.public.artanis.health_staleness',
] as const

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/#{}-]{0,280}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const unsafeCommandPattern =
  /(ARTANIS_SCHEDULED_RUNNER_ENABLED[=:](?!true|false)\S+|OPENAGENTS_ADMIN_API_TOKEN=\S+|RESEND_API_KEY=\S+|bearer (?!\$OPENAGENTS_ADMIN_API_TOKEN\b)[A-Za-z0-9._-]+|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|sk-[a-z0-9]|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mnemonic|recovery phrase|seed phrase|private key|wallet secret|wallet seed|preimage|payment hash|raw invoice|\/Users\/|\/home\/)/i
const unsafePublicClaimPattern =
  /(continuously running autonomously|fully autonomous|autonomous production administrator|Artanis is running on its own|Pylon v0\.2 is shipped|Pylon v0\.2 is live for everyone|paid work is settled|wallet spend is live|accepted work payouts are live)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const stringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(stringValues)
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value as Record<string, unknown>).flatMap(stringValues)
  }

  return []
}

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ArtanisProductionLaunchGateUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, private repo, raw material, or a raw timestamp.`,
    })
  }
}

const assertRunbookCommandSafe = (
  command: ArtanisLaunchRunbookCommand,
): void => {
  if (
    command.command.trim().length < 8 ||
    containsProviderSecretMaterial(command.command) ||
    unsafeCommandPattern.test(command.command) ||
    rawTimestampPattern.test(command.command)
  ) {
    throw new ArtanisProductionLaunchGateUnsafe({
      reason:
        'Artanis production launch runbook commands must be concrete but cannot expose literal secrets, wallet material, payment material, private paths, or raw timestamps.',
    })
  }

  assertSafeRefs('Artanis launch command refs', [
    command.kind,
    command.runFromRef,
    ...command.requiresEnvRefs,
  ])
}

const assertCopySafe = (
  projection: ArtanisProductionLaunchGateProjection,
): void => {
  const unsafeBlockedCopy = projection.publicBlockedClaimPhrases.find(
    phrase =>
      unsafePublicClaimPattern.test(phrase) ||
      unsafeRefPattern.test(phrase) ||
      rawTimestampPattern.test(phrase) ||
      containsProviderSecretMaterial(phrase),
  )
  const unsafeSafeCopy = projection.publicSafeClaimPhrases.find(
    phrase =>
      unsafePublicClaimPattern.test(phrase) ||
      unsafeRefPattern.test(phrase) ||
      rawTimestampPattern.test(phrase) ||
      containsProviderSecretMaterial(phrase),
  )

  if (unsafeBlockedCopy !== undefined || unsafeSafeCopy !== undefined) {
    throw new ArtanisProductionLaunchGateUnsafe({
      reason:
        'Artanis launch gate public-copy policy must use blocked-claim refs and keep safe copy conservative.',
    })
  }
}

const assertRequiredCategoriesPresent = (
  checks: ReadonlyArray<ArtanisProductionLaunchGateCheck>,
): void => {
  const categories = new Set(checks.map(check => check.category))
  const missing = requiredCategories.filter(
    category => !categories.has(category),
  )

  if (missing.length > 0) {
    throw new ArtanisProductionLaunchGateUnsafe({
      reason: `Artanis production launch gate missing required categories: ${missing.join(', ')}.`,
    })
  }
}

const assertRequiredRunbookKindsPresent = (
  commands: ReadonlyArray<ArtanisLaunchRunbookCommand>,
): void => {
  const kinds = new Set(commands.map(command => command.kind))
  const missing = [
    'check',
    'disable',
    'enable',
    'pause',
    'recover',
    'revoke',
  ].filter(kind => !kinds.has(kind as ArtanisLaunchRunbookCommandKind))

  if (missing.length > 0) {
    throw new ArtanisProductionLaunchGateUnsafe({
      reason: `Artanis production runbook missing command kinds: ${missing.join(', ')}.`,
    })
  }
}

const assertRequiredRollbackKindsPresent = (
  steps: ReadonlyArray<ArtanisLaunchRollbackStep>,
): void => {
  const kinds = new Set(steps.map(step => step.kind))
  const missing = [
    'dispatch',
    'payment_reward',
    'publication',
    'public_claim',
  ].filter(kind => !kinds.has(kind as ArtanisLaunchRollbackKind))

  if (missing.length > 0) {
    throw new ArtanisProductionLaunchGateUnsafe({
      reason: `Artanis production rollback runbook missing rollback kinds: ${missing.join(', ')}.`,
    })
  }
}

const assertRequiredVerificationTargetsPresent = (
  targets: ReadonlyArray<ArtanisProductionVerificationTarget>,
): void => {
  const targetRefs = new Set(targets.map(target => target.targetRef))
  const missing = requiredVerificationTargetRefs.filter(
    ref => !targetRefs.has(ref),
  )

  if (missing.length > 0) {
    throw new ArtanisProductionLaunchGateUnsafe({
      reason: `Artanis production launch gate missing verification targets: ${missing.join(', ')}.`,
    })
  }
}

const assertRecordSafe = (record: ArtanisProductionLaunchGateRecord): void => {
  if (!Number.isFinite(Date.parse(record.updatedAtIso))) {
    throw new ArtanisProductionLaunchGateUnsafe({
      reason: 'Artanis production launch gate updatedAtIso must be valid.',
    })
  }

  assertRequiredCategoriesPresent(record.checks)
  assertRequiredRunbookKindsPresent(record.runbookCommands)
  assertRequiredRollbackKindsPresent(record.rollbackSteps)
  assertRequiredVerificationTargetsPresent(record.verificationTargets)

  assertSafeRefs('Artanis production launch gate identity refs', [
    record.agentRef,
    record.environmentRef,
    record.gateRef,
  ])
  record.checks.forEach(check =>
    assertSafeRefs('Artanis production launch gate check refs', [
      check.category,
      check.checkRef,
      check.status,
      ...check.issueRefs,
      ...check.routeRefs,
      ...check.testRefs,
    ]),
  )
  record.runbookCommands.forEach(assertRunbookCommandSafe)
  record.rollbackSteps.forEach(step =>
    assertSafeRefs('Artanis production rollback refs', [
      step.kind,
      step.rollbackRef,
      ...step.commandRefs,
    ]),
  )
  record.verificationTargets.forEach(target =>
    assertSafeRefs('Artanis production verification refs', [target.targetRef]),
  )
}

export const projectArtanisProductionLaunchGate = (
  record: ArtanisProductionLaunchGateRecord,
  nowIso: string,
): ArtanisProductionLaunchGateProjection => {
  assertRecordSafe(record)

  const failedOrPendingRequiredChecks = record.checks.filter(
    check =>
      check.requiredForAutonomousClaim &&
      check.status !== 'passed' &&
      check.status !== 'not_required',
  )
  const state = failedOrPendingRequiredChecks.length === 0 ? 'ready' : 'blocked'
  const runbookCommandRefs = record.runbookCommands.map(
    command => `runbook.public.artanis.production_launch.${command.kind}`,
  )
  const projection = new ArtanisProductionLaunchGateProjection({
    agentRef: record.agentRef,
    blockerRefs: uniqueRefs(
      failedOrPendingRequiredChecks.map(
        check =>
          `blocker.public.artanis.launch_gate.${check.category}.${check.status}`,
      ),
    ),
    canClaimBoundedStatusProjection: state === 'ready',
    canClaimContinuouslyRunning: false,
    checkCount: record.checks.length,
    checkRefs: uniqueRefs(record.checks.map(check => check.checkRef)),
    dispatchAuthorityAllowed: false,
    docsRefs: uniqueRefs([
      'docs/artanis/2026-06-08-bounded-gepa-scheduled-runner.md',
      'docs/artanis/2026-06-08-probe-gepa-pylon-production-equivalent-smoke.md',
      'docs/artanis/2026-06-06-production-launch-gate-runbook.md',
      'docs/artanis/2026-06-06-artanis-deployment-readiness-audit.md',
    ]),
    enableCommandRefs: uniqueRefs(
      runbookCommandRefs.filter(
        ref => ref.endsWith('.enable') || ref.endsWith('.disable'),
      ),
    ),
    environmentRef: record.environmentRef,
    failedOrPendingRequiredCount: failedOrPendingRequiredChecks.length,
    forumAutoPublishAllowed: false,
    gateRef: record.gateRef,
    providerMutationAuthorityAllowed: false,
    publicBlockedClaimPhrases: record.publicBlockedClaimPhrases,
    publicSafeClaimPhrases: record.publicSafeClaimPhrases,
    requiredIssueRefs: uniqueRefs(
      record.checks.flatMap(check => check.issueRefs),
    ),
    rollbackRefs: uniqueRefs(
      record.rollbackSteps.map(step => step.rollbackRef),
    ),
    routeRefs: uniqueRefs(record.checks.flatMap(check => check.routeRefs)),
    runbookCommandRefs: uniqueRefs(runbookCommandRefs),
    settlementAuthorityAllowed: false,
    state,
    stateLabel:
      state === 'ready'
        ? 'Ready for controlled production enablement'
        : 'Blocked before autonomous public claims',
    testRefs: uniqueRefs(record.checks.flatMap(check => check.testRefs)),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    verificationTargetRefs: uniqueRefs(
      record.verificationTargets.map(target => target.targetRef),
    ),
    walletSpendAuthorityAllowed: false,
  })

  if (
    containsProviderSecretMaterial(JSON.stringify(projection)) ||
    stringValues(projection).some(
      value => unsafeRefPattern.test(value) || rawTimestampPattern.test(value),
    )
  ) {
    throw new ArtanisProductionLaunchGateUnsafe({
      reason:
        'Artanis production launch gate projection contains private or raw material.',
    })
  }

  assertCopySafe(projection)

  return projection
}

export const assertArtanisContinuousAutonomyClaimAllowed = (
  projection: ArtanisProductionLaunchGateProjection,
  copy: string,
): void => {
  if (
    !projection.canClaimContinuouslyRunning &&
    unsafePublicClaimPattern.test(copy)
  ) {
    throw new ArtanisProductionLaunchGateUnsafe({
      reason:
        'Public copy cannot claim Artanis is continuously running autonomously until the production launch gate is ready.',
    })
  }
}

const check = (input: {
  category: ArtanisProductionLaunchGateCategory
  checkRef: string
  description: string
  issueRefs: ReadonlyArray<string>
  requiredForAutonomousClaim?: boolean | undefined
  routeRefs?: ReadonlyArray<string> | undefined
  status: ArtanisProductionLaunchGateStatus
  testRefs: ReadonlyArray<string>
}): ArtanisProductionLaunchGateCheck =>
  new ArtanisProductionLaunchGateCheck({
    category: input.category,
    checkRef: input.checkRef,
    description: input.description,
    issueRefs: [...input.issueRefs],
    requiredForAutonomousClaim: input.requiredForAutonomousClaim ?? true,
    routeRefs: [...(input.routeRefs ?? [])],
    status: input.status,
    testRefs: [...input.testRefs],
  })

const command = (input: {
  command: string
  description: string
  kind: ArtanisLaunchRunbookCommandKind
  requiresEnvRefs?: ReadonlyArray<string> | undefined
  runFromRef?: string | undefined
}): ArtanisLaunchRunbookCommand =>
  new ArtanisLaunchRunbookCommand({
    command: input.command,
    description: input.description,
    kind: input.kind,
    requiresEnvRefs: [...(input.requiresEnvRefs ?? [])],
    runFromRef: input.runFromRef ?? 'repo:OpenAgentsInc/autopilot-omega',
  })

const rollback = (input: {
  commandRefs: ReadonlyArray<string>
  description: string
  kind: ArtanisLaunchRollbackKind
  rollbackRef: string
}): ArtanisLaunchRollbackStep =>
  new ArtanisLaunchRollbackStep({
    commandRefs: [...input.commandRefs],
    description: input.description,
    kind: input.kind,
    rollbackRef: input.rollbackRef,
  })

const verificationTarget = (
  targetRef: string,
  expectedSignal: string,
): ArtanisProductionVerificationTarget =>
  new ArtanisProductionVerificationTarget({ expectedSignal, targetRef })

export const exampleArtanisProductionLaunchGateRecord = (
  updatedAtIso = '2026-06-06T21:30:00.000Z',
): ArtanisProductionLaunchGateRecord => {
  const productionSmokeCheck =
    artanisProductionLaunchGateCheckInputFromGepaSmoke(
      exampleArtanisGepaProductionSmokeRecord(updatedAtIso),
      updatedAtIso,
    )
  const scheduledRunnerCheck =
    artanisProductionLaunchGateCheckInputFromGepaScheduledRunnerProof(
      exampleArtanisGepaScheduledRunnerProofRecord(updatedAtIso),
      updatedAtIso,
      productionSmokeCheck.status === 'passed',
    )

  return new ArtanisProductionLaunchGateRecord({
    agentRef: 'agent.public.artanis',
    checks: [
      check({
        category: 'persistence',
        checkRef: 'gate.public.artanis.persistence',
        description:
          'D1 persistence contracts exist for runtime, loops, ticks, approval gates, health snapshots, Forum intents, and work-routing proposals.',
        issueRefs: ['issue:#403'],
        status: 'passed',
        testRefs: ['test:workers/api/src/artanis-persistence.test.ts'],
      }),
      check(scheduledRunnerCheck),
      check({
        category: 'operator_console',
        checkRef: 'gate.public.artanis.operator_console',
        description:
          'Operator console can inspect private state and issue pause, resume, clear, approval, and rejection controls behind admin auth.',
        issueRefs: ['issue:#405'],
        routeRefs: [
          'route:/autopilot',
          'route:/api/operator/artanis/console',
          'route:/api/operator/autopilot/goals/{goalId}/pause',
        ],
        status: 'passed',
        testRefs: [
          'test:workers/api/src/artanis-operator-console-routes.test.ts',
          'test:workers/api/src/agent-goal-routes.test.ts',
        ],
      }),
      check({
        category: 'approval_gates',
        checkRef: 'gate.public.artanis.approval_gates',
        description:
          'Risky actions require explicit operator approval and reject wallet spend, dispatch, deployment, and settlement by default.',
        issueRefs: ['issue:#393', 'issue:#405'],
        routeRefs: [
          'route:/api/operator/artanis/approval-gates/{gateRef}/approve',
          'route:/api/operator/artanis/approval-gates/{gateRef}/reject',
        ],
        status: 'passed',
        testRefs: [
          'test:workers/api/src/artanis-approval-gates.test.ts',
          'test:workers/api/src/artanis-operator-console-routes.test.ts',
        ],
      }),
      check({
        category: 'forum_delivery',
        checkRef: 'gate.public.artanis.forum_delivery',
        description:
          'Forum publication queue records status-post intents and delivered status-topic links without private payloads.',
        issueRefs: ['issue:#406'],
        routeRefs: [
          'route:/api/forum/topics/88888888-4001-4001-8001-888888888888',
        ],
        status: 'passed',
        testRefs: ['test:workers/api/src/artanis-forum-publication.test.ts'],
      }),
      check({
        category: 'forum_listener',
        checkRef: 'gate.public.artanis.forum_listener',
        description:
          'Forum listener can inspect Artanis status and work topics as public-safe context, but does not itself grant write or spend authority.',
        issueRefs: ['issue:#407'],
        routeRefs: ['route:/forum/f/artanis'],
        status: 'passed',
        testRefs: ['test:workers/api/src/artanis-forum-listener.test.ts'],
      }),
      check({
        category: 'nexus_pylon_adapter',
        checkRef: 'gate.public.artanis.nexus_pylon_adapter',
        description:
          'Nexus/Pylon adapter contracts are modeled and public stats are visible; live job dispatch remains separately approval-gated.',
        issueRefs: ['issue:#408'],
        routeRefs: ['route:/api/public/pylon-stats'],
        status: 'passed',
        testRefs: [
          'test:workers/api/src/artanis-nexus-pylon-adapters.test.ts',
          'test:workers/api/src/public-pylon-stats.test.ts',
        ],
      }),
      check({
        category: 'marketplace_intake',
        checkRef: 'gate.public.artanis.marketplace_intake',
        description:
          'Pylon marketplace intake APIs support operator-created jobs and triage while keeping actual dispatch gated.',
        issueRefs: ['issue:#410'],
        routeRefs: [
          'route:/api/operator/artanis/pylon-marketplace/jobs',
          'route:/api/operator/artanis/pylon-marketplace/jobs/{intakeRef}/triage',
        ],
        status: 'passed',
        testRefs: [
          'test:workers/api/src/operator-pylon-marketplace-routes.test.ts',
        ],
      }),
      check({
        category: 'continual_learning_templates',
        checkRef: 'gate.public.artanis.continual_learning_templates',
        description:
          'Initial continual-learning templates cover inference, DSPy/GEPA loops, validation, and fine-tuning/training proposals.',
        issueRefs: ['issue:#411'],
        status: 'passed',
        testRefs: [
          'test:workers/api/src/artanis-continual-learning-templates.test.ts',
        ],
      }),
      check({
        category: 'payment_reward_boundary',
        checkRef: 'gate.public.artanis.payment_reward_boundary',
        description:
          'Forum reward and accepted-work payout projections are evidence-only until wallet authority, spend caps, and settlement receipts are present.',
        issueRefs: ['issue:#412'],
        routeRefs: ['route:/api/public/artanis/report'],
        status: 'passed',
        testRefs: [
          'test:workers/api/src/artanis-forum-reward-visibility.test.ts',
          'test:workers/api/src/artanis-forum-reward-smoke.test.ts',
        ],
      }),
      check({
        category: 'public_report_projection',
        checkRef: 'gate.public.artanis.public_report_projection',
        description:
          'Public report exposes Artanis state, Pylon stats, reward visibility, Pylon launch communication, health, and blocker refs without private material.',
        issueRefs: ['issue:#392', 'issue:#413', 'issue:#414'],
        routeRefs: [
          'route:/artanis',
          'route:/api/public/artanis/report',
          'route:/api/public/pylon-stats',
        ],
        status: 'passed',
        testRefs: [
          'test:workers/api/src/artanis-public-report.test.ts',
          'test:apps/web/src/docs-blog-route.test.ts',
        ],
      }),
      check(productionSmokeCheck),
      check({
        category: 'rollback_runbook',
        checkRef: 'gate.public.artanis.rollback_runbook',
        description:
          'Rollback commands and procedures exist for publication mistakes, dispatch mistakes, payment/reward mistakes, and public overclaim copy.',
        issueRefs: ['issue:#414'],
        status: 'passed',
        testRefs: [
          'test:workers/api/src/artanis-production-launch-gate.test.ts',
        ],
      }),
    ],
    environmentRef: 'env.production.openagents.worker',
    gateRef: 'gate.public.artanis.production_launch.v1',
    publicBlockedClaimPhrases: [
      'blocked_claim.public.artanis.unbounded_autonomy',
      'blocked_claim.public.artanis.ungated_production_admin',
      'blocked_claim.public.pylon_v0_2.shipped',
    ],
    publicSafeClaimPhrases: [
      'Artanis has a public evidence surface and operator-gated launch path.',
      'Artanis has a bounded scheduled runner for public-safe GEPA status projection.',
      'Probe GEPA smoke and scheduled runner evidence are retained; wallet, provider, payout, release, and promotion authority remain gated.',
      'Pylon v0.2 launch communication is prepared, while general release claims remain gated.',
    ],
    rollbackSteps: [
      rollback({
        commandRefs: [
          'runbook.public.artanis.production_launch.disable',
          'runbook.public.artanis.production_launch.revoke',
        ],
        description:
          'If a wrong status post is published, disable scheduled execution, reject the relevant publication gate, post a correction in the Artanis status topic, and keep the bad post linked only as historical evidence.',
        kind: 'publication',
        rollbackRef: 'rollback.public.artanis.publication_mistake',
      }),
      rollback({
        commandRefs: [
          'runbook.public.artanis.production_launch.disable',
          'runbook.public.artanis.production_launch.pause',
        ],
        description:
          'If a dispatch proposal points at the wrong Pylon/Nexus job, disable scheduled execution, pause the owning goal, and leave dispatch authority rejected until a new operator-reviewed proposal is recorded.',
        kind: 'dispatch',
        rollbackRef: 'rollback.public.artanis.dispatch_mistake',
      }),
      rollback({
        commandRefs: [
          'runbook.public.artanis.production_launch.disable',
          'runbook.public.artanis.production_launch.check',
        ],
        description:
          'If reward or payment copy overstates live bitcoin movement, disable the runner, verify public receipts, remove the claim from public copy, and publish a correction that distinguishes simulations from settled payments.',
        kind: 'payment_reward',
        rollbackRef: 'rollback.public.artanis.payment_reward_mistake',
      }),
      rollback({
        commandRefs: [
          'runbook.public.artanis.production_launch.disable',
          'runbook.public.artanis.production_launch.check',
        ],
        description:
          'If public copy says Artanis is continuously autonomous before the gate passes, revert the copy, keep the gate blocked, and cite this launch gate as the authority boundary.',
        kind: 'public_claim',
        rollbackRef: 'rollback.public.artanis.public_claim_mistake',
      }),
    ],
    runbookCommands: [
      command({
        command:
          "curl -fsS https://openagents.com/api/public/artanis/report | jq '{runtimeState, autonomousLoop, healthSummary, pylonSummary, productionLaunchGate}'",
        description:
          'Check the public Artanis report, launch gate, health, loop state, and Pylon summary.',
        kind: 'check',
      }),
      command({
        command:
          'bun run --cwd workers/api build:web && bunx wrangler deploy --config workers/api/wrangler.jsonc --keep-vars --var ARTANIS_SCHEDULED_RUNNER_ENABLED:true',
        description:
          'Enable scheduled Artanis execution for a controlled launch window after every required gate is passed.',
        kind: 'enable',
      }),
      command({
        command:
          'bun run --cwd workers/api build:web && bunx wrangler deploy --config workers/api/wrangler.jsonc --keep-vars --var ARTANIS_SCHEDULED_RUNNER_ENABLED:false',
        description:
          'Disable scheduled Artanis execution by removing or setting the flag false and redeploying the Worker.',
        kind: 'disable',
      }),
      command({
        command:
          'curl -fsS -X POST https://openagents.com/api/operator/autopilot/goals/GOAL_ID/pause -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" -H "Idempotency-Key: artanis-pause-GOAL_ID"',
        description: 'Pause a specific Artanis goal through the operator API.',
        kind: 'pause',
        requiresEnvRefs: ['env:OPENAGENTS_ADMIN_API_TOKEN'],
      }),
      command({
        command:
          'curl -fsS -X POST https://openagents.com/api/operator/artanis/approval-gates/GATE_REF/reject -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" -H "Idempotency-Key: artanis-reject-GATE_REF"',
        description:
          'Revoke authority for a pending or mistaken approval gate.',
        kind: 'revoke',
        requiresEnvRefs: ['env:OPENAGENTS_ADMIN_API_TOKEN'],
      }),
      command({
        command:
          'bunx wrangler d1 execute openagents-autopilot --remote --command "SELECT record_ref, record_type, updated_at FROM artanis_records ORDER BY updated_at DESC LIMIT 20;"',
        description:
          'Inspect recent retained Artanis records when a tick fails, duplicates, or becomes stale.',
        kind: 'recover',
      }),
    ],
    updatedAtIso,
    verificationTargets: [
      verificationTarget(
        'route:/artanis',
        'Page renders public report, launch gate state, Forum refs, Pylon stats, and no autonomy overclaim.',
      ),
      verificationTarget(
        'route:/api/public/artanis/report',
        'JSON contains productionLaunchGate, healthSummary, autonomousLoop, and public-safe blocker refs.',
      ),
      verificationTarget(
        'topic.public.forum.artanis.status',
        'Status topic is readable and latest retained status post is linked.',
      ),
      verificationTarget(
        'route:/autopilot',
        'Operator console can see Artanis goals, approval state, and pause controls.',
      ),
      verificationTarget(
        'route:/api/operator/artanis/console',
        'Admin API returns Artanis runtime, loop, health, approval gates, and staleness summaries.',
      ),
      verificationTarget(
        'route:/api/operator/artanis/approval-gates',
        'Approval and rejection endpoints are reachable with admin auth and idempotency.',
      ),
      verificationTarget(
        'route:/api/public/pylon-stats',
        'Omega public Pylon stats load and distinguish unavailable stats from zero stats.',
      ),
      verificationTarget(
        'signal.public.artanis.health_staleness',
        'Health projection exposes stale or blocked signals before public copy can overclaim.',
      ),
    ],
  })
}

export const exampleArtanisProductionLaunchGateProjection = (
  nowIso: string,
): ArtanisProductionLaunchGateProjection =>
  projectArtanisProductionLaunchGate(
    exampleArtanisProductionLaunchGateRecord(nowIso),
    nowIso,
  )
