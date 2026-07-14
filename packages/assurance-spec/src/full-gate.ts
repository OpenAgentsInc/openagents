export type VitePlusTestSummary = Readonly<{
  passed: number
  skipped: number
  failed: number
}>

/** Parse the final Vite Plus/Vitest `Tests` row without treating missing zero axes as failure. */
export const parseVitePlusTestSummary = (output: string): VitePlusTestSummary | null => {
  const rows = [...output.matchAll(/^\s*Tests\s+(.+)$/gmu)]
  const row = rows.at(-1)?.[1]
  if (row === undefined) return null
  const count = (label: "passed" | "skipped" | "failed"): number | null => {
    const value = row.match(new RegExp(`(?:^|\\|)\\s*(\\d+)\\s+${label}(?:\\s|\\||$)`, "u"))?.[1]
    return value === undefined ? null : Number(value)
  }
  const passed = count("passed")
  if (passed === null) return null
  return { passed, skipped: count("skipped") ?? 0, failed: count("failed") ?? 0 }
}
