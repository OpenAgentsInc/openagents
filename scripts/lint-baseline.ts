export interface LintMessage {
  readonly ruleId: string | null
}

export interface LintResult {
  readonly messages: readonly LintMessage[]
}

export const legacyLintBudgets: Readonly<Record<string, number>> = {
  "@typescript-eslint/consistent-type-assertions": 7_863,
  "@typescript-eslint/no-unused-vars": 1_429,
  "@typescript-eslint/no-explicit-any": 140,
  fatal: 2,
}

export const summarizeLintResults = (results: readonly LintResult[]): Readonly<Record<string, number>> => {
  const counts: Record<string, number> = {}
  for (const result of results) {
    for (const message of result.messages) {
      const rule = message.ruleId ?? "fatal"
      counts[rule] = (counts[rule] ?? 0) + 1
    }
  }
  return counts
}

export const lintBudgetViolations = (
  counts: Readonly<Record<string, number>>,
  budgets: Readonly<Record<string, number>> = legacyLintBudgets,
): readonly string[] => {
  const violations: string[] = []
  for (const [rule, count] of Object.entries(counts).sort(([left], [right]) => left.localeCompare(right))) {
    const budget = budgets[rule]
    if (budget === undefined) violations.push(`${rule}: ${count} new unbudgeted violation(s)`)
    else if (count > budget) violations.push(`${rule}: ${count} exceeds legacy budget ${budget}`)
  }
  return violations
}

const run = async (): Promise<void> => {
  const child = Bun.spawn(["bunx", "eslint", ".", "-f", "json"], {
    cwd: "apps/openagents.com",
    stdout: "pipe",
    stderr: "inherit",
  })
  const output = await new Response(child.stdout).text()
  await child.exited
  const results = JSON.parse(output) as readonly LintResult[]
  const counts = summarizeLintResults(results)
  const violations = lintBudgetViolations(counts)

  for (const [rule, count] of Object.entries(counts).sort(([left], [right]) => left.localeCompare(right))) {
    console.error(`[lint] ${rule}: ${count}/${legacyLintBudgets[rule] ?? 0}`)
  }
  if (violations.length > 0) throw new Error(violations.join("\n"))
  console.error("[lint] no new violations above the explicit legacy baseline")
}

if (import.meta.main) {
  try {
    await run()
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    process.exitCode = 1
  }
}
