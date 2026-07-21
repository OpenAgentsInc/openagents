import { Schema as S } from "effect";
import {
  KhalaRuntimeEvent,
  KhalaRuntimeEventSchemaLiteral,
} from "@openagentsinc/agent-runtime-schema";

/**
 * The harness stream event IS the neutral `KhalaRuntimeEvent`
 * (`openagents.khala_runtime_event.v1`). It is deliberately NOT a new event
 * union: `KhalaRuntimeEvent` already carries a superset of the AI SDK
 * `HarnessV1StreamPart` vocabulary — text/reasoning deltas, tool call/result
 * (with `providerExecuted`), step/turn boundaries with usage, `file.change`,
 * `compaction.recorded`, `usage.recorded`, `agent.child.*`, and
 * `raw.sidecar_ref` — plus the `sequence` field every event needs to serve as
 * the durable replay cursor (HARN-02).
 *
 * Adapters emit these; the renderer-facing `ClaudeLocalEvent` envelope becomes
 * one projection of this stream (HARN-03), never a second source of truth.
 */
export const HarnessStreamEvent = KhalaRuntimeEvent;
export type HarnessStreamEvent = typeof KhalaRuntimeEvent.Type;

export { KhalaRuntimeEventSchemaLiteral };

/**
 * Monotonic per-turn cursor. Equal to the `sequence` field of the last
 * `HarnessStreamEvent` delivered to a consumer. A consumer holding cursor N
 * attaches and receives events N+1.. with no gap and no duplicate (HARN-02).
 */
export const HarnessCursor = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));
export type HarnessCursor = typeof HarnessCursor.Type;

/** Read the replay cursor from a stream event. */
export const cursorOf = (event: HarnessStreamEvent): HarnessCursor => event.sequence;
