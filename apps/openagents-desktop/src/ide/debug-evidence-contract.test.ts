import { describe, expect, test } from "vite-plus/test";
import { Schema } from "effect";

import {
  IDE_DEBUG_ACCESSIBILITY_NAMES,
  IDE_DEBUG_CONTROL_NAMES,
  IDE_DEBUG_FAULT_NAMES,
  IDE_DEBUG_LIFECYCLE_NAMES,
  IDE_DEBUG_METRIC_NAMES,
  IDE_DEBUG_SOURCE_KINDS,
  IdeDebugEvidenceInputSchema,
  validateIdeDebugCapturedEvidence,
} from "./debug-evidence-contract.ts";

const decodeEvidenceInput = Schema.decodeUnknownSync(IdeDebugEvidenceInputSchema);

const journey = (
  journeyRef: string,
  adapterKind: "deterministic-fake" | "representative-real",
  language: string,
  mode: "launch" | "attach",
) => ({
  journeyRef,
  adapterKind,
  adapterName:
    adapterKind === "deterministic-fake"
      ? "OpenAgents fixture DAP"
      : `${language} reference adapter`,
  adapterVersion: "1.0.0",
  language,
  languageVersion: "1.0.0",
  mode,
  desktopTarget: "macos-arm64",
  targetKind: mode === "launch" ? "local-process" : "remote-process",
  transport: mode === "launch" ? "stdio" : "tcp",
  configurationRef: `evidence/config/${journeyRef}.json`,
  effectiveConfigurationDigest: "b".repeat(64),
  dataSourceRefs: ["workspace-manifest:v1", "secret-ref:test-only"],
  environmentValueRefsOnly: true,
  generations: {
    project: 1,
    worktree: 1,
    attachment: 1,
    language: 1,
    target: 1,
    placement: 1,
    service: 1,
  },
  capabilities: {
    supported: ["continue", "pause", "evaluate"],
    unsupported: ["stepBack"],
    negotiatedBeforeCommands: true,
  },
  projections: {
    breakpoints: true,
    threads: true,
    stacks: true,
    scopes: true,
    variables: true,
    watches: true,
    console: true,
    modules: true,
    loadedSources: true,
  },
  screenshotRef: `apps/openagents-desktop/benchmarks/ide/${journeyRef}.png`,
  traceRef: `apps/openagents-desktop/benchmarks/ide/${journeyRef}-trace.json`,
  receiptRef: `apps/openagents-desktop/benchmarks/ide/${journeyRef}-receipt.json`,
  passed: true,
});

const elementAt = <Value>(values: ReadonlyArray<Value>, index: number): Value => {
  const value = values[index];
  if (value === undefined) throw new Error(`test fixture does not have element ${index}`);
  return value;
};

const capturedInput = (): unknown => {
  return {
    _tag: "Captured",
    schemaVersion: "openagents.desktop.ide-debug-evidence-input.v1",
    issue: "IDE-11",
    recordedAt: "2026-07-20T12:00:00.000Z",
    candidateCommitSha: "a".repeat(40),
    environment: {
      platform: "darwin",
      architecture: "arm64",
      node: "v24.0.0",
      electron: "43.1.0",
      appVersion: "0.1.0-rc.25",
      runtime: "Effect v4 + supervised DAP transport",
      corpusRef: "apps/openagents-desktop/benchmarks/ide/ide-11-corpus.json",
    },
    artifact: {
      treeSha256: "c".repeat(64),
      files: 10,
      bytes: 1000,
      artifactRef: "out/OpenAgents-darwin-arm64/OpenAgents.app",
    },
    journeys: [
      journey("fake-launch", "deterministic-fake", "fixture", "launch"),
      journey("fake-attach", "deterministic-fake", "fixture", "attach"),
      journey("node-launch", "representative-real", "typescript", "launch"),
      journey("python-attach", "representative-real", "python", "attach"),
    ],
    controls: IDE_DEBUG_CONTROL_NAMES.map((control) => ({
      control,
      supported: control !== "step-out",
      capabilityNegotiated: true,
      cancellable: true,
      receiptRef: `evidence/control/${control}.json`,
      unsupportedStateHonest: true,
      passed: true,
    })),
    sources: IDE_DEBUG_SOURCE_KINDS.map((kind) => ({
      kind,
      canonicalIdentityUsed: true,
      guessedPosition: false,
      explicitState: true,
      evidenceRef: `evidence/source/${kind}.json`,
      passed: true,
    })),
    lifecycle: IDE_DEBUG_LIFECYCLE_NAMES.map((transition) => ({
      transition,
      oldGeneration: 1,
      newGeneration: 2,
      lateEventSent: true,
      lateEventRejected: true,
      currentStateUnchanged: true,
      cleanupReceiptRef: `evidence/lifecycle/${transition}.json`,
      passed: true,
    })),
    faultMatrix: IDE_DEBUG_FAULT_NAMES.map((name) => ({
      name,
      evidenceRef: `evidence/fault/${name}.json`,
      passed: true,
    })),
    accessibilityMatrix: IDE_DEBUG_ACCESSIBILITY_NAMES.map((name) => ({
      name,
      evidenceRef: `evidence/a11y/${name}.json`,
      passed: true,
    })),
    metrics: IDE_DEBUG_METRIC_NAMES.map((name) => ({
      name,
      unit: "milliseconds",
      repetitions: 30,
      warmup: 5,
      p50: 1,
      p95: 2,
      p99: 3,
      thresholdP50: 10,
      thresholdP95: 20,
      thresholdP99: 30,
      passed: true,
    })),
    policy: {
      oneSchemaGraph: true,
      effectAuthority: true,
      rendererProjectionOnly: true,
      adapterMechanicsOnly: true,
      exactConfigurationDisclosed: true,
      exactGenerationsBound: true,
      launchAttachSeparatePaths: true,
      humanAgentSamePolicy: true,
      humanAgentSameBudgets: true,
      humanAgentSameIntervention: true,
      humanAgentSameObservability: true,
      humanAgentSameCleanup: true,
    },
    security: {
      secretsRemainReferences: true,
      projectedDataRedacted: true,
      protocolQueueBounded: true,
      consoleRetentionBounded: true,
      variableDepthBounded: true,
      variableCountBounded: true,
      retainedDataDeleted: true,
      rendererReceivesCredentials: false,
      evidenceContainsForbiddenMaterial: false,
    },
    resources: {
      activeHandlesAfter: 0,
      adapterProcessesAfter: 0,
      subscriptionsAfter: 0,
      queuedProtocolMessagesAfter: 0,
      retainedVariableBytesAfterDeletion: 0,
      peakHeapBytes: 1000000,
      peakCpuPercent: 5,
    },
    targets: [
      {
        target: "macos-arm64",
        claimed: true,
        packagedJourneyRef: "evidence/target/macos-arm64.json",
        nativeHelper: false,
        typescriptFallback: true,
        disposition: "packaged-journey-passed",
      },
      ...["macos-x64", "windows-arm64", "windows-x64", "linux-arm64", "linux-x64"].map(
        (target) => ({
          target,
          claimed: false,
          packagedJourneyRef: null,
          nativeHelper: false,
          typescriptFallback: true,
          disposition: "not-claimed",
        }),
      ),
    ],
    nativeDecision: {
      rustAdmitted: false,
      ac47AdmissionEvidencePresent: false,
      reason:
        "The supervised TypeScript transport meets the measured corpus. No AC-47 native-helper admission evidence is present.",
    },
    ownerDisposition: "unreviewed",
    assuranceLifecycle: "proposed",
  };
};

