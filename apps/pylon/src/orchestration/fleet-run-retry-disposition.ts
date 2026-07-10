export const ACCOUNT_HEALTH_REDISPATCH_DISPOSITION = "account_health_redispatch" as const

type PersistedFleetRunTaskResult = {
  readonly result: string | null
  readonly status: string
}

/**
 * The retry marker is durable task metadata, not an inference from a later
 * planner snapshot. Keeping the decoder shared prevents restart planning from
 * treating an account failure as an ordinary failed work unit.
 */
export const taskHasAccountHealthRedispatchDisposition = (
  task: PersistedFleetRunTaskResult | null | undefined,
): boolean => {
  if (task?.status !== "failed" || task.result === null) return false
  try {
    const result = JSON.parse(task.result) as { readonly retryDisposition?: unknown }
    return result.retryDisposition === ACCOUNT_HEALTH_REDISPATCH_DISPOSITION
  } catch {
    return false
  }
}
