import { createHash } from "node:crypto"

export const PYLON_DISPATCH_BREAKER_SCHEMA = "openagents.pylon.dispatch_breaker.v0.1" as const

export type PylonDispatchFailureKind = "permanent" | "transient"

export type PylonDispatchFailureReason =
  | "account_credentials_revoked"
  | "account_rate_limited"
  | "account_usage_limited"
  | "lane_assignment_conflict"
  | "lane_capacity_unavailable"
  | "lane_internal"
  | "lane_network"
  | "lane_provider_unavailable"
  | "lane_public_safety_blocked"
  | "lane_timeout"

export type PylonDispatchFailureLane = "claude_agent" | "codex" | "generic" | "unknown"

export type PylonDispatchFailureClassification = {
  blockerRef: string
  cooldownMs: number | null
  failureKind: PylonDispatchFailureKind
  reason: PylonDispatchFailureReason
  sourceDigestRef: string
}

export type PylonDispatchFailureInput = {
  blockerRefs?: readonly string[]
  error?: unknown
  status?: string | null
}

export type PylonDispatchBreakerSnapshot = {
  accountRefHash: string | null
  blockerRefs: string[]
  contextId: string | null
  cooldownUntil: string | null
  failureCount: number
  failureKind: PylonDispatchFailureKind
  firstObservedAt: string
  lane: PylonDispatchFailureLane
  lastObservedAt: string
  reason: PylonDispatchFailureReason
  schema: typeof PYLON_DISPATCH_BREAKER_SCHEMA
  scopeKey: string
  sourceDigestRef: string
}

const DEFAULT_TRANSIENT_COOLDOWN_MS = 5 * 60 * 1000
const RATE_LIMIT_COOLDOWN_MS = 30 * 60 * 1000
const ASSIGNMENT_CONFLICT_COOLDOWN_MS = 60 * 1000
const TIMEOUT_COOLDOWN_MS = 2 * 60 * 1000

