import { createHash } from "node:crypto"

export function classifySessionError(error: unknown): {
  errorClass: string
  errorDigestRef: string
} {
  const message =
    error instanceof Error ? `${error.name}: ${error.message}` : String(error)
  const lowerMessage = message.toLowerCase()
  const errorDigestRef = `digest.pylon.session.error.${createHash("sha256")
    .update(message)
    .digest("hex")
    .slice(0, 24)}`

  if (lowerMessage.includes("cancel")) {
    return { errorClass: "cancelled", errorDigestRef }
  }

  if (
    lowerMessage.includes("codex account exhausted") ||
    lowerMessage.includes("usage limit") ||
    lowerMessage.includes("quota") ||
    lowerMessage.includes("purchase more credits") ||
    lowerMessage.includes("billing limit") ||
    lowerMessage.includes("out of credits")
  ) {
    return { errorClass: "account_exhausted", errorDigestRef }
  }

  if (
    lowerMessage.includes("rate limit") ||
    lowerMessage.includes("too many requests") ||
    lowerMessage.includes("429")
  ) {
    return { errorClass: "account_rate_limited", errorDigestRef }
  }

  if (
    lowerMessage.includes("invalid session id") ||
    lowerMessage.includes("expected an optional prefix of urn:uuid")
  ) {
    return { errorClass: "invalid_codex_session_id", errorDigestRef }
  }

  if (lowerMessage.includes("account")) {
    return { errorClass: "account_selection", errorDigestRef }
  }

  if (
    lowerMessage.includes("worktree") ||
    lowerMessage.includes("workspace")
  ) {
    return { errorClass: "workspace_materialization", errorDigestRef }
  }

  if (lowerMessage.includes("verify") || lowerMessage.includes("dev check")) {
    return { errorClass: "verification_failed", errorDigestRef }
  }

  if (lowerMessage.includes("redaction scan")) {
    return { errorClass: "redaction_gate", errorDigestRef }
  }

  return { errorClass: "execution_error", errorDigestRef }
}
