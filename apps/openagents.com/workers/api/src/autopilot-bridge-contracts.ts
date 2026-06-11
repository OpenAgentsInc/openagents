import { Schema as S } from 'effect'

import { evaluateArtanisLaborBudgetGate } from './labor-escrow'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export const AutopilotBridgeMvpCapability = S.Literals([
  'account_pool_visibility',
  'artifact_receipts',
  'budget_usage_visibility',
  'decisions_review',
  'placement_pricing_visibility',
  'scheduling_continuation',
  'status_events',
  'submit_work',
])
export type AutopilotBridgeMvpCapability =
  typeof AutopilotBridgeMvpCapability.Type

export const AutopilotBridgeWaiverKind = S.Literals([
  'deferred_to_gate_live_proof',
  'not_user_facing_in_mvp',
])
export type AutopilotBridgeWaiverKind = typeof AutopilotBridgeWaiverKind.Type

export class AutopilotBridgeWaiver extends S.Class<AutopilotBridgeWaiver>(
  'AutopilotBridgeWaiver',
)({
  expiresWithIssueRef: S.String,
  kind: AutopilotBridgeWaiverKind,
  reasonRef: S.String,
}) {}

export class AutopilotBridgeParityRow extends S.Class<AutopilotBridgeParityRow>(
  'AutopilotBridgeParityRow',
)({
  apiPeerRefs: S.Array(S.String),
  capability: AutopilotBridgeMvpCapability,
  proofRefs: S.Array(S.String),
  testRefs: S.Array(S.String),
  waiver: S.NullOr(AutopilotBridgeWaiver),
  webSurfaceRef: S.String,
}) {}

export class AutopilotBridgeParityProjection extends S.Class<AutopilotBridgeParityProjection>(
  'AutopilotBridgeParityProjection',
)({
  blockedCapabilityRefs: S.Array(S.String),
  generatedAt: S.String,
  matrixRef: S.String,
  ready: S.Boolean,
  rows: S.Array(AutopilotBridgeParityRow),
  staleness: PublicProjectionStalenessContract,
  waivedCapabilityRefs: S.Array(S.String),
}) {}

export const AutopilotBridgePaymentMode = S.Literals([
  'free_slice',
  'l402',
  'mdk_checkout',
  'operator_credit',
])
export type AutopilotBridgePaymentMode = typeof AutopilotBridgePaymentMode.Type

export const AutopilotBridgeLifecycleKind = S.Literals([
  'delivered',
  'placed',
  'queued',
  'reviewed',
])
export type AutopilotBridgeLifecycleKind =
  typeof AutopilotBridgeLifecycleKind.Type

export class AutopilotBridgeLifecycleReceipt extends S.Class<AutopilotBridgeLifecycleReceipt>(
  'AutopilotBridgeLifecycleReceipt',
)({
  idempotencyKey: S.String,
  kind: AutopilotBridgeLifecycleKind,
  receiptRef: S.String,
  topicPostRef: S.String,
}) {}

export class AutopilotBridgeForumCodingOrderInput extends S.Class<AutopilotBridgeForumCodingOrderInput>(
  'AutopilotBridgeForumCodingOrderInput',
)({
  budgetRef: S.String,
  forumActionRef: S.String,
  generatedAt: S.String,
  lifecycleReceipts: S.Array(AutopilotBridgeLifecycleReceipt),
  missionRef: S.String,
  paymentMode: AutopilotBridgePaymentMode,
  requestingAgentRef: S.String,
  threadRef: S.String,
  workOrderRef: S.String,
}) {}

export class AutopilotBridgeForumCodingOrderLink extends S.Class<AutopilotBridgeForumCodingOrderLink>(
  'AutopilotBridgeForumCodingOrderLink',
)({
  budgetRef: S.String,
  caveatRefs: S.Array(S.String),
  forumActionRef: S.String,
  generatedAt: S.String,
  idempotencyKey: S.String,
  lifecycleReceiptRefs: S.Array(S.String),
  missionRef: S.String,
  paymentMode: AutopilotBridgePaymentMode,
  requestingAgentRef: S.String,
  staleness: PublicProjectionStalenessContract,
  threadRef: S.String,
  topicPostRefs: S.Array(S.String),
  workOrderRef: S.String,
}) {}

