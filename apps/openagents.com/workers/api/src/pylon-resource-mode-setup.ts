import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { PublicClaimProjectionAudience } from './public-claim-projections'

export const PylonResourceMode = S.Literals([
  'background_20',
  'balanced',
  'dedicated_full_blast',
  'overnight_full',
])
export type PylonResourceMode = typeof PylonResourceMode.Type

export const PylonResourceModeFamily = S.Literals([
  'background',
  'balanced',
  'dedicated',
  'overnight',
])
export type PylonResourceModeFamily = typeof PylonResourceModeFamily.Type

export const PylonResourceModeSetupState = S.Literals([
  'approved',
  'blocked',
  'completed',
  'draft',
  'ready',
])
export type PylonResourceModeSetupState =
  typeof PylonResourceModeSetupState.Type

export const PylonSetupCommandKind = S.Literals([
  'install_launcher',
  'launch_pylon',
  'set_resource_mode',
  'training_status',
  'version_check',
  'runtime_status',
  'balance_check',
  'history_check',
])
export type PylonSetupCommandKind = typeof PylonSetupCommandKind.Type

export class PylonResourceEnvelope extends S.Class<PylonResourceEnvelope>(
  'PylonResourceEnvelope',
)({
  cpuPercentMax: S.Number,
  diskBudgetRef: S.String,
  gpuPercentMax: S.Number,
  memoryPercentMax: S.Number,
  networkBudgetRef: S.String,
  pauseResumePolicyRef: S.String,
  scheduleWindowRef: S.String,
}) {}

export class PylonResourceModeRecord extends S.Class<PylonResourceModeRecord>(
  'PylonResourceModeRecord',
)({
  caveatRefs: S.Array(S.String),
  eligibilityCaveatRefs: S.Array(S.String),
  envelope: PylonResourceEnvelope,
  family: PylonResourceModeFamily,
  label: S.String,
  mode: PylonResourceMode,
  ownerApprovalRefs: S.Array(S.String),
  publicDescriptionRef: S.String,
  setupCommandRefs: S.Array(S.String),
  state: PylonResourceModeSetupState,
  workRoutingRefs: S.Array(S.String),
}) {}

export class PylonSetupCommandRecord extends S.Class<PylonSetupCommandRecord>(
  'PylonSetupCommandRecord',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  commandRef: S.String,
  evidenceHandlingRef: S.String,
  kind: PylonSetupCommandKind,
  ownerApprovalRefs: S.Array(S.String),
  privateEvidenceRefs: S.Array(S.String),
  publicReceiptRefs: S.Array(S.String),
  safeInstructionRef: S.String,
  state: PylonResourceModeSetupState,
  updatedAtIso: S.String,
}) {}

