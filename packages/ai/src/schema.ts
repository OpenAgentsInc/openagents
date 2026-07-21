/**
 * L1 VOCABULARY — the neutral runtime event vocabulary.
 *
 * Re-exports `@openagentsinc/agent-runtime-schema`: `KhalaRuntimeEvent` (the
 * single neutral event union — sequence as the durable cursor, visibility,
 * redaction class, causality refs), `RuntimeInteraction`, the route schemas,
 * and the AI SDK ingestion parts. Every SDK layer speaks this vocabulary
 * upward.
 */
export * from "@openagentsinc/agent-runtime-schema";
