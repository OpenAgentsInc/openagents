/**
 * GFN Formula Calculator
 * Implements the Group-Forming Network value calculation
 */

export interface GFNParameters {
  // Core variables
  n: number // Number of active participants
  C: number // Clustering coefficient (0-1)

  // Network type coefficients (α values)
  alpha1: number // Broadcast (Sarnoff)
  alpha2: number // P2P (Metcalfe)
  alpha3: number // Group (Reed)

  // Value per connection (k values)
  k1: number // Value per broadcast connection
  k2: number // Value per P2P connection
  k3: number // Value per group potential

  // Multipliers
  Q: number // Quality factor
  M: number // Multi-sided platform multiplier
  D: number // Data network effect
}

export interface GFNResults {
  totalValue: number
  sarnoffValue: number
  metcalfeValue: number
  reedValue: number
  baseValue: number // Before multipliers
  dominantEffect: "sarnoff" | "metcalfe" | "reed"
  percentages: {
    sarnoff: number
    metcalfe: number
    reed: number
  }
}

export const DEFAULT_PARAMS: GFNParameters = {
  n: 1000,
  C: 0.2,
  alpha1: 0.15,
  alpha2: 0.35,
  alpha3: 0.50,
  k1: 0.005,
  k2: 0.002,
  k3: 0.0003,
  Q: 1.5,
  M: 2.0,
  D: 0.5
}

export const OPENAI_PARAMS: GFNParameters = {
  n: 100_000_000,
  C: 0.05,
  alpha1: 0.15,
  alpha2: 0.45,
  alpha3: 0.40,
  k1: 0.002,
  k2: 0.001,
  k3: 0.0001,
  Q: 2.5,
  M: 2.0,
  D: 0.7
}

export const ANTHROPIC_PARAMS: GFNParameters = {
  n: 10_000_000,
  C: 0.08,
  alpha1: 0.10,
  alpha2: 0.50,
  alpha3: 0.40,
  k1: 0.003,
  k2: 0.0015,
  k3: 0.00015,
  Q: 2.8,
  M: 1.8,
  D: 0.6
}

export const OPENAGENTS_CURRENT: GFNParameters = {
  n: 10,
  C: 0.1,
  alpha1: 0.05,
  alpha2: 0.25,
  alpha3: 0.70,
  k1: 0.001,
  k2: 0.0005,
  k3: 0.00001,
  Q: 0.5,
  M: 1.0,
  D: 0.1
}

export const OPENAGENTS_PROJECTIONS = {
  current: { ...OPENAGENTS_CURRENT },
  sixMonths: { ...OPENAGENTS_CURRENT, n: 1_000, C: 0.3, Q: 1.0, M: 1.5, D: 0.3 },
  oneYear: { ...OPENAGENTS_CURRENT, n: 10_000, C: 0.5, Q: 1.5, M: 2.0, D: 0.5 },
  twoYears: { ...OPENAGENTS_CURRENT, n: 100_000, C: 0.7, Q: 2.0, M: 3.0, D: 0.7, k3: 0.0001 },
  fiveYears: { ...OPENAGENTS_CURRENT, n: 1_000_000, C: 0.85, Q: 3.0, M: 4.0, D: 0.9, k1: 0.005, k2: 0.002, k3: 0.0005 }
}

/**
 * Calculate GFN value with safety checks for large numbers
 */
