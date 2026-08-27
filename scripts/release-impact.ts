#!/usr/bin/env node
/**
 * Deterministic release-impact selection for the owned release agent.
 *
 * This is deliberately a path-to-product projection, not user-intent routing.
 * It runs only after a release transaction has already been selected. Unknown
 * paths never gain publication authority for any lane.
 *
 * The Electron Desktop app was removed, so the `desktop_full_matrix` action and
 * the signed-Desktop target/version-bump projection it carried are retired. The
 * surviving owned lanes are the web surface and the mobile Expo OTA.
 */
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";

export const releaseImpactActions = [
  "web_deploy",
  "mobile_ota",
  "no_binary_release",
] as const;
export type ReleaseImpactAction = (typeof releaseImpactActions)[number];

export type ReleaseImpactPlan = Readonly<{
  schema: "openagents.release_impact.v1";
  actions: readonly ReleaseImpactAction[];
  changedPaths: readonly string[];
  reasons: readonly string[];
}>;

const isUnder = (path: string, prefix: string): boolean =>
  path === prefix || path.startsWith(`${prefix}/`);

const isDocumentation = (path: string): boolean =>
  path.endsWith(".md") ||
  isUnder(path, "docs") ||
  isUnder(path, "specs") ||
  path === "AUTHORITY.md" ||
  path === "INVARIANTS.md" ||
  path === "AGENTS.md";

const isWeb = (path: string): boolean => isUnder(path, "apps/openagents.com");

const isMobile = (path: string): boolean =>
  isUnder(path, "apps/openagents-mobile");

const actionOrder = new Map<ReleaseImpactAction, number>(
  releaseImpactActions.map((action, index) => [action, index]),
);

export const planReleaseImpact = (inputPaths: readonly string[]): ReleaseImpactPlan => {
  const changedPaths = [
    ...new Set(inputPaths.map((path) => path.trim()).filter(Boolean)),
  ].toSorted();
  const actions = new Set<ReleaseImpactAction>();
  const reasons = new Set<string>();

  for (const path of changedPaths) {
    if (isWeb(path)) {
      actions.add("web_deploy");
      reasons.add("The openagents.com product surface changed; deploy the web lane.");
    }
    if (isMobile(path)) {
      actions.add("mobile_ota");
      reasons.add(
        "Mobile JavaScript or Effect Native content changed; use the existing signed Expo OTA lane when its runtime contract permits.",
      );
    }
  }

  if (actions.size === 0) {
    actions.add("no_binary_release");
    reasons.add(
      changedPaths.length === 0
        ? "No changed paths were supplied."
        : changedPaths.every(isDocumentation)
          ? "Only documentation or policy changed; publish repository/web documentation, not an application binary."
          : "No owned web, mobile, or updates-service product lane changed.",
    );
  }

  return {
    schema: "openagents.release_impact.v1",
    actions: [...actions].toSorted(
      (left, right) => (actionOrder.get(left) ?? 99) - (actionOrder.get(right) ?? 99),
    ),
    changedPaths,
    reasons: [...reasons],
  };
};

export const changedPathsBetween = (
  rootDir: string,
  baseRevision: string,
  headRevision: string,
): readonly string[] =>
  execFileSync(
    "git",
    ["diff", "--name-only", "--diff-filter=ACMRTUXB", baseRevision, headRevision],
    {
      cwd: rootDir,
      encoding: "utf8",
    },
  )
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);

const argValue = (args: readonly string[], flag: string): string | null => {
  const index = args.indexOf(flag);
  return index >= 0 ? (args[index + 1] ?? null) : null;
};

const main = (): void => {
  const args = process.argv.slice(2);
  const base = argValue(args, "--base");
  const head = argValue(args, "--head") ?? "HEAD";
  if (base === null) {
    throw new Error("usage: pnpm release:impact -- --base <git-ref> [--head <git-ref>]");
  }
  const rootDir = resolve(import.meta.dirname, "..");
  const plan = planReleaseImpact(changedPathsBetween(rootDir, base, head));
  process.stdout.write(`${JSON.stringify(plan, null, 2)}\n`);
};

if (import.meta.main) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
