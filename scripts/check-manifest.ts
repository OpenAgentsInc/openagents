export type CheckComponent = "test" | "typecheck" | "lint" | "fmt"

export interface CheckTarget {
  readonly name: string
  readonly command: readonly string[]
}

/**
 * Repository-owned checks that cannot be inferred from workspace package.json
 * scripts. Package test/typecheck/lint/fmt targets are discovered automatically.
 */
export const rootTestTargets: readonly CheckTarget[] = [
  { name: "workspace-check", command: ["bun", "test", "scripts/check-workspace.test.ts"] },
  { name: "lint-baseline", command: ["bun", "test", "scripts/lint-baseline.test.ts"] },
  { name: "sol-docs", command: ["bun", "test", "scripts/check-sol-docs.test.ts"] },
  { name: "qa-pre-push-smoke", command: ["bun", "test", "scripts/qa-pre-push-smoke.test.ts"] },
  { name: "qa-async-gce-trigger", command: ["bun", "test", "scripts/qa-async-gce-trigger.test.ts"] },
  { name: "qa-nightly-matrix", command: ["bun", "test", "scripts/qa-nightly-matrix.test.ts"] },
  { name: "qa-visual-smoke-gate", command: ["bun", "test", "scripts/qa-visual-smoke-gate.test.ts"] },
  { name: "github-issue-triage", command: ["bun", "test", "scripts/github-issue-triage.test.ts"] },
  { name: "khala-sync-runtime-dogfood-evidence", command: ["bun", "test", "scripts/validate-khala-sync-runtime-dogfood-evidence.test.ts"] },
  { name: "ui-velocity-receipt", command: ["bun", "test", "scripts/ui-velocity-receipt.test.ts"] },
]

export const rootComponentTargets: Readonly<Partial<Record<CheckComponent, readonly CheckTarget[]>>> = {
  lint: [{ name: "openagents.com:lint-baseline", command: ["bun", "scripts/lint-baseline.ts"] }],
}

export const fastPolicyTargets: readonly CheckTarget[] = [
  {
    name: "conflict-markers",
    command: ["bun", "run", "--cwd", "apps/openagents.com", "check:conflict-markers"],
  },
  {
    name: "no-github-actions",
    command: ["bun", "run", "--cwd", "apps/openagents.com", "check:no-github-actions"],
  },
  { name: "sol-docs", command: ["bun", "run", "check:sol-docs"] },
  { name: "sol-doc-tests", command: ["bun", "run", "test:sol-docs"] },
]

export const completionTargets: readonly CheckTarget[] = [
  { name: "deploy-contract", command: ["bun", "run", "check:deploy"] },
]

/** Aggregators whose child workspaces are already discovered independently. */
export const componentExclusions: Readonly<Partial<Record<CheckComponent, readonly string[]>>> = {
  test: ["apps/openagents.com", "packages/probe"],
  typecheck: ["apps/openagents.com"],
  lint: ["apps/openagents.com"],
}

/** A workspace may expose a stronger completion script than its ordinary script. */
export const componentOverrides: Readonly<Record<string, Partial<Record<CheckComponent, string>>>> = {
  "apps/openagents-desktop": { test: "verify" },
}
