export type ConnectionRetryInput = {
  attempt: number
  maxMs?: number
}

export type ConnectionRetryPlan = {
  delayMs: number
  giveUp: boolean
}

const baseDelayMs = 1_000
const defaultMaxDelayMs = 30_000
const giveUpAttempt = 8

export function nextRetry(input: ConnectionRetryInput): ConnectionRetryPlan {
  const attempt = Math.max(0, Math.floor(input.attempt))
  const maxMs = input.maxMs ?? defaultMaxDelayMs
  const delayMs = Math.min(baseDelayMs * 2 ** attempt, maxMs)

  return {
    delayMs,
    giveUp: input.attempt >= giveUpAttempt,
  }
}
