import type {
  OpenAgentsAutopilotBuyerPaymentMode,
  OpenAgentsAutopilotPrivacyTier,
  OpenAgentsAutopilotRunnerKind,
  OpenAgentsAutopilotSettlementMode,
  OpenAgentsAutopilotTaskKind,
  OpenAgentsAutopilotWorkRequest,
} from './autopilot-work-request'

export type AutopilotWorkQuoteLineItem = Readonly<{
  amountCents: number
  code: string
  descriptionRef: string
}>

export type AutopilotWorkQuote = Readonly<{
  amountCents: number
  buyerPaymentMode: OpenAgentsAutopilotBuyerPaymentMode
  currency: 'USD'
  freeSlice: boolean
  lineItems: ReadonlyArray<AutopilotWorkQuoteLineItem>
  maxSpendCents: number
  paymentRequired: boolean
  pricingVersion: 'openagents.autopilot_work_quote.v1'
  quoteRef: string
  settlementMode: OpenAgentsAutopilotSettlementMode
}>

const PricingVersion = 'openagents.autopilot_work_quote.v1' as const

const taskKindAmountCents = (
  kind: OpenAgentsAutopilotTaskKind,
): number => {
  switch (kind) {
    case 'benchmark_or_gepa':
      return 3000
    case 'code_change':
    case 'repo_change':
    case 'test_repair':
      return 2500
    case 'research_and_patch':
      return 2200
    case 'site_adjustment':
      return 1800
    case 'site_generation':
      return 3500
  }
}

const privacyTierAmountCents = (
  privacyTier: OpenAgentsAutopilotPrivacyTier,
): number => {
  switch (privacyTier) {
    case 'public_beta':
      return 0
    case 'cloud_allowed':
      return 500
    case 'customer_local_pylon':
      return 700
    case 'local_only':
      return 900
    case 'openagents_shc':
      return 1500
    case 'maple_ai':
      return 3000
    case 'tee':
      return 3500
  }
}

const runnerKindAmountCents = (
  runnerKind: OpenAgentsAutopilotRunnerKind,
): number => {
  switch (runnerKind) {
    case 'requester_pylon':
    case 'pylon_network':
      return 500
    case 'hosted_gemini':
    case 'cloud_sandbox':
      return 1000
    case 'openagents_shc':
    case 'shc':
      return 1800
    case 'maple_ai':
    case 'tee':
      return 3000
  }
}

const maxRunnerAmountCents = (
  runnerKinds: ReadonlyArray<OpenAgentsAutopilotRunnerKind>,
): number =>
  runnerKinds.reduce(
    (maxAmount, runnerKind) =>
      Math.max(maxAmount, runnerKindAmountCents(runnerKind)),
    0,
  )

const safeQuoteRefSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:-]/g, '_').slice(0, 120)

const computedQuoteLineItems = (
  request: OpenAgentsAutopilotWorkRequest,
): ReadonlyArray<AutopilotWorkQuoteLineItem> => {
  const taskAmount = request.tasks.reduce(
    (sum, task) => sum + taskKindAmountCents(task.kind),
    0,
  )
  const privacyAmount = privacyTierAmountCents(
    request.placementPolicy.privacyTier,
  )
  const runnerAmount = Math.max(
    maxRunnerAmountCents(request.placementPolicy.allowedRunnerKinds),
    maxRunnerAmountCents(request.placementPolicy.preferredRunnerKinds),
  )
  const localOnlyAmount = request.placementPolicy.localOnlyAllowed ? 500 : 0
  const secretBrokerAmount = request.placementPolicy.requiresSecretBroker
    ? 1200
    : 0
  const privateTraceAmount = request.placementPolicy.publicTraceAllowed
    ? 0
    : 600

  return [
    {
      amountCents: taskAmount,
      code: 'task_scope',
      descriptionRef: `pricing.tasks.${request.tasks.length}`,
    },
    {
      amountCents: privacyAmount,
      code: 'privacy_tier',
      descriptionRef:
        `pricing.privacy.${request.placementPolicy.privacyTier}`,
    },
    {
      amountCents: runnerAmount,
      code: 'runner_class',
      descriptionRef: 'pricing.runner.max_requested',
    },
    {
      amountCents: localOnlyAmount,
      code: 'local_only',
      descriptionRef: 'pricing.local_only',
    },
    {
      amountCents: secretBrokerAmount,
      code: 'secret_broker',
      descriptionRef: 'pricing.secret_broker',
    },
    {
      amountCents: privateTraceAmount,
      code: 'private_trace',
      descriptionRef: 'pricing.private_trace',
    },
  ].filter(item => item.amountCents > 0)
}

export const makeAutopilotWorkQuote = (
  request: OpenAgentsAutopilotWorkRequest,
): AutopilotWorkQuote => {
  const persistedQuote =
    request.paymentPolicy.quoteRef !== null &&
    request.paymentPolicy.quotedAmountCents !== null
  const freeSlice = request.paymentPolicy.buyerPaymentMode === 'free_slice'
  const lineItems = freeSlice
    ? [
        {
          amountCents: 0,
          code: 'public_free_slice',
          descriptionRef: 'pricing.public_free_slice',
        },
      ]
    : persistedQuote
      ? [
          {
            amountCents: request.paymentPolicy.quotedAmountCents ?? 0,
            code: 'persisted_quote',
            descriptionRef: request.paymentPolicy.quoteRef ?? 'quote.persisted',
          },
        ]
      : computedQuoteLineItems(request)
  const amountCents = freeSlice
    ? 0
    : lineItems.reduce((sum, item) => sum + item.amountCents, 0)
  const quoteRef = persistedQuote
    ? request.paymentPolicy.quoteRef ?? ''
    : [
        'quote.autopilot_work',
        safeQuoteRefSuffix(request.clientRequestRef),
        String(amountCents),
        PricingVersion,
      ].join('.')

  return {
    amountCents,
    buyerPaymentMode: request.paymentPolicy.buyerPaymentMode,
    currency: 'USD',
    freeSlice,
    lineItems,
    maxSpendCents: request.paymentPolicy.maxSpendCents,
    paymentRequired: amountCents > 0,
    pricingVersion: PricingVersion,
    quoteRef,
    settlementMode: request.paymentPolicy.settlementMode,
  }
}
