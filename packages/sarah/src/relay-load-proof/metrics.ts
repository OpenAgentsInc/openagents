import type {
  LoadProofErrorClass,
  LoadProofThresholds,
  LatencyStats,
  PhaseMetrics,
  LoadProofPhase,
} from "./types.js";

export const emptyErrorClasses = (): Record<LoadProofErrorClass, number> => ({
  connect_failed: 0,
  timeout: 0,
  ok_false: 0,
  protocol_error: 0,
  closed: 0,
  other: 0,
});

export const percentile = (
  sortedAscending: readonly number[],
  p: number,
): number => {
  if (sortedAscending.length === 0) return 0;
  const index = Math.min(
    sortedAscending.length - 1,
    Math.max(0, Math.ceil((p / 100) * sortedAscending.length) - 1),
  );
  return sortedAscending[index] ?? 0;
};

export const latencyStats = (samplesMs: readonly number[]): LatencyStats => {
  if (samplesMs.length === 0) {
    return { count: 0, medianMs: 0, p99Ms: 0, minMs: 0, maxMs: 0 };
  }
  const sorted = [...samplesMs].sort((a, b) => a - b);
  return {
    count: sorted.length,
    medianMs: percentile(sorted, 50),
    p99Ms: percentile(sorted, 99),
    minMs: sorted[0] ?? 0,
    maxMs: sorted[sorted.length - 1] ?? 0,
  };
};

export const buildPhaseMetrics = (input: {
  phase: LoadProofPhase;
  durationMs: number;
  latenciesMs: readonly number[];
  failures: number;
  errorClasses: Readonly<Record<LoadProofErrorClass, number>>;
}): PhaseMetrics => {
  const successes = input.latenciesMs.length;
  const attempts = successes + input.failures;
  const seconds = Math.max(input.durationMs / 1000, 0.001);
  return {
    phase: input.phase,
    attempts,
    successes,
    failures: input.failures,
    rps: successes / seconds,
    latency: latencyStats(input.latenciesMs),
    errorClasses: { ...input.errorClasses },
  };
};

export const evaluateThresholds = (input: {
  publish: PhaseMetrics;
  subscribe: PhaseMetrics;
  thresholds: LoadProofThresholds;
}): { pass: boolean; failures: string[] } => {
  const failures: string[] = [];
  const { thresholds, publish, subscribe } = input;

  if (publish.rps < thresholds.minPublishRps) {
    failures.push(
      `publish_rps ${publish.rps.toFixed(2)} < min ${thresholds.minPublishRps}`,
    );
  }
  if (subscribe.rps < thresholds.minSubscribeRps) {
    failures.push(
      `subscribe_rps ${subscribe.rps.toFixed(2)} < min ${thresholds.minSubscribeRps}`,
    );
  }
  if (publish.latency.medianMs > thresholds.maxMedianLatencyMs) {
    failures.push(
      `publish_median_ms ${publish.latency.medianMs} > max ${thresholds.maxMedianLatencyMs}`,
    );
  }
  if (subscribe.latency.medianMs > thresholds.maxMedianLatencyMs) {
    failures.push(
      `subscribe_median_ms ${subscribe.latency.medianMs} > max ${thresholds.maxMedianLatencyMs}`,
    );
  }
  if (publish.latency.p99Ms > thresholds.maxP99LatencyMs) {
    failures.push(
      `publish_p99_ms ${publish.latency.p99Ms} > max ${thresholds.maxP99LatencyMs}`,
    );
  }
  if (subscribe.latency.p99Ms > thresholds.maxP99LatencyMs) {
    failures.push(
      `subscribe_p99_ms ${subscribe.latency.p99Ms} > max ${thresholds.maxP99LatencyMs}`,
    );
  }

  const publishErrorRate =
    publish.attempts === 0 ? 1 : publish.failures / publish.attempts;
  const subscribeErrorRate =
    subscribe.attempts === 0 ? 1 : subscribe.failures / subscribe.attempts;
  if (publishErrorRate > thresholds.maxErrorRate) {
    failures.push(
      `publish_error_rate ${publishErrorRate.toFixed(4)} > max ${thresholds.maxErrorRate}`,
    );
  }
  if (subscribeErrorRate > thresholds.maxErrorRate) {
    failures.push(
      `subscribe_error_rate ${subscribeErrorRate.toFixed(4)} > max ${thresholds.maxErrorRate}`,
    );
  }

  return { pass: failures.length === 0, failures };
};
