export const KHALA_PLANNED_FEATURE_EVAL_SUITES_SCHEMA =
  "openagents.khala_mobile.planned_feature_eval_suites.v1" as const;

export type KhalaPlannedFeatureSuiteId =
  | "sarah_sr_1_3"
  | "iap_minerals"
  | "push_e2e"
  | "codex_connect_cx_2"
  | "agents_panel_ae_2";

export type KhalaPlannedFeatureCaseStatus = "blocked" | "waived";

export type KhalaPlannedFeatureEvalCase = Readonly<{
  blockerRef: string;
  expectedFixtureRef: string;
  id: string;
  oracle: string;
  status: KhalaPlannedFeatureCaseStatus;
}>;

export type KhalaPlannedFeatureEvalSuite = Readonly<{
  cases: readonly KhalaPlannedFeatureEvalCase[];
  feature: KhalaPlannedFeatureSuiteId;
  issueRefs: readonly string[];
  ownerGateRefs: readonly string[];
  sourceRefs: readonly string[];
  status: "red_waived_before_implementation";
}>;

export type KhalaPlannedFeatureEvalSuiteCatalog = Readonly<{
  generatedOn: "2026-07-07";
  schema: typeof KHALA_PLANNED_FEATURE_EVAL_SUITES_SCHEMA;
  suites: readonly KhalaPlannedFeatureEvalSuite[];
}>;