export class AutopilotBridgeAutonomicCodingProposal extends S.Class<AutopilotBridgeAutonomicCodingProposal>(
  'AutopilotBridgeAutonomicCodingProposal',
)({
  action: S.Literal('request_coding_work'),
  acceptanceCriteriaRefs: S.Array(S.String),
  budgetSats: S.Number,
  objectiveRef: S.String,
  repositoryRefs: S.Array(S.String),
  requestingAutonomicRef: S.String,
  reviewPolicyRef: S.String,
  verificationCommandRef: S.String,
}) {}

export class AutopilotBridgeAutonomicCodingEvaluationInput extends S.Class<AutopilotBridgeAutonomicCodingEvaluationInput>(
  'AutopilotBridgeAutonomicCodingEvaluationInput',
)({
  alreadyReservedThisTickMsat: S.Number,
  generatedAt: S.String,
  operatorEnabled: S.Boolean,
  paymentAuthorityRef: S.NullOr(S.String),
  perTickBudgetMsat: S.Number,
  proposal: AutopilotBridgeAutonomicCodingProposal,
  seededBalanceAvailableMsat: S.Number,
  tickRef: S.String,
}) {}

export const AutopilotBridgeAutonomicDecisionKind = S.Literals([
  'proposed',
  'refused',
  'skipped',
])
export type AutopilotBridgeAutonomicDecisionKind =
  typeof AutopilotBridgeAutonomicDecisionKind.Type

export class AutopilotBridgeAutonomicCodingEvaluation extends S.Class<AutopilotBridgeAutonomicCodingEvaluation>(
  'AutopilotBridgeAutonomicCodingEvaluation',
)({
  blockerRefs: S.Array(S.String),
  budgetMsat: S.NullOr(S.Number),
  decision: AutopilotBridgeAutonomicDecisionKind,
  generatedAt: S.String,
  paymentAuthorityRef: S.NullOr(S.String),
  proposedWorkOrderDraftRef: S.NullOr(S.String),
  reserveIntentRef: S.NullOr(S.String),
  reviewGateRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  tickRef: S.String,
}) {}

export class AutopilotBridgeContractUnsafe extends S.TaggedErrorClass<AutopilotBridgeContractUnsafe>()(
  'AutopilotBridgeContractUnsafe',
  {
    reason: S.String,
  },
) {}

const requiredMvpCapabilities: ReadonlyArray<AutopilotBridgeMvpCapability> = [
  'account_pool_visibility',
  'artifact_receipts',
  'budget_usage_visibility',
  'decisions_review',
  'placement_pricing_visibility',
  'scheduling_continuation',
  'status_events',
  'submit_work',
]

