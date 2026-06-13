import { createHash } from "node:crypto"

export type QuotaSignal = {
  exhausted: boolean
  retryAtRaw: string | null
  retryAtIso: string | null
  sourceDigestRef: string
}

const quotaPattern =
  /usage limit|hit your usage limit|purchase more credits|rate limit|try again at/

function retryAtIsoFromRaw(retryAtRaw: string | null): string | null {
  if (retryAtRaw === null) return null

  try {
    const normalized = retryAtRaw.replace(/\b(\d{1,2})(st|nd|rd|th)\b/gi, "$1")
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
): QuotaSignal {
  void provider

  try {
    const normalizedOutput = String(output ?? "")
    const lowerOutput = normalizedOutput.toLowerCase()
    const retryAtRaw = normalizedOutput.match(/try again at\s+(.+?)(?:\.|$)/i)?.[1]?.trim() ?? null

    return {
      exhausted: quotaPattern.test(lowerOutput),
      retryAtRaw,
      retryAtIso: retryAtIsoFromRaw(retryAtRaw),
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