export class PylonResourceModeSetupPlan extends S.Class<PylonResourceModeSetupPlan>(
  'PylonResourceModeSetupPlan',
)({
  agentRef: S.String,
  commandRecords: S.Array(PylonSetupCommandRecord),
  modes: S.Array(PylonResourceModeRecord),
  planRef: S.String,
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class PylonSetupCommandProjection extends S.Class<PylonSetupCommandProjection>(
  'PylonSetupCommandProjection',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  commandRef: S.String,
  evidenceHandlingRef: S.String,
  kind: PylonSetupCommandKind,
  ownerApprovalRefs: S.Array(S.String),
  privateEvidenceRefs: S.Array(S.String),
  publicReceiptRefs: S.Array(S.String),
  safeInstructionRef: S.String,
  state: PylonResourceModeSetupState,
  updatedAtDisplay: S.String,
}) {}

export class PylonResourceModeSetupProjection extends S.Class<PylonResourceModeSetupProjection>(
  'PylonResourceModeSetupProjection',
)({
  agentRef: S.String,
  audience: PublicClaimProjectionAudience,
  commandRecords: S.Array(PylonSetupCommandProjection),
  modes: S.Array(PylonResourceModeRecord),
  planRef: S.String,
  sourceRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export const PylonLocalAgentCommandPacketState = S.Literals([
  'approved_for_local_execution',
  'blocked',
  'dry_run_ready',
])
export type PylonLocalAgentCommandPacketState =
  typeof PylonLocalAgentCommandPacketState.Type

export class PylonLocalAgentResourceIntent extends S.Class<PylonLocalAgentResourceIntent>(
  'PylonLocalAgentResourceIntent',
)({
  cpuPercentMax: S.Number,
  gpuPercentMax: S.Number,
  memoryPercentMax: S.Number,
  networkIntentRef: S.String,
  storageIntentRef: S.String,
}) {}

export class PylonLocalAgentCommandPacket extends S.Class<PylonLocalAgentCommandPacket>(
  'PylonLocalAgentCommandPacket',
)({
  agentRef: S.String,
  caveatRefs: S.Array(S.String),
  checkpointExpectationRefs: S.Array(S.String),
  dryRunCommandRefs: S.Array(S.String),
  dryRunOutputEvidenceRefs: S.Array(S.String),
  earningCaveatRefs: S.Array(S.String),
  family: PylonResourceModeFamily,
  localExecutionAllowed: S.Boolean,
  mode: PylonResourceMode,
  ownerApprovalPromptRef: S.String,
  ownerApprovalRefs: S.Array(S.String),
  packetRef: S.String,
  pauseResumeExpectationRefs: S.Array(S.String),
  publicReceiptRefs: S.Array(S.String),
  resourceIntent: PylonLocalAgentResourceIntent,
  safeInstructionRefs: S.Array(S.String),
  state: PylonLocalAgentCommandPacketState,
  telemetryRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class PylonLocalAgentCommandPacketProjection extends S.Class<PylonLocalAgentCommandPacketProjection>(
  'PylonLocalAgentCommandPacketProjection',
)({
  agentRef: S.String,
  audience: PublicClaimProjectionAudience,
  caveatRefs: S.Array(S.String),
  checkpointExpectationRefs: S.Array(S.String),
  dryRunCommandRefs: S.Array(S.String),
  dryRunOutputEvidenceRefs: S.Array(S.String),
  earningCaveatRefs: S.Array(S.String),
  family: PylonResourceModeFamily,
  localExecutionAllowed: S.Boolean,
  mode: PylonResourceMode,
  ownerApprovalPromptRef: S.String,
  ownerApprovalRefs: S.Array(S.String),
  packetRef: S.String,
  pauseResumeExpectationRefs: S.Array(S.String),
  publicReceiptRefs: S.Array(S.String),
  resourceIntent: PylonLocalAgentResourceIntent,
  safeInstructionRefs: S.Array(S.String),
  state: PylonLocalAgentCommandPacketState,
  telemetryRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class PylonResourceModeSetupUnsafe extends S.TaggedErrorClass<PylonResourceModeSetupUnsafe>()(
  'PylonResourceModeSetupUnsafe',
  {
    reason: S.String,
  },
) {}

const requiredModes: ReadonlyArray<PylonResourceMode> = [
  'background_20',
  'balanced',
  'dedicated_full_blast',
  'overnight_full',
]
const requiredCommandKinds: ReadonlyArray<PylonSetupCommandKind> = [
  'install_launcher',
  'launch_pylon',
  'set_resource_mode',
  'version_check',
  'runtime_status',
  'training_status',
  'balance_check',
  'history_check',
]
const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|credential|grant|payload|token)|raw[_-]?(command|invoice|output|payment|payload|prompt|runner|run[_-]?log|source[_-]?archive|stdout|stderr|webhook)|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicPrivateEvidencePattern =
  /(evidence\.private|approval\.private|local\.private|operator\.private)/i
const unconditionalEarningPattern =
  /(earn[_-]?money|earn[_-]?payout|earning[_-]?guarantee|guaranteed[_-]?(earning|payout|payment|revenue)|paid[_-]?for[_-]?online|pylon[_-]?always[_-]?earns|run[_-]?pylon[_-]?and[_-]?earn)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonResourceModeSetupUnsafe({
      reason: `${label} contains raw local paths, wallet material, node secrets, provider credentials, raw command output, payment material, customer data, or raw timestamps.`,
    })
  }
}

const assertNoUnconditionalEarning = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    unconditionalEarningPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new PylonResourceModeSetupUnsafe({
      reason: `${label} contains an unconditional earning claim.`,
    })
  }
}

const assertEnvelope = (envelope: PylonResourceEnvelope): void => {
  assertSafeRefs('Pylon resource envelope refs', [
    envelope.diskBudgetRef,
    envelope.networkBudgetRef,
    envelope.pauseResumePolicyRef,
    envelope.scheduleWindowRef,
  ])

  for (const [label, value] of [
    ['cpuPercentMax', envelope.cpuPercentMax],
    ['gpuPercentMax', envelope.gpuPercentMax],
    ['memoryPercentMax', envelope.memoryPercentMax],
  ] as const) {
    if (value < 0 || value > 100) {
      throw new PylonResourceModeSetupUnsafe({
        reason: `Pylon resource ${label} must be between 0 and 100.`,
      })
    }
  }
}

const assertMode = (mode: PylonResourceModeRecord): void => {
  assertSafeRefs('Pylon resource mode refs', [
    mode.publicDescriptionRef,
  ])
  if (
    mode.label.trim() === '' ||
    containsProviderSecretMaterial(mode.label) ||
    unsafeRefPattern.test(mode.label) ||
    rawTimestampPattern.test(mode.label)
  ) {
    throw new PylonResourceModeSetupUnsafe({
      reason: 'Pylon resource mode label contains unsafe material.',
    })
  }
  assertSafeRefs('Pylon resource mode caveat refs', mode.caveatRefs)
  assertSafeRefs(
    'Pylon resource mode eligibility caveat refs',
    mode.eligibilityCaveatRefs,
  )
  assertSafeRefs(
    'Pylon resource mode owner approval refs',
    mode.ownerApprovalRefs,
  )
  assertSafeRefs(
    'Pylon resource mode setup command refs',
    mode.setupCommandRefs,
  )
  assertSafeRefs(
    'Pylon resource mode work routing refs',
    mode.workRoutingRefs,
  )
  assertEnvelope(mode.envelope)

  if (mode.ownerApprovalRefs.length === 0) {
    throw new PylonResourceModeSetupUnsafe({
      reason: 'Pylon resource modes require owner approval refs.',
    })
  }

  if (mode.mode === 'background_20' && mode.family !== 'background') {
    throw new PylonResourceModeSetupUnsafe({
      reason: 'background_20 must use the background family.',
    })
  }

  if (mode.mode === 'overnight_full' && mode.family !== 'overnight') {
    throw new PylonResourceModeSetupUnsafe({
      reason: 'overnight_full must use the overnight family.',
    })
  }

  if (
    mode.mode === 'dedicated_full_blast' &&
    mode.family !== 'dedicated'
  ) {
    throw new PylonResourceModeSetupUnsafe({
      reason: 'dedicated_full_blast must use the dedicated family.',
    })
  }
}

const assertCommand = (command: PylonSetupCommandRecord): void => {
  assertSafeRefs('Pylon setup command refs', [
    command.commandRef,
    command.evidenceHandlingRef,
    command.safeInstructionRef,
  ])
  assertSafeRefs('Pylon setup command blocker refs', command.blockerRefs)
  assertSafeRefs('Pylon setup command caveat refs', command.caveatRefs)
  assertSafeRefs(
    'Pylon setup command owner approval refs',
    command.ownerApprovalRefs,
  )
  assertSafeRefs(
    'Pylon setup command private evidence refs',
    command.privateEvidenceRefs,
  )
  assertSafeRefs(
    'Pylon setup command public receipt refs',
    command.publicReceiptRefs,
  )

  if (command.ownerApprovalRefs.length === 0) {
    throw new PylonResourceModeSetupUnsafe({
      reason: 'Pylon setup commands require explicit owner approval refs.',
    })
  }

  if (
    command.privateEvidenceRefs.length === 0 ||
    !command.privateEvidenceRefs.every(ref => ref.startsWith('evidence.private.'))
  ) {
    throw new PylonResourceModeSetupUnsafe({
      reason:
        'Pylon setup command output must be recorded as private evidence refs.',
    })
  }

  if (command.evidenceHandlingRef !== 'evidence_handling.private_by_default') {
    throw new PylonResourceModeSetupUnsafe({
      reason: 'Pylon setup command output must be private by default.',
    })
  }
}

const assertPacket = (packet: PylonLocalAgentCommandPacket): void => {
  assertSafeRefs('Pylon local-agent command packet refs', [
    packet.agentRef,
    packet.ownerApprovalPromptRef,
    packet.packetRef,
  ])
  assertSafeRefs('Pylon local-agent packet caveat refs', packet.caveatRefs)
  assertSafeRefs(
    'Pylon local-agent checkpoint expectation refs',
    packet.checkpointExpectationRefs,
  )
  assertSafeRefs(
    'Pylon local-agent dry-run command refs',
    packet.dryRunCommandRefs,
  )
  assertSafeRefs(
    'Pylon local-agent dry-run evidence refs',
    packet.dryRunOutputEvidenceRefs,
  )
  assertSafeRefs(
    'Pylon local-agent earning caveat refs',
    packet.earningCaveatRefs,
  )
  assertSafeRefs(
    'Pylon local-agent owner approval refs',
    packet.ownerApprovalRefs,
  )
  assertSafeRefs(
    'Pylon local-agent pause/resume refs',
    packet.pauseResumeExpectationRefs,
  )
  assertSafeRefs(
    'Pylon local-agent public receipt refs',
    packet.publicReceiptRefs,
  )
  assertSafeRefs(
    'Pylon local-agent safe instruction refs',
    packet.safeInstructionRefs,
  )
  assertSafeRefs('Pylon local-agent telemetry refs', packet.telemetryRefs)
  assertSafeRefs('Pylon local-agent resource intent refs', [
    packet.resourceIntent.networkIntentRef,
    packet.resourceIntent.storageIntentRef,
  ])
  assertNoUnconditionalEarning('Pylon local-agent packet refs', [
    packet.ownerApprovalPromptRef,
    packet.packetRef,
    ...packet.caveatRefs,
    ...packet.earningCaveatRefs,
    ...packet.safeInstructionRefs,
    ...packet.publicReceiptRefs,
  ])

  if (packet.agentRef !== 'agent_artanis') {
    throw new PylonResourceModeSetupUnsafe({
      reason: 'Pylon local-agent packets must be administered by agent_artanis.',
    })
  }

  if (
    packet.ownerApprovalRefs.length === 0 ||
    packet.ownerApprovalPromptRef.trim() === ''
  ) {
    throw new PylonResourceModeSetupUnsafe({
      reason:
        'Pylon local-agent packets require explicit owner approval prompts and refs.',
    })
  }

  if (
    packet.dryRunCommandRefs.length === 0 ||
    packet.dryRunOutputEvidenceRefs.length === 0 ||
    !packet.dryRunOutputEvidenceRefs.every(ref => ref.startsWith('evidence.private.'))
  ) {
    throw new PylonResourceModeSetupUnsafe({
      reason:
        'Pylon local-agent packets require dry-run commands and private dry-run evidence refs before execution.',
    })
  }

  if (
    packet.safeInstructionRefs.length === 0 ||
    packet.telemetryRefs.length === 0 ||
    packet.pauseResumeExpectationRefs.length === 0 ||
    packet.checkpointExpectationRefs.length === 0 ||
    packet.earningCaveatRefs.length === 0
  ) {
    throw new PylonResourceModeSetupUnsafe({
      reason:
        'Pylon local-agent packets require safe instructions, telemetry, pause/resume, checkpoint, and earning caveat refs.',
    })
  }

  if (
    packet.localExecutionAllowed &&
    packet.state !== 'approved_for_local_execution'
  ) {
    throw new PylonResourceModeSetupUnsafe({
      reason:
        'Pylon local-agent execution can be allowed only after owner approval.',
    })
  }
}

const assertRequiredCoverage = (
  modes: ReadonlyArray<PylonResourceModeRecord>,
  commands: ReadonlyArray<PylonSetupCommandRecord>,
): void => {
  const presentModes = new Set(modes.map(mode => mode.mode))
  const presentCommands = new Set(commands.map(command => command.kind))
  const missingModes = requiredModes.filter(mode => !presentModes.has(mode))
  const missingCommands = requiredCommandKinds.filter(
    command => !presentCommands.has(command),
  )

  if (missingModes.length > 0 || missingCommands.length > 0) {
    throw new PylonResourceModeSetupUnsafe({
      reason:
        `Pylon setup plan missing modes [${missingModes.join(', ')}] or commands [${missingCommands.join(', ')}].`,
    })
  }
}

export const pylonResourceModeSetupProjectionHasPrivateMaterial = (
  projection: PylonResourceModeSetupProjection,
): boolean => {
  const serialized = JSON.stringify(projection)

  return containsProviderSecretMaterial(serialized) ||
    unsafeRefPattern.test(serialized) ||
    rawTimestampPattern.test(serialized)
}

export const pylonLocalAgentCommandPacketProjectionHasPrivateMaterial = (
  projection: PylonLocalAgentCommandPacketProjection,
): boolean => {
  const serialized = JSON.stringify(projection)

  return containsProviderSecretMaterial(serialized) ||
    unsafeRefPattern.test(serialized) ||
    rawTimestampPattern.test(serialized) ||
    unconditionalEarningPattern.test(serialized)
}

const projectCommand = (
  command: PylonSetupCommandRecord,
  audience: PublicClaimProjectionAudience,
  nowIso: string,
): PylonSetupCommandProjection => ({
  blockerRefs: uniqueRefs(command.blockerRefs),
  caveatRefs: uniqueRefs(command.caveatRefs),
  commandRef: command.commandRef,
  evidenceHandlingRef: command.evidenceHandlingRef,
  kind: command.kind,
  ownerApprovalRefs: uniqueRefs(command.ownerApprovalRefs),
  privateEvidenceRefs:
    audience === 'operator'
      ? uniqueRefs(command.privateEvidenceRefs)
      : [],
  publicReceiptRefs: uniqueRefs(command.publicReceiptRefs),
  safeInstructionRef: command.safeInstructionRef,
  state: command.state,
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    command.updatedAtIso,
    nowIso,
  ),
})

export const projectPylonResourceModeSetupPlan = (
  plan: PylonResourceModeSetupPlan,
  audience: PublicClaimProjectionAudience,
  nowIso: string,
): PylonResourceModeSetupProjection => {
  assertSafeRefs('Pylon setup plan refs', [
    plan.agentRef,
    plan.planRef,
  ])
  assertSafeRefs('Pylon setup plan source refs', plan.sourceRefs)

  if (plan.agentRef !== 'agent_artanis') {
    throw new PylonResourceModeSetupUnsafe({
      reason: 'Pylon resource mode setup plans must be administered by agent_artanis.',
    })
  }

  plan.modes.forEach(assertMode)
  plan.commandRecords.forEach(assertCommand)
  assertRequiredCoverage(plan.modes, plan.commandRecords)

  const projection: PylonResourceModeSetupProjection = {
    agentRef: plan.agentRef,
    audience,
    commandRecords: plan.commandRecords.map(command =>
      projectCommand(command, audience, nowIso),
    ),
    modes: [...plan.modes],
    planRef: plan.planRef,
    sourceRefs: uniqueRefs(plan.sourceRefs),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      plan.updatedAtIso,
      nowIso,
    ),
  }

  if (
    audience !== 'operator' &&
    JSON.stringify(projection).match(publicPrivateEvidencePattern)
  ) {
    throw new PylonResourceModeSetupUnsafe({
      reason: 'Public Pylon setup projections cannot expose private evidence refs.',
    })
  }

  if (pylonResourceModeSetupProjectionHasPrivateMaterial(projection)) {
    throw new PylonResourceModeSetupUnsafe({
      reason: 'Pylon setup projection contains private material.',
    })
  }

  return projection
}

