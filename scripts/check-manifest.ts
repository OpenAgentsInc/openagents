export type CheckComponent = "test" | "typecheck" | "lint" | "fmt";

export interface CheckTarget {
  readonly name: string;
  readonly command: readonly string[];
}

/**
 * Repository-owned checks that cannot be inferred from workspace package.json
 * scripts. Package test/typecheck/lint/fmt targets are discovered automatically.
 */
export const rootTestTargets: readonly CheckTarget[] = [
  {
    name: "workspace-check",
    command: ["pnpm", "exec", "vp", "test", "--run", "scripts/check-workspace.test.ts"],
  },
  {
    name: "lint-baseline",
    command: ["pnpm", "exec", "vp", "test", "--run", "scripts/lint-baseline.test.ts"],
  },
  {
    name: "node-vp-inventory",
    command: ["node", "--test", "scripts/node-vp-cutover-inventory.test.mjs"],
  },
  {
    name: "zero-supported-bun",
    command: ["node", "--test", "scripts/zero-supported-bun-guard.test.mjs"],
  },
  {
    name: "sol-docs",
    command: ["pnpm", "exec", "vp", "test", "--run", "scripts/check-sol-docs.test.ts"],
  },
  { name: "ste", command: ["pnpm", "exec", "vp", "test", "--run", "scripts/check-ste.test.ts"] },
  {
    name: "qa-pre-push-smoke",
    command: ["pnpm", "exec", "vp", "test", "--run", "scripts/qa-pre-push-smoke.test.ts"],
  },
  {
    name: "qa-async-gce-trigger",
    command: ["pnpm", "exec", "vp", "test", "--run", "scripts/qa-async-gce-trigger.test.ts"],
  },
  {
    name: "qa-nightly-matrix",
    command: ["pnpm", "exec", "vp", "test", "--run", "scripts/qa-nightly-matrix.test.ts"],
  },
  {
    name: "qa-visual-smoke-gate",
    command: ["pnpm", "exec", "vp", "test", "--run", "scripts/qa-visual-smoke-gate.test.ts"],
  },
  {
    name: "github-issue-triage",
    command: ["pnpm", "exec", "vp", "test", "--run", "scripts/github-issue-triage.test.ts"],
  },
  {
    name: "khala-sync-runtime-dogfood-evidence",
    command: [
      "pnpm",
      "exec",
      "vp",
      "test",
      "--run",
      "scripts/validate-khala-sync-runtime-dogfood-evidence.test.ts",
    ],
  },
  {
    name: "ui-velocity-receipt",
    command: ["pnpm", "exec", "vp", "test", "--run", "scripts/ui-velocity-receipt.test.ts"],
  },
];

export const rootComponentTargets: Readonly<
  Partial<Record<CheckComponent, readonly CheckTarget[]>>
> = {
  lint: [
    {
      name: "openagents.com:lint-baseline",
      command: ["node", "--import", "tsx", "scripts/lint-baseline.ts"],
    },
  ],
};

export const fastPolicyTargets: readonly CheckTarget[] = [
  {
    name: "conflict-markers",
    command: ["pnpm", "--dir", "apps/openagents.com", "run", "check:conflict-markers"],
  },
  {
    name: "no-github-actions",
    command: ["pnpm", "--dir", "apps/openagents.com", "run", "check:no-github-actions"],
  },
  { name: "sol-docs", command: ["pnpm", "run", "check:sol-docs"] },
  { name: "sol-doc-tests", command: ["pnpm", "run", "test:sol-docs"] },
  { name: "ste", command: ["pnpm", "run", "check:ste:all"] },
  { name: "ste-control-semantics", command: ["pnpm", "run", "check:ste-control-semantics"] },
  { name: "ste-tests", command: ["pnpm", "run", "test:ste"] },
  { name: "node-vp-freeze", command: ["node", "scripts/node-vp-cutover-inventory.mjs", "--check"] },
  { name: "zero-supported-bun", command: ["node", "scripts/zero-supported-bun-guard.mjs", "."] },
  {
    name: "vp1-retirement",
    command: ["node", "scripts/vp1-retired-money-surface-guard.mjs", "."],
  },
];

export const completionTargets: readonly CheckTarget[] = [
  { name: "deploy-contract", command: ["pnpm", "run", "check:deploy"] },
];

/** Aggregators whose child workspaces are already discovered independently. */
export const componentExclusions: Readonly<Partial<Record<CheckComponent, readonly string[]>>> = {
  test: ["apps/openagents.com", "packages/probe"],
  typecheck: ["apps/openagents.com"],
  lint: ["apps/openagents.com"],
};

/** A workspace may expose a stronger completion script than its ordinary script. */
export const componentOverrides: Readonly<Record<string, Partial<Record<CheckComponent, string>>>> =
  {
    "apps/openagents-desktop": { test: "verify" },
  };