export const AutopilotBridgeMvpParityMatrix: ReadonlyArray<AutopilotBridgeParityRow> =
  [
    new AutopilotBridgeParityRow({
      apiPeerRefs: ['api.post./api/autopilot/work'],
      capability: 'submit_work',
      proofRefs: ['issue.public.openagents.4773'],
      testRefs: ['workers/api/src/autopilot-work-routes.test.ts'],
      waiver: null,
      webSurfaceRef: 'web.autopilot.work_composer',
    }),
    new AutopilotBridgeParityRow({
      apiPeerRefs: [
        'api.get./api/autopilot/work/:id',
        'api.get./api/autopilot/work/:id/events',
        'api.sse./api/autopilot/work/:id/events',
      ],
      capability: 'status_events',
      proofRefs: [
        'issue.public.openagents.4773',
        'issue.public.openagents.4808',
      ],
      testRefs: ['workers/api/src/autopilot-work-routes.test.ts'],
      waiver: null,
      webSurfaceRef: 'web.autopilot.workroom_status',
    }),
    new AutopilotBridgeParityRow({
      apiPeerRefs: [
        'api.get./api/autopilot/decisions',
        'api.post./api/autopilot/decisions/:id/actions',
      ],
      capability: 'decisions_review',
      proofRefs: ['issue.public.openagents.4765'],
      testRefs: ['workers/api/src/autopilot-decision-routes.test.ts'],
      waiver: null,
      webSurfaceRef: 'web.autopilot.decision_review',
    }),
    new AutopilotBridgeParityRow({
      apiPeerRefs: [
        'api.get./api/autopilot/continuation-policy',
        'api.post./api/autopilot/work',
      ],
      capability: 'scheduling_continuation',
      proofRefs: [
        'issue.public.openagents.4764',
        'issue.public.openagents.4815',
      ],
      testRefs: [
        'workers/api/src/autopilot-continuation-policy.test.ts',
        'workers/api/src/autopilot-work-scheduled-launch.test.ts',
      ],
      waiver: null,
      webSurfaceRef: 'web.autopilot.schedule_controls',
    }),
    new AutopilotBridgeParityRow({
      apiPeerRefs: [
        'api.get./api/autopilot/work/placement',
        'api.get./api/autopilot/work/quote',
      ],
      capability: 'placement_pricing_visibility',
      proofRefs: [
        'issue.public.openagents.4761',
        'issue.public.openagents.4769',
      ],
      testRefs: [
        'workers/api/src/autopilot-work-quote.test.ts',
        'workers/api/src/coding-autopilot-repo-placement.test.ts',
      ],
      waiver: null,
      webSurfaceRef: 'web.autopilot.placement_pricing',
    }),
    new AutopilotBridgeParityRow({
      apiPeerRefs: ['api.get./api/settings/provider-accounts'],
      capability: 'account_pool_visibility',
      proofRefs: ['issue.public.openagents.4766'],
      testRefs: ['workers/api/src/provider-account-pool.test.ts'],
      waiver: null,
      webSurfaceRef: 'web.settings.connections',
    }),
    new AutopilotBridgeParityRow({
      apiPeerRefs: ['api.get./api/autopilot/ledger/usage'],
      capability: 'budget_usage_visibility',
      proofRefs: ['issue.public.openagents.4821'],
      testRefs: ['workers/api/src/autopilot-pack-a-ledger.test.ts'],
      waiver: null,
      webSurfaceRef: 'web.autopilot.usage_budget',
    }),
    new AutopilotBridgeParityRow({
      apiPeerRefs: ['api.get./api/autopilot/ledger/artifacts'],
      capability: 'artifact_receipts',
      proofRefs: ['issue.public.openagents.4819'],
      testRefs: ['workers/api/src/autopilot-pack-a-ledger.test.ts'],
      waiver: null,
      webSurfaceRef: 'web.autopilot.artifacts_receipts',
    }),
  ]

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeBridgePattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer\s+|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|file:\/\/|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/\s]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(key|repo)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(command|content|email|invoice|payment|payload|prompt|provider|repo|runner|run[_-]?log|source[_-]?archive|state|tool[_-]?log|webhook)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?archive|ssh:\/\/|token|wallet[._-]?(home|key|material|mnemonic|path|preimage|private|secret|seed)|webhook[_-]?secret|xprv)/i

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertIso = (label: string, value: string): void => {
  if (!Number.isFinite(Date.parse(value))) {
    throw new AutopilotBridgeContractUnsafe({
      reason: `${label} must be an ISO timestamp.`,
    })
  }
}

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafeBridgePattern.test(JSON.stringify(value) ?? '')) {
    throw new AutopilotBridgeContractUnsafe({
      reason: `${label} contains private, secret, provider, payment, wallet, raw prompt, raw source, or runner material.`,
    })
  }
}

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref => !safeRefPattern.test(ref) || unsafeBridgePattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new AutopilotBridgeContractUnsafe({
      reason: `${label} must contain stable public refs only.`,
    })
  }
}

const assertNonEmptySafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  if (refs.length === 0) {
    throw new AutopilotBridgeContractUnsafe({
      reason: `${label} must contain at least one ref.`,
    })
  }

  assertSafeRefs(label, refs)
}

const validateParityRow = (
  row: AutopilotBridgeParityRow,
): AutopilotBridgeParityRow => {
  assertSafeRefs('parity identity refs', [
    row.webSurfaceRef,
    ...row.apiPeerRefs,
    ...row.proofRefs,
    ...row.testRefs,
  ])

  if (row.waiver !== null) {
    assertSafeRefs('parity waiver refs', [
      row.waiver.reasonRef,
      row.waiver.expiresWithIssueRef,
    ])
  }

  if (row.apiPeerRefs.length === 0 && row.waiver === null) {
    throw new AutopilotBridgeContractUnsafe({
      reason:
        'Every MVP capability row must have an agent/API peer or an explicit waiver.',
    })
  }

  return new AutopilotBridgeParityRow({
    ...row,
    apiPeerRefs: uniqueRefs(row.apiPeerRefs),
    proofRefs: uniqueRefs(row.proofRefs),
    testRefs: uniqueRefs(row.testRefs),
  })
}

