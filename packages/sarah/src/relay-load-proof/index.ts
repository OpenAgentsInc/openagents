/**
 * SARAH-NR-03 owned-relay load proof.
 *
 * @see docs/ops/2026-07-24-owned-nostr-relay-deploy.md
 */
export {
  runLoadProof,
  startLocalLoadProofHost,
  resolveThresholds,
  type StartedHost,
} from "./harness.js";
export {
  LOCAL_LOAD_PROOF_THRESHOLDS,
  REMOTE_LOAD_PROOF_THRESHOLDS,
  DEFAULT_LOCAL_LOAD_PROOF_CONFIG,
  NOSTR_EFFECT_NODE_PIN,
  NOSTR_EFFECT_NODE_EXPORTS,
  type LoadProofConfig,
  type LoadProofReport,
  type LoadProofThresholds,
  type PhaseMetrics,
  type LatencyStats,
} from "./types.js";
export { startMockRelay, type MockRelayHandle } from "./mock-relay.js";
export {
  buildPhaseMetrics,
  evaluateThresholds,
  latencyStats,
  percentile,
} from "./metrics.js";
export { createSignedEvent, generatePrivateKeyHex } from "./event.js";
