import { describe, expect, test } from "vite-plus/test";

import { planReleaseImpact } from "./release-impact.js";

describe("release impact planner", () => {
  test("selects the web lane alone for a web-only change", () => {
    const plan = planReleaseImpact([
      "apps/openagents.com/apps/start/src/routes/-changelog-page.tsx",
      "docs/changelog/2026-07-18-desktop-0.1.0-rc.20.md",
    ]);

    expect(plan.actions).toEqual(["web_deploy"]);
  });

  test("selects independent mobile and web lanes", () => {
    const plan = planReleaseImpact([
      "apps/openagents-mobile/src/App.tsx",
      "apps/openagents.com/apps/start/src/routes/index.tsx",
    ]);

    expect(plan.actions).toEqual(["web_deploy", "mobile_ota"]);
  });

  test("shared UI or lockfile churn no longer manufactures a binary release", () => {
    const plan = planReleaseImpact(["packages/ui/src/workbench.css", "pnpm-lock.yaml"]);

    expect(plan.actions).toEqual(["no_binary_release"]);
  });

  test("documentation-only work has no binary release", () => {
    const plan = planReleaseImpact(["AUTHORITY.md", "docs/sol/release-plan.md"]);

    expect(plan.actions).toEqual(["no_binary_release"]);
  });
});
