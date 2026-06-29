import type {
  OpenAgentsAutopilotRunnerKind,
} from './autopilot-work-request'

export type AutopilotWorkPricingMeterKind =
  | 'none'
  | 'usd_credits'

export type AutopilotWorkPricingLanePolicy = Readonly<{
  buyerDebitRequired: boolean
  laneRef: string
  meterKind: AutopilotWorkPricingMeterKind
  reasonRefs: ReadonlyArray<string>
  runnerKind: OpenAgentsAutopilotRunnerKind
  unitAmountCents: number
}>

export type AutopilotWorkPricingPolicy = Readonly<{
  laneMeters: ReadonlyArray<AutopilotWorkPricingLanePolicy>
  policyRef: string
  version: 'openagents.autopilot_work_pricing_policy.v0.3'
}>

export const autopilotWorkPricingPolicy = {
  laneMeters: [
    {
      buyerDebitRequired: false,
      laneRef: 'lane.autopilot_work.requester_pylon_own_job',
      meterKind: 'none',
      reasonRefs: [
        'pricing.autopilot_work.own_pylon_free',
        'placement.reason.placed_on_your_pylon_free',
      ],
      runnerKind: 'requester_pylon',
      unitAmountCents: 0,
    },
    {
      buyerDebitRequired: true,
      laneRef: 'lane.autopilot_work.openagents_shc_fallback',
      meterKind: 'usd_credits',
      reasonRefs: [
        'pricing.autopilot_work.hosted_runner_metered',
        'placement.reason.your_pylon_unavailable_hosted_metered',
      ],
      runnerKind: 'openagents_shc',
      unitAmountCents: 1,
    },
    {
      // Public paid model gateway lane for the hosted Gemini placement.
      // Advances blocker.product_promises.public_paid_model_gateway_missing on
      // api.hosted_gemini.v1: a buyer-funded, usd_credits-metered lane is the
      // metering/pricing-policy piece a paid model gateway needs. It defines
      // the meter only — it does NOT settle, pay out, or imply a live product,
      // and remains inert until the (still-missing) armed executor delivers
      // hosted Gemini work. See docs/launch/vertex-fleet/api.hosted_gemini.v1.md.
      buyerDebitRequired: true,
      laneRef: 'lane.autopilot_work.hosted_gemini_gateway',
      meterKind: 'usd_credits',
      reasonRefs: [
        'pricing.autopilot_work.hosted_gemini_metered',
        'placement.reason.hosted_gemini_gateway_metered',
      ],
      runnerKind: 'hosted_gemini',
      unitAmountCents: 1,
    },
  ],
  policyRef: 'pricing_policy.autopilot_work.v0_3.lane_meter_mapping',
  version: 'openagents.autopilot_work_pricing_policy.v0.3',
} satisfies AutopilotWorkPricingPolicy

export const pricingLaneForRunnerKind = (
  runnerKind: OpenAgentsAutopilotRunnerKind | null,
): AutopilotWorkPricingLanePolicy | null =>
  runnerKind === null
    ? null
    : autopilotWorkPricingPolicy.laneMeters.find(lane =>
        lane.runnerKind === runnerKind
      ) ?? null

export const pricingReasonRefsForRunnerKind = (
  runnerKind: OpenAgentsAutopilotRunnerKind | null,
): ReadonlyArray<string> =>
  pricingLaneForRunnerKind(runnerKind)?.reasonRefs ?? []