export const validateAutopilotBridgeParityMatrix = (
  rows: ReadonlyArray<AutopilotBridgeParityRow>,
): ReadonlyArray<AutopilotBridgeParityRow> => {
  const normalizedRows = rows.map(validateParityRow)
  const present = new Set(normalizedRows.map(row => row.capability))
  const missing = requiredMvpCapabilities.filter(
    capability => !present.has(capability),
  )

  if (missing.length > 0) {
    throw new AutopilotBridgeContractUnsafe({
      reason: `MVP parity matrix is missing capability rows: ${missing.join(', ')}.`,
    })
  }

  if (present.size !== normalizedRows.length) {
    throw new AutopilotBridgeContractUnsafe({
      reason: 'MVP parity matrix must contain exactly one row per capability.',
    })
  }

  return normalizedRows
}

export const projectAutopilotBridgeParityMatrix = (
  rows: ReadonlyArray<AutopilotBridgeParityRow>,
  generatedAt: string,
): AutopilotBridgeParityProjection => {
  assertIso('generatedAt', generatedAt)
  const normalizedRows = validateAutopilotBridgeParityMatrix(rows)
  const blockedCapabilityRefs = normalizedRows
    .filter(row => row.apiPeerRefs.length === 0 && row.waiver === null)
    .map(row => `capability.autopilot.mvp.${row.capability}`)
  const waivedCapabilityRefs = normalizedRows
    .filter(row => row.waiver !== null)
    .map(row => `capability.autopilot.mvp.${row.capability}`)

  return new AutopilotBridgeParityProjection({
    blockedCapabilityRefs,
    generatedAt,
    matrixRef: 'matrix.autopilot.mvp_api_parity.v1',
    ready: blockedCapabilityRefs.length === 0,
    rows: normalizedRows,
    staleness: liveAtReadStaleness([
      'autopilot.mvp_surface_added',
      'autopilot.agent_api_peer_added',
      'autopilot.parity_waiver_changed',
    ]),
    waivedCapabilityRefs,
  })
}

export const assertNewMvpSurfaceHasApiPeer = (
  row: AutopilotBridgeParityRow,
): AutopilotBridgeParityRow => validateParityRow(row)

export const planForumCodingOrderBridge = (
  input: AutopilotBridgeForumCodingOrderInput,
): AutopilotBridgeForumCodingOrderLink => {
  assertIso('generatedAt', input.generatedAt)
  assertPublicSafeValue('Forum coding order bridge input', input)
  assertSafeRefs('Forum coding order bridge refs', [
    input.budgetRef,
    input.forumActionRef,
    input.missionRef,
    input.requestingAgentRef,
    input.threadRef,
    input.workOrderRef,
  ])

  if (!input.requestingAgentRef.startsWith('agent:')) {
    throw new AutopilotBridgeContractUnsafe({
      reason:
        'Forum coding orders must be requested by a registered agent ref.',
    })
  }

  const lifecycleKinds = new Set(
    input.lifecycleReceipts.map(receipt => receipt.kind),
  )
  const missingLifecycle = ['queued', 'placed', 'delivered', 'reviewed'].filter(
    kind => !lifecycleKinds.has(kind as AutopilotBridgeLifecycleKind),
  )

  if (missingLifecycle.length > 0) {
    throw new AutopilotBridgeContractUnsafe({
      reason:
        'Forum coding order links require queued, placed, delivered, and reviewed lifecycle receipts.',
    })
  }

  const lifecycleReceiptRefs = uniqueRefs(
    input.lifecycleReceipts.map(receipt => receipt.receiptRef),
  )
  const topicPostRefs = uniqueRefs(
    input.lifecycleReceipts.map(receipt => receipt.topicPostRef),
  )
  const idempotencyKeys = uniqueRefs(
    input.lifecycleReceipts.map(receipt => receipt.idempotencyKey),
  )

  assertNonEmptySafeRefs('Forum lifecycle receipt refs', lifecycleReceiptRefs)
  assertNonEmptySafeRefs('Forum lifecycle topic post refs', topicPostRefs)
  assertNonEmptySafeRefs('Forum lifecycle idempotency keys', idempotencyKeys)

  return new AutopilotBridgeForumCodingOrderLink({
    budgetRef: input.budgetRef,
    caveatRefs:
      input.paymentMode === 'free_slice'
        ? ['caveat.forum_coding_order.no_spend_policy']
        : [],
    forumActionRef: input.forumActionRef,
    generatedAt: input.generatedAt,
    idempotencyKey: `idempotency.forum_coding_order.${input.workOrderRef}`,
    lifecycleReceiptRefs,
    missionRef: input.missionRef,
    paymentMode: input.paymentMode,
    requestingAgentRef: input.requestingAgentRef,
    staleness: liveAtReadStaleness([
      'forum.coding_order.created',
      'autopilot.work_order.lifecycle_receipt_posted',
    ]),
    threadRef: input.threadRef,
    topicPostRefs,
    workOrderRef: input.workOrderRef,
  })
}

