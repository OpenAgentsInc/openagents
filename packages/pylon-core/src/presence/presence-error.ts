export class PresenceRequestError extends Error {
  override readonly name = "PresenceRequestError"

  constructor(
    readonly status: number,
    readonly responseText: string,
  ) {
    super(`OpenAgents presence request failed (${status}): ${responseText}`)
  }
}

export function isPresenceUnauthorizedError(error: unknown) {
  if (error instanceof PresenceRequestError) return error.status === 401
  const maybeStatus = typeof error === "object" && error !== null && "status" in error ? error.status : undefined
  if (maybeStatus === 401) return true
  const message = error instanceof Error ? error.message : String(error)
  return /\bOpenAgents presence request failed \(401\):/.test(message)
}
