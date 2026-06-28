import { Schema as S } from 'effect'

import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const TraceEconomicUnderwritingEndpoint =
  '/api/public/underwriting/trace-economic'
export const TraceEconomicUnderwritingSchemaVersion =
  'openagents.underwriting.trace_economic.v1'
export const TraceEconomicUnderwritingMinimumReadyOutcomeCount = 8
export const TraceEconomicUnderwritingMinimumReadyCoverageBps = 9500
export const TraceEconomicUnderwritingStaleness = liveAtReadStaleness([
  'accepted_outcome_receipt_published',
  'trace_replay_verdict_published',
  'pylon_settlement_receipt_published',
  'khala_metering_receipt_published',
  'product_promise_registry_updated',
])

export const TraceEconomicUnderwritingWarrantyKind = S.Literals([
  'refund_on_rejection',
  'verified_outcome_sla',
])
export type TraceEconomicUnderwritingWarrantyKind = S.Schema.Type<
  typeof TraceEconomicUnderwritingWarrantyKind
>

export class TraceEconomicUnderwritingOutcome extends S.Class<TraceEconomicUnderwritingOutcome>(
  'TraceEconomicUnderwritingOutcome',
)({
  outcomeRef: S.String,
  workClass: S.String,
  acceptedOutcomeRef: S.String,
  traceRef: S.String,
  verdictRef: S.String,
  settlementReceiptRef: S.String,
  meteringReceiptRef: S.String,
  replayVerified: S.Boolean,
  accepted: S.Boolean,
  settled: S.Boolean,
  meterable: S.Boolean,
  claimableLossBps: S.Int,
  observedAt: S.String,
  sourceRefs: S.Array(S.String),
}) {}

export class TraceEconomicUnderwritingWarrantyShape extends S.Class<TraceEconomicUnderwritingWarrantyShape>(
  'TraceEconomicUnderwritingWarrantyShape',
)({
  kind: TraceEconomicUnderwritingWarrantyKind,
  state: S.Literal('modeled_not_bound'),
  trigger: S.String,
  requiredEvidenceRefs: S.Array(S.String),
  settlementMode: S.Literal('existing_metering_and_settlement_spine'),
  premiumQuoted: S.Literal(false),
  policyBound: S.Literal(false),
  claimPayoutEnabled: S.Literal(false),
  inert: S.Literal(true),
}) {}

export class TraceEconomicUnderwritingGate extends S.Class<TraceEconomicUnderwritingGate>(
  'TraceEconomicUnderwritingGate',
)({
  state: S.Literals(['yellow', 'green']),
  currentQualifyingOutcomeCount: S.Int,
  requiredQualifyingOutcomeCount: S.Int,
  currentReceiptCoverageBps: S.Int,
  requiredReceiptCoverageBps: S.Int,
  warrantyOfferAllowed: S.Boolean,
  refundPromiseAllowed: S.Boolean,
  slaPromiseAllowed: S.Boolean,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
}) {}

export class TraceEconomicUnderwritingProjection extends S.Class<TraceEconomicUnderwritingProjection>(
  'TraceEconomicUnderwritingProjection',
)({
  schemaVersion: S.String,
  generatedAt: S.String,
  projectionId: S.Literal('underwriting.trace_economic.v1'),
  definitionRef: S.String,
  issueRef: S.Literal('github:OpenAgentsInc/openagents#6426'),
  promiseRef: S.Literal('promise:markets.risk_bonds_underwriting.v1'),
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('instrumented_seed'),
  statusLabel: S.String,
  substrate: S.Struct({
    traceSurface: S.Literal('/trace/{uuid}'),
    verdictSource: S.Literal('replay_verdicts'),
    settlementSource: S.Literal('public_safe_settlement_receipts'),
    meteringSource: S.Literal('khala_metering_receipts'),
    pricingBasis: S.Literal('claimable_loss_bps_over_qualifying_outcomes'),
  }),
  observed: S.Struct({
    inputOutcomeCount: S.Int,
    qualifyingOutcomeCount: S.Int,
    incompleteOutcomeCount: S.Int,
    receiptCoverageBps: S.Int,
    averageClaimableLossBps: S.Int,
  }),
  gate: TraceEconomicUnderwritingGate,
  warrantyShapes: S.Array(TraceEconomicUnderwritingWarrantyShape),
  qualifyingOutcomeRefs: S.Array(S.String),
  incompleteOutcomeRefs: S.Array(S.String),
  authorityBoundary: S.String,
  unsafeCopy: S.String,
  sourceRefs: S.Array(S.String),
}) {}

