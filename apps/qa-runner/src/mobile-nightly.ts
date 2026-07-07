export const KHALA_MOBILE_NIGHTLY_ROW_SCHEMA =
  "openagents.khala_mobile.qa_nightly_row.v1" as const;

export type KhalaMobileNightlyVerdict = "passed" | "failed" | "blocked" | "inconclusive";

export type KhalaMobileNightlyStepId =
  | "ios-maestro-flows"
  | "device-monkey"
  | "visual-capture"
  | "perf-budgets"
  | "seam-probes";

export type KhalaMobileNightlyStep = Readonly<{
  command: readonly string[];
  id: KhalaMobileNightlyStepId;
  label: string;
  ownedRunner: "tailnet-macos-launchd";
  requiredArtifactRefs: readonly string[];
  verdict: KhalaMobileNightlyVerdict;
}>;

export type KhalaMobileNightlyPerfBudgetId =
  | "budget.khala_mobile.cold_launch.v1"
  | "budget.khala_mobile.thread_switch.v1"
  | "budget.khala_mobile.sync_bootstrap_to_live.v1"
  | "budget.khala_mobile.ota_check_overhead.v1";

export type KhalaMobileNightlySeamProbeId =
  | "khala_sync_transport_live_classification"
  | "mobile_session_bearer_bridge";

export type KhalaMobileNightlyReport = Readonly<{
  generatedAt: string;
  issueRefs: readonly string[];
  launchd: {
    label: "com.openagents.khala-mobile-nightly";
    plistRef: "docs/khala-code/receipts/qam-5/com.openagents.khala-mobile-nightly.plist";
    schedule: "owned_tailnet_mac_local_02_30";
  };
  mobileCoverageLedgerRef: string;
  perfBudgets: ReadonlyArray<{
    id: KhalaMobileNightlyPerfBudgetId;
    status: "scheduled";
  }>;
  qaSwarmProjectionNode: {
    board: "qa-swarm";
    nodeRef: "projection.qa_swarm.mobile.khala_code_nightly";
    status: KhalaMobileNightlyVerdict;
  };
  schema: typeof KHALA_MOBILE_NIGHTLY_ROW_SCHEMA;
  seamProbes: ReadonlyArray<{
    id: KhalaMobileNightlySeamProbeId;
    requiredClassification?: "live";
    status: "scheduled";
  }>;
  steps: readonly KhalaMobileNightlyStep[];
  strictIssueDiscipline: {
    autoFileFailurePath: "required";
    issueTemplate: "openagents.khala_mobile.qa_nightly_strict_issue.v1";
  };
  verdict: KhalaMobileNightlyVerdict;
  visualTier: {
    qam4Dependency: "#8539";
    storybookV1: "blocked_until_qam4_storybook_device_walk_proven" | "scheduled";
    v2Checkpoints: "scheduled";
  };
}>;

export type KhalaMobileNightlyReceiptSummary = Readonly<{
  generatedAt: string;
  receiptRef: string;
  verdict: KhalaMobileNightlyVerdict;
}>;

const PRIVATE_MATERIAL_PATTERN =
  /\/Users\/|\/home\/|bearer[_ -]?token|access[_-]?token|api[_-]?key|secret|ghp_|gho_|sk-[a-z0-9]/i;

export const khalaMobileNightlyPerfBudgetIds = (): readonly KhalaMobileNightlyPerfBudgetId[] => [
  "budget.khala_mobile.cold_launch.v1",
  "budget.khala_mobile.thread_switch.v1",
  "budget.khala_mobile.sync_bootstrap_to_live.v1",
  "budget.khala_mobile.ota_check_overhead.v1",
];

export const buildKhalaMobileNightlySteps = (): readonly KhalaMobileNightlyStep[] => [
  {
    command: ["maestro", "test", "clients/khala-mobile/.maestro"],
    id: "ios-maestro-flows",
    label: "Khala Mobile iOS simulator Maestro flows",
    ownedRunner: "tailnet-macos-launchd",
    requiredArtifactRefs: ["artifact.khala_mobile.maestro.ios_flows"],
    verdict: "inconclusive",
  },
  {
    command: ["bun", "run", "--cwd", "packages/khala-qa-harness", "mobile:device-monkey"],
    id: "device-monkey",
    label: "Seeded device monkey with screenshot-on-crash and memory oracle",
    ownedRunner: "tailnet-macos-launchd",
    requiredArtifactRefs: [
      "artifact.khala_mobile.device_monkey.report",
      "artifact.khala_mobile.device_monkey.coverage_ledger",
      "artifact.khala_mobile.device_monkey.memory_oracle",
    ],
    verdict: "inconclusive",
  },
  {
    command: ["bun", "run", "--cwd", "packages/khala-qa-harness", "mobile:visual-capture"],
    id: "visual-capture",
    label: "QAM-4 mobile visual capture and baseline comparison",
    ownedRunner: "tailnet-macos-launchd",
    requiredArtifactRefs: [
      "artifact.khala_mobile.visual.storybook_v1",
      "artifact.khala_mobile.visual.maestro_v2",
    ],
    verdict: "blocked",
  },
  {
    command: ["bun", "run", "--cwd", "clients/khala-mobile", "qa:mobile:perf-budgets"],
    id: "perf-budgets",
    label: "Khala Mobile named perf budgets",
    ownedRunner: "tailnet-macos-launchd",
    requiredArtifactRefs: khalaMobileNightlyPerfBudgetIds().map(id => `perf.${id}`),
    verdict: "inconclusive",
  },
  {
    command: ["bun", "run", "--cwd", "apps/qa-runner", "khala-sync-once", "--mobile-nightly"],
    id: "seam-probes",
    label: "R6 khala-sync transport live classification and mobile-session bearer probe",
    ownedRunner: "tailnet-macos-launchd",
    requiredArtifactRefs: [
      "trace.khala_mobile.seam.khala_sync_transport_live",
      "trace.khala_mobile.seam.mobile_session_bearer_bridge",
    ],
    verdict: "inconclusive",
  },
];

