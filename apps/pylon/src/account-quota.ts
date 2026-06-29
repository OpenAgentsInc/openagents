import { createHash } from "node:crypto"

export type QuotaSignal = {
  exhausted: boolean
  retryAtRaw: string | null
  retryAtIso: string | null
  sourceDigestRef: string
}

const quotaPattern =
  /usage limit|hit your usage limit|purchase more credits|rate limit|retry-after|try again (?:at|after|in)/

type RetryAtParseOptions = {
  now?: Date
}

function retryAtIsoFromRaw(retryAtRaw: string | null, options: RetryAtParseOptions = {}): string | null {
  if (retryAtRaw === null) return null

  try {
    const normalized = retryAtRaw.trim().replace(/\b(\d{1,2})(st|nd|rd|th)\b/gi, "$1")
    const retryAfterSeconds = normalized.match(/^(?:retry-after:\s*)?(\d{1,8})$/i)?.[1]
    if (retryAfterSeconds !== undefined) {
      return new Date((options.now ?? new Date()).getTime() + Number(retryAfterSeconds) * 1000).toISOString()
    }
    const relative = normalized.match(/^(\d{1,5})\s*(second|seconds|sec|secs|s|minute|minutes|min|mins|m|hour|hours|hr|hrs|h)\b/i)
    if (relative) {
      const amount = Number(relative[1])
      const unit = relative[2]?.toLowerCase() ?? ""
      const multiplier = unit.startsWith("s")
        ? 1000
        : unit.startsWith("m")
          ? 60_000
          : 3600_000
      return new Date((options.now ?? new Date()).getTime() + amount * multiplier).toISOString()
    }
    const parsed = new Date(normalized)
    if (Number.isNaN(parsed.getTime())) return null

    return parsed.toISOString()
  } catch {
    return null
  }
}

export function classifyQuotaSignal(
  output: string,
  provider: "codex" | "claude_agent",
  options: RetryAtParseOptions = {},
): QuotaSignal {
  void provider

  try {
    const normalizedOutput = String(output ?? "")
    const lowerOutput = normalizedOutput.toLowerCase()
    const retryAtRaw =
      normalizedOutput.match(/\bretry-after:\s*([^\r\n]+)/i)?.[1]?.trim() ??
      normalizedOutput.match(/try again (?:at|after|in)\s+(.+?)(?:\.|$)/i)?.[1]?.trim() ??
      null

    return {
      exhausted: quotaPattern.test(lowerOutput),
      retryAtRaw,
      retryAtIso: retryAtIsoFromRaw(retryAtRaw, options),
      sourceDigestRef: `digest.pylon.account_quota.${createHash("sha256")
        .update(normalizedOutput)
        .digest("hex")
        .slice(0, 24)}`,
    }
  } catch {
    return {
      exhausted: false,
      retryAtRaw: null,
      retryAtIso: null,
      sourceDigestRef: `digest.pylon.account_quota.${createHash("sha256").update("").digest("hex").slice(0, 24)}`,
    }
  }
}