export type ProjectTraceEconomicUnderwritingInput = Readonly<{
  generatedAt?: string | undefined
  outcomes?: ReadonlyArray<TraceEconomicUnderwritingOutcome> | undefined
}>

const isQualifyingOutcome = (
  outcome: TraceEconomicUnderwritingOutcome,
): boolean =>
  outcome.accepted &&
  outcome.replayVerified &&
  outcome.settled &&
  outcome.meterable &&
  outcome.traceRef.length > 0 &&
  outcome.verdictRef.length > 0 &&
  outcome.settlementReceiptRef.length > 0 &&
  outcome.meteringReceiptRef.length > 0

const clampBps = (value: number): number =>
  Math.min(10000, Math.max(0, Math.round(value)))

const averageClaimableLossBps = (
  outcomes: ReadonlyArray<TraceEconomicUnderwritingOutcome>,
): number =>
  outcomes.length === 0
    ? 0
    : clampBps(
        outcomes.reduce((sum, outcome) => sum + outcome.claimableLossBps, 0) /
          outcomes.length,
      )

export const seedTraceEconomicUnderwritingOutcomes =
  (): ReadonlyArray<TraceEconomicUnderwritingOutcome> => [
    new TraceEconomicUnderwritingOutcome({
      accepted: true,
      acceptedOutcomeRef:
        'closeout.public.pylon.labor_market.fe1ee748e332a9b9ff7f1e0b',
      claimableLossBps: 10000,
      meterable: true,
      meteringReceiptRef:
        'receipt.metering.khala.accepted_outcome.labor_4777.public',
      observedAt: '2026-06-14T03:06:15.399Z',
      outcomeRef:
        'work_result.public.788b59de-8ee9-4029-9f5b-c6cf23dc668d',
      replayVerified: true,
      settlementReceiptRef:
        'receipt.labor_escrow.release.b74bb55c-849c-43a3-b8d9-9a741316b528.quote.public.pylon.labor_market.b97f312478504e9df212e333',
      settled: true,
      sourceRefs: [
        'docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md',
        'docs/research/idai/roadmap-alignment.md#2-trace-economic-underwriting-safety-economics',
      ],
      traceRef: 'trace.public.labor_market.b74bb55c',
      verdictRef: 'verdict.public.pylon.labor_market.b74bb55c.bun_test.pass',
      workClass: 'labor_market_fixture',
    }),
    new TraceEconomicUnderwritingOutcome({
      accepted: false,
      acceptedOutcomeRef: '',
      claimableLossBps: 0,
      meterable: true,
      meteringReceiptRef: 'receipt.metering.khala.rejected.synthetic',
      observedAt: '2026-06-27T12:00:00.000Z',
      outcomeRef: 'work_result.public.synthetic.rejected',
      replayVerified: true,
      settlementReceiptRef: '',
      settled: false,
      sourceRefs: [
        'docs/research/idai/roadmap-alignment.md#2-trace-economic-underwriting-safety-economics',
      ],
      traceRef: 'trace.public.synthetic.rejected',
      verdictRef: 'verdict.public.synthetic.rejected',
      workClass: 'synthetic_counterexample',
    }),
  ]

const warrantyShapes = (): ReadonlyArray<TraceEconomicUnderwritingWarrantyShape> => [
  new TraceEconomicUnderwritingWarrantyShape({
    claimPayoutEnabled: false,
    inert: true,
    kind: 'refund_on_rejection',
    policyBound: false,
    premiumQuoted: false,
    requiredEvidenceRefs: [
      'acceptedOutcomeRef',
      'traceRef',
      'verdictRef',
      'settlementReceiptRef',
      'meteringReceiptRef',
    ],
    settlementMode: 'existing_metering_and_settlement_spine',
    state: 'modeled_not_bound',
    trigger:
      'A future bound warranty would refund an eligible rejected outcome only when the rejection verdict and metering receipt both identify the same work unit.',
  }),
  new TraceEconomicUnderwritingWarrantyShape({
    claimPayoutEnabled: false,
    inert: true,
    kind: 'verified_outcome_sla',
    policyBound: false,
    premiumQuoted: false,
    requiredEvidenceRefs: [
      'acceptedOutcomeRef',
      'traceRef',
      'verdictRef',
      'settlementReceiptRef',
      'meteringReceiptRef',
    ],
    settlementMode: 'existing_metering_and_settlement_spine',
    state: 'modeled_not_bound',
    trigger:
      'A future bound SLA would attach service terms to verified accepted outcomes only after the trace, verdict, settlement, and metering receipts agree.',
  }),
]