export const projectPylonLocalAgentCommandPacket = (
  packet: PylonLocalAgentCommandPacket,
  audience: PublicClaimProjectionAudience,
  nowIso: string,
): PylonLocalAgentCommandPacketProjection => {
  assertPacket(packet)

  const projection = new PylonLocalAgentCommandPacketProjection({
    agentRef: packet.agentRef,
    audience,
    caveatRefs: uniqueRefs(packet.caveatRefs),
    checkpointExpectationRefs: uniqueRefs(packet.checkpointExpectationRefs),
    dryRunCommandRefs: uniqueRefs(packet.dryRunCommandRefs),
    dryRunOutputEvidenceRefs:
      audience === 'operator'
        ? uniqueRefs(packet.dryRunOutputEvidenceRefs)
        : [],
    earningCaveatRefs: uniqueRefs(packet.earningCaveatRefs),
    family: packet.family,
    localExecutionAllowed: packet.localExecutionAllowed,
    mode: packet.mode,
    ownerApprovalPromptRef: packet.ownerApprovalPromptRef,
    ownerApprovalRefs: uniqueRefs(packet.ownerApprovalRefs),
    packetRef: packet.packetRef,
    pauseResumeExpectationRefs: uniqueRefs(packet.pauseResumeExpectationRefs),
    publicReceiptRefs: uniqueRefs(packet.publicReceiptRefs),
    resourceIntent: packet.resourceIntent,
    safeInstructionRefs: uniqueRefs(packet.safeInstructionRefs),
    state: packet.state,
    telemetryRefs: uniqueRefs(packet.telemetryRefs),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      packet.updatedAtIso,
      nowIso,
    ),
  })

  if (
    audience !== 'operator' &&
    JSON.stringify(projection).match(publicPrivateEvidencePattern)
  ) {
    throw new PylonResourceModeSetupUnsafe({
      reason:
        'Public Pylon local-agent packet projections cannot expose private evidence refs.',
    })
  }

  if (pylonLocalAgentCommandPacketProjectionHasPrivateMaterial(projection)) {
    throw new PylonResourceModeSetupUnsafe({
      reason: 'Pylon local-agent packet projection contains private material.',
    })
  }

  return projection
}

