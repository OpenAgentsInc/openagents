export type CloudCostEstimateInput = {
  tokensIn: number
  tokensOut: number
  ratePerMTokIn: number
  ratePerMTokOut: number
}

export type CloudCostEstimate = {
  costSats: number
  breakdown: {
    in: number
    out: number
  }
}

export function estimateCloudCost(
  input: CloudCostEstimateInput,
): CloudCostEstimate {
  const tokensIn = nonNegativeFinite(input.tokensIn)
  const tokensOut = nonNegativeFinite(input.tokensOut)
  const ratePerMTokIn = nonNegativeFinite(input.ratePerMTokIn)
  const ratePerMTokOut = nonNegativeFinite(input.ratePerMTokOut)

  const inCost = Math.round((tokensIn / 1_000_000) * ratePerMTokIn)
  const outCost = Math.round((tokensOut / 1_000_000) * ratePerMTokOut)

  return {
    costSats: inCost + outCost,
    breakdown: {
      in: inCost,
      out: outCost,
    },
  }
}

function nonNegativeFinite(value: number): number {
  if (!Number.isFinite(value) || value < 0) return 0
  return value
}
