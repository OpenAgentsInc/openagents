import { readdir, readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  decodeProbeBenchmarkCloseout,
  decodeProbeBenchmarkRouteScorecard,
  decodeProbeBenchmarkRun,
} from "../src";

const testDir = dirname(fileURLToPath(import.meta.url));
const canaryDir = join(testDir, "../../../docs/benchmarks/canaries/20260608151057");
const bundleDir = join(canaryDir, "closeout-bundle");
const CANARY_CLOSEOUT_BUNDLE_FILE_NAMES = [
  "probe-run-record.json",
  "probe-closeout.json",
  "decision-trace-summary.json",
  "selected-signatures.json",
  "tool-menu.json",
  "candidate-ref.json",
  "artifact-refs.json",
  "resource-usage-ref.json",
  "policy-findings.json",
  "failure-classification.json",
  "route-scorecard.json",
] as const;

const readJson = async (path: string): Promise<unknown> => JSON.parse(await readFile(path, "utf8"));

describe("live Probe GEPA Terminal-Bench canary receipt", () => {
  test("retains a schema-backed unpaid Pylon closeout bundle with public-safe refs", async () => {
    const receipt = await readJson(join(canaryDir, "canary-receipt.json")) as {
      readonly assignmentRef: string;
      readonly claimBoundary: Record<string, unknown>;
      readonly closeoutBundle: { readonly fileRefs: ReadonlyArray<string> };
      readonly pylonLifecycle: {
        readonly acceptedWorkRefs: ReadonlyArray<string>;
        readonly artifactRefs: ReadonlyArray<string>;
        readonly assignmentState: string;
        readonly closeoutRefs: ReadonlyArray<string>;
        readonly leaseState: string;
        readonly proofRefs: ReadonlyArray<string>;
      };
      readonly publicStatsSnapshot: Record<string, unknown>;
      readonly runRef: string;
      readonly taskRefs: ReadonlyArray<string>;
    };
    const run = await Effect.runPromise(decodeProbeBenchmarkRun(await readJson(join(bundleDir, "probe-run-record.json"))));
    const closeout = await Effect.runPromise(
      decodeProbeBenchmarkCloseout(await readJson(join(bundleDir, "probe-closeout.json"))),
    );
    const decisionTrace = await readJson(join(bundleDir, "decision-trace-summary.json")) as {
      readonly assignmentRef: string;
      readonly redactionState: string;
      readonly runRef: string;
      readonly selectedSignatureRefs: ReadonlyArray<string>;
      readonly traceRef: string;
    };
    const routeScorecard = await Effect.runPromise(
      decodeProbeBenchmarkRouteScorecard(await readJson(join(bundleDir, "route-scorecard.json"))),
    );
    const importRequest = await readJson(join(canaryDir, "psionic-import-request.json")) as {
      readonly acceptedForFrontierMutation: boolean;
      readonly requestedFrontierState: string;
      readonly safetyBoundary: Record<string, unknown>;
    };

    expect(receipt.assignmentRef).toBe("assignment.public.probe_gepa.terminal_bench_2.canary.20260608151057");
    expect(receipt.runRef).toBe("probe_run.public.probe_gepa.terminal_bench_2.canary.20260608151057");
    expect(receipt.taskRefs).toEqual([
      "task.public.terminal_bench_2.configure_git_webserver.retained",
      "task.public.terminal_bench_2.filter_js_from_html.retained",
    ]);
    const bundleFileNames = await readdir(bundleDir);
    expect(receipt.closeoutBundle.fileRefs.sort()).toEqual([...CANARY_CLOSEOUT_BUNDLE_FILE_NAMES].sort());
    expect(bundleFileNames.sort()).toEqual([...CANARY_CLOSEOUT_BUNDLE_FILE_NAMES].sort());

    expect(receipt.pylonLifecycle.assignmentState).toBe("accepted_work");
    expect(receipt.pylonLifecycle.leaseState).toBe("terminal");
    expect(receipt.pylonLifecycle.artifactRefs).toContain(
      "artifact.public.probe_run.public.probe_gepa.terminal_bench_2.canary.20260608151057.probe_closeout_bundle",
    );
    expect(receipt.pylonLifecycle.proofRefs).toContain(
      "proof.public.probe_run.public.probe_gepa.terminal_bench_2.canary.20260608151057.no_spend_assignment_lifecycle",
    );
    expect(receipt.pylonLifecycle.acceptedWorkRefs).toContain(
      "accepted_work.public.probe_run.public.probe_gepa.terminal_bench_2.canary.20260608151057.probe_gepa_canary",
    );
    expect(receipt.pylonLifecycle.closeoutRefs).toContain(
      "probe_closeout.probe_run.public.probe_gepa.terminal_bench_2.canary.20260608151057",
    );

    expect(run.assignmentRef).toBe(receipt.assignmentRef);
    expect(run.runRef).toBe(receipt.runRef);
    expect(run.status).toBe("failed");
    expect(closeout.assignmentRef).toBe(receipt.assignmentRef);
    expect(closeout.runStatus).toBe("failed");
    expect(closeout.evidenceSplit).toBe("retained");
    expect(closeout.failureClassification.family).toBe("service_readiness");
    expect(closeout.promotionStatus).toBe("blocked");
    expect(closeout.routeScorecardRef).toBe(routeScorecard.scorecardRef);
    expect(decisionTrace.assignmentRef).toBe(receipt.assignmentRef);
    expect(routeScorecard.selectedRouteKind).toBe("pylon");
    expect(routeScorecard.trustTier).toBe("registered_pylon");
    expect(routeScorecard.privacyTier).toBe("pylon_worker");

    expect(receipt.publicStatsSnapshot.status).toBe("live");
    expect(receipt.publicStatsSnapshot.pylonsAssignmentReadyNow).toBe(1);
    expect(receipt.claimBoundary.publicBenchmarkScoreClaimAllowed).toBe(false);
    expect(receipt.claimBoundary.paidWorkClaimAllowed).toBe(false);
    expect(receipt.claimBoundary.settledBitcoinClaimAllowed).toBe(false);
    expect(receipt.claimBoundary.modelTrainingClaimAllowed).toBe(false);
    expect(receipt.claimBoundary.runtimePromotionAllowed).toBe(false);
    expect(importRequest.acceptedForFrontierMutation).toBe(false);
    expect(importRequest.requestedFrontierState).toBe("pending_live_import_review");
    expect(importRequest.safetyBoundary.paidWorkClaimAllowed).toBe(false);

    const trackedMaterial = JSON.stringify({
      receipt,
      run,
      closeout,
      decisionTrace,
      routeScorecard,
      importRequest,
    });
    expect(trackedMaterial).not.toMatch(/\/Users\/|Bearer |OPENAGENTS_|sk-[A-Za-z0-9]|lnbc|mnemonic|recovery phrase/i);
  });
});