const checkpointExpectationRef = (modeValue: PylonResourceMode): string =>
  ({
    background_20: 'checkpoint.public.pylon.owner_interruptible',
    balanced: 'checkpoint.public.pylon.owner_selected',
    dedicated_full_blast: 'checkpoint.public.pylon.operator_managed',
    overnight_full: 'checkpoint.public.pylon.resume_after_window',
  })[modeValue]

const packetForMode = (
  modeRecord: PylonResourceModeRecord,
  nowIso: string,
): PylonLocalAgentCommandPacket =>
  new PylonLocalAgentCommandPacket({
    agentRef: 'agent_artanis',
    caveatRefs: uniqueRefs([
      ...modeRecord.caveatRefs,
      'caveat.public.local_execution_requires_owner_approval',
      'caveat.public.no_unconditional_earning_claim',
    ]),
    checkpointExpectationRefs: [checkpointExpectationRef(modeRecord.mode)],
    dryRunCommandRefs: [`command.public.pylon.dry_run.${modeRecord.mode}`],
    dryRunOutputEvidenceRefs: [
      `evidence.private.pylon.dry_run.${modeRecord.mode}`,
    ],
    earningCaveatRefs: [
      'caveat.public.online_not_paid_work',
      'caveat.public.accepted_work_receipts_required',
      'caveat.public.settlement_receipts_required',
    ],
    family: modeRecord.family,
    localExecutionAllowed: false,
    mode: modeRecord.mode,
    ownerApprovalPromptRef:
      `approval_prompt.public.pylon.local_agent.${modeRecord.mode}`,
    ownerApprovalRefs: uniqueRefs(modeRecord.ownerApprovalRefs),
    packetRef: `packet.public.pylon.local_agent.${modeRecord.mode}`,
    pauseResumeExpectationRefs: [modeRecord.envelope.pauseResumePolicyRef],
    publicReceiptRefs: [`receipt.public.pylon.local_agent.${modeRecord.mode}`],
    resourceIntent: new PylonLocalAgentResourceIntent({
      cpuPercentMax: modeRecord.envelope.cpuPercentMax,
      gpuPercentMax: modeRecord.envelope.gpuPercentMax,
      memoryPercentMax: modeRecord.envelope.memoryPercentMax,
      networkIntentRef: modeRecord.envelope.networkBudgetRef,
      storageIntentRef: modeRecord.envelope.diskBudgetRef,
    }),
    safeInstructionRefs: [
      modeRecord.publicDescriptionRef,
      'instruction.public.pylon.local_agent_dry_run_first',
      'instruction.public.pylon.keep_outputs_private',
    ],
    state: 'dry_run_ready',
    telemetryRefs: [
      `telemetry.public.pylon.local_status.${modeRecord.mode}`,
      `telemetry.public.pylon.resource_mode.${modeRecord.mode}`,
    ],
    updatedAtIso: nowIso,
  })

