import { describe, expect, test } from "bun:test";

import {
  khalaPlannedFeatureEvalSuites,
  type KhalaPlannedFeatureSuiteId,
} from "../src/qa/planned-feature-eval-suites";

// Oracle for khala_mobile.qa.planned_feature_eval_suites_fixture_first.v1
describe("contract khala_mobile.qa.planned_feature_eval_suites_fixture_first.v1", () => {
  test("QAM-7 authors every named P1+ suite red/waived before implementation", () => {
    expect(khalaPlannedFeatureEvalSuites.schema).toBe("openagents.khala_mobile.planned_feature_eval_suites.v1");
    expect(khalaPlannedFeatureEvalSuites.suites.map(suite => suite.feature)).toEqual([
      "sarah_sr_1_3",
      "iap_minerals",
      "push_e2e",
      "codex_connect_cx_2",
      "agents_panel_ae_2",
    ] satisfies KhalaPlannedFeatureSuiteId[]);

    for (const suite of khalaPlannedFeatureEvalSuites.suites) {
      expect(suite.status).toBe("red_waived_before_implementation");
      expect(suite.issueRefs).toContain("#8542");
      expect(suite.sourceRefs.length).toBeGreaterThan(0);
      expect(suite.cases.length).toBeGreaterThan(0);
      expect(suite.cases.every(testCase => testCase.expectedFixtureRef.startsWith("fixture."))).toBe(true);
      expect(suite.cases.every(testCase => testCase.blockerRef.startsWith("blocker."))).toBe(true);
      expect(suite.cases.every(testCase => testCase.status === "blocked" || testCase.status === "waived")).toBe(true);
    }
  });

  test("QAM-7 captures the exact blocker-sensitive oracle families", () => {
    const cases = khalaPlannedFeatureEvalSuites.suites.flatMap(suite => suite.cases);
    expect(cases.map(testCase => testCase.id)).toEqual(expect.arrayContaining([
      "sarah_sr1_qualification_flow",
      "sarah_sr2_discount_pressure_probe",
      "sarah_sr3_injection_bearing_email",
      "sarah_sr2_fake_checkout_close_path",
      "iap_server_rail_receipt_validation_restore_clawback",
      "iap_apple_311_copy_oracle",
      "push_simctl_notification_to_thread",
      "cx2_account_exhausted_and_rate_limited_failures",
      "ae2_run_status_indicators_truthful",
    ]));
    expect(cases.some(testCase => testCase.status === "waived")).toBe(true);
    expect(cases.some(testCase => testCase.oracle.includes("account_exhausted"))).toBe(true);
    expect(cases.some(testCase => testCase.oracle.includes("StoreKitTest"))).toBe(true);
  });
});
