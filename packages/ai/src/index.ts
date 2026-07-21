/**
 * OpenAgents AI SDK — the umbrella entry point (AISDK-02, #9148).
 *
 * The OpenAgents AI SDK is an Effect-native toolkit for building durable
 * agent applications. This package holds no logic. It re-exports the entry
 * points of the SDK layer packages so one install gives the full surface:
 *
 * - L0 MODEL CALL   — `@openagentsinc/ai-model` (over
 *   `effect/unstable/ai`, consumed and never forked)
 * - L1 VOCABULARY   — `@openagentsinc/agent-runtime-schema`
 * - L2 DURABLE LOG  — `@openagentsinc/agent-harness-contract` (event log)
 * - L3 SANDBOX      — `@openagentsinc/agent-harness-contract` (sandbox
 *   provider contract)
 * - L4 HARNESS      — `@openagentsinc/agent-harness-contract` (adapters,
 *   session verbs, readiness)
 * - L5 UI STREAM    — `@openagentsinc/agent-harness-contract` (UI message
 *   chunks and reducer)
 * - L6 RECALL       — `@openagentsinc/history-corpus`
 *
 * The layer map and the publishable roster live in
 * `docs/fable/2026-07-21-effect-native-openagents-ai-sdk-analysis.md` §1.
 * Curated per-layer subpaths (`./model`, `./schema`, `./event-log`,
 * `./sandbox`, `./harness`, `./ui-stream`, `./recall`) mirror the same map.
 *
 * The star re-exports below are collision-audited. The four package roots
 * share exactly one export name, `KhalaRuntimeEventSchemaLiteral`, and both
 * occurrences resolve to the same binding in
 * `@openagentsinc/agent-runtime-schema`, so the union is unambiguous.
 */
export * from "@openagentsinc/agent-runtime-schema";
export * from "@openagentsinc/agent-harness-contract";
export * from "@openagentsinc/history-corpus";
export * from "@openagentsinc/ai-model";