export const pylonLocalAgentCommandPacketsFromSetupPlan = (
  plan: PylonResourceModeSetupPlan,
  nowIso: string,
): ReadonlyArray<PylonLocalAgentCommandPacket> => {
  projectPylonResourceModeSetupPlan(plan, 'operator', nowIso)

  return plan.modes.map(modeRecord => packetForMode(modeRecord, nowIso))
}

const mode = (
  input: Readonly<{
    caveatRefs: ReadonlyArray<string>
    cpuPercentMax: number
    diskBudgetRef: string
    eligibilityCaveatRefs: ReadonlyArray<string>
    family: PylonResourceModeFamily
    gpuPercentMax: number
    label: string
    memoryPercentMax: number
    mode: PylonResourceMode
    networkBudgetRef: string
    pauseResumePolicyRef: string
    publicDescriptionRef: string
    scheduleWindowRef: string
    state: PylonResourceModeSetupState
    workRoutingRefs: ReadonlyArray<string>
  }>,
): PylonResourceModeRecord => ({
  caveatRefs: [...input.caveatRefs],
  eligibilityCaveatRefs: [...input.eligibilityCaveatRefs],
  envelope: {
    cpuPercentMax: input.cpuPercentMax,
    diskBudgetRef: input.diskBudgetRef,
    gpuPercentMax: input.gpuPercentMax,
    memoryPercentMax: input.memoryPercentMax,
    networkBudgetRef: input.networkBudgetRef,
    pauseResumePolicyRef: input.pauseResumePolicyRef,
    scheduleWindowRef: input.scheduleWindowRef,
  },
  family: input.family,
  label: input.label,
  mode: input.mode,
  ownerApprovalRefs: ['approval.public.owner.local_compute_pylon'],
  publicDescriptionRef: input.publicDescriptionRef,
  setupCommandRefs: [
    'command.public.pylon.install_launcher',
    'command.public.pylon.set_resource_mode',
    'command.public.pylon.runtime_status',
  ],
  state: input.state,
  workRoutingRefs: [...input.workRoutingRefs],
})

