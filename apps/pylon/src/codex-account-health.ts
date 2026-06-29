import { createHash } from "node:crypto"

export type PylonCodexAccountHealthReason =
  | "credentials_revoked"
  | "usage_limited"
  | "rate_limited"
  | "network"
  | "timeout"
  | "other"

export type PylonCodexAccountFailure = {
  reason: PylonCodexAccountHealthReason
  publicMessage: string
  sourceDigestRef: string
}

const SECRET_PATTERNS: RegExp[] = [
  /sk-[A-Za-z0-9_-]{12,}/g,
  /sess-[A-Za-z0-9_-]{12,}/g,
  /eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/g,
  /(?:access|refresh|id)_token["'\s:=]+[A-Za-z0-9._~+/=-]{8,}/gi,
  /Bearer\s+[A-Za-z0-9._~+/=-]{8,}/gi,
]

export function publicSafeCodexFailureMessage(value: unknown): string {
  const raw =
    value instanceof Error
      ? value.message
      : typeof value === "string"
        ? value
        : value === undefined || value === null
          ? ""
          : String(value)
  const collapsed = raw.replace(/\s+/g, " ").trim()
  const redacted = SECRET_PATTERNS.reduce(
    (text, pattern) => text.replace(pattern, "[redacted]"),
    collapsed,
  )
  return redacted.length > 280 ? `${redacted.slice(0, 277)}...` : redacted
}

export function classifyCodexAccountFailure(value: unknown): PylonCodexAccountFailure {
  const publicMessage = publicSafeCodexFailureMessage(value)
  const text = publicMessage.toLowerCase()
  const reason: PylonCodexAccountHealthReason =
    /revok/.test(text)
      ? "credentials_revoked"
      : /5\s*[- ]?\s*hour|five\s+hour|rate limit|too many requests|\b429\b/.test(text)
          ? "rate_limited"
          : /usage limit|quota|purchase more credits|billing limit/.test(text)
            ? "usage_limited"
            : /timed? ?out|deadline|abort/.test(text)
              ? "timeout"
              : /network|econn|enotfound|etimedout|socket|dns|wss|websocket|fetch failed/.test(text)
                ? "network"
                : "other"
  return {
    reason,
    publicMessage,
    sourceDigestRef: `digest.pylon.codex_account_failure.${createHash("sha256")
      .update(publicMessage)
      .digest("hex")
      .slice(0, 24)}`,
  }
}

export function codexAccountFailureBlockerRefs(reason: PylonCodexAccountHealthReason): string[] {
  return [
    `blocker.assignment.codex_agent_execution_${reason}`,
    ...(reason === "credentials_revoked"
      ? ["blocker.assignment.codex_account_credentials_revoked_needs_owner_reauth"]
      : []),
  ]
}
