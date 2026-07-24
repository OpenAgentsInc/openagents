/**
 * SARAH-NR-03 load-proof types.
 *
 * Public-safe metrics only. No secrets, private keys, or raw event bodies
 * appear in the report.
 */

export type LoadProofPhase = "publish" | "subscribe" | "mixed";

export type LoadProofErrorClass =
  | "connect_failed"
  | "timeout"
  | "ok_false"
  | "protocol_error"
  | "closed"
  | "other";

export interface LoadProofThresholds {
  /** Minimum successful EVENT→OK rate (events per second). */
  readonly minPublishRps: number;
  /** Minimum successful REQ→EOSE subscribe completions per second. */
  readonly minSubscribeRps: number;
  /** Maximum median latency in milliseconds for the measured phase. */
  readonly maxMedianLatencyMs: number;
  /** Maximum 99th-percentile latency in milliseconds. */
  readonly maxP99LatencyMs: number;
  /** Maximum fraction of failed operations (0–1). */
  readonly maxErrorRate: number;
}

/** Local in-process memory relay thresholds (startTestRelay / mock). */
export const LOCAL_LOAD_PROOF_THRESHOLDS: LoadProofThresholds = {
  minPublishRps: 40,
  minSubscribeRps: 20,
  maxMedianLatencyMs: 150,
  maxP99LatencyMs: 750,
  maxErrorRate: 0.02,
};

/**
 * Production Cloud Run + Postgres thresholds (stricter durability path).
 * Used when RELAY_URL points at a remote host.
 */
export const REMOTE_LOAD_PROOF_THRESHOLDS: LoadProofThresholds = {
  minPublishRps: 20,
  minSubscribeRps: 10,
  maxMedianLatencyMs: 400,
  maxP99LatencyMs: 2_000,
  maxErrorRate: 0.05,
};

export interface LoadProofConfig {
  readonly relayUrl: string;
  readonly durationMs: number;
  readonly publishers: number;
  readonly subscribers: number;
  readonly publishIntervalMs: number;
  readonly thresholds: LoadProofThresholds;
  readonly connectTimeoutMs: number;
  readonly operationTimeoutMs: number;
}

export const DEFAULT_LOCAL_LOAD_PROOF_CONFIG: Omit<LoadProofConfig, "relayUrl"> =
  {
    durationMs: 5_000,
    publishers: 4,
    subscribers: 2,
    publishIntervalMs: 20,
    thresholds: LOCAL_LOAD_PROOF_THRESHOLDS,
    connectTimeoutMs: 3_000,
    operationTimeoutMs: 3_000,
  };

export interface LatencyStats {
  readonly count: number;
  readonly medianMs: number;
  readonly p99Ms: number;
  readonly minMs: number;
  readonly maxMs: number;
}

export interface PhaseMetrics {
  readonly phase: LoadProofPhase;
  readonly attempts: number;
  readonly successes: number;
  readonly failures: number;
  readonly rps: number;
  readonly latency: LatencyStats;
  readonly errorClasses: Readonly<Record<LoadProofErrorClass, number>>;
}

export interface LoadProofReport {
  readonly schema: "openagents.sarah.relay_load_proof.v1";
  readonly packet: "SARAH-NR-03";
  readonly measuredAt: string;
  readonly relayUrl: string;
  readonly hostMode: "local_started" | "remote" | "mock";
  readonly nostrEffectPin: string | null;
  readonly durationMs: number;
  readonly publish: PhaseMetrics;
  readonly subscribe: PhaseMetrics;
  readonly thresholds: LoadProofThresholds;
  readonly pass: boolean;
  readonly failures: ReadonlyArray<string>;
  readonly notes: ReadonlyArray<string>;
}

export const NOSTR_EFFECT_NODE_PIN =
  "77073343c68f159f3dea80ddbe9e9896b1f052f2" as const;

export const NOSTR_EFFECT_NODE_EXPORTS = {
  host: "nostr-effect/relay/node",
  sqlite: "nostr-effect/relay/node/sqlite",
  postgres: "nostr-effect/relay/node/postgres",
  entry: "src/relay/main.ts",
} as const;