const command = (
  input: Readonly<{
    commandRef: string
    kind: PylonSetupCommandKind
    safeInstructionRef: string
    state?: PylonResourceModeSetupState | undefined
  }>,
): PylonSetupCommandRecord => ({
  blockerRefs: [],
  caveatRefs: ['caveat.public.pylon_output_private_by_default'],
  commandRef: input.commandRef,
  evidenceHandlingRef: 'evidence_handling.private_by_default',
  kind: input.kind,
  ownerApprovalRefs: ['approval.public.owner.local_compute_pylon'],
  privateEvidenceRefs: [`evidence.private.pylon.${input.kind}`],
  publicReceiptRefs: [`receipt.public.pylon.${input.kind}.checked`],
  safeInstructionRef: input.safeInstructionRef,
  state: input.state ?? 'ready',
  updatedAtIso: '2026-06-07T00:05:00.000Z',
})

export const examplePylonResourceModeSetupPlan = ():
  PylonResourceModeSetupPlan => ({
  agentRef: 'agent_artanis',
  commandRecords: [
    command({
      commandRef: 'command.public.pylon.install_launcher',
      kind: 'install_launcher',
      safeInstructionRef: 'instruction.public.pylon.use_package_launcher',
    }),
    command({
      commandRef: 'command.public.pylon.launch',
      kind: 'launch_pylon',
      safeInstructionRef: 'instruction.public.pylon.launch_after_approval',
    }),
    command({
      commandRef: 'command.public.pylon.set_resource_mode',
      kind: 'set_resource_mode',
      safeInstructionRef: 'instruction.public.pylon.apply_owner_mode',
    }),
    command({
      commandRef: 'command.public.pylon.version',
      kind: 'version_check',
      safeInstructionRef: 'instruction.public.pylon.version_check',
    }),
    command({
      commandRef: 'command.public.pylon.runtime_status',
      kind: 'runtime_status',
      safeInstructionRef: 'instruction.public.pylon.runtime_status_check',
    }),
    command({
      commandRef: 'command.public.pylon.training_status',
      kind: 'training_status',
      safeInstructionRef: 'instruction.public.pylon.training_status_check',
    }),
    command({
      commandRef: 'command.public.pylon.balance_check',
      kind: 'balance_check',
      safeInstructionRef: 'instruction.public.pylon.balance_check',
    }),
    command({
      commandRef: 'command.public.pylon.history_check',
      kind: 'history_check',
      safeInstructionRef: 'instruction.public.pylon.history_check',
    }),
  ],
  modes: [
    mode({
      caveatRefs: ['caveat.public.background_mode_may_not_receive_paid_jobs'],
      cpuPercentMax: 20,
      diskBudgetRef: 'disk.public.pylon.low_cache',
      eligibilityCaveatRefs: ['caveat.public.online_is_not_eligible'],
      family: 'background',
      gpuPercentMax: 0,
      label: 'Background 20',
      memoryPercentMax: 20,
      mode: 'background_20',
      networkBudgetRef: 'network.public.pylon.low',
      pauseResumePolicyRef: 'pause.public.owner_interruptible',
      publicDescriptionRef: 'description.public.pylon.background_20',
      scheduleWindowRef: 'schedule.public.while_owner_working',
      state: 'ready',
      workRoutingRefs: ['routing.public.artanis.background_inference_probe'],
    }),
    mode({
      caveatRefs: ['caveat.public.balanced_mode_owner_selected'],
      cpuPercentMax: 50,
      diskBudgetRef: 'disk.public.pylon.medium_cache',
      eligibilityCaveatRefs: ['caveat.public.capability_policy_still_applies'],
      family: 'balanced',
      gpuPercentMax: 40,
      label: 'Balanced',
      memoryPercentMax: 50,
      mode: 'balanced',
      networkBudgetRef: 'network.public.pylon.medium',
      pauseResumePolicyRef: 'pause.public.owner_can_pause',
      publicDescriptionRef: 'description.public.pylon.balanced',
      scheduleWindowRef: 'schedule.public.owner_selected',
      state: 'ready',
      workRoutingRefs: ['routing.public.artanis.balanced_validation'],
    }),
    mode({
      caveatRefs: ['caveat.public.overnight_mode_requires_owner_window'],
      cpuPercentMax: 90,
      diskBudgetRef: 'disk.public.pylon.large_cache',
      eligibilityCaveatRefs: ['caveat.public.accepted_work_not_implied'],
      family: 'overnight',
      gpuPercentMax: 90,
      label: 'Overnight full',
      memoryPercentMax: 85,
      mode: 'overnight_full',
      networkBudgetRef: 'network.public.pylon.high',
      pauseResumePolicyRef: 'pause.public.resume_after_window',
      publicDescriptionRef: 'description.public.pylon.overnight_full',
      scheduleWindowRef: 'schedule.public.overnight_owner_window',
      state: 'ready',
      workRoutingRefs: ['routing.public.artanis.overnight_training'],
    }),
    mode({
      caveatRefs: ['caveat.public.dedicated_mode_for_operator_machine'],
      cpuPercentMax: 100,
      diskBudgetRef: 'disk.public.pylon.dedicated_cache',
      eligibilityCaveatRefs: ['caveat.public.settlement_not_implied'],
      family: 'dedicated',
      gpuPercentMax: 100,
      label: 'Dedicated full blast',
      memoryPercentMax: 95,
      mode: 'dedicated_full_blast',
      networkBudgetRef: 'network.public.pylon.dedicated',
      pauseResumePolicyRef: 'pause.public.operator_managed',
      publicDescriptionRef: 'description.public.pylon.dedicated_full_blast',
      scheduleWindowRef: 'schedule.public.always_on_operator_machine',
      state: 'ready',
      workRoutingRefs: ['routing.public.artanis.dedicated_pylon_jobs'],
    }),
  ],
  planRef: 'plan.public.artanis.pylon_resource_modes',
  sourceRefs: [
    'docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md',
    'docs/artanis/2026-06-06-work-routing-contract.md',
  ],
  updatedAtIso: '2026-06-07T00:05:00.000Z',
})
