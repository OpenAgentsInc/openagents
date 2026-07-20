#!/usr/bin/env node
/**
 * Deterministic release-impact selection for the owned release agent.
 *
 * This is deliberately a path-to-product projection, not user-intent routing.
 * It runs only after a release transaction has already been selected. Unknown
 * paths never gain Desktop publication authority and Desktop renderer changes
 * remain full-matrix until the signed renderer-OTA contract is implemented.
 */
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";

import { releaseTargetKeys, type ReleaseTargetKey } from "./release.js";

export const releaseImpactActions = [
  "desktop_full_matrix",
  "web_deploy",
  "mobile_ota",
  "updates_service_deploy",
  "no_binary_release",
] as const;
export type ReleaseImpactAction = (typeof releaseImpactActions)[number];

export type ReleaseImpactPlan = Readonly<{
  schema: "openagents.release_impact.v1";
  actions: readonly ReleaseImpactAction[];
  changedPaths: readonly string[];
  desktopTargets: readonly ReleaseTargetKey[];
  reasons: readonly string[];
  requiresDesktopVersionBump: boolean;
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

const isReleaseInfrastructure = (path: string): boolean =>
  isUnder(path, "apps/oa-updates") ||
  path.startsWith("scripts/release") ||
  path.startsWith("scripts/github-release") ||
  path.startsWith("scripts/changelog") ||
  path.startsWith("scripts/check-authority-delegation");

const isDesktopRuntime = (path: string): boolean =>
  isUnder(path, "apps/openagents-desktop") ||
  isUnder(path, "crates/oa-desktop-audio") ||
  isUnder(path, "packages/agent-runtime-schema") ||
  isUnder(path, "packages/codex-app-server-protocol") ||
  isUnder(path, "packages/runtime-platform") ||
  isUnder(path, "packages/ui") ||
  isUnder(path, "packages/effect-native") ||
  path === "pnpm-lock.yaml";

const isDesktopRenderer = (path: string): boolean =>
  isUnder(path, "apps/openagents-desktop/src/renderer") ||
  path === "apps/openagents-desktop/src/desktop-renderer-location.ts" ||
  isUnder(path, "packages/ui");

const isWeb = (path: string): boolean => isUnder(path, "apps/openagents.com");

const isMobile = (path: string): boolean =>
  isUnder(path, "apps/openagents-mobile") || isUnder(path, "effect-native");

const actionOrder = new Map<ReleaseImpactAction, number>(
  releaseImpactActions.map((action, index) => [action, index]),
);

export const planReleaseImpact = (inputPaths: readonly string[]): ReleaseImpactPlan => {
  const changedPaths = [
    ...new Set(inputPaths.map((path) => path.trim()).filter(Boolean)),
  ].toSorted();
  const actions = new Set<ReleaseImpactAction>();
  const reasons = new Set<string>();
  let requiresDesktopVersionBump = false;

  for (const path of changedPaths) {
    if (isDesktopRuntime(path)) {
      actions.add("desktop_full_matrix");
      requiresDesktopVersionBump = true;
      reasons.add(
        isDesktopRenderer(path)
          ? "Desktop renderer bytes changed; signed renderer OTA is not admitted yet, so all four required Desktop targets are required."
          : "Desktop host, runtime, dependency, or shared Desktop closure changed; all four required Desktop targets are required.",
      );
    }
    if (isWeb(path)) {
      actions.add("web_deploy");
      reasons.add(
        "The openagents.com product surface changed; deploy the web lane without a Desktop build.",
      );
    }
    if (isMobile(path)) {
      actions.add("mobile_ota");
      reasons.add(
        "Mobile JavaScript or Effect Native content changed; use the existing signed Expo OTA lane when its runtime contract permits.",
      );
    }
    if (isReleaseInfrastructure(path)) {
      actions.add("updates_service_deploy");
      reasons.add(
        "Release/update infrastructure changed; verify and deploy that service without manufacturing a Desktop version.",
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
          : "No owned Desktop, web, mobile, or updates-service product lane changed.",
    );
  }

  return {
    schema: "openagents.release_impact.v1",
    actions: [...actions].toSorted(
      (left, right) => (actionOrder.get(left) ?? 99) - (actionOrder.get(right) ?? 99),
    ),
    changedPaths,
    desktopTargets: actions.has("desktop_full_matrix") ? [...releaseTargetKeys] : [],
    reasons: [...reasons],
    requiresDesktopVersionBump,
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
