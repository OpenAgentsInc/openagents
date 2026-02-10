import { Schema } from "effect";

import type { PromptRenderStatsV1 } from "./render.js";

export type ContextPressureV1 = {
  readonly format: "openagents.dse.context_pressure";
  readonly formatVersion: 1;

  // Heuristic v1: based on rendered prompt character count.
  readonly pressure01: number;

  readonly renderedChars: number;
  readonly softLimitChars: number;

  // Debug string describing how pressure was computed.
  readonly reason: string;
};

export const ContextPressureV1Schema: Schema.Schema<ContextPressureV1> =
  Schema.Struct({
    format: Schema.Literal("openagents.dse.context_pressure"),
    formatVersion: Schema.Literal(1),
    pressure01: Schema.Number,
    renderedChars: Schema.Number,
    softLimitChars: Schema.Number,
    reason: Schema.String
  });

const clamp01 = (n: number): number => (n <= 0 ? 0 : n >= 1 ? 1 : n);

// Empirical “soft limit” varies by model and prompt composition. For v1 we use a
// conservative chars-based estimate and record the inputs so we can iterate.
const DEFAULT_SOFT_LIMIT_CHARS = 80_000;

export function contextPressureFromRenderStats(
  stats: PromptRenderStatsV1,
  options?: { readonly softLimitChars?: number | undefined }
): ContextPressureV1 {
  const renderedChars = Math.max(0, Math.floor(stats.totalChars));
  const softLimitChars = Math.max(
    1,
    Math.floor(options?.softLimitChars ?? DEFAULT_SOFT_LIMIT_CHARS)
  );
  const pressure01 = clamp01(renderedChars / softLimitChars);

  return {
    format: "openagents.dse.context_pressure",
    formatVersion: 1,
    pressure01,
    renderedChars,
    softLimitChars,
    reason: `pressure01=clamp01(renderedChars/softLimitChars); renderedChars=${renderedChars} softLimitChars=${softLimitChars}`
  };
}