export const buildKhalaMobileNightlyReport = (input: Readonly<{
  generatedAt: string;
  qam4StorybookBlocked?: boolean;
  runRef?: string;
}>): KhalaMobileNightlyReport => {
  const steps = buildKhalaMobileNightlySteps();
  const qam4Blocked = input.qam4StorybookBlocked ?? true;
  const verdict: KhalaMobileNightlyVerdict = qam4Blocked ? "blocked" : "inconclusive";
  return assertKhalaMobileNightlyPublicSafe({
    generatedAt: input.generatedAt,
    issueRefs: ["#8540", "#8539"],
    launchd: {
      label: "com.openagents.khala-mobile-nightly",
      plistRef: "docs/khala-code/receipts/qam-5/com.openagents.khala-mobile-nightly.plist",
      schedule: "owned_tailnet_mac_local_02_30",
    },
    mobileCoverageLedgerRef: `coverage.khala_mobile.nightly.${input.runRef ?? "pending-owned-runner"}`,
    perfBudgets: khalaMobileNightlyPerfBudgetIds().map(id => ({ id, status: "scheduled" })),
    qaSwarmProjectionNode: {
      board: "qa-swarm",
      nodeRef: "projection.qa_swarm.mobile.khala_code_nightly",
      status: verdict,
    },
    schema: KHALA_MOBILE_NIGHTLY_ROW_SCHEMA,
    seamProbes: [
      {
        id: "khala_sync_transport_live_classification",
        requiredClassification: "live",
        status: "scheduled",
      },
      {
        id: "mobile_session_bearer_bridge",
        status: "scheduled",
      },
    ],
    steps,
    strictIssueDiscipline: {
      autoFileFailurePath: "required",
      issueTemplate: "openagents.khala_mobile.qa_nightly_strict_issue.v1",
    },
    verdict,
    visualTier: {
      qam4Dependency: "#8539",
      storybookV1: qam4Blocked ? "blocked_until_qam4_storybook_device_walk_proven" : "scheduled",
      v2Checkpoints: "scheduled",
    },
  });
};

export const renderKhalaMobileNightlyLaunchdPlist = (input: Readonly<{
  openagentsCheckout: string;
}>): string => `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.openagents.khala-mobile-nightly</string>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/zsh</string>
    <string>-lc</string>
    <string>cd ${input.openagentsCheckout} &amp;&amp; OA_QA_NIGHTLY_INCLUDE_MOBILE=1 bun scripts/qa-nightly-matrix.ts</string>
  </array>
  <key>StartCalendarInterval</key>
  <dict>
    <key>Hour</key>
    <integer>2</integer>
    <key>Minute</key>
    <integer>30</integer>
  </dict>
  <key>StandardOutPath</key>
  <string>${input.openagentsCheckout}/var/qa-nightly/khala-mobile-nightly.out.log</string>
  <key>StandardErrorPath</key>
  <string>${input.openagentsCheckout}/var/qa-nightly/khala-mobile-nightly.err.log</string>
</dict>
</plist>
`;

export const buildKhalaMobileNightlyStrictIssueBody = (input: Readonly<{
  failedStepId: KhalaMobileNightlyStepId;
  reportRef: string;
  seedRef: string;
}>): string => `### Affected surface

Khala Mobile nightly row

### Failing check

\`${input.failedStepId}\`

### Evidence

- Report: \`${input.reportRef}\`
- Seed/log/artifact ref: \`${input.seedRef}\`

### Required discipline

Distill the failure into a committed regression test or keep the nightly row blocked with a named issue reference.
`;

export const evaluateKhalaMobileConsecutiveNightlyReceipts = (
  receipts: readonly KhalaMobileNightlyReceiptSummary[],
): Readonly<{
  consecutivePasses: number;
  exitSatisfied: boolean;
  latestReceiptRef?: string;
}> => {
  const sortedReceipts = [...receipts].sort((left, right) => left.generatedAt.localeCompare(right.generatedAt));
  let consecutivePasses = 0;
  for (const receipt of sortedReceipts.reverse()) {
    if (receipt.verdict !== "passed") break;
    consecutivePasses += 1;
  }
  return {
    consecutivePasses,
    exitSatisfied: consecutivePasses >= 7,
    ...(sortedReceipts[0]?.receiptRef === undefined ? {} : { latestReceiptRef: sortedReceipts[0].receiptRef }),
  };
};

export const assertKhalaMobileNightlyPublicSafe = <T>(value: T): T => {
  const serialized = JSON.stringify(value);
  if (PRIVATE_MATERIAL_PATTERN.test(serialized)) {
    throw new Error("Khala Mobile nightly artifact contains private material");
  }
  return value;
};
