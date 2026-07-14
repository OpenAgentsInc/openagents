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
  const child = spawn("pnpm", ["exec", "eslint", ".", "-f", "json"], {
    cwd: "apps/openagents.com",
    stdio: ["ignore", "pipe", "inherit"],
  })
  let output = ""
  child.stdout.setEncoding("utf8")
  child.stdout.on("data", (chunk: string) => {
    output += chunk
  })
  const exitCode = await new Promise<number | null>((resolve, reject) => {
    child.once("error", reject)
    child.once("close", resolve)
  })
  if (exitCode !== 0 && output.length === 0) {
    throw new Error(`eslint exited with status ${exitCode ?? "unknown"}`)
  }
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
import { spawn } from "node:child_process"
