import { describe, expect, test } from "vite-plus/test";

import { planReleaseImpact } from "./release-impact.js";
import { releaseTargetKeys } from "./release.js";

describe("release impact planner", () => {
  test("never dispatches the Desktop matrix for a web-only change", () => {
    const plan = planReleaseImpact([
      "apps/openagents.com/apps/start/src/routes/-changelog-page.tsx",
      "docs/changelog/2026-07-18-desktop-0.1.0-rc.20.md",
    ]);

    expect(plan.actions).toEqual(["web_deploy"]);
    expect(plan.desktopTargets).toEqual([]);
    expect(plan.requiresDesktopVersionBump).toBe(false);
  });

  test("keeps renderer changes on the complete signed matrix until renderer OTA exists", () => {
    const plan = planReleaseImpact(["apps/openagents-desktop/src/renderer/react-composer.tsx"]);

    expect(plan.actions).toEqual(["desktop_full_matrix"]);
    expect(plan.desktopTargets).toEqual([...releaseTargetKeys]);
    expect(plan.requiresDesktopVersionBump).toBe(true);
    expect(plan.reasons.join(" ")).toContain("signed renderer OTA is not admitted yet");
  });

  test("a shared Desktop dependency and lockfile require all targets", () => {
    const plan = planReleaseImpact(["packages/ui/src/desktop-workbench.css", "pnpm-lock.yaml"]);

    expect(plan.actions).toEqual(["desktop_full_matrix"]);
    expect(plan.desktopTargets).toHaveLength(5);
  });

  test("selects independent mobile and web lanes without a Desktop build", () => {
    const plan = planReleaseImpact([
      "apps/openagents-mobile/src/App.tsx",
      "apps/openagents.com/apps/start/src/routes/index.tsx",
    ]);

    expect(plan.actions).toEqual(["web_deploy", "mobile_ota"]);
    expect(plan.desktopTargets).toEqual([]);
  });

  test("release infrastructure deploys without manufacturing a Desktop release", () => {
    const plan = planReleaseImpact([
      "apps/oa-updates/src/release-set-feed.ts",
      "scripts/release.ts",
    ]);

    expect(plan.actions).toEqual(["updates_service_deploy"]);
    expect(plan.requiresDesktopVersionBump).toBe(false);
  });

  test("documentation-only work has no binary release", () => {
    const plan = planReleaseImpact(["AUTHORITY.md", "docs/sol/release-plan.md"]);

    expect(plan.actions).toEqual(["no_binary_release"]);
    expect(plan.desktopTargets).toEqual([]);
  });
});