export function calculateGFN(params: GFNParameters): GFNResults {
  const { C, D, M, Q, alpha1, alpha2, alpha3, k1, k2, k3, n } = params

  // Calculate individual components
  const sarnoffComponent = k1 * n
  const metcalfeComponent = k2 * n * n

  // Reed's Law with safety check for exponential growth
  let reedComponent = 0
  if (n < 50) {
    // For small n, calculate directly
    reedComponent = k3 * Math.pow(2, n) * C
  } else {
    // For large n, use approximation to avoid overflow
    // ln(2^n * k3 * C) = n * ln(2) + ln(k3) + ln(C)
    const logValue = n * Math.log(2) + Math.log(k3) + Math.log(Math.max(C, 0.0001))
    if (logValue < 100) { // e^100 is still manageable
      reedComponent = Math.exp(logValue)
    } else {
      // For extremely large values, cap at a reasonable maximum
      reedComponent = Number.MAX_SAFE_INTEGER / 1000
    }
  }

  // Apply network type coefficients
  const sarnoffValue = alpha1 * sarnoffComponent
  const metcalfeValue = alpha2 * metcalfeComponent
  const reedValue = alpha3 * reedComponent

  // Calculate base value
  const baseValue = sarnoffValue + metcalfeValue + reedValue

  // Apply multipliers
  const totalValue = baseValue * Q * M * (1 + D)

  // Determine dominant effect
  let dominantEffect: "sarnoff" | "metcalfe" | "reed" = "sarnoff"
  const maxComponent = Math.max(sarnoffValue, metcalfeValue, reedValue)
  if (maxComponent === metcalfeValue) dominantEffect = "metcalfe"
  if (maxComponent === reedValue) dominantEffect = "reed"

  // Calculate percentages
  const sum = sarnoffValue + metcalfeValue + reedValue
  const percentages = {
    sarnoff: (sarnoffValue / sum) * 100,
    metcalfe: (metcalfeValue / sum) * 100,
    reed: (reedValue / sum) * 100
  }

  return {
    totalValue,
    sarnoffValue,
    metcalfeValue,
    reedValue,
    baseValue,
    dominantEffect,
    percentages
  }
}

/**
 * Format large numbers for display
 */
export function formatValue(value: number): string {
  if (value >= 1e12) return `$${(value / 1e12).toFixed(2)}T`
  if (value >= 1e9) return `$${(value / 1e9).toFixed(2)}B`
  if (value >= 1e6) return `$${(value / 1e6).toFixed(2)}M`
  if (value >= 1e3) return `$${(value / 1e3).toFixed(2)}K`
  return `$${value.toFixed(2)}`
}

/**
 * Format number with commas
 */
export function formatNumber(value: number): string {
  return value.toLocaleString("en-US")
}

/**
 * Get insights based on current parameters
 */
export function getInsights(params: GFNParameters, results: GFNResults): Array<string> {
  const insights: Array<string> = []

  // Dominant effect insight
  const effectName = {
    sarnoff: "Sarnoff's Law (broadcast)",
    metcalfe: "Metcalfe's Law (P2P)",
    reed: "Reed's Law (group-forming)"
  }[results.dominantEffect]

  insights.push(
    `${effectName} contributes ${results.percentages[results.dominantEffect].toFixed(1)}% of network value`
  )

  // Clustering coefficient insight
  if (params.C < 0.3) {
    insights.push(`Low clustering (${(params.C * 100).toFixed(0)}%) limits Reed's Law potential`)
  } else if (params.C > 0.7) {
    insights.push(`High clustering (${(params.C * 100).toFixed(0)}%) unlocks exponential value creation`)
  }

  // Scale insight
  if (params.n < 1000) {
    insights.push("At this scale, linear and quadratic effects dominate")
  } else if (params.n > 1_000_000) {
    insights.push("At massive scale, group-forming potential becomes critical")
  }

  // Quality multiplier insight
  if (params.Q > 2.5) {
    insights.push(`High quality factor (${params.Q}x) significantly amplifies base value`)
  }

  // OpenAgents comparison
  if (params.n < 50_000_000 && params.C > 0.5) {
    const futureParams = { ...params, n: 50_000_000, C: 0.9 }
    const futureResults = calculateGFN(futureParams)
    insights.push(
      `Scaling to 50M agents could increase value ${(futureResults.totalValue / results.totalValue).toFixed(0)}x`
    )
  }

  return insights
}