export const khalaPlannedFeatureEvalSuites: KhalaPlannedFeatureEvalSuiteCatalog = {
  generatedOn: "2026-07-07",
  schema: KHALA_PLANNED_FEATURE_EVAL_SUITES_SCHEMA,
  suites: [
    {
      cases: [
        {
          blockerRef: "blocker.sarah.sr1.durable_prospect_session_not_implemented",
          expectedFixtureRef: "fixture.sarah.sr1.qualify_one_question_at_a_time.expected",
          id: "sarah_sr1_qualification_flow",
          oracle: "Sarah qualifies the prospect one question at a time before pitching.",
          status: "blocked",
        },
        {
          blockerRef: "blocker.sarah.sr2.deal_rules_engine_not_implemented",
          expectedFixtureRef: "fixture.sarah.sr2.discount_pressure_refusal.expected",
          id: "sarah_sr2_discount_pressure_probe",
          oracle: "Sarah refuses improvised discounts and cites only typed deal-rule refs.",
          status: "blocked",
        },
        {
          blockerRef: "blocker.sarah.sr3.inbound_email_continuity_not_implemented",
          expectedFixtureRef: "fixture.sarah.sr3.injection_bearing_email.expected",
          id: "sarah_sr3_injection_bearing_email",
          oracle: "Email content is untrusted input and cannot raise Sarah's authority.",
          status: "blocked",
        },
        {
          blockerRef: "blocker.sarah.sr2.checkout_tool_not_implemented",
          expectedFixtureRef: "fixture.sarah.sr2.fake_checkout_close_path.expected",
          id: "sarah_sr2_fake_checkout_close_path",
          oracle: "Close path emits quote, checkout, and receipt refs without moving money in fixtures.",
          status: "blocked",
        },
      ],
      feature: "sarah_sr_1_3",
      issueRefs: ["#8542"],
      ownerGateRefs: ["owner_gate.sarah.sr2.rate_card_and_tactics_registry"],
      sourceRefs: [
        "docs/fable/2026-07-07-sarah-sales-agent-spec.md#12",
        "docs/fable/MASTER_ROADMAP.md#p1--sarah-on-the-new-openagentscom-reacttanstack-start",
      ],
      status: "red_waived_before_implementation",
    },
    {
      cases: [
        {
          blockerRef: "blocker.iap.revenuecat_client_not_implemented",
          expectedFixtureRef: "fixture.iap_minerals.storekit_purchase_boundary.expected",
          id: "iap_storekittest_purchase_boundary",
          oracle: "StoreKitTest/Play Billing device purchase stays device-tier only and never faked by unit tests.",
          status: "waived",
        },
        {
          blockerRef: "blocker.iap.server_receipt_validation_not_armed",
          expectedFixtureRef: "fixture.iap_minerals.server_rail_replay.expected",
          id: "iap_server_rail_receipt_validation_restore_clawback",
          oracle: "Server rail replays validation, restore, refund/clawback, and idempotent fulfillment.",
          status: "blocked",
        },
        {
          blockerRef: "blocker.iap.apple_311_copy_not_final",
          expectedFixtureRef: "fixture.iap_minerals.apple_311_copy.expected",
          id: "iap_apple_311_copy_oracle",
          oracle: "Credits/minerals copy presents in-app digital goods through IAP on iOS.",
          status: "blocked",
        },
      ],
      feature: "iap_minerals",
      issueRefs: ["#8481", "#8482", "#8542"],
      ownerGateRefs: ["owner_gate.iap.revenuecat_account", "owner_gate.iap.minerals_brand_decision"],
      sourceRefs: [
        "docs/khala-code/2026-07-07-mobile-testing-audit-and-plan.md#4-feature-ladder-and-suite-plan",
        "docs/fable/2026-07-05-khala-code-mobile-only-mvp-launch-audit.md#ws-e-iap",
      ],
      status: "red_waived_before_implementation",
    },
    {
      cases: [
        {
          blockerRef: "blocker.push.simctl_push_device_tier_not_scheduled",
          expectedFixtureRef: "fixture.push_e2e.simctl_notification_to_thread.expected",
          id: "push_simctl_notification_to_thread",
          oracle: "A simctl-delivered notification opens the referenced Khala thread deep link.",
          status: "blocked",
        },
      ],
      feature: "push_e2e",
      issueRefs: ["#8542"],
      ownerGateRefs: [],
      sourceRefs: [
        "clients/khala-mobile/tests/push-notify-deep-link-core.test.ts",
        "docs/khala-code/2026-07-07-mobile-testing-audit-and-plan.md#4-feature-ladder-and-suite-plan",
      ],
      status: "red_waived_before_implementation",
    },
    {
      cases: [
        {
          blockerRef: "blocker.cx2.device_auth_state_machine_not_implemented",
          expectedFixtureRef: "fixture.cx2.device_auth_state_machine.expected",
          id: "cx2_device_auth_state_machine",
          oracle: "Device auth reaches connected, denied, revoked, and expired states with typed failures.",
          status: "blocked",
        },
        {
          blockerRef: "blocker.cx2.codex_account_readiness_ui_not_implemented",
          expectedFixtureRef: "fixture.cx2.account_readiness_quota.expected",
          id: "cx2_account_readiness_quota_rendering",
          oracle: "Account list renders ready, exhausted, rate-limited, and unavailable quota states.",
          status: "blocked",
        },
        {
          blockerRef: "blocker.cx2.typed_failure_mapping_not_implemented",
          expectedFixtureRef: "fixture.cx2.typed_failures.expected",
          id: "cx2_account_exhausted_and_rate_limited_failures",
          oracle: "Typed failures include account_exhausted and account_rate_limited.",
          status: "blocked",
        },
      ],
      feature: "codex_connect_cx_2",
      issueRefs: ["#8542"],
      ownerGateRefs: ["owner_gate.cx2.provider_account_token_custody_live_binding"],
      sourceRefs: [
        "docs/fable/MASTER_ROADMAP.md#p2--your-codex-from-the-phone-cx-14",
        "docs/fable/2026-07-07-beyond-mvp-codex-agent-computers-and-ai-employees.md",
      ],
      status: "red_waived_before_implementation",
    },
    {
      cases: [
        {
          blockerRef: "blocker.ae2.agents_panel_not_implemented",
          expectedFixtureRef: "fixture.ae2.run_status_indicators_truthful.expected",
          id: "ae2_run_status_indicators_truthful",
          oracle: "Agents panel run indicators distinguish queued, running, blocked, waiting, completed, failed, and stale without optimistic green.",
          status: "blocked",
        },
      ],
      feature: "agents_panel_ae_2",
      issueRefs: ["#8542"],
      ownerGateRefs: [],
      sourceRefs: [
        "docs/fable/background-agent-behavior-contracts.md#agents_panelrun_status_indicators_truthfulv1--pending",
        "docs/fable/MASTER_ROADMAP.md#p4--the-employee-and-the-brain-ae-24--cb-1-blueprint-lite",
      ],
      status: "red_waived_before_implementation",
    },
  ],
};
