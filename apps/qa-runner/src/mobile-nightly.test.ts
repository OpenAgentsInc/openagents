import { describe, expect, test } from "bun:test";

import {
  buildKhalaMobileNightlyReport,
  buildKhalaMobileNightlySteps,
  buildKhalaMobileNightlyStrictIssueBody,
  evaluateKhalaMobileConsecutiveNightlyReceipts,
  khalaMobileNightlyPerfBudgetIds,
  renderKhalaMobileNightlyLaunchdPlist,
} from "./mobile-nightly";

// Oracle for khala_mobile.qa.nightly_mobile_row_owned_runner_discipline.v1
describe("contract khala_mobile.qa.nightly_mobile_row_owned_runner_discipline.v1", () => {
  test("QAM-5 row schedules owned-Mac device, visual, monkey, perf, and seam nodes", () => {
    const report = buildKhalaMobileNightlyReport({
      generatedAt: "2026-07-07T23:10:00.000Z",
      qam4StorybookBlocked: true,
    });

    expect(report.schema).toBe("openagents.khala_mobile.qa_nightly_row.v1");
    expect(report.launchd.label).toBe("com.openagents.khala-mobile-nightly");
    expect(report.steps.map(step => step.id)).toEqual([
      "ios-maestro-flows",
      "device-monkey",
      "visual-capture",
      "perf-budgets",
      "seam-probes",
    ]);
    expect(report.steps.every(step => step.ownedRunner === "tailnet-macos-launchd")).toBe(true);
    expect(report.visualTier.storybookV1).toBe("blocked_until_qam4_storybook_device_walk_proven");
    expect(report.qaSwarmProjectionNode.nodeRef).toBe("projection.qa_swarm.mobile.khala_code_nightly");
    expect(report.strictIssueDiscipline.autoFileFailurePath).toBe("required");
  });

  test("QAM-5 carries the required mobile perf budgets and seam probe classifications", () => {
    const report = buildKhalaMobileNightlyReport({
      generatedAt: "2026-07-07T23:10:00.000Z",
    });

    expect(khalaMobileNightlyPerfBudgetIds()).toEqual([
      "budget.khala_mobile.cold_launch.v1",
      "budget.khala_mobile.thread_switch.v1",
      "budget.khala_mobile.sync_bootstrap_to_live.v1",
      "budget.khala_mobile.ota_check_overhead.v1",
    ]);
    expect(report.perfBudgets.map(budget => budget.id)).toEqual([...khalaMobileNightlyPerfBudgetIds()]);
    expect(report.seamProbes).toContainEqual({
      id: "khala_sync_transport_live_classification",
      requiredClassification: "live",
      status: "scheduled",
    });
    expect(report.seamProbes.map(probe => probe.id)).toContain("mobile_session_bearer_bridge");
  });

  test("QAM-5 launchd definition stays owned-runner/local and excludes hosted CI/EAS", () => {
    const plist = renderKhalaMobileNightlyLaunchdPlist({
      openagentsCheckout: "/opt/openagents",
    });

    expect(plist).toContain("com.openagents.khala-mobile-nightly");
    expect(plist).toContain("OA_QA_NIGHTLY_INCLUDE_MOBILE=1");
    expect(plist).not.toContain(".github/workflows");
    expect(plist).not.toContain("eas");
    expect(plist).not.toContain("github.com");
  });

  test("QAM-5 strict issue body records failure evidence without private material", () => {
    const body = buildKhalaMobileNightlyStrictIssueBody({
      failedStepId: "seam-probes",
      reportRef: "artifact.khala_mobile.nightly.report.2026-07-07",
      seedRef: "trace.khala_mobile.seam.khala_sync_transport_live",
    });

    expect(body).toContain("### Affected surface");
    expect(body).toContain("seam-probes");
    expect(body).toContain("trace.khala_mobile.seam.khala_sync_transport_live");
    expect(body).not.toContain("/Users/");
    expect(body).not.toContain("bearer");
  });

  test("QAM-5 exit stays false until seven consecutive passed receipts exist", () => {
    expect(evaluateKhalaMobileConsecutiveNightlyReceipts([
      { generatedAt: "2026-07-01T02:30:00.000Z", receiptRef: "artifact.one", verdict: "passed" },
      { generatedAt: "2026-07-02T02:30:00.000Z", receiptRef: "artifact.two", verdict: "failed" },
      { generatedAt: "2026-07-03T02:30:00.000Z", receiptRef: "artifact.three", verdict: "passed" },
    ])).toMatchObject({
      consecutivePasses: 1,
      exitSatisfied: false,
    });
    expect(evaluateKhalaMobileConsecutiveNightlyReceipts(
      Array.from({ length: 7 }, (_, index) => ({
        generatedAt: `2026-07-${String(index + 1).padStart(2, "0")}T02:30:00.000Z`,
        receiptRef: `artifact.${index}`,
        verdict: "passed" as const,
      })),
    )).toMatchObject({
      consecutivePasses: 7,
      exitSatisfied: true,
      latestReceiptRef: "artifact.6",
    });
  });

  test("QAM-5 commands are public-safe scheduled commands", () => {
    const commands = buildKhalaMobileNightlySteps().map(step => step.command.join(" "));
    expect(commands.join("\n")).not.toContain("/Users/");
    expect(commands.join("\n")).not.toContain("token");
    expect(commands.find(command => command.includes("maestro test"))).toBeDefined();
  });
});
