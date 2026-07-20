import type {
  InferenceProviderDescriptor,
  OwnerBoundCandidateSet,
  ReleasedArtifact,
  RouteDecision,
  RouteRecommendation,
  SafeTurnProjection,
  TurnCandidate,
  TurnIntent,
  TurnLifecycleState,
  TurnReceipt,
  TurnRefusalReason,
  WorkContextEnvelope,
} from "@openagentsinc/agent-runtime-schema";
import type { IdeRuntimeContextManifestRef } from "@openagentsinc/ide-runtime";
import type { PortableTargetClass } from "@openagentsinc/portable-session-contract";
import type { ProviderAccountRef } from "@openagentsinc/provider-account-schema";

/**
 * `@openagentsinc/agent-turn-runtime` — the shared, UI-neutral Effect turn
 * kernel (AFS-00 reservation).
 *
 * Packet AFS-01 adds the scoped `TurnService`, the deterministic state
 * transitions, and the injected ports. AFS-00 reserves the package graph, the
 * port type surface, and the import boundary. This package owns turn policy and
 * turn state machines. It must not own providers, storage, UI, or platform
 * APIs. Apple FM implements the provider port here; this package must not import
 * `@openagentsinc/apple-fm-runtime`.
 */
export const AGENT_TURN_RUNTIME_PACKAGE = "@openagentsinc/agent-turn-runtime" as const;
export const AGENT_TURN_RUNTIME_RESERVED = true as const;

/**
 * The port surface AFS-01 implements. These are the injected boundaries the
 * turn kernel composes. Each is a pure type at AFS-00. AFS-01 promotes them to
 * scoped `Context.Service` boundaries with `Layer` implementations and
 * deterministic in-memory test adapters.
 */
export interface TurnPolicyPort {
  readonly decide: (
    intent: TurnIntent,
    context: WorkContextEnvelope,
    candidateSet: OwnerBoundCandidateSet,
    recommendation: RouteRecommendation | null,
  ) => RouteDecision;
}

export interface InferenceProviderRegistryPort {
  readonly describe: () => ReadonlyArray<InferenceProviderDescriptor>;
}

export interface ContextSourcePort {
  readonly manifestRef: IdeRuntimeContextManifestRef;
}

export interface ArtifactResolverPort {
  readonly resolve: () => ReleasedArtifact | null;
}

export interface TurnJournalPort {
  readonly state: TurnLifecycleState;
}

export interface TurnKernelResult {
  readonly candidate: TurnCandidate | null;
  readonly receipt: TurnReceipt;
  readonly projection: SafeTurnProjection;
  readonly refusal: TurnRefusalReason | null;
}

/** Account custody references stay owned by the account schema, never re-modeled here. */
export type TurnAccountRef = ProviderAccountRef;

/** Placement classification is owned by the portable-session contract. */
export type TurnPlacementClass = PortableTargetClass;