export const projectTraceEconomicUnderwriting = (
  input: ProjectTraceEconomicUnderwritingInput = {},
): TraceEconomicUnderwritingProjection => {
  const outcomes = input.outcomes ?? seedTraceEconomicUnderwritingOutcomes()
  const qualifyingOutcomes = outcomes.filter(isQualifyingOutcome)
  const incompleteOutcomes = outcomes.filter(outcome => !isQualifyingOutcome(outcome))
  const receiptCoverageBps =
    outcomes.length === 0
      ? 0
      : clampBps((qualifyingOutcomes.length / outcomes.length) * 10000)
  const gateState =
    qualifyingOutcomes.length >=
      TraceEconomicUnderwritingMinimumReadyOutcomeCount &&
    receiptCoverageBps >= TraceEconomicUnderwritingMinimumReadyCoverageBps
      ? 'green'
      : 'yellow'

  return new TraceEconomicUnderwritingProjection({
    authorityBoundary:
      'Trace-economic underwriting is a public read-only readiness projection. It grants no policy binding, premium quote, underwriting, claims adjudication, refund, payout, settlement, custody, marketplace ranking, dispatch, public risk-market claim, or spend authority.',
    definitionRef:
      'docs/research/idai/roadmap-alignment.md#2-trace-economic-underwriting-safety-economics',
    gate: new TraceEconomicUnderwritingGate({
      blockerRefs:
        gateState === 'green'
          ? []
          : [
              'blocker.underwriting.needs_more_verified_settled_metered_outcomes',
              'blocker.underwriting.receipt_coverage_below_warranty_threshold',
              'blocker.underwriting.no_bound_policy_or_premium_authority',
            ],
      caveatRefs: [
        'caveat.underwriting.seed_projection_not_insurance_product',
        'caveat.underwriting.shapley_credit_assignment_deferred',
      ],
      currentQualifyingOutcomeCount: qualifyingOutcomes.length,
      currentReceiptCoverageBps: receiptCoverageBps,
      refundPromiseAllowed: gateState === 'green',
      requiredQualifyingOutcomeCount:
        TraceEconomicUnderwritingMinimumReadyOutcomeCount,
      requiredReceiptCoverageBps:
        TraceEconomicUnderwritingMinimumReadyCoverageBps,
      slaPromiseAllowed: gateState === 'green',
      state: gateState,
      warrantyOfferAllowed: gateState === 'green',
    }),
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    incompleteOutcomeRefs: incompleteOutcomes.map(outcome => outcome.outcomeRef),
    issueRef: 'github:OpenAgentsInc/openagents#6426',
    observed: {
      averageClaimableLossBps: averageClaimableLossBps(qualifyingOutcomes),
      incompleteOutcomeCount: incompleteOutcomes.length,
      inputOutcomeCount: outcomes.length,
      qualifyingOutcomeCount: qualifyingOutcomes.length,
      receiptCoverageBps,
    },
    projectionId: 'underwriting.trace_economic.v1',
    promiseRef: 'promise:markets.risk_bonds_underwriting.v1',
    qualifyingOutcomeRefs: qualifyingOutcomes.map(outcome => outcome.outcomeRef),
    schemaVersion: TraceEconomicUnderwritingSchemaVersion,
    sourceRefs: [
      'docs/research/idai/roadmap-alignment.md',
      'docs/research/idai/safety-economics.md',
      'apps/openagents.com/workers/api/src/trace-economic-underwriting.ts',
    ],
    staleness: TraceEconomicUnderwritingStaleness,
    status: 'instrumented_seed',
    statusLabel:
      gateState === 'green'
        ? 'Trace-economic underwriting has enough verified, settled, metered outcomes for warranty offer copy gates.'
        : `${qualifyingOutcomes.length} of ${TraceEconomicUnderwritingMinimumReadyOutcomeCount} qualifying verified, settled, metered outcomes are present; warranty and SLA offers remain blocked.`,
    substrate: {
      meteringSource: 'khala_metering_receipts',
      pricingBasis: 'claimable_loss_bps_over_qualifying_outcomes',
      settlementSource: 'public_safe_settlement_receipts',
      traceSurface: '/trace/{uuid}',
      verdictSource: 'replay_verdicts',
    },
    unsafeCopy:
      'Do not describe this projection as insurance, a bound warranty, a premium quote, a refund entitlement, a claim payout, a live SLA offer, a green risk market, or underwriting authority. It only reports whether public-safe trace, verdict, settlement, and metering evidence is ready to support a future owner-approved warranty product.',
    warrantyShapes: warrantyShapes(),
  })
}
