/**
 * L0 MODEL CALL — the model-call substrate.
 *
 * Re-exports `@openagentsinc/ai-model`: the Effect AI
 * `LanguageModel` Layer over the existing provider transport, the
 * bidirectional `Response.StreamPart` maps (STREAM-01), and the
 * `ExecutionPlan` in-lane provider fallback (STREAM-05). The upstream model
 * primitives (`LanguageModel`, `Tool`, `Toolkit`, `AiError`, `Model`,
 * `ExecutionPlan`, `Chat`) stay in `effect/unstable/ai` — the SDK consumes
 * them and never forks them.
 */
export * from "@openagentsinc/ai-model";
