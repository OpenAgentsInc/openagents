import type {
  AgentCardState,
  SafeTurnProjection,
  TurnStageKind,
} from "@openagentsinc/agent-runtime-schema";

/**
 * `@openagentsinc/agent-surface` — the UI-neutral surface projectors (AFS-00
 * reservation).
 *
 * Packet AFS-04 adds the first real projectors that turn canonical turn facts
 * into safe cards and message chains. AFS-00 reserves the package and its
 * import boundary.
 *
 * This package owns pure projectors and surface-intent helpers. It must not own
 * schemas, renderers, or providers. It imports its schemas from
 * `@openagentsinc/agent-runtime-schema`; it must not define a second wire
 * contract.
 */
export const AGENT_SURFACE_PACKAGE = "@openagentsinc/agent-surface" as const;
export const AGENT_SURFACE_RESERVED = true as const;

/**
 * The projector surface AFS-04 implements. A projector reads a safe turn
 * projection and emits a bounded card. The projector is a pure function; it
 * acquires no schema, renderer, or provider authority.
 */
export interface SafeCardProjector {
  readonly project: (projection: SafeTurnProjection) => {
    readonly cardState: AgentCardState;
    readonly stage: TurnStageKind;
  };
}
