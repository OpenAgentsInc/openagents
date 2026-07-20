/**
 * `@openagentsinc/agent-turn-runtime` — the shared, UI-neutral Effect turn
 * kernel.
 *
 * AFS-00 reserved the package graph and import boundary. AFS-01 implements the
 * scoped `TurnService`, the deterministic turn state machine, the injected ports,
 * the bounded generation-fenced event gateway, and the safe projection/receipt
 * derivation. This package owns turn policy application and turn state machines.
 * It owns no providers, no storage driver, no UI, and no platform API. Apple FM
 * implements the provider port here; this package must not import
 * `@openagentsinc/apple-fm-runtime` or `@openagentsinc/agent-turn-store`.
 */
export const AGENT_TURN_RUNTIME_PACKAGE = "@openagentsinc/agent-turn-runtime" as const;
export const AGENT_TURN_RUNTIME_RESERVED = true as const;

export * from "./turn-state.js";
export * from "./ports.js";
export * from "./event-gateway.js";
export * from "./projection.js";
export * from "./turn-service.js";
export { layer as TurnServiceLayer } from "./turn-service.js";
export * as TurnServiceTesting from "./testing.js";
