import { Schema as S } from "effect";
import type {
  AgentDefinitionHarnessKind,
  AgentRuntimeAdapterKind,
} from "@openagentsinc/agent-runtime-schema";

/**
 * HARN-05 core: ONE readiness projection over harness adapters. Today the
 * desktop models readiness twice — the Provider Lane SPI's
 * `ProviderLaneCapabilityReport` and the AFS kernel's
 * `InferenceProviderDescriptor` / Pylon heartbeat capacity refs — and derives
 * routing candidates, FAV admission, and the Apple FM constrained-candidate set
 * from those separate projections. This module makes the harness adapter set the
 * single readiness source: one input list projects into (a) the router candidate
 * vocabulary, (b) the admitted (ready) subset, and (c) Pylon-style capacity refs.
 *
 * The snapshot shape aligns field-for-field with
 * `@openagentsinc/harness-conformance` `HarnessReadiness` (ready, harness,
 * capacityAvailable, capacityReady, busy, queued, models, failureClass) so the
 * desktop wiring can hand these straight to the conformance registry without a
 * second vocabulary.
 */
export interface HarnessReadinessInput {
  readonly harnessId: string;
  readonly harnessKind: AgentDefinitionHarnessKind;
  readonly adapterKind: AgentRuntimeAdapterKind;
  readonly ready: boolean;
  /** Advertised free slots. Defaults to `ready ? 1 : 0` when omitted. */
  readonly capacityAvailable?: number;
  readonly busy?: number;
  readonly queued?: number;
  readonly models?: ReadonlyArray<string>;
  /** Operator-facing failure class when not ready (account_exhausted, ...). */
  readonly failureClass?: string;
}

/** A routing candidate — the vocabulary the Apple FM constrained sampler admits. */
export const HarnessCandidate = S.Struct({
  harnessId: S.NonEmptyString,
  harnessKind: S.String,
  adapterKind: S.String,
  ready: S.Boolean,
});
export interface HarnessCandidate extends S.Schema.Type<typeof HarnessCandidate> {}

/** Normalized readiness snapshot, aligned with harness-conformance HarnessReadiness. */
export interface HarnessReadinessSnapshot {
  readonly harness: string;
  readonly ready: boolean;
  readonly capacityAvailable: number;
  readonly capacityReady: number;
  readonly busy: number;
  readonly queued: number;
  readonly models: ReadonlyArray<string>;
  readonly failureClass?: string;
}

export interface HarnessReadinessProjection {
  /** Every adapter as a candidate, ready or not. */
  readonly candidates: ReadonlyArray<HarnessCandidate>;
  /** The admitted subset — ready adapters only. The router's constrained set. */
  readonly readyCandidates: ReadonlyArray<HarnessCandidate>;
  /** Per-adapter normalized snapshots. */
  readonly snapshots: ReadonlyArray<HarnessReadinessSnapshot>;
  /**
   * Pylon-style heartbeat capacity refs derived from the SAME projection, e.g.
   * `capacity.coding.codex.available=1`, `load.coding.codex.busy=0`. Counted
   * refs (`=N`) are valid and must not be stripped.
   */
  readonly capacityRefs: ReadonlyArray<string>;
}

const snapshotOf = (input: HarnessReadinessInput): HarnessReadinessSnapshot => {
  const capacityAvailable = input.capacityAvailable ?? (input.ready ? 1 : 0);
  return {
    harness: input.harnessId,
    ready: input.ready,
    capacityAvailable,
    capacityReady: input.ready ? capacityAvailable : 0,
    busy: input.busy ?? 0,
    queued: input.queued ?? 0,
    models: input.models ?? [],
    ...(input.failureClass === undefined ? {} : { failureClass: input.failureClass }),
  };
};

const candidateOf = (input: HarnessReadinessInput): HarnessCandidate => ({
  harnessId: input.harnessId,
  harnessKind: input.harnessKind,
  adapterKind: input.adapterKind,
  ready: input.ready,
});

const capacityRefsOf = (
  input: HarnessReadinessInput,
  snapshot: HarnessReadinessSnapshot,
): ReadonlyArray<string> => {
  const k = input.harnessKind;
  return [
    `capacity.coding.${k}.available=${snapshot.capacityAvailable}`,
    `capacity.coding.${k}.ready=${snapshot.capacityReady}`,
    `load.coding.${k}.busy=${snapshot.busy}`,
    `load.coding.${k}.queued=${snapshot.queued}`,
  ];
};

/**
 * Project a set of harness adapter readiness inputs into the unified readiness
 * view. Pure and deterministic — the desktop wiring feeds live adapter readiness
 * in, and the kernel descriptor, FAV routing, the Apple FM candidate set, and
 * the Pylon heartbeat all read out of the one result.
 */
export const projectHarnessReadiness = (
  inputs: ReadonlyArray<HarnessReadinessInput>,
): HarnessReadinessProjection => {
  const candidates = inputs.map(candidateOf);
  const snapshots = inputs.map(snapshotOf);
  const capacityRefs = inputs.flatMap((input, index) => capacityRefsOf(input, snapshots[index]!));
  return {
    candidates,
    readyCandidates: candidates.filter((c) => c.ready),
    snapshots,
    capacityRefs,
  };
};

/** The admitted candidate ids (ready adapters) — the router's constrained vocabulary. */
export const admittedHarnessIds = (projection: HarnessReadinessProjection): ReadonlyArray<string> =>
  projection.readyCandidates.map((c) => c.harnessId);
