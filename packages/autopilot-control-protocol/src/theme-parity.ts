import { autopilotCoreProtocolDarkTokens } from "@openagentsinc/design-tokens"

export const CANONICAL_DARK = autopilotCoreProtocolDarkTokens()

export const assertThemeParity = (
  actual: Record<string, string>,
): { ok: boolean; mismatches: string[] } => {
  const mismatches = Object.entries(CANONICAL_DARK).flatMap(([key, expected]) => {
    const received = actual[key]

    if (received === undefined) return [`${key}: missing, expected ${expected}`]
    if (received !== expected) return [`${key}: expected ${expected}, received ${received}`]
    return []
  })

  return {
    ok: mismatches.length === 0,
    mismatches,
  }
}
