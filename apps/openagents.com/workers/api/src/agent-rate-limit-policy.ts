export const AgentRateLimitPolicy = {
  limit: 60,
  paidRecovery: 'planned_not_live',
  paymentPreviewRequired: true,
  recoveryModes: [
    'wait',
    'operator_review',
    'future_credit_top_up',
    'future_l402',
  ],
  spendCapRequired: true,
  windowSeconds: 60,
} as const

export type AgentRateLimitPolicy = typeof AgentRateLimitPolicy

export const agentRateLimitProjection = () => ({
  limit: AgentRateLimitPolicy.limit,
  paidRecovery: AgentRateLimitPolicy.paidRecovery,
  paymentPreviewRequired: AgentRateLimitPolicy.paymentPreviewRequired,
  recoveryModes: AgentRateLimitPolicy.recoveryModes,
  spendCapRequired: AgentRateLimitPolicy.spendCapRequired,
  windowSeconds: AgentRateLimitPolicy.windowSeconds,
})

export const withAgentRateLimitHeaders = <A extends { headers: Headers }>(
  response: A,
): A => {
  response.headers.set('ratelimit-limit', String(AgentRateLimitPolicy.limit))
  response.headers.set(
    'ratelimit-policy',
    `${AgentRateLimitPolicy.limit};w=${AgentRateLimitPolicy.windowSeconds}`,
  )
  response.headers.set(
    'ratelimit-reset',
    String(AgentRateLimitPolicy.windowSeconds),
  )
  response.headers.set(
    'x-openagents-paid-recovery',
    AgentRateLimitPolicy.paidRecovery,
  )
  response.headers.set(
    'x-openagents-payment-preview-required',
    String(AgentRateLimitPolicy.paymentPreviewRequired),
  )
  response.headers.set(
    'x-openagents-recovery-modes',
    AgentRateLimitPolicy.recoveryModes.join(', '),
  )
  response.headers.set(
    'x-openagents-spend-cap-required',
    String(AgentRateLimitPolicy.spendCapRequired),
  )

  return response
}
