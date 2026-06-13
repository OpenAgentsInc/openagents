export const CANONICAL_DARK = {
  bg: "#000",
  bgSecondary: "#151515",
  text: "#d7d8e5",
  textSecondary: "#8a8c93",
  outline: "#525458",
  primary: "#fff",
  success: "#00c853",
  warning: "#ffb400",
  danger: "#d32f2f",
  info: "#2979ff",
} as const

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
