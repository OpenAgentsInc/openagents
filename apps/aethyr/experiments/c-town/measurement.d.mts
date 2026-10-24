export const MEASUREMENT_SCHEMA: "aethyrforge.measurement.v1";
export const QUANTILE_METHOD: "linear-n-minus-one";
export interface Distribution {
  count: number;
  min: number;
  p50: number;
  p95: number;
  p99: number;
  max: number;
  mean: number;
  missing: number;
}
export interface LegacyDistribution extends Omit<
  Distribution,
  "p50" | "missing"
> {
  median: number;
}
export function distribution(
  values: readonly (number | null | undefined)[],
): Distribution | null;
export function statistics(values: readonly number[]): LegacyDistribution;
export function summarizeResponseBodies(
  entries: readonly { responseBodySize?: unknown }[],
): {
  responseBodyBytes: number | null;
  knownResponseBodyBytes: number;
  responses: number;
  unavailableResponses: number;
  invalidResponses: number;
};
export function balancedOrder<T extends string | number>(
  candidates: readonly T[],
  rounds: number,
): T[][];
export interface FrameObservation {
  at: number;
  submitted: boolean;
  elapsedMs?: number | null;
  stepMs?: number | null;
  renderCpuMs?: number | null;
  cullMs?: number | null;
  clusterMs?: number | null;
  gpuMs?: number | null;
  bridgeEncodeMs?: number | null;
  wasmRequestMs?: number | null;
  bridgeDecodeMs?: number | null;
  bridgeOverheadMs?: number | null;
  commandEncodeMs?: number | null;
  kernelDroppedSeconds?: number | null;
  hostClampedSeconds?: number | null;
}
export interface FrameSummary {
  callbacks: number;
  acceptedSubmissions: number;
  skippedCallbacks: number;
  measurementMs: number;
  acceptedSubmissionsPerSecond: number | null;
  callbackIntervalMs: Distribution | null;
  submissionIntervalMs: Distribution | null;
  metrics: Record<string, Distribution | null>;
  acceptedMetrics: Record<string, Distribution | null>;
  dropped: { kernelSeconds: number | null; hostClampedSeconds: number | null };
  presentationMs: null;
  scopes: Readonly<Record<string, string>>;
}
export const FRAME_TIMING_SCOPES: Readonly<Record<string, string>>;
export function summarizeFrames(
  samples: readonly FrameObservation[],
): FrameSummary;
export function summarizeSamples(
  samples: Record<string, unknown>[],
): Record<string, (LegacyDistribution & { missing: number }) | null>;
export function frameSummaries(samples: Record<string, any>[]): {
  summary: ReturnType<typeof summarizeSamples>;
  submittedSummary: ReturnType<typeof summarizeSamples> | null;
  submissionIntervalsMs: LegacyDistribution | null;
  submissionMeasurementMs: number;
  submittedFramesPerSecond: number | null;
};
export interface ArchiveEvidenceInput {
  path: string;
  bytes: Uint8Array;
  issue?: number;
  limitations: string[];
}
export interface ArchiveEvidenceReference {
  path: string;
  sha256: string;
  bytes: number;
  schema: string;
  issue?: number;
  limitations: string[];
}
export function archiveEvidence(
  inputs: readonly ArchiveEvidenceInput[],
): ArchiveEvidenceReference[];
export interface MeasurementReport {
  schema: typeof MEASUREMENT_SCHEMA;
  quantile: typeof QUANTILE_METHOD;
  id: string;
  startedAt: string;
  finishedAt: string;
  ok: boolean;
  environment: {
    browser: string | null;
    os: string | null;
    gpu: Record<string, unknown> | null;
  };
  pins: {
    source: {
      revision: string | null;
      dirty: boolean;
      patchSha256: string | null;
    };
    assets: { sha256: string | null; scope: string };
    kernel: { sha256: string | null; abi: string | number | null };
    wgsl: { sha256: string | null; scope: string };
    quality: { sha256: string | null; settings: Record<string, unknown> };
    build: {
      mode: string;
      sha256: string | null;
      toolchain: Record<string, unknown>;
    };
  };
  cache: {
    state: "cold" | "warm" | "mixed" | "not-applicable" | "unknown";
    scope: string;
  };
  warmup: { durationMs: number | null; iterations: number | null };
  trialOrder: string[][];
  phases: {
    id: string;
    kind: "simulation-cpu" | "integrated" | "startup" | "edit" | "archive";
    scope: string;
    metrics: Record<string, unknown>;
  }[];
  evidence: ArchiveEvidenceReference[];
  limitations: string[];
  details?: unknown;
}
export function validateMeasurementReport(value: unknown): MeasurementReport;
export function createMeasurementReport(
  input: Omit<MeasurementReport, "schema" | "quantile">,
): MeasurementReport;
export interface PlayerByteBudget {
  jsRawBytes: number;
  jsGzipBytes: number;
  wasmRawBytes: number;
  assetRawBytes: number;
  deliveredFiles: number;
}
export function checkByteBudgets(
  audit: {
    ok: boolean;
    requests: number;
    totals: Record<string, { rawBytes: number; gzipBytes: number }>;
  },
  budget: PlayerByteBudget,
): {
  ok: boolean;
  checks: { metric: string; actual: number; max: number; ok: boolean }[];
  violations: string[];
};