const SECRET_PATTERNS: RegExp[] = [
  /sk-[A-Za-z0-9_-]{12,}/g,
  /sess-[A-Za-z0-9_-]{12,}/g,
  /eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/g,
  /(?:access|refresh|id)_token["'\s:=]+[A-Za-z0-9._~+/=-]{8,}/gi,
  /Bearer\s+[A-Za-z0-9._~+/=-]{8,}/gi,
]

const publicSafeText = (value: unknown): string => {
  const raw =
    value instanceof Error
      ? value.message
      : typeof value === "string"
        ? value
        : value === undefined || value === null
          ? ""
          : String(value)
  const collapsed = raw.replace(/\s+/g, " ").trim()
  return SECRET_PATTERNS.reduce((text, pattern) => text.replace(pattern, "[redacted]"), collapsed)
}

const digestRef = (input: string): string =>
  `digest.pylon.dispatch_failure.${createHash("sha256").update(input).digest("hex").slice(0, 24)}`

const classification = (
  reason: PylonDispatchFailureReason,
  failureKind: PylonDispatchFailureKind,
  cooldownMs: number | null,
  digestInput: string,
): PylonDispatchFailureClassification => ({
  blockerRef: `blocker.pylon.dispatch.${reason}`,
  cooldownMs,
  failureKind,
  reason,
  sourceDigestRef: digestRef(digestInput),
})

export function normalizePylonDispatchFailureLane(value: unknown): PylonDispatchFailureLane {
  if (value === "codex") return "codex"
  if (value === "claude" || value === "claude_agent") return "claude_agent"
  if (value === "generic") return "generic"
  return "unknown"
}

export function classifyPylonDispatchFailure(
  input: PylonDispatchFailureInput,
): PylonDispatchFailureClassification {
  const blockerRefs = [...(input.blockerRefs ?? [])].filter(ref => ref.trim() !== "")
  const text = [publicSafeText(input.error), input.status ?? "", ...blockerRefs].join(" ").toLowerCase()
  const digestInput = [publicSafeText(input.error), input.status ?? "", ...blockerRefs].join("\n")

  if (/revok|invalid credential|credentials?_revoked|reauth|unauthori[sz]ed|\b401\b|\b403\b/.test(text)) {
    return classification("account_credentials_revoked", "permanent", null, digestInput)
  }
  if (/public[-_ ]?safety|request_public_safety_blocked|contains private|private data|wallet material|raw wallet|mnemonic/.test(text)) {
    return classification("lane_public_safety_blocked", "permanent", null, digestInput)
  }
  if (/usage[_ -]?limited|usage limit|quota|exhausted|weekly[_ -]?exhausted|out of credits|credit balance|purchase more credits|billing limit|has[_-]?credits["'\s:=]+false/.test(text)) {
    return classification("account_usage_limited", "permanent", null, digestInput)
  }
  if (/rate[_ -]?limited|rate limit|too many requests|\b429\b|cooldown/.test(text)) {
    return classification("account_rate_limited", "transient", RATE_LIMIT_COOLDOWN_MS, digestInput)
  }
  if (/assignment[_ -]?http[_ -]?409|http\s*409|\b409\b|already active|duplicate active/.test(text)) {
    return classification("lane_assignment_conflict", "transient", ASSIGNMENT_CONFLICT_COOLDOWN_MS, digestInput)
  }
  if (/no[_ -]?advertised[_ -]?.*availability|no[_ -]?ready[_ -]?.*slots|target_pylon_unavailable|capacity/.test(text)) {
    return classification("lane_capacity_unavailable", "transient", DEFAULT_TRANSIENT_COOLDOWN_MS, digestInput)
  }
  if (/timed? ?out|timeout|abort|deadline/.test(text)) {
    return classification("lane_timeout", "transient", TIMEOUT_COOLDOWN_MS, digestInput)
  }
  if (/network|econn|enotfound|etimedout|socket|dns|wss|websocket|fetch failed/.test(text)) {
    return classification("lane_network", "transient", DEFAULT_TRANSIENT_COOLDOWN_MS, digestInput)
  }
  if (/\b5\d\d\b|provider unavailable|service unavailable|temporar(?:y|ily) unavailable|upstream/.test(text)) {
    return classification("lane_provider_unavailable", "transient", DEFAULT_TRANSIENT_COOLDOWN_MS, digestInput)
  }
  return classification("lane_internal", "transient", DEFAULT_TRANSIENT_COOLDOWN_MS, digestInput)
}

export function pylonDispatchBreakerScopeKey(input: {
  accountRefHash?: string | null
  contextId?: string | null
  lane: PylonDispatchFailureLane
}): string {
  if (input.accountRefHash && input.accountRefHash.trim() !== "") {
    return `dispatch-breaker.account-lane.${input.lane}.${input.accountRefHash.trim()}`
  }
  if (input.contextId && input.contextId.trim() !== "") {
    return `dispatch-breaker.context-lane.${input.lane}.${input.contextId.trim()}`
  }
  return `dispatch-breaker.lane.${input.lane}`
}

export function pylonDispatchBreakerIsActive(
  breaker: PylonDispatchBreakerSnapshot,
  now: Date = new Date(),
): boolean {
  if (breaker.failureKind === "permanent") return true
  if (breaker.cooldownUntil === null) return false
  const cooldownUntilMs = Date.parse(breaker.cooldownUntil)
  return Number.isFinite(cooldownUntilMs) && cooldownUntilMs > now.getTime()
}

export function activePylonDispatchBreakers(
  breakers: readonly PylonDispatchBreakerSnapshot[],
  now: Date = new Date(),
): PylonDispatchBreakerSnapshot[] {
  return breakers.filter(breaker => pylonDispatchBreakerIsActive(breaker, now))
}

export function pylonDispatchBreakerForAccount(input: {
  accountRefHash: string
  breakers: readonly PylonDispatchBreakerSnapshot[]
  lane: PylonDispatchFailureLane
  now?: Date
}): PylonDispatchBreakerSnapshot | null {
  const now = input.now ?? new Date()
  return activePylonDispatchBreakers(input.breakers, now).find(
    breaker =>
      breaker.lane === input.lane &&
      breaker.accountRefHash === input.accountRefHash,
  ) ?? null
}
