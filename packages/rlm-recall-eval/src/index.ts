/**
 * @openagentsinc/rlm-recall-eval — hermetic dense-recall evaluation and honesty
 * gate over OpenAgents transcript shapes (RLM-07, #9143).
 *
 * The eval CONSUMES the published `@openagentsinc/rlm` engine, corpus builder,
 * program/result Schemas, and scripted-model plan interface. It forks no eval
 * module. It adds OpenAgents-specific synthetic transcript generators, tier
 * runners, scoring, versioned price-catalog cost scoring, and the product
 * admission gates. Live-provider runs live in a separate owner-triggered CLI
 * (`live-cli.ts`) and can never be confused with hermetic results.
 */

export * from "./price-catalog.ts";
export * from "./transcripts.ts";
export * from "./scripted-models.ts";
export * from "./scoring.ts";
export * from "./tiers.ts";
export * from "./gates.ts";
export * from "./harness.ts";