export const evaluateAutonomicCodingWorkProposal = (
  input: AutopilotBridgeAutonomicCodingEvaluationInput,
): AutopilotBridgeAutonomicCodingEvaluation => {
  assertIso('generatedAt', input.generatedAt)
  assertPublicSafeValue('autonomic coding work proposal input', input)
  assertSafeRefs('autonomic coding refs', [
    input.tickRef,
    input.proposal.objectiveRef,
    input.proposal.requestingAutonomicRef,
    input.proposal.reviewPolicyRef,
    input.proposal.verificationCommandRef,
    ...input.proposal.acceptanceCriteriaRefs,
    ...input.proposal.repositoryRefs,
    ...(input.paymentAuthorityRef === null ? [] : [input.paymentAuthorityRef]),
  ])

  const base = {
    generatedAt: input.generatedAt,
    paymentAuthorityRef: input.paymentAuthorityRef,
    staleness: liveAtReadStaleness([
      'autonomic.tick.evaluated',
      'autonomic.coding_work.proposal_changed',
      'autonomic.coding_work.operator_gate_changed',
    ]),
    tickRef: input.tickRef,
  }

  if (!input.operatorEnabled) {
    return new AutopilotBridgeAutonomicCodingEvaluation({
      ...base,
      blockerRefs: ['blocker.autonomic_coding_work.operator_disabled'],
      budgetMsat: null,
      decision: 'skipped',
      proposedWorkOrderDraftRef: null,
      reserveIntentRef: null,
      reviewGateRefs: [],
    })
  }

  if (
    input.proposal.action !== 'request_coding_work' ||
    input.proposal.acceptanceCriteriaRefs.length === 0 ||
    input.proposal.repositoryRefs.length === 0
  ) {
    return new AutopilotBridgeAutonomicCodingEvaluation({
      ...base,
      blockerRefs: ['blocker.autonomic_coding_work.schema_invalid'],
      budgetMsat: null,
      decision: 'refused',
      proposedWorkOrderDraftRef: null,
      reserveIntentRef: null,
      reviewGateRefs: [],
    })
  }

  if (input.paymentAuthorityRef === null) {
    return new AutopilotBridgeAutonomicCodingEvaluation({
      ...base,
      blockerRefs: ['blocker.autonomic_coding_work.payment_authority_missing'],
      budgetMsat: null,
      decision: 'refused',
      proposedWorkOrderDraftRef: null,
      reserveIntentRef: null,
      reviewGateRefs: [],
    })
  }

  const budgetMsat = input.proposal.budgetSats * 1000
  const budgetGate = evaluateArtanisLaborBudgetGate({
    alreadyReservedThisTickMsat: input.alreadyReservedThisTickMsat,
    perTickBudgetMsat: input.perTickBudgetMsat,
    requestedAmountMsat: budgetMsat,
    seededBalanceAvailableMsat: input.seededBalanceAvailableMsat,
  })

  if (budgetGate.kind === 'refused') {
    return new AutopilotBridgeAutonomicCodingEvaluation({
      ...base,
      blockerRefs: [budgetGate.refusalRef],
      budgetMsat,
      decision: 'refused',
      proposedWorkOrderDraftRef: null,
      reserveIntentRef: null,
      reviewGateRefs: [],
    })
  }

  return new AutopilotBridgeAutonomicCodingEvaluation({
    ...base,
    blockerRefs: [],
    budgetMsat,
    decision: 'proposed',
    proposedWorkOrderDraftRef: `work_order_draft.autonomic.${input.tickRef}`,
    reserveIntentRef: `reserve_intent.autonomic_coding.${input.tickRef}`,
    reviewGateRefs: [
      input.proposal.reviewPolicyRef,
      `verification_command.${input.proposal.verificationCommandRef}`,
      'gate.human_review.repo_authority',
    ],
  })
}