describe("IDE-11 debug evidence contract", () => {
  test("keeps an unexecuted fixture explicit", () => {
    const input = decodeEvidenceInput({
      _tag: "Unexecuted",
      schemaVersion: "openagents.desktop.ide-debug-evidence-input.v1",
      issue: "IDE-11",
      reason: "The packaged runner has not run.",
      requiredRunner: "Run the exact candidate packaged debugger corpus.",
    });
    expect(IdeDebugEvidenceInputSchema.guards.Unexecuted(input)).toBe(true);
  });

  test("accepts the complete fake and representative launch/attach evidence corpus", () => {
    const input = decodeEvidenceInput(capturedInput());
    if (!IdeDebugEvidenceInputSchema.guards.Captured(input))
      throw new Error("test fixture decoded to the wrong variant");
    expect(() => validateIdeDebugCapturedEvidence(input)).not.toThrow();
    expect(input.journeys).toHaveLength(4);
  });

  test("fails a duplicate metric even when the array length is correct", () => {
    const complete = decodeEvidenceInput(capturedInput());
    if (!IdeDebugEvidenceInputSchema.guards.Captured(complete))
      throw new Error("test fixture decoded to the wrong variant");
    const input = decodeEvidenceInput({
      ...complete,
      metrics: [
        elementAt(complete.metrics, 0),
        elementAt(complete.metrics, 0),
        ...complete.metrics.slice(2),
      ],
    });
    if (!IdeDebugEvidenceInputSchema.guards.Captured(input))
      throw new Error("test fixture decoded to the wrong variant");
    expect(() => validateIdeDebugCapturedEvidence(input)).toThrow(/metrics is incomplete/u);
  });

  test("fails a metric that exceeds a percentile threshold", () => {
    const complete = decodeEvidenceInput(capturedInput());
    if (!IdeDebugEvidenceInputSchema.guards.Captured(complete))
      throw new Error("test fixture decoded to the wrong variant");
    const input = decodeEvidenceInput({
      ...complete,
      metrics: [{ ...elementAt(complete.metrics, 0), p99: 31 }, ...complete.metrics.slice(1)],
    });
    if (!IdeDebugEvidenceInputSchema.guards.Captured(input))
      throw new Error("test fixture decoded to the wrong variant");
    expect(() => validateIdeDebugCapturedEvidence(input)).toThrow(/within all thresholds/u);
  });

  test("fails a corpus without two representative real languages", () => {
    const complete = decodeEvidenceInput(capturedInput());
    if (!IdeDebugEvidenceInputSchema.guards.Captured(complete))
      throw new Error("test fixture decoded to the wrong variant");
    const input = decodeEvidenceInput({
      ...complete,
      journeys: [
        elementAt(complete.journeys, 0),
        elementAt(complete.journeys, 1),
        elementAt(complete.journeys, 0),
        elementAt(complete.journeys, 3),
      ],
    });
    if (!IdeDebugEvidenceInputSchema.guards.Captured(input))
      throw new Error("test fixture decoded to the wrong variant");
    expect(() => validateIdeDebugCapturedEvidence(input)).toThrow(
      /two representative real languages/u,
    );
  });
});
