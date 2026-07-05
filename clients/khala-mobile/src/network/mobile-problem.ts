export type MobileProblemKind =
  | "network_unavailable"
  | "timeout"
  | "unauthorized"
  | "forbidden"
  | "not_found"
  | "rate_limited"
  | "server_error"
  | "malformed_response"
  | "unknown"

export type MobileProblem = Readonly<{
  kind: MobileProblemKind
  messageSafe: string
  status: number | null
}>

export class MobileProblemError extends Error {
  readonly problem: MobileProblem

  constructor(problem: MobileProblem) {
    super(problem.messageSafe)
    this.name = "MobileProblemError"
    this.problem = problem
  }
}

export type MobileProblemResponseLike = Readonly<{
  ok: boolean
  status?: number
  json: () => Promise<unknown>
}>

const DEFAULT_MESSAGES: Record<MobileProblemKind, string> = {
  forbidden: "request forbidden",
  malformed_response: "server returned an unreadable response",
  network_unavailable: "network unavailable",
  not_found: "resource not found",
  rate_limited: "rate limited",
  server_error: "server error",
  timeout: "request timed out",
  unauthorized: "sign-in required",
  unknown: "request failed",
}

const safeBodyMessage = (body: unknown): string | null => {
  if (body === null || typeof body !== "object" || !("messageSafe" in body)) return null
  const message = (body as { messageSafe: unknown }).messageSafe
  return typeof message === "string" && message.trim().length > 0 ? message.trim() : null
}

export const classifyMobileStatusProblem = (
  status: number | undefined,
  body: unknown,
  fallbackContext: string,
): MobileProblem => {
  const safe = safeBodyMessage(body)
  const kind: MobileProblemKind =
    status === 401
      ? "unauthorized"
      : status === 403
        ? "forbidden"
        : status === 404
          ? "not_found"
          : status === 429
            ? "rate_limited"
            : typeof status === "number" && status >= 500
              ? "server_error"
              : "unknown"
  const suffix = typeof status === "number" ? ` (${status})` : ""
  return {
    kind,
    messageSafe: safe ?? `${fallbackContext}: ${DEFAULT_MESSAGES[kind]}${suffix}`,
    status: status ?? null,
  }
}

export const classifyMobileThrownProblem = (
  error: unknown,
  fallbackContext: string,
): MobileProblem => {
  const name = error instanceof Error ? error.name.toLowerCase() : ""
  const message = error instanceof Error ? error.message.toLowerCase() : ""
  const kind: MobileProblemKind =
    name === "aborterror" || message.includes("abort") || message.includes("timeout")
      ? "timeout"
      : message.includes("network") || message.includes("failed to fetch")
        ? "network_unavailable"
        : "unknown"
  return {
    kind,
    messageSafe: `${fallbackContext}: ${DEFAULT_MESSAGES[kind]}`,
    status: null,
  }
}

export const malformedMobileResponseProblem = (
  fallbackContext: string,
): MobileProblem => ({
  kind: "malformed_response",
  messageSafe: `${fallbackContext}: ${DEFAULT_MESSAGES.malformed_response}`,
  status: null,
})

export const readMobileJsonResponse = async (
  response: MobileProblemResponseLike,
  fallbackContext: string,
): Promise<unknown> => {
  try {
    return await response.json()
  } catch {
    throw new MobileProblemError(malformedMobileResponseProblem(fallbackContext))
  }
}

export const readOkMobileJsonResponse = async (
  response: MobileProblemResponseLike,
  fallbackContext: string,
): Promise<unknown> => {
  const body = await readMobileJsonResponse(response, fallbackContext)
  if (!response.ok) {
    throw new MobileProblemError(
      classifyMobileStatusProblem(response.status, body, fallbackContext),
    )
  }
  return body
}

export const mobileProblemMessageSafe = (
  error: unknown,
  fallbackContext: string,
): string =>
  error instanceof MobileProblemError
    ? error.problem.messageSafe
    : classifyMobileThrownProblem(error, fallbackContext).messageSafe
