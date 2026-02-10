import { dseCompileReportPageTemplate } from "../../effuse-pages/dseCompileReport";
import { dseOpsRunDetailPageTemplate } from "../../effuse-pages/dseOpsRunDetail";
import { dseOpsRunsPageTemplate } from "../../effuse-pages/dseOpsRuns";
import { dseSignaturePageTemplate } from "../../effuse-pages/dseSignature";

import type { Story } from "../types";

export const dseVizStories: ReadonlyArray<Story> = [
  {
    id: "dse-ops-runs",
    title: "DSE/Ops runs list",
    kind: "organism",
    render: () =>
      dseOpsRunsPageTemplate({
        errorText: null,
        runs: [
          {
            runId: "overnight_2026-02-10T09:00:00Z",
            status: "finished",
            startedAtMs: Date.now() - 60_000 * 45,
            endedAtMs: Date.now() - 60_000 * 5,
            commitSha: "abc123def456",
            baseUrl: "https://openagents.com",
            actorUserId: "user_dse_admin",
            signatureIds: ["@openagents/autopilot/blueprint/SelectTool.v1", "@openagents/autopilot/canary/RecapThread.v1"],
            updatedAtMs: Date.now() - 60_000 * 5,
            createdAtMs: Date.now() - 60_000 * 45,
          },
          {
            runId: "overnight_2026-02-10T10:00:00Z",
            status: "running",
            startedAtMs: Date.now() - 60_000 * 3,
            endedAtMs: null,
            commitSha: "abc123def456",
            baseUrl: "http://localhost:5173",
            actorUserId: "user_dse_admin",
            signatureIds: ["@openagents/autopilot/blueprint/SelectTool.v1"],
            updatedAtMs: Date.now() - 5_000,
            createdAtMs: Date.now() - 60_000 * 3,
          },
        ],
      }),
  },
  {
    id: "dse-ops-run-detail",
    title: "DSE/Ops run detail",
    kind: "organism",
    render: () =>
      dseOpsRunDetailPageTemplate({
        runId: "overnight_2026-02-10T09:00:00Z",
        errorText: null,
        run: {
          runId: "overnight_2026-02-10T09:00:00Z",
          status: "finished",
          startedAtMs: Date.now() - 60_000 * 45,
          endedAtMs: Date.now() - 60_000 * 5,
          commitSha: "abc123def456",
          baseUrl: "https://openagents.com",
          actorUserId: "user_dse_admin",
          signatureIds: ["@openagents/autopilot/blueprint/SelectTool.v1"],
          notes: "Phase 5 prod run: compile -> canary -> promote.",
          linksJson: JSON.stringify({ reportId: "cr_01", canary: "enabled" }, null, 2),
          summaryJson: JSON.stringify({ status: "finished", promoted: true }, null, 2),
          updatedAtMs: Date.now() - 60_000 * 5,
        },
        events: [
          {
            tsMs: Date.now() - 60_000 * 44,
            level: "info",
            phase: "phase5.start",
            message: "starting prod run",
            jsonPreview: null,
          },
          {
            tsMs: Date.now() - 60_000 * 20,
            level: "info",
            phase: "phase5.canary",
            message: "canary okCount=20 errorCount=0",
            jsonPreview: JSON.stringify({ okCount: 20, errorCount: 0 }, null, 2),
          },
          {
            tsMs: Date.now() - 60_000 * 5,
            level: "info",
            phase: "phase5.finish",
            message: "promoted compiled_id=c_selecttool_v2",
            jsonPreview: JSON.stringify({ compiled_id: "c_selecttool_v2" }, null, 2),
          },
        ],
      }),
  },
  {
    id: "dse-signature-detail",
    title: "DSE/Signature detail",
    kind: "organism",
    render: () =>
      dseSignaturePageTemplate({
        signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
        errorText: null,
        active: { compiled_id: "c_selecttool_v2", updatedAtMs: Date.now() - 60_000 * 5 },
        activeHistory: [
          {
            action: "set",
            fromCompiledId: "c_selecttool_v1",
            toCompiledId: "c_selecttool_v2",
            reason: "compile improved holdout reward",
            actorUserId: "user_dse_admin",
            createdAtMs: Date.now() - 60_000 * 5,
          },
        ],
        canary: {
          enabled: true,
          control_compiled_id: "c_selecttool_v1",
          canary_compiled_id: "c_selecttool_v2",
          rolloutPct: 10,
          okCount: 20,
          errorCount: 0,
          minSamples: 20,
          maxErrorRate: 0.2,
          updatedAtMs: Date.now() - 60_000,
        },
        canaryHistory: [
          {
            action: "start",
            control_compiled_id: "c_selecttool_v1",
            canary_compiled_id: "c_selecttool_v2",
            rolloutPct: 10,
            okCount: 0,
            errorCount: 0,
            reason: "phase5",
            actorUserId: "user_dse_admin",
            createdAtMs: Date.now() - 60_000 * 15,
          },
        ],
        compileReports: [
          {
            jobHash: "job_selecttool_instruction_grid_v1",
            datasetHash: "sha256:deadbeef",
            compiled_id: "c_selecttool_v2",
            createdAtMs: Date.now() - 60_000 * 30,
          },
        ],
        examples: [
          {
            exampleId: "selecttool_001",
            split: "holdout",
            tags: ["overnight", "selecttool", "v1"],
            inputJson: JSON.stringify({ userMessage: "list repo tree" }, null, 2),
            expectedJson: JSON.stringify({ toolName: "github.getRepoTree" }, null, 2),
          },
        ],
        receipts: [
          {
            receiptId: "rcpt_01",
            compiled_id: "c_selecttool_v2",
            createdAtMs: Date.now() - 60_000 * 2,
            strategyId: "direct.v1",
            resultTag: "Ok",
            rlmTraceBlobId: null,
            rlmTraceEventCount: null,
          },
        ],
      }),
  },
  {
    id: "dse-compile-report-detail",
    title: "DSE/Compile report detail",
    kind: "organism",
    render: () =>
      dseCompileReportPageTemplate({
        signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
        jobHash: "job_selecttool_instruction_grid_v1",
        datasetHash: "sha256:deadbeef",
        errorText: null,
        report: {
          signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
          jobHash: "job_selecttool_instruction_grid_v1",
          datasetHash: "sha256:deadbeef",
          datasetId: "dataset_selecttool_v1",
          compiled_id: "c_selecttool_v2",
          createdAtMs: Date.now() - 60_000 * 30,
          jsonPretty: JSON.stringify({ report: { holdoutReward: 0.74 }, candidates: 4 }, null, 2),
        },
      }),
  },
];
